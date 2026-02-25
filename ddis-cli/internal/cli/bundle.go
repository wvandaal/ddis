package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/bundle"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-014 (glossary expansion bound)
// ddis:interfaces APP-INV-005 (context self-containment)

var (
	bundleJSON        bool
	bundleContentOnly bool
)

var bundleCmd = &cobra.Command{
	Use:   "bundle <domain> [db-path]",
	Short: "Assemble domain context bundle (constitution + modules + interface stubs)",
	Long: `Assembles all context needed for working in a specific domain: the system
constitution, domain modules, and interface invariant stubs from adjacent domains.
This is the "pullback" construction from the three-tier model.

Examples:
  ddis bundle parsing index.db
  ddis bundle parsing index.db --json
  ddis bundle parsing index.db --content-only`,
	Args:          cobra.RangeArgs(1, 2),
	RunE:          runBundle,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	bundleCmd.Flags().BoolVar(&bundleJSON, "json", false, "Output as JSON")
	bundleCmd.Flags().BoolVar(&bundleContentOnly, "content-only", false, "Output raw content only (for LLM piping)")
}

func runBundle(cmd *cobra.Command, args []string) error {
	domain := args[0]
	var dbPath string
	if len(args) >= 2 {
		dbPath = args[1]
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

	opts := bundle.Options{ContentOnly: bundleContentOnly, AsJSON: bundleJSON}
	result, err := bundle.Assemble(db, specID, domain, opts)
	if err != nil {
		return err
	}

	out, err := bundle.Render(result, bundleJSON, bundleContentOnly)
	if err != nil {
		return err
	}
	fmt.Print(out)
	return nil
}
