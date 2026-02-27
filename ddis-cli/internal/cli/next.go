package cli

// ddis:implements APP-ADR-033 (ddis next as universal entry point)
// ddis:implements APP-ADR-041 (challenge-feedback loop closes bilateral lifecycle)
// ddis:maintains APP-INV-042 (guidance emission — next emits guidance based on state)
// ddis:maintains APP-INV-051 (challenge-informed navigation)
// ddis:maintains APP-INV-068 (fixpoint termination — triage-aware priority routing)

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/coverage"
	"github.com/wvandaal/ddis/internal/drift"
	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/triage"
	"github.com/wvandaal/ddis/internal/validator"
)

var nextCmd = &cobra.Command{
	Use:   "next",
	Short: "Show current status and recommend next action",
	Long: `Reads the current spec state (validation, coverage, drift) and recommends
the highest-impact next command to run.

This is the universal entry point — it tells you what to do.

Examples:
  ddis next
  ddis         (bare invocation delegates here)`,
	Args:          cobra.NoArgs,
	RunE:          runNext,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func runNext(cmd *cobra.Command, args []string) error {
	dbPath, err := FindDB()
	if err != nil {
		// Cold-start detection: distinguish "no workspace" from "workspace but no DB"
		_, manifestErr := os.Stat("manifest.yaml")
		_, ddisErr := os.Stat(".ddis")
		if os.IsNotExist(manifestErr) && os.IsNotExist(ddisErr) {
			fmt.Println("No DDIS workspace found in current directory.")
			fmt.Println("\nNext: ddis init --name \"My Spec\"")
			fmt.Println("  Initialize a new DDIS specification workspace.")
			return nil
		}
		fmt.Println("No DDIS database found in current directory.")
		fmt.Println("\nNext: ddis parse manifest.yaml")
		return nil
	}

	db, err := storage.OpenExisting(dbPath)
	if err != nil {
		fmt.Printf("Cannot open %s: %v\n", dbPath, err)
		fmt.Println("\nNext: ddis parse manifest.yaml")
		return nil
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		fmt.Printf("No spec found in %s.\n", dbPath)
		fmt.Println("\nNext: ddis parse manifest.yaml")
		return nil
	}

	spec, _ := storage.GetSpecIndex(db, specID)
	specName := dbPath
	if spec != nil && spec.SpecName != "" {
		specName = spec.SpecName
	}

	// Run quick validation
	report, err := validator.Validate(db, specID, validator.ValidateOptions{})
	if err != nil {
		fmt.Printf("%s: validation error: %v\n", specName, err)
		return nil
	}

	// Run quick coverage
	covResult, covErr := coverage.Analyze(db, specID, coverage.Options{})

	// Run quick drift
	driftReport, driftErr := drift.Analyze(db, specID, drift.Options{Report: true})

	// Query challenge results
	challenges, _ := storage.ListChallengeResults(db, specID)
	confirmed, provisional, refuted, inconclusive := 0, 0, 0, 0
	var refutedIDs, provisionalIDs []string
	for _, cr := range challenges {
		switch cr.Verdict {
		case "confirmed":
			confirmed++
		case "provisional":
			provisional++
			provisionalIDs = append(provisionalIDs, cr.InvariantID)
		case "refuted":
			refuted++
			refutedIDs = append(refutedIDs, cr.InvariantID)
		case "inconclusive":
			inconclusive++
		}
	}

	// Build status line
	var status strings.Builder
	fmt.Fprintf(&status, "%s: %d/%d validation", specName, report.Passed, report.TotalChecks)

	covPct := 0.0
	if covErr == nil && covResult != nil {
		covPct = covResult.Summary.Score * 100
		fmt.Fprintf(&status, ", %.0f%% coverage", covPct)
	}

	driftTotal := 0
	if driftErr == nil && driftReport != nil {
		driftTotal = driftReport.EffectiveDrift
		fmt.Fprintf(&status, ", %d drift", driftTotal)
	}

	if len(challenges) > 0 {
		fmt.Fprintf(&status, ", %d/%d confirmed", confirmed, len(challenges))
		if refuted > 0 {
			fmt.Fprintf(&status, " (%d REFUTED)", refuted)
		}
	}
	fmt.Println(status.String())

	// Priority 1: Refuted invariants — ALWAYS highest priority
	if refuted > 0 {
		fmt.Printf("\nREFUTED INVARIANTS (%d):\n", refuted)
		for _, id := range refutedIDs {
			fmt.Printf("  %s — challenge found contradiction or test failure\n", id)
		}
		fmt.Printf("\nNext: ddis context %s\n", refutedIDs[0])
		fmt.Println("  Refuted invariant requires immediate remediation — fix implementation or amend spec.")
		return nil
	}

	// Priority 1.5: Triage lifecycle advancement (APP-INV-068)
	// Surface lifecycle bookends: verified (ready to close) and filed (needs triage)
	{
		wsRoot := events.WorkspaceRoot(dbPath)
		implStream := events.StreamPath(wsRoot, events.StreamImplementation)
		implEvts, _ := events.ReadStream(implStream, events.EventFilters{})
		allIssues := triage.DeriveAllIssueStates(derefEvents(implEvts))

		var best *triage.IssueInfo
		for _, info := range allIssues {
			if info.State.IsTerminal() {
				continue
			}
			if best == nil || info.State.Order() > best.State.Order() {
				best = info
			}
		}

		if best != nil {
			switch best.State {
			case triage.StateVerified:
				fmt.Printf("\nNext: ddis issue close %d\n", best.Number)
				fmt.Println("  Verified issue ready to close — evidence chain will be verified.")
				return nil
			case triage.StateImplementing:
				fmt.Printf("\nNext: ddis challenge --all %s --code-root .\n", dbPath)
				fmt.Printf("  Issue #%d implementing — challenge to advance to verified.\n", best.Number)
				return nil
			case triage.StateSpecified:
				if len(best.AffectedInvariants) > 0 {
					fmt.Printf("\nNext: ddis witness %s --type test --evidence \"...\"\n", best.AffectedInvariants[0])
				} else {
					fmt.Println("\nNext: ddis witness <INV-ID> --type test --evidence \"...\"")
				}
				fmt.Printf("  Issue #%d specified — implement and witness affected invariants.\n", best.Number)
				return nil
			case triage.StateTriaged:
				if best.ThreadID != "" {
					fmt.Printf("\nNext: ddis discover --thread %s\n", best.ThreadID)
					fmt.Println("  Continue investigation until spec converges.")
				}
				// Don't return — let quality signals take priority for triaged issues
			case triage.StateFiled:
				fmt.Printf("\nNext: ddis issue triage %d --thread <thread-id>\n", best.Number)
				fmt.Println("  Filed issue awaiting triage — link to discovery thread.")
				return nil
			}
		}
	}

	// Priority 2: Non-challenge validation failures
	if report.Failed > 0 {
		// Find the first failing check (skip Check 17 if challenges exist and it's the only failure)
		for _, res := range report.Results {
			if !res.Passed {
				// Check 17 (challenge freshness) is expected to fail if challenges haven't been run yet
				if res.CheckID == 17 && len(challenges) == 0 {
					fmt.Printf("\nNext: ddis challenge --all %s --code-root .\n", dbPath)
					fmt.Println("  No challenge results — run challenges to verify invariant witnesses.")
					return nil
				}
				firstElement := ""
				for _, f := range res.Findings {
					if f.InvariantID != "" {
						firstElement = f.InvariantID
						break
					}
				}
				if firstElement != "" {
					fmt.Printf("\nNext: ddis context %s\n", firstElement)
					fmt.Printf("  Check %d (%s) failed — investigate the highest-impact element.\n", res.CheckID, res.CheckName)
				} else {
					fmt.Printf("\nNext: ddis validate --checks %d\n", res.CheckID)
					fmt.Printf("  Check %d (%s) failed — review details.\n", res.CheckID, res.CheckName)
				}
				return nil
			}
		}
	}

	// Priority 3: Coverage gaps
	if covErr == nil && covResult != nil && len(covResult.Gaps) > 0 {
		gap := covResult.Gaps[0]
		element := strings.SplitN(gap, ":", 2)[0]
		fmt.Printf("\nNext: ddis exemplar %s\n", element)
		fmt.Printf("  Coverage gap: %s — see corpus examples for improvement.\n", gap)
		return nil
	}

	// Priority 4: Drift
	if driftErr == nil && driftReport != nil && driftTotal > 0 {
		q := driftReport.QualityBreakdown
		if q.Correctness > 0 && len(driftReport.ImplDrift.Details) > 0 {
			element := driftReport.ImplDrift.Details[0].Element
			fmt.Printf("\nNext: ddis context %s\n", element)
			fmt.Println("  Correctness drift — investigate the first drifted element.")
		} else if q.Coherence > 0 {
			fmt.Println("\nNext: ddis validate --checks 1")
			fmt.Println("  Coherence drift — check cross-reference integrity.")
		} else {
			fmt.Println("\nNext: ddis drift --report")
			fmt.Println("  Depth drift — review full drift report for formalization targets.")
		}
		return nil
	}

	// Priority 5: Provisional invariants — suggest targeted upgrades
	if provisional > 0 {
		fmt.Printf("\nChallenge summary: %d confirmed, %d provisional, %d refuted\n", confirmed, provisional, refuted)
		fmt.Printf("\nProvisional invariants (%d) — upgrade paths:\n", provisional)
		shown := 0
		for _, id := range provisionalIDs {
			if shown >= 5 {
				fmt.Printf("  ... and %d more\n", provisional-5)
				break
			}
			fmt.Printf("  %s — write behavioral test or add annotations across more packages\n", id)
			shown++
		}
		if len(provisionalIDs) > 0 {
			fmt.Printf("\nNext: ddis challenge %s %s --code-root .\n", provisionalIDs[0], dbPath)
			fmt.Println("  Strengthen evidence for provisional invariants.")
		}
		return nil
	}

	// Priority 6: No challenges run yet
	if len(challenges) == 0 {
		fmt.Printf("\nNext: ddis challenge --all %s --code-root .\n", dbPath)
		fmt.Println("  No challenge results — run challenges to verify invariant witnesses.")
		return nil
	}

	// All gates passing, all challenges confirmed — consult mode for guidance
	fmt.Printf("\nAll quality gates passing. %d/%d invariants confirmed. Spec and implementation are fully aligned.\n", confirmed, len(challenges))

	// ddis:maintains APP-INV-026 (classification non-prescriptive — mode informs, never dictates)
	if hint := modeHint(dbPath); hint != "" {
		fmt.Printf("\nMode-informed suggestion: %s\n", hint)
	}

	return nil
}

// modeHint reads the latest mode_observed event and returns a contextual suggestion.
// Returns empty string if no mode data is available (best-effort, non-blocking).
func modeHint(dbPath string) string {
	wsRoot := events.WorkspaceRoot(dbPath)
	streamPath := events.StreamPath(wsRoot, events.StreamDiscovery)
	evts, err := events.ReadStream(streamPath, events.EventFilters{Type: events.TypeModeObserved})
	if err != nil || len(evts) == 0 {
		return ""
	}
	latest := evts[len(evts)-1]
	var payload map[string]interface{}
	if err := json.Unmarshal(latest.Payload, &payload); err != nil {
		return ""
	}
	mode, _ := payload["mode"].(string)
	switch mode {
	case "crystallization":
		return "crystallization mode — consider `ddis witness` or `ddis challenge` to crystallize discoveries into proof"
	case "divergent":
		return "divergent mode — consider `ddis discover` to widen the inquiry space"
	case "incubation":
		return "incubation mode — consider `ddis context` to review without commitment"
	case "convergent":
		return "convergent mode — consider `ddis challenge --all` to tighten toward closure"
	case "dialectical":
		return "dialectical mode — consider `ddis contradict` or `ddis refine plan --surface-ambiguity`"
	default:
		return ""
	}
}
