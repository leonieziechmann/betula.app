package catalogbuild

import (
	"fmt"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/normalize"
)

type program struct {
	id          string
	slug        string
	name        string
	degree      normalize.DegreeInfo
	poVersion   string
	poYear      int
	familyKey   string
	tree        *poTree
	degreeLabel string
}

// writePrograms writes programs, their documents, their area tree and the
// membership statements of the tree.
func (b *builder) writePrograms() error {
	b.programByID = make(map[string]*program)
	for _, t := range b.src.poTrees {
		if t.context.ProgramName == "" || t.context.Degree == "" || t.node.Stg == "" {
			continue // not a PO page we understand; validate reports the difference in counts
		}
		p := &program{
			id:        t.node.ProgramID(),
			name:      t.context.ProgramName,
			degree:    normalize.Degree(t.context.Degree),
			poVersion: t.context.POVersion,
			familyKey: t.node.Stg + "-" + t.node.Abschl,
			tree:      t,
		}
		if p.poVersion == "" {
			p.poVersion = t.node.PVersion
		}
		if p.poYear, _ = strconv.Atoi(t.node.PVersion); p.poYear == 0 {
			p.poYear, _ = normalize.POVersion(p.poVersion)
		}
		if p.degree.Level == normalize.LevelAbroad {
			continue
		}
		if _, dup := b.programByID[p.id]; dup {
			continue
		}
		b.programByID[p.id] = p
		b.programs = append(b.programs, p)
	}
	sort.Slice(b.programs, func(i, j int) bool { return b.programs[i].id < b.programs[j].id })

	assignSlugs(b.programs)
	b.assignDegreeLabels()

	latest := make(map[string]*program)
	for _, p := range b.programs {
		if cur := latest[p.familyKey]; cur == nil || p.poYear > cur.poYear || (p.poYear == cur.poYear && p.poVersion > cur.poVersion) {
			latest[p.familyKey] = p
		}
	}

	for _, p := range b.programs {
		_, amendment := normalize.POVersion(p.poVersion)
		var labelBasis any
		if p.degreeLabel != "" {
			labelBasis = "stated"
			b.report.ProgramsWithLabel++
		}
		_, err := b.tx.Exec(`
			INSERT INTO program (
				id, slug, name, stg_code, abschl_code, degree_raw, degree_level, degree_type, study_variant,
				degree_label, degree_label_basis, po_version, po_year, po_amendment,
				family_key, name_key, is_latest_po, source_url, fetched_at
			) VALUES (?,?,?,?,?,?,?,?,?, ?,?,?,?,?, ?,?,?,?,?)`,
			p.id, p.slug, p.name, p.tree.node.Stg, p.tree.node.Abschl, p.tree.context.Degree,
			p.degree.Level, null(p.degree.Type), null(p.degree.Variant),
			null(p.degreeLabel), labelBasis, p.poVersion, null(p.poYear), null(amendment),
			p.familyKey, normalize.Slug(p.name), boolInt(latest[p.familyKey] == p),
			p.tree.url, p.tree.fetchedAt.UTC().Format(time.RFC3339))
		if err != nil {
			return fmt.Errorf("program %s: %w", p.id, err)
		}
		b.report.Programs++

		for i, doc := range p.tree.documents {
			docType := doc.DocType
			if docType != "statute" && docType != "amendment" {
				docType = "other"
			}
			if _, err := b.tx.Exec("INSERT INTO program_document (program_id, ord, title, doc_type, url) VALUES (?, ?, ?, ?, ?)",
				p.id, i+1, doc.Title, docType, doc.URL); err != nil {
				return err
			}
		}

		if err := b.writeProgramTree(p); err != nil {
			return fmt.Errorf("program %s: %w", p.id, err)
		}
	}
	return nil
}

// isBookkeepingLabel: QIS account nodes that structure nothing.
func isBookkeepingLabel(label string) bool {
	return label == "Gesamtkonto" || label == "Total Account"
}

// writeProgramTree stores the area nodes below a PO and one assertion per module leaf.
func (b *builder) writeProgramTree(p *program) error {
	areaIDs := make(map[string]int64) // full label path → area id (0 for bookkeeping-only paths)
	seen := make(map[string]bool)     // module|area → asserted
	ord := 0

	for _, page := range p.tree.pages {
		var labels []string
		for _, label := range page.Path {
			if !isBookkeepingLabel(label) {
				labels = append(labels, label)
			}
		}

		var areaID int64
		key := strings.Join(page.Path, "\x1f")
		if len(page.Path) > 0 && !isBookkeepingLabel(page.Path[len(page.Path)-1]) {
			parentID := areaIDs[strings.Join(page.Path[:len(page.Path)-1], "\x1f")]
			ord++
			res, err := b.tx.Exec(`
				INSERT INTO program_area (program_id, parent_id, ord, depth, label, path, section, stated_kind, source_url)
				VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
				p.id, null(parentID), ord, len(labels), labels[len(labels)-1], strings.Join(labels, " / "),
				null(sectionFromAreaLabels(labels)), null(kindFromAreaLabels(labels)), page.URL)
			if err != nil {
				return err
			}
			if areaID, err = res.LastInsertId(); err != nil {
				return err
			}
			b.report.Areas++
		} else if len(page.Path) > 0 {
			areaID = areaIDs[strings.Join(page.Path[:len(page.Path)-1], "\x1f")]
		}
		areaIDs[key] = areaID

		for _, node := range page.Nodes {
			if !node.IsModule {
				continue
			}
			b.report.TreeLeaves++
			if node.ModuleID == "" || !b.moduleIDs[node.ModuleID] {
				name := node.ModuleID
				if name == "" {
					name = node.Text
				}
				b.report.TreeLeavesNoModule[name]++
				continue
			}
			dedup := node.ModuleID + "|" + strconv.FormatInt(areaID, 10)
			if seen[dedup] {
				continue
			}
			seen[dedup] = true

			kind, basis := kindFromAreaLabels(labels), "stated"
			if kind == "" && thesisTitle.MatchString(node.Title) {
				kind, basis = kindThesis, "inferred"
			}
			if err := b.insertAssertion(p.id, node.ModuleID, "qis_tree", areaID, "", kind, basis); err != nil {
				return err
			}
		}
	}
	return nil
}

func (b *builder) insertAssertion(programID, moduleID, source string, areaID int64, areaLabel, kind, basis string) error {
	if kind == "" {
		basis = ""
	}
	_, err := b.tx.Exec(`
		INSERT INTO program_module_assertion (program_id, module_id, source, area_id, area_label, kind, kind_basis)
		VALUES (?, ?, ?, ?, ?, ?, ?)`,
		programID, moduleID, source, null(areaID), null(areaLabel), null(kind), null(basis))
	if err == nil {
		b.report.Assertions[source]++
	}
	return err
}

func levelSlug(level string) string {
	switch level {
	case normalize.LevelTeachingBachelor:
		return "lehramt-bachelor"
	case normalize.LevelTeachingMaster:
		return "lehramt-master"
	case normalize.LevelDoctoral:
		return "promotion"
	case normalize.LevelNone:
		return "ohne-abschluss"
	case normalize.LevelOther, "":
		return "studiengang"
	}
	return level
}

// assignSlugs builds readable, unique URL keys: <level>-<name>-<po year>. The
// level, not the short label, is the prefix, so a slug does not change when a
// source starts to state „B.Sc.". Programs that would collide (study variants of
// the same subject) all get their degree code appended.
func assignSlugs(programs []*program) {
	byBase := make(map[string][]*program)
	for _, p := range programs {
		base := levelSlug(p.degree.Level) + "-" + normalize.Slug(p.name)
		if p.poYear > 0 {
			base += "-" + strconv.Itoa(p.poYear)
		}
		p.slug = base
		byBase[base] = append(byBase[base], p)
	}
	for base, group := range byBase {
		if len(group) == 1 {
			continue
		}
		for _, p := range group {
			p.slug = base + "-" + strings.ToLower(normalize.Slug(p.tree.node.Abschl))
		}
	}
	// Last resort for a collision nobody foresaw: the ID is unique.
	used := make(map[string]bool)
	for _, p := range programs {
		if used[p.slug] {
			p.slug = p.slug + "-" + normalize.Slug(p.id)
		}
		used[p.slug] = true
	}
}

// assignDegreeLabels derives „B.Sc." / „M.A." from what module pages say about a
// program („Studiengang Informatik B.Sc.: …"). A label is only used when it fits
// the degree level and clearly dominates the mentions of that program.
func (b *builder) assignDegreeLabels() {
	votes := make(map[string]map[string]int) // name|levelGroup → label → mentions
	for _, page := range b.src.modulePages {
		for _, st := range parseRemarkStatements(page.detail.Remarks) {
			group := labelLevelGroup(st.degreeLabel)
			if group == "" {
				continue
			}
			key := strings.ToLower(st.programName) + "|" + group
			if votes[key] == nil {
				votes[key] = make(map[string]int)
			}
			votes[key][st.degreeLabel]++
		}
	}

	// The same name can be offered as a university and as an applied program
	// (Maschinenbau B.Sc. / B.Eng.). Remarks do not say which one they mean, so
	// for such names *.Eng. mentions count for the applied program only and all
	// other labels for the university program only.
	types := make(map[string]map[string]bool) // name|group → degree types
	for _, p := range b.programs {
		key := labelVoteKey(p)
		if key == "" {
			continue
		}
		if types[key] == nil {
			types[key] = make(map[string]bool)
		}
		types[key][p.degree.Type] = true
	}

	for _, p := range b.programs {
		key := labelVoteKey(p)
		if key == "" {
			continue
		}
		mixed := len(types[key]) > 1
		total, best, bestN := 0, "", 0
		for label, n := range votes[key] {
			if mixed && strings.Contains(label, "Eng") != (p.degree.Type == normalize.TypeApplied) {
				continue
			}
			total += n
			if n > bestN || (n == bestN && label < best) {
				best, bestN = label, n
			}
		}
		if bestN >= 2 && bestN*10 >= total*7 && normalize.LabelMatchesLevel(best, p.degree.Level) {
			p.degreeLabel = best
		}
	}
}

// labelVoteKey groups a program with the remarks that may mention it. The dual
// variant of a subject („Maschinenbau - dual") awards the same degree.
func labelVoteKey(p *program) string {
	var group string
	switch p.degree.Level {
	case normalize.LevelBachelor:
		group = "B"
	case normalize.LevelMaster:
		group = "M"
	default:
		return ""
	}
	return strings.ToLower(strings.TrimSuffix(p.name, " - dual")) + "|" + group
}

func labelLevelGroup(label string) string {
	switch {
	case strings.HasPrefix(label, "B.") || label == "LL.B.":
		return "B"
	case strings.HasPrefix(label, "M.") || label == "LL.M.":
		return "M"
	}
	return ""
}
