package impact

// ddis:maintains APP-INV-013 (invariant ownership)

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"regexp"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ImpactNode represents one element reachable from the analysis target.
type ImpactNode struct {
	ElementType string `json:"element_type"`
	ElementID   string `json:"element_id"`
	Title       string `json:"title"`
	Distance    int    `json:"distance"`
	Via         string `json:"via"` // the cross-reference that led here
}

// ImpactResult holds the complete impact analysis output.
type ImpactResult struct {
	Target     string       `json:"target"`
	Direction  string       `json:"direction"`
	MaxDepth   int          `json:"max_depth"`
	Nodes      []ImpactNode `json:"nodes"`
	TotalCount int          `json:"total_count"`
}

// ImpactOptions controls the impact analysis behavior.
type ImpactOptions struct {
	MaxDepth  int    // default 2, max 5
	Direction string // "forward" (default), "backward", "both"
}

var (
	sectionRe = regexp.MustCompile(`^§(\d+(?:\.\d+)*)$`)
	invRe     = regexp.MustCompile(`^((?:APP-)?INV-\d{3})$`)
	adrRe     = regexp.MustCompile(`^((?:APP-)?ADR-\d{3})$`)
	gateRe    = regexp.MustCompile(`^Gate-?((?:M-)?[1-9]\d*)$`)
)

// parseImpactTarget normalizes a target string and returns (elementType, canonicalID).
func parseImpactTarget(target string) (string, string, error) {
	target = strings.TrimSpace(target)

	if m := sectionRe.FindStringSubmatch(target); m != nil {
		return "section", "§" + m[1], nil
	}
	if m := invRe.FindStringSubmatch(target); m != nil {
		return "invariant", strings.ToUpper(m[1]), nil
	}
	if m := adrRe.FindStringSubmatch(target); m != nil {
		return "adr", strings.ToUpper(m[1]), nil
	}
	if m := gateRe.FindStringSubmatch(target); m != nil {
		return "gate", "Gate-" + m[1], nil
	}

	if strings.HasPrefix(target, "PART-") || strings.HasPrefix(target, "Chapter-") ||
		strings.HasPrefix(target, "Appendix-") {
		return "section", target, nil
	}

	return "", "", fmt.Errorf("cannot parse target %q: expected §N.M, INV-NNN, ADR-NNN, Gate-N, PART-N, Chapter-N, or Appendix-X", target)
}

// Analyze performs BFS impact analysis from the given target.
func Analyze(db *sql.DB, specID int64, target string, opts ImpactOptions) (*ImpactResult, error) {
	if opts.MaxDepth <= 0 {
		opts.MaxDepth = 2
	}
	if opts.MaxDepth > 5 {
		opts.MaxDepth = 5
	}
	if opts.Direction == "" {
		opts.Direction = "forward"
	}

	_, canonicalID, err := parseImpactTarget(target)
	if err != nil {
		return nil, err
	}

	// Verify the target exists
	if !elementExists(db, specID, canonicalID) {
		return nil, fmt.Errorf("target %q not found in spec index", canonicalID)
	}

	result := &ImpactResult{
		Target:    canonicalID,
		Direction: opts.Direction,
		MaxDepth:  opts.MaxDepth,
	}

	visited := make(map[string]bool)
	visited[canonicalID] = true

	switch opts.Direction {
	case "forward":
		result.Nodes = bfsForward(db, specID, canonicalID, opts.MaxDepth, visited)
	case "backward":
		result.Nodes = bfsBackward(db, specID, canonicalID, opts.MaxDepth, visited)
	case "both":
		fwd := bfsForward(db, specID, canonicalID, opts.MaxDepth, visited)
		// Reset visited for backward pass (target remains visited)
		visited2 := make(map[string]bool)
		visited2[canonicalID] = true
		for _, n := range fwd {
			visited2[n.ElementID] = true
		}
		bwd := bfsBackward(db, specID, canonicalID, opts.MaxDepth, visited2)
		result.Nodes = append(fwd, bwd...)
	}

	result.TotalCount = len(result.Nodes)
	return result, nil
}

// bfsForward finds everything that references the target (what's affected if we change it).
func bfsForward(db *sql.DB, specID int64, startID string, maxDepth int, visited map[string]bool) []ImpactNode {
	var nodes []ImpactNode
	queue := []struct {
		id    string
		depth int
	}{{startID, 0}}

	for len(queue) > 0 {
		cur := queue[0]
		queue = queue[1:]

		if cur.depth >= maxDepth {
			continue
		}

		// Find all cross-references that target cur.id (backlinks = things referencing us)
		backlinks, err := storage.GetBacklinks(db, specID, cur.id)
		if err != nil {
			continue
		}

		for _, bl := range backlinks {
			// Determine the source element's canonical ID
			sourceID := resolveSourceID(db, bl)
			if sourceID == "" || visited[sourceID] {
				continue
			}
			visited[sourceID] = true

			title := resolveTitle(db, specID, sourceID)
			nodes = append(nodes, ImpactNode{
				ElementType: bl.RefType,
				ElementID:   sourceID,
				Title:       title,
				Distance:    cur.depth + 1,
				Via:         fmt.Sprintf("%s → %s", sourceID, cur.id),
			})

			queue = append(queue, struct {
				id    string
				depth int
			}{sourceID, cur.depth + 1})
		}
	}
	return nodes
}

// bfsBackward finds what the target references (what it depends on).
func bfsBackward(db *sql.DB, specID int64, startID string, maxDepth int, visited map[string]bool) []ImpactNode {
	var nodes []ImpactNode

	// First we need the section ID for this target
	sectionID := resolveSectionID(db, specID, startID)
	if sectionID <= 0 {
		return nodes
	}

	queue := []struct {
		secID int64
		id    string
		depth int
	}{{sectionID, startID, 0}}

	for len(queue) > 0 {
		cur := queue[0]
		queue = queue[1:]

		if cur.depth >= maxDepth {
			continue
		}

		// Find outgoing refs from this section
		refs, err := storage.GetOutgoingRefs(db, specID, cur.secID)
		if err != nil {
			continue
		}

		for _, ref := range refs {
			targetID := ref.RefTarget
			if visited[targetID] {
				continue
			}
			visited[targetID] = true

			title := resolveTitle(db, specID, targetID)
			nodes = append(nodes, ImpactNode{
				ElementType: ref.RefType,
				ElementID:   targetID,
				Title:       title,
				Distance:    cur.depth + 1,
				Via:         fmt.Sprintf("%s → %s", cur.id, targetID),
			})

			// Resolve section for next hop
			nextSecID := resolveSectionID(db, specID, targetID)
			if nextSecID > 0 {
				queue = append(queue, struct {
					secID int64
					id    string
					depth int
				}{nextSecID, targetID, cur.depth + 1})
			}
		}
	}
	return nodes
}

// resolveSourceID returns the canonical ID of the section containing a cross-reference.
func resolveSourceID(db *sql.DB, xref storage.CrossReference) string {
	if xref.SourceSectionID == nil {
		return ""
	}
	var path string
	err := db.QueryRow("SELECT section_path FROM sections WHERE id = ?", *xref.SourceSectionID).Scan(&path)
	if err != nil {
		return ""
	}
	return path
}

// resolveSectionID returns the section database ID for a given canonical element ID.
func resolveSectionID(db *sql.DB, specID int64, elementID string) int64 {
	// Try as a direct section
	var id int64
	err := db.QueryRow("SELECT id FROM sections WHERE spec_id = ? AND section_path = ?",
		specID, elementID).Scan(&id)
	if err == nil {
		return id
	}

	// Try as an invariant's section
	err = db.QueryRow("SELECT section_id FROM invariants WHERE spec_id = ? AND invariant_id = ?",
		specID, elementID).Scan(&id)
	if err == nil {
		return id
	}

	// Try as an ADR's section
	err = db.QueryRow("SELECT section_id FROM adrs WHERE spec_id = ? AND adr_id = ?",
		specID, elementID).Scan(&id)
	if err == nil {
		return id
	}

	// Try as a gate's section
	err = db.QueryRow("SELECT section_id FROM quality_gates WHERE spec_id = ? AND gate_id = ?",
		specID, elementID).Scan(&id)
	if err == nil {
		return id
	}

	return 0
}

// resolveTitle returns the title for an element by its canonical ID.
func resolveTitle(db *sql.DB, specID int64, elementID string) string {
	var title string

	// Try section
	if err := db.QueryRow("SELECT title FROM sections WHERE spec_id = ? AND section_path = ?",
		specID, elementID).Scan(&title); err == nil {
		return title
	}

	// Try invariant
	if err := db.QueryRow("SELECT title FROM invariants WHERE spec_id = ? AND invariant_id = ?",
		specID, elementID).Scan(&title); err == nil {
		return title
	}

	// Try ADR
	if err := db.QueryRow("SELECT title FROM adrs WHERE spec_id = ? AND adr_id = ?",
		specID, elementID).Scan(&title); err == nil {
		return title
	}

	// Try gate
	if err := db.QueryRow("SELECT title FROM quality_gates WHERE spec_id = ? AND gate_id = ?",
		specID, elementID).Scan(&title); err == nil {
		return title
	}

	return ""
}

// elementExists checks if an element with the given ID exists in the spec.
func elementExists(db *sql.DB, specID int64, id string) bool {
	var count int
	// Check sections
	if db.QueryRow("SELECT COUNT(*) FROM sections WHERE spec_id = ? AND section_path = ?",
		specID, id).Scan(&count) == nil && count > 0 {
		return true
	}
	// Check invariants
	if db.QueryRow("SELECT COUNT(*) FROM invariants WHERE spec_id = ? AND invariant_id = ?",
		specID, id).Scan(&count) == nil && count > 0 {
		return true
	}
	// Check ADRs
	if db.QueryRow("SELECT COUNT(*) FROM adrs WHERE spec_id = ? AND adr_id = ?",
		specID, id).Scan(&count) == nil && count > 0 {
		return true
	}
	// Check gates
	if db.QueryRow("SELECT COUNT(*) FROM quality_gates WHERE spec_id = ? AND gate_id = ?",
		specID, id).Scan(&count) == nil && count > 0 {
		return true
	}
	return false
}

// RenderImpact formats an ImpactResult for human or JSON output.
func RenderImpact(result *ImpactResult, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(result, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal impact: %w", err)
		}
		return string(data), nil
	}

	return renderHumanImpact(result), nil
}

func renderHumanImpact(r *ImpactResult) string {
	var b strings.Builder

	dirLabel := "Forward Impact"
	switch r.Direction {
	case "backward":
		dirLabel = "Backward Trace"
	case "both":
		dirLabel = "Bidirectional Impact"
	}

	fmt.Fprintf(&b, "%s Analysis: %s (depth=%d)\n", dirLabel, r.Target, r.MaxDepth)
	b.WriteString("═══════════════════════════════════════════\n\n")

	if r.TotalCount == 0 {
		b.WriteString("No connected elements found.\n")
		return b.String()
	}

	fmt.Fprintf(&b, "%d connected element(s):\n\n", r.TotalCount)

	for _, n := range r.Nodes {
		indent := strings.Repeat("  ", n.Distance)
		title := n.Title
		if title == "" {
			title = "(untitled)"
		}
		fmt.Fprintf(&b, "%s[d=%d] %s: %s\n", indent, n.Distance, n.ElementID, title)
		fmt.Fprintf(&b, "%s       via: %s\n", indent, n.Via)
	}

	return b.String()
}
