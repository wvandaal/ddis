package cli

// ddis:implements APP-ADR-033 (ddis next as universal entry point)
// ddis:maintains APP-INV-042 (guidance emission — next emits guidance based on state)

import (
	"fmt"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/coverage"
	"github.com/wvandaal/ddis/internal/drift"
	"github.com/wvandaal/ddis/internal/storage"
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
		fmt.Println("No DDIS database found in current directory.")
		fmt.Println("\nNext: ddis parse manifest.yaml")
		return nil
	}

	db, err := storage.Open(dbPath)
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
	fmt.Println(status.String())

	// Determine next action
	if report.Failed > 0 {
		// Find the first failing check
		for _, res := range report.Results {
			if !res.Passed {
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

	if covErr == nil && covResult != nil && len(covResult.Gaps) > 0 {
		// Find the first gap element
		gap := covResult.Gaps[0]
		element := strings.SplitN(gap, ":", 2)[0]
		fmt.Printf("\nNext: ddis exemplar %s\n", element)
		fmt.Printf("  Coverage gap: %s — see corpus examples for improvement.\n", gap)
		return nil
	}

	if driftErr == nil && driftReport != nil && driftTotal > 0 {
		q := driftReport.QualityBreakdown
		if q.Correctness > 0 && len(driftReport.ImplDrift.Details) > 0 {
			element := driftReport.ImplDrift.Details[0].Element
			fmt.Printf("\nNext: ddis context %s\n", element)
			fmt.Printf("  Correctness drift — investigate the first drifted element.\n")
		} else if q.Coherence > 0 {
			fmt.Println("\nNext: ddis validate --checks 1")
			fmt.Println("  Coherence drift — check cross-reference integrity.")
		} else {
			fmt.Println("\nNext: ddis drift --report")
			fmt.Println("  Depth drift — review full drift report for formalization targets.")
		}
		return nil
	}

	// All gates passing
	fmt.Println("\nAll quality gates passing. Spec and implementation are aligned.")
	return nil
}
