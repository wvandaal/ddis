package discovery

// ddis:implements APP-ADR-016 (auto-prompting over manual prompting)
// ddis:implements APP-ADR-041 (challenge-feedback loop — task derivation)
// ddis:maintains APP-INV-052 (challenge-driven task derivation)

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"sort"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// DeriveTasks applies the 8 derivation rules to an artifact map.
// It generates tasks mechanically — no inference, no skipping.
func DeriveTasks(state *DiscoveryState, phases [][]string) (*TasksResult, error) {
	result := &TasksResult{
		ByRule: make(map[int]int),
	}

	taskCounter := 0

	// Assign phase membership for dependency computation.
	phaseOf := make(map[string]int) // artifact_id -> phase index
	for i, phase := range phases {
		for _, artID := range phase {
			phaseOf[artID] = i
		}
	}

	// Process each artifact in the map (sorted for determinism).
	for _, entry := range sortedArtifacts(state.ArtifactMap) {
		phase := ""
		if pi, ok := phaseOf[entry.ArtifactID]; ok {
			phase = fmt.Sprintf("phase-%d", pi)
		}

		// RULE 7: Deletion generates 3 tasks.
		if entry.Status == "deleted" {
			taskCounter++
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-remove-impl", entry.ArtifactID),
				Title:              fmt.Sprintf("Remove implementation of %s", entry.ArtifactID),
				Type:               "task",
				Priority:           2,
				Labels:             []string{"removal"},
				AcceptanceCriteria: fmt.Sprintf("No implementation code references %s", entry.ArtifactID),
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 7,
					Phase:          phase,
				},
			})
			result.ByRule[7]++

			taskCounter++
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-remove-test", entry.ArtifactID),
				Title:              fmt.Sprintf("Remove tests for %s", entry.ArtifactID),
				Type:               "task",
				Priority:           2,
				Labels:             []string{"removal"},
				AcceptanceCriteria: fmt.Sprintf("No test code references %s", entry.ArtifactID),
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 7,
					Phase:          phase,
				},
			})
			result.ByRule[7]++

			taskCounter++
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-orphan-check", entry.ArtifactID),
				Title:              fmt.Sprintf("Verify no orphan references to %s", entry.ArtifactID),
				Type:               "task",
				Priority:           2,
				Labels:             []string{"removal"},
				AcceptanceCriteria: fmt.Sprintf("Zero cross-references to %s remain in spec and impl", entry.ArtifactID),
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 7,
					Phase:          phase,
				},
			})
			result.ByRule[7]++
			continue
		}

		labels := func(extra ...string) []string {
			ls := make([]string, 0, len(extra)+1)
			ls = append(ls, extra...)
			if entry.Domain != "" {
				ls = append(ls, entry.Domain)
			}
			return ls
		}

		acceptance := entry.Tests
		if acceptance == "" {
			acceptance = entry.ValidationMethod
		}

		switch entry.ArtifactType {
		case "adr":
			// RULE 1: 1 implementation task.
			taskCounter++
			acc := acceptance
			if acc == "" {
				acc = "ADR consequences realized"
			}
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-impl", entry.ArtifactID),
				Title:              fmt.Sprintf("Implement %s", entry.Title),
				Type:               "task",
				Priority:           2,
				Labels:             labels("implementation"),
				AcceptanceCriteria: acc,
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 1,
					Phase:          phase,
				},
			})
			result.ByRule[1]++

		case "invariant":
			// RULE 2: 2 tasks — implementation + property test.
			taskCounter++
			accImpl := acceptance
			if accImpl == "" {
				accImpl = "Validation method passes"
			}
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-impl", entry.ArtifactID),
				Title:              fmt.Sprintf("Implement constraint: %s", entry.Title),
				Type:               "task",
				Priority:           1,
				Labels:             labels("constraint"),
				AcceptanceCriteria: accImpl,
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 2,
					Phase:          phase,
				},
			})
			result.ByRule[2]++

			taskCounter++
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-test", entry.ArtifactID),
				Title:              fmt.Sprintf("Property test: %s", entry.Title),
				Type:               "test",
				Priority:           1,
				Labels:             labels("constraint"),
				AcceptanceCriteria: "Test triggers violation scenario",
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 2,
					Phase:          phase,
				},
			})
			result.ByRule[2]++

		case "negative_spec":
			// RULE 3: 2 tasks — guard + regression test.
			text := entry.Text
			if text == "" {
				text = entry.Title
			}
			taskCounter++
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-guard", entry.ArtifactID),
				Title:              fmt.Sprintf("Guard: %s", text),
				Type:               "task",
				Priority:           2,
				Labels:             labels("guard"),
				AcceptanceCriteria: fmt.Sprintf("Guard prevents %s", text),
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 3,
					Phase:          phase,
				},
			})
			result.ByRule[3]++

			taskCounter++
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-test", entry.ArtifactID),
				Title:              fmt.Sprintf("Regression test: %s", text),
				Type:               "test",
				Priority:           2,
				Labels:             labels("guard"),
				AcceptanceCriteria: fmt.Sprintf("Test verifies guard against %s", text),
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 3,
					Phase:          phase,
				},
			})
			result.ByRule[3]++

		case "glossary":
			// RULE 4: 1 documentation task.
			taskCounter++
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-glossary", entry.ArtifactID),
				Title:              fmt.Sprintf("Add to glossary: %s", entry.Title),
				Type:               "task",
				Priority:           3,
				Labels:             []string{"documentation"},
				AcceptanceCriteria: fmt.Sprintf("Glossary entry for %s exists with definition and examples", entry.Title),
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 4,
					Phase:          phase,
				},
			})
			result.ByRule[4]++

		case "gate":
			// RULE 5: 1 gate integration task.
			taskCounter++
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-gate", entry.ArtifactID),
				Title:              fmt.Sprintf("Gate integration: %s", entry.Title),
				Type:               "task",
				Priority:           2,
				Labels:             []string{"gate"},
				AcceptanceCriteria: fmt.Sprintf("Gate %s integrated into validation pipeline", entry.Title),
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 5,
					Phase:          phase,
				},
			})
			result.ByRule[5]++

		case "cross_ref":
			// RULE 8: 1 cross-spec verification task.
			source := getString(entry.Data, "source")
			target := getString(entry.Data, "target")
			title := entry.Title
			if title == "" && source != "" && target != "" {
				title = fmt.Sprintf("%s -> %s", source, target)
			}
			taskCounter++
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-xref", entry.ArtifactID),
				Title:              fmt.Sprintf("Verify cross-spec contract: %s", title),
				Type:               "task",
				Priority:           2,
				Labels:             []string{"cross-spec"},
				AcceptanceCriteria: fmt.Sprintf("Cross-spec contract %s verified bidirectionally", title),
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 8,
					Phase:          phase,
				},
			})
			result.ByRule[8]++
		}

		// RULE 6: Amendments generate 2 tasks each.
		for i, amendment := range entry.Amendments {
			change := getString(amendment, "change")
			if change == "" {
				change = fmt.Sprintf("amendment %d", i+1)
			}

			taskCounter++
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-amend%d-impl", entry.ArtifactID, i+1),
				Title:              fmt.Sprintf("Update implementation of %s: %s", entry.ArtifactID, change),
				Type:               "task",
				Priority:           1,
				Labels:             labels("amendment"),
				AcceptanceCriteria: fmt.Sprintf("Implementation reflects amendment: %s", change),
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 6,
					Phase:          phase,
				},
			})
			result.ByRule[6]++

			taskCounter++
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:                 fmt.Sprintf("TASK-%s-amend%d-test", entry.ArtifactID, i+1),
				Title:              fmt.Sprintf("Update tests for %s: %s", entry.ArtifactID, change),
				Type:               "test",
				Priority:           1,
				Labels:             labels("amendment"),
				AcceptanceCriteria: fmt.Sprintf("Tests verify amendment: %s", change),
				Metadata: TaskMetadata{
					SourceArtifact: entry.ArtifactID,
					DerivationRule: 6,
					Phase:          phase,
				},
			})
			result.ByRule[6]++
		}
	}

	// Apply phase-based dependencies.
	if len(phases) > 1 {
		applyPhaseDependencies(result, state, phases)
	}

	result.TotalTasks = len(result.Tasks)
	return result, nil
}

// applyPhaseDependencies sets inter-phase and intra-phase dependency edges.
func applyPhaseDependencies(result *TasksResult, state *DiscoveryState, phases [][]string) {
	// Build a lookup: task ID -> index in result.Tasks.
	taskIdx := make(map[string]int)
	for i, t := range result.Tasks {
		taskIdx[t.ID] = i
	}

	// Build phase membership: artifact_id -> phase index.
	phaseOf := make(map[string]int)
	for i, phase := range phases {
		for _, artID := range phase {
			phaseOf[artID] = i
		}
	}

	// Collect task IDs per phase.
	phaseTasks := make(map[int][]string) // phase index -> task IDs
	for _, t := range result.Tasks {
		if pi, ok := phaseOf[t.Metadata.SourceArtifact]; ok {
			phaseTasks[pi] = append(phaseTasks[pi], t.ID)
		}
	}

	// Inter-phase: tasks in phase N+1 depend on all tasks from phase N.
	for pi := 1; pi < len(phases); pi++ {
		prevTasks := phaseTasks[pi-1]
		for _, tid := range phaseTasks[pi] {
			if idx, ok := taskIdx[tid]; ok {
				result.Tasks[idx].DependsOn = append(result.Tasks[idx].DependsOn, prevTasks...)
			}
		}
	}

	// Intra-phase: invariant test tasks depend on their ADR's implementation task.
	// Convention: if an invariant and ADR share the same domain within a phase,
	// the invariant impl task depends on the ADR impl task.
	for _, phase := range phases {
		adrImplIDs := make(map[string]string) // domain -> ADR impl task ID
		for _, artID := range phase {
			entry, ok := state.ArtifactMap[artID]
			if !ok || entry.ArtifactType != "adr" {
				continue
			}
			implID := fmt.Sprintf("TASK-%s-impl", artID)
			if _, exists := taskIdx[implID]; exists && entry.Domain != "" {
				adrImplIDs[entry.Domain] = implID
			}
		}
		for _, artID := range phase {
			entry, ok := state.ArtifactMap[artID]
			if !ok || entry.ArtifactType != "invariant" {
				continue
			}
			if entry.Domain == "" {
				continue
			}
			adrImpl, hasADR := adrImplIDs[entry.Domain]
			if !hasADR {
				continue
			}
			invImplID := fmt.Sprintf("TASK-%s-impl", artID)
			if idx, exists := taskIdx[invImplID]; exists {
				result.Tasks[idx].DependsOn = append(result.Tasks[idx].DependsOn, adrImpl)
			}
		}
	}
}

// CrossValidate checks artifact IDs against a spec database.
// For each task whose source_artifact is not found in the spec, it is flagged as orphaned.
func CrossValidate(result *TasksResult, db *sql.DB, specID int64) error {
	if db == nil {
		return nil
	}

	seen := make(map[string]bool)
	for _, task := range result.Tasks {
		artID := task.Metadata.SourceArtifact
		if artID == "" || seen[artID] {
			continue
		}
		seen[artID] = true

		if !resolveArtifact(db, specID, artID) {
			result.OrphanedRefs = append(result.OrphanedRefs, artID)
		}
	}
	return nil
}

// resolveArtifact checks if an artifact ID exists in any spec element table.
func resolveArtifact(db *sql.DB, specID int64, artID string) bool {
	var x int
	// Try invariant
	if err := db.QueryRow(
		`SELECT 1 FROM invariants WHERE spec_id = ? AND invariant_id = ?`,
		specID, artID).Scan(&x); err == nil {
		return true
	}
	// Try ADR
	if err := db.QueryRow(
		`SELECT 1 FROM adrs WHERE spec_id = ? AND adr_id = ?`,
		specID, artID).Scan(&x); err == nil {
		return true
	}
	// Try gate
	if err := db.QueryRow(
		`SELECT 1 FROM quality_gates WHERE spec_id = ? AND gate_id = ?`,
		specID, artID).Scan(&x); err == nil {
		return true
	}
	return false
}

// EnrichWithWitnesses decorates tasks with witness status and adjusts priority
// for stale witnesses (regression risk outranks greenfield risk).
// Best-effort: errors are silently ignored.
// ddis:maintains APP-INV-104
func EnrichWithWitnesses(result *TasksResult, db *sql.DB, specID int64) {
	if db == nil {
		return
	}
	witnesses, err := storage.ListWitnesses(db, specID)
	if err != nil {
		return
	}
	wStatus := make(map[string]string)
	for _, w := range witnesses {
		wStatus[w.InvariantID] = w.Status
	}
	for i := range result.Tasks {
		t := &result.Tasks[i]
		artID := t.Metadata.SourceArtifact
		if !strings.HasPrefix(artID, "APP-INV-") {
			continue
		}
		if status, ok := wStatus[artID]; ok {
			t.WitnessStatus = status
			// Priority boost for stale witnesses (regression risk)
			if status == "stale_spec" || status == "stale_code" {
				if t.Priority > 0 {
					t.Priority--
				}
			}
		} else {
			t.WitnessStatus = "unwitnessed"
		}
	}
}

// sortedArtifacts returns artifacts sorted by ID for deterministic output.
func sortedArtifacts(m map[string]*ArtifactEntry) []*ArtifactEntry {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	result := make([]*ArtifactEntry, 0, len(keys))
	for _, k := range keys {
		result = append(result, m[k])
	}
	return result
}

// FormatBeads outputs tasks as beads-compatible JSONL (one JSON object per line).
func FormatBeads(result *TasksResult) string {
	var sb strings.Builder
	for _, task := range result.Tasks {
		bead := map[string]interface{}{
			"title":      task.Title,
			"type":       task.Type,
			"priority":   task.Priority,
			"labels":     task.Labels,
			"acceptance": task.AcceptanceCriteria,
			"depends_on": task.DependsOn,
			"metadata":   task.Metadata,
		}
		if task.WitnessStatus != "" {
			bead["witness_status"] = task.WitnessStatus
		}
		line, err := json.Marshal(bead)
		if err != nil {
			continue
		}
		sb.Write(line)
		sb.WriteByte('\n')
	}
	return sb.String()
}

// FormatJSON outputs tasks as a pretty-printed JSON array.
func FormatJSON(result *TasksResult) (string, error) {
	data, err := json.MarshalIndent(result, "", "  ")
	if err != nil {
		return "", fmt.Errorf("marshal tasks JSON: %w", err)
	}
	return string(data), nil
}

// FormatMarkdown outputs tasks as a markdown checklist grouped by priority.
func FormatMarkdown(result *TasksResult) string {
	var sb strings.Builder
	sb.WriteString("## Tasks\n\n")
	for _, task := range result.Tasks {
		indicator := ""
		if task.WitnessStatus == "stale_spec" || task.WitnessStatus == "stale_code" {
			indicator = " [STALE]"
		} else if task.WitnessStatus == "valid" {
			indicator = " [VALID]"
		}
		sb.WriteString(fmt.Sprintf("- [ ] P%d: %s (source: %s)%s\n",
			task.Priority, task.Title, task.Metadata.SourceArtifact, indicator))
		if len(task.DependsOn) > 0 {
			sb.WriteString(fmt.Sprintf("  Depends on: %s\n", strings.Join(task.DependsOn, ", ")))
		}
		sb.WriteString(fmt.Sprintf("  Acceptance: %s\n", task.AcceptanceCriteria))
	}
	return sb.String()
}

// DeriveFromChallenges generates tasks from challenge verdicts (Rules 9-10).
// Rule 9: Provisional invariants → upgrade tasks (write test, add annotations).
// Rule 10: Refuted invariants → remediation tasks (fix impl or amend spec).
func DeriveFromChallenges(db storage.DB, specID int64) (*TasksResult, error) {
	challenges, err := storage.ListChallengeResults(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list challenges: %w", err)
	}

	result := &TasksResult{
		ByRule: make(map[int]int),
	}

	for _, cr := range challenges {
		switch cr.Verdict {
		case "refuted":
			// Rule 10: Refuted → highest priority remediation
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:       fmt.Sprintf("TASK-%s-remediate", cr.InvariantID),
				Title:    fmt.Sprintf("REMEDIATE: %s (refuted by challenge)", cr.InvariantID),
				Type:     "task",
				Priority: 0, // Higher than any discovery-derived task
				Labels:   []string{"remediation", "challenge"},
				AcceptanceCriteria: fmt.Sprintf(
					"Fix implementation to satisfy %s, or amend spec if invariant is wrong. "+
						"Re-challenge must return confirmed or provisional.", cr.InvariantID),
				Metadata: TaskMetadata{
					SourceArtifact: cr.InvariantID,
					DerivationRule: 10,
				},
			})
			result.ByRule[10]++

		case "provisional":
			// Rule 9: Provisional → upgrade task
			result.Tasks = append(result.Tasks, DerivedTask{
				ID:       fmt.Sprintf("TASK-%s-upgrade", cr.InvariantID),
				Title:    fmt.Sprintf("Upgrade evidence: %s (provisional)", cr.InvariantID),
				Type:     "task",
				Priority: 1,
				Labels:   []string{"upgrade", "challenge"},
				AcceptanceCriteria: fmt.Sprintf(
					"Write behavioral test with ddis:tests %s annotation, or add "+
						"ddis:implements annotations across 3+ packages. Re-challenge "+
						"must return confirmed.", cr.InvariantID),
				Metadata: TaskMetadata{
					SourceArtifact: cr.InvariantID,
					DerivationRule: 9,
				},
			})
			result.ByRule[9]++
		}
	}

	result.TotalTasks = len(result.Tasks)
	return result, nil
}
