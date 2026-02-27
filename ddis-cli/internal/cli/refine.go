package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/refine"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:implements APP-ADR-022 (state monad architecture)
// ddis:maintains APP-INV-022 (refinement drift monotonicity)
// ddis:maintains APP-INV-024 (ambiguity surfacing — --surface-ambiguity flag surfaces questions without autonomous resolution)

var (
	refineSpec      string
	refineIteration int
	refinePromptOnly bool
	refineSurfaceAmb bool
)

var refineCmd = &cobra.Command{
	Use:   "refine",
	Short: "Iteratively improve spec quality (RALPH loop)",
	Long: `The refine loop (audit → plan → apply → judge) iteratively improves
spec quality. Each iteration targets one quality dimension and enforces
drift monotonicity — quality must not regress.

Subcommands:
  audit   Generate diagnostic report from drift + validation + coverage
  plan    Select one quality dimension to focus on
  apply   Generate LLM edit prompt with exemplars
  judge   Evaluate quality trajectory and enforce monotonicity

All subcommands return (output, state, guidance) — the state monad triple.
Use --prompt-only to emit guidance without side effects.

Examples:
  ddis refine audit --spec index.db
  ddis refine plan --spec index.db --surface-ambiguity
  ddis refine apply --spec index.db
  ddis refine judge --spec index.db --iteration 1`,
	SilenceErrors: true,
	SilenceUsage:  true,
}

var refineAuditCmd = &cobra.Command{
	Use:   "audit",
	Short: "Generate diagnostic report",
	RunE:  runRefineAudit,
	SilenceErrors: true,
	SilenceUsage:  true,
}

var refinePlanCmd = &cobra.Command{
	Use:   "plan",
	Short: "Select quality dimension to focus on",
	RunE:  runRefinePlan,
	SilenceErrors: true,
	SilenceUsage:  true,
}

var refineApplyCmd = &cobra.Command{
	Use:   "apply",
	Short: "Generate LLM edit prompt with exemplars",
	RunE:  runRefineApply,
	SilenceErrors: true,
	SilenceUsage:  true,
}

var refineJudgeCmd = &cobra.Command{
	Use:   "judge",
	Short: "Evaluate quality trajectory",
	RunE:  runRefineJudge,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	refineCmd.PersistentFlags().StringVar(&refineSpec, "spec", "", "Path to spec database (required)")
	refineCmd.PersistentFlags().IntVar(&refineIteration, "iteration", 0, "Current iteration number")
	refineCmd.PersistentFlags().BoolVar(&refinePromptOnly, "prompt-only", false, "Emit guidance without side effects")

	refinePlanCmd.Flags().BoolVar(&refineSurfaceAmb, "surface-ambiguity", false, "Surface design ambiguities as questions")

	refineCmd.AddCommand(refineAuditCmd)
	refineCmd.AddCommand(refinePlanCmd)
	refineCmd.AddCommand(refineApplyCmd)
	refineCmd.AddCommand(refineJudgeCmd)

	// --spec is no longer required; auto-discovery or positional arg can provide DB path
}

// resolveRefineSpec resolves the spec DB path from --spec flag, positional arg, or auto-discovery.
func resolveRefineSpec(args []string) (string, error) {
	if refineSpec != "" {
		return refineSpec, nil
	}
	if len(args) >= 1 {
		return args[0], nil
	}
	return FindDB()
}

func runRefineAudit(cmd *cobra.Command, args []string) error {
	specPath, err := resolveRefineSpec(args)
	if err != nil {
		return err
	}
	db, err := storage.OpenExisting(specPath)
	if err != nil {
		return fmt.Errorf("open spec database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	result, err := refine.Audit(db, specID, refineIteration)
	if err != nil {
		return fmt.Errorf("refine audit: %w", err)
	}

	out, err := result.RenderJSON()
	if err != nil {
		return err
	}
	fmt.Println(out)

	// ddis:maintains APP-INV-053 (event stream completeness — emits finding_recorded to stream 1)
	emitEvent(specPath, events.StreamDiscovery, events.TypeFindingRecorded, specHashFromDB(db, specID), map[string]interface{}{
		"subcommand":      "audit",
		"iteration":       refineIteration,
		"limiting_factor": result.State.LimitingFactor,
		"spec_drift":      result.State.SpecDrift,
		"command":         "refine",
	})

	return nil
}

func runRefinePlan(cmd *cobra.Command, args []string) error {
	specPath, err := resolveRefineSpec(args)
	if err != nil {
		return err
	}
	db, err := storage.OpenExisting(specPath)
	if err != nil {
		return fmt.Errorf("open spec database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	result, err := refine.Plan(db, specID, refineIteration, refineSurfaceAmb)
	if err != nil {
		return fmt.Errorf("refine plan: %w", err)
	}

	out, err := result.RenderJSON()
	if err != nil {
		return err
	}
	fmt.Println(out)

	// ddis:maintains APP-INV-053 (event stream completeness — emits finding_recorded to stream 1)
	emitEvent(specPath, events.StreamDiscovery, events.TypeFindingRecorded, specHashFromDB(db, specID), map[string]interface{}{
		"subcommand":      "plan",
		"iteration":       refineIteration,
		"limiting_factor": result.State.LimitingFactor,
		"command":         "refine",
	})

	return nil
}

func runRefineApply(cmd *cobra.Command, args []string) error {
	specPath, err := resolveRefineSpec(args)
	if err != nil {
		return err
	}
	db, err := storage.OpenExisting(specPath)
	if err != nil {
		return fmt.Errorf("open spec database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	result, err := refine.Apply(db, specID, refineIteration)
	if err != nil {
		return fmt.Errorf("refine apply: %w", err)
	}

	out, err := result.RenderJSON()
	if err != nil {
		return err
	}
	fmt.Println(out)

	// ddis:maintains APP-INV-053 (event stream completeness — emits finding_recorded to stream 1)
	emitEvent(specPath, events.StreamDiscovery, events.TypeFindingRecorded, specHashFromDB(db, specID), map[string]interface{}{
		"subcommand": "apply",
		"iteration":  refineIteration,
		"command":    "refine",
	})

	return nil
}

func runRefineJudge(cmd *cobra.Command, args []string) error {
	specPath, err := resolveRefineSpec(args)
	if err != nil {
		return err
	}
	db, err := storage.OpenExisting(specPath)
	if err != nil {
		return fmt.Errorf("open spec database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	result, err := refine.Judge(db, specID, refineIteration)
	if err != nil {
		return fmt.Errorf("refine judge: %w", err)
	}

	out, err := result.RenderJSON()
	if err != nil {
		return err
	}
	fmt.Println(out)

	// ddis:maintains APP-INV-053 (event stream completeness — emits finding_recorded to stream 1)
	emitEvent(specPath, events.StreamDiscovery, events.TypeFindingRecorded, specHashFromDB(db, specID), map[string]interface{}{
		"subcommand": "judge",
		"iteration":  refineIteration,
		"spec_drift": result.State.SpecDrift,
		"command":    "refine",
	})

	return nil
}
