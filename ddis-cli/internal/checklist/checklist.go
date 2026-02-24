package checklist

import (
	"database/sql"
	"sort"
	"strings"
	"unicode"

	"github.com/wvandaal/ddis/internal/storage"
)

// Options configures checklist generation.
type Options struct {
	Section   string // filter by section path prefix
	Invariant string // filter by single invariant ID
	AsJSON    bool
}

// ChecklistResult is the top-level output.
type ChecklistResult struct {
	Spec            string             `json:"spec"`
	TotalInvariants int                `json:"total_invariants"`
	TotalItems      int                `json:"total_items"`
	Sections        []SectionChecklist `json:"sections"`
}

// SectionChecklist groups checklist items under a section.
type SectionChecklist struct {
	Section string          `json:"section"`
	Title   string          `json:"title"`
	Items   []ChecklistItem `json:"items"`
}

// ChecklistItem is one invariant's verification steps.
type ChecklistItem struct {
	InvariantID      string   `json:"invariant_id"`
	Title            string   `json:"title"`
	ValidationMethod string   `json:"validation_method"`
	Checklist        []string `json:"checklist"`
}

// Analyze builds a checklist from invariants grouped by their containing section.
func Analyze(db *sql.DB, specID int64, opts Options) (*ChecklistResult, error) {
	spec, err := storage.GetSpecIndex(db, specID)
	if err != nil {
		return nil, err
	}

	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, err
	}

	sections, err := storage.ListSections(db, specID)
	if err != nil {
		return nil, err
	}

	// Build section ID → Section lookup
	sectionByID := make(map[int64]storage.Section, len(sections))
	for _, s := range sections {
		sectionByID[s.ID] = s
	}

	result := &ChecklistResult{
		Spec: spec.SpecName,
	}

	sectionMap := make(map[string]*SectionChecklist)

	for _, inv := range invs {
		if opts.Invariant != "" && inv.InvariantID != opts.Invariant {
			continue
		}

		// Determine containing section
		sec, ok := sectionByID[inv.SectionID]
		sectionPath := "Uncategorized"
		sectionTitle := ""
		if ok {
			sectionPath = sec.SectionPath
			sectionTitle = sec.Title
		}

		if opts.Section != "" && !strings.HasPrefix(sectionPath, opts.Section) {
			continue
		}

		items := parseValidationMethod(inv.ValidationMethod)
		if len(items) == 0 {
			// If no parseable steps, use the raw validation method as a single item
			if inv.ValidationMethod != "" {
				items = []string{strings.TrimSpace(inv.ValidationMethod)}
			} else {
				items = []string{"(no validation method specified)"}
			}
		}

		ci := ChecklistItem{
			InvariantID:      inv.InvariantID,
			Title:            inv.Title,
			ValidationMethod: inv.ValidationMethod,
			Checklist:        items,
		}

		if _, ok := sectionMap[sectionPath]; !ok {
			sectionMap[sectionPath] = &SectionChecklist{
				Section: sectionPath,
				Title:   sectionTitle,
			}
		}
		sectionMap[sectionPath].Items = append(sectionMap[sectionPath].Items, ci)
		result.TotalItems += len(items)
		result.TotalInvariants++
	}

	// Sort sections by path
	for _, sc := range sectionMap {
		result.Sections = append(result.Sections, *sc)
	}
	sort.Slice(result.Sections, func(i, j int) bool {
		return result.Sections[i].Section < result.Sections[j].Section
	})

	return result, nil
}

// parseValidationMethod splits a validation method text into individual checklist steps.
func parseValidationMethod(text string) []string {
	if text == "" {
		return nil
	}

	var items []string
	lines := strings.Split(text, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		// Skip markdown headers
		if strings.HasPrefix(line, "#") {
			continue
		}

		// Strip bullet markers
		line = stripListMarker(line)
		if line == "" {
			continue
		}
		items = append(items, line)
	}

	// If we got nothing from line splitting (single-line text), try splitting on semicolons
	if len(items) == 0 {
		parts := strings.Split(text, ";")
		for _, p := range parts {
			p = strings.TrimSpace(p)
			if p != "" {
				items = append(items, p)
			}
		}
	}

	return items
}

// stripListMarker removes leading bullet or numbered-list markers.
func stripListMarker(line string) string {
	// Markdown bullets: "- ", "* ", "+ "
	for _, prefix := range []string{"- ", "* ", "+ "} {
		if strings.HasPrefix(line, prefix) {
			return strings.TrimSpace(line[len(prefix):])
		}
	}

	// Numbered lists: "1. ", "12. ", etc.
	i := 0
	for i < len(line) && line[i] >= '0' && line[i] <= '9' {
		i++
	}
	if i > 0 && i < len(line) {
		rest := line[i:]
		if strings.HasPrefix(rest, ". ") || strings.HasPrefix(rest, ") ") {
			trimmed := strings.TrimSpace(rest[2:])
			if trimmed != "" {
				return trimmed
			}
		}
		// Handle "1." without space (just period then content)
		if len(rest) > 1 && rest[0] == '.' && unicode.IsLetter(rune(rest[1])) {
			return strings.TrimSpace(rest[1:])
		}
	}

	return line
}
