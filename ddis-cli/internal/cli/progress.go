package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/progress"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:interfaces APP-INV-016 (implementation traceability)

var (
	progressDone    string
	progressJSON    bool
	progressWitness bool
)

var progressCmd = &cobra.Command{
	Use:   "progress [db-path]",
	Short: "Show what's done, ready (frontier), and blocked",
	Long: `Analyzes the invariant dependency graph to partition work into three
categories: done, frontier (ready to work on), and blocked (waiting on
dependencies). Accepts invariant IDs or domain names via --done to mark
completed work.

Examples:
  ddis progress index.db
  ddis progress index.db --done APP-INV-001,APP-INV-002
  ddis progress index.db --done parsing --json
  ddis progress index.db --done parsing,validation`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runProgress,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	progressCmd.Flags().StringVar(&progressDone, "done", "", "Comma-separated invariant IDs or domain names to mark as done")
	progressCmd.Flags().BoolVar(&progressJSON, "json", false, "Output as JSON")
	progressCmd.Flags().BoolVar(&progressWitness, "witness", true, "Use persistent witnesses for done set")
}

func runProgress(cmd *cobra.Command, args []string) error {
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

	db, err := storage.OpenExisting(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	opts := progress.Options{Done: progressDone, AsJSON: progressJSON, UseWitnesses: progressWitness}
	result, err := progress.Analyze(db, specID, opts)
	if err != nil {
		return err
	}

	out, err := progress.Render(result, progressJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)

	if !NoGuidance && !progressJSON {
		fmt.Println("\nNext: ddis context <first-ready-element>")
		fmt.Println("  Investigate the first ready-to-implement element.")
	}

	return nil
}
