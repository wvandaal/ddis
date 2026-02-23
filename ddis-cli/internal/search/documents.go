package search

import (
	"database/sql"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// SearchDocument represents a single searchable document extracted from the spec index.
type SearchDocument struct {
	DocID       int    // internal index (0-based)
	ElementType string // section, invariant, adr, gate, glossary, negative_spec, etc.
	ElementID   string // §0.5, INV-006, ADR-003, etc.
	Title       string
	Content     string // concatenated searchable text
	SectionID   int64  // owning section DB id (for scoped queries)
}

// ExtractDocuments pulls all indexable elements from the database into searchable documents.
func ExtractDocuments(db *sql.DB, specID int64) ([]SearchDocument, error) {
	var docs []SearchDocument
	idx := 0

	// Sections
	sections, err := storage.ListSections(db, specID)
	if err != nil {
		return nil, fmt.Errorf("extract sections: %w", err)
	}
	for _, s := range sections {
		docs = append(docs, SearchDocument{
			DocID:       idx,
			ElementType: "section",
			ElementID:   s.SectionPath,
			Title:       s.Title,
			Content:     s.Title + "\n" + s.RawText,
			SectionID:   s.ID,
		})
		idx++
	}

	// Invariants
	invariants, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, fmt.Errorf("extract invariants: %w", err)
	}
	for _, inv := range invariants {
		content := joinNonEmpty("\n",
			inv.Title, inv.Statement, inv.SemiFormal,
			inv.ViolationScenario, inv.ValidationMethod, inv.WhyThisMatters,
		)
		docs = append(docs, SearchDocument{
			DocID:       idx,
			ElementType: "invariant",
			ElementID:   inv.InvariantID,
			Title:       inv.Title,
			Content:     content,
			SectionID:   inv.SectionID,
		})
		idx++
	}

	// ADRs
	adrs, err := storage.ListADRs(db, specID)
	if err != nil {
		return nil, fmt.Errorf("extract adrs: %w", err)
	}
	for _, a := range adrs {
		content := joinNonEmpty("\n",
			a.Title, a.Problem, a.DecisionText, a.Consequences,
		)
		docs = append(docs, SearchDocument{
			DocID:       idx,
			ElementType: "adr",
			ElementID:   a.ADRID,
			Title:       a.Title,
			Content:     content,
			SectionID:   a.SectionID,
		})
		idx++
	}

	// Quality Gates
	gates, err := storage.ListQualityGates(db, specID)
	if err != nil {
		return nil, fmt.Errorf("extract gates: %w", err)
	}
	for _, g := range gates {
		content := g.Title + "\n" + g.Predicate
		docs = append(docs, SearchDocument{
			DocID:       idx,
			ElementType: "gate",
			ElementID:   g.GateID,
			Title:       g.Title,
			Content:     content,
			SectionID:   g.SectionID,
		})
		idx++
	}

	// Glossary entries
	glossary, err := storage.ListGlossaryEntries(db, specID)
	if err != nil {
		return nil, fmt.Errorf("extract glossary: %w", err)
	}
	for _, ge := range glossary {
		content := ge.Term + "\n" + ge.Definition
		docs = append(docs, SearchDocument{
			DocID:       idx,
			ElementType: "glossary",
			ElementID:   "glossary:" + ge.Term,
			Title:       ge.Term,
			Content:     content,
			SectionID:   ge.SectionID,
		})
		idx++
	}

	// Negative specs
	negSpecs, err := storage.ListNegativeSpecs(db, specID)
	if err != nil {
		return nil, fmt.Errorf("extract negative_specs: %w", err)
	}
	for _, ns := range negSpecs {
		content := joinNonEmpty("\n", ns.ConstraintText, ns.Reason)
		eid := fmt.Sprintf("neg-spec:%d", ns.ID)
		docs = append(docs, SearchDocument{
			DocID:       idx,
			ElementType: "negative_spec",
			ElementID:   eid,
			Title:       ns.ConstraintText,
			Content:     content,
			SectionID:   ns.SectionID,
		})
		idx++
	}

	return docs, nil
}

// joinNonEmpty joins non-empty strings with the given separator.
func joinNonEmpty(sep string, parts ...string) string {
	var nonEmpty []string
	for _, p := range parts {
		if strings.TrimSpace(p) != "" {
			nonEmpty = append(nonEmpty, p)
		}
	}
	return strings.Join(nonEmpty, sep)
}
