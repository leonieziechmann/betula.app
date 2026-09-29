package catalogbuild

import (
	"sort"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/normalize"
	"github.com/leonieziechmann/betula/internal/parser"
)

// writeDepartments creates one row per organisational unit. German and English
// module pages name the same unit differently, and the faculty numbers were
// reassigned in a restructuring, so a code alone is not an identity. An English
// name is paired with the German name of the same code; when a code has several
// German names, the one whose modules share the most responsible persons wins.
// A name that cannot be paired still gets its own row: nothing is dropped.
func (b *builder) writeDepartments() error {
	type unit struct {
		raw, code, name string
		english         bool
		persons         map[string]bool
	}
	units := make(map[string]*unit)
	for _, page := range b.src.modulePages {
		raw := strings.TrimSpace(page.detail.Department)
		code, name, english := normalize.Department(raw)
		if name == "" {
			continue
		}
		u := units[raw]
		if u == nil {
			u = &unit{raw: raw, code: code, name: name, english: english, persons: make(map[string]bool)}
			units[raw] = u
		}
		for _, p := range page.detail.ResponsiblePersons {
			if p.Name != "" {
				u.persons[p.Name] = true
			}
		}
	}

	raws := make([]string, 0, len(units))
	for raw := range units {
		raws = append(raws, raw)
	}
	sort.Strings(raws)

	b.departmentIDs = make(map[string]int64)
	insert := func(u *unit, nameDE, nameEN string) (int64, error) {
		res, err := b.tx.Exec("INSERT INTO department (code, name_de, name_en, label) VALUES (?, ?, ?, ?)",
			null(u.code), null(nameDE), null(nameEN), u.raw)
		if err != nil {
			return 0, err
		}
		b.report.Departments++
		return res.LastInsertId()
	}

	for _, raw := range raws {
		if u := units[raw]; !u.english {
			id, err := insert(u, u.name, "")
			if err != nil {
				return err
			}
			b.departmentIDs[raw] = id
		}
	}

	for _, raw := range raws {
		u := units[raw]
		if !u.english {
			continue
		}
		var best *unit
		bestShared, tie, candidates := 0, false, 0
		for _, otherRaw := range raws {
			other := units[otherRaw]
			if other.english || other.code != u.code || u.code == "" {
				continue
			}
			candidates++
			shared := 0
			for name := range u.persons {
				if other.persons[name] {
					shared++
				}
			}
			switch {
			case best == nil || shared > bestShared:
				best, bestShared, tie = other, shared, false
			case shared == bestShared:
				tie = true
			}
		}

		if best != nil && (candidates == 1 || (bestShared > 0 && !tie)) {
			id := b.departmentIDs[best.raw]
			if _, err := b.tx.Exec("UPDATE department SET name_en = COALESCE(name_en, ?) WHERE id = ?", u.name, id); err != nil {
				return err
			}
			b.departmentIDs[raw] = id
			continue
		}
		id, err := insert(u, "", u.name)
		if err != nil {
			return err
		}
		b.departmentIDs[raw] = id
		b.report.UnpairedEnglishDep = append(b.report.UnpairedEnglishDep, raw)
	}
	return nil
}

// writeModules writes every module any source knows: the catalog list, the FÜS
// list, or an archived module page. The module page is the authority for all
// fields; the FÜS list only fills in for a module without a page.
func (b *builder) writeModules() error {
	ids := make(map[string]bool)
	for id := range b.src.catalogTitles {
		ids[id] = true
	}
	for id := range b.src.qisTitles {
		ids[id] = true
	}
	for id := range b.src.fues {
		ids[id] = true
	}
	for id := range b.src.modulePages {
		ids[id] = true
	}
	sorted := make([]string, 0, len(ids))
	for id := range ids {
		sorted = append(sorted, id)
	}
	sort.Strings(sorted)

	stmt, err := b.tx.Prepare(`
		INSERT INTO module (
			id, title, title_de, title_en, detail_status, page_lang, department_id, department_raw,
			credits, language_raw, teaches_german, teaches_english, duration_raw, duration_semesters,
			turnus_raw, turnus_season, turnus_parity, offer_status,
			limitation_raw, is_limited, participant_limit,
			exam_form_raw, exam_form, exam_details,
			exam_written, exam_oral, exam_paper, exam_presentation, exam_project, exam_practical,
			grading_raw, is_graded, is_fues, page_states_fues,
			learning_outcomes, contents, prerequisites_recommended, prerequisites_mandatory, remarks,
			source_url, fetched_at, description_source
		) VALUES (?,?,?,?,?,?,?,?, ?,?,?,?,?,?, ?,?,?,?, ?,?,?, ?,?,?, ?,?,?,?,?,?, ?,?,?,?, ?,?,?,?,?, ?,?,?)`)
	if err != nil {
		return err
	}
	defer stmt.Close()

	b.moduleIDs = make(map[string]bool, len(sorted))
	for _, id := range sorted {
		fuesEntry, isFUES := b.src.fues[id]
		page := b.src.modulePages[id]

		if page == nil {
			// Known from a list only. The FÜS list is the fallback for the few fields it has.
			title := firstNonEmpty(b.src.catalogTitles[id], b.src.qisTitles[id], fuesEntry.Title)
			if title == "" {
				title = id
			}
			german, english := normalize.Languages(fuesEntry.Language)
			known, limited, limit := normalize.Limitation(fuesEntry.Limitation)
			_, err := stmt.Exec(
				id, title, nil, nil, "missing", nil, nil, nil,
				null(fuesEntry.Credits), null(fuesEntry.Language), flagIf(fuesEntry.Language != "", german), flagIf(fuesEntry.Language != "", english), nil, nil,
				nil, nil, nil, "active",
				null(fuesEntry.Limitation), flagIf(known, limited), null(limit),
				nil, nil, nil,
				nil, nil, nil, nil, nil, nil,
				nil, nil, boolInt(isFUES), nil,
				nil, nil, nil, nil, nil,
				nil, nil, nil)
			if err != nil {
				return err
			}
			b.moduleIDs[id] = true
			b.report.Modules++
			b.report.ModulesWithoutPage++
			continue
		}

		d := page.detail
		// The heading carries the title in the page language, the row below it the other one.
		titleDE, titleEN, lang := d.TitleDE, d.TitleEN, "de"
		if page.english {
			titleDE, titleEN, lang = d.TitleEN, d.TitleDE, "en"
		}
		title := d.TitleDE
		if title == "" {
			title = firstNonEmpty(d.TitleEN, b.src.catalogTitles[id], b.src.qisTitles[id], fuesEntry.Title, id)
		}

		german, english := normalize.Languages(d.Language)
		semesters, _ := normalize.DurationSemesters(d.Duration)
		season, parity := normalize.Turnus(d.Turnus)
		limKnown, limited, limit := normalize.Limitation(d.Limitation)
		graded, gradedKnown := normalize.Graded(d.Grading)

		status := "active"
		switch {
		case d.IsNotOffered:
			status = "not_offered"
		case d.IsPhaseOut:
			status = "phase_out"
		}

		examDetails := strings.TrimSpace(d.ExamDetails)
		if examDetails == "-" {
			examDetails = ""
		}
		var kinds [6]any
		if examDetails != "" {
			k := normalize.ParseExamKinds(examDetails)
			kinds = [6]any{boolInt(k.Written), boolInt(k.Oral), boolInt(k.Paper), boolInt(k.Presentation), boolInt(k.Project), boolInt(k.Practical)}
		}

		departmentRaw := strings.TrimSpace(d.Department)
		if departmentRaw == "-" {
			departmentRaw = ""
		}
		var departmentID any
		if depID, ok := b.departmentIDs[departmentRaw]; ok {
			departmentID = depID
		}

		_, err := stmt.Exec(
			id, title, null(titleDE), null(titleEN), "ok", lang, departmentID, null(departmentRaw),
			null(d.Credits), null(d.Language), flagIf(d.Language != "", german), flagIf(d.Language != "", english), null(d.Duration), null(semesters),
			null(d.Turnus), null(season), null(parity), status,
			null(d.Limitation), flagIf(limKnown, limited), null(limit),
			null(d.ExamType), null(normalize.ExamForm(d.ExamType)), null(examDetails),
			kinds[0], kinds[1], kinds[2], kinds[3], kinds[4], kinds[5],
			null(d.Grading), flagIf(gradedKnown, graded), boolInt(isFUES), boolInt(d.CrossDisciplinary),
			freeText(d.LearningOutcomes), freeText(d.Contents),
			freeText(d.PrerequisitesRecommended), freeText(d.PrerequisitesMandatory), freeText(d.Remarks),
			page.url, page.fetchedAt.UTC().Format(time.RFC3339), page.descriptionSource())
		if err != nil {
			return err
		}
		b.moduleIDs[id] = true
		b.report.Modules++

		if err := b.writeModuleChildren(id, page); err != nil {
			return err
		}
	}
	return nil
}

func (b *builder) writeModuleChildren(id string, page *modulePage) error {
	d := page.detail

	ord := 0
	for _, p := range d.ResponsiblePersons {
		if strings.TrimSpace(p.Name) == "" {
			continue
		}
		ord++
		if _, err := b.tx.Exec("INSERT INTO module_person (module_id, ord, name, title) VALUES (?, ?, ?, ?)",
			id, ord, strings.TrimSpace(p.Name), null(strings.TrimSpace(p.Title))); err != nil {
			return err
		}
	}

	ord = 0
	for _, tf := range d.TeachingForms {
		forms := normalize.TeachingForm(tf.Type)
		if len(forms) == 0 || parser.IsNoAssignment(tf.Type) {
			continue
		}
		ord++
		sws, hours := normalize.Workload(tf.Workload)
		if _, err := b.tx.Exec(`INSERT INTO module_teaching_form (module_id, ord, form, form_raw, workload_raw, sws, hours)
			VALUES (?, ?, ?, ?, ?, ?, ?)`, id, ord, forms[0], tf.Type, null(tf.Workload), null(sws), null(hours)); err != nil {
			return err
		}
	}

	for kind, items := range map[string][]string{"literature": d.Literature, "course": d.AssociatedCourses} {
		ord = 0
		for _, item := range items {
			item = strings.TrimSpace(item)
			if normalize.IsNone(item) || parser.IsNoAssignment(item) {
				continue
			}
			ord++
			if _, err := b.tx.Exec("INSERT INTO module_text_item (module_id, kind, ord, text) VALUES (?, ?, ?, ?)", id, kind, ord, item); err != nil {
				return err
			}
		}
	}
	return nil
}

// writeModuleLinks runs after all modules exist, because it only keeps
// references to modules that are in the catalog.
func (b *builder) writeModuleLinks() error {
	ids := make([]string, 0, len(b.src.modulePages))
	for id := range b.src.modulePages {
		ids = append(ids, id)
	}
	sort.Strings(ids)

	for _, id := range ids {
		d := b.src.modulePages[id].detail
		for kind, text := range map[string]string{"mandatory": d.PrerequisitesMandatory, "recommended": d.PrerequisitesRecommended} {
			for _, required := range normalize.ModuleIDs(text) {
				if required == id || !b.moduleIDs[required] {
					continue
				}
				if _, err := b.tx.Exec("INSERT OR IGNORE INTO module_prerequisite (module_id, required_module_id, kind) VALUES (?, ?, ?)", id, required, kind); err != nil {
					return err
				}
			}
		}
		for _, successor := range d.SuccessorModules {
			if successor == id || !b.moduleIDs[successor] {
				continue
			}
			if _, err := b.tx.Exec("INSERT OR IGNORE INTO module_successor (module_id, successor_id) VALUES (?, ?)", id, successor); err != nil {
				return err
			}
		}
		// The successor states the replacement as well, so a module whose own page does
		// not name its successor still gets it.
		for _, predecessor := range d.PredecessorModules {
			if predecessor == id || !b.moduleIDs[predecessor] {
				continue
			}
			if _, err := b.tx.Exec("INSERT OR IGNORE INTO module_successor (module_id, successor_id) VALUES (?, ?)", predecessor, id); err != nil {
				return err
			}
		}
	}
	return nil
}

// flagIf is NULL when the fact is unknown, else 0/1.
func flagIf(known, value bool) any {
	if !known {
		return nil
	}
	return boolInt(value)
}

// freeText is NULL when a text field only states „keine" / "None" / „-".
func freeText(s string) any {
	s = strings.TrimSpace(s)
	if normalize.IsNone(s) {
		return nil
	}
	return s
}

func firstNonEmpty(values ...string) string {
	for _, v := range values {
		if v != "" {
			return v
		}
	}
	return ""
}
