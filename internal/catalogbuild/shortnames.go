package catalogbuild

import (
	"database/sql"
	"fmt"
	"sort"
	"strings"

	"github.com/leonieziechmann/betula/internal/abbrev"
	"github.com/leonieziechmann/betula/internal/normalize"
)

// writeRoomShorts gives every event room its short form (normalize.RoomShort): „ZHG/HS.A“
// for „Zentrales Hörsaalgebäude - Hörsaal A - Zentralcampus“. room keeps the full name.
//
// A short form names one room. Where two rooms would share one, both keep their long form
// (the name without its campus), and the report says so, so that the building table or a
// rule gets fixed.
func (b *builder) writeRoomShorts() error {
	rows, err := b.tx.Query("SELECT room, COUNT(*) FROM event_date WHERE room IS NOT NULL GROUP BY room ORDER BY room")
	if err != nil {
		return err
	}
	var rooms []string
	dates := map[string]int{}
	for rows.Next() {
		var room string
		var n int
		if err := rows.Scan(&room, &n); err != nil {
			rows.Close()
			return err
		}
		rooms = append(rooms, room)
		dates[room] = n
	}
	rows.Close()
	if err := rows.Err(); err != nil {
		return err
	}

	short := make(map[string]string, len(rooms))
	for _, room := range rooms {
		s, known := normalize.RoomShort(room)
		if !known {
			b.report.RoomsUnknownBuilding[room] = dates[room]
		}
		short[room] = s
	}
	collisions := roomShortCollisions(rooms, short)
	sort.Strings(collisions)
	b.report.RoomShortCollisions = collisions

	for _, room := range rooms {
		if _, err := b.tx.Exec("UPDATE event_date SET room_short = ? WHERE room = ?", short[room], room); err != nil {
			return fmt.Errorf("room %q: %w", room, err)
		}
	}
	return nil
}

// roomShortCollisions replaces short forms that two rooms share by the long form, and that
// by the full name where even the long forms are the same, until every form names one room.
// It returns the rooms that lost their short form.
func roomShortCollisions(rooms []string, short map[string]string) []string {
	var lost []string
	isLost := map[string]bool{}
	for level := 1; level <= 2; level++ {
		byShort := map[string][]string{}
		for _, room := range rooms {
			byShort[short[room]] = append(byShort[short[room]], room)
		}
		changed := false
		for _, rs := range byShort {
			if len(rs) < 2 {
				continue
			}
			for _, room := range rs {
				if !isLost[room] {
					isLost[room] = true
					lost = append(lost, room)
				}
				long := room
				if level == 1 {
					long = strings.Join(strings.Fields(room), " ")
					if c := campusLabel(long); c != "" {
						long = strings.TrimSuffix(long, " - "+c)
					}
				}
				if short[room] != long {
					short[room] = long
					changed = true
				}
			}
		}
		if !changed {
			break
		}
	}
	return lost
}

// campusLabel is the campus suffix of an event's room („Campus Senftenberg“), or "".
func campusLabel(room string) string {
	i := strings.LastIndex(room, " - ")
	if i < 0 || normalize.Campus(room) == "" {
		return ""
	}
	return room[i+3:]
}

// readAbbreviations reads the (program, module) abbreviations of the build before, which the
// build is about to replace, so that it can count what moved.
func readAbbreviations(tx *sql.Tx) (map[[2]string]string, error) {
	rows, err := tx.Query("SELECT program_id, module_id, abbrev FROM program_module_abbrev")
	if err != nil {
		return nil, fmt.Errorf("previous abbreviations: %w", err)
	}
	defer rows.Close()
	out := map[[2]string]string{}
	for rows.Next() {
		var pid, mid, a string
		if err := rows.Scan(&pid, &mid, &a); err != nil {
			return nil, err
		}
		out[[2]string{pid, mid}] = a
	}
	return out, rows.Err()
}

// writeAbbreviations derives the abbreviation of every module and of every module of every
// program (package abbrev). It reads module and program_module, so it runs after them.
func (b *builder) writeAbbreviations() error {
	in, err := abbrev.ReadCatalog(b.tx)
	if err != nil {
		return err
	}
	overrides, err := abbrev.Overrides()
	if err != nil {
		return err
	}
	res := abbrev.Derive(in.Modules, in.Members, in.Titles, overrides)
	b.report.AbbrevOverridesUnused = res.UnusedOverrides
	b.report.AbbrevFellBack = res.FellBack
	b.report.AbbrevTwins = res.Twins
	// The churn: a new title anywhere can move forms in other programs (the splitter learns
	// from every title), so build.finished says how many pairs moved.
	for pid, choices := range res.Programs {
		for id, c := range choices {
			if prev, ok := b.previousAbbrevs[[2]string{pid, id}]; ok && prev != c.Abbrev {
				b.report.AbbrevChanged++
			}
		}
	}

	ids := make([]string, 0, len(res.Defaults))
	for id := range res.Defaults {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		c := res.Defaults[id]
		if _, err := b.tx.Exec("INSERT INTO module_abbrev (module_id, abbrev, is_override) VALUES (?, ?, ?)",
			id, c.Abbrev, boolInt(c.Override)); err != nil {
			return fmt.Errorf("module %s: %w", id, err)
		}
	}
	programs := make([]string, 0, len(res.Programs))
	for pid := range res.Programs {
		programs = append(programs, pid)
	}
	sort.Strings(programs)
	for _, pid := range programs {
		choices := res.Programs[pid]
		ids := make([]string, 0, len(choices))
		for id := range choices {
			ids = append(ids, id)
		}
		sort.Strings(ids)
		for _, id := range ids {
			c := choices[id]
			if _, err := b.tx.Exec(`INSERT INTO program_module_abbrev (program_id, module_id, abbrev, is_override, choice, is_twin)
				VALUES (?, ?, ?, ?, ?, ?)`, pid, id, c.Abbrev, boolInt(c.Override), c.Choice, boolInt(c.Twin)); err != nil {
				return fmt.Errorf("program %s module %s (%s): %w", pid, id, c.Abbrev, err)
			}
		}
	}
	return nil
}
