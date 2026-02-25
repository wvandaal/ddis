package search

// ddis:maintains APP-INV-012 (LSI dimension bound)

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"sort"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// SearchResult is a single search result with fused signals.
type SearchResult struct {
	ElementType string         `json:"element_type"`
	ElementID   string         `json:"element_id"`
	Title       string         `json:"title"`
	Score       float64        `json:"score"`
	Snippet     string         `json:"snippet,omitempty"`
	Signals     map[string]int `json:"signals,omitempty"` // signal → rank
}

// SearchOptions controls search behavior.
type SearchOptions struct {
	Limit           int
	TypeFilter      string // empty = all types
	LexicalOnly     bool   // skip LSI
	IncludeSnippets bool
}

// BuildIndex populates FTS5, computes LSI vectors, and computes PageRank authority.
// Should be called after parsing a spec.
func BuildIndex(db *sql.DB, specID int64) error {
	// Extract documents
	docs, err := ExtractDocuments(db, specID)
	if err != nil {
		return fmt.Errorf("extract documents: %w", err)
	}

	if len(docs) == 0 {
		return nil
	}

	// Clear existing search data
	if err := storage.ClearSearchData(db, specID); err != nil {
		return fmt.Errorf("clear search data: %w", err)
	}

	// Populate FTS5
	if err := PopulateFTS(db, docs); err != nil {
		return fmt.Errorf("populate fts: %w", err)
	}

	// Build LSI index
	k := 50
	if len(docs) < k {
		k = len(docs)
	}
	lsiIndex, err := BuildLSI(docs, k)
	if err != nil {
		return fmt.Errorf("build lsi: %w", err)
	}

	// Serialize and store LSI model
	if lsiIndex.Uk != nil {
		modelData, err := json.Marshal(struct {
			K         int            `json:"k"`
			TermIndex map[string]int `json:"term_index"`
			IDF       []float64      `json:"idf"`
			DocIDs    []string       `json:"doc_ids"`
		}{
			K:         lsiIndex.K,
			TermIndex: lsiIndex.TermIndex,
			IDF:       lsiIndex.IDF,
			DocIDs:    lsiIndex.DocIDs,
		})
		if err == nil {
			_ = storage.InsertSearchModel(db, specID, "lsi", lsiIndex.K,
				len(lsiIndex.TermIndex), len(docs), modelData)
		}
	}

	// Compute PageRank authority
	if _, err := ComputeAuthority(db, specID); err != nil {
		return fmt.Errorf("compute authority: %w", err)
	}

	return nil
}

// Search executes a hybrid search combining BM25, LSI, and authority signals.
func Search(db *sql.DB, specID int64, queryStr string, opts SearchOptions) ([]SearchResult, error) {
	if strings.TrimSpace(queryStr) == "" {
		return nil, fmt.Errorf("search query cannot be empty")
	}

	if opts.Limit <= 0 {
		opts.Limit = 10
	}

	// Glossary expansion
	expandedTerms := expandQuery(db, specID, queryStr)
	fullQuery := queryStr
	if len(expandedTerms) > 0 {
		fullQuery = queryStr + " " + strings.Join(expandedTerms, " ")
	}

	// Signal 1: BM25 via FTS5
	ftsResults, err := SearchFTS(db, fullQuery, opts.Limit*3) // over-fetch for fusion
	if err != nil {
		// FTS5 might fail with certain queries; fall through
		ftsResults = nil
	}

	// Build per-element rankings
	type elementInfo struct {
		elementType string
		elementID   string
		title       string
		snippet     string
		bm25Rank    int // 0 = not ranked
		lsiRank     int
		authRank    int
	}

	elements := make(map[string]*elementInfo) // keyed by elementID

	// Index BM25 results
	for i, r := range ftsResults {
		if opts.TypeFilter != "" && r.ElementType != opts.TypeFilter {
			continue
		}
		elements[r.ElementID] = &elementInfo{
			elementType: r.ElementType,
			elementID:   r.ElementID,
			title:       r.Title,
			snippet:     r.Snippet,
			bm25Rank:    i + 1,
		}
	}

	// Signal 2: LSI (if not lexical-only)
	if !opts.LexicalOnly {
		docs, err := ExtractDocuments(db, specID)
		if err == nil && len(docs) > 0 {
			k := 50
			if len(docs) < k {
				k = len(docs)
			}
			lsiIdx, err := BuildLSI(docs, k)
			if err == nil && lsiIdx.Uk != nil {
				qVec := lsiIdx.QueryVec(fullQuery)
				if qVec != nil {
					ranked := lsiIdx.RankAll(qVec)
					for rank, rd := range ranked {
						if rd.Similarity <= 0 {
							continue
						}
						if opts.TypeFilter != "" {
							// Check type by looking up in docs
							if rd.DocIndex < len(docs) && docs[rd.DocIndex].ElementType != opts.TypeFilter {
								continue
							}
						}
						eid := rd.ElementID
						if el, ok := elements[eid]; ok {
							el.lsiRank = rank + 1
						} else {
							if rd.DocIndex < len(docs) {
								doc := docs[rd.DocIndex]
								elements[eid] = &elementInfo{
									elementType: doc.ElementType,
									elementID:   doc.ElementID,
									title:       doc.Title,
									lsiRank:     rank + 1,
								}
							}
						}
					}
				}
			}
		}
	}

	// Signal 3: Authority (PageRank)
	authScores, _ := storage.GetAuthorityScores(db, specID)
	if len(authScores) > 0 {
		// Sort by authority score descending
		type authEntry struct {
			id    string
			score float64
		}
		var authList []authEntry
		for id, score := range authScores {
			authList = append(authList, authEntry{id, score})
		}
		sort.Slice(authList, func(i, j int) bool {
			return authList[i].score > authList[j].score
		})

		for rank, ae := range authList {
			if el, ok := elements[ae.id]; ok {
				el.authRank = rank + 1
			}
		}
	}

	// Signal 4: Type boosting (applied as rank adjustment in RRF)
	typeBoost := map[string]float64{
		"invariant":     1.2,
		"adr":           1.1,
		"gate":          1.1,
		"section":       1.0,
		"glossary":      0.8,
		"negative_spec": 0.9,
	}

	// RRF fusion: score(d) = Σ 1/(K + rank(d))
	const rrfK = 60.0

	var results []SearchResult
	for _, el := range elements {
		score := 0.0
		signals := make(map[string]int)

		if el.bm25Rank > 0 {
			score += 1.0 / (rrfK + float64(el.bm25Rank))
			signals["bm25"] = el.bm25Rank
		}
		if el.lsiRank > 0 {
			score += 1.0 / (rrfK + float64(el.lsiRank))
			signals["lsi"] = el.lsiRank
		}
		if el.authRank > 0 {
			score += 0.5 / (rrfK + float64(el.authRank)) // authority weighted lower
			signals["authority"] = el.authRank
		}

		// Apply type boost
		if boost, ok := typeBoost[el.elementType]; ok {
			score *= boost
		}

		snippet := ""
		if opts.IncludeSnippets {
			snippet = el.snippet
		}

		results = append(results, SearchResult{
			ElementType: el.elementType,
			ElementID:   el.elementID,
			Title:       el.title,
			Score:       score,
			Snippet:     snippet,
			Signals:     signals,
		})
	}

	// Sort by score descending
	sort.Slice(results, func(i, j int) bool {
		return results[i].Score > results[j].Score
	})

	// Limit
	if len(results) > opts.Limit {
		results = results[:opts.Limit]
	}

	return results, nil
}

// expandQuery uses the glossary to expand query terms with domain synonyms.
func expandQuery(db *sql.DB, specID int64, queryStr string) []string {
	glossary, err := storage.ListGlossaryEntries(db, specID)
	if err != nil || len(glossary) == 0 {
		return nil
	}

	queryLower := strings.ToLower(queryStr)
	var expansions []string

	for _, ge := range glossary {
		termLower := strings.ToLower(ge.Term)
		defLower := strings.ToLower(ge.Definition)

		// If query contains a glossary term, add key words from its definition
		if strings.Contains(queryLower, termLower) {
			// Extract significant words from definition (skip short ones)
			words := strings.Fields(defLower)
			for _, w := range words {
				w = strings.Trim(w, ".,;:()\"'")
				if len(w) > 4 && !strings.Contains(queryLower, w) {
					expansions = append(expansions, w)
					if len(expansions) >= 5 {
						return expansions
					}
				}
			}
		}

		// If query words appear in a glossary definition, add the term
		queryWords := strings.Fields(queryLower)
		for _, qw := range queryWords {
			if len(qw) > 3 && strings.Contains(defLower, qw) && !strings.Contains(queryLower, termLower) {
				expansions = append(expansions, ge.Term)
				if len(expansions) >= 5 {
					return expansions
				}
				break
			}
		}
	}

	return expansions
}

// RenderSearch formats search results for output.
func RenderSearch(results []SearchResult, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(results, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal search results: %w", err)
		}
		return string(data), nil
	}

	var b strings.Builder
	fmt.Fprintf(&b, "Search Results (%d matches)\n", len(results))
	b.WriteString("═══════════════════════════════════════════\n\n")

	if len(results) == 0 {
		b.WriteString("No results found.\n")
		return b.String(), nil
	}

	for i, r := range results {
		fmt.Fprintf(&b, "%d. [%s] %s: %s  (score: %.4f)\n",
			i+1, r.ElementType, r.ElementID, r.Title, r.Score)
		if r.Snippet != "" {
			fmt.Fprintf(&b, "   %s\n", r.Snippet)
		}
		if len(r.Signals) > 0 {
			var sigs []string
			for sig, rank := range r.Signals {
				sigs = append(sigs, fmt.Sprintf("%s=#%d", sig, rank))
			}
			sort.Strings(sigs)
			fmt.Fprintf(&b, "   signals: %s\n", strings.Join(sigs, ", "))
		}
		b.WriteString("\n")
	}

	return b.String(), nil
}
