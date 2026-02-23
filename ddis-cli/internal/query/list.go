package query

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ListType identifies what kind of element to list.
type ListType string

const (
	ListInvariants ListType = "invariants"
	ListADRs       ListType = "adrs"
	ListGates      ListType = "gates"
	ListSections   ListType = "sections"
	ListGlossary   ListType = "glossary"
	ListModules    ListType = "modules"
)

// ParseListType converts a string to a ListType, returning an error for unknown types.
func ParseListType(s string) (ListType, error) {
	switch strings.ToLower(s) {
	case "invariants", "inv":
		return ListInvariants, nil
	case "adrs", "adr":
		return ListADRs, nil
	case "gates", "gate":
		return ListGates, nil
	case "sections", "section":
		return ListSections, nil
	case "glossary":
		return ListGlossary, nil
	case "modules", "module":
		return ListModules, nil
	default:
		return "", fmt.Errorf("unknown list type %q: valid types are invariants, adrs, gates, sections, glossary, modules", s)
	}
}

// ListItem is a generic list entry.
type ListItem struct {
	ID    string `json:"id"`
	Title string `json:"title"`
	Extra string `json:"extra,omitempty"`
}

// ListElements returns all elements of the given type as ListItems.
func ListElements(db *sql.DB, specID int64, listType ListType) ([]ListItem, error) {
	switch listType {
	case ListInvariants:
		items, err := storage.ListInvariants(db, specID)
		if err != nil {
			return nil, err
		}
		result := make([]ListItem, len(items))
		for i, inv := range items {
			extra := ""
			if inv.ConditionalTag != "" {
				extra = "[" + inv.ConditionalTag + "]"
			}
			result[i] = ListItem{ID: inv.InvariantID, Title: inv.Title, Extra: extra}
		}
		return result, nil

	case ListADRs:
		items, err := storage.ListADRs(db, specID)
		if err != nil {
			return nil, err
		}
		result := make([]ListItem, len(items))
		for i, a := range items {
			extra := a.Status
			if a.Confidence != "" {
				extra += " | " + a.Confidence
			}
			result[i] = ListItem{ID: a.ADRID, Title: a.Title, Extra: extra}
		}
		return result, nil

	case ListGates:
		items, err := storage.ListQualityGates(db, specID)
		if err != nil {
			return nil, err
		}
		result := make([]ListItem, len(items))
		for i, g := range items {
			extra := ""
			if g.IsModular {
				extra = "[modular]"
			}
			result[i] = ListItem{ID: g.GateID, Title: g.Title, Extra: extra}
		}
		return result, nil

	case ListSections:
		items, err := storage.ListSections(db, specID)
		if err != nil {
			return nil, err
		}
		result := make([]ListItem, len(items))
		for i, s := range items {
			indent := strings.Repeat("  ", s.HeadingLevel-1)
			result[i] = ListItem{
				ID:    s.SectionPath,
				Title: indent + s.Title,
				Extra: fmt.Sprintf("L%d–%d", s.LineStart, s.LineEnd),
			}
		}
		return result, nil

	case ListGlossary:
		items, err := storage.ListGlossaryEntries(db, specID)
		if err != nil {
			return nil, err
		}
		result := make([]ListItem, len(items))
		for i, ge := range items {
			// Truncate long definitions
			def := ge.Definition
			if len(def) > 80 {
				def = def[:77] + "..."
			}
			result[i] = ListItem{ID: ge.Term, Title: def}
		}
		return result, nil

	case ListModules:
		items, err := storage.ListModules(db, specID)
		if err != nil {
			return nil, err
		}
		result := make([]ListItem, len(items))
		for i, m := range items {
			result[i] = ListItem{
				ID:    m.ModuleName,
				Title: m.Domain,
				Extra: fmt.Sprintf("%d lines", m.LineCount),
			}
		}
		return result, nil

	default:
		return nil, fmt.Errorf("unsupported list type: %s", listType)
	}
}

// SpecStats holds summary statistics for a spec index.
type SpecStats struct {
	SpecPath         string         `json:"spec_path"`
	SourceType       string         `json:"source_type"`
	DDISVersion      string         `json:"ddis_version,omitempty"`
	TotalLines       int            `json:"total_lines"`
	ElementCounts    map[string]int `json:"element_counts"`
	XRefTotal        int            `json:"xref_total"`
	XRefResolved     int            `json:"xref_resolved"`
	XRefResolutionPc float64        `json:"xref_resolution_percent"`
}

// ComputeStats computes summary statistics for a spec.
func ComputeStats(db *sql.DB, specID int64) (*SpecStats, error) {
	spec, err := storage.GetSpecIndex(db, specID)
	if err != nil {
		return nil, err
	}

	counts, err := storage.CountElements(db, specID)
	if err != nil {
		return nil, err
	}

	total := counts["cross_references"]
	resolved := counts["cross_references_resolved"]
	resPc := 0.0
	if total > 0 {
		resPc = float64(resolved) / float64(total) * 100
	}

	return &SpecStats{
		SpecPath:         spec.SpecPath,
		SourceType:       spec.SourceType,
		DDISVersion:      spec.DDISVersion,
		TotalLines:       spec.TotalLines,
		ElementCounts:    counts,
		XRefTotal:        total,
		XRefResolved:     resolved,
		XRefResolutionPc: resPc,
	}, nil
}

// RenderList formats a list of items as markdown or JSON.
func RenderList(items []ListItem, listType ListType, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(items, "", "  ")
		if err != nil {
			return "", err
		}
		return string(data), nil
	}

	var b strings.Builder
	heading := string(listType)
	if len(heading) > 0 {
		heading = strings.ToUpper(heading[:1]) + heading[1:]
	}
	fmt.Fprintf(&b, "%s (%d items)\n", heading, len(items))
	b.WriteString(strings.Repeat("─", 60) + "\n")

	for _, item := range items {
		if item.Extra != "" {
			fmt.Fprintf(&b, "  %-16s  %s  %s\n", item.ID, item.Title, item.Extra)
		} else {
			fmt.Fprintf(&b, "  %-16s  %s\n", item.ID, item.Title)
		}
	}
	return b.String(), nil
}

// RenderStats formats spec stats as markdown or JSON.
func RenderStats(stats *SpecStats, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(stats, "", "  ")
		if err != nil {
			return "", err
		}
		return string(data), nil
	}

	var b strings.Builder
	b.WriteString("DDIS Spec Index Statistics\n")
	b.WriteString("═════════════════════════════════\n\n")
	fmt.Fprintf(&b, "  Path:        %s\n", stats.SpecPath)
	fmt.Fprintf(&b, "  Source Type: %s\n", stats.SourceType)
	if stats.DDISVersion != "" {
		fmt.Fprintf(&b, "  DDIS Version: %s\n", stats.DDISVersion)
	}
	fmt.Fprintf(&b, "  Total Lines: %d\n\n", stats.TotalLines)

	b.WriteString("Element Counts:\n")
	b.WriteString("─────────────────────────────────\n")

	displayOrder := []struct {
		key   string
		label string
	}{
		{"sections", "Sections"},
		{"invariants", "Invariants"},
		{"adrs", "ADRs"},
		{"quality_gates", "Quality Gates"},
		{"negative_specs", "Negative Specs"},
		{"verification_prompts", "Verification Prompts"},
		{"meta_instructions", "Meta-Instructions"},
		{"worked_examples", "Worked Examples"},
		{"why_not_annotations", "WHY NOT Annotations"},
		{"comparison_blocks", "Comparison Blocks"},
		{"performance_budgets", "Performance Budgets"},
		{"state_machines", "State Machines"},
		{"glossary_entries", "Glossary Entries"},
		{"modules", "Modules"},
	}

	for _, d := range displayOrder {
		if count, ok := stats.ElementCounts[d.key]; ok && count > 0 {
			fmt.Fprintf(&b, "  %-24s %d\n", d.label, count)
		}
	}

	b.WriteString("\nCross-References:\n")
	b.WriteString("─────────────────────────────────\n")
	fmt.Fprintf(&b, "  Total:      %d\n", stats.XRefTotal)
	fmt.Fprintf(&b, "  Resolved:   %d\n", stats.XRefResolved)
	fmt.Fprintf(&b, "  Resolution: %.1f%%\n", stats.XRefResolutionPc)

	return b.String(), nil
}
