package cli

// ddis:implements APP-ADR-056 (spec fitness function as endogenous quality signal)
// ddis:implements APP-ADR-057 (agent-executable protocol for zero-knowledge participation)
// ddis:maintains APP-INV-069 (triage monotonic fitness — F(S) computed and ranked)
// ddis:maintains APP-INV-070 (protocol completeness — self-contained JSON output)

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/coverage"
	"github.com/wvandaal/ddis/internal/drift"
	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/triage"
	"github.com/wvandaal/ddis/internal/validator"
)

var (
	triageAuto     bool
	triageDryRun   bool
	triageJSON     bool
	triageProtocol bool
	triageHistory  bool
)

var triageCmd = &cobra.Command{
	Use:   "triage [db-path]",
	Short: "Recursive self-improvement: fitness analysis, ranked work, agent protocol",
	Long: `Computes the Spec Fitness Function F(S) and provides ranked work items
for driving the spec toward fixpoint (F=1.0).

Modes:
  ddis triage                     Show current fitness and measure
  ddis triage --auto              Full analysis with auto-filed issues (ranked by ΔF)
  ddis triage --auto --dry-run    Preview without filing
  ddis triage --protocol          Emit agent-executable JSON protocol
  ddis triage --history           Show F(S) trajectory over time

The fitness function combines 6 signals:
  Validation (20%), Coverage (20%), Drift (20%),
  Challenge health (15%), Contradictions (15%), Issue backlog (10%)

Examples:
  ddis triage
  ddis triage --auto --dry-run
  ddis triage --protocol --json
  ddis triage --history`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runTriage,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	triageCmd.Flags().BoolVar(&triageAuto, "auto", false, "Run full fitness analysis and rank deficiencies by ΔF")
	triageCmd.Flags().BoolVar(&triageDryRun, "dry-run", false, "Preview auto-filed issues without filing (requires --auto)")
	triageCmd.Flags().BoolVar(&triageJSON, "json", false, "Machine-readable JSON output")
	triageCmd.Flags().BoolVar(&triageProtocol, "protocol", false, "Emit agent-executable protocol JSON")
	triageCmd.Flags().BoolVar(&triageHistory, "history", false, "Show fitness trajectory over time")
}

func runTriage(cmd *cobra.Command, args []string) error {
	dbPath, err := resolveDB(args)
	if err != nil {
		return err
	}

	db, err := storage.OpenExisting(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found in %s: %w", dbPath, err)
	}

	// Collect all quality signals
	signals := collectSignals(db, specID, dbPath)
	fitness := triage.ComputeFitness(signals)

	// Load event stream for measure computation
	wsRoot := events.WorkspaceRoot(dbPath)
	streamPath := events.StreamPath(wsRoot, events.StreamImplementation)
	rawEvts, _ := events.ReadStream(streamPath, events.EventFilters{})
	evts := derefEvents(rawEvts)

	// Compute triage measure
	covResult, _ := coverage.Analyze(db, specID, coverage.Options{})
	unspecified := 0
	if covResult != nil {
		unspecified = len(covResult.Gaps)
	}
	driftReport, _ := drift.Analyze(db, specID, drift.Options{Report: true})
	driftScore := 0
	if driftReport != nil {
		driftScore = driftReport.EffectiveDrift
	}
	measure := triage.ComputeMeasure(evts, unspecified, driftScore)

	if triageProtocol {
		return runTriageProtocol(cmd, specID, fitness, measure, evts, dbPath)
	}

	if triageHistory {
		return runTriageHistory(cmd, evts, fitness)
	}

	if triageAuto {
		return runTriageAuto(cmd, fitness, measure, dbPath, evts)
	}

	// Default: show current fitness and measure
	if triageJSON {
		out := map[string]interface{}{
			"fitness": fitness,
			"measure": measure,
		}
		enc := json.NewEncoder(cmd.OutOrStdout())
		enc.SetIndent("", "  ")
		return enc.Encode(out)
	}

	fmt.Fprintf(cmd.OutOrStdout(), "Fitness: F(S) = %.4f\n", fitness.Score)
	fmt.Fprintf(cmd.OutOrStdout(), "Measure: μ = (%d, %d, %d)\n",
		measure.OpenIssues, measure.Unspecified, measure.DriftScore)
	fmt.Fprintf(cmd.OutOrStdout(), "Lyapunov: V(S) = %.4f\n", 1.0-fitness.Score)

	if fitness.Score >= 1.0 {
		fmt.Fprintln(cmd.OutOrStdout(), "\nFixpoint reached: F(S) = 1.0, μ = (0,0,0)")
	} else if !NoGuidance {
		fmt.Fprintln(cmd.ErrOrStderr(), "\nNext: ddis triage --auto --dry-run")
		fmt.Fprintln(cmd.ErrOrStderr(), "  Preview ranked deficiencies and estimated ΔF.")
	}

	return nil
}

func runTriageAuto(cmd *cobra.Command, fitness triage.FitnessResult, measure triage.Measure, dbPath string, evts []events.Event) error {
	defs := triage.RankDeficiencies(fitness.Signals, dbPath)

	if triageJSON {
		out := map[string]interface{}{
			"fitness":      fitness,
			"measure":      measure,
			"deficiencies": defs,
			"dry_run":      triageDryRun,
		}
		enc := json.NewEncoder(cmd.OutOrStdout())
		enc.SetIndent("", "  ")
		return enc.Encode(out)
	}

	fmt.Fprintf(cmd.OutOrStdout(), "Fitness: F(S) = %.4f  |  Lyapunov: V(S) = %.4f\n", fitness.Score, 1.0-fitness.Score)
	fmt.Fprintf(cmd.OutOrStdout(), "Measure: μ = (%d, %d, %d)\n\n",
		measure.OpenIssues, measure.Unspecified, measure.DriftScore)

	if len(defs) == 0 {
		fmt.Fprintln(cmd.OutOrStdout(), "No deficiencies — fixpoint reached.")
		return nil
	}

	fmt.Fprintf(cmd.OutOrStdout(), "Ranked deficiencies (%d):\n", len(defs))
	for i, d := range defs {
		prefix := "  "
		if triageDryRun {
			prefix = "  [dry-run] "
		}
		fmt.Fprintf(cmd.OutOrStdout(), "%s%d. [%s] ΔF≈%.4f  %s\n", prefix, i+1, d.Category, d.DeltaF, d.Description)
		fmt.Fprintf(cmd.OutOrStdout(), "%s   → %s\n", prefix, d.Action)
	}

	// Emit triage event
	emitEvent(dbPath, events.StreamImplementation, events.TypeStatusChanged, "", map[string]interface{}{
		"action":  "triage_auto",
		"fitness": fitness.Score,
		"measure": measure,
		"dry_run": triageDryRun,
	})

	if !NoGuidance && len(defs) > 0 {
		fmt.Fprintf(cmd.ErrOrStderr(), "\nNext: %s\n", defs[0].Action)
		fmt.Fprintln(cmd.ErrOrStderr(), "  Address the highest-ΔF deficiency first.")
	}

	return nil
}

func runTriageProtocol(cmd *cobra.Command, specID int64, fitness triage.FitnessResult, measure triage.Measure, evts []events.Event, dbPath string) error {
	protocol := triage.GenerateProtocol(specID, fitness, measure, evts, dbPath)

	enc := json.NewEncoder(cmd.OutOrStdout())
	enc.SetIndent("", "  ")
	return enc.Encode(protocol)
}

func runTriageHistory(cmd *cobra.Command, evts []events.Event, current triage.FitnessResult) error {
	trajectory := triage.LoadFitnessTrajectory(evts)
	trajectory = append(trajectory, current.Score)

	if triageJSON {
		out := map[string]interface{}{
			"trajectory": trajectory,
			"current":    current.Score,
		}
		enc := json.NewEncoder(cmd.OutOrStdout())
		enc.SetIndent("", "  ")
		return enc.Encode(out)
	}

	fmt.Fprintln(cmd.OutOrStdout(), "Fitness trajectory:")
	for i, f := range trajectory {
		bar := ""
		for j := 0; j < int(f*40); j++ {
			bar += "█"
		}
		label := ""
		if i == len(trajectory)-1 {
			label = " ← current"
		}
		fmt.Fprintf(cmd.OutOrStdout(), "  S%d: %.4f %s%s\n", i, f, bar, label)
	}

	return nil
}

// collectSignals gathers the 6 normalized quality signals from the spec index.
func collectSignals(db storage.DB, specID int64, dbPath string) triage.FitnessSignals {
	signals := triage.FitnessSignals{}

	// V(S) = validation_passed / validation_total
	report, err := validator.Validate(db, specID, validator.ValidateOptions{})
	if err == nil && report.TotalChecks > 0 {
		signals.Validation = float64(report.Passed) / float64(report.TotalChecks)
	}

	// C(S) = coverage_pct
	covResult, err := coverage.Analyze(db, specID, coverage.Options{})
	if err == nil && covResult != nil {
		signals.Coverage = covResult.Summary.Score
	}

	// D(S) = drift_score / max_drift (capped at 1.0)
	driftReport, err := drift.Analyze(db, specID, drift.Options{Report: true})
	if err == nil && driftReport != nil {
		maxDrift := 100 // reasonable ceiling
		if driftReport.EffectiveDrift > maxDrift {
			signals.Drift = 1.0
		} else if maxDrift > 0 {
			signals.Drift = float64(driftReport.EffectiveDrift) / float64(maxDrift)
		}
	}

	// H(S) = challenges_confirmed / challenges_total
	challenges, err := storage.ListChallengeResults(db, specID)
	if err == nil && len(challenges) > 0 {
		confirmed := 0
		for _, c := range challenges {
			if c.Verdict == "confirmed" {
				confirmed++
			}
		}
		signals.ChallengeHP = float64(confirmed) / float64(len(challenges))
	} else {
		signals.ChallengeHP = 1.0 // No challenges needed = perfect
	}

	// K(S) = contradictions not implemented here — default to 0
	signals.Contradictions = 0.0

	// I(S) = open_issues / total_issues — requires event stream
	wsRoot := events.WorkspaceRoot(dbPath)
	streamPath := events.StreamPath(wsRoot, events.StreamImplementation)
	rawEvts, _ := events.ReadStream(streamPath, events.EventFilters{})
	issues := triage.DeriveAllIssueStates(derefEvents(rawEvts))
	totalIssues := len(issues)
	openIssues := 0
	for _, info := range issues {
		if !info.State.IsTerminal() {
			openIssues++
		}
	}
	if totalIssues > 0 {
		signals.IssueBacklog = float64(openIssues) / float64(totalIssues)
	}

	return signals
}

// resolveDB resolves the database path from args or auto-discovery.
func resolveDB(args []string) (string, error) {
	if len(args) > 0 {
		if _, err := os.Stat(args[0]); err != nil {
			return "", fmt.Errorf("database not found: %s", args[0])
		}
		return args[0], nil
	}
	return FindDB()
}
