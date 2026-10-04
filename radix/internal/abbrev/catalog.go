package abbrev

import (
	"database/sql"
	"fmt"
)

// Queryer is a *sql.DB or a *sql.Tx.
type Queryer interface {
	Query(query string, args ...any) (*sql.Rows, error)
}

// Input is what Derive reads from a catalog database.
type Input struct {
	Modules []Module
	Members []Member
	Titles  []string
}

// ReadCatalog reads the modules, their titles and the program memberships of a built
// catalog (module and v_program_module), in a stable order.
func ReadCatalog(q Queryer) (*Input, error) {
	in := &Input{}
	rows, err := q.Query("SELECT id, title, title_de, title_en FROM module ORDER BY id")
	if err != nil {
		return nil, fmt.Errorf("modules: %w", err)
	}
	for rows.Next() {
		var id, title string
		var de, en sql.NullString
		if err := rows.Scan(&id, &title, &de, &en); err != nil {
			rows.Close()
			return nil, err
		}
		in.Modules = append(in.Modules, Module{ID: id, Title: title})
		in.Titles = append(in.Titles, title)
		if de.Valid {
			in.Titles = append(in.Titles, de.String)
		}
		if en.Valid {
			in.Titles = append(in.Titles, en.String)
		}
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return nil, err
	}

	rows, err = q.Query("SELECT program_id, module_id, relation, kind, plan_semester FROM v_program_module ORDER BY program_id, module_id")
	if err != nil {
		return nil, fmt.Errorf("program modules: %w", err)
	}
	defer rows.Close()
	for rows.Next() {
		var pid, mid, relation string
		var kind sql.NullString
		var semester sql.NullInt64
		if err := rows.Scan(&pid, &mid, &relation, &kind, &semester); err != nil {
			return nil, err
		}
		sem := 99
		if semester.Valid {
			sem = int(semester.Int64)
		}
		in.Members = append(in.Members, Member{ProgramID: pid, ModuleID: mid, Tier: Tier(relation, kind.String), PlanSemester: sem})
	}
	return in, rows.Err()
}
