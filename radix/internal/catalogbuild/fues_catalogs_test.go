package catalogbuild

import (
	"fmt"
	"strings"
	"testing"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/fuescatalog"
)

// TestFUESListsFollowTheCatalogs: with the BTU's FÜS catalogs a program's FÜS list is, per season,
// the newest catalog less the program's own modules less what its negative list excludes for every
// student of it (docs/radix/schema-v2.md, „FÜS catalogs"). In the fixture the winter's catalog
// holds ERP (11152), 14037 and Lineare Algebra (11101, Informatik B.Sc.'s own); Informatik's
// negative list excludes ERP for the Anwendungsfach Physik and, for the M.Sc., altogether. The
// summer's holds 14037 and excludes it for every student. A catalog not published yet (404) is
// none, one that cannot be read is left out.
func TestFUESListsFollowTheCatalogs(t *testing.T) {
	restore := readFUESCatalog
	defer func() { readFUESCatalog = restore }()
	catalogs := map[string]*fuescatalog.Catalog{
		"winter": {Semester: "2026W", Issued: "2026-10-14",
			Modules:  []fuescatalog.Module{{ID: "11152", Campus: "zentralcampus"}, {ID: "14037"}, {ID: "11101"}, {ID: "99999"}},
			Programs: []string{"Informatik", "Astrologie"},
			Exclusions: []fuescatalog.Exclusion{
				{Program: "Informatik", Scope: "Anwendungsfach Physik", ModuleID: "11152"},
				{Program: "Informatik", Scope: "für M.Sc", ModuleID: "11152"},
				{Program: "Informatik", Scope: "Unabhängig vom Anwendungsfach", ModuleID: "88888"},
			},
			Notes: []fuescatalog.Note{{Program: "Informatik", Text: "Bitte bei der Studiengangleitung erfragen!"}},
		},
		"summer": {Semester: "2026S", Issued: "2026-03-24",
			Modules:    []fuescatalog.Module{{ID: "14037"}},
			Programs:   []string{"Informatik"},
			Exclusions: []fuescatalog.Exclusion{{Program: "Informatik", Scope: "Unabhängig vom Anwendungsfach", ModuleID: "14037"}},
		},
	}
	readFUESCatalog = func(body []byte) (*fuescatalog.Catalog, error) {
		if c, ok := catalogs[string(body)]; ok {
			return c, nil
		}
		return nil, fmt.Errorf("not a catalog")
	}
	db, report := buildFixtureWith(t, func(put func(source, key, body string)) {
		put(catalogdb.SourceBTUFUESCatalog, "2026W", "winter")
		put(catalogdb.SourceBTUFUESCatalog, "2026S", "summer")
		put(catalogdb.SourceBTUFUESCatalog, "2027S", "") // 404: not published yet
		put(catalogdb.SourceBTUFUESCatalog, "2025W", "garbage")
	})
	if report.FUESCatalogs != 2 || report.FUESModulesUnknown["99999"] != 1 || report.FUESModulesUnknown["88888"] != 1 || len(report.FUESCatalogsUnreadable) != 1 {
		t.Errorf("catalogs %d, unknown %v, unreadable %v", report.FUESCatalogs, report.FUESModulesUnknown, report.FUESCatalogsUnreadable)
	}

	want(t, db, `SELECT semester, season, issued, modules, is_newest FROM v_fues_catalog ORDER BY semester`,
		"2026S|summer|2026-03-24|1|1", "2026W|winter|2026-10-14|3|1")
	want(t, db, `SELECT semester, heading, programs FROM fues_catalog_heading ORDER BY 1, 2`,
		"2026S|Informatik|2", "2026W|Astrologie|0", "2026W|Informatik|2")
	want(t, db, `SELECT semester, program_id, IFNULL(note, '∅') FROM fues_catalog_program ORDER BY 1, 2`,
		"2026S|079-82-2008|∅", "2026S|079-88-2008|∅",
		"2026W|079-82-2008|Bitte bei der Studiengangleitung erfragen!", "2026W|079-88-2008|Bitte bei der Studiengangleitung erfragen!")
	want(t, db, `SELECT semester, program_id, module_id FROM fues_negative ORDER BY 1, 2, 3`,
		"2026S|079-82-2008|14037", "2026S|079-88-2008|14037", "2026W|079-88-2008|11152")
	want(t, db, `SELECT semester, program_id, module_id, choice FROM fues_negative_choice ORDER BY 1, 2, 3, 4`,
		"2026W|079-82-2008|11152|Anwendungsfach Physik", "2026W|079-88-2008|11152|Anwendungsfach Physik")

	// B.Sc.: ERP by its page and the winter's catalog, with a note; 14037 by the winter's catalog
	// alone, the summer's excludes it; its own Lineare Algebra stays its own.
	want(t, db, `SELECT module_id, relation, IFNULL(kind, '∅'), in_tree, on_module_page, in_plan FROM v_program_module WHERE program_id = '079-82-2008' ORDER BY module_id`,
		"11101|curricular|compulsory|1|1|1",
		"11152|fues|fues|0|1|0",
		"11881|curricular|elective|1|1|1",
		"12999|curricular|thesis|1|0|0",
		"14037|fues|fues|0|0|0")
	// M.Sc.: its negative list excludes ERP, which its page names: off the list. Lineare Algebra,
	// not its own, is on it by the catalog, though QIS does not flag it.
	want(t, db, `SELECT module_id, relation FROM v_program_module WHERE program_id = '079-88-2008' ORDER BY module_id`,
		"11101|fues", "11881|curricular", "14037|fues")
	want(t, db, `SELECT program_id, module_id, season, semester, admitted, IFNULL(excluded_for, '∅') FROM v_program_fues_note ORDER BY 1, 2, 3`,
		"079-82-2008|11152|winter|2026W|1|Anwendungsfach Physik",
		"079-82-2008|14037|summer|2026S|0|∅",
		"079-88-2008|14037|summer|2026S|0|∅")
	want(t, db, `SELECT id, fues_modules FROM v_program ORDER BY id`, "079-82-2008|2", "079-88-2008|2")

	checks := validated(t, db)
	for name, want := range map[string][2]any{
		"FÜS catalogs read":                                                 {catalogdb.StatusInfo, int64(2)},
		"seasons without a FÜS catalog":                                     {catalogdb.StatusOK, int64(0)},
		"headings of the FÜS catalogs that name no program":                 {catalogdb.StatusWarn, int64(1)},
		"FÜS pairs a module page states that the FÜS catalogs do not admit": {catalogdb.StatusInfo, int64(1)},
		"modules of FÜS lists with a note of a season":                      {catalogdb.StatusInfo, int64(3)},
		"program_module matches its source view":                            {catalogdb.StatusOK, int64(0)},
	} {
		if c, ok := checks[name]; !ok || c.Status != want[0] || c.Value != want[1] {
			t.Errorf("%s: %+v, want %v", name, c, want)
		}
	}
	if c := checks["headings of the FÜS catalogs that name no program"]; strings.Join(c.Samples, ";") != "2026W: Astrologie" {
		t.Errorf("samples: %q", c.Samples)
	}
	// What the catalogs write passes validate: a failing check keeps the export back, and until
	// schema 24 a line for every student had the scope '', which the check of placeholders refused
	// (canary, 2026-11-17).
	for name, c := range checks {
		if c.Status == catalogdb.StatusFail {
			t.Errorf("%s fails: %+v", name, c)
		}
	}
}

// Without any catalog the FÜS lists are what the FÜS modules' pages admit, and validate says that
// both seasons lack a catalog.
func TestWithoutCatalogsTheFUESListsAreThePages(t *testing.T) {
	db, _ := buildFixture(t)
	want(t, db, `SELECT program_id, module_id FROM v_program_module WHERE relation = 'fues' ORDER BY 1, 2`, "079-82-2008|11152", "079-88-2008|11152")
	want(t, db, `SELECT COUNT(*) FROM v_program_fues_note`, "0")
	if c := validated(t, db)["seasons without a FÜS catalog"]; c.Status != catalogdb.StatusWarn || c.Value != 2 {
		t.Errorf("%+v", c)
	}
}

func TestProgramsOfAHeading(t *testing.T) {
	ps := []*program{
		{id: "a", name: "Informatik"}, {id: "b", name: "Elektrotechnik"}, {id: "c", name: "Elektrotechnik - dual"},
		{id: "d", name: "Städtebau und Stadtplanung"}, {id: "e", name: "Stadtplanung und Städtebau"}, {id: "f", name: "Stadt- und Regionalplanung"},
		{id: "g", name: "Lehramt Primarstufe Mathematik-Deutsch"}, {id: "h", name: "Lehramt Primarstufe - dual Mathematik-Deutsch"},
		{id: "i", name: "Hybrid Electric Propulsion Technology"}, {id: "j", name: "Urban Design and Sustainable Revitalization"},
		{id: "k", name: "Physik"}, {id: "l", name: "Physics"},
	}
	m := newProgramMatcher(ps)
	ids := func(heading string) string {
		var out []string
		for _, p := range m.programs(heading) {
			out = append(out, p.id)
		}
		return strings.Join(out, ",")
	}
	for heading, wantIDs := range map[string]string{
		"Informatik":     "a",
		"Elektrotechnik": "b,c",
		"Städtebau und Stadtplanung/ Stadtplanung und Städtebau; Stadt- und Regionalplanung": "d,e,f",
		"Städtebau und Stadtplanung/ Stadtplanung; Stadt- und Regionalplanung":               "d,e,f",
		"Lehramt Primarstufe":                         "g,h",
		"Hybrid Electric Propulsation Technology":     "i",
		"Urban Design and Sustainable Revitalisation": "j",
		"Physik":                          "k",
		"Informations- und Medientechnik": "",
	} {
		if got := ids(heading); got != wantIDs {
			t.Errorf("%q → %q, want %q", heading, got, wantIDs)
		}
	}
}
