package tests

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/impact"
	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/storage"
)

// sharedImpactDB caches a parsed monolith DB for impact tests.
var sharedImpactDB *impactTestDB

type impactTestDB struct {
	db     *storage.DB
	specID int64
}

func getImpactDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedImpactDB != nil {
		return sharedImpactDB.db, sharedImpactDB.specID
	}

	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Skipf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "impact_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	sharedImpactDB = &impactTestDB{db: &db, specID: specID}
	return sharedImpactDB.db, sharedImpactDB.specID
}

func TestImpactForward(t *testing.T) {
	dbPtr, specID := getImpactDB(t)
	db := *dbPtr

	result, err := impact.Analyze(db, specID, "INV-006", impact.ImpactOptions{
		Direction: "forward",
		MaxDepth:  2,
	})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	if result.Target != "INV-006" {
		t.Errorf("target = %s, want INV-006", result.Target)
	}
	if result.Direction != "forward" {
		t.Errorf("direction = %s, want forward", result.Direction)
	}

	// INV-006 (Cross-Reference Density) should be referenced by multiple sections
	t.Logf("INV-006 forward impact: %d connected nodes", result.TotalCount)
	for _, n := range result.Nodes {
		t.Logf("  [d=%d] %s: %s (via: %s)", n.Distance, n.ElementID, n.Title, n.Via)
	}
}

func TestImpactBackward(t *testing.T) {
	dbPtr, specID := getImpactDB(t)
	db := *dbPtr

	result, err := impact.Analyze(db, specID, "§4.2", impact.ImpactOptions{
		Direction: "backward",
		MaxDepth:  2,
	})
	if err != nil {
		// §4.2 might not exist in this spec; try a different section
		result, err = impact.Analyze(db, specID, "§0.5", impact.ImpactOptions{
			Direction: "backward",
			MaxDepth:  2,
		})
		if err != nil {
			t.Fatalf("analyze backward: %v", err)
		}
	}

	if result.Direction != "backward" {
		t.Errorf("direction = %s, want backward", result.Direction)
	}

	t.Logf("Backward trace from %s: %d nodes", result.Target, result.TotalCount)
	for _, n := range result.Nodes {
		t.Logf("  [d=%d] %s: %s", n.Distance, n.ElementID, n.Title)
	}
}

func TestImpactDepthLimit(t *testing.T) {
	dbPtr, specID := getImpactDB(t)
	db := *dbPtr

	result1, err := impact.Analyze(db, specID, "INV-006", impact.ImpactOptions{
		Direction: "forward",
		MaxDepth:  1,
	})
	if err != nil {
		t.Fatalf("analyze depth 1: %v", err)
	}

	result3, err := impact.Analyze(db, specID, "INV-006", impact.ImpactOptions{
		Direction: "forward",
		MaxDepth:  3,
	})
	if err != nil {
		t.Fatalf("analyze depth 3: %v", err)
	}

	// Deeper traversal should find at least as many nodes
	if result3.TotalCount < result1.TotalCount {
		t.Errorf("depth 3 (%d nodes) < depth 1 (%d nodes)", result3.TotalCount, result1.TotalCount)
	}

	t.Logf("Depth 1: %d nodes, Depth 3: %d nodes", result1.TotalCount, result3.TotalCount)
}

func TestImpactBothDirections(t *testing.T) {
	dbPtr, specID := getImpactDB(t)
	db := *dbPtr

	result, err := impact.Analyze(db, specID, "INV-006", impact.ImpactOptions{
		Direction: "both",
		MaxDepth:  2,
	})
	if err != nil {
		t.Fatalf("analyze both: %v", err)
	}

	if result.Direction != "both" {
		t.Errorf("direction = %s, want both", result.Direction)
	}

	t.Logf("Bidirectional impact for INV-006: %d total nodes", result.TotalCount)
}

func TestImpactBadTarget(t *testing.T) {
	dbPtr, specID := getImpactDB(t)
	db := *dbPtr

	_, err := impact.Analyze(db, specID, "INV-999", impact.ImpactOptions{})
	if err == nil {
		t.Error("expected error for nonexistent target, got nil")
	}

	_, err = impact.Analyze(db, specID, "not-a-valid-target", impact.ImpactOptions{})
	if err == nil {
		t.Error("expected error for invalid target format, got nil")
	}
}

func TestImpactJSON(t *testing.T) {
	dbPtr, specID := getImpactDB(t)
	db := *dbPtr

	result, err := impact.Analyze(db, specID, "INV-006", impact.ImpactOptions{
		Direction: "forward",
		MaxDepth:  2,
	})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	out, err := impact.RenderImpact(result, true)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	var parsed impact.ImpactResult
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("parse JSON: %v", err)
	}

	if parsed.Target != "INV-006" {
		t.Errorf("JSON target = %s, want INV-006", parsed.Target)
	}
	if parsed.TotalCount != len(parsed.Nodes) {
		t.Errorf("total_count (%d) != len(nodes) (%d)", parsed.TotalCount, len(parsed.Nodes))
	}
}
