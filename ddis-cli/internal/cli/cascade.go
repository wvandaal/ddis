package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/cascade"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-016 (implementation traceability)

var (
	cascadeDepth int
	cascadeJSON  bool
)

var cascadeCmd = &cobra.Command{
	Use:   "cascade <element-id> [db-path]",
	Short: "Analyze cascade impact of changing an element",
	Long: `Analyzes what modules and domains would be affected if a specific
invariant, ADR, or quality gate changes. Performs a reverse lookup on the
reference graph, grouped by domain.

Examples:
  ddis cascade APP-INV-001 index.db
  ddis cascade APP-INV-001 index.db --json
  ddis cascade APP-INV-001 index.db --depth 3`,
	Args:          cobra.RangeArgs(1, 2),
	RunE:          runCascade,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	cascadeCmd.Flags().IntVar(&cascadeDepth, "depth", 3, "Maximum BFS depth (1-5)")
	cascadeCmd.Flags().BoolVar(&cascadeJSON, "json", false, "Output as JSON")
}

func runCascade(cmd *cobra.Command, args []string) error {
	elementID := args[0]
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

	db, err := storage.OpenExisting(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	opts := cascade.Options{Depth: cascadeDepth, AsJSON: cascadeJSON}
	result, err := cascade.Analyze(db, specID, elementID, opts)
	if err != nil {
		return err
	}

	out, err := cascade.Render(result, cascadeJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)

	if !NoGuidance && !cascadeJSON {
		fmt.Println("\nNext: ddis validate")
		fmt.Println("  Verify structural integrity after cascade analysis.")
	}

	return nil
}
