package discover

// ddis:maintains APP-INV-028 (spec-as-trunk)
// ddis:implements APP-INV-023 (prompt self-containment — BuildContext assembles threads, mode, confidence, drift into single CommandResult)
// ddis:maintains APP-INV-035 (guidance attenuation — applies KStarEff and Attenuation to scale guidance by depth)
// ddis:implements APP-INV-026 (classification non-prescriptive — ClassifyMode returns ObservedMode, guidance is SuggestedNext not mandatory)
// ddis:implements APP-INV-030 (contributor topology graceful degradation — BuildContext handles nil DB and empty eventsDir without error)
// ddis:implements APP-INV-036 (human format transparency — BuildContext output uses readable markdown, no internal format leakage)

import (
	"database/sql"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/autoprompt"
	"github.com/wvandaal/ddis/internal/storage"
)

// BuildContext assembles the discovery context bundle.
// This is the opening prompt for the LLM interpreter.
func BuildContext(db *sql.DB, specID int64, opts DiscoverOptions) (*autoprompt.CommandResult, error) {
	// 1. Load threads from events directory.
	threads, err := LoadThreads(opts.EventsDir)
	if err != nil {
		return nil, fmt.Errorf("load threads: %w", err)
	}

	// 2. Converge on thread (auto or override).
	match := ConvergeThread(opts.Content, threads, opts.ThreadID)

	// If new thread, create and save it.
	if match.Method == "new_thread" {
		summary := opts.Content
		if len(summary) > 120 {
			summary = summary[:120] + "..."
		}
		newThread := Thread{
			ID:        match.ThreadID,
			Status:    "active",
			Summary:   summary,
			CreatedAt: nowRFC3339(),
		}
		if err := SaveThread(opts.EventsDir, newThread); err != nil {
			return nil, fmt.Errorf("save new thread: %w", err)
		}
		threads = append(threads, newThread)
	}

	// 3. Load recent events for the selected thread.
	events, err := LoadEvents(opts.EventsDir, match.ThreadID)
	if err != nil {
		return nil, fmt.Errorf("load events: %w", err)
	}

	// 4. Classify cognitive mode.
	mode := ClassifyMode(events)

	// 5. Count open questions for the thread.
	openQuestions := countOpenQuestions(events)

	// 6. Get spec drift score from cross_references table.
	specDrift := computeDriftProxy(db, specID)

	// 7. Build confidence array using basic heuristics.
	conf := buildConfidence(db, specID)

	// 8. Build output string.
	var out strings.Builder
	out.WriteString("## Discovery Context\n\n")
	out.WriteString(fmt.Sprintf("**Thread**: %s (matched via %s, score: %.2f)\n", match.ThreadID, match.Method, match.Score))
	out.WriteString(fmt.Sprintf("**Mode**: %s (confidence: %.2f, DoF: %s)\n", mode.Mode, mode.Confidence, mode.DoFHint))
	out.WriteString(fmt.Sprintf("**Open questions**: %d\n", openQuestions))
	out.WriteString(fmt.Sprintf("**Events in thread**: %d\n", len(events)))
	out.WriteString(fmt.Sprintf("**Spec drift**: %.2f\n", specDrift))
	out.WriteString(fmt.Sprintf("**Confidence**: [coverage=%d, depth=%d, coherence=%d, completeness=%d, formality=%d]\n",
		conf[0], conf[1], conf[2], conf[3], conf[4]))

	// Thread topology summary.
	active, parked, merged := 0, 0, 0
	for _, t := range threads {
		switch t.Status {
		case "active":
			active++
		case "parked":
			parked++
		case "merged":
			merged++
		}
	}
	out.WriteString(fmt.Sprintf("\n**Thread topology**: %d active, %d parked, %d merged\n", active, parked, merged))

	// List active threads.
	if active > 0 {
		out.WriteString("\n### Active Threads\n")
		for _, t := range threads {
			if t.Status == "active" {
				marker := ""
				if t.ID == match.ThreadID {
					marker = " [current]"
				}
				out.WriteString(fmt.Sprintf("- **%s**: %s (events: %d)%s\n", t.ID, t.Summary, t.EventCount, marker))
			}
		}
	}

	// 9. Build guidance with mode-appropriate suggestions.
	suggestions := modeGuidance(mode, conf)

	kEff := autoprompt.KStarEff(opts.Depth)
	attenuation := autoprompt.Attenuation(opts.Depth)

	guidance := autoprompt.Guidance{
		ObservedMode:  mode.Mode,
		DoFHint:       mode.DoFHint,
		SuggestedNext: suggestions,
		Attenuation:   attenuation,
	}

	// Add relevant context hints.
	if specDrift > 0 {
		guidance.RelevantContext = append(guidance.RelevantContext,
			fmt.Sprintf("Spec drift score: %.2f — consider running `ddis drift` before crystallizing", specDrift))
	}
	if openQuestions > 3 {
		guidance.RelevantContext = append(guidance.RelevantContext,
			fmt.Sprintf("%d open questions — consider narrowing focus", openQuestions))
	}

	limitingFactor := findLimitingFactor(conf)

	// 10. Return CommandResult.
	return &autoprompt.CommandResult{
		Output: out.String(),
		State: autoprompt.StateSnapshot{
			ActiveThread:   match.ThreadID,
			Confidence:     conf,
			LimitingFactor: limitingFactor,
			OpenQuestions:   openQuestions,
			SpecDrift:      specDrift,
			Iteration:      kEff,
			ModeObserved:   mode.Mode,
		},
		Guidance: guidance,
	}, nil
}

// Status returns a summary of the current discovery state.
func Status(db *sql.DB, specID int64, eventsDir string) (*autoprompt.CommandResult, error) {
	threads, err := LoadThreads(eventsDir)
	if err != nil {
		return nil, fmt.Errorf("load threads: %w", err)
	}

	events, err := LoadEvents(eventsDir, "")
	if err != nil {
		return nil, fmt.Errorf("load events: %w", err)
	}

	active, parked, merged := 0, 0, 0
	for _, t := range threads {
		switch t.Status {
		case "active":
			active++
		case "parked":
			parked++
		case "merged":
			merged++
		}
	}

	var out strings.Builder
	out.WriteString("## Discovery Status\n\n")
	out.WriteString(fmt.Sprintf("**Threads**: %d total (%d active, %d parked, %d merged)\n",
		len(threads), active, parked, merged))
	out.WriteString(fmt.Sprintf("**Total events**: %d\n\n", len(events)))

	if len(threads) > 0 {
		out.WriteString("### Thread Topology\n")
		for _, t := range threads {
			out.WriteString(fmt.Sprintf("- [%s] **%s**: %s (events: %d, created: %s)\n",
				t.Status, t.ID, t.Summary, t.EventCount, t.CreatedAt))
		}
	}

	return &autoprompt.CommandResult{
		Output: out.String(),
		State: autoprompt.StateSnapshot{
			OpenQuestions: countOpenQuestions(events),
		},
		Guidance: autoprompt.Guidance{
			DoFHint:       "mid",
			SuggestedNext: []string{"Select a thread to continue discovery", "Create a new thread with fresh content"},
		},
	}, nil
}

// ListThreads returns information about all threads.
func ListThreads(eventsDir string) (*autoprompt.CommandResult, error) {
	threads, err := LoadThreads(eventsDir)
	if err != nil {
		return nil, fmt.Errorf("load threads: %w", err)
	}

	var out strings.Builder
	out.WriteString("## Discovery Threads\n\n")
	if len(threads) == 0 {
		out.WriteString("No threads found. Start discovery with `ddis discover`.\n")
	} else {
		for _, t := range threads {
			out.WriteString(fmt.Sprintf("### %s [%s]\n", t.ID, t.Status))
			out.WriteString(fmt.Sprintf("- Summary: %s\n", t.Summary))
			out.WriteString(fmt.Sprintf("- Events: %d\n", t.EventCount))
			out.WriteString(fmt.Sprintf("- Created: %s\n", t.CreatedAt))
			if t.LastEventAt != "" {
				out.WriteString(fmt.Sprintf("- Last event: %s\n", t.LastEventAt))
			}
			if len(t.SpecAttachment) > 0 {
				out.WriteString(fmt.Sprintf("- Attached to: %s\n", strings.Join(t.SpecAttachment, ", ")))
			}
			out.WriteString("\n")
		}
	}

	return &autoprompt.CommandResult{
		Output: out.String(),
		State:  autoprompt.StateSnapshot{},
		Guidance: autoprompt.Guidance{
			DoFHint:       "mid",
			SuggestedNext: []string{"Continue an active thread", "Park an inactive thread", "Merge related threads"},
		},
	}, nil
}

// countOpenQuestions counts question_opened events minus question_closed events for a thread.
func countOpenQuestions(events []Event) int {
	opened := 0
	closed := 0
	for _, e := range events {
		switch e.Type {
		case "question_opened":
			opened++
		case "question_closed":
			closed++
		}
	}
	result := opened - closed
	if result < 0 {
		return 0
	}
	return result
}

// computeDriftProxy computes a lightweight drift proxy from cross-references.
// drift = unresolved_refs / total_refs (0 if no refs).
func computeDriftProxy(db *sql.DB, specID int64) float64 {
	if db == nil {
		return 0.0
	}
	counts, err := storage.CountElements(db, specID)
	if err != nil {
		return 0.0
	}
	total := counts["cross_references"]
	resolved := counts["cross_references_resolved"]
	if total == 0 {
		return 0.0
	}
	unresolved := total - resolved
	return float64(unresolved) / float64(total)
}

// buildConfidence builds the 5-element confidence array from spec data.
func buildConfidence(db *sql.DB, specID int64) [5]int {
	var conf [5]int
	if db == nil {
		return conf
	}

	counts, err := storage.CountElements(db, specID)
	if err != nil {
		return conf
	}

	// Coverage: based on presence of key element types (sections, invariants, ADRs, gates).
	coverageScore := 0
	if counts["sections"] > 0 {
		coverageScore += 3
	}
	if counts["invariants"] > 0 {
		coverageScore += 3
	}
	if counts["adrs"] > 0 {
		coverageScore += 2
	}
	if counts["quality_gates"] > 0 {
		coverageScore += 2
	}
	conf[autoprompt.ConfCoverage] = coverageScore

	// Depth: based on invariant component completeness.
	invs, err := storage.ListInvariants(db, specID)
	if err == nil && len(invs) > 0 {
		totalComponents := 0
		for _, inv := range invs {
			if inv.Statement != "" {
				totalComponents++
			}
			if inv.SemiFormal != "" {
				totalComponents++
			}
			if inv.ViolationScenario != "" {
				totalComponents++
			}
			if inv.ValidationMethod != "" {
				totalComponents++
			}
			if inv.WhyThisMatters != "" {
				totalComponents++
			}
		}
		// 5 possible components per invariant, scale to 0-10.
		maxComponents := len(invs) * 5
		conf[autoprompt.ConfDepth] = (totalComponents * 10) / maxComponents
	}

	// Coherence: resolved/total cross-refs ratio.
	totalRefs := counts["cross_references"]
	resolvedRefs := counts["cross_references_resolved"]
	if totalRefs > 0 {
		conf[autoprompt.ConfCoherence] = (resolvedRefs * 10) / totalRefs
	}

	// Completeness: similar to coverage but includes more element types.
	completenessScore := 0
	elementTypes := []string{"sections", "invariants", "adrs", "quality_gates",
		"glossary_entries", "negative_specs", "verification_prompts"}
	for _, et := range elementTypes {
		if counts[et] > 0 {
			completenessScore++
		}
	}
	// Scale: 7 element types -> 0-10.
	conf[autoprompt.ConfCompleteness] = (completenessScore * 10) / 7

	// Formality: invariants with semi_formal / total invariants.
	if len(invs) > 0 {
		formalCount := 0
		for _, inv := range invs {
			if inv.SemiFormal != "" {
				formalCount++
			}
		}
		conf[autoprompt.ConfFormality] = (formalCount * 10) / len(invs)
	}

	return conf
}

// findLimitingFactor returns the dimension name with the lowest confidence score.
func findLimitingFactor(conf [5]int) string {
	minIdx := 0
	minVal := conf[0]
	// Use DimensionPriority for tie-breaking.
	for _, idx := range autoprompt.DimensionPriority {
		if conf[idx] < minVal {
			minVal = conf[idx]
			minIdx = idx
		}
	}
	return autoprompt.DimensionNames[minIdx]
}

// modeGuidance returns suggested next actions based on the observed mode and confidence.
func modeGuidance(mode ModeClassification, conf [5]int) []string {
	limitingFactor := findLimitingFactor(conf)

	switch mode.Mode {
	case "divergent":
		return []string{
			fmt.Sprintf("Explore alternatives for %s (current weakest dimension)", limitingFactor),
			"Open new questions to widen the inquiry space",
			"Record findings as they emerge",
		}
	case "convergent":
		return []string{
			"Narrow focus to the most promising thread",
			"Close resolved questions",
			"Crystallize emerging decisions into spec artifacts",
		}
	case "dialectical":
		return []string{
			"Continue the debate: pose challenges to current assumptions",
			"Synthesize opposing positions into a unified view",
			"Record the dialectical tension as an ADR problem statement",
		}
	case "abductive":
		return []string{
			"Follow the pattern: what hypothesis best explains the findings?",
			"Record the abductive inference as a finding",
			"Test the hypothesis against existing spec constraints",
		}
	case "metacognitive":
		return []string{
			"Review the thread topology and identify gaps",
			"Park or merge threads that have served their purpose",
			"Reflect on what modes have been underrepresented",
		}
	case "incubation":
		return []string{
			"Resume with fresh perspective",
			"Review parked threads for new insights",
			"Check if the spec has changed since last session",
		}
	case "crystallization":
		return []string{
			"Commit findings as spec artifacts (invariants, ADRs, sections)",
			"Run `ddis validate` to check artifact quality",
			"Close resolved questions and merge completed threads",
		}
	default:
		return []string{
			fmt.Sprintf("Focus on improving %s", limitingFactor),
			"Record any findings or open questions",
		}
	}
}

// nowRFC3339 returns the current UTC time in RFC3339 format.
func nowRFC3339() string {
	return timeNow().UTC().Format("2006-01-02T15:04:05Z07:00")
}
