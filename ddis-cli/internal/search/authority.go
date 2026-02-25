package search

import (
	"database/sql"
	"fmt"
	"math"

	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-004 (authority monotonicity)

// ComputeAuthority runs PageRank over the cross-reference graph and stores scores in the DB.
func ComputeAuthority(db *sql.DB, specID int64) (map[string]float64, error) {
	// Build adjacency list from cross-references
	rows, err := db.Query(
		`SELECT source_section_id, ref_target FROM cross_references
		 WHERE spec_id = ? AND source_section_id IS NOT NULL AND resolved = 1`, specID,
	)
	if err != nil {
		return nil, fmt.Errorf("query xrefs for authority: %w", err)
	}
	defer rows.Close()

	// Map section IDs to section paths for uniform node naming
	sectionPaths := make(map[int64]string)
	sections, err := storage.ListSections(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list sections for authority: %w", err)
	}
	for _, s := range sections {
		sectionPaths[s.ID] = s.SectionPath
	}

	// Build directed graph: source_path → target
	type edge struct{ from, to string }
	var edges []edge
	nodes := make(map[string]bool)

	for rows.Next() {
		var sourceSectionID int64
		var refTarget string
		if err := rows.Scan(&sourceSectionID, &refTarget); err != nil {
			return nil, fmt.Errorf("scan xref: %w", err)
		}
		sourcePath, ok := sectionPaths[sourceSectionID]
		if !ok {
			continue
		}
		edges = append(edges, edge{from: sourcePath, to: refTarget})
		nodes[sourcePath] = true
		nodes[refTarget] = true
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}

	if len(nodes) == 0 {
		return make(map[string]float64), nil
	}

	// Assign indices
	nodeList := make([]string, 0, len(nodes))
	nodeIdx := make(map[string]int)
	for n := range nodes {
		nodeIdx[n] = len(nodeList)
		nodeList = append(nodeList, n)
	}

	n := len(nodeList)
	damping := 0.85
	maxIter := 100
	convergence := 1e-6

	// Count outgoing edges per node
	outDegree := make([]int, n)
	for _, e := range edges {
		outDegree[nodeIdx[e.from]]++
	}

	// Initialize PageRank
	pr := make([]float64, n)
	newPR := make([]float64, n)
	initial := 1.0 / float64(n)
	for i := range pr {
		pr[i] = initial
	}

	// Iterative PageRank
	for iter := 0; iter < maxIter; iter++ {
		// Reset
		for i := range newPR {
			newPR[i] = (1.0 - damping) / float64(n)
		}

		// Distribute rank along edges
		for _, e := range edges {
			fromIdx := nodeIdx[e.from]
			toIdx := nodeIdx[e.to]
			if outDegree[fromIdx] > 0 {
				newPR[toIdx] += damping * pr[fromIdx] / float64(outDegree[fromIdx])
			}
		}

		// Handle dangling nodes (nodes with no outgoing edges)
		var danglingSum float64
		for i, d := range outDegree {
			if d == 0 {
				danglingSum += pr[i]
			}
		}
		danglingContrib := damping * danglingSum / float64(n)
		for i := range newPR {
			newPR[i] += danglingContrib
		}

		// Check convergence
		var diff float64
		for i := range pr {
			diff += math.Abs(newPR[i] - pr[i])
		}

		copy(pr, newPR)

		if diff < convergence {
			break
		}
	}

	// Build result map and persist to DB
	scores := make(map[string]float64)
	for i, nodeName := range nodeList {
		scores[nodeName] = pr[i]
		if err := storage.InsertAuthority(db, specID, nodeName, pr[i]); err != nil {
			return nil, fmt.Errorf("persist authority %s: %w", nodeName, err)
		}
	}

	return scores, nil
}
