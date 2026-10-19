package catalogbuild

import (
	"context"
	"fmt"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
)

// The modules of 2026W the rules were tried on (canary's snapshot of 2026-10-19,
// research/wahlangebote), as QIS and their descriptions state them.
func TestChoicesOf(t *testing.T) {
	ev := func(id, title string, sws float64, forms ...string) *choiceEvent {
		f := make(formSet)
		for _, form := range forms {
			f[form] = true
		}
		return &choiceEvent{id: id, title: title, sws: sws, forms: f}
	}
	who := map[string]map[string]bool{
		"148245": {"048-82": true, "211-82": true, "774-82": true}, "148246": {"017-82": true}, "148932": {"104-82": true, "774-82": true},
		"152426": {"768-88": true, "865-88": true}, "152436": {"768-O8": true},
	}
	// What a scope's choices and extras come to: „kind/basis/sws/pick: options“, each option its
	// events (with its label), and „+event/basis“ for an extra.
	sum := func(extras []moduleExtra, choices []*moduleChoice) string {
		var parts []string
		for _, x := range extras {
			parts = append(parts, "+"+x.event.id+"/"+x.basis)
		}
		for _, c := range choices {
			var options []string
			for _, o := range c.options {
				var ids []string
				for _, e := range o.events {
					ids = append(ids, e.id)
				}
				option := strings.Join(ids, ",")
				if o.label != "" {
					option = o.label + "=" + option
				}
				options = append(options, option)
			}
			parts = append(parts, fmt.Sprintf("%s/%s/%g/%d: %s", c.kind, c.basis, c.sws, c.pick, strings.Join(options, " ")))
		}
		return strings.Join(parts, "; ")
	}
	cases := []struct {
		name, title string
		sws         map[string]float64
		events      []*choiceEvent
		want        string
	}{{
		name: "11206: three exercises, one per program; the Aufbaukurs an extra", title: "Höhere Mathematik - T3",
		sws: map[string]float64{"lecture": 4, "exercise": 2},
		events: []*choiceEvent{ev("148060", "Höhere Mathematik - T3", 4, "lecture"), ev("148245", "Höhere Mathematik - T3", 2, "exercise"),
			ev("148246", "Höhere Mathematik - T3", 2, "exercise"), ev("148932", "Höhere Mathematik - T3", 2, "exercise"),
			ev("150190", "Aufbaukurs Höhere Mathematik - T3", 2, "exercise")},
		want: "+150190/title; program/form/2/1: 148245 148246 148932",
	}, {
		name: "13151: two titles in three groups, one choice of groups", title: "Musiktheorie - Grundlagen",
		sws: map[string]float64{"exercise": 2, "seminar": 4},
		events: []*choiceEvent{ev("149750", "Kontrapunkt/Tonsatz I (Gruppe 1)", 2, "seminar"), ev("149752", "Kontrapunkt/Tonsatz I (Gruppe 2)", 2, "seminar"),
			ev("149753", "Gehörbildung I (Gruppe 1)", 1, "exercise"), ev("149755", "Gehörbildung I (Gruppe 2)", 1, "exercise"),
			ev("150377", "Gehörbildung I (Gruppe 3)", 1, "exercise"), ev("150378", "Kontrapunkt/Tonsatz I (Gruppe 3)", 2, "seminar")},
		want: "group/named/0/1: Gruppe 1=149753,149750 Gruppe 2=149755,149752 Gruppe 3=150377,150378",
	}, {
		name: "12672: projects where the module says seminar, one of them", title: "Berufspraktische Profilierung",
		sws: map[string]float64{"seminar": 4},
		events: []*choiceEvent{ev("149199", "Modul 12.2 OM3: Geschichtenkoffer", 4, "project"), ev("150724", "Modul 12.2 OM3: Digitale Pioniere", 4, "project"),
			ev("151748", "OM3 Praxisprojekte: Positive Psychologie", 4, "project"), ev("151771", "OM3 Praxisprojekte: Teilhabe und Teilhabebarrieren", 4, "project")},
		want: "topic/module/4/1: 149199 150724 151748 151771",
	}, {
		name: "12818: courses of 4 SWS for 8, two of them", title: "Basis-Deutsch für den Uni-Alltag",
		sws: map[string]float64{"exercise": 8},
		events: []*choiceEvent{ev("148524", "Deutsch als Fremdsprache A1.2", 4, "exercise"), ev("148542", "Deutsch als Fremdsprache Start A1.1", 4, "exercise"),
			ev("148585", "Deutsch als Fremdsprache A2.2", 4, "exercise"), ev("148676", "Deutsch als Fremdsprache A2.1", 4, "exercise")},
		want: "topic/form/8/2: 148524 148542 148585 148676",
	}, {
		name: "12823: the same course in person and online, one per program", title: "Introduction / Coaching (Online)",
		sws: map[string]float64{"seminar": 4},
		events: []*choiceEvent{ev("152426", "Theoretical and Methodological Foundations for Heritage Research", 4, "lecture", "seminar"),
			ev("152436", "Theoretical and Methodological Foundations for Heritage Research (Online)", 4, "lecture", "seminar")},
		want: "program/module/4/1: 152426 152436",
	}, {
		name: "11212: a tutorial the module does not state", title: "Statistics",
		sws:    map[string]float64{"lecture": 2, "exercise": 2},
		events: []*choiceEvent{ev("148299", "Statistics", 2, "lecture"), ev("148301", "Statistics", 2, "exercise"), ev("148503", "Statistics", 2, "tutorial")},
		want:   "+148503/tutorial",
	}, {
		name: "13280: the module's title names both labs, parts", title: "Werkstoff- und Physiklabor",
		sws:    map[string]float64{"practical": 4},
		events: []*choiceEvent{ev("150600", "Laborwoche Werkstofflabor", 4, "practical"), ev("150716", "Laborwoche Physiklabor", 4, "practical")},
	}, {
		name: "12118: titles apart by a part's number, parts", title: "Physiotherapie im Handlungsfeld Innere Medizin und Rheumatologie",
		sws: map[string]float64{"lecture": 4, "exercise": 2},
		events: []*choiceEvent{ev("150179", "BT17 SKL Innere Medizin", 2, "lecture"), ev("151065", "BT17 Methodik Innere Medizin", 2, "lecture"),
			ev("150180", "BT17 Methodik Innere Medizin - T1", 2, "exercise"), ev("153082", "BT17 Methodik Innere Medizin - T2", 2, "exercise"),
			ev("153083", "BT17 Methodik Innere Medizin - T3", 2, "exercise")},
	}, {
		name: "14824: data that do not fit (1 SWS stated, 1 + 3 held), nothing", title: "Automated Biodiversity Surveying Tools and Methods",
		sws:    map[string]float64{"lecture": 1},
		events: []*choiceEvent{ev("151712", "Biodiversity surveying tools and methods", 1, "lecture"), ev("151713", "Biodiversity recording and analysis with Audiomoth", 3, "practical")},
	}, {
		name: "11760: two halves at the SWS, both required", title: "Betriebssysteme und Rechnernetze",
		sws:    map[string]float64{"lecture": 2, "practical": 2},
		events: []*choiceEvent{ev("150029", "Betriebssysteme und Rechnernetze - Betriebssysteme", 2, "lecture", "practical"), ev("150033", "Betriebssysteme und Rechnernetze - Rechnernetze", 2, "lecture", "practical")},
	}, {
		name: "no SWS stated, nothing", title: "Ohne Angaben",
		events: []*choiceEvent{ev("1", "A", 2, "seminar"), ev("2", "B", 2, "seminar"), ev("3", "C", 2, "seminar")},
	}}
	for _, c := range cases {
		extras, choices := choicesOf(c.title, c.sws, c.events, who)
		if got := sum(extras, choices); got != c.want {
			t.Errorf("%s:\n got  %s\n want %s", c.name, got, c.want)
		}
	}

	if label, order := groupLabel("BP34 Begleitseminar VIII (A-Kurs)"); label != "A-Kurs" || order != 1 {
		t.Errorf("groupLabel A-Kurs = %q %d", label, order)
	}
	if label, order := groupLabel("Repertoirekunde für Pianisten (Gruppe 2)"); label != "Gruppe 2" || order != 2 {
		t.Errorf("groupLabel Gruppe 2 = %q %d", label, order)
	}
	if got := choiceStem("Seminar Town and House"); got != "town and house" {
		t.Errorf("choiceStem = %q", got)
	}
}

// A miniature of Höhere Mathematik T3 of 2026W at the Zentralcampus, through the whole build: the
// lecture for everybody, two exercises QIS lists for Informatik and Medizininformatik, an
// Aufbaukurs and a tutorial (the module pages state 4 SWS of lectures and 2 of exercises, as
// modulePageDE does).
func TestBuildChoices(t *testing.T) {
	db, err := catalogdb.Open(filepath.Join(t.TempDir(), "choices.db"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = db.Close() })
	put := func(source, key, body string) {
		t.Helper()
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: key, HTTPStatus: 200, Body: []byte(body),
			FetchedAt: time.Date(2026, 10, 19, 9, 0, 0, 0, time.UTC)}); err != nil {
			t.Fatal(err)
		}
	}
	const (
		central    = "Zentrales Hörsaalgebäude - Hörsaal C - Zentralcampus"
		informatik = "Informatik Bachelor (universitäres Profil)"
		medinf     = "Medizininformatik Bachelor (universitäres Profil)"
	)
	events := []string{"148060", "148245", "148246", "150190", "148503"}
	var links strings.Builder
	for _, event := range events {
		links.WriteString(qisEventLink(event, event))
	}
	put(catalogdb.SourceModulePage, "11206", modulePageDE("11206", "Höhere Mathematik - T3", `<tr><td>Veranstaltungen im aktuellen Semester:</td><td><ul>`+links.String()+`</ul></td></tr>`))
	put(catalogdb.SourceModuleCatalog, "list", `<table><tbody class="list"><tr><td class="moduleNumber"><a href="/modul/11206">11206</a></td><td class="title">Höhere Mathematik - T3</td></tr></tbody></table>`)
	put(catalogdb.SourceQISEvent, "148060", trackEventPage("Höhere Mathematik - T3", "Vorlesung", "4", central, informatik, medinf))
	put(catalogdb.SourceQISEvent, "148245", trackEventPage("Höhere Mathematik - T3", "Übung", "2", central, informatik))
	put(catalogdb.SourceQISEvent, "148246", trackEventPage("Höhere Mathematik - T3", "Übung", "2", central, medinf))
	put(catalogdb.SourceQISEvent, "150190", trackEventPage("Aufbaukurs Höhere Mathematik - T3", "Übung", "2", central, informatik, medinf))
	put(catalogdb.SourceQISEvent, "148503", trackEventPage("Höhere Mathematik - T3", "Tutorium", "2", central))
	for _, p := range []fixtureProgram{
		{name: "Informatik", stg: "079", abschl: "82", degree: "Bachelor (universitär)", pversion: "2008", po: "2008 - 2. SÄ 2024"},
		{name: "Medizininformatik", stg: "247", abschl: "82", degree: "Bachelor (universitär)", pversion: "2016", po: "2016 - 1. SÄ 2024"},
	} {
		node := "auswahlBaum|studiengang:stg=" + p.stg + "|abschluss:abschl=" + p.abschl + "|stgSpecials:vert=,schwp=,kzfa=H,pversion=" + p.pversion
		put(catalogdb.SourceQISTree, treeBase+node, fixtureTreePage(p, node+"|pruefung:1", "11206 Höhere Mathematik - T3"))
	}

	report, err := Build(context.Background(), db)
	if err != nil {
		t.Fatal(err)
	}
	if report.ModulesWithChoices != 1 || report.ChoicesByProgram != 1 || report.ChoiceExtras != 2 {
		t.Errorf("report: %d modules with choices, %d by program, %d extras", report.ModulesWithChoices, report.ChoicesByProgram, report.ChoiceExtras)
	}
	want(t, db, `SELECT DISTINCT event_id, IFNULL(choice, '∅'), IFNULL(option, '∅'), extra FROM v_module_schedule WHERE module_id = '11206' ORDER BY event_id`,
		"148060|∅|∅|0", "148245|1|1|0", "148246|1|2|0", "148503|∅|∅|1", "150190|∅|∅|1")
	want(t, db, `SELECT module_id, semester_key, choice, IFNULL(course, '∅'), kind, basis, sws, pick, option, event_id, IFNULL(label, '∅') FROM v_module_choice ORDER BY option`,
		"11206|2026W|1|∅|program|form|2|1|1|148245|∅", "11206|2026W|1|∅|program|form|2|1|2|148246|∅")
	want(t, db, `SELECT choice, option, family_key FROM v_module_choice_family ORDER BY option`, "1|1|079-82", "1|2|247-82")
	want(t, db, `SELECT event_id, basis FROM module_extra ORDER BY event_id`, "148503|tutorial", "150190|title")

	again, err := Build(context.Background(), db)
	if err != nil {
		t.Fatal(err)
	}
	if again.ContentDigest != report.ContentDigest {
		t.Errorf("the second build changed the content: %s, %s", report.ContentDigest, again.ContentDigest)
	}
}
