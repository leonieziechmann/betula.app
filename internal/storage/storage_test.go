package storage

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/jakob/btu-scraper/internal/model"
)

func TestStorage(t *testing.T) {
	tempDir, err := os.MkdirTemp("", "btu_storage_test_*")
	if err != nil {
		t.Fatalf("failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tempDir)

	dbPath := filepath.Join(tempDir, "test.db")
	store, err := NewStorage(dbPath)
	if err != nil {
		t.Fatalf("failed to initialize storage: %v", err)
	}
	defer store.Close()

	// 1. Test UpsertCatalog
	summaries := []model.ModuleSummary{
		{ID: "1001", Code: "1001", Title: "Mathematik I", URL: "https://b-tu.de/modul/1001", ScrapedAt: time.Now()},
		{ID: "1002", Code: "1002", Title: "Informatik I", URL: "https://b-tu.de/modul/1002", ScrapedAt: time.Now()},
	}

	if err := store.UpsertCatalog(summaries); err != nil {
		t.Fatalf("UpsertCatalog failed: %v", err)
	}

	count, err := store.Count()
	if err != nil || count != 2 {
		t.Fatalf("expected count 2, got %d, err=%v", count, err)
	}

	// 2. Test UpsertModuleDetail
	detail := &model.ModuleDetail{
		ID:                 "1001",
		Code:               "1001",
		TitleDE:            "Mathematik I (Vertieft)",
		TitleEN:            "Mathematics I",
		Department:         "Fakultät 1",
		ResponsiblePersons: []string{"Prof. Euler"},
		Language:           "Deutsch",
		Credits:            6.0,
		CreditsRaw:         "6",
		TeachingForms: []model.TeachingForm{
			{Type: "Vorlesung", Workload: "4 SWS"},
		},
		StudyPrograms: []model.StudyProgram{
			{Degree: "B.Sc.", Program: "Informatik", Regulation: "PO 2024"},
		},
		RawURL:        "https://b-tu.de/modul/1001",
		LastScrapedAt: time.Now(),
	}

	if err := store.UpsertModuleDetail(detail); err != nil {
		t.Fatalf("UpsertModuleDetail failed: %v", err)
	}

	// 3. Test GetModule
	retrieved, err := store.GetModule("1001")
	if err != nil {
		t.Fatalf("GetModule failed: %v", err)
	}
	if retrieved.TitleDE != "Mathematik I (Vertieft)" {
		t.Errorf("expected updated title 'Mathematik I (Vertieft)', got %s", retrieved.TitleDE)
	}
	if retrieved.TitleEN != "Mathematics I" {
		t.Errorf("expected TitleEN 'Mathematics I', got %s", retrieved.TitleEN)
	}
	if retrieved.Credits != 6.0 {
		t.Errorf("expected credits 6.0, got %f", retrieved.Credits)
	}
	if len(retrieved.TeachingForms) != 1 || retrieved.TeachingForms[0].Type != "Vorlesung" {
		t.Errorf("unexpected teaching forms: %+v", retrieved.TeachingForms)
	}
	if len(retrieved.StudyPrograms) != 1 || retrieved.StudyPrograms[0].Program != "Informatik" {
		t.Errorf("unexpected study programs: %+v", retrieved.StudyPrograms)
	}

	// 4. Test ListModules with Filter
	results, err := store.ListModules(Filter{Query: "Informatik"})
	if err != nil {
		t.Fatalf("ListModules failed: %v", err)
	}
	if len(results) != 1 || results[0].ID != "1002" {
		t.Fatalf("expected 1 result with ID 1002, got %d results: %+v", len(results), results)
	}

	// 5. Test Filter by MinCredits
	results, err = store.ListModules(Filter{MinCredits: 5.0})
	if err != nil {
		t.Fatalf("ListModules by MinCredits failed: %v", err)
	}
	if len(results) != 1 || results[0].ID != "1001" {
		t.Fatalf("expected module 1001, got %v", results)
	}

	// 6. Test UpsertEvent & GetEvent
	event := &model.EventDetail{
		ID:          "147828",
		EventNumber: "140037",
		Title:       "Vorlesung Informatik 1",
		EventType:   "Vorlesung",
		Semester:    "SS 2026",
		SWS:         "2",
		ResponsiblePersons: []model.EventResponsiblePerson{
			{Name: "Prof. Weigert", Role: "verantwort"},
		},
		AssociatedModules: []string{"1001"},
		Schedules: []model.EventSchedule{
			{
				GroupName: "Gruppe 1",
				DayOfWeek: "Di.",
				TimeSlot:  "10:00 bis 11:30",
				StartTime: "10:00",
				EndTime:   "11:30",
				Room:      "Gebäude 6 - SFB - 6.210",
			},
		},
		RawURL:        "https://b-tu.de/qis?veranstid=147828",
		LastScrapedAt: time.Now(),
	}

	if err := store.UpsertEvent(event); err != nil {
		t.Fatalf("UpsertEvent failed: %v", err)
	}

	retEvent, err := store.GetEvent("147828")
	if err != nil {
		t.Fatalf("GetEvent failed: %v", err)
	}
	if retEvent.Title != "Vorlesung Informatik 1" {
		t.Errorf("expected title 'Vorlesung Informatik 1', got %s", retEvent.Title)
	}
	if len(retEvent.Schedules) != 1 {
		t.Fatalf("expected 1 schedule, got %d", len(retEvent.Schedules))
	}
	if retEvent.Schedules[0].Room != "Gebäude 6 - SFB - 6.210" {
		t.Errorf("unexpected room: %s", retEvent.Schedules[0].Room)
	}
	if retEvent.Schedules[0].DayOfWeek != "Di." {
		t.Errorf("unexpected day: %s", retEvent.Schedules[0].DayOfWeek)
	}

	// 7. Test GetEventsForModule
	modEvents, err := store.GetEventsForModule("1001")
	if err != nil {
		t.Fatalf("GetEventsForModule failed: %v", err)
	}
	if len(modEvents) != 1 || modEvents[0].ID != "147828" {
		t.Fatalf("expected 1 event with ID 147828 for module 1001, got %v", modEvents)
	}

	// 8. Test UpsertFUESList
	fuesItems := []model.FUESModule{
		{
			ID:         "1001", // already in DB, has Informatik major
			Title:      "Mathematik I (Vertieft)",
			Language:   "Deutsch",
			Credits:    6.0,
			CreditsRaw: "6",
			IsFUES:     true,
			Limitation: "keine",
			QISURL:     "https://qis/1001",
			ScrapedAt:  time.Now(),
		},
		{
			ID:         "2001", // new module, e.g. Philosophy / Architecture
			Title:      "Philosophie und Ethik",
			Language:   "Deutsch",
			Credits:    3.0,
			CreditsRaw: "3",
			IsFUES:     true,
			Limitation: "max 30",
			QISURL:     "https://qis/2001",
			ScrapedAt:  time.Now(),
		},
	}
	if err := store.UpsertFUESList(fuesItems); err != nil {
		t.Fatalf("UpsertFUESList failed: %v", err)
	}

	// Give 2001 study programs (Architecture, not Informatik)
	detail2001 := &model.ModuleDetail{
		ID:                "2001",
		Code:              "2001",
		TitleDE:           "Philosophie und Ethik",
		Credits:           3.0,
		IsFUES:            true,
		CrossDisciplinary: true,
		StudyPrograms: []model.StudyProgram{
			{Degree: "B.A.", Program: "Kultur und Technik", Regulation: "PO 2020"},
		},
		LastScrapedAt: time.Now(),
	}
	if err := store.UpsertModuleDetail(detail2001); err != nil {
		t.Fatalf("UpsertModuleDetail 2001 failed: %v", err)
	}

	// 9. Test ListMajors
	majors, err := store.ListMajors()
	if err != nil {
		t.Fatalf("ListMajors failed: %v", err)
	}
	if len(majors) < 2 {
		t.Fatalf("expected at least 2 majors, got %d: %v", len(majors), majors)
	}

	// 10. Test GetFUESForMajor
	// For "Informatik": module 1001 is adjacent (Informatik) so it should be EXCLUDED.
	// Module 2001 is Kultur und Technik, so it should be INCLUDED.
	eligibleForCS, err := store.GetFUESForMajor("Informatik", 0)
	if err != nil {
		t.Fatalf("GetFUESForMajor failed: %v", err)
	}
	found2001 := false
	for _, m := range eligibleForCS {
		if m.ID == "1001" {
			t.Errorf("expected 1001 to be excluded for Informatik, but it was included")
		}
		if m.ID == "2001" {
			found2001 = true
		}
	}
	if !found2001 {
		t.Errorf("expected 2001 to be eligible for Informatik")
	}

	// Test with minCredits filter: 4.0 should exclude 2001 (which has 3.0 credits)
	filtered, err := store.GetFUESForMajor("Informatik", 4.0)
	if err != nil {
		t.Fatalf("GetFUESForMajor with minCredits failed: %v", err)
	}
	if len(filtered) != 0 {
		t.Errorf("expected 0 modules with minCredits 4.0, got %d", len(filtered))
	}

	// 11. Test Official Study Programs Storage
	sampleProg := &model.OfficialStudyProgram{
		ID:          "stg_749_abschl_88_pversion_2019",
		ProgramName: "Angewandte Mathematik",
		ProgramCode: "749",
		Degree:      "Master (universitär)",
		DegreeCode:  "88",
		POVersion:   "2019 - 1. SÄ 2021",
		QISNodeID:   "auswahlBaum|studiengang:stg=749",
		QISURL:      "https://qis/tree/749",
		Documents: []model.ProgramRegulationDocument{
			{
				Title:          "Prüfungsordnung ABl. 25/2019",
				DocType:        "statute",
				URL:            "https://opus4.kobv.de/opus4-btu/files/5006/AMbl-25_2019_AnMa_M.Sc.pdf",
				DownloadStatus: "blocked_bot_checker",
			},
		},
		ScrapedAt: time.Now(),
	}

	if err := store.UpsertOfficialProgram(sampleProg); err != nil {
		t.Fatalf("UpsertOfficialProgram failed: %v", err)
	}

	retProg, err := store.GetOfficialProgram("stg_749_abschl_88_pversion_2019")
	if err != nil {
		t.Fatalf("GetOfficialProgram failed: %v", err)
	}
	if retProg.ProgramName != "Angewandte Mathematik" {
		t.Errorf("expected program name 'Angewandte Mathematik', got %s", retProg.ProgramName)
	}
	if len(retProg.Documents) != 1 || retProg.Documents[0].DocType != "statute" {
		t.Errorf("unexpected documents in retrieved program: %+v", retProg.Documents)
	}

	progs, err := store.ListOfficialPrograms("Mathematik", "")
	if err != nil {
		t.Fatalf("ListOfficialPrograms failed: %v", err)
	}
	if len(progs) != 1 || progs[0].ID != sampleProg.ID {
		t.Errorf("unexpected list results: %+v", progs)
	}

	// 12. Test FindOfficialProgram and LinkModuleToStudyProgram
	found, err := store.FindOfficialProgram("Angewandte Mathematik", "Master (universitär)", "PO 2019")
	if err != nil {
		t.Fatalf("FindOfficialProgram failed: %v", err)
	}
	if found.ID != sampleProg.ID {
		t.Errorf("expected found ID %s, got %s", sampleProg.ID, found.ID)
	}

	if err := store.LinkModuleToStudyProgram("1001", sampleProg.ID, sampleProg.ProgramName, sampleProg.Degree, "PO 2019"); err != nil {
		t.Fatalf("LinkModuleToStudyProgram failed: %v", err)
	}

	linkedProgs, err := store.GetLinkedStudyProgramsForModule("1001")
	if err != nil {
		t.Fatalf("GetLinkedStudyProgramsForModule failed: %v", err)
	}
	if len(linkedProgs) != 1 || linkedProgs[0].ID != sampleProg.ID {
		t.Errorf("unexpected linked programs: %+v", linkedProgs)
	}

	linkedMods, err := store.GetModulesForStudyProgram(sampleProg.ID)
	if err != nil {
		t.Fatalf("GetModulesForStudyProgram failed: %v", err)
	}
	if len(linkedMods) != 1 || linkedMods[0].ID != "1001" {
		t.Errorf("unexpected linked modules: %+v", linkedMods)
	}
}
