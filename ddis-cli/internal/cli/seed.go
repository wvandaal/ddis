package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/oplog"
	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/validator"
)

// ddis:maintains APP-INV-010 (oplog append-only)

var seedOplogPath string

var seedCmd = &cobra.Command{
	Use:   "seed <index.db>",
	Short: "Seed the oplog with a baseline record for an existing spec",
	Long: `Creates a "genesis" transaction in the oplog with the current validation state.
This captures the epoch state so future diffs have a baseline.

Idempotent — skips if the oplog already contains a genesis transaction.

Examples:
  ddis seed index.db
  ddis seed index.db --oplog-path .ddis/oplog.jsonl`,
	Args:          cobra.ExactArgs(1),
	RunE:          runSeed,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	seedCmd.Flags().StringVar(&seedOplogPath, "oplog-path", "", "Custom oplog path (default: .ddis/oplog.jsonl)")
}

func runSeed(cmd *cobra.Command, args []string) error {
	dbPath := args[0]

	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec in database: %w", err)
	}

	oplogPath := seedOplogPath
	if oplogPath == "" {
		oplogPath = oplog.DefaultPath(".")
	}

	// Idempotency check
	hasGenesis, err := oplog.HasGenesisTransaction(oplogPath)
	if err != nil {
		return fmt.Errorf("check genesis: %w", err)
	}
	if hasGenesis {
		fmt.Fprintln(cmd.OutOrStdout(), "Genesis transaction already exists, skipping.")
		return nil
	}

	spec, err := storage.GetSpecIndex(db, specID)
	if err != nil {
		return fmt.Errorf("get spec: %w", err)
	}

	txID := generateTxID()

	// 1. Begin genesis transaction
	beginRec, err := oplog.NewTxRecord(txID, &oplog.TxData{
		Action:      oplog.TxActionBegin,
		Description: "Genesis: initial spec state",
	})
	if err != nil {
		return err
	}

	// 2. Run validation and import
	report, err := validator.Validate(db, specID, validator.ValidateOptions{})
	if err != nil {
		return fmt.Errorf("validate: %w", err)
	}

	vd := oplog.ImportValidation(report, spec.SpecPath, spec.ContentHash)
	validateRec, err := oplog.NewValidateRecord(txID, vd)
	if err != nil {
		return err
	}

	// 3. Commit genesis transaction
	commitRec, err := oplog.NewTxRecord(txID, &oplog.TxData{
		Action: oplog.TxActionCommit,
	})
	if err != nil {
		return err
	}

	// Append all three records atomically
	if err := oplog.Append(oplogPath, beginRec, validateRec, commitRec); err != nil {
		return fmt.Errorf("append to oplog: %w", err)
	}

	fmt.Fprintf(cmd.OutOrStdout(), "Genesis transaction %s seeded (%d checks: %d passed, %d failed)\n",
		txID, report.TotalChecks, report.Passed, report.Failed)
	return nil
}
