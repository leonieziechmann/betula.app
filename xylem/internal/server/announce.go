package server

import (
	"errors"
	"fmt"
	"net/http"
	"net/url"
	"regexp"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/leonieziechmann/betula/xylem/internal/oplog"
	"github.com/leonieziechmann/betula/xylem/internal/store"
	"github.com/leonieziechmann/betula/xylem/internal/webpush"
)

// The commands' side of the API (cmd/xylem: status, announce, announcements, cancel): plain
// HTTP for whoever is on this machine, which in the container is nobody but `xylem` itself.
// serveHTTP has checked that the request comes from here.

// AnnounceRequest is the body of POST /announcements.
type AnnounceRequest struct {
	// Key is the sender's own name for the announcement (letters, digits, . _ : -; 64 at
	// most): one made again under a key that is taken is not made twice. "": none.
	Key string `json:"key,omitempty"`
	// Topic is who hears it: the subscriptions that listen to it.
	Topic string `json:"topic"`
	// At is when it is told; nil: now.
	At *time.Time `json:"at,omitempty"`
	// For is how long it is told, as a duration ("24h", "90m"): a device that was off hears it
	// when it comes back within it, and nobody after. "": a day.
	For string `json:"for,omitempty"`
	// Urgency is the push message's (very-low, low, normal, high); "": normal.
	Urgency string `json:"urgency,omitempty"`
	// Link is the path on the site its notification opens; "": "/".
	Link string `json:"link,omitempty"`
	// Texts are its title and body by language; the default language's must be there. A text's
	// own link wins over Link (a page that has another address in that language).
	Texts map[string]store.Text `json:"texts"`
	// Dry: check it and say whom it would reach, keep nothing.
	Dry bool `json:"dry,omitempty"`
}

// AnnouncementView is an announcement as the commands read it.
type AnnouncementView struct {
	ID       int64                 `json:"id,omitempty"`
	Key      string                `json:"key,omitempty"`
	Topic    string                `json:"topic"`
	At       time.Time             `json:"at"`
	Until    time.Time             `json:"until"`
	Urgency  string                `json:"urgency"`
	Link     string                `json:"link"`
	Texts    map[string]store.Text `json:"texts"`
	Source   string                `json:"source"` // who made it: command (a person) or feed
	State    string                `json:"state"`
	Sent     int64                 `json:"sent"`
	Gone     int64                 `json:"gone"`
	Failed   int64                 `json:"failed"`
	Created  *time.Time            `json:"created,omitempty"`
	Finished *time.Time            `json:"finished,omitempty"`
}

// AnnounceResponse is the answer of POST /announcements.
type AnnounceResponse struct {
	Announcement AnnouncementView `json:"announcement"`
	// Made: kept by this request (false: a dry run, or its key was taken and this is the one
	// that has it).
	Made bool `json:"made"`
	// Reach is how many subscriptions listen to its topic now, by language.
	Reach map[string]int64 `json:"reach"`
}

// Status is the answer of GET /status.
type Status struct {
	Version       string           `json:"version"`
	Build         string           `json:"build,omitempty"`
	Stack         string           `json:"stack,omitempty"`
	Started       time.Time        `json:"started"`
	Site          string           `json:"site"`
	Lang          string           `json:"lang"` // the default language
	Key           string           `json:"key"`  // the public half of the VAPID key
	Subscriptions int64            `json:"subscriptions"`
	Max           int64            `json:"max_subscriptions"`
	Topics        map[string]int64 `json:"topics"`
	Languages     map[string]int64 `json:"languages"`
	// Announcements counts the ones not finished, by state; Retries the attempts that wait.
	Announcements map[string]int64 `json:"announcements"`
	Retries       int64            `json:"retries"`
	Next          *time.Time       `json:"next,omitempty"` // when the next one that waits is due
	// Feed is the feed Betula's own announcements are read from; none without one.
	Feed *FeedStatus `json:"feed,omitempty"`
	// Refused are the hosts of endpoints refused since the start as no push service's Xylem
	// writes to, with how often: a browser that could not subscribe (--push-hosts takes more).
	Refused map[string]int64 `json:"refused,omitempty"`
}

func view(a store.Announcement) AnnouncementView {
	v := AnnouncementView{
		ID: a.ID, Key: a.Key, Topic: a.Topic, At: a.SendAt, Until: a.Until, Urgency: a.Urgency, Link: a.Link, Texts: a.Texts,
		Source: a.Source, State: a.State, Sent: a.Sent, Gone: a.Gone, Failed: a.Failed,
	}
	if v.Source == "" {
		v.Source = store.ByCommand
	}
	if !a.Created.IsZero() {
		v.Created = &a.Created
	}
	if !a.Finished.IsZero() {
		v.Finished = &a.Finished
	}
	return v
}

func (s *Server) serveLocal(w http.ResponseWriter, r *http.Request) {
	path, method := r.URL.Path, r.Method
	id, hasID := strings.CutPrefix(path, "/announcements/")
	switch {
	case path == "/status" && method == http.MethodGet:
		s.status(w, r)
	case path == "/announcements" && method == http.MethodGet:
		s.listAnnouncements(w, r)
	case path == "/announcements" && method == http.MethodPost:
		s.announce(w, r)
	case hasID && method == http.MethodDelete:
		s.cancelAnnouncement(w, r, id)
	case path == "/feed" && method == http.MethodPost:
		s.readFeedNow(w, r)
	default:
		writeError(w, http.StatusMethodNotAllowed, "method")
	}
}

func (s *Server) status(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	topics, langs, err := s.st.Listeners(ctx)
	if err != nil {
		s.internal(w, "listeners", err)
		return
	}
	pending, retries, err := s.st.Pending(ctx)
	if err != nil {
		s.internal(w, "pending", err)
		return
	}
	st := Status{
		Version: s.opt.Version, Build: s.opt.Build, Stack: s.opt.Stack, Started: s.started, Site: s.site, Lang: s.lang, Key: s.key.Public(),
		Subscriptions: s.st.Subscriptions(), Max: s.max, Topics: topics, Languages: langs,
		Announcements: pending, Retries: retries, Feed: s.feedStatus(),
	}
	if at, ok, err := s.st.Next(ctx); err != nil {
		s.internal(w, "next", err)
		return
	} else if ok {
		st.Next = &at
	}
	s.mu.Lock()
	if len(s.unknown) > 0 {
		st.Refused = make(map[string]int64, len(s.unknown))
		for host, n := range s.unknown {
			st.Refused[host] = n
		}
	}
	s.mu.Unlock()
	writeJSON(w, http.StatusOK, st)
}

// listAnnouncements is GET /announcements: the newest, 50 unless ?limit= says otherwise.
func (s *Server) listAnnouncements(w http.ResponseWriter, r *http.Request) {
	limit := 50
	if n, err := strconv.Atoi(r.URL.Query().Get("limit")); err == nil && n > 0 && n <= 1000 {
		limit = n
	}
	list, err := s.st.Announcements(r.Context(), limit)
	if err != nil {
		s.internal(w, "announcements", err)
		return
	}
	out := make([]AnnouncementView, 0, len(list))
	for _, a := range list {
		out = append(out, view(a))
	}
	writeJSON(w, http.StatusOK, map[string]any{"lang": s.lang, "announcements": out})
}

// refuse answers 400 with what is wrong, for the person at the command.
func refuse(w http.ResponseWriter, format string, args ...any) {
	writeJSON(w, http.StatusBadRequest, map[string]string{"error": "invalid", "detail": fmt.Sprintf(format, args...)})
}

var keyName = regexp.MustCompile(`^[A-Za-z0-9._:-]{1,64}$`)

// urgencies are the urgencies of a push message (RFC 8030 section 5.3).
var urgencies = map[string]bool{"very-low": true, "low": true, "normal": true, "high": true}

// announce is POST /announcements: an announcement is checked and kept, and the sender looks
// for what is due at once.
// Log events: announcement.made.
func (s *Server) announce(w http.ResponseWriter, r *http.Request) {
	var req AnnounceRequest
	if !s.readBody(w, r, &req) {
		return
	}
	now := s.now()
	a, problem, _ := s.check(req, now)
	if problem != "" {
		refuse(w, "%s", problem)
		return
	}
	reach, err := s.st.Reach(r.Context(), a.Topic)
	if err != nil {
		s.internal(w, "reach", err)
		return
	}
	if req.Dry {
		a.State = store.Waiting
		writeJSON(w, http.StatusOK, AnnounceResponse{Announcement: view(a), Reach: reach})
		return
	}
	kept, made, err := s.st.Announce(r.Context(), a, now)
	if err != nil {
		s.internal(w, "announce", err)
		return
	}
	if made {
		s.m.announcements.Inc("made")
		oplog.For("announce").Info("an announcement was made", "event", "announcement.made", "id", kept.ID, "key", kept.Key,
			"topic", kept.Topic, "at", kept.SendAt.Format(time.RFC3339), "until", kept.Until.Format(time.RFC3339))
		select {
		case s.wake <- struct{}{}:
		default:
		}
	} else {
		s.m.announcements.Inc("again")
	}
	status := http.StatusOK
	if made {
		status = http.StatusCreated
	}
	writeJSON(w, status, AnnounceResponse{Announcement: view(kept), Made: made, Reach: reach})
}

// check reads a request into an announcement, or says what is wrong with it; over: nothing
// but that its time is over.
func (s *Server) check(req AnnounceRequest, now time.Time) (a store.Announcement, problem string, over bool) {
	a, problem = s.checkParts(req, now)
	if problem == "" && !a.Until.After(now) {
		life := a.Until.Sub(a.SendAt)
		return a, fmt.Sprintf("its time is over: told from %s for %s, it ended %s", a.SendAt.Format(time.RFC3339), life, a.Until.Format(time.RFC3339)), true
	}
	return a, problem, false
}

// checkParts is check without the look at the clock.
func (s *Server) checkParts(req AnnounceRequest, now time.Time) (store.Announcement, string) {
	var a store.Announcement
	if req.Key != "" && !keyName.MatchString(req.Key) {
		return a, "key: letters, digits and . _ : - only, 64 at most"
	}
	if !topicName.MatchString(req.Topic) {
		return a, fmt.Sprintf("topic %q: lower-case letters, digits and hyphens, a letter first, 32 at most", req.Topic)
	}
	a.Key, a.Topic = req.Key, req.Topic
	a.Urgency = req.Urgency
	if a.Urgency == "" {
		a.Urgency = "normal"
	}
	if !urgencies[a.Urgency] {
		return a, fmt.Sprintf("urgency %q: very-low, low, normal or high", req.Urgency)
	}
	a.Link = req.Link
	if a.Link == "" {
		a.Link = "/"
	}
	if why := s.checkLink(a.Link); why != "" {
		return a, "link: " + why
	}
	life := s.lim.DefaultLife
	if req.For != "" {
		d, err := time.ParseDuration(req.For)
		if err != nil {
			return a, fmt.Sprintf("for %q: a duration such as 24h or 90m", req.For)
		}
		life = d
	}
	if life < s.lim.MinLife || life > s.lim.MaxLife {
		return a, fmt.Sprintf("for %s: between %s and %s (no push service keeps a message longer)", life, s.lim.MinLife, s.lim.MaxLife)
	}
	a.SendAt = now
	if req.At != nil {
		a.SendAt = *req.At
	}
	a.SendAt = a.SendAt.Truncate(time.Millisecond)
	a.Until = a.SendAt.Add(life)
	if len(req.Texts) == 0 {
		return a, "texts: none"
	}
	if len(req.Texts) > s.lim.MaxLanguages {
		return a, fmt.Sprintf("texts: %d languages, %d at most", len(req.Texts), s.lim.MaxLanguages)
	}
	if _, ok := req.Texts[s.lang]; !ok {
		return a, fmt.Sprintf("texts: none in %q, the language everybody reads who has none of their own", s.lang)
	}
	a.Texts = make(map[string]store.Text, len(req.Texts))
	for lang, text := range req.Texts {
		if !langName.MatchString(lang) {
			return a, fmt.Sprintf("texts: %q is no language (de, en, pt-br)", lang)
		}
		text.Title, text.Body = strings.TrimSpace(text.Title), strings.TrimSpace(text.Body)
		if text.Title == "" {
			return a, fmt.Sprintf("texts: %q has no title", lang)
		}
		if n := utf8.RuneCountInString(text.Title); n > s.lim.MaxTitle {
			return a, fmt.Sprintf("texts: the title in %q has %d characters, %d at most", lang, n, s.lim.MaxTitle)
		}
		if n := utf8.RuneCountInString(text.Body); n > s.lim.MaxBody {
			return a, fmt.Sprintf("texts: the body in %q has %d characters, %d at most", lang, n, s.lim.MaxBody)
		}
		if text.Link != "" {
			if why := s.checkLink(text.Link); why != "" {
				return a, fmt.Sprintf("texts: the link in %q: %s", lang, why)
			}
		}
		a.Texts[lang] = text
	}
	for lang := range a.Texts {
		if payload, _ := s.payload(a, lang); len(payload) > webpush.MaxPlaintext {
			return a, fmt.Sprintf("texts: the notification in %q is %d bytes, a push message carries %d", lang, len(payload), webpush.MaxPlaintext)
		}
	}
	return a, ""
}

// checkLink says what is wrong with a link: it is a path of the site, so that a notification
// of Betula's never opens another site.
func (s *Server) checkLink(link string) string {
	if len(link) > s.lim.MaxLink {
		return fmt.Sprintf("%d bytes, %d at most", len(link), s.lim.MaxLink)
	}
	if !strings.HasPrefix(link, "/") || strings.HasPrefix(link, "//") || strings.ContainsAny(link, "\\ \t\r\n") {
		return "a path of the site, beginning with one /"
	}
	if u, err := url.Parse(link); err != nil || u.Scheme != "" || u.Host != "" {
		return "a path of the site, beginning with one /"
	}
	return ""
}

// cancelAnnouncement is DELETE /announcements/<id>: nobody else gets it. 404 not_found, 409
// finished.
// Log events: announcement.cancelled.
func (s *Server) cancelAnnouncement(w http.ResponseWriter, r *http.Request, raw string) {
	id, err := strconv.ParseInt(raw, 10, 64)
	if err != nil || id <= 0 {
		writeError(w, http.StatusNotFound, "not_found")
		return
	}
	a, err := s.st.Cancel(r.Context(), id, s.now())
	switch {
	case errors.Is(err, store.ErrNotFound):
		writeError(w, http.StatusNotFound, "not_found")
	case errors.Is(err, store.ErrFinished):
		writeError(w, http.StatusConflict, "finished")
	case err != nil:
		s.internal(w, "cancel", err)
	default:
		s.m.announcements.Inc("cancelled")
		oplog.For("announce").Info("an announcement was cancelled", "event", "announcement.cancelled", "id", a.ID, "key", a.Key,
			"topic", a.Topic, "sent", a.Sent)
		writeJSON(w, http.StatusOK, map[string]any{"announcement": view(a)})
	}
}
