// Package materialize — structural diff and state hashing for materialized state.
//
// ddis:implements APP-INV-089 (structural equivalence verification — content-only diff gates phase transitions)
// ddis:implements APP-INV-093 (snapshot creation determinism — StateHash is deterministic)
// ddis:implements APP-INV-096 (pipeline round-trip preservation — StructuralDiff verifies equivalence)
// ddis:implements APP-ADR-067 (structural equivalence definition)
// ddis:implements APP-ADR-072 (snapshot as SQLite state hash)

package materialize

import (
	"crypto/sha256"
	"database/sql"
	"fmt"
	"sort"
	"strings"
)

// Difference represents a single row-level difference between two materialized DBs.
type Difference struct {
	Table    string // e.g., "invariants", "adrs", "sections"
	Key      string // natural key, e.g., "APP-INV-071" or "1.2.3/Title"
	Kind     string // "added", "removed", "modified"
	Field    string // which field differs (empty for added/removed)
	Left     string // value in db1
	Right    string // value in db2
}

func (d Difference) String() string {
	switch d.Kind {
	case "added":
		return fmt.Sprintf("%s[%s]: added in right", d.Table, d.Key)
	case "removed":
		return fmt.Sprintf("%s[%s]: removed from right", d.Table, d.Key)
	default:
		return fmt.Sprintf("%s[%s].%s: %q → %q", d.Table, d.Key, d.Field, d.Left, d.Right)
	}
}

// StructuralDiff compares two materialized SQLite databases, returning content-level
// differences. Metadata fields (auto-increment id, parsed_at, created_at, raw_text,
// content_hash, line_start, line_end) are excluded per APP-ADR-067.
func StructuralDiff(db1, db2 *sql.DB, specID1, specID2 int64) []Difference {
	var diffs []Difference

	diffs = append(diffs, diffInvariants(db1, db2, specID1, specID2)...)
	diffs = append(diffs, diffADRs(db1, db2, specID1, specID2)...)
	diffs = append(diffs, diffSections(db1, db2, specID1, specID2)...)
	diffs = append(diffs, diffModules(db1, db2, specID1, specID2)...)
	diffs = append(diffs, diffGlossary(db1, db2, specID1, specID2)...)
	diffs = append(diffs, diffQualityGates(db1, db2, specID1, specID2)...)
	diffs = append(diffs, diffNegativeSpecs(db1, db2, specID1, specID2)...)
	diffs = append(diffs, diffCrossRefs(db1, db2, specID1, specID2)...)

	return diffs
}

// StateHash computes a deterministic SHA-256 hash over all content tables for a given
// spec_id. The hash covers only content fields (excluding auto-IDs, timestamps, raw_text,
// content_hash). Rows are sorted by natural key within each table, tables in canonical order.
//
// This is the hash stored in snapshots.state_hash (APP-ADR-072).
func StateHash(db *sql.DB, specID int64) (string, error) {
	h := sha256.New()

	// Canonical table order
	tables := []struct {
		name  string
		query string
	}{
		{"invariants", `SELECT invariant_id, title, statement, COALESCE(semi_formal,''), COALESCE(violation_scenario,''), COALESCE(validation_method,''), COALESCE(why_this_matters,'') FROM invariants WHERE spec_id = ? ORDER BY invariant_id`},
		{"adrs", `SELECT adr_id, title, COALESCE(problem,''), COALESCE(decision_text,''), COALESCE(chosen_option,''), COALESCE(consequences,''), COALESCE(tests,''), COALESCE(status,'active'), COALESCE(superseded_by,'') FROM adrs WHERE spec_id = ? ORDER BY adr_id`},
		{"sections", `SELECT section_path, title, heading_level FROM sections WHERE spec_id = ? ORDER BY section_path`},
		{"modules", `SELECT module_name, COALESCE(domain,'') FROM modules WHERE spec_id = ? ORDER BY module_name`},
		{"glossary_entries", `SELECT term, definition FROM glossary_entries WHERE spec_id = ? ORDER BY term`},
		{"quality_gates", `SELECT gate_id, title, COALESCE(predicate,'') FROM quality_gates WHERE spec_id = ? ORDER BY gate_id`},
		{"negative_specs", `SELECT constraint_text, COALESCE(reason,'') FROM negative_specs WHERE spec_id = ? ORDER BY constraint_text`},
		{"cross_references", `SELECT COALESCE(ref_type,''), ref_target, COALESCE(ref_text,'') FROM cross_references WHERE spec_id = ? ORDER BY ref_target, ref_text`},
	}

	for _, tbl := range tables {
		fmt.Fprintf(h, "TABLE:%s\n", tbl.name)

		rows, err := db.Query(tbl.query, specID)
		if err != nil {
			return "", fmt.Errorf("query %s: %w", tbl.name, err)
		}

		cols, err := rows.Columns()
		if err != nil {
			rows.Close()
			return "", fmt.Errorf("columns %s: %w", tbl.name, err)
		}

		for rows.Next() {
			vals := make([]interface{}, len(cols))
			ptrs := make([]interface{}, len(cols))
			for i := range vals {
				ptrs[i] = &vals[i]
			}
			if err := rows.Scan(ptrs...); err != nil {
				rows.Close()
				return "", fmt.Errorf("scan %s: %w", tbl.name, err)
			}
			for i, v := range vals {
				if i > 0 {
					fmt.Fprint(h, "\t")
				}
				fmt.Fprintf(h, "%v", v)
			}
			fmt.Fprint(h, "\n")
		}
		rows.Close()
	}

	return fmt.Sprintf("%x", h.Sum(nil)), nil
}

// --- Per-table diff helpers ---

type invRow struct {
	id, title, statement, semiFormal, violation, validation, why string
}

func diffInvariants(db1, db2 *sql.DB, sid1, sid2 int64) []Difference {
	q := `SELECT invariant_id, title, statement, COALESCE(semi_formal,''), COALESCE(violation_scenario,''), COALESCE(validation_method,''), COALESCE(why_this_matters,'') FROM invariants WHERE spec_id = ? ORDER BY invariant_id`

	left := scanInvariants(db1, q, sid1)
	right := scanInvariants(db2, q, sid2)

	return diffMaps("invariants", left, right, func(a, b invRow) []Difference {
		var d []Difference
		if a.title != b.title {
			d = append(d, Difference{Table: "invariants", Key: a.id, Kind: "modified", Field: "title", Left: a.title, Right: b.title})
		}
		if a.statement != b.statement {
			d = append(d, Difference{Table: "invariants", Key: a.id, Kind: "modified", Field: "statement", Left: a.statement, Right: b.statement})
		}
		if a.semiFormal != b.semiFormal {
			d = append(d, Difference{Table: "invariants", Key: a.id, Kind: "modified", Field: "semi_formal", Left: a.semiFormal, Right: b.semiFormal})
		}
		if a.violation != b.violation {
			d = append(d, Difference{Table: "invariants", Key: a.id, Kind: "modified", Field: "violation_scenario", Left: a.violation, Right: b.violation})
		}
		if a.validation != b.validation {
			d = append(d, Difference{Table: "invariants", Key: a.id, Kind: "modified", Field: "validation_method", Left: a.validation, Right: b.validation})
		}
		if a.why != b.why {
			d = append(d, Difference{Table: "invariants", Key: a.id, Kind: "modified", Field: "why_this_matters", Left: a.why, Right: b.why})
		}
		return d
	})
}

func scanInvariants(db *sql.DB, q string, specID int64) map[string]invRow {
	m := make(map[string]invRow)
	rows, err := db.Query(q, specID)
	if err != nil {
		return m
	}
	defer rows.Close()
	for rows.Next() {
		var r invRow
		rows.Scan(&r.id, &r.title, &r.statement, &r.semiFormal, &r.violation, &r.validation, &r.why)
		m[r.id] = r
	}
	return m
}

type adrRow struct {
	id, title, problem, decision, chosenOption, consequences, tests, status, supersededBy string
}

func diffADRs(db1, db2 *sql.DB, sid1, sid2 int64) []Difference {
	q := `SELECT adr_id, title, COALESCE(problem,''), COALESCE(decision_text,''), COALESCE(chosen_option,''), COALESCE(consequences,''), COALESCE(tests,''), COALESCE(status,'active'), COALESCE(superseded_by,'') FROM adrs WHERE spec_id = ? ORDER BY adr_id`

	left := scanADRs(db1, q, sid1)
	right := scanADRs(db2, q, sid2)

	return diffMaps("adrs", left, right, func(a, b adrRow) []Difference {
		var d []Difference
		fields := []struct{ name, l, r string }{
			{"title", a.title, b.title},
			{"problem", a.problem, b.problem},
			{"decision_text", a.decision, b.decision},
			{"chosen_option", a.chosenOption, b.chosenOption},
			{"consequences", a.consequences, b.consequences},
			{"tests", a.tests, b.tests},
			{"status", a.status, b.status},
			{"superseded_by", a.supersededBy, b.supersededBy},
		}
		for _, f := range fields {
			if f.l != f.r {
				d = append(d, Difference{Table: "adrs", Key: a.id, Kind: "modified", Field: f.name, Left: f.l, Right: f.r})
			}
		}
		return d
	})
}

func scanADRs(db *sql.DB, q string, specID int64) map[string]adrRow {
	m := make(map[string]adrRow)
	rows, err := db.Query(q, specID)
	if err != nil {
		return m
	}
	defer rows.Close()
	for rows.Next() {
		var r adrRow
		rows.Scan(&r.id, &r.title, &r.problem, &r.decision, &r.chosenOption, &r.consequences, &r.tests, &r.status, &r.supersededBy)
		m[r.id] = r
	}
	return m
}

type secRow struct {
	path  string
	title string
	level int
}

func diffSections(db1, db2 *sql.DB, sid1, sid2 int64) []Difference {
	q := `SELECT section_path, title, heading_level FROM sections WHERE spec_id = ? ORDER BY section_path`

	left := scanSections(db1, q, sid1)
	right := scanSections(db2, q, sid2)

	return diffMaps("sections", left, right, func(a, b secRow) []Difference {
		var d []Difference
		if a.title != b.title {
			d = append(d, Difference{Table: "sections", Key: a.path, Kind: "modified", Field: "title", Left: a.title, Right: b.title})
		}
		if a.level != b.level {
			d = append(d, Difference{Table: "sections", Key: a.path, Kind: "modified", Field: "heading_level", Left: fmt.Sprintf("%d", a.level), Right: fmt.Sprintf("%d", b.level)})
		}
		return d
	})
}

func scanSections(db *sql.DB, q string, specID int64) map[string]secRow {
	m := make(map[string]secRow)
	rows, err := db.Query(q, specID)
	if err != nil {
		return m
	}
	defer rows.Close()
	for rows.Next() {
		var r secRow
		rows.Scan(&r.path, &r.title, &r.level)
		m[r.path] = r
	}
	return m
}

type modRow struct {
	name, domain string
}

func diffModules(db1, db2 *sql.DB, sid1, sid2 int64) []Difference {
	q := `SELECT module_name, COALESCE(domain,'') FROM modules WHERE spec_id = ? ORDER BY module_name`

	left := scanModules(db1, q, sid1)
	right := scanModules(db2, q, sid2)

	return diffMaps("modules", left, right, func(a, b modRow) []Difference {
		var d []Difference
		if a.domain != b.domain {
			d = append(d, Difference{Table: "modules", Key: a.name, Kind: "modified", Field: "domain", Left: a.domain, Right: b.domain})
		}
		return d
	})
}

func scanModules(db *sql.DB, q string, specID int64) map[string]modRow {
	m := make(map[string]modRow)
	rows, err := db.Query(q, specID)
	if err != nil {
		return m
	}
	defer rows.Close()
	for rows.Next() {
		var r modRow
		rows.Scan(&r.name, &r.domain)
		m[r.name] = r
	}
	return m
}

type glossRow struct {
	term, definition string
}

func diffGlossary(db1, db2 *sql.DB, sid1, sid2 int64) []Difference {
	q := `SELECT term, definition FROM glossary_entries WHERE spec_id = ? ORDER BY term`

	left := scanGlossary(db1, q, sid1)
	right := scanGlossary(db2, q, sid2)

	return diffMaps("glossary_entries", left, right, func(a, b glossRow) []Difference {
		var d []Difference
		if a.definition != b.definition {
			d = append(d, Difference{Table: "glossary_entries", Key: a.term, Kind: "modified", Field: "definition", Left: a.definition, Right: b.definition})
		}
		return d
	})
}

func scanGlossary(db *sql.DB, q string, specID int64) map[string]glossRow {
	m := make(map[string]glossRow)
	rows, err := db.Query(q, specID)
	if err != nil {
		return m
	}
	defer rows.Close()
	for rows.Next() {
		var r glossRow
		rows.Scan(&r.term, &r.definition)
		m[r.term] = r
	}
	return m
}

type gateRow struct {
	id, title, predicate string
}

func diffQualityGates(db1, db2 *sql.DB, sid1, sid2 int64) []Difference {
	q := `SELECT gate_id, title, COALESCE(predicate,'') FROM quality_gates WHERE spec_id = ? ORDER BY gate_id`

	left := scanGates(db1, q, sid1)
	right := scanGates(db2, q, sid2)

	return diffMaps("quality_gates", left, right, func(a, b gateRow) []Difference {
		var d []Difference
		if a.title != b.title {
			d = append(d, Difference{Table: "quality_gates", Key: a.id, Kind: "modified", Field: "title", Left: a.title, Right: b.title})
		}
		if a.predicate != b.predicate {
			d = append(d, Difference{Table: "quality_gates", Key: a.id, Kind: "modified", Field: "predicate", Left: a.predicate, Right: b.predicate})
		}
		return d
	})
}

func scanGates(db *sql.DB, q string, specID int64) map[string]gateRow {
	m := make(map[string]gateRow)
	rows, err := db.Query(q, specID)
	if err != nil {
		return m
	}
	defer rows.Close()
	for rows.Next() {
		var r gateRow
		rows.Scan(&r.id, &r.title, &r.predicate)
		m[r.id] = r
	}
	return m
}

type negSpecRow struct {
	constraint, reason string
}

func diffNegativeSpecs(db1, db2 *sql.DB, sid1, sid2 int64) []Difference {
	q := `SELECT constraint_text, COALESCE(reason,'') FROM negative_specs WHERE spec_id = ? ORDER BY constraint_text`

	left := scanNegSpecs(db1, q, sid1)
	right := scanNegSpecs(db2, q, sid2)

	return diffMaps("negative_specs", left, right, func(a, b negSpecRow) []Difference {
		var d []Difference
		if a.reason != b.reason {
			d = append(d, Difference{Table: "negative_specs", Key: a.constraint, Kind: "modified", Field: "reason", Left: a.reason, Right: b.reason})
		}
		return d
	})
}

func scanNegSpecs(db *sql.DB, q string, specID int64) map[string]negSpecRow {
	m := make(map[string]negSpecRow)
	rows, err := db.Query(q, specID)
	if err != nil {
		return m
	}
	defer rows.Close()
	for rows.Next() {
		var r negSpecRow
		rows.Scan(&r.constraint, &r.reason)
		m[r.constraint] = r
	}
	return m
}

type xrefRow struct {
	refType, target, text string
}

func diffCrossRefs(db1, db2 *sql.DB, sid1, sid2 int64) []Difference {
	q := `SELECT COALESCE(ref_type,''), ref_target, COALESCE(ref_text,'') FROM cross_references WHERE spec_id = ? ORDER BY ref_target, ref_text`

	left := scanCrossRefs(db1, q, sid1)
	right := scanCrossRefs(db2, q, sid2)

	// ddis:maintains APP-INV-101 (composite key includes all discriminants — ref_type|target|text)
	return diffMaps("cross_references", left, right, func(a, b xrefRow) []Difference {
		var d []Difference
		if a.refType != b.refType {
			d = append(d, Difference{Table: "cross_references", Key: a.refType + "|" + a.target + "|" + a.text, Kind: "modified", Field: "ref_type", Left: a.refType, Right: b.refType})
		}
		return d
	})
}

func scanCrossRefs(db *sql.DB, q string, specID int64) map[string]xrefRow {
	m := make(map[string]xrefRow)
	rows, err := db.Query(q, specID)
	if err != nil {
		return m
	}
	defer rows.Close()
	for rows.Next() {
		var r xrefRow
		rows.Scan(&r.refType, &r.target, &r.text)
		// ddis:maintains APP-INV-101 (composite key completeness — include ref_type to prevent collision)
		key := r.refType + "|" + r.target + "|" + r.text
		m[key] = r
	}
	return m
}

// diffMaps is a generic helper that diffs two maps by key.
func diffMaps[V any](table string, left, right map[string]V, compare func(V, V) []Difference) []Difference {
	var diffs []Difference

	// Collect all keys sorted for determinism
	allKeys := make(map[string]bool)
	for k := range left {
		allKeys[k] = true
	}
	for k := range right {
		allKeys[k] = true
	}

	sorted := make([]string, 0, len(allKeys))
	for k := range allKeys {
		sorted = append(sorted, k)
	}
	sort.Strings(sorted)

	for _, k := range sorted {
		l, inLeft := left[k]
		r, inRight := right[k]

		if inLeft && !inRight {
			diffs = append(diffs, Difference{Table: table, Key: k, Kind: "removed"})
		} else if !inLeft && inRight {
			diffs = append(diffs, Difference{Table: table, Key: k, Kind: "added"})
		} else {
			diffs = append(diffs, compare(l, r)...)
		}
	}

	return diffs
}

// FormatDiffs returns a human-readable summary of differences.
func FormatDiffs(diffs []Difference) string {
	if len(diffs) == 0 {
		return "No structural differences"
	}

	var b strings.Builder
	fmt.Fprintf(&b, "%d structural difference(s):\n", len(diffs))
	for _, d := range diffs {
		fmt.Fprintf(&b, "  %s\n", d)
	}
	return b.String()
}
