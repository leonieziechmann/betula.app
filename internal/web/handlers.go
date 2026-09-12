package web

import (
	"context"
	"encoding/json"
	"fmt"
	"html/template"
	"io"
	"net/http"
	"os"
	"runtime"
	"strconv"
	"strings"
	"time"

	"github.com/jakob/btu-scraper/internal/analytics"
	"github.com/jakob/btu-scraper/internal/logger"
	"github.com/jakob/btu-scraper/internal/model"
	"github.com/jakob/btu-scraper/internal/refresher"
	"github.com/jakob/btu-scraper/internal/storage"
)

// ModuleCardViewModel enriches ModuleCardItem with client-specific state (prerequisites, completion, bookmarks).
type ModuleCardViewModel struct {
	storage.ModuleCardItem
	PrereqStatus        string   `json:"prereq_status"` // "met", "recommended_missing", "missing", "none"
	MissingMandatory    []string `json:"missing_mandatory"`
	MissingRecommended  []string `json:"missing_recommended"`
	IsCompleted         bool     `json:"is_completed"`
	IsBookmarked        bool     `json:"is_bookmarked"`
}

// IndexPageData holds data passed to index.html template.
type IndexPageData struct {
	Programs        []storage.StudyProgramOption
	CurrentSemester string
	DefaultTurnus   string
	NextSemesterTag string
	TotalModules    int
}

// FilterAutocompleteItem is a lightweight representation of a module for instant client-side autocomplete.
type FilterAutocompleteItem struct {
	ID      string  `json:"id"`
	TitleDE string  `json:"title_de"`
	TitleEN string  `json:"title_en,omitempty"`
	Credits float64 `json:"credits"`
	Turnus  string  `json:"turnus,omitempty"`
	IsFUES  bool    `json:"is_fues,omitempty"`
	IsPhase bool    `json:"is_phase,omitempty"`
}

// ModuleListData holds data passed to module_cards.html template.
type ModuleListData struct {
	Modules           []ModuleCardViewModel
	RegularModules    []ModuleCardViewModel
	FUESModules       []ModuleCardViewModel
	HasFUESDivider    bool
	FilterModulesJSON string
	Total             int
	TotalInProgram    int
	Showing           int
	Offset            int
	HasNext           bool
	NextOffset        int
	ViewMode          string // "grid" or "table"
	Query             string
	ProgramName       string
	IsAppend          bool
	IsBookmarksView   bool
	IsOpenPassedView  bool
	CompletedCredits  float64
}

// CalendarSchedule represents an entry in the weekly visual schedule grid.
type CalendarSchedule struct {
	EventID       string  `json:"event_id"`
	Title         string  `json:"title"`
	EventType     string  `json:"event_type"`
	DayOfWeek     string  `json:"day_of_week"` // "Mo", "Di", "Mi", "Do", "Fr"
	DayIndex      int     `json:"day_index"`   // 1=Mo, 2=Di, 3=Mi, 4=Do, 5=Fr
	TimeSlot      string  `json:"time_slot"`
	StartTime     string  `json:"start_time"`
	EndTime       string  `json:"end_time"`
	TopPercent    float64 `json:"top_percent"`
	HeightPercent float64 `json:"height_percent"`
	Room          string  `json:"room"`
	Rhythm        string  `json:"rhythm"`
	Instructor    string  `json:"instructor"`
}

// ModalPageData holds data passed to module_modal.html template.
type ModalPageData struct {
	Module               *model.ModuleDetail
	LinkedPrograms       []model.OfficialStudyProgram
	ShortStudyPrograms   []string
	Events               []model.EventDetail
	CalendarSchedules    []CalendarSchedule
	HasEvents            bool
	HasCalendarSchedules bool
	IsCompleted          bool
	IsBookmarked         bool
	PrereqStatus         string
	MissingMandatory     []string
	MissingRecommended   []string
}

func detectNextSemester() (defaultTurnus, label, tag string) {
	now := time.Now()
	month := now.Month()
	year := now.Year()

	if month >= 4 && month <= 9 {
		// Next is Wintersemester starting in Oct
		return "wise", fmt.Sprintf("Wintersemester %d/%d", year, (year+1)%100), "WiSe"
	}
	// Next is Sommersemester starting in April
	targetYear := year
	if month >= 10 {
		targetYear++
	}
	return "sose", fmt.Sprintf("Sommersemester %d", targetYear), "SoSe"
}

func parseIDList(val string) map[string]bool {
	set := make(map[string]bool)
	if val == "" {
		return set
	}
	parts := strings.Split(val, ",")
	for _, p := range parts {
		p = strings.TrimSpace(p)
		if p != "" {
			set[p] = true
		}
	}
	return set
}

func (s *Server) handleIndex(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path != "/" {
		http.NotFound(w, r)
		return
	}

	if s.tracker != nil {
		s.tracker.TrackPageView()
	}

	programs, _ := s.store.GetAllStudyPrograms()
	total, _ := s.store.Count()
	defTurnus, currentSem, nextTag := detectNextSemester()

	data := IndexPageData{
		Programs:        programs,
		CurrentSemester: currentSem,
		DefaultTurnus:   defTurnus,
		NextSemesterTag: nextTag,
		TotalModules:    total,
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := s.templates.ExecuteTemplate(w, "index.html", data); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}

func (s *Server) handleModules(w http.ResponseWriter, r *http.Request) {
	q := strings.TrimSpace(r.URL.Query().Get("q"))
	programID := r.URL.Query().Get("program_id")
	turnus := r.URL.Query().Get("turnus")
	onlyFUES := r.URL.Query().Get("only_fues") == "true" || r.URL.Query().Get("only_fues") == "1"
	onlyPrereqsMet := r.URL.Query().Get("only_prereqs_met") == "true" || r.URL.Query().Get("only_prereqs_met") == "1"
	onlyBookmarked := r.URL.Query().Get("only_bookmarked") == "true" || r.URL.Query().Get("only_bookmarked") == "1"
	onlyCompleted := r.URL.Query().Get("only_completed") == "true" || r.URL.Query().Get("only_completed") == "1"
	language := r.URL.Query().Get("language")
	viewMode := r.URL.Query().Get("view")
	if viewMode == "" {
		viewMode = "grid"
	}

	// Phase-out filter: hide_phase_out is true by default unless explicitly set to false
	hidePhaseOutParam := r.URL.Query().Get("hide_phase_out")
	hidePhaseOut := hidePhaseOutParam == "" || hidePhaseOutParam == "true" || hidePhaseOutParam == "1"

	minCredits, _ := strconv.ParseFloat(r.URL.Query().Get("min_credits"), 64)
	maxCredits, _ := strconv.ParseFloat(r.URL.Query().Get("max_credits"), 64)
	offset, _ := strconv.Atoi(r.URL.Query().Get("offset"))
	if offset < 0 {
		offset = 0
	}
	limit := 36

	// Completed modules from client localStorage
	completedRaw := r.URL.Query().Get("completed")
	if completedRaw == "" {
		completedRaw = r.Header.Get("X-BTU-Completed-Modules")
	}
	completedSet := parseIDList(completedRaw)

	// Bookmarks from client localStorage
	bookmarksRaw := r.URL.Query().Get("bookmarks")
	if bookmarksRaw == "" {
		bookmarksRaw = r.Header.Get("X-BTU-Bookmarked-Modules")
	}
	bookmarkedSet := parseIDList(bookmarksRaw)

	// Special views override regular filters to show all saved/passed items
	activeTurnus := turnus
	activeProg := programID
	if onlyBookmarked || onlyCompleted {
		activeTurnus = "all"
		activeProg = ""
	}

	filter := storage.AdvancedFilter{
		Query:           q,
		ProgramID:       activeProg,
		SemesterTurnus:  activeTurnus,
		MinCredits:      minCredits,
		MaxCredits:      maxCredits,
		Language:        language,
		OnlyFUES:        onlyFUES,
		ExcludePhaseOut: hidePhaseOut,
		Limit:           limit,
		Offset:          offset,
	}

	items, total, err := s.store.SearchModulesAdvanced(filter)
	if err != nil {
		http.Error(w, fmt.Sprintf("Error querying modules: %v", err), http.StatusInternalServerError)
		return
	}

	var viewModels []ModuleCardViewModel
	var completedCreditsSum float64

	for _, it := range items {
		vm := ModuleCardViewModel{
			ModuleCardItem: it,
			IsCompleted:    completedSet[it.ID],
			IsBookmarked:   bookmarkedSet[it.ID],
		}

		if vm.IsCompleted {
			completedCreditsSum += it.Credits
		}

		// Filter for bookmarked only if in bookmark view
		if onlyBookmarked && !vm.IsBookmarked {
			continue
		}

		// Filter for completed only if in completed view
		if onlyCompleted && !vm.IsCompleted {
			continue
		}

		// Evaluate Prerequisites
		var missingMand []string
		for _, reqID := range it.MandatoryPrereqIDs {
			if !completedSet[reqID] {
				missingMand = append(missingMand, reqID)
			}
		}

		var missingRec []string
		for _, recID := range it.RecommendedPrereqIDs {
			if !completedSet[recID] {
				missingRec = append(missingRec, recID)
			}
		}

		vm.MissingMandatory = missingMand
		vm.MissingRecommended = missingRec

		hasPrereqsText := (it.PrerequisitesMandatory != "" && !strings.EqualFold(strings.TrimSpace(it.PrerequisitesMandatory), "keine")) ||
			(it.PrerequisitesRecommended != "" && !strings.EqualFold(strings.TrimSpace(it.PrerequisitesRecommended), "keine"))

		if len(missingMand) > 0 {
			vm.PrereqStatus = "missing"
		} else if len(missingRec) > 0 {
			vm.PrereqStatus = "recommended_missing"
		} else if hasPrereqsText {
			vm.PrereqStatus = "met"
		} else {
			vm.PrereqStatus = "none"
		}

		// If user only wants modules whose prerequisites are satisfied, skip missing ones
		if onlyPrereqsMet && vm.PrereqStatus == "missing" {
			continue
		}

		viewModels = append(viewModels, vm)
	}

	// Calculate total modules in program
	totalInProgram, _ := s.store.GetProgramTotalModules(programID)

	programName := ""
	if programID != "" {
		progs, _ := s.store.GetAllStudyPrograms()
		for _, p := range progs {
			if p.ID == programID {
				programName = p.ShortTitle
				break
			}
		}
		if s.tracker != nil && offset == 0 {
			s.tracker.TrackProgramSelect(programID, programName)
		}
	}

	var regularModules []ModuleCardViewModel
	var fuesModules []ModuleCardViewModel
	hasFUESDivider := false

	// Requirement 5: Abgrenzung von Modulen, die zum Studiengang gehören, und darunter die FÜS-Module
	if programID != "" && !onlyBookmarked && !onlyCompleted && !onlyFUES && offset == 0 {
		programMajor := strings.TrimSpace(strings.Split(programName, "(")[0])
		fuesFilter := storage.AdvancedFilter{
			OnlyFUES:         true,
			NonAdjacentMajor: programMajor,
			SemesterTurnus:   activeTurnus,
			MinCredits:       minCredits,
			MaxCredits:       maxCredits,
			Language:         language,
			ExcludePhaseOut:  hidePhaseOut,
			Limit:            100,
		}
		fuesItems, _, _ := s.store.SearchModulesAdvanced(fuesFilter)
		for _, it := range fuesItems {
			fvm := ModuleCardViewModel{
				ModuleCardItem: it,
				IsCompleted:    completedSet[it.ID],
				IsBookmarked:   bookmarkedSet[it.ID],
			}
			if fvm.IsCompleted {
				completedCreditsSum += it.Credits
			}
			// Evaluate prerequisites for FÜS module
			var missingMand []string
			for _, reqID := range it.MandatoryPrereqIDs {
				if !completedSet[reqID] {
					missingMand = append(missingMand, reqID)
				}
			}
			var missingRec []string
			for _, recID := range it.RecommendedPrereqIDs {
				if !completedSet[recID] {
					missingRec = append(missingRec, recID)
				}
			}
			fvm.MissingMandatory = missingMand
			fvm.MissingRecommended = missingRec

			hasPrereqsText := (it.PrerequisitesMandatory != "" && !strings.EqualFold(strings.TrimSpace(it.PrerequisitesMandatory), "keine")) ||
				(it.PrerequisitesRecommended != "" && !strings.EqualFold(strings.TrimSpace(it.PrerequisitesRecommended), "keine"))

			if len(missingMand) > 0 {
				fvm.PrereqStatus = "missing"
			} else if len(missingRec) > 0 {
				fvm.PrereqStatus = "recommended_missing"
			} else if hasPrereqsText {
				fvm.PrereqStatus = "met"
			} else {
				fvm.PrereqStatus = "none"
			}

			if onlyPrereqsMet && fvm.PrereqStatus == "missing" {
				continue
			}
			fuesModules = append(fuesModules, fvm)
		}

		if len(fuesModules) > 0 {
			hasFUESDivider = true
			regularModules = viewModels
		}
	}

	// Requirement 1: Compile JSON list of all filtered modules for instant client-side autocomplete
	var filterModulesJSON string
	if offset == 0 {
		var autocompleteItems []FilterAutocompleteItem
		seenID := make(map[string]bool)

		for _, vm := range viewModels {
			if !seenID[vm.ID] {
				seenID[vm.ID] = true
				autocompleteItems = append(autocompleteItems, FilterAutocompleteItem{
					ID:      vm.ID,
					TitleDE: vm.TitleDE,
					TitleEN: vm.TitleEN,
					Credits: vm.Credits,
					Turnus:  vm.Turnus,
					IsFUES:  vm.IsFUES,
					IsPhase: vm.IsPhaseOut,
				})
			}
		}

		for _, vm := range fuesModules {
			if !seenID[vm.ID] {
				seenID[vm.ID] = true
				autocompleteItems = append(autocompleteItems, FilterAutocompleteItem{
					ID:      vm.ID,
					TitleDE: vm.TitleDE,
					TitleEN: vm.TitleEN,
					Credits: vm.Credits,
					Turnus:  vm.Turnus,
					IsFUES:  vm.IsFUES,
					IsPhase: vm.IsPhaseOut,
				})
			}
		}

		// If total > len(viewModels), fetch remaining module titles so search covers the entire filtered scope
		if total > len(viewModels) {
			fullFilter := filter
			fullFilter.Limit = 1500
			fullFilter.Offset = 0
			allFiltered, _, _ := s.store.SearchModulesAdvanced(fullFilter)
			for _, it := range allFiltered {
				if !seenID[it.ID] {
					seenID[it.ID] = true
					autocompleteItems = append(autocompleteItems, FilterAutocompleteItem{
						ID:      it.ID,
						TitleDE: it.TitleDE,
						TitleEN: it.TitleEN,
						Credits: it.Credits,
						Turnus:  it.Turnus,
						IsFUES:  it.IsFUES,
						IsPhase: it.IsPhaseOut,
					})
				}
			}
		}

		if b, err := json.Marshal(autocompleteItems); err == nil {
			filterModulesJSON = string(b)
		}
	}

	data := ModuleListData{
		Modules:           viewModels,
		RegularModules:    regularModules,
		FUESModules:       fuesModules,
		HasFUESDivider:    hasFUESDivider,
		FilterModulesJSON: filterModulesJSON,
		Total:             total,
		TotalInProgram:    totalInProgram,
		Showing:           len(viewModels),
		Offset:            offset,
		HasNext:           offset+limit < total,
		NextOffset:        offset + limit,
		ViewMode:          viewMode,
		Query:             q,
		ProgramName:       programName,
		IsAppend:          offset > 0,
		IsBookmarksView:   onlyBookmarked,
		IsOpenPassedView:  onlyCompleted,
		CompletedCredits:  completedCreditsSum,
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := s.templates.ExecuteTemplate(w, "module_cards.html", data); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}

func (s *Server) handleSuggestionsAPI(w http.ResponseWriter, r *http.Request) {
	q := strings.TrimSpace(r.URL.Query().Get("q"))
	if len(q) < 2 {
		w.Header().Set("Content-Type", "text/html")
		w.Write([]byte(""))
		return
	}

	filter := storage.AdvancedFilter{
		Query: q,
		Limit: 8,
	}
	items, _, err := s.store.SearchModulesAdvanced(filter)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	var sb strings.Builder
	if len(items) == 0 {
		sb.WriteString(`<div class="suggestion-empty">Keine Module gefunden</div>`)
	} else {
		for _, it := range items {
			cred := it.CreditsRaw
			if cred == "" {
				cred = fmt.Sprintf("%.1f", it.Credits)
			}
			turnusText := it.Turnus
			if turnusText == "" {
				turnusText = "Kein Turnus"
			}
			fmt.Fprintf(&sb, `<div class="suggestion-item" onclick="openModal('%s'); document.getElementById('search-suggestions').style.display='none';">
				<div class="suggestion-main">
					<span class="badge badge-id">%s</span>
					<strong class="suggestion-title">%s</strong>
				</div>
				<div class="suggestion-meta">%s ECTS • %s</div>
			</div>`, it.ID, it.ID, template.HTMLEscapeString(it.TitleDE), cred, template.HTMLEscapeString(turnusText))
		}
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Write([]byte(sb.String()))
}

func (s *Server) handleModuleModal(w http.ResponseWriter, r *http.Request) {
	id := strings.TrimPrefix(r.URL.Path, "/modules/")
	if id == "" {
		http.NotFound(w, r)
		return
	}

	// Anonymous GDPR-compliant telemetry
	if s.tracker != nil {
		s.tracker.TrackModuleClick(id)
	}

	detail, err := s.store.GetModule(id)
	// Cache miss detection: module not in DB or details not yet scraped
	if err != nil || detail == nil || (detail.Contents == "" && detail.LearningOutcomes == "") {
		if s.refresher != nil {
			ctx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
			fetched, fetchErr := s.refresher.FetchModuleNow(ctx, id)
			cancel()
			if fetchErr == nil && fetched != nil {
				detail = fetched
				err = nil
			} else {
				// Queue for polite background crawler
				s.refresher.EnqueueCacheMiss(id)
			}
		}
	}

	if err != nil || detail == nil {
		http.Error(w, "Module not found", http.StatusNotFound)
		return
	}

	linkedProgs, _ := s.store.GetLinkedStudyProgramsForModule(id)

	// Build compact short titles for study programs
	seenShort := make(map[string]bool)
	var shortPrograms []string
	for _, lp := range linkedProgs {
		st := storage.FormatProgramShort(lp.ProgramName, lp.Degree)
		if !seenShort[st] {
			seenShort[st] = true
			shortPrograms = append(shortPrograms, st)
		}
	}
	for _, sp := range detail.StudyPrograms {
		if sp.Program != "" {
			st := storage.FormatProgramShort(sp.Program, sp.Degree)
			if !seenShort[st] {
				seenShort[st] = true
				shortPrograms = append(shortPrograms, st)
			}
		}
	}

	completedRaw := r.URL.Query().Get("completed")
	if completedRaw == "" {
		completedRaw = r.Header.Get("X-BTU-Completed-Modules")
	}
	completedSet := parseIDList(completedRaw)

	bookmarksRaw := r.URL.Query().Get("bookmarks")
	if bookmarksRaw == "" {
		bookmarksRaw = r.Header.Get("X-BTU-Bookmarked-Modules")
	}
	bookmarkedSet := parseIDList(bookmarksRaw)

	mandIDs := storage.ExtractPrereqIDs(detail.PrerequisitesMandatory)
	recIDs := storage.ExtractPrereqIDs(detail.PrerequisitesRecommended)

	var missingMand []string
	for _, reqID := range mandIDs {
		if !completedSet[reqID] {
			missingMand = append(missingMand, reqID)
		}
	}
	var missingRec []string
	for _, recID := range recIDs {
		if !completedSet[recID] {
			missingRec = append(missingRec, recID)
		}
	}

	status := "none"
	hasText := (detail.PrerequisitesMandatory != "" && !strings.EqualFold(strings.TrimSpace(detail.PrerequisitesMandatory), "keine")) ||
		(detail.PrerequisitesRecommended != "" && !strings.EqualFold(strings.TrimSpace(detail.PrerequisitesRecommended), "keine"))

	if len(missingMand) > 0 {
		status = "missing"
	} else if len(missingRec) > 0 {
		status = "recommended_missing"
	} else if hasText {
		status = "met"
	}

	// Fetch module events and extract recurring schedules for weekly calendar
	events, _ := s.store.GetEventsForModule(id)
	if len(events) == 0 && len(detail.CurrentSemesterEvents) > 0 && s.eventProv != nil {
		ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
		_, _ = s.eventProv.ScrapeEventsForModule(ctx, id, false)
		cancel()
		events, _ = s.store.GetEventsForModule(id)
	}

	var calendarSchedules []CalendarSchedule

	for _, evt := range events {
		isExam := strings.Contains(strings.ToLower(evt.EventType), "prüfung") ||
			strings.Contains(strings.ToLower(evt.Title), "prüfung")
		if isExam {
			continue // exclude pure exams from the recurring weekly timetable
		}

		for _, sc := range evt.Schedules {
			dayIdx := parseDayOfWeek(sc.DayOfWeek)
			if dayIdx < 1 || dayIdx > 5 {
				continue
			}

			startDec, endDec := parseTimeRange(sc.StartTime, sc.EndTime, sc.TimeSlot)
			if startDec < 8.0 {
				startDec = 8.0
			}
			if endDec > 20.0 {
				endDec = 20.0
			}
			if endDec <= startDec {
				endDec = startDec + 1.5
			}

			// Calendar span: 08:00 to 20:00 (12 hours)
			topPct := (startDec - 8.0) / 12.0 * 100.0
			heightPct := (endDec - startDec) / 12.0 * 100.0

			calendarSchedules = append(calendarSchedules, CalendarSchedule{
				EventID:       evt.ID,
				Title:         evt.Title,
				EventType:     evt.EventType,
				DayOfWeek:     formatDayName(dayIdx),
				DayIndex:      dayIdx,
				TimeSlot:      sc.TimeSlot,
				StartTime:     sc.StartTime,
				EndTime:       sc.EndTime,
				TopPercent:    topPct,
				HeightPercent: heightPct,
				Room:          sc.Room,
				Rhythm:        sc.Rhythm,
				Instructor:    sc.Instructor,
			})
		}
	}

	data := ModalPageData{
		Module:               detail,
		LinkedPrograms:       linkedProgs,
		ShortStudyPrograms:   shortPrograms,
		Events:               events,
		CalendarSchedules:    calendarSchedules,
		HasEvents:            len(events) > 0 || len(detail.CurrentSemesterEvents) > 0,
		HasCalendarSchedules: len(calendarSchedules) > 0,
		IsCompleted:          completedSet[id],
		IsBookmarked:         bookmarkedSet[id],
		PrereqStatus:         status,
		MissingMandatory:     missingMand,
		MissingRecommended:   missingRec,
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := s.templates.ExecuteTemplate(w, "module_modal.html", data); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}

func parseDayOfWeek(day string) int {
	low := strings.ToLower(strings.TrimSpace(day))
	switch {
	case strings.HasPrefix(low, "mo"):
		return 1
	case strings.HasPrefix(low, "di"):
		return 2
	case strings.HasPrefix(low, "mi"):
		return 3
	case strings.HasPrefix(low, "do"):
		return 4
	case strings.HasPrefix(low, "fr"):
		return 5
	default:
		return 0
	}
}

func formatDayName(idx int) string {
	days := []string{"", "Mo", "Di", "Mi", "Do", "Fr"}
	if idx >= 1 && idx <= 5 {
		return days[idx]
	}
	return ""
}

func parseTimeRange(startStr, endStr, slotStr string) (float64, float64) {
	if startStr != "" && endStr != "" {
		s := parseDecimalHour(startStr)
		e := parseDecimalHour(endStr)
		if s > 0 && e > 0 {
			return s, e
		}
	}
	// Try parsing "10:00 bis 11:30"
	if strings.Contains(slotStr, "bis") {
		parts := strings.Split(slotStr, "bis")
		if len(parts) == 2 {
			s := parseDecimalHour(strings.TrimSpace(parts[0]))
			e := parseDecimalHour(strings.TrimSpace(parts[1]))
			if s > 0 && e > 0 {
				return s, e
			}
		}
	}
	return 10.0, 11.5
}

func parseDecimalHour(t string) float64 {
	parts := strings.Split(strings.TrimSpace(t), ":")
	if len(parts) >= 2 {
		h, _ := strconv.ParseFloat(parts[0], 64)
		m, _ := strconv.ParseFloat(parts[1], 64)
		return h + (m / 60.0)
	}
	return 0
}

func (s *Server) handleProgramsAPI(w http.ResponseWriter, r *http.Request) {
	progs, err := s.store.GetAllStudyPrograms()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(progs)
}

func (s *Server) handleStatsAPI(w http.ResponseWriter, r *http.Request) {
	data := s.collectStatsData()
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(data)
}

func (s *Server) handleStatsPage(w http.ResponseWriter, r *http.Request) {
	data := s.collectStatsData()
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := s.templates.ExecuteTemplate(w, "stats.html", data); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}

func (s *Server) handleTrackAPI(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}
	if s.tracker == nil {
		w.WriteHeader(http.StatusNoContent)
		return
	}

	var payload struct {
		Type       analytics.EventType `json:"type"`
		TargetID   string              `json:"target_id"`
		TargetName string              `json:"target_name"`
	}

	// Strictly limit payload size to avoid memory abuse
	if err := json.NewDecoder(io.LimitReader(r.Body, 1024)).Decode(&payload); err != nil {
		http.Error(w, "Invalid tracking payload", http.StatusBadRequest)
		return
	}

	// Anonymous GDPR-compliant event: No client IP, no user agent, no cookies stored
	s.tracker.Track(analytics.Event{
		Type:       payload.Type,
		TargetID:   payload.TargetID,
		TargetName: payload.TargetName,
	})

	w.WriteHeader(http.StatusNoContent)
}

func (s *Server) handleLogsAPI(w http.ResponseWriter, r *http.Request) {
	var logs []logger.LogEntry
	if s.logger != nil {
		logs = s.logger.GetRecentLogs(100, logger.LevelDebug)
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(logs)
}

// StatsViewData bundles data for the /stats HTML dashboard and /api/stats JSON.
type StatsViewData struct {
	Catalog struct {
		TotalModules  int `json:"total_modules"`
		FUESModules   int `json:"fues_modules"`
		TotalPrograms int `json:"total_programs"`
		TotalEvents   int `json:"total_events"`
	} `json:"catalog"`
	Analytics analytics.Summary `json:"analytics"`
	Health    struct {
		ConsecutiveErrors uint64 `json:"consecutive_scrape_errors"`
		TotalScrapeErrors uint64 `json:"total_scrape_errors"`
		TotalRequests     uint64 `json:"total_http_requests"`
		TotalReqErrors    uint64 `json:"total_http_errors"`
	} `json:"health"`
	Refresher refresher.Status `json:"refresher"`
	System    struct {
		Uptime          string `json:"uptime"`
		Goroutines      int    `json:"goroutines"`
		AllocMB         string `json:"alloc_mb"`
		MainDBSize      string `json:"main_db_size"`
		AnalyticsDBSize string `json:"analytics_db_size"`
	} `json:"system"`
	Logs []logger.LogEntry `json:"logs,omitempty"`
}

func (s *Server) collectStatsData() StatsViewData {
	var data StatsViewData

	// 1. Catalog metrics
	count, _ := s.store.Count()
	data.Catalog.TotalModules = count
	progs, _ := s.store.GetAllStudyPrograms()
	data.Catalog.TotalPrograms = len(progs)

	fuesCount := 0
	_ = s.store.DB().QueryRow("SELECT COUNT(*) FROM modules WHERE is_fues = 1 OR cross_disciplinary = 1").Scan(&fuesCount)
	data.Catalog.FUESModules = fuesCount

	var eventCount int
	_ = s.store.DB().QueryRow("SELECT COUNT(*) FROM events").Scan(&eventCount)
	data.Catalog.TotalEvents = eventCount

	// 2. Analytics metrics
	if s.tracker != nil {
		summary, _ := s.tracker.GetSummary()
		// Enrich top modules with titles from store
		for i := range summary.TopModules {
			if mod, err := s.store.GetModule(summary.TopModules[i].ModuleID); err == nil && mod != nil {
				summary.TopModules[i].Title = mod.TitleDE
			}
		}
		data.Analytics = summary
	}

	// 3. Logger / Health metrics
	if s.logger != nil {
		h := s.logger.GetHealthStats()
		data.Health.ConsecutiveErrors = h["consecutive_scrape_errors"].(uint64)
		data.Health.TotalScrapeErrors = h["total_scrape_errors"].(uint64)
		data.Health.TotalRequests = h["total_http_requests"].(uint64)
		data.Health.TotalReqErrors = h["total_http_errors"].(uint64)
		data.Logs = s.logger.GetRecentLogs(100, logger.LevelDebug)
	}

	// 4. Refresher status
	if s.refresher != nil {
		data.Refresher = s.refresher.GetStatus()
	}

	// 5. System metrics
	var mem runtime.MemStats
	runtime.ReadMemStats(&mem)
	data.System.Goroutines = runtime.NumGoroutine()
	data.System.AllocMB = fmt.Sprintf("%.1f", float64(mem.Alloc)/(1024*1024))
	data.System.Uptime = time.Since(s.startTime).Round(time.Second).String()
	data.System.MainDBSize = getFileSize("btu_modules.db")
	data.System.AnalyticsDBSize = getFileSize("btu_analytics.db")

	return data
}

func getFileSize(path string) string {
	fi, err := os.Stat(path)
	if err != nil {
		return "N/A"
	}
	mb := float64(fi.Size()) / (1024 * 1024)
	if mb < 1.0 {
		return fmt.Sprintf("%.1f KB", float64(fi.Size())/1024)
	}
	return fmt.Sprintf("%.2f MB", mb)
}
