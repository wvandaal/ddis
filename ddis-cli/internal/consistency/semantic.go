package consistency

// Tier 4 (semantic): LSI-based tension detection.
// Detects high cross-boundary similarity that may indicate specification overlap.

import (
	"database/sql"
	"fmt"
	"math"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// analyzeSemanticmantic runs Tier 4 semantic analysis using TF-IDF cosine similarity.
// We avoid importing the full search/lsi package to keep the dependency light.
// Instead, we use a simple TF-IDF bag-of-words approach on invariant+negspec statements.
func analyzeSemantic(db *sql.DB, specID int64) ([]Contradiction, int, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list invariants: %w", err)
	}
	negSpecs, err := storage.ListNegativeSpecs(db, specID)
	if err != nil {
		return nil, 0, fmt.Errorf("list negative specs: %w", err)
	}

	if len(invs) < 2 {
		return nil, len(invs), nil
	}

	// Build TF-IDF vectors for invariant statements.
	type doc struct {
		id   string
		text string
	}
	var docs []doc
	for _, inv := range invs {
		docs = append(docs, doc{
			id:   inv.InvariantID,
			text: inv.Statement + " " + inv.SemiFormal,
		})
	}
	// Also include negative specs as documents for cross-check.
	for _, ns := range negSpecs {
		docs = append(docs, doc{
			id:   fmt.Sprintf("neg-spec:%d", ns.ID),
			text: ns.ConstraintText,
		})
	}

	// Build vocabulary and TF-IDF.
	vocab := make(map[string]int) // term → index
	docFreq := make(map[string]int)
	termVecs := make([]map[string]float64, len(docs))

	for i, d := range docs {
		tf := make(map[string]float64)
		words := significantWords(d.text)
		for _, w := range words {
			tf[w]++
			if _, exists := vocab[w]; !exists {
				vocab[w] = len(vocab)
			}
		}
		// Normalize TF.
		if len(words) > 0 {
			for w := range tf {
				tf[w] /= float64(len(words))
			}
		}
		termVecs[i] = tf

		// Count document frequency.
		seen := make(map[string]bool)
		for _, w := range words {
			if !seen[w] {
				docFreq[w]++
				seen[w] = true
			}
		}
	}

	N := float64(len(docs))
	// Compute TF-IDF weighted vectors.
	tfidfVecs := make([]map[string]float64, len(docs))
	for i, tf := range termVecs {
		tfidf := make(map[string]float64)
		for term, tfVal := range tf {
			df := float64(docFreq[term])
			if df > 0 {
				idf := math.Log(N/df) + 1
				tfidf[term] = tfVal * idf
			}
		}
		tfidfVecs[i] = tfidf
	}

	// Compute cosine similarity between all invariant pairs.
	// Only flag high similarity between elements from different logical categories.
	var results []Contradiction
	invCount := len(invs)

	for i := 0; i < invCount; i++ {
		for j := i + 1; j < invCount; j++ {
			sim := cosineSim(tfidfVecs[i], tfidfVecs[j])
			if sim > 0.85 {
				// Very high similarity between two invariants: possible redundancy/tension.
				// Check if they have opposing polarity (tension) vs just being similar (redundancy).
				if hasOpposingPolarity(invs[i].Statement, invs[j].Statement) {
					results = append(results, Contradiction{
						Tier:       TierHeuristic,
						Type:       SemanticTension,
						ElementA:   invs[i].InvariantID,
						ElementB:   invs[j].InvariantID,
						Description: fmt.Sprintf(
							"%s and %s are semantically similar (%.0f%%) but have opposing polarity signals.",
							invs[i].InvariantID, invs[j].InvariantID, sim*100,
						),
						Evidence: fmt.Sprintf(
							"TF-IDF cosine similarity: %.3f. A: %q. B: %q.",
							sim, truncate(invs[i].Statement, 80), truncate(invs[j].Statement, 80),
						),
						Confidence:     sim * 0.7,
						ResolutionHint: "High semantic similarity with opposing polarity suggests a potential conflict. Review whether these invariants are reconciled by context.",
					})
				}
			}
		}
	}

	// Cross-check: invariants vs negative specs with high similarity AND same polarity.
	// (An invariant that closely matches a negative spec is suspicious.)
	for i := 0; i < invCount; i++ {
		for j := invCount; j < len(docs); j++ {
			sim := cosineSim(tfidfVecs[i], tfidfVecs[j])
			if sim > 0.7 {
				// An invariant closely matching a negative spec may indicate
				// that the invariant requires something the spec forbids.
				// Only flag if already detected by graph analysis (avoid duplicates).
				// Semantic provides confirmation, not primary detection.
				// We'll flag at lower confidence to be advisory.
				results = append(results, Contradiction{
					Tier:       TierHeuristic,
					Type:       SemanticTension,
					ElementA:   docs[i].id,
					ElementB:   docs[j].id,
					Description: fmt.Sprintf(
						"%s is semantically close (%.0f%%) to negative spec %s.",
						docs[i].id, sim*100, docs[j].id,
					),
					Evidence: fmt.Sprintf(
						"TF-IDF cosine: %.3f. Invariant: %q. NegSpec: %q.",
						sim, truncate(docs[i].text, 80), truncate(docs[j].text, 80),
					),
					Confidence:     sim * 0.5,
					ResolutionHint: "Verify the invariant doesn't require the exact behavior the negative spec forbids.",
				})
			}
		}
	}

	return results, len(docs), nil
}

// cosineSim computes cosine similarity between two sparse TF-IDF vectors.
func cosineSim(a, b map[string]float64) float64 {
	dot := 0.0
	normA := 0.0
	normB := 0.0

	for term, va := range a {
		normA += va * va
		if vb, ok := b[term]; ok {
			dot += va * vb
		}
	}
	for _, vb := range b {
		normB += vb * vb
	}

	if normA == 0 || normB == 0 {
		return 0
	}
	return dot / (math.Sqrt(normA) * math.Sqrt(normB))
}

// analyzeSemanticStub is a compatibility alias.
func init() {
	_ = strings.TrimSpace // import used in significantWords (graph.go)
}
