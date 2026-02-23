package search

import (
	"database/sql"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// PopulateFTS populates the FTS5 index from extracted documents.
func PopulateFTS(db *sql.DB, docs []SearchDocument) error {
	if err := storage.ClearFTSIndex(db); err != nil {
		return fmt.Errorf("clear fts: %w", err)
	}

	for _, doc := range docs {
		if err := storage.InsertFTSEntry(db, doc.ElementType, doc.ElementID, doc.Title, doc.Content); err != nil {
			return fmt.Errorf("populate fts %s: %w", doc.ElementID, err)
		}
	}
	return nil
}

// FTSResult holds a ranked FTS5 search result.
type FTSResult struct {
	ElementType string
	ElementID   string
	Title       string
	Rank        float64 // BM25 rank (lower = better)
	Snippet     string
}

// SearchFTS performs a BM25 search over the FTS5 index.
func SearchFTS(db *sql.DB, queryStr string, limit int) ([]FTSResult, error) {
	if limit <= 0 {
		limit = 10
	}

	// Sanitize query for FTS5: escape double quotes, wrap terms
	ftsQuery := sanitizeFTSQuery(queryStr)
	if ftsQuery == "" {
		return nil, nil
	}

	sResults, err := storage.SearchFTS5(db, ftsQuery, limit)
	if err != nil {
		return nil, err
	}

	var results []FTSResult
	for _, sr := range sResults {
		results = append(results, FTSResult{
			ElementType: sr.ElementType,
			ElementID:   sr.ElementID,
			Title:       sr.Title,
			Rank:        sr.Rank,
			Snippet:     sr.Snippet,
		})
	}
	return results, nil
}

// sanitizeFTSQuery prepares a user query for FTS5.
// Handles exact phrases (quoted), element IDs, and plain terms.
func sanitizeFTSQuery(q string) string {
	q = strings.TrimSpace(q)
	if q == "" {
		return ""
	}

	// If it looks like an element ID (INV-006, ADR-003, §0.5, Gate-1), search as-is
	if isElementID(q) {
		return `"` + q + `"`
	}

	// If already quoted, pass through
	if strings.HasPrefix(q, `"`) && strings.HasSuffix(q, `"`) {
		return q
	}

	// Split into terms, join with implicit AND (FTS5 default)
	terms := strings.Fields(q)
	var sanitized []string
	for _, t := range terms {
		// Remove FTS5 operators from user input
		t = strings.TrimLeft(t, "-")
		t = strings.ReplaceAll(t, `"`, "")
		if t != "" && t != "OR" && t != "AND" && t != "NOT" {
			sanitized = append(sanitized, t)
		}
	}
	return strings.Join(sanitized, " ")
}

func isElementID(s string) bool {
	s = strings.TrimSpace(s)
	if strings.HasPrefix(s, "§") || strings.HasPrefix(s, "INV-") ||
		strings.HasPrefix(s, "ADR-") || strings.HasPrefix(s, "Gate-") ||
		strings.HasPrefix(s, "APP-INV-") || strings.HasPrefix(s, "APP-ADR-") ||
		strings.HasPrefix(s, "PART-") || strings.HasPrefix(s, "Chapter-") ||
		strings.HasPrefix(s, "Appendix-") {
		return true
	}
	return false
}
