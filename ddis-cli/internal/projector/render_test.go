package projector

// ddis:tests APP-INV-076 (projection purity — pure functions of structured data)
// ddis:tests APP-INV-077 (synthetic render — structured fields, not raw_text)

import (
	"strings"
	"testing"
)

func TestRenderInvariant_AllFields(t *testing.T) {
	inv := Invariant{
		ID:                "APP-INV-073",
		Title:             "Fold Determinism",
		Statement:         "Same event sequence produces identical state",
		SemiFormal:        "∀e₁…eₙ: fold(e₁…eₙ) = fold(e₁…eₙ)",
		ViolationScenario: "Two replays produce different SQL state",
		ValidationMethod:  "Replay events twice and compare SHA-256 of DB",
		WhyThisMatters:    "Without determinism, replay is meaningless",
	}

	rendered := RenderInvariant(inv)

	// Must contain structured fields, not raw_text
	if !strings.Contains(rendered, "APP-INV-073") {
		t.Error("rendered invariant missing ID")
	}
	if !strings.Contains(rendered, "Fold Determinism") {
		t.Error("rendered invariant missing title")
	}
	if !strings.Contains(rendered, "Same event sequence") {
		t.Error("rendered invariant missing statement")
	}
	if !strings.Contains(rendered, "∀e₁…eₙ") {
		t.Error("rendered invariant missing semi-formal")
	}
	if !strings.Contains(rendered, "---") {
		t.Error("rendered invariant missing separator")
	}
}

func TestRenderInvariant_MinimalFields(t *testing.T) {
	inv := Invariant{
		ID:    "INV-001",
		Title: "Minimal",
	}

	rendered := RenderInvariant(inv)
	if !strings.Contains(rendered, "INV-001") {
		t.Error("rendered invariant missing ID")
	}
	// Should not contain empty field markers
	if strings.Contains(rendered, "Violation scenario:") {
		t.Error("should not render empty violation scenario")
	}
}

func TestRenderADR_AllFields(t *testing.T) {
	adr := ADR{
		ID:           "APP-ADR-058",
		Title:        "JSONL as Canonical",
		Problem:      "Multiple sources of truth",
		Options:      "Option A: SQL. Option B: JSONL.",
		Decision:     "JSONL as canonical representation",
		Consequences: "SQL becomes derived, can be deleted",
		Tests:        "Delete DB, replay, compare",
	}

	rendered := RenderADR(adr)

	if !strings.Contains(rendered, "APP-ADR-058") {
		t.Error("rendered ADR missing ID")
	}
	if !strings.Contains(rendered, "#### Problem") {
		t.Error("rendered ADR missing Problem heading")
	}
	if !strings.Contains(rendered, "#### Decision") {
		t.Error("rendered ADR missing Decision heading")
	}
	if !strings.Contains(rendered, "---") {
		t.Error("rendered ADR missing separator")
	}
}

func TestRenderModule_Structure(t *testing.T) {
	mod := ModuleSpec{
		Name:       "event-sourcing",
		Domain:     "eventsourcing",
		Maintains:  []string{"APP-INV-071", "APP-INV-072"},
		Interfaces: []string{"APP-INV-001"},
		Invariants: []Invariant{
			{ID: "APP-INV-071", Title: "Log Canonicality"},
		},
		ADRs: []ADR{
			{ID: "APP-ADR-058", Title: "JSONL as Canonical"},
		},
		NegSpecs: []string{"Must NOT read markdown as canonical source"},
	}

	rendered := RenderModule(mod)

	// Frontmatter
	if !strings.Contains(rendered, "module: event-sourcing") {
		t.Error("missing module frontmatter")
	}
	if !strings.Contains(rendered, "domain: eventsourcing") {
		t.Error("missing domain frontmatter")
	}
	if !strings.Contains(rendered, "maintains:") {
		t.Error("missing maintains frontmatter")
	}

	// Content sections
	if !strings.Contains(rendered, "## Invariants") {
		t.Error("missing Invariants section")
	}
	if !strings.Contains(rendered, "## Architecture Decision Records") {
		t.Error("missing ADR section")
	}
	if !strings.Contains(rendered, "## Negative Specifications") {
		t.Error("missing Negative Specifications section")
	}
	if !strings.Contains(rendered, "**DO NOT**") {
		t.Error("missing DO NOT pattern")
	}
}

func TestRenderModule_EmptyModule(t *testing.T) {
	mod := ModuleSpec{
		Name:   "empty",
		Domain: "test",
	}

	rendered := RenderModule(mod)
	if !strings.Contains(rendered, "module: empty") {
		t.Error("empty module should still have frontmatter")
	}
	// Should not have empty sections
	if strings.Contains(rendered, "## Invariants") {
		t.Error("empty module should not have Invariants section")
	}
}

func TestRenderInvariant_Purity(t *testing.T) {
	// APP-INV-076: rendering is a pure function — same input → same output
	inv := Invariant{
		ID:        "INV-001",
		Title:     "Test",
		Statement: "Pure function test",
	}

	r1 := RenderInvariant(inv)
	r2 := RenderInvariant(inv)

	if r1 != r2 {
		t.Error("purity violation: same input produced different output")
	}
}
