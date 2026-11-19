package catalogbuild

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/privacy"
)

// A person added to the privacy register is out of the next build, validate holds it, and the
// export's snapshot names them nowhere, in no table and no free page of the file; the snapshot
// built before is removed. The person is made up.
func TestThePrivacyRegisterTakesAPersonOut(t *testing.T) {
	ctx := context.Background()
	db, first := buildFixtureWith(t, func(put func(source, key, body string)) {
		page := strings.Replace(modulePageDE("13000", "Verschwundenes Modul", `
			<tr><td>Bemerkungen:</td><td>Fragen an max.mustermann@b-tu.de oder Herrn Mustermann.</td></tr>`),
			"Köhler, Ekkehard", "Mustermann, Max", 1)
		put(catalogdb.SourceModulePage, "13000", page)
	})
	want(t, db, `SELECT name FROM module_person WHERE module_id = '13000'`, "Mustermann, Max")
	if first.PrivacyPeople != 0 || first.PrivacyRows != 0 {
		t.Fatalf("an empty register took something out: %+v", first)
	}

	dir := filepath.Join(t.TempDir(), "snapshot")
	before, err := db.Export(ctx, dir)
	if err != nil {
		t.Fatal(err)
	}

	if _, err := privacy.Add(ctx, db.SQL(), privacy.Person{Surname: "Mustermann", Given: "Max", Note: "test"}, time.Now()); err != nil {
		t.Fatal(err)
	}
	report, err := Build(ctx, db)
	if err != nil {
		t.Fatal(err)
	}
	if report.PrivacyPeople != 1 || report.PrivacyRows != 1 || report.PrivacyValues == 0 {
		t.Errorf("report: people %d, rows %d, values %d", report.PrivacyPeople, report.PrivacyRows, report.PrivacyValues)
	}
	if report.ContentDigest == first.ContentDigest {
		t.Error("the content digest did not change: the snapshot without the person would never be exported")
	}
	want(t, db, `SELECT COUNT(*) FROM module_person WHERE name LIKE '%Mustermann%'`, "0")
	want(t, db, `SELECT COUNT(*) FROM v_module WHERE id = '13000' AND remarks LIKE '%ustermann%'`, "0")
	want(t, db, `SELECT (LENGTH(remarks) - LENGTH(REPLACE(remarks, '[entfernt]', ''))) / LENGTH('[entfernt]') FROM module WHERE id = '13000'`, "2")
	want(t, db, `SELECT name FROM v_module_lecturer WHERE module_id = '11881' ORDER BY role`, "Meyer", "Köhler, Ekkehard")

	checks, err := db.Validate(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	if catalogdb.HasFailures(checks) {
		t.Fatalf("validate fails after the build took the person out: %+v", checks)
	}
	found := false
	for _, c := range checks {
		if c.Name == "persons in the privacy register" && c.Value == 1 {
			found = true
		}
	}
	if !found {
		t.Error("validate does not count the register")
	}

	// A step that wrote the name after the redaction: validate fails, and says where, not who.
	if _, err := db.SQL().Exec(`UPDATE module SET remarks = 'Sprechstunde bei Prof. Mustermann' WHERE id = '11101'`); err != nil {
		t.Fatal(err)
	}
	checks, err = db.Validate(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	for _, c := range checks {
		if c.Name == "no person of the privacy register is left in the catalog" {
			if c.Status != catalogdb.StatusFail || c.Value != 1 || len(c.Samples) != 1 || strings.Contains(c.Samples[0], "Mustermann") {
				t.Errorf("check = %+v", c)
			}
		}
	}
	if _, err := Build(ctx, db); err != nil {
		t.Fatal(err)
	}

	after, err := db.Export(ctx, dir)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(dir, before.File)); !os.IsNotExist(err) {
		t.Errorf("the snapshot built before the person was added is still there: %v", err)
	}
	body, err := os.ReadFile(filepath.Join(dir, after.File))
	if err != nil {
		t.Fatal(err)
	}
	for _, needle := range []string{"Mustermann", "mustermann", "privacy_person"} {
		if bytes.Contains(body, []byte(needle)) {
			t.Errorf("the snapshot holds %q", needle)
		}
	}
}
