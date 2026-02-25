package cli

import (
	"encoding/json"
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/validator"
)

// ddis:interfaces APP-INV-002 (validation determinism)

var (
	checkpointGates string
	checkpointJSON  bool
)

var checkpointCmd = &cobra.Command{
	Use:   "checkpoint [db-path]",
	Short: "Run quality gate checks",
	Long: `Runs quality gate checks against a parsed DDIS spec index.
A friendlier wrapper around validate that uses gate-focused terminology.

Examples:
  ddis checkpoint index.db
  ddis checkpoint index.db --gate 1,2
  ddis checkpoint --json`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runCheckpoint,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	checkpointCmd.Flags().StringVar(&checkpointGates, "gate", "", "Comma-separated gate/check IDs to run (default: all)")
	checkpointCmd.Flags().BoolVar(&checkpointJSON, "json", false, "Output as JSON")
}

func runCheckpoint(cmd *cobra.Command, args []string) error {
	var dbPath string
	if len(args) >= 1 {
		dbPath = args[0]
	}
	if dbPath == "" {
		var err error
		dbPath, err = findDB()
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
		return fmt.Errorf("no spec found: %w", err)
	}

	checkIDs, err := validator.ParseCheckIDs(checkpointGates)
	if err != nil {
		return fmt.Errorf("invalid gate IDs: %w", err)
	}

	opts := validator.ValidateOptions{
		CheckIDs: checkIDs,
	}

	report, err := validator.Validate(db, specID, opts)
	if err != nil {
		return err
	}

	if checkpointJSON {
		data, err := json.MarshalIndent(report, "", "  ")
		if err != nil {
			return fmt.Errorf("marshal report: %w", err)
		}
		fmt.Println(string(data))
	} else {
		fmt.Printf("DDIS Checkpoint: %s\n", report.SpecPath)
		fmt.Println("═══════════════════════════════════════════")
		for _, r := range report.Results {
			status := "FAIL"
			marker := "✗"
			if r.Passed {
				status = "PASS"
				marker = "✓"
			}
			fmt.Printf("  Gate %2d: %-35s %s %s\n", r.CheckID, r.CheckName, status, marker)
		}
		fmt.Println("───────────────────────────────────────────")
		fmt.Printf("%d/%d gates passed\n", report.Passed, report.TotalChecks)
	}

	if report.Errors > 0 {
		return ErrValidationFailed
	}
	return nil
}
