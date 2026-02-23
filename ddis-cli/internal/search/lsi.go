package search

import (
	"math"
	"regexp"
	"sort"
	"strings"

	"gonum.org/v1/gonum/mat"
)

// LSIIndex holds the Latent Semantic Indexing model.
type LSIIndex struct {
	K          int            // number of dimensions
	TermIndex  map[string]int // term → column index
	IDF        []float64      // IDF weight per term
	DocVectors [][]float64    // k-dimensional vector per document
	DocIDs     []string       // elementID per document (parallel to DocVectors)
	Uk         *mat.Dense     // truncated U matrix (terms × k)
	Sk         []float64      // truncated singular values
}

// RankedDoc holds a document with its similarity score.
type RankedDoc struct {
	DocIndex   int
	ElementID  string
	Similarity float64
}

var wordRe = regexp.MustCompile(`[a-zA-Z0-9§\-]+`)

// tokenize splits text into lowercase terms.
func tokenize(text string) []string {
	matches := wordRe.FindAllString(strings.ToLower(text), -1)
	return matches
}

// BuildLSI constructs an LSI index from documents via TF-IDF + truncated SVD.
func BuildLSI(docs []SearchDocument, k int) (*LSIIndex, error) {
	if len(docs) == 0 {
		return &LSIIndex{K: k}, nil
	}
	if k <= 0 {
		k = 50
	}

	// Build vocabulary and document term frequencies
	vocab := make(map[string]int)                 // term → index
	docTerms := make([]map[string]int, len(docs)) // per-doc term counts
	docFreq := make(map[string]int)               // term → number of docs containing it

	for i, doc := range docs {
		tokens := tokenize(doc.Content)
		tf := make(map[string]int)
		seen := make(map[string]bool)
		for _, t := range tokens {
			tf[t]++
			if !seen[t] {
				docFreq[t]++
				seen[t] = true
			}
			if _, ok := vocab[t]; !ok {
				vocab[t] = len(vocab)
			}
		}
		docTerms[i] = tf
	}

	nDocs := len(docs)
	nTerms := len(vocab)

	// Clamp k to min(nDocs, nTerms)
	maxK := nDocs
	if nTerms < maxK {
		maxK = nTerms
	}
	if k > maxK {
		k = maxK
	}
	if k <= 0 {
		k = 1
	}

	// Compute IDF: log(N / df)
	idf := make([]float64, nTerms)
	for term, idx := range vocab {
		df := docFreq[term]
		if df > 0 {
			idf[idx] = math.Log(float64(nDocs) / float64(df))
		}
	}

	// Build TF-IDF matrix (nTerms × nDocs)
	tfidfData := make([]float64, nTerms*nDocs)
	for docIdx, tf := range docTerms {
		for term, count := range tf {
			termIdx := vocab[term]
			// TF: 1 + log(count) for count > 0
			tfWeight := 1.0 + math.Log(float64(count))
			tfidfData[termIdx*nDocs+docIdx] = tfWeight * idf[termIdx]
		}
	}

	tfidf := mat.NewDense(nTerms, nDocs, tfidfData)

	// SVD: A = U * S * V^T
	var svd mat.SVD
	ok := svd.Factorize(tfidf, mat.SVDThin)
	if !ok {
		// Fallback: return empty LSI index
		return &LSIIndex{K: k, TermIndex: vocab, IDF: idf}, nil
	}

	// Extract truncated components
	var u, v mat.Dense
	svd.UTo(&u)
	svd.VTo(&v)
	sVals := svd.Values(nil)

	// Truncate to k dimensions
	uRows, _ := u.Dims()
	uk := mat.NewDense(uRows, k, nil)
	for i := 0; i < uRows; i++ {
		for j := 0; j < k; j++ {
			uk.Set(i, j, u.At(i, j))
		}
	}

	sk := make([]float64, k)
	copy(sk, sVals[:k])

	// Compute document vectors: V_k * S_k (each doc is a k-dimensional vector)
	vRows, _ := v.Dims()
	docVecs := make([][]float64, vRows)
	for i := 0; i < vRows; i++ {
		vec := make([]float64, k)
		for j := 0; j < k; j++ {
			vec[j] = v.At(i, j) * sk[j]
		}
		docVecs[i] = vec
	}

	docIDs := make([]string, len(docs))
	for i, doc := range docs {
		docIDs[i] = doc.ElementID
	}

	return &LSIIndex{
		K:          k,
		TermIndex:  vocab,
		IDF:        idf,
		DocVectors: docVecs,
		DocIDs:     docIDs,
		Uk:         uk,
		Sk:         sk,
	}, nil
}

// QueryVec projects a query string into the LSI space.
func (idx *LSIIndex) QueryVec(queryStr string) []float64 {
	if idx.Uk == nil || len(idx.Sk) == 0 {
		return nil
	}

	tokens := tokenize(queryStr)
	if len(tokens) == 0 {
		return nil
	}

	// Build query TF-IDF vector
	nTerms := len(idx.TermIndex)
	qVec := make([]float64, nTerms)
	tf := make(map[string]int)
	for _, t := range tokens {
		tf[t]++
	}
	for term, count := range tf {
		if termIdx, ok := idx.TermIndex[term]; ok {
			tfWeight := 1.0 + math.Log(float64(count))
			qVec[termIdx] = tfWeight * idx.IDF[termIdx]
		}
	}

	// Project into LSI space: q_k = q^T * U_k * S_k^{-1}
	qMat := mat.NewVecDense(nTerms, qVec)
	result := make([]float64, idx.K)
	for j := 0; j < idx.K; j++ {
		var dot float64
		for i := 0; i < nTerms; i++ {
			dot += qMat.AtVec(i) * idx.Uk.At(i, j)
		}
		if idx.Sk[j] > 1e-10 {
			result[j] = dot // Don't divide by Sk — keep consistent with doc vectors
		}
	}

	return result
}

// CosineSimilarity computes cosine similarity between the query vector and a document vector.
func (idx *LSIIndex) CosineSimilarity(queryVec []float64, docIdx int) float64 {
	if docIdx < 0 || docIdx >= len(idx.DocVectors) || queryVec == nil {
		return 0
	}
	return cosine(queryVec, idx.DocVectors[docIdx])
}

// RankAll ranks all documents by cosine similarity to the query vector.
func (idx *LSIIndex) RankAll(queryVec []float64) []RankedDoc {
	if queryVec == nil || len(idx.DocVectors) == 0 {
		return nil
	}

	ranked := make([]RankedDoc, len(idx.DocVectors))
	for i, dv := range idx.DocVectors {
		eid := ""
		if i < len(idx.DocIDs) {
			eid = idx.DocIDs[i]
		}
		ranked[i] = RankedDoc{
			DocIndex:   i,
			ElementID:  eid,
			Similarity: cosine(queryVec, dv),
		}
	}

	sort.Slice(ranked, func(i, j int) bool {
		return ranked[i].Similarity > ranked[j].Similarity
	})

	return ranked
}

func cosine(a, b []float64) float64 {
	if len(a) != len(b) {
		return 0
	}
	var dot, normA, normB float64
	for i := range a {
		dot += a[i] * b[i]
		normA += a[i] * a[i]
		normB += b[i] * b[i]
	}
	if normA == 0 || normB == 0 {
		return 0
	}
	return dot / (math.Sqrt(normA) * math.Sqrt(normB))
}
