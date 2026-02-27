package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/discovery"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-039 (task derivation completeness)
// ddis:maintains APP-INV-052 (challenge-driven task derivation)
// ddis:implements APP-ADR-029 (beads-compatible task output)
// ddis:implements APP-ADR-041 (challenge-feedback loop — task derivation)

var (
	tasksDiscovery     string
	tasksSpec          string
	tasksFormat        string
	tasksFromChallenges bool
)

var tasksCmd = &cobra.Command{
	Use:   "tasks",
	Short: "Generate implementation tasks from discovery artifacts or challenge verdicts",
	Long: `Reads a discovery JSONL stream (or challenge results from the spec DB),
reduces to an artifact map, and mechanically derives implementation tasks.

Discovery derivation uses 8 rules (Rules 1-8).
Challenge derivation uses 2 additional rules:
  Rule  9: Provisional invariants → upgrade tasks (write test, add annotations)
  Rule 10: Refuted invariants → remediation tasks (fix impl or amend spec)

Output formats:
  beads    - JSONL compatible with br import (default)
  json     - JSON array
  markdown - Human-readable checklist

Examples:
  ddis tasks --from-discovery session.jsonl
  ddis tasks --from-challenges --spec index.db
  ddis tasks --from-discovery session.jsonl --spec index.db
  ddis tasks --from-discovery session.jsonl --format markdown`,
	RunE:          runTasks,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	tasksCmd.Flags().StringVar(&tasksDiscovery, "from-discovery", "", "Path to discovery JSONL file")
	tasksCmd.Flags().BoolVar(&tasksFromChallenges, "from-challenges", false, "Derive tasks from challenge verdicts")
	tasksCmd.Flags().StringVar(&tasksSpec, "spec", "", "Path to spec database for cross-validation")
	tasksCmd.Flags().StringVar(&tasksFormat, "format", "beads", "Output format: beads, json, markdown")
}

func runTasks(cmd *cobra.Command, args []string) error {
	if tasksDiscovery == "" && !tasksFromChallenges {
		return fmt.Errorf("either --from-discovery or --from-challenges is required")
	}

	var result *discovery.TasksResult

	// Challenge-derived tasks
	if tasksFromChallenges {
		specPath := tasksSpec
		if specPath == "" {
			found, err := FindDB()
			if err != nil {
				return fmt.Errorf("no spec database: specify --spec or ensure manifest.ddis.db exists")
			}
			specPath = found
		}
		db, err := storage.OpenExisting(specPath)
		if err != nil {
			return fmt.Errorf("open spec database: %w", err)
		}
		defer db.Close()

		specID, err := storage.GetFirstSpecID(db)
		if err != nil {
			return fmt.Errorf("no spec found: %w", err)
		}

		challengeResult, err := discovery.DeriveFromChallenges(db, specID)
		if err != nil {
			return fmt.Errorf("derive from challenges: %w", err)
		}
		result = challengeResult
	}

	// Discovery-derived tasks
	if tasksDiscovery != "" {
		state, err := discovery.ReduceToState(tasksDiscovery)
		if err != nil {
			return fmt.Errorf("reduce discovery: %w", err)
		}

		discoveryResult, err := discovery.DeriveTasks(state, nil)
		if err != nil {
			return fmt.Errorf("derive tasks: %w", err)
		}

		if result != nil {
			// Merge: challenge tasks first (higher priority)
			result.Tasks = append(result.Tasks, discoveryResult.Tasks...)
			result.TotalTasks = len(result.Tasks)
			for rule, count := range discoveryResult.ByRule {
				result.ByRule[rule] += count
			}
		} else {
			result = discoveryResult
		}
	}

	// Cross-validate against spec if provided
	if tasksSpec != "" && tasksDiscovery != "" {
		db, err := storage.OpenExisting(tasksSpec)
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

	// Output in requested format
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
