package cli

import (
	"errors"
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/validator"
)

var (
	validateJSON   bool
	validateChecks string
)

// ErrValidationFailed is returned when validation finds errors.
// The caller should exit with code 1.
var ErrValidationFailed = errors.New("validation failed")

var validateCmd = &cobra.Command{
	Use:   "validate <index.db>",
	Short: "Run mechanical validation checks against the spec index",
	Long: `Runs validation checks against a parsed DDIS spec index.
Checks include cross-reference integrity, invariant structure,
glossary completeness, structural conformance, and more.

Examples:
  ddis validate index.db
  ddis validate index.db --json
  ddis validate index.db --checks 1,2,3,9`,
	Args:          cobra.ExactArgs(1),
	RunE:          runValidate,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	validateCmd.Flags().BoolVar(&validateJSON, "json", false, "Output as JSON (for RALPH integration)")
	validateCmd.Flags().StringVar(&validateChecks, "checks", "", "Comma-separated list of check IDs to run (default: all)")
}

func runValidate(cmd *cobra.Command, args []string) error {
	dbPath := args[0]

	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found in database: %w", err)
	}

	checkIDs, err := validator.ParseCheckIDs(validateChecks)
	if err != nil {
		return err
	}

	opts := validator.ValidateOptions{
		CheckIDs: checkIDs,
	}

	report, err := validator.Validate(db, specID, opts)
	if err != nil {
		return err
	}

	out, err := validator.RenderReport(report, validateJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)

	if report.Errors > 0 {
		return ErrValidationFailed
	}

	return nil
}
