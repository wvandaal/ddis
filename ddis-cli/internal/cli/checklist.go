package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/checklist"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:interfaces APP-INV-002 (validation determinism)

var (
	checklistSection   string
	checklistInvariant string
	checklistJSON      bool
)

var checklistCmd = &cobra.Command{
	Use:   "checklist [db-path]",
	Short: "Generate verification checklist from invariants",
	Long: `Extracts validation methods from invariants and produces an actionable
checklist grouped by section. Transforms "how do I verify this?" into
copy-paste verification steps.

Examples:
  ddis checklist index.db
  ddis checklist index.db --json
  ddis checklist index.db --section "§1"
  ddis checklist index.db --invariant APP-INV-001`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runChecklist,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	checklistCmd.Flags().StringVar(&checklistSection, "section", "", "Filter by section path prefix")
	checklistCmd.Flags().StringVar(&checklistInvariant, "invariant", "", "Filter by invariant ID")
	checklistCmd.Flags().BoolVar(&checklistJSON, "json", false, "Output as JSON")
}

func runChecklist(cmd *cobra.Command, args []string) error {
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
		return fmt.Errorf("no spec found in database: %w", err)
	}

	opts := checklist.Options{
		Section:   checklistSection,
		Invariant: checklistInvariant,
		AsJSON:    checklistJSON,
	}

	result, err := checklist.Analyze(db, specID, opts)
	if err != nil {
		return err
	}

	out, err := checklist.Render(result, checklistJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)
	return nil
}
