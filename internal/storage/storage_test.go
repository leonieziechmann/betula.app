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
		ResponsiblePersons: []model.ResponsiblePerson{{Title: "Prof.", Name: "Euler", Raw: "Prof. Euler"}},
		IsNotOffered:       true,
		SuccessorModules:   []string{"1002"},
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
	if !retrieved.IsNotOffered {
		t.Errorf("expected IsNotOffered to be true")
	}
	if len(retrieved.SuccessorModules) != 1 || retrieved.SuccessorModules[0] != "1002" {
		t.Errorf("unexpected successor modules: %v", retrieved.SuccessorModules)
	}
	if len(retrieved.ResponsiblePersons) != 1 || retrieved.ResponsiblePersons[0].Name != "Euler" || retrieved.ResponsiblePersons[0].Title != "Prof." {
		t.Errorf("unexpected responsible persons: %+v", retrieved.ResponsiblePersons)
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

func TestAdvancedFiltersAndGroupedPrograms(t *testing.T) {
	tempDir, err := os.MkdirTemp("", "btu_adv_test_*")
	if err != nil {
		t.Fatalf("failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tempDir)

	dbPath := filepath.Join(tempDir, "adv_test.db")
	store, err := NewStorage(dbPath)
	if err != nil {
		t.Fatalf("failed to initialize storage: %v", err)
	}
	defer store.Close()

	// Insert modules with diverse turnus, limitation, languages
	modules := []*model.ModuleDetail{
		{
			ID:         "M1",
			Code:       "M1",
			TitleDE:    "Algorithmen",
			Language:   "Deutsch",
			Turnus:     "jedes Sommersemester gerader Jahre",
			Limitation: "keine",
			Credits:    6.0,
			ResponsiblePersons: []model.ResponsiblePerson{
				{Name: "Köhler, Ekkehard", Title: "Prof. Dr."},
			},
		},
		{
			ID:         "M2",
			Code:       "M2",
			TitleDE:    "Data Science",
			Language:   "English",
			Turnus:     "jedes Wintersemester ungerader Jahre",
			Limitation: "25",
			Credits:    6.0,
			ResponsiblePersons: []model.ResponsiblePerson{
				{Name: "Borchers, Wolfgang", Title: "Dr."},
			},
		},
		{
			ID:         "M3",
			Code:       "M3",
			TitleDE:    "Spezialkurs",
			Language:   "Deutsch",
			Turnus:     "sporadisch nach Ankündigung",
			Limitation: "",
			Credits:    3.0,
			IsFUES:     true,
			ResponsiblePersons: []model.ResponsiblePerson{
				{Name: "Köhler, Ekkehard", Title: "Prof. Dr."},
			},
		},
	}

	for _, m := range modules {
		if err := store.UpsertModuleDetail(m); err != nil {
			t.Fatalf("failed to upsert module %s: %v", m.ID, err)
		}
	}

	// Insert events with rooms for campus testing
	// Event 1: Hauptcampus
	e1 := &model.EventDetail{
		ID:          "EV1",
		EventNumber: "E101",
		Title:       "Vorlesung Algorithmen",
		Schedules: []model.EventSchedule{
			{Room: "Hauptgebäude - HG 3.45 - Zentralcampus"},
		},
		AssociatedModules: []string{"M1"},
	}
	if err := store.UpsertEvent(e1); err != nil {
		t.Fatalf("failed to upsert event 1: %v", err)
	}

	// Event 2: Senftenberg
	e2 := &model.EventDetail{
		ID:          "EV2",
		EventNumber: "E102",
		Title:       "Labor Data Science",
		Schedules: []model.EventSchedule{
			{Room: "Gebäude 11 - Hörsaal SFB - 11.122 Grosser Hörsaal - Campus Senftenberg"},
		},
		AssociatedModules: []string{"M2"},
	}
	if err := store.UpsertEvent(e2); err != nil {
		t.Fatalf("failed to upsert event 2: %v", err)
	}

	// 1. Test Turnus
	_, totalSoseEven, err := store.SearchModulesAdvanced(AdvancedFilter{SemesterTurnus: "sose_even"})
	if err != nil || totalSoseEven != 1 {
		t.Errorf("expected 1 module for sose_even, got %d, err=%v", totalSoseEven, err)
	}

	_, totalWiseOdd, err := store.SearchModulesAdvanced(AdvancedFilter{SemesterTurnus: "wise_odd"})
	if err != nil || totalWiseOdd != 1 {
		t.Errorf("expected 1 module for wise_odd, got %d, err=%v", totalWiseOdd, err)
	}

	_, totalSporadic, err := store.SearchModulesAdvanced(AdvancedFilter{SemesterTurnus: "sporadic"})
	if err != nil || totalSporadic != 1 {
		t.Errorf("expected 1 module for sporadic, got %d, err=%v", totalSporadic, err)
	}

	// Multi-select turnuses: WiSe Ungerade + SoSe Gerade + Sporadisch
	_, totalCombo, err := store.SearchModulesAdvanced(AdvancedFilter{
		SemesterTurnuses: []string{"wise_odd", "sose_even", "sporadic"},
	})
	if err != nil || totalCombo != 3 {
		t.Errorf("expected 3 modules for wise_odd + sose_even + sporadic, got %d, err=%v", totalCombo, err)
	}

	// Just WiSe Ungerade + Sporadisch
	_, totalTwo, err := store.SearchModulesAdvanced(AdvancedFilter{
		SemesterTurnuses: []string{"wise_odd", "sporadic"},
	})
	if err != nil || totalTwo != 2 {
		t.Errorf("expected 2 modules for wise_odd + sporadic, got %d, err=%v", totalTwo, err)
	}

	// 2. Test Limitation
	// "nein": unbeschränkt (M1 with "keine" and M3 with "")
	_, totalUnlim, err := store.SearchModulesAdvanced(AdvancedFilter{Limitation: "nein"})
	if err != nil || totalUnlim != 2 {
		t.Errorf("expected 2 unbeschränkte modules, got %d, err=%v", totalUnlim, err)
	}

	// "nur": teilnehmerbeschränkt (M2 with "25")
	_, totalLim, err := store.SearchModulesAdvanced(AdvancedFilter{Limitation: "nur"})
	if err != nil || totalLim != 1 {
		t.Errorf("expected 1 teilnehmerbeschränkt module, got %d, err=%v", totalLim, err)
	}

	// 3. Test Languages
	_, totalDE, err := store.SearchModulesAdvanced(AdvancedFilter{Languages: []string{"Deutsch"}})
	if err != nil || totalDE != 2 {
		t.Errorf("expected 2 German modules, got %d, err=%v", totalDE, err)
	}

	_, totalEN, err := store.SearchModulesAdvanced(AdvancedFilter{Languages: []string{"English"}})
	if err != nil || totalEN != 1 {
		t.Errorf("expected 1 English module, got %d, err=%v", totalEN, err)
	}

	// 4. Test Campuses
	_, totalHC, err := store.SearchModulesAdvanced(AdvancedFilter{Campuses: []string{"hauptcampus"}})
	if err != nil || totalHC != 1 {
		t.Errorf("expected 1 module at hauptcampus, got %d, err=%v", totalHC, err)
	}

	_, totalSFB, err := store.SearchModulesAdvanced(AdvancedFilter{Campuses: []string{"senftenberg"}, CampusStrict: true})
	if err != nil || totalSFB != 1 {
		t.Errorf("expected 1 module strictly at senftenberg, got %d, err=%v", totalSFB, err)
	}

	// 5. Test FUESFilter
	_, totalFuesInkl, err := store.SearchModulesAdvanced(AdvancedFilter{FUESFilter: "inkl"})
	if err != nil || totalFuesInkl != 3 {
		t.Errorf("expected 3 modules for fues inkl, got %d, err=%v", totalFuesInkl, err)
	}
	_, totalFuesExkl, err := store.SearchModulesAdvanced(AdvancedFilter{FUESFilter: "exkl"})
	if err != nil || totalFuesExkl != 2 {
		t.Errorf("expected 2 modules for fues exkl, got %d, err=%v", totalFuesExkl, err)
	}
	_, totalFuesNur, err := store.SearchModulesAdvanced(AdvancedFilter{FUESFilter: "nur"})
	if err != nil || totalFuesNur != 1 {
		t.Errorf("expected 1 module for fues nur, got %d, err=%v", totalFuesNur, err)
	}

	// 6. Test Instructors Whitelist
	_, totalProfKoehler, err := store.SearchModulesAdvanced(AdvancedFilter{Instructors: []string{"Köhler, Ekkehard"}})
	if err != nil || totalProfKoehler != 2 {
		t.Errorf("expected 2 modules for Köhler, got %d, err=%v", totalProfKoehler, err)
	}
	_, totalProfBoth, err := store.SearchModulesAdvanced(AdvancedFilter{Instructors: []string{"Köhler, Ekkehard", "Borchers, Wolfgang"}})
	if err != nil || totalProfBoth != 3 {
		t.Errorf("expected 3 modules for Köhler OR Borchers, got %d, err=%v", totalProfBoth, err)
	}

	// 7. Test GetAllInstructors
	allInsts, err := store.GetAllInstructors()
	if err != nil || len(allInsts) != 2 {
		t.Errorf("expected 2 instructors from GetAllInstructors, got %d, err=%v", len(allInsts), err)
	}

	// 8. Test StudyProgramGroup and GetGroupedStudyPrograms
	p1 := model.OfficialStudyProgram{
		ID:          "stg1_po2024",
		ProgramName: "Informatik",
		Degree:      "Bachelor (universitär)",
		POVersion:   "2024",
	}
	p2 := model.OfficialStudyProgram{
		ID:          "stg1_po2019",
		ProgramName: "Informatik",
		Degree:      "Bachelor (universitär)",
		POVersion:   "2019",
	}
	_ = store.UpsertOfficialProgram(&p1)
	_ = store.UpsertOfficialProgram(&p2)

	// p3 has the same POVersion (2024) but is an erweiterte Fachsemester variant degree
	p3 := model.OfficialStudyProgram{
		ID:          "stg1_po2024_erweitert",
		ProgramName: "Informatik",
		Degree:      "Bachelor (universitär) - erweiterte Fachsemester",
		POVersion:   "2024",
	}
	_ = store.UpsertOfficialProgram(&p3)

	// Architecture Master with Doppelabschluss variant sharing PO 2022 - 1. SÄ 2023
	arch1 := model.OfficialStudyProgram{
		ID:          "arch_master_std",
		ProgramName: "Architektur",
		Degree:      "Master (universitär)",
		POVersion:   "2022 - 1. SÄ 2023",
	}
	arch2 := model.OfficialStudyProgram{
		ID:          "arch_master_doppel",
		ProgramName: "Architektur",
		Degree:      "Master (universitär) - Doppelabschluss",
		POVersion:   "2022 - 1. SÄ 2023",
	}
	_ = store.UpsertOfficialProgram(&arch1)
	_ = store.UpsertOfficialProgram(&arch2)

	_ = store.LinkModuleToStudyProgram("M1", arch1.ID, arch1.ProgramName, arch1.Degree, arch1.POVersion)
	_ = store.LinkModuleToStudyProgram("M2", arch2.ID, arch2.ProgramName, arch2.Degree, arch2.POVersion)

	groups, err := store.GetGroupedStudyPrograms()
	if err != nil {
		t.Fatalf("GetGroupedStudyPrograms failed: %v", err)
	}
	if len(groups) != 2 {
		t.Fatalf("expected 2 grouped study programs, got %d", len(groups))
	}

	var infoGroup, archGroup *StudyProgramGroup
	for i := range groups {
		if groups[i].Key == "Informatik (Bachelor)" {
			infoGroup = &groups[i]
		} else if groups[i].Key == "Architektur (Master)" {
			archGroup = &groups[i]
		}
	}

	if infoGroup == nil {
		t.Fatalf("Informatik group missing")
	}
	if len(infoGroup.POs) != 2 {
		t.Errorf("expected 2 deduplicated POs in Informatik group, got %d", len(infoGroup.POs))
	}
	// Check that canonical ID is preferred (stg1_po2024 instead of stg1_po2024_dual)
	for _, po := range infoGroup.POs {
		if po.POVersion == "2024" {
			if po.ID != "stg1_po2024" {
				t.Errorf("expected canonical ID stg1_po2024, got %s", po.ID)
			}
			if len(po.RelatedIDs) != 2 {
				t.Errorf("expected 2 related IDs for PO 2024, got %v", po.RelatedIDs)
			}
		}
	}

	if archGroup == nil {
		t.Fatalf("Architektur group missing")
	}
	if len(archGroup.POs) != 1 {
		t.Errorf("expected exactly 1 deduplicated PO in Architektur Master, got %d", len(archGroup.POs))
	} else {
		if archGroup.POs[0].POVersion != "2022 - 1. SÄ 2023" {
			t.Errorf("expected PO '2022 - 1. SÄ 2023', got %s", archGroup.POs[0].POVersion)
		}
		if archGroup.POs[0].ID != "arch_master_std" {
			t.Errorf("expected canonical ID arch_master_std, got %s", archGroup.POs[0].ID)
		}
		if archGroup.POs[0].Count != 2 {
			t.Errorf("expected 2 linked modules across both variants for Architektur Master, got %d", archGroup.POs[0].Count)
		}
	}

	// Test SearchModulesAdvanced matching across related IDs
	_, totalArch, err := store.SearchModulesAdvanced(AdvancedFilter{ProgramID: "arch_master_std"})
	if err != nil || totalArch != 2 {
		t.Errorf("expected 2 modules when searching with arch_master_std, got %d, err=%v", totalArch, err)
	}
}

func TestRealDBNoDuplicatePOs(t *testing.T) {
	dbPath := "../../btu_modules.db"
	if _, err := os.Stat(dbPath); os.IsNotExist(err) {
		t.Skip("btu_modules.db not found, skipping real db test")
	}

	store, err := NewStorage(dbPath)
	if err != nil {
		t.Fatalf("failed to open real db: %v", err)
	}
	defer store.Close()

	groups, err := store.GetGroupedStudyPrograms()
	if err != nil {
		t.Fatalf("failed to get grouped study programs: %v", err)
	}

	for _, g := range groups {
		seen := make(map[string]int)
		for _, po := range g.POs {
			seen[po.POVersion]++
			if seen[po.POVersion] > 1 {
				t.Errorf("group %q has duplicate PO version %q", g.ShortTitle, po.POVersion)
			}
		}
		if g.ShortTitle == "Architektur (Master)" {
			if len(g.POs) != 1 {
				t.Errorf("expected Architektur (Master) to have exactly 1 PO, got %d", len(g.POs))
			} else if g.POs[0].POVersion != "2022 - 1. SÄ 2023" {
				t.Errorf("expected Architektur (Master) PO to be '2022 - 1. SÄ 2023', got %q", g.POs[0].POVersion)
			}
		}
	}
}

