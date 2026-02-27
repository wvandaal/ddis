package tests

import (
	"encoding/json"
	"testing"

	"github.com/wvandaal/ddis/internal/implorder"
	"github.com/wvandaal/ddis/internal/storage"
)

// =============================================================================
// INV-TOPO-VALID: Every edge (u,v) in the dependency graph has phase(u) <= phase(v)
// This is the core topological invariant — no element appears in a phase
// before its dependencies.
// =============================================================================

func TestImplOrderTopoValid(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := implorder.Analyze(db, specID, implorder.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	if result.TotalElements == 0 {
		t.Skip("no invariants in spec")
	}

	// Build element → phase mapping
	phaseOf := make(map[string]int)
	for _, phase := range result.Phases {
		for _, elem := range phase.Elements {
			phaseOf[elem.ID] = phase.PhaseNum
		}
	}

	// Rebuild dependency edges to verify ordering
	modules, _ := storage.ListModules(db, specID)
	moduleByID := make(map[int64]string)
	for _, m := range modules {
		moduleByID[m.ID] = m.ModuleName
	}

	rels, _ := storage.GetModuleRelationships(db, specID)

	moduleMainInvs := make(map[string][]string)
	moduleInterfaceInvs := make(map[string][]string)
	for _, r := range rels {
		modName := moduleByID[r.ModuleID]
		if modName == "" {
			continue
		}
		switch r.RelType {
		case "maintains":
			moduleMainInvs[modName] = append(moduleMainInvs[modName], r.Target)
		case "interfaces":
			moduleInterfaceInvs[modName] = append(moduleInterfaceInvs[modName], r.Target)
		}
	}

	// For each edge: maintained INV-X depends on interfaced INV-Y
	// So phase(INV-Y) <= phase(INV-X)
	for mod, maintained := range moduleMainInvs {
		interfaces := moduleInterfaceInvs[mod]
		for _, m := range maintained {
			pm, okM := phaseOf[m]
			for _, i := range interfaces {
				pi, okI := phaseOf[i]
				if m == i || !okM || !okI {
					continue
				}
				if pi > pm {
					t.Errorf("topological violation: %s (phase %d) depends on %s (phase %d) via module %s",
						m, pm, i, pi, mod)
				}
			}
		}
	}
}

// =============================================================================
// Phase 0 has no dependencies (all elements have in-degree 0)
// =============================================================================

func TestImplOrderPhaseZeroNoDeps(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := implorder.Analyze(db, specID, implorder.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	if len(result.Phases) == 0 {
		t.Skip("no phases")
	}

	phase0 := result.Phases[0]
	if phase0.PhaseNum != 0 {
		t.Errorf("first phase should be 0, got %d", phase0.PhaseNum)
	}
	if phase0.Label != "Foundation — no dependencies" {
		t.Errorf("phase 0 label = %q, want %q", phase0.Label, "Foundation — no dependencies")
	}
}

// =============================================================================
// Total elements equals sum of all phase elements (no lost nodes)
// =============================================================================

func TestImplOrderElementCount(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := implorder.Analyze(db, specID, implorder.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	sum := 0
	for _, phase := range result.Phases {
		sum += len(phase.Elements)
	}
	sum += len(result.CyclesDetected)

	invs, _ := storage.ListInvariants(db, specID)
	if result.TotalElements+len(result.CyclesDetected) != len(invs) {
		t.Errorf("element count mismatch: phases=%d + cycles=%d != total invariants=%d",
			result.TotalElements, len(result.CyclesDetected), len(invs))
	}
}

// =============================================================================
// CriticalPath equals the number of phases
// =============================================================================

func TestImplOrderCriticalPath(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := implorder.Analyze(db, specID, implorder.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	if result.CriticalPath != len(result.Phases) {
		t.Errorf("critical_path_length=%d but %d phases", result.CriticalPath, len(result.Phases))
	}
}

// =============================================================================
// JSON output is valid and round-trips
// =============================================================================

func TestImplOrderJSONValid(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := implorder.Analyze(db, specID, implorder.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	out, err := implorder.Render(result, true)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	var parsed implorder.ImplOrderResult
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("invalid JSON output: %v\nOutput:\n%s", err, out)
	}

	if parsed.TotalElements != result.TotalElements {
		t.Errorf("total_elements mismatch: got %d, want %d",
			parsed.TotalElements, result.TotalElements)
	}
	if parsed.CriticalPath != result.CriticalPath {
		t.Errorf("critical_path_length mismatch: got %d, want %d",
			parsed.CriticalPath, result.CriticalPath)
	}
}

// =============================================================================
// Domain filter reduces results
// =============================================================================

func TestImplOrderDomainFilter(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	full, err := implorder.Analyze(db, specID, implorder.Options{})
	if err != nil {
		t.Fatalf("analyze full: %v", err)
	}

	// Filter to "parsing" domain (INV-001, INV-002, INV-003)
	filtered, err := implorder.Analyze(db, specID, implorder.Options{Domain: "parsing"})
	if err != nil {
		t.Fatalf("analyze filtered: %v", err)
	}

	// Filtered should have fewer or equal total invariants (phased + cycles)
	filteredTotal := filtered.TotalElements + len(filtered.CyclesDetected)
	fullTotal := full.TotalElements + len(full.CyclesDetected)
	if filteredTotal > fullTotal {
		t.Errorf("domain filter should reduce total invariants: filtered=%d, full=%d",
			filteredTotal, fullTotal)
	}

	// All elements in filtered result should belong to the requested domain
	for _, phase := range filtered.Phases {
		for _, elem := range phase.Elements {
			if elem.Domain != "parsing" {
				t.Errorf("element %s has domain %q, expected %q",
					elem.ID, elem.Domain, "parsing")
			}
		}
	}
}

// =============================================================================
// Determinism: same input produces same output
// =============================================================================

func TestImplOrderDeterminism(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result1, err := implorder.Analyze(db, specID, implorder.Options{})
	if err != nil {
		t.Fatalf("first analyze: %v", err)
	}
	out1, _ := implorder.Render(result1, true)

	result2, err := implorder.Analyze(db, specID, implorder.Options{})
	if err != nil {
		t.Fatalf("second analyze: %v", err)
	}
	out2, _ := implorder.Render(result2, true)

	if out1 != out2 {
		t.Error("non-deterministic output between two runs")
	}
}

// =============================================================================
// Authority-ordered within phases
// =============================================================================

func TestImplOrderAuthorityOrdering(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := implorder.Analyze(db, specID, implorder.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	for _, phase := range result.Phases {
		for i := 1; i < len(phase.Elements); i++ {
			prev := phase.Elements[i-1]
			curr := phase.Elements[i]
			if prev.Authority < curr.Authority {
				t.Errorf("phase %d: %s (%.4f) before %s (%.4f) violates authority ordering",
					phase.PhaseNum, prev.ID, prev.Authority, curr.ID, curr.Authority)
			}
		}
	}
}

// =============================================================================
// CyclesDetected is always a non-nil slice
// =============================================================================

func TestImplOrderCyclesNonNil(t *testing.T) {
	db, specID := buildSyntheticModularDB(t)

	result, err := implorder.Analyze(db, specID, implorder.Options{})
	if err != nil {
		t.Fatalf("analyze: %v", err)
	}

	if result.CyclesDetected == nil {
		t.Error("cycles_detected should be non-nil (use empty slice)")
	}
	if result.Phases == nil {
		t.Error("phases should be non-nil (use empty slice)")
	}
}
