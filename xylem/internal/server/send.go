package server

import (
	"bytes"
	"context"
	"encoding/json"
	"io"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/leonieziechmann/betula/xylem/internal/oplog"
	"github.com/leonieziechmann/betula/xylem/internal/store"
	"github.com/leonieziechmann/betula/xylem/internal/webpush"
)

// The sender: one goroutine that looks for what is due (every Limits.TickEvery, and at once
// when an announcement is made), walks an announcement through the subscriptions of its topic
// in batches, and comes back to the ones whose push service did not take it.
//
// A batch is written and recorded as one step. A Xylem that stops lets the batch under way end
// first; one that is killed in it writes that batch again when it is back (Limits.BatchSize
// subscriptions at most hear an announcement twice; where the browser knows tags, the second
// replaces the first without a sound).

// message is a declarative push message (the Push API, „Declarative push message"): Safari
// shows its notification by itself, every other browser hands it to the site's service worker,
// which shows it (docs/xylem/protocol.md section 4).
type message struct {
	WebPush      int          `json:"web_push"`
	Notification notification `json:"notification"`
}

type notification struct {
	Title     string   `json:"title"`
	Body      string   `json:"body,omitempty"`
	Navigate  string   `json:"navigate"`
	Lang      string   `json:"lang"`
	Tag       string   `json:"tag"`
	Timestamp int64    `json:"timestamp"`
	Data      noteData `json:"data"`
}

type noteData struct {
	Topic string `json:"topic"`
}

// webPushMagic says that a push message is a declarative one (the number of RFC 8030).
const webPushMagic = 8030

// pickText is the text of an announcement for a language: its own, else the one of the
// language it is a variety of (en for en-gb), else the default's.
func pickText(texts map[string]store.Text, lang, def string) (store.Text, string) {
	if t, ok := texts[lang]; ok {
		return t, lang
	}
	if base, _, ok := strings.Cut(lang, "-"); ok {
		if t, ok := texts[base]; ok {
			return t, base
		}
	}
	return texts[def], def
}

// tagOf is the tag of an announcement's notification: one announcement is one notification, and
// one told again under its key takes the place of the first.
func tagOf(a store.Announcement) string {
	if a.Key != "" {
		return a.Key
	}
	return "xylem-" + strconv.FormatInt(a.ID, 10)
}

// payload is the push message of an announcement for a browser of lang, and the language of
// its text.
func (s *Server) payload(a store.Announcement, lang string) ([]byte, string) {
	text, used := pickText(a.Texts, lang, s.lang)
	link := text.Link
	if link == "" {
		link = a.Link
	}
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	_ = enc.Encode(message{WebPush: webPushMagic, Notification: notification{
		Title: text.Title, Body: text.Body, Navigate: s.site + link, Lang: used, Tag: tagOf(a),
		Timestamp: a.SendAt.UnixMilli(), Data: noteData{Topic: a.Topic},
	}})
	return bytes.TrimRight(buf.Bytes(), "\n"), used
}

// token is a VAPID token for one push service.
type token struct {
	text  string
	renew time.Time // from here on a new one is signed
}

// tokenFor is the VAPID token for the push service of origin: signed once and used for every
// request to it until an hour before it ends.
func (s *Server) tokenFor(origin string, now time.Time) (string, error) {
	s.tokenMu.Lock()
	defer s.tokenMu.Unlock()
	if t, ok := s.tokens[origin]; ok && now.Before(t.renew) {
		return t.text, nil
	}
	life := min(s.lim.TokenLife, webpush.MaxTokenLife)
	text, err := s.key.Token(origin, s.contact, now.Add(life))
	if err != nil {
		return "", err
	}
	s.tokens[origin] = token{text: text, renew: now.Add(life - life/6)}
	return text, nil
}

func (s *Server) stopping() bool {
	select {
	case <-s.stop:
		return true
	default:
		return false
	}
}

// sendLoop runs the sender until Close.
func (s *Server) sendLoop() {
	defer s.wg.Done()
	t := time.NewTicker(s.lim.TickEvery)
	defer t.Stop()
	for {
		s.work()
		select {
		case <-s.stop:
			return
		case <-t.C:
		case <-s.wake:
		}
	}
}

// work does what is due now: the announcements whose time has come, the attempts that waited
// long enough, and it finishes what has nothing left to do.
func (s *Server) work() {
	s.workMu.Lock()
	defer s.workMu.Unlock()
	if s.stopping() {
		return
	}
	ctx := context.Background() // not s.ctx: what was sent is recorded even while Xylem stops
	s.settle(ctx)
	due, err := s.st.Due(ctx, s.now())
	if err != nil {
		s.storeFailed("due", err)
		return
	}
	for _, a := range due {
		if s.stopping() {
			return
		}
		s.walk(ctx, a)
	}
	s.retry(ctx)
	s.settle(ctx)
}

// walk writes an announcement to the subscriptions of its topic after its cursor, batch by
// batch, until it has been through all of them, its time is over, it is cancelled or Xylem
// stops.
// Log events: announcement.started.
func (s *Server) walk(ctx context.Context, a store.Announcement) {
	if a.State == store.Waiting {
		s.m.announcements.Inc("started")
		oplog.For("send").Info("an announcement is told", "event", "announcement.started", "id", a.ID, "key", a.Key, "topic", a.Topic)
	}
	for !s.stopping() {
		now := s.now()
		if !now.Before(a.Until) {
			return // settle finishes it
		}
		batch, err := s.st.Batch(ctx, a, s.lim.BatchSize)
		if err != nil {
			s.storeFailed("batch", err)
			return
		}
		if len(batch) == 0 {
			if err := s.st.Walked(ctx, a.ID, now); err != nil {
				s.storeFailed("walked", err)
				return
			}
			s.finished(ctx, a.ID)
			return
		}
		payloads := s.payloads(a, batch)
		outcomes := s.pushAll(len(batch), func(i int) store.Outcome {
			return s.push(a, batch[i], 0, payloads)
		})
		cursor := batch[len(batch)-1].Nr
		ok, err := s.st.Record(ctx, a.ID, cursor, outcomes)
		if err != nil {
			s.storeFailed("record", err)
			return
		}
		if !ok {
			return // cancelled meanwhile
		}
		a.Cursor, a.State = cursor, store.Sending
	}
}

// retry makes the attempts that are due.
func (s *Server) retry(ctx context.Context) {
	for !s.stopping() {
		list, err := s.st.Retries(ctx, s.now(), s.lim.BatchSize)
		if err != nil {
			s.storeFailed("retries", err)
			return
		}
		if len(list) == 0 {
			return
		}
		// One announcement at a time, in the order the attempts are due.
		for len(list) > 0 {
			a := list[0].Announcement
			n := 0
			for n < len(list) && list[n].Announcement.ID == a.ID {
				n++
			}
			group := list[:n]
			list = list[n:]
			targets := make([]store.Target, len(group))
			for i, r := range group {
				targets[i] = r.Target
			}
			payloads := s.payloads(a, targets)
			outcomes := s.pushAll(len(group), func(i int) store.Outcome {
				return s.push(a, group[i].Target, group[i].Attempts, payloads)
			})
			if err := s.st.Resolve(ctx, a.ID, outcomes); err != nil {
				s.storeFailed("resolve", err)
				return
			}
		}
	}
}

// settle finishes the announcements that have nothing left to do, and says so.
func (s *Server) settle(ctx context.Context) {
	settled, err := s.st.Settle(ctx, s.now())
	if err != nil {
		s.storeFailed("settle", err)
		return
	}
	for _, f := range settled {
		s.finished(ctx, f.ID)
	}
}

// finished says what became of an announcement that is finished.
// Log events: announcement.done, announcement.expired (WARN).
func (s *Server) finished(ctx context.Context, id int64) {
	a, err := s.st.Announcement(ctx, id)
	if err != nil {
		s.storeFailed("announcement", err)
		return
	}
	switch a.State {
	case store.Done:
		s.m.announcements.Inc("done")
		oplog.For("send").Info("an announcement was told", "event", "announcement.done", "id", a.ID, "key", a.Key, "topic", a.Topic,
			"sent", a.Sent, "gone", a.Gone, "failed", a.Failed, "duration_ms", a.Finished.Sub(a.SendAt).Milliseconds())
	case store.Expired:
		s.m.announcements.Inc("expired")
		oplog.For("send").Warn("an announcement was never told: Xylem did not run while it was due", "event", "announcement.expired",
			"id", a.ID, "key", a.Key, "topic", a.Topic, "at", a.SendAt.Format(time.RFC3339), "until", a.Until.Format(time.RFC3339))
	}
}

// payloads are the push messages of an announcement for the languages of some targets.
func (s *Server) payloads(a store.Announcement, targets []store.Target) map[string][]byte {
	out := map[string][]byte{}
	for _, t := range targets {
		if _, ok := out[t.Lang]; !ok {
			out[t.Lang], _ = s.payload(a, t.Lang)
		}
	}
	return out
}

// pushAll runs n pushes, Limits.Senders at a time, and returns what became of each.
func (s *Server) pushAll(n int, one func(i int) store.Outcome) []store.Outcome {
	out := make([]store.Outcome, n)
	gate := make(chan struct{}, s.lim.Senders)
	var wg sync.WaitGroup
	for i := range n {
		gate <- struct{}{}
		wg.Add(1)
		go func() {
			defer wg.Done()
			defer func() { <-gate }()
			out[i] = one(i)
		}()
	}
	wg.Wait()
	return out
}

// push writes one announcement to one subscription: the message sealed for it, to its endpoint,
// signed as Xylem's. made is how many attempts were made before this one. Nothing of it is
// logged: an endpoint is a browser's address.
func (s *Server) push(a store.Announcement, t store.Target, made int, payloads map[string][]byte) store.Outcome {
	out := store.Outcome{Sub: t.Nr, Result: store.Failed}
	now := s.now()
	ep, err := s.endpointOf(t.Endpoint)
	if err != nil { // a push service Xylem no longer writes to (its --push-hosts changed)
		s.m.pushes.Inc(otherService, "refused")
		return out
	}
	left := a.Until.Sub(now)
	if left <= 0 {
		s.m.pushes.Inc(ep.service, "gave_up")
		return out
	}
	body, err := webpush.Seal(payloads[t.Lang], t.P256dh, t.Auth)
	if err != nil {
		s.m.pushes.Inc(ep.service, "invalid")
		return out
	}
	auth, err := s.tokenFor(ep.origin, now)
	if err != nil {
		s.m.pushes.Inc(ep.service, "invalid")
		return out
	}
	ctx, cancel := context.WithTimeout(context.Background(), s.lim.SendTimeout)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, t.Endpoint, bytes.NewReader(body))
	if err != nil {
		s.m.pushes.Inc(ep.service, "invalid")
		return out
	}
	req.Header.Set("Content-Type", "application/octet-stream")
	req.Header.Set("Content-Encoding", "aes128gcm")
	// How long the push service keeps it for a device that is off: as long as it is told.
	req.Header.Set("TTL", strconv.FormatInt(int64(left/time.Second), 10))
	req.Header.Set("Urgency", a.Urgency)
	req.Header.Set("Authorization", s.key.Authorization(auth))

	began := time.Now()
	resp, err := s.client.Do(req)
	s.m.pushSeconds.Observe(time.Since(began).Seconds(), ep.service)
	status, wait := 0, time.Duration(0)
	if err == nil {
		status, wait = resp.StatusCode, retryAfter(resp.Header)
		_, _ = io.Copy(io.Discard, io.LimitReader(resp.Body, 4<<10))
		_ = resp.Body.Close()
	}
	result := "refused"
	switch {
	case err != nil, status == http.StatusTooManyRequests, status >= 500:
		// The push service did not answer, is busy or broken: another attempt later, while
		// attempts and time are left.
		made++
		if made > len(s.lim.Backoff) {
			result = "gave_up"
			break
		}
		due := now.Add(max(s.lim.Backoff[made-1], min(wait, s.lim.MaxWait)))
		if !due.Before(a.Until) {
			result = "gave_up"
			break
		}
		out.Result, out.Due, result = store.Again, due, "retry"
	case status >= 200 && status < 300:
		out.Result, result = store.Sent, "sent"
	case status == http.StatusNotFound, status == http.StatusGone:
		// The subscription is no more (the browser's data was cleared, the permission taken
		// back, the app removed): Xylem forgets it.
		out.Result, result = store.Gone, "gone"
		s.m.subs.Inc("gone", ep.service)
	case status == http.StatusUnauthorized, status == http.StatusForbidden:
		// The push service does not take Xylem's token for this subscription. Never a reason
		// to forget the subscription: a wrong clock or key of Xylem's would empty the database.
		result = "unauthorized"
		s.complain(ep.service, status)
	case status == http.StatusRequestEntityTooLarge:
		result = "too_big"
	case status == http.StatusBadRequest:
		result = "invalid"
	}
	s.m.pushes.Inc(ep.service, result)
	return out
}

// complain logs that a push service refuses Xylem's token, once an hour per service: every
// subscription of the service is refused then, and somebody has to look (the clock, the key,
// the contact address).
// Log events: push.unauthorized (ERROR).
func (s *Server) complain(service string, status int) {
	now := time.Now()
	s.mu.Lock()
	last := s.complained[service]
	if now.Sub(last) < time.Hour {
		s.mu.Unlock()
		return
	}
	s.complained[service] = now
	s.mu.Unlock()
	oplog.For("send").Error("a push service refuses Xylem's token", "event", "push.unauthorized", "service", service, "status", status)
}
