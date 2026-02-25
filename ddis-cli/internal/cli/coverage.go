package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/coverage"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:interfaces APP-INV-003 (cross-reference integrity)

var (
	coverageDomain string
	coverageModule string
	coverageJSON   bool
)

var coverageCmd = &cobra.Command{
	Use:   "coverage [db-path]",
	Short: "Show spec completeness dashboard",
	Long: `Analyzes how well each invariant and ADR in a spec has all its required
components filled in with quality content. Shows overall coverage score,
per-domain breakdown, and identifies specific gaps.

Examples:
  ddis coverage index.db
  ddis coverage index.db --json
  ddis coverage index.db --domain search
  ddis coverage index.db --module query-validation`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runCoverage,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	coverageCmd.Flags().StringVar(&coverageDomain, "domain", "", "Filter by domain")
	coverageCmd.Flags().StringVar(&coverageModule, "module", "", "Filter by module")
	coverageCmd.Flags().BoolVar(&coverageJSON, "json", false, "Output as JSON")
}

func runCoverage(cmd *cobra.Command, args []string) error {
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

	opts := coverage.Options{
		Domain: coverageDomain,
		Module: coverageModule,
		AsJSON: coverageJSON,
	}

	result, err := coverage.Analyze(db, specID, opts)
	if err != nil {
		return err
	}

	out, err := coverage.Render(result, coverageJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)
	return nil
}
