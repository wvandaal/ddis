package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/implorder"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:interfaces APP-INV-004 (authority monotonicity)

var (
	implorderDomain string
	implorderJSON   bool
)

var implorderCmd = &cobra.Command{
	Use:   "impl-order [db-path]",
	Short: "Compute optimal implementation order using topological sort",
	Long: `Uses Kahn's algorithm to compute the optimal implementation order for
invariants. Elements are grouped into phases where Phase 0 has no dependencies
and each subsequent phase depends on earlier phases. Within a phase, elements
are sorted by authority score (PageRank) for tie-breaking.

Examples:
  ddis impl-order index.db
  ddis impl-order index.db --json
  ddis impl-order index.db --domain parsing`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runImplOrder,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	implorderCmd.Flags().StringVar(&implorderDomain, "domain", "", "Filter by domain")
	implorderCmd.Flags().BoolVar(&implorderJSON, "json", false, "Output as JSON")
}

func runImplOrder(cmd *cobra.Command, args []string) error {
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
		return fmt.Errorf("no spec found: %w", err)
	}

	opts := implorder.Options{Domain: implorderDomain, AsJSON: implorderJSON}
	result, err := implorder.Analyze(db, specID, opts)
	if err != nil {
		return err
	}

	out, err := implorder.Render(result, implorderJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)

	if !NoGuidance && !implorderJSON {
		fmt.Println("\nNext: ddis progress")
		fmt.Println("  See implementation status of the ordered elements.")
	}

	return nil
}
