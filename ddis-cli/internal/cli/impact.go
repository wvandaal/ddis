package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/impact"
	"github.com/wvandaal/ddis/internal/storage"
)

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

Examples:
  ddis impact INV-006 index.db
  ddis impact §4.2 index.db --direction backward
  ddis impact ADR-003 index.db --depth 3 --json`,
	Args:          cobra.ExactArgs(2),
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
	target, dbPath := args[0], args[1]

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

	return nil
}
