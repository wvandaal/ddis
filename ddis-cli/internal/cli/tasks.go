package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/discovery"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-039 (task derivation completeness)
// ddis:implements APP-ADR-029 (beads-compatible task output)

var (
	tasksDiscovery string
	tasksSpec      string
	tasksFormat    string
)

var tasksCmd = &cobra.Command{
	Use:   "tasks",
	Short: "Generate implementation tasks from discovery artifacts",
	Long: `Reads a discovery JSONL stream, reduces it to an artifact map,
and mechanically derives implementation tasks using 8 deterministic rules.

Output formats:
  beads    - JSONL compatible with br import (default)
  json     - JSON array
  markdown - Human-readable checklist

Examples:
  ddis tasks --from-discovery session.jsonl
  ddis tasks --from-discovery session.jsonl --spec index.db
  ddis tasks --from-discovery session.jsonl --format markdown
  ddis tasks --from-discovery session.jsonl --format json`,
	RunE:          runTasks,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	tasksCmd.Flags().StringVar(&tasksDiscovery, "from-discovery", "", "Path to discovery JSONL file (required)")
	tasksCmd.Flags().StringVar(&tasksSpec, "spec", "", "Path to spec database for cross-validation")
	tasksCmd.Flags().StringVar(&tasksFormat, "format", "beads", "Output format: beads, json, markdown")
	tasksCmd.MarkFlagRequired("from-discovery")
}

func runTasks(cmd *cobra.Command, args []string) error {
	// 1. Reduce discovery JSONL to state
	state, err := discovery.ReduceToState(tasksDiscovery)
	if err != nil {
		return fmt.Errorf("reduce discovery: %w", err)
	}

	// 2. Derive tasks from artifact map
	result, err := discovery.DeriveTasks(state, nil)
	if err != nil {
		return fmt.Errorf("derive tasks: %w", err)
	}

	// 3. Cross-validate against spec if provided
	if tasksSpec != "" {
		db, err := storage.Open(tasksSpec)
		if err != nil {
			return fmt.Errorf("open spec database: %w", err)
		}
		defer db.Close()

		specID, err := storage.GetFirstSpecID(db)
		if err != nil {
			return fmt.Errorf("no spec found: %w", err)
		}

		if err := discovery.CrossValidate(result, db, specID); err != nil {
			return fmt.Errorf("cross-validate: %w", err)
		}
	}

	// 4. Output in requested format
	switch tasksFormat {
	case "json":
		out, err := discovery.FormatJSON(result)
		if err != nil {
			return err
		}
		fmt.Println(out)
	case "markdown":
		fmt.Print(discovery.FormatMarkdown(result))
	default: // "beads"
		fmt.Print(discovery.FormatBeads(result))
	}

	return nil
}
