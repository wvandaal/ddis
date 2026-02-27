package tests

import (
	"encoding/json"
	"testing"

	"github.com/wvandaal/ddis/internal/impact"
)

func TestImpactForward(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	result, err := impact.Analyze(db, specID, "INV-001", impact.ImpactOptions{
		Direction: "forward",
		MaxDepth:  2,
	})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	if result.Target != "INV-001" {
		t.Errorf("target = %s, want INV-001", result.Target)
	}
	if result.Direction != "forward" {
		t.Errorf("direction = %s, want forward", result.Direction)
	}

	t.Logf("INV-001 forward impact: %d connected nodes", result.TotalCount)
	for _, n := range result.Nodes {
		t.Logf("  [d=%d] %s: %s (via: %s)", n.Distance, n.ElementID, n.Title, n.Via)
	}
}

func TestImpactBackward(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	result, err := impact.Analyze(db, specID, "§1.1", impact.ImpactOptions{
		Direction: "backward",
		MaxDepth:  2,
	})
	if err != nil {
		t.Fatalf("analyze backward: %v", err)
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
	db, specID := buildSyntheticDB(t)

	result1, err := impact.Analyze(db, specID, "INV-001", impact.ImpactOptions{
		Direction: "forward",
		MaxDepth:  1,
	})
	if err != nil {
		t.Fatalf("analyze depth 1: %v", err)
	}

	result3, err := impact.Analyze(db, specID, "INV-001", impact.ImpactOptions{
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
	db, specID := buildSyntheticDB(t)

	result, err := impact.Analyze(db, specID, "INV-001", impact.ImpactOptions{
		Direction: "both",
		MaxDepth:  2,
	})
	if err != nil {
		t.Fatalf("analyze both: %v", err)
	}

	if result.Direction != "both" {
		t.Errorf("direction = %s, want both", result.Direction)
	}

	t.Logf("Bidirectional impact for INV-001: %d total nodes", result.TotalCount)
}

func TestImpactBadTarget(t *testing.T) {
	db, specID := buildSyntheticDB(t)

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
	db, specID := buildSyntheticDB(t)

	result, err := impact.Analyze(db, specID, "INV-001", impact.ImpactOptions{
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

	if parsed.Target != "INV-001" {
		t.Errorf("JSON target = %s, want INV-001", parsed.Target)
	}
	if parsed.TotalCount != len(parsed.Nodes) {
		t.Errorf("total_count (%d) != len(nodes) (%d)", parsed.TotalCount, len(parsed.Nodes))
	}
}
