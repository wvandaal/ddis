package exemplar

import (
	"database/sql"
	"sort"

	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// candidate holds a potential exemplar element with its extracted fields.
type candidate struct {
	elementType string
	elementID   string
	title       string
	fields      map[string]string
}

// FindExemplars finds the best exemplars for the given gaps from the corpus.
func FindExemplars(
	db *sql.DB,
	specID int64,
	target string,
	elementType string,
	gaps []ComponentGap,
	lsi *search.LSIIndex,
	opts Options,
) ([]Exemplar, error) {
	if len(gaps) == 0 {
		return nil, nil
	}

	limit := opts.Limit
	if limit <= 0 {
		limit = 3
	}
	minScore := opts.MinScore
	if minScore <= 0 {
		minScore = 0.3
	}

	// Collect candidates from the primary DB
	candidates, err := collectCandidates(db, specID, target, elementType)
	if err != nil {
		return nil, err
	}

	// Collect candidates from cross-spec corpus DBs
	for _, corpusPath := range opts.Corpus {
		corpusDB, err := storage.Open(corpusPath)
		if err != nil {
			continue // skip unreachable corpus DBs
		}
		corpusSpecID, err := storage.GetFirstSpecID(corpusDB)
		if err != nil {
			corpusDB.Close()
			continue
		}
		extra, err := collectCandidates(corpusDB, corpusSpecID, "", elementType)
		corpusDB.Close()
		if err != nil {
			continue
		}
		candidates = append(candidates, extra...)
	}

	// Get authority scores (PageRank)
	authorityScores, _ := storage.GetAuthorityScores(db, specID)

	// Build LSI similarity map for the target
	var targetSim map[string]float64
	if lsi != nil {
		targetSim = buildSimilarityMap(lsi, target)
	}

	// Find max authority for normalization
	maxAuth := 0.0
	for _, score := range authorityScores {
		if score > maxAuth {
			maxAuth = score
		}
	}

	var result []Exemplar
	for _, gap := range gaps {
		exemplars := scoreAndRank(candidates, gap, target, elementType, authorityScores, maxAuth, targetSim, limit, minScore)
		result = append(result, exemplars...)
	}

	return result, nil
}

// collectCandidates retrieves all elements of the given type, excluding the target.
func collectCandidates(db *sql.DB, specID int64, excludeID, elementType string) ([]candidate, error) {
	var candidates []candidate

	switch elementType {
	case "invariant":
		invs, err := storage.ListInvariants(db, specID)
		if err != nil {
			return nil, err
		}
		for _, inv := range invs {
			if inv.InvariantID == excludeID {
				continue // EX-INV-005: self-exclusion
			}
			candidates = append(candidates, candidate{
				elementType: "invariant",
				elementID:   inv.InvariantID,
				title:       inv.Title,
				fields:      ExtractInvariantFields(inv),
			})
		}
	case "adr":
		adrs, err := storage.ListADRs(db, specID)
		if err != nil {
			return nil, err
		}
		for _, adr := range adrs {
			if adr.ADRID == excludeID {
				continue // EX-INV-005: self-exclusion
			}
			candidates = append(candidates, candidate{
				elementType: "adr",
				elementID:   adr.ADRID,
				title:       adr.Title,
				fields:      ExtractADRFields(adr),
			})
		}
	}

	return candidates, nil
}

// scoreAndRank scores candidates for a specific gap and returns the top exemplars.
func scoreAndRank(
	candidates []candidate,
	gap ComponentGap,
	targetID, elementType string,
	authorityScores map[string]float64,
	maxAuth float64,
	targetSim map[string]float64,
	limit int,
	minScore float64,
) []Exemplar {
	type scored struct {
		cand    candidate
		score   float64
		signals ExemplarSignals
	}

	var scoredCandidates []scored
	for _, cand := range candidates {
		// EX-INV-003: only candidates with strong gap component
		substanceScore := WeakScore(cand.fields[gap.Component], gap.Component, elementType)
		if substanceScore <= 0.6 {
			continue
		}

		// Completeness: count of non-empty fields / total fields
		components := ComponentsForType(elementType)
		nonEmpty := 0
		for _, comp := range components {
			if cand.fields[comp] != "" {
				nonEmpty++
			}
		}
		completeness := float64(nonEmpty) / float64(len(components))

		// Authority: normalized PageRank
		authNorm := 0.0
		if maxAuth > 0 {
			authNorm = authorityScores[cand.elementID] / maxAuth
		}

		// Similarity: LSI cosine
		similarity := 0.0
		if targetSim != nil {
			similarity = targetSim[cand.elementID]
		}

		// Composite quality score
		quality := 0.25*completeness + 0.35*substanceScore + 0.15*authNorm + 0.25*similarity

		scoredCandidates = append(scoredCandidates, scored{
			cand:  cand,
			score: quality,
			signals: ExemplarSignals{
				Completeness: completeness,
				Substance:    substanceScore,
				Authority:    authNorm,
				Similarity:   similarity,
			},
		})
	}

	// EX-INV-004: sort by quality score descending
	sort.Slice(scoredCandidates, func(i, j int) bool {
		return scoredCandidates[i].score > scoredCandidates[j].score
	})

	// Take top N above threshold
	var result []Exemplar
	for _, sc := range scoredCandidates {
		if sc.score < minScore {
			break
		}
		if len(result) >= limit {
			break
		}

		cue := GenerateSubstrateCue(sc.cand.elementID, targetID, gap.Component)
		result = append(result, Exemplar{
			ElementType:           sc.cand.elementType,
			ElementID:             sc.cand.elementID,
			Title:                 sc.cand.title,
			QualityScore:          roundTo(sc.score, 2),
			Signals:               roundSignals(sc.signals),
			DemonstratedComponent: gap.Component,
			Content:               sc.cand.fields[gap.Component],
			SubstrateCue:          cue,
		})
	}

	return result
}

// buildSimilarityMap computes LSI cosine similarity from the target to all documents.
func buildSimilarityMap(lsi *search.LSIIndex, targetID string) map[string]float64 {
	// Find target's doc index
	targetIdx := -1
	for i, id := range lsi.DocIDs {
		if id == targetID {
			targetIdx = i
			break
		}
	}

	var queryVec []float64
	if targetIdx >= 0 && targetIdx < len(lsi.DocVectors) {
		queryVec = lsi.DocVectors[targetIdx]
	} else {
		// Target not in LSI index; project its ID as text
		queryVec = lsi.QueryVec(targetID)
	}
	if queryVec == nil {
		return nil
	}

	ranked := lsi.RankAll(queryVec)
	simMap := make(map[string]float64, len(ranked))
	for _, rd := range ranked {
		simMap[rd.ElementID] = rd.Similarity
	}
	return simMap
}

// roundTo rounds a float to n decimal places.
func roundTo(val float64, places int) float64 {
	shift := 1.0
	for i := 0; i < places; i++ {
		shift *= 10
	}
	return float64(int(val*shift+0.5)) / shift
}

// roundSignals rounds all signal values to 2 decimal places.
func roundSignals(s ExemplarSignals) ExemplarSignals {
	return ExemplarSignals{
		Completeness: roundTo(s.Completeness, 2),
		Substance:    roundTo(s.Substance, 2),
		Authority:    roundTo(s.Authority, 2),
		Similarity:   roundTo(s.Similarity, 2),
	}
}
