package diff

import (
	"database/sql"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// MatchPair pairs a base element with its head counterpart (or marks as added/removed).
type MatchPair struct {
	ElementType string // "section", "invariant", "adr", "gate", "glossary", "negative_spec"
	ElementID   string // canonical ID (section_path, invariant_id, etc.)
	BaseDBID    *int64 // nil if added in head
	HeadDBID    *int64 // nil if removed from head
	BaseHash    string // content_hash of base element
	HeadHash    string // content_hash of head element
}

// MatchElements pairs all elements between base and head spec indexes.
func MatchElements(baseDB, headDB *sql.DB, baseSpec, headSpec int64) ([]MatchPair, error) {
	var pairs []MatchPair

	// Match sections
	sp, err := matchSections(baseDB, headDB, baseSpec, headSpec)
	if err != nil {
		return nil, err
	}
	pairs = append(pairs, sp...)

	// Match invariants
	ip, err := matchByID(baseDB, headDB, baseSpec, headSpec, "invariant",
		listInvariantIDs, getInvariantHash)
	if err != nil {
		return nil, err
	}
	pairs = append(pairs, ip...)

	// Match ADRs
	ap, err := matchByID(baseDB, headDB, baseSpec, headSpec, "adr",
		listADRIDs, getADRHash)
	if err != nil {
		return nil, err
	}
	pairs = append(pairs, ap...)

	// Match quality gates
	gp, err := matchByID(baseDB, headDB, baseSpec, headSpec, "gate",
		listGateIDs, getGateHash)
	if err != nil {
		return nil, err
	}
	pairs = append(pairs, gp...)

	// Match glossary entries
	glp, err := matchByID(baseDB, headDB, baseSpec, headSpec, "glossary",
		listGlossaryIDs, getGlossaryHash)
	if err != nil {
		return nil, err
	}
	pairs = append(pairs, glp...)

	return pairs, nil
}

// matchSections implements two-pass section matching: exact path, then fuzzy.
func matchSections(baseDB, headDB *sql.DB, baseSpec, headSpec int64) ([]MatchPair, error) {
	baseSecs, err := storage.ListSections(baseDB, baseSpec)
	if err != nil {
		return nil, err
	}
	headSecs, err := storage.ListSections(headDB, headSpec)
	if err != nil {
		return nil, err
	}

	// Index by section_path
	baseByPath := make(map[string]*storage.Section, len(baseSecs))
	for i := range baseSecs {
		baseByPath[baseSecs[i].SectionPath] = &baseSecs[i]
	}
	headByPath := make(map[string]*storage.Section, len(headSecs))
	for i := range headSecs {
		headByPath[headSecs[i].SectionPath] = &headSecs[i]
	}

	matched := make(map[string]bool) // head paths already matched
	var pairs []MatchPair

	// Pass 1: Exact match by section_path
	for _, bs := range baseSecs {
		if hs, ok := headByPath[bs.SectionPath]; ok {
			bid, hid := bs.ID, hs.ID
			pairs = append(pairs, MatchPair{
				ElementType: "section",
				ElementID:   bs.SectionPath,
				BaseDBID:    &bid,
				HeadDBID:    &hid,
				BaseHash:    bs.ContentHash,
				HeadHash:    hs.ContentHash,
			})
			matched[bs.SectionPath] = true
		}
	}

	// Pass 2: Fuzzy match for unmatched base sections (handles ~N disambiguation)
	for _, bs := range baseSecs {
		if matched[bs.SectionPath] {
			continue
		}
		// Look for a head section with similar path
		if hs := fuzzyMatchSection(&bs, headSecs, matched); hs != nil {
			bid, hid := bs.ID, hs.ID
			pairs = append(pairs, MatchPair{
				ElementType: "section",
				ElementID:   bs.SectionPath,
				BaseDBID:    &bid,
				HeadDBID:    &hid,
				BaseHash:    bs.ContentHash,
				HeadHash:    hs.ContentHash,
			})
			matched[hs.SectionPath] = true
		} else {
			// Removed from head
			bid := bs.ID
			pairs = append(pairs, MatchPair{
				ElementType: "section",
				ElementID:   bs.SectionPath,
				BaseDBID:    &bid,
				BaseHash:    bs.ContentHash,
			})
		}
	}

	// Any unmatched head sections are additions
	for _, hs := range headSecs {
		if !matched[hs.SectionPath] {
			hid := hs.ID
			pairs = append(pairs, MatchPair{
				ElementType: "section",
				ElementID:   hs.SectionPath,
				HeadDBID:    &hid,
				HeadHash:    hs.ContentHash,
			})
		}
	}

	return pairs, nil
}

// fuzzyMatchSection finds a head section with the same parent and similar title.
func fuzzyMatchSection(base *storage.Section, headSecs []storage.Section, matched map[string]bool) *storage.Section {
	baseParent := sectionParent(base.SectionPath)
	baseTitle := strings.ToLower(base.Title)

	for i := range headSecs {
		hs := &headSecs[i]
		if matched[hs.SectionPath] {
			continue
		}
		headParent := sectionParent(hs.SectionPath)
		if baseParent != headParent {
			continue
		}
		headTitle := strings.ToLower(hs.Title)
		// Case-insensitive prefix match or Levenshtein ≤ 3
		if strings.HasPrefix(headTitle, baseTitle) || strings.HasPrefix(baseTitle, headTitle) ||
			levenshtein(baseTitle, headTitle) <= 3 {
			return hs
		}
	}
	return nil
}

// sectionParent returns the parent portion of a section path.
// e.g., "§4.2.1" → "§4.2", "§4" → "§"
func sectionParent(path string) string {
	if idx := strings.LastIndex(path, "."); idx >= 0 {
		return path[:idx]
	}
	// For paths with ~N suffix, strip the suffix first
	if idx := strings.LastIndex(path, "~"); idx >= 0 {
		return sectionParent(path[:idx])
	}
	return ""
}

// levenshtein computes the edit distance between two strings.
func levenshtein(a, b string) int {
	if len(a) == 0 {
		return len(b)
	}
	if len(b) == 0 {
		return len(a)
	}

	prev := make([]int, len(b)+1)
	curr := make([]int, len(b)+1)

	for j := range prev {
		prev[j] = j
	}

	for i := 1; i <= len(a); i++ {
		curr[0] = i
		for j := 1; j <= len(b); j++ {
			cost := 1
			if a[i-1] == b[j-1] {
				cost = 0
			}
			curr[j] = min(curr[j-1]+1, min(prev[j]+1, prev[j-1]+cost))
		}
		prev, curr = curr, prev
	}
	return prev[len(b)]
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

// Generic ID-based matching for invariants, ADRs, gates, glossary.
type idLister func(db *sql.DB, specID int64) (map[string]int64, error) // ID → dbID
type hashGetter func(db *sql.DB, specID int64, id string) (string, error)

func matchByID(baseDB, headDB *sql.DB, baseSpec, headSpec int64, elemType string,
	lister idLister, hasher hashGetter) ([]MatchPair, error) {

	baseIDs, err := lister(baseDB, baseSpec)
	if err != nil {
		return nil, err
	}
	headIDs, err := lister(headDB, headSpec)
	if err != nil {
		return nil, err
	}

	var pairs []MatchPair

	// Matched + removed
	for id, bdbID := range baseIDs {
		bID := bdbID
		baseHash, _ := hasher(baseDB, baseSpec, id)

		if hdbID, ok := headIDs[id]; ok {
			hID := hdbID
			headHash, _ := hasher(headDB, headSpec, id)
			pairs = append(pairs, MatchPair{
				ElementType: elemType,
				ElementID:   id,
				BaseDBID:    &bID,
				HeadDBID:    &hID,
				BaseHash:    baseHash,
				HeadHash:    headHash,
			})
		} else {
			pairs = append(pairs, MatchPair{
				ElementType: elemType,
				ElementID:   id,
				BaseDBID:    &bID,
				BaseHash:    baseHash,
			})
		}
	}

	// Added
	for id, hdbID := range headIDs {
		if _, ok := baseIDs[id]; !ok {
			hID := hdbID
			headHash, _ := hasher(headDB, headSpec, id)
			pairs = append(pairs, MatchPair{
				ElementType: elemType,
				ElementID:   id,
				HeadDBID:    &hID,
				HeadHash:    headHash,
			})
		}
	}

	return pairs, nil
}

// ID listers for each element type.

func listInvariantIDs(db *sql.DB, specID int64) (map[string]int64, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, err
	}
	m := make(map[string]int64, len(invs))
	for _, inv := range invs {
		m[inv.InvariantID] = inv.ID
	}
	return m, nil
}

func listADRIDs(db *sql.DB, specID int64) (map[string]int64, error) {
	adrs, err := storage.ListADRs(db, specID)
	if err != nil {
		return nil, err
	}
	m := make(map[string]int64, len(adrs))
	for _, a := range adrs {
		m[a.ADRID] = a.ID
	}
	return m, nil
}

func listGateIDs(db *sql.DB, specID int64) (map[string]int64, error) {
	gates, err := storage.ListQualityGates(db, specID)
	if err != nil {
		return nil, err
	}
	m := make(map[string]int64, len(gates))
	for _, g := range gates {
		m[g.GateID] = g.ID
	}
	return m, nil
}

func listGlossaryIDs(db *sql.DB, specID int64) (map[string]int64, error) {
	entries, err := storage.ListGlossaryEntries(db, specID)
	if err != nil {
		return nil, err
	}
	m := make(map[string]int64, len(entries))
	for _, ge := range entries {
		m[ge.Term] = ge.ID
	}
	return m, nil
}

// Hash getters for each element type.

func getInvariantHash(db *sql.DB, specID int64, id string) (string, error) {
	inv, err := storage.GetInvariant(db, specID, id)
	if err != nil {
		return "", err
	}
	return inv.ContentHash, nil
}

func getADRHash(db *sql.DB, specID int64, id string) (string, error) {
	adr, err := storage.GetADR(db, specID, id)
	if err != nil {
		return "", err
	}
	return adr.ContentHash, nil
}

func getGateHash(db *sql.DB, specID int64, id string) (string, error) {
	// Quality gates don't have content_hash; use raw_text hash.
	gate, err := storage.GetQualityGate(db, specID, id)
	if err != nil {
		return "", err
	}
	return gate.RawText, nil // Use RawText as a proxy for identity
}

func getGlossaryHash(db *sql.DB, specID int64, id string) (string, error) {
	// Glossary entries don't have content_hash; use definition as proxy.
	entries, err := storage.ListGlossaryEntries(db, specID)
	if err != nil {
		return "", err
	}
	for _, ge := range entries {
		if ge.Term == id {
			return ge.Definition, nil
		}
	}
	return "", nil
}
