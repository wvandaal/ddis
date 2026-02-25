package cli

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/oplog"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-006 (transaction state machine)

var txOplogPath string

var txCmd = &cobra.Command{
	Use:   "tx <action> [args...]",
	Short: "Transaction lifecycle management",
	Long: `Manage spec modification transactions.

Actions:
  begin <index.db> "description"   Start a new transaction
  commit <tx_id> <index.db>        Commit and flush to oplog
  rollback <tx_id> <index.db>      Rollback transaction
  list <index.db>                  List all transactions
  show <tx_id> <index.db>          Show transaction details`,
	Args:          cobra.MinimumNArgs(1),
	RunE:          runTx,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	txCmd.Flags().StringVar(&txOplogPath, "oplog", "", "Custom oplog path (default: .ddis/oplog.jsonl)")
}

func runTx(cmd *cobra.Command, args []string) error {
	action := args[0]

	switch action {
	case "begin":
		return txBegin(cmd, args[1:])
	case "commit":
		return txCommit(cmd, args[1:])
	case "rollback":
		return txRollback(cmd, args[1:])
	case "list":
		return txList(cmd, args[1:])
	case "show":
		return txShow(cmd, args[1:])
	default:
		return fmt.Errorf("unknown tx action %q: expected begin, commit, rollback, list, or show", action)
	}
}

func txBegin(cmd *cobra.Command, args []string) error {
	if len(args) < 2 {
		return fmt.Errorf("usage: ddis tx begin <index.db> \"description\"")
	}
	dbPath := args[0]
	description := strings.Join(args[1:], " ")

	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec in database: %w", err)
	}

	txID := generateTxID()
	if err := storage.CreateTransaction(db, specID, txID, description); err != nil {
		return err
	}

	// Write begin record to oplog
	oplogPath := resolveOplogPath(txOplogPath)
	rec, err := oplog.NewTxRecord(txID, &oplog.TxData{
		Action:      oplog.TxActionBegin,
		Description: description,
	})
	if err != nil {
		return err
	}
	if err := oplog.Append(oplogPath, rec); err != nil {
		return fmt.Errorf("append to oplog: %w", err)
	}

	fmt.Fprintf(cmd.OutOrStdout(), "%s\n", txID)
	return nil
}

func txCommit(cmd *cobra.Command, args []string) error {
	if len(args) < 2 {
		return fmt.Errorf("usage: ddis tx commit <tx_id> <index.db>")
	}
	txID, dbPath := args[0], args[1]

	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	if err := storage.CommitTransaction(db, txID); err != nil {
		return err
	}

	// Write commit record to oplog
	oplogPath := resolveOplogPath(txOplogPath)
	rec, err := oplog.NewTxRecord(txID, &oplog.TxData{
		Action: oplog.TxActionCommit,
	})
	if err != nil {
		return err
	}
	if err := oplog.Append(oplogPath, rec); err != nil {
		return fmt.Errorf("append to oplog: %w", err)
	}

	fmt.Fprintf(cmd.OutOrStdout(), "Transaction %s committed\n", txID)
	return nil
}

func txRollback(cmd *cobra.Command, args []string) error {
	if len(args) < 2 {
		return fmt.Errorf("usage: ddis tx rollback <tx_id> <index.db>")
	}
	txID, dbPath := args[0], args[1]

	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	if err := storage.RollbackTransaction(db, txID); err != nil {
		return err
	}

	// Write rollback record to oplog
	oplogPath := resolveOplogPath(txOplogPath)
	rec, err := oplog.NewTxRecord(txID, &oplog.TxData{
		Action: oplog.TxActionRollback,
	})
	if err != nil {
		return err
	}
	if err := oplog.Append(oplogPath, rec); err != nil {
		return fmt.Errorf("append to oplog: %w", err)
	}

	fmt.Fprintf(cmd.OutOrStdout(), "Transaction %s rolled back\n", txID)
	return nil
}

func txList(cmd *cobra.Command, args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("usage: ddis tx list <index.db>")
	}
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

	txns, err := storage.ListTransactions(db, specID)
	if err != nil {
		return err
	}

	if len(txns) == 0 {
		fmt.Fprintln(cmd.OutOrStdout(), "No transactions found.")
		return nil
	}

	var b strings.Builder
	fmt.Fprintf(&b, "Transactions (%d)\n", len(txns))
	b.WriteString("═══════════════════════════════════════════\n\n")
	for _, tx := range txns {
		fmt.Fprintf(&b, "  %-20s [%s] %s\n", tx.TxID, tx.Status, tx.Description)
		fmt.Fprintf(&b, "    created: %s", tx.CreatedAt)
		if tx.CommittedAt != nil {
			fmt.Fprintf(&b, "  completed: %s", *tx.CommittedAt)
		}
		b.WriteString("\n\n")
	}
	fmt.Fprint(cmd.OutOrStdout(), b.String())
	return nil
}

func txShow(cmd *cobra.Command, args []string) error {
	if len(args) < 2 {
		return fmt.Errorf("usage: ddis tx show <tx_id> <index.db>")
	}
	txID, dbPath := args[0], args[1]

	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	tx, err := storage.GetTransaction(db, txID)
	if err != nil {
		return err
	}

	ops, err := storage.GetTxOperations(db, txID)
	if err != nil {
		return err
	}

	// JSON output
	out := struct {
		Transaction *storage.Transaction  `json:"transaction"`
		Operations  []storage.TxOperation `json:"operations"`
	}{tx, ops}

	data, err := json.MarshalIndent(out, "", "  ")
	if err != nil {
		return err
	}
	fmt.Fprintln(cmd.OutOrStdout(), string(data))
	return nil
}

func generateTxID() string {
	b := make([]byte, 8)
	if _, err := rand.Read(b); err != nil {
		// Fallback — shouldn't happen
		return "tx-fallback"
	}
	return "tx-" + hex.EncodeToString(b)
}

func resolveOplogPath(custom string) string {
	if custom != "" {
		return custom
	}
	return oplog.DefaultPath(".")
}
