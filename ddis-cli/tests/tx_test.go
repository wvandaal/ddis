package tests

import (
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/oplog"
	"github.com/wvandaal/ddis/internal/storage"
)

func TestTxBeginCommit(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	// Begin
	txID := "tx-test-begin-commit"
	if err := storage.CreateTransaction(db, specID, txID, "Test begin/commit"); err != nil {
		t.Fatalf("create: %v", err)
	}

	// Verify pending
	tx, err := storage.GetTransaction(db, txID)
	if err != nil {
		t.Fatalf("get: %v", err)
	}
	if tx.Status != "pending" {
		t.Errorf("status = %s, want pending", tx.Status)
	}
	if tx.Description != "Test begin/commit" {
		t.Errorf("description = %q", tx.Description)
	}

	// Commit
	if err := storage.CommitTransaction(db, txID); err != nil {
		t.Fatalf("commit: %v", err)
	}

	tx, _ = storage.GetTransaction(db, txID)
	if tx.Status != "committed" {
		t.Errorf("status = %s, want committed", tx.Status)
	}
	if tx.CommittedAt == nil {
		t.Error("committed_at is nil after commit")
	}
}

func TestTxRollback(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	txID := "tx-test-rollback"
	if err := storage.CreateTransaction(db, specID, txID, "Test rollback"); err != nil {
		t.Fatalf("create: %v", err)
	}

	// Rollback
	if err := storage.RollbackTransaction(db, txID); err != nil {
		t.Fatalf("rollback: %v", err)
	}

	tx, _ := storage.GetTransaction(db, txID)
	if tx.Status != "rolled_back" {
		t.Errorf("status = %s, want rolled_back", tx.Status)
	}

	// Double commit should fail
	err := storage.CommitTransaction(db, txID)
	if err == nil {
		t.Error("expected error on commit after rollback")
	}
}

func TestTxOperations(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	txID := "tx-test-ops"
	if err := storage.CreateTransaction(db, specID, txID, "Test operations"); err != nil {
		t.Fatalf("create: %v", err)
	}

	// Add operations
	if err := storage.AddTxOperation(db, txID, 1, "create", `{"type":"invariant","id":"INV-999"}`, ""); err != nil {
		t.Fatalf("add op 1: %v", err)
	}
	if err := storage.AddTxOperation(db, txID, 2, "update", `{"type":"section","id":"§2.1"}`, `["INV-001","§2.1"]`); err != nil {
		t.Fatalf("add op 2: %v", err)
	}

	ops, err := storage.GetTxOperations(db, txID)
	if err != nil {
		t.Fatalf("get ops: %v", err)
	}
	if len(ops) != 2 {
		t.Fatalf("got %d ops, want 2", len(ops))
	}

	// Verify ordering
	if ops[0].Ordinal != 1 || ops[1].Ordinal != 2 {
		t.Errorf("ordinals: %d, %d, want 1, 2", ops[0].Ordinal, ops[1].Ordinal)
	}
	if ops[0].OperationType != "create" {
		t.Errorf("op 0 type = %s, want create", ops[0].OperationType)
	}
	if ops[1].ImpactSet == nil {
		t.Error("op 1 impact_set is nil, expected value")
	}
}

func TestTxList(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	// Create multiple transactions
	for i, desc := range []string{"First tx", "Second tx", "Third tx"} {
		txID := "tx-list-" + string(rune('a'+i))
		if err := storage.CreateTransaction(db, specID, txID, desc); err != nil {
			t.Fatalf("create %s: %v", txID, err)
		}
	}

	txns, err := storage.ListTransactions(db, specID)
	if err != nil {
		t.Fatalf("list: %v", err)
	}
	if len(txns) != 3 {
		t.Errorf("got %d transactions, want 3", len(txns))
	}

	// All should be pending
	for _, tx := range txns {
		if tx.Status != "pending" {
			t.Errorf("tx %s status = %s, want pending", tx.TxID, tx.Status)
		}
	}
}

func TestTxFlushToOplog(t *testing.T) {
	db, specID := buildSyntheticDB(t)

	txID := "tx-flush-test"
	oplogPath := filepath.Join(t.TempDir(), "flush-oplog.jsonl")

	// Begin
	if err := storage.CreateTransaction(db, specID, txID, "Flush test"); err != nil {
		t.Fatalf("create: %v", err)
	}

	// Write begin record
	beginRec, _ := oplog.NewTxRecord(txID, &oplog.TxData{
		Action:      oplog.TxActionBegin,
		Description: "Flush test",
	})
	if err := oplog.Append(oplogPath, beginRec); err != nil {
		t.Fatalf("append begin: %v", err)
	}

	// Commit in DB
	if err := storage.CommitTransaction(db, txID); err != nil {
		t.Fatalf("commit: %v", err)
	}

	// Write commit record
	commitRec, _ := oplog.NewTxRecord(txID, &oplog.TxData{Action: oplog.TxActionCommit})
	if err := oplog.Append(oplogPath, commitRec); err != nil {
		t.Fatalf("append commit: %v", err)
	}

	// Verify oplog contains both records
	records, err := oplog.ReadFiltered(oplogPath, oplog.FilterOpts{TxID: txID})
	if err != nil {
		t.Fatalf("read: %v", err)
	}
	if len(records) != 2 {
		t.Fatalf("got %d records, want 2", len(records))
	}

	td1, _ := records[0].DecodeTx()
	td2, _ := records[1].DecodeTx()
	if td1.Action != oplog.TxActionBegin {
		t.Errorf("record 0 action = %s, want begin", td1.Action)
	}
	if td2.Action != oplog.TxActionCommit {
		t.Errorf("record 1 action = %s, want commit", td2.Action)
	}
}
