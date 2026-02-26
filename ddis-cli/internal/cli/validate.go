package cli

import (
	"errors"
	"fmt"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/oplog"
	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/validator"
)

// ddis:maintains APP-INV-002 (validation determinism)
// ddis:maintains APP-INV-003 (cross-reference integrity)
// ddis:implements APP-ADR-004 (validation architecture)

var (
	validateJSON      bool
	validateChecks    string
	validateFocus     int
	validateLevel     int
	validateLog       bool
	validateOplogPath string
	validateCodeRoot  string
)

// ErrValidationFailed is returned when validation finds errors.
// The caller should exit with code 1.
var ErrValidationFailed = errors.New("validation failed")

var validateCmd = &cobra.Command{
	Use:   "validate [index.db]",
	Short: "Run mechanical validation checks against the spec index",
	Long: `Runs validation checks against a parsed DDIS spec index.
Checks include cross-reference integrity, invariant structure,
glossary completeness, structural conformance, and more.

If no database path is given, auto-discovers a *.ddis.db file in the current directory.

Examples:
  ddis validate
  ddis validate index.db
  ddis validate index.db --json
  ddis validate index.db --checks 1,2,3,9
  ddis validate index.db --focus 5`,
	Args:          cobra.RangeArgs(0, 1),
	RunE:          runValidate,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	validateCmd.Flags().BoolVar(&validateJSON, "json", false, "Output as JSON (for RALPH integration)")
	validateCmd.Flags().StringVar(&validateChecks, "checks", "", "Comma-separated list of check IDs to run (default: all)")
	validateCmd.Flags().IntVar(&validateFocus, "focus", 0, "Deep single-check mode: run one check with verbose findings")
	validateCmd.Flags().IntVar(&validateLevel, "level", 0, "Progressive validation level: 1=structural, 2=+content, 3=all")
	validateCmd.Flags().BoolVar(&validateLog, "log", false, "Append validation report to oplog")
	validateCmd.Flags().StringVar(&validateOplogPath, "oplog-path", "", "Custom oplog path (default: .ddis/oplog.jsonl)")
	validateCmd.Flags().StringVar(&validateCodeRoot, "code-root", "", "Source code root for implementation traceability check (Check 13)")
}

func runValidate(cmd *cobra.Command, args []string) error {
	var dbPath string
	if len(args) >= 1 {
		dbPath = args[0]
	}
	if dbPath == "" {
		var err error
		dbPath, err = FindDB()
		if err != nil {
			return err
		}
	}

	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found in database: %w", err)
	}

	WarnIfStale(db, specID)

	checkIDs, err := validator.ParseCheckIDs(validateChecks)
	if err != nil {
		return err
	}

	// --level sets progressive validation tiers per spec check-to-level table
	// (workspace-ops.md:654-672). L1 ⊂ L2 ⊂ L3 (monotonicity, APP-INV-040).
	if validateLevel > 0 && len(checkIDs) == 0 {
		switch validateLevel {
		case 1:
			checkIDs = []int{10, 12} // G1 + NS (structural fast)
		case 2:
			checkIDs = []int{1, 2, 4, 5, 7, 8, 10, 12} // L1 + C1,C2,C4,C5,C7,C8
		default:
			// level 3+ = all (leave checkIDs nil)
		}
	}

	// --focus overrides --checks/--level: single check, verbose output.
	if validateFocus > 0 {
		checkIDs = []int{validateFocus}
	}

	opts := validator.ValidateOptions{
		CheckIDs: checkIDs,
		CodeRoot: validateCodeRoot,
	}

	report, err := validator.Validate(db, specID, opts)
	if err != nil {
		return err
	}

	if validateFocus > 0 && !validateJSON {
		// Focus mode: verbose single-check output.
		return renderFocusReport(report, validateFocus)
	}

	out, err := validator.RenderReport(report, validateJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)

	// Emit validation_run event to Stream 2 (Specification).
	emitEvent(dbPath, events.StreamSpecification, events.TypeValidationRun, specHashFromDB(db, specID), map[string]interface{}{
		"passed": report.Passed,
		"failed": report.Failed,
		"errors": report.Errors,
		"checks": len(report.Results),
	})

	if validateLog {
		spec, specErr := storage.GetSpecIndex(db, specID)
		if specErr != nil {
			return fmt.Errorf("get spec for oplog: %w", specErr)
		}

		oplogPath := validateOplogPath
		if oplogPath == "" {
			oplogPath = oplog.DefaultPath(".")
		}

		vd := oplog.ImportValidation(report, spec.SpecPath, spec.ContentHash)
		rec, recErr := oplog.NewValidateRecord("", vd)
		if recErr != nil {
			return fmt.Errorf("create validate record: %w", recErr)
		}
		if appendErr := oplog.Append(oplogPath, rec); appendErr != nil {
			return fmt.Errorf("append to oplog: %w", appendErr)
		}
		fmt.Fprintf(cmd.ErrOrStderr(), "Validation record appended to %s\n", oplogPath)
	}

	// Guidance postscript
	if !NoGuidance && !validateJSON {
		emitValidateGuidance(report)
	}

	if report.Errors > 0 {
		return ErrValidationFailed
	}

	return nil
}

func renderFocusReport(report *validator.Report, checkID int) error {
	for _, res := range report.Results {
		if res.CheckID != checkID {
			continue
		}
		status := "PASSED"
		if !res.Passed {
			status = "FAILED"
		}
		fmt.Printf("Focus: Check %d — %s [%s]\n", res.CheckID, res.CheckName, status)
		fmt.Println(strings.Repeat("═", 60))
		fmt.Printf("Summary: %s\n", res.Summary)
		if res.InvariantID != "" {
			fmt.Printf("Invariant: %s\n", res.InvariantID)
		}

		if len(res.Findings) == 0 {
			fmt.Println("\nNo findings.")
		} else {
			errors, warnings, infos := 0, 0, 0
			for _, f := range res.Findings {
				switch f.Severity {
				case "error":
					errors++
				case "warning":
					warnings++
				default:
					infos++
				}
			}
			fmt.Printf("\nFindings: %d total (%d errors, %d warnings, %d info)\n",
				len(res.Findings), errors, warnings, infos)
			fmt.Println(strings.Repeat("─", 60))

			for i, f := range res.Findings {
				sev := "INFO"
				switch f.Severity {
				case "error":
					sev = "ERROR"
				case "warning":
					sev = "WARN"
				}
				fmt.Printf("  %d. [%s] %s", i+1, sev, f.Message)
				if f.Location != "" {
					fmt.Printf(" (at %s)", f.Location)
				}
				if f.InvariantID != "" {
					fmt.Printf(" [%s]", f.InvariantID)
				}
				fmt.Println()
			}
		}

		if !NoGuidance && !res.Passed {
			fmt.Printf("\nNext: ddis context %s\n", res.InvariantID)
			fmt.Println("  Investigate the failing check's related elements.")
		}
		return nil
	}
	return fmt.Errorf("check %d not found in report", checkID)
}

func emitValidateGuidance(report *validator.Report) {
	if report.Failed > 0 {
		// Find first failing check with an element to investigate
		for _, res := range report.Results {
			if res.Passed {
				continue
			}
			for _, f := range res.Findings {
				if f.InvariantID != "" {
					fmt.Printf("\nNext: ddis context %s\n", f.InvariantID)
					fmt.Printf("  Investigate the highest-impact failing element.\n")
					return
				}
			}
			// No element ID — suggest re-running with focus
			fmt.Printf("\nNext: ddis validate --checks %d\n", res.CheckID)
			fmt.Printf("  Review Check %d details.\n", res.CheckID)
			return
		}
	} else {
		fmt.Println("\nNext: ddis coverage && ddis drift --report")
		fmt.Println("  All checks passing — verify completeness and alignment.")
	}
}
