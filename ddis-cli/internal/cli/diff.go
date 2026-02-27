package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/diff"
	"github.com/wvandaal/ddis/internal/oplog"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-007 (diff completeness)
// ddis:interfaces APP-ADR-021 (contributor topology via git blame)
// ddis:interfaces APP-INV-030 (contributor topology graceful degradation)

var (
	diffJSON      bool
	diffLog       bool
	diffTx        string
	diffOplogPath string
)

var diffCmd = &cobra.Command{
	Use:   "diff <base.db> <head.db>",
	Short: "Structural diff between two spec indexes",
	Long: `Compares two parsed DDIS spec indexes and reports structural changes.
Detects added, removed, and modified sections, invariants, ADRs, gates, and glossary entries.

Examples:
  ddis diff base.db head.db
  ddis diff base.db head.db --json
  ddis diff base.db head.db --log
  ddis diff base.db head.db --tx tx-abc123`,
	Args:          cobra.ExactArgs(2),
	RunE:          runDiff,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	diffCmd.Flags().BoolVar(&diffJSON, "json", false, "Output as JSON")
	diffCmd.Flags().BoolVar(&diffLog, "log", false, "Append diff record to oplog")
	diffCmd.Flags().StringVar(&diffTx, "tx", "", "Associate with transaction ID")
	diffCmd.Flags().StringVar(&diffOplogPath, "oplog-path", "", "Custom oplog path (default: .ddis/oplog.jsonl)")
}

func runDiff(cmd *cobra.Command, args []string) error {
	basePath, headPath := args[0], args[1]

	baseDB, err := storage.OpenExisting(basePath)
	if err != nil {
		return fmt.Errorf("open base database: %w", err)
	}
	defer baseDB.Close()

	headDB, err := storage.OpenExisting(headPath)
	if err != nil {
		return fmt.Errorf("open head database: %w", err)
	}
	defer headDB.Close()

	baseSpecID, err := storage.GetFirstSpecID(baseDB)
	if err != nil {
		return fmt.Errorf("no spec in base database: %w", err)
	}
	headSpecID, err := storage.GetFirstSpecID(headDB)
	if err != nil {
		return fmt.Errorf("no spec in head database: %w", err)
	}

	result, err := diff.ComputeDiff(baseDB, headDB, baseSpecID, headSpecID)
	if err != nil {
		return fmt.Errorf("compute diff: %w", err)
	}

	out, err := diff.RenderDiff(result, diffJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)

	if !NoGuidance && !diffJSON {
		fmt.Println("\nNext: ddis impact <changed-element>")
		fmt.Println("  Assess downstream impact of the changes.")
	}

	if diffLog {
		oplogPath := diffOplogPath
		if oplogPath == "" {
			oplogPath = oplog.DefaultPath(".")
		}

		rec, err := oplog.NewDiffRecord(diffTx, result.ToDiffData())
		if err != nil {
			return fmt.Errorf("create diff record: %w", err)
		}
		if err := oplog.Append(oplogPath, rec); err != nil {
			return fmt.Errorf("append to oplog: %w", err)
		}
		fmt.Fprintf(cmd.ErrOrStderr(), "Diff record appended to %s\n", oplogPath)
	}

	return nil
}
