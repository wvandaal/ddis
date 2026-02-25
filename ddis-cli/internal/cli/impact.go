package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/impact"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-013 (impact termination)

var (
	impactJSON      bool
	impactDirection string
	impactDepth     int
)

var impactCmd = &cobra.Command{
	Use:   "impact <target> <index.db>",
	Short: "Analyze forward/backward impact of a spec element",
	Long: `Performs BFS traversal over the cross-reference graph to find
elements connected to the given target.

Forward impact: "I'm changing X, what else is affected?"
Backward trace: "Why does X say this? What does it depend on?"

If no database path is given, auto-discovers a *.ddis.db file in the current directory.

Examples:
  ddis impact APP-INV-006 forward
  ddis impact §4.2 backward --direction backward
  ddis impact ADR-003 forward index.db --depth 3 --json`,
	Args:          cobra.RangeArgs(1, 2),
	RunE:          runImpact,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	impactCmd.Flags().BoolVar(&impactJSON, "json", false, "Output as JSON")
	impactCmd.Flags().StringVar(&impactDirection, "direction", "forward", "Direction: forward, backward, both")
	impactCmd.Flags().IntVar(&impactDepth, "depth", 2, "Maximum traversal depth (1-5)")
}

func runImpact(cmd *cobra.Command, args []string) error {
	target := args[0]
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
		return fmt.Errorf("no spec found in database: %w", err)
	}

	opts := impact.ImpactOptions{
		MaxDepth:  impactDepth,
		Direction: impactDirection,
	}

	result, err := impact.Analyze(db, specID, target, opts)
	if err != nil {
		return err
	}

	out, err := impact.RenderImpact(result, impactJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)

	if !NoGuidance && !impactJSON {
		fmt.Println("\nNext: ddis context <top-impacted-element>")
		fmt.Println("  Investigate the most impacted element.")
	}

	return nil
}
