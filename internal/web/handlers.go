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
	PrereqStatus       string   `json:"prereq_status"` // "met", "recommended_missing", "missing", "none"
	MissingMandatory   []string `json:"missing_mandatory"`
	MissingRecommended []string `json:"missing_recommended"`
	IsCompleted        bool     `json:"is_completed"`
	IsBookmarked       bool     `json:"is_bookmarked"`
	IsLimited          bool     `json:"is_limited"`
}

// IndexPageData holds data passed to index.html template.
type IndexPageData struct {
	Programs          []storage.StudyProgramOption
	ProgramGroups     []storage.StudyProgramGroup
	ProgramGroupsJSON template.HTML
	Instructors       []storage.InstructorItem
	InstructorsJSON   template.HTML
	CurrentSemester   string
	DefaultTurnus     string
	NextSemesterTag   string
	TotalModules      int
	Query             string
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
	Modules             []ModuleCardViewModel
	RegularModules      []ModuleCardViewModel
	FUESModules         []ModuleCardViewModel
	HasRegular          bool
	HasFUES             bool
	RegularShowing      int
	RegularTotal        int
	FUESShowing         int
	FUESTotal           int
	HasFUESDivider      bool
	FilterModulesJSON   string
	FilterModuleIDsJSON string
	Total               int
	TotalInProgram      int
	Showing             int
	Offset              int
	HasNext             bool
	NextOffset          int
	ViewMode            string // "table"
	Query               string
	ProgramName         string
	IsAppend            bool
	IsBookmarksView     bool
	IsOpenPassedView    bool
	CompletedCredits    float64
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
	CurriculumEntries    []model.CurriculumModule
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

	// Switchover in the middle of each semester:
	// - Wintersemester (Oct-Mar): middle is Jan 1 -> months 1-6 target Sommersemester
	// - Sommersemester (Apr-Sep): middle is Jul 1 -> months 7-12 target Wintersemester
	if month >= 1 && month <= 6 {
		return "next", fmt.Sprintf("Sommersemester %d", year), fmt.Sprintf("SoSe %02d", year%100)
	}
	return "next", fmt.Sprintf("Wintersemester %d/%d", year, (year+1)%100), fmt.Sprintf("WiSe %02d", year%100)
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

func evalPrereqStatus(mandIDs, recIDs []string, completedSet map[string]bool) (status string, missingMand, missingRec []string) {
	for _, reqID := range mandIDs {
		if !completedSet[reqID] {
			missingMand = append(missingMand, reqID)
		}
	}
	for _, recID := range recIDs {
		if !completedSet[recID] {
			missingRec = append(missingRec, recID)
		}
	}

	hadPrereqs := len(mandIDs) > 0 || len(recIDs) > 0
	if len(missingMand) > 0 {
		return "missing", missingMand, missingRec
	}
	if len(missingRec) > 0 {
		return "recommended_missing", missingMand, missingRec
	}
	if hadPrereqs {
		return "met", missingMand, missingRec
	}
	return "none", missingMand, missingRec
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
	groups, _ := s.store.GetGroupedStudyPrograms()
	groupsJSON, _ := json.Marshal(groups)
	instructors, _ := s.store.GetAllInstructors()
	instructorsJSON, _ := json.Marshal(instructors)
	total, _ := s.store.Count()
	_, currentSem, nextTag := detectNextSemester()
	q := strings.TrimSpace(r.URL.Query().Get("q"))

	data := IndexPageData{
		Programs:          programs,
		ProgramGroups:     groups,
		ProgramGroupsJSON: template.HTML(groupsJSON),
		Instructors:       instructors,
		InstructorsJSON:   template.HTML(instructorsJSON),
		CurrentSemester:   currentSem,
		DefaultTurnus:     "all", // Default as requested: "Alle"
		NextSemesterTag:   nextTag,
		TotalModules:      total,
		Query:             q,
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := s.templates.ExecuteTemplate(w, "index.html", data); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}

func (s *Server) handleModules(w http.ResponseWriter, r *http.Request) {
	// If a standard browser navigates directly to /modules?... instead of HTMX fragment fetching, redirect to /?...
	if r.Header.Get("HX-Request") == "" && strings.Contains(r.Header.Get("Accept"), "text/html") {
		target := "/?" + r.URL.RawQuery
		if r.URL.RawQuery == "" {
			target = "/"
		}
		http.Redirect(w, r, target, http.StatusTemporaryRedirect)
		return
	}

	q := strings.TrimSpace(r.URL.Query().Get("q"))
	if q == "" {
		q = strings.TrimSpace(r.URL.Query().Get("search"))
	}
	programID := r.URL.Query().Get("program_id")
	if programID == "" {
		programID = r.URL.Query().Get("program")
	}
	if programID == "" {
		programID = r.URL.Query().Get("studiengang")
	}
	if programID == "" {
		programID = r.URL.Query().Get("stg")
	}
	var turnuses []string
	if tList := r.URL.Query()["turnus"]; len(tList) > 0 {
		for _, t := range tList {
			t = strings.TrimSpace(t)
			if t != "" && t != "all" {
				turnuses = append(turnuses, t)
			}
		}
	}
	if len(turnuses) == 0 {
		if tStr := r.URL.Query().Get("turnuses"); tStr != "" {
			for _, part := range strings.Split(tStr, ",") {
				part = strings.TrimSpace(part)
				if part != "" && part != "all" {
					turnuses = append(turnuses, part)
				}
			}
		}
	}

	// FÜS filter: "inkl" (default), "exkl", "nur"
	fues := strings.ToLower(strings.TrimSpace(r.URL.Query().Get("fues")))
	if fues == "" {
		if r.URL.Query().Get("only_fues") == "true" || r.URL.Query().Get("only_fues") == "1" {
			fues = "nur"
		} else {
			fues = "inkl"
		}
	}
	switch fues {
	case "exkl", "nur":
	default:
		fues = "inkl"
	}

	// Whitelist Instructors filter
	var instructors []string
	if insts := r.URL.Query()["instructor"]; len(insts) > 0 {
		for _, inst := range insts {
			inst = strings.TrimSpace(inst)
			if inst != "" {
				instructors = append(instructors, inst)
			}
		}
	}
	if len(instructors) == 0 {
		if insts := r.URL.Query().Get("instructors"); insts != "" {
			for _, part := range strings.Split(insts, ",") {
				part = strings.TrimSpace(part)
				if part != "" {
					instructors = append(instructors, part)
				}
			}
		}
	}

	onlyPrereqsMet := r.URL.Query().Get("only_prereqs_met") == "true" || r.URL.Query().Get("only_prereqs_met") == "1" ||
		r.URL.Query().Get("prereqs_met") == "true" || r.URL.Query().Get("prereqs_met") == "1"
	onlyBookmarked := r.URL.Query().Get("only_bookmarked") == "true" || r.URL.Query().Get("only_bookmarked") == "1"
	onlyCompleted := r.URL.Query().Get("only_completed") == "true" || r.URL.Query().Get("only_completed") == "1"
	language := r.URL.Query().Get("language")
	limitation := r.URL.Query().Get("limitation")
	langDE := r.URL.Query().Get("lang_de") == "true" || r.URL.Query().Get("lang_de") == "1"
	langEN := r.URL.Query().Get("lang_en") == "true" || r.URL.Query().Get("lang_en") == "1"
	var languages []string
	if langDE {
		languages = append(languages, "Deutsch")
	}
	if langEN {
		languages = append(languages, "English")
	}

	campuses := r.URL.Query()["campus"]
	if len(campuses) == 0 {
		if cp := r.URL.Query().Get("campuses"); cp != "" {
			for _, part := range strings.Split(cp, ",") {
				part = strings.TrimSpace(part)
				if part != "" {
					campuses = append(campuses, part)
				}
			}
		}
	}
	campusStrict := r.URL.Query().Get("campus_strict") == "true" || r.URL.Query().Get("campus_strict") == "1"

	viewMode := r.URL.Query().Get("view")
	if viewMode == "" {
		viewMode = "grid"
	}

	// Phase-out filter: hide_phase_out is true by default unless explicitly set to false
	hidePhaseOutParam := r.URL.Query().Get("hide_phase_out")
	hidePhaseOut := hidePhaseOutParam == "" || hidePhaseOutParam == "true" || hidePhaseOutParam == "1"

	minCredits, _ := strconv.ParseFloat(r.URL.Query().Get("min_credits"), 64)
	maxCredits, _ := strconv.ParseFloat(r.URL.Query().Get("max_credits"), 64)
	if maxCredits >= 30 {
		maxCredits = 0
	}
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

	// Resolve study program info
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

	// Special views override regular filters to show all saved/passed items
	activeTurnuses := turnuses
	activeProg := programID
	var nonAdjacentMajor string
	programMajor := ""
	if programName != "" {
		programMajor = strings.TrimSpace(strings.Split(programName, "(")[0])
	}

	if onlyBookmarked || onlyCompleted {
		activeTurnuses = nil
		activeProg = ""
	} else if fues == "nur" && programID != "" {
		activeProg = ""
		nonAdjacentMajor = programMajor
	}

	baseFilter := storage.AdvancedFilter{
		Query:            q,
		ProgramID:        activeProg,
		NonAdjacentMajor: nonAdjacentMajor,
		SemesterTurnuses: activeTurnuses,
		MinCredits:       minCredits,
		MaxCredits:       maxCredits,
		Language:         language,
		Languages:        languages,
		Campuses:         campuses,
		CampusStrict:     campusStrict,
		Limitation:       limitation,
		FUESFilter:       fues,
		Instructors:      instructors,
		ExcludePhaseOut:  hidePhaseOut,
		Limit:            limit,
		Offset:           offset,
	}

	var completedCreditsSum float64
	toViewModels := func(rawItems []storage.ModuleCardItem) []ModuleCardViewModel {
		var list []ModuleCardViewModel
		for _, it := range rawItems {
			vm := ModuleCardViewModel{
				ModuleCardItem: it,
				IsCompleted:    completedSet[it.ID],
				IsBookmarked:   bookmarkedSet[it.ID],
				IsLimited:      it.Limitation != "" && !strings.EqualFold(strings.TrimSpace(it.Limitation), "keine"),
			}

			if vm.IsCompleted {
				completedCreditsSum += it.Credits
			}

			if onlyBookmarked && !vm.IsBookmarked {
				continue
			}
			if onlyCompleted && !vm.IsCompleted {
				continue
			}

			vm.PrereqStatus, vm.MissingMandatory, vm.MissingRecommended = evalPrereqStatus(it.MandatoryPrereqIDs, it.RecommendedPrereqIDs, completedSet)
			if onlyPrereqsMet && vm.PrereqStatus == "missing" {
				continue
			}
			list = append(list, vm)
		}
		return list
	}

	var regularModules []ModuleCardViewModel
	var fuesModules []ModuleCardViewModel
	var regularShowing, regularTotal int
	var fuesShowing, fuesTotal int
	var hasRegular, hasFUES bool

	if onlyBookmarked || onlyCompleted {
		items, _, err := s.store.SearchModulesAdvanced(baseFilter)
		if err != nil {
			http.Error(w, fmt.Sprintf("Error querying modules: %v", err), http.StatusInternalServerError)
			return
		}
		regularModules = toViewModels(items)
		regularShowing = len(regularModules)
		regularTotal = len(regularModules)
		hasRegular = len(regularModules) > 0
	} else {
		// Calculate total available counts
		if programID != "" {
			regularTotal, _ = s.store.GetProgramTotalModules(programID)
			_ = s.store.DB().QueryRow(`SELECT COUNT(*) FROM modules WHERE (is_fues = 1 OR cross_disciplinary = 1) AND id NOT IN (SELECT module_id FROM module_study_programs WHERE LOWER(program_name) LIKE ?)`, "%"+strings.ToLower(programMajor)+"%").Scan(&fuesTotal)
		} else {
			totalCatalog, _ := s.store.Count()
			_ = s.store.DB().QueryRow("SELECT COUNT(*) FROM modules WHERE is_fues = 1 OR cross_disciplinary = 1").Scan(&fuesTotal)
			regularTotal = totalCatalog - fuesTotal
			if regularTotal < 0 {
				regularTotal = 0
			}
		}

		switch fues {
		case "exkl":
			regFilter := baseFilter
			regFilter.FUESFilter = "exkl"
			regItems, totalReg, _ := s.store.SearchModulesAdvanced(regFilter)
			regularModules = toViewModels(regItems)
			regularShowing = totalReg
			hasRegular = len(regularModules) > 0

		case "nur":
			fFilter := baseFilter
			fFilter.FUESFilter = "nur"
			if programID != "" {
				fFilter.ProgramID = ""
				fFilter.NonAdjacentMajor = programMajor
			}
			fItems, totalFuesFiltered, _ := s.store.SearchModulesAdvanced(fFilter)
			fuesModules = toViewModels(fItems)
			fuesShowing = totalFuesFiltered
			hasFUES = len(fuesModules) > 0

		default: // "inkl"
			// 1. Regular modules
			regFilter := baseFilter
			if programID == "" {
				regFilter.FUESFilter = "exkl"
			}
			regItems, totalReg, _ := s.store.SearchModulesAdvanced(regFilter)
			regularModules = toViewModels(regItems)
			regularShowing = totalReg
			hasRegular = len(regularModules) > 0

			// 2. FÜS modules
			fFilter := baseFilter
			fFilter.FUESFilter = "nur"
			if programID != "" {
				fFilter.ProgramID = ""
				fFilter.NonAdjacentMajor = programMajor
			}
			fItems, totalFuesFiltered, _ := s.store.SearchModulesAdvanced(fFilter)
			fuesModules = toViewModels(fItems)
			fuesShowing = totalFuesFiltered
			hasFUES = len(fuesModules) > 0
		}
	}

	allViewModels := append([]ModuleCardViewModel{}, regularModules...)
	allViewModels = append(allViewModels, fuesModules...)

	// Compile JSON list of all filtered modules for client-side autocomplete
	// Compile JSON list of all module IDs matching the active sidebar filters WITHOUT text search query q (Requirement 2 & 3)
	var filterModuleIDsJSON string
	if offset == 0 {
		filterWithoutQ := baseFilter
		filterWithoutQ.Query = ""
		filterWithoutQ.Limit = 0
		filterWithoutQ.Offset = 0

		var matchedIDs []string
		if onlyBookmarked || onlyCompleted {
			for _, vm := range regularModules {
				matchedIDs = append(matchedIDs, vm.ID)
			}
		} else {
			switch fues {
			case "exkl":
				regF := filterWithoutQ
				regF.FUESFilter = "exkl"
				matchedIDs, _ = s.store.SearchModuleIDs(regF)
			case "nur":
				fF := filterWithoutQ
				fF.FUESFilter = "nur"
				if programID != "" {
					fF.ProgramID = ""
					fF.NonAdjacentMajor = programMajor
				}
				matchedIDs, _ = s.store.SearchModuleIDs(fF)
			default: // "inkl"
				regF := filterWithoutQ
				if programID == "" {
					regF.FUESFilter = "exkl"
				}
				ids1, _ := s.store.SearchModuleIDs(regF)

				fF := filterWithoutQ
				fF.FUESFilter = "nur"
				if programID != "" {
					fF.ProgramID = ""
					fF.NonAdjacentMajor = programMajor
				}
				ids2, _ := s.store.SearchModuleIDs(fF)

				seen := make(map[string]bool)
				for _, id := range ids1 {
					if !seen[id] {
						seen[id] = true
						matchedIDs = append(matchedIDs, id)
					}
				}
				for _, id := range ids2 {
					if !seen[id] {
						seen[id] = true
						matchedIDs = append(matchedIDs, id)
					}
				}
			}
		}

		if matchedIDs == nil {
			matchedIDs = []string{}
		}
		if b, err := json.Marshal(matchedIDs); err == nil {
			filterModuleIDsJSON = string(b)
		}
	}

	data := ModuleListData{
		Modules:             allViewModels,
		RegularModules:      regularModules,
		FUESModules:         fuesModules,
		HasRegular:          hasRegular,
		HasFUES:             hasFUES,
		RegularShowing:      regularShowing,
		RegularTotal:        regularTotal,
		FUESShowing:         fuesShowing,
		FUESTotal:           fuesTotal,
		FilterModulesJSON:   filterModuleIDsJSON,
		FilterModuleIDsJSON: filterModuleIDsJSON,
		Total:               regularShowing + fuesShowing,
		Showing:             len(allViewModels),
		Offset:              offset,
		HasNext:             false,
		NextOffset:          offset,
		ViewMode:            "table",
		Query:               q,
		ProgramName:         programName,
		IsAppend:            offset > 0,
		IsBookmarksView:     onlyBookmarked,
		IsOpenPassedView:    onlyCompleted,
		CompletedCredits:    completedCreditsSum,
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := s.templates.ExecuteTemplate(w, "module_cards.html", data); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}

func (s *Server) handleAllAutocompleteAPI(w http.ResponseWriter, r *http.Request) {
	items, err := s.store.GetAllAutocompleteModules()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.Header().Set("Cache-Control", "public, max-age=300")
	json.NewEncoder(w).Encode(items)
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
			fmt.Fprintf(&sb, `<div class="suggestion-item" onclick="openModule('%s'); document.getElementById('search-suggestions').style.display='none';">
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

	// If a browser navigates directly to /modules/{id} in the address bar, redirect to /?module={id}
	if r.Header.Get("HX-Request") == "" && (r.Header.Get("Sec-Fetch-Dest") == "document" || strings.Contains(r.Header.Get("Accept"), "text/html")) {
		http.Redirect(w, r, "/?module="+id, http.StatusTemporaryRedirect)
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

	status, missingMand, missingRec := evalPrereqStatus(mandIDs, recIDs, completedSet)

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

	curEntries, _ := s.store.GetModuleCurriculumEntries(id)

	data := ModalPageData{
		Module:               detail,
		LinkedPrograms:       linkedProgs,
		ShortStudyPrograms:   shortPrograms,
		CurriculumEntries:    curEntries,
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
	if err := s.templates.ExecuteTemplate(w, "module_detail.html", data); err != nil {
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

func (s *Server) handleCurriculumAPI(w http.ResponseWriter, r *http.Request) {
	progID := r.URL.Query().Get("program_id")
	modID := r.URL.Query().Get("module_id")

	if modID != "" {
		cur, err := s.store.GetModuleCurriculumEntries(modID)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(cur)
		return
	}

	if progID != "" {
		cur, err := s.store.GetProgramCurriculum(progID)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(cur)
		return
	}

	http.Error(w, "missing program_id or module_id", http.StatusBadRequest)
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

	// 4. Refresher status & Data Freshness
	if s.refresher != nil {
		data.Refresher = s.refresher.GetStatus()
	} else if s.store != nil {
		data.Refresher.Freshness, _ = s.store.GetFreshnessStats()
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

func (s *Server) handleDBDownload(w http.ResponseWriter, r *http.Request) {
	if s.store == nil {
		http.Error(w, "Database not available", http.StatusServiceUnavailable)
		return
	}
	// Checkpoint WAL so the DB file is consistent and up to date
	_ = s.store.Checkpoint()

	dbPath := s.store.Path()
	if dbPath == "" {
		dbPath = "btu_modules.db"
	}

	fi, err := os.Stat(dbPath)
	if err != nil {
		http.Error(w, "Database file not found: "+err.Error(), http.StatusNotFound)
		return
	}

	// Cache-Control and ETag
	etag := fmt.Sprintf(`W/"%x-%x"`, fi.ModTime().UnixNano(), fi.Size())
	w.Header().Set("ETag", etag)
	w.Header().Set("Cache-Control", "public, max-age=300")
	w.Header().Set("Content-Type", "application/vnd.sqlite3")
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Expose-Headers", "Content-Length, Content-Range, ETag")

	http.ServeFile(w, r, dbPath)
}

func (s *Server) handleStatusAPI(w http.ResponseWriter, r *http.Request) {
	var totalModules int
	if s.store != nil {
		totalModules, _ = s.store.Count()
		// A completed scraper transaction may still live in WAL. Refresh the
		// snapshot timestamp before the browser compares its cached database.
		_ = s.store.Checkpoint()
	}

	var dbSize int64
	var modTime time.Time
	if s.store != nil && s.store.Path() != "" {
		if fi, err := os.Stat(s.store.Path()); err == nil {
			dbSize = fi.Size()
			modTime = fi.ModTime()
		}
	}
	etag := fmt.Sprintf(`W/"%x-%x"`, modTime.UnixNano(), dbSize)

	res := map[string]any{
		"status":         "ok",
		"version":        "2.0",
		"total_modules":  totalModules,
		"db_size_bytes":  dbSize,
		"db_last_update": modTime.Format(time.RFC3339),
		"uptime":         time.Since(s.startTime).Round(time.Second).String(),
		"database": map[string]any{
			"size_bytes":  dbSize,
			"last_update": modTime.Format(time.RFC3339),
			"etag":        etag,
		},
	}

	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.Header().Set("Access-Control-Allow-Origin", "*")
	_ = json.NewEncoder(w).Encode(res)
}
