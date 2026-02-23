package tests

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/oplog"
	"github.com/wvandaal/ddis/internal/validator"
)

func TestOplogAppend(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "test-oplog.jsonl")

	// Append a transaction record
	rec, err := oplog.NewTxRecord("tx-001", &oplog.TxData{
		Action:      oplog.TxActionBegin,
		Description: "Test transaction",
	})
	if err != nil {
		t.Fatalf("create record: %v", err)
	}

	if err := oplog.Append(path, rec); err != nil {
		t.Fatalf("append: %v", err)
	}

	// Read it back
	records, err := oplog.ReadAll(path)
	if err != nil {
		t.Fatalf("read all: %v", err)
	}
	if len(records) != 1 {
		t.Fatalf("got %d records, want 1", len(records))
	}
	if records[0].Type != oplog.RecordTypeTransaction {
		t.Errorf("type = %s, want transaction", records[0].Type)
	}
	if records[0].TxID != "tx-001" {
		t.Errorf("tx_id = %s, want tx-001", records[0].TxID)
	}

	// Decode the tx data
	td, err := records[0].DecodeTx()
	if err != nil {
		t.Fatalf("decode tx: %v", err)
	}
	if td.Action != oplog.TxActionBegin {
		t.Errorf("action = %s, want begin", td.Action)
	}
	if td.Description != "Test transaction" {
		t.Errorf("description = %q, want %q", td.Description, "Test transaction")
	}
}

func TestOplogFilter(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "test-oplog.jsonl")

	// Append multiple records of different types
	txRec, _ := oplog.NewTxRecord("tx-001", &oplog.TxData{Action: oplog.TxActionBegin, Description: "tx1"})
	valRec, _ := oplog.NewValidateRecord("tx-001", &oplog.ValidateData{
		SpecPath: "test.md", TotalChecks: 5, Passed: 4, Failed: 1,
	})
	txRec2, _ := oplog.NewTxRecord("tx-002", &oplog.TxData{Action: oplog.TxActionBegin, Description: "tx2"})

	if err := oplog.Append(path, txRec, valRec, txRec2); err != nil {
		t.Fatalf("append: %v", err)
	}

	// Filter by type
	records, err := oplog.ReadFiltered(path, oplog.FilterOpts{
		Types: []oplog.RecordType{oplog.RecordTypeValidate},
	})
	if err != nil {
		t.Fatalf("read filtered: %v", err)
	}
	if len(records) != 1 {
		t.Fatalf("got %d validate records, want 1", len(records))
	}

	// Filter by tx_id
	records, err = oplog.ReadFiltered(path, oplog.FilterOpts{TxID: "tx-001"})
	if err != nil {
		t.Fatalf("read filtered: %v", err)
	}
	if len(records) != 2 {
		t.Errorf("got %d records for tx-001, want 2", len(records))
	}

	// Filter with limit
	records, err = oplog.ReadFiltered(path, oplog.FilterOpts{Limit: 2})
	if err != nil {
		t.Fatalf("read filtered: %v", err)
	}
	if len(records) != 2 {
		t.Errorf("got %d records with limit 2, want 2", len(records))
	}
}

func TestOplogDiffRecord(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "test-oplog.jsonl")

	diffData := &oplog.DiffData{
		Base:    oplog.SpecRef{SpecPath: "v0.md", ContentHash: "sha256:abc"},
		Head:    oplog.SpecRef{SpecPath: "v1.md", ContentHash: "sha256:def"},
		Summary: oplog.DiffSummary{Added: 3, Removed: 1, Modified: 2, Unchanged: 100},
		Changes: []oplog.Change{
			{ElementType: "invariant", ElementID: "INV-021", Action: "added"},
			{ElementType: "section", ElementID: "§4.2", Action: "modified"},
		},
	}

	rec, err := oplog.NewDiffRecord("tx-diff", diffData)
	if err != nil {
		t.Fatalf("create diff record: %v", err)
	}
	if err := oplog.Append(path, rec); err != nil {
		t.Fatalf("append: %v", err)
	}

	records, err := oplog.ReadAll(path)
	if err != nil {
		t.Fatalf("read: %v", err)
	}
	if len(records) != 1 {
		t.Fatalf("got %d records, want 1", len(records))
	}

	dd, err := records[0].DecodeDiff()
	if err != nil {
		t.Fatalf("decode diff: %v", err)
	}
	if dd.Summary.Added != 3 {
		t.Errorf("added = %d, want 3", dd.Summary.Added)
	}
	if len(dd.Changes) != 2 {
		t.Errorf("changes = %d, want 2", len(dd.Changes))
	}
	if dd.Base.SpecPath != "v0.md" {
		t.Errorf("base path = %s, want v0.md", dd.Base.SpecPath)
	}
}

func TestOplogValidateRecord(t *testing.T) {
	// Create a mock validator.Report
	report := &validator.Report{
		SpecPath:    "test.md",
		SourceType:  "monolith",
		TotalChecks: 8,
		Passed:      7,
		Failed:      1,
		Errors:      1,
		Warnings:    2,
		Results: []validator.CheckResult{
			{CheckID: 1, CheckName: "XRef Integrity", Passed: true, Summary: "All resolved"},
			{CheckID: 2, CheckName: "INV-003", Passed: false, Summary: "Missing components"},
		},
	}

	vd := oplog.ImportValidation(report, "test.md", "sha256:abc")
	if vd.TotalChecks != 8 {
		t.Errorf("total_checks = %d, want 8", vd.TotalChecks)
	}
	if vd.Passed != 7 {
		t.Errorf("passed = %d, want 7", vd.Passed)
	}
	if len(vd.Results) != 2 {
		t.Errorf("results = %d, want 2", len(vd.Results))
	}
	if vd.Results[0].CheckName != "XRef Integrity" {
		t.Errorf("results[0].check_name = %s, want XRef Integrity", vd.Results[0].CheckName)
	}

	// Write and read back
	dir := t.TempDir()
	path := filepath.Join(dir, "test-oplog.jsonl")

	rec, _ := oplog.NewValidateRecord("tx-val", vd)
	if err := oplog.Append(path, rec); err != nil {
		t.Fatalf("append: %v", err)
	}

	records, _ := oplog.ReadAll(path)
	decoded, err := records[0].DecodeValidate()
	if err != nil {
		t.Fatalf("decode: %v", err)
	}
	if decoded.Errors != 1 {
		t.Errorf("decoded errors = %d, want 1", decoded.Errors)
	}
}

func TestOplogTxRecord(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "test-oplog.jsonl")

	// Write begin, commit, rollback lifecycle
	begin, _ := oplog.NewTxRecord("tx-life", &oplog.TxData{
		Action:      oplog.TxActionBegin,
		Description: "Lifecycle test",
	})
	commit, _ := oplog.NewTxRecord("tx-life", &oplog.TxData{
		Action: oplog.TxActionCommit,
	})
	rollback, _ := oplog.NewTxRecord("tx-rb", &oplog.TxData{
		Action:      oplog.TxActionRollback,
		Description: "Something went wrong",
	})

	if err := oplog.Append(path, begin, commit, rollback); err != nil {
		t.Fatalf("append: %v", err)
	}

	records, _ := oplog.ReadAll(path)
	if len(records) != 3 {
		t.Fatalf("got %d records, want 3", len(records))
	}

	// Verify lifecycle events
	for _, rec := range records {
		if rec.Type != oplog.RecordTypeTransaction {
			t.Errorf("unexpected type %s", rec.Type)
		}
	}

	td1, _ := records[0].DecodeTx()
	if td1.Action != oplog.TxActionBegin {
		t.Errorf("record 0 action = %s, want begin", td1.Action)
	}
	td2, _ := records[1].DecodeTx()
	if td2.Action != oplog.TxActionCommit {
		t.Errorf("record 1 action = %s, want commit", td2.Action)
	}
	td3, _ := records[2].DecodeTx()
	if td3.Action != oplog.TxActionRollback {
		t.Errorf("record 2 action = %s, want rollback", td3.Action)
	}
}

func TestOplogEmpty(t *testing.T) {
	// Non-existent file returns empty slice, no error
	records, err := oplog.ReadAll("/tmp/nonexistent-oplog-" + t.Name() + ".jsonl")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if records != nil {
		t.Errorf("got %d records, want nil", len(records))
	}

	// Empty file returns empty slice
	dir := t.TempDir()
	emptyPath := filepath.Join(dir, "empty.jsonl")
	if err := os.WriteFile(emptyPath, []byte(""), 0o644); err != nil {
		t.Fatal(err)
	}
	records, err = oplog.ReadAll(emptyPath)
	if err != nil {
		t.Fatalf("unexpected error on empty file: %v", err)
	}
	if len(records) != 0 {
		t.Errorf("got %d records from empty file, want 0", len(records))
	}
}

func TestOplogRenderJSON(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "test-oplog.jsonl")

	rec, _ := oplog.NewTxRecord("tx-json", &oplog.TxData{Action: oplog.TxActionBegin, Description: "JSON test"})
	if err := oplog.Append(path, rec); err != nil {
		t.Fatalf("append: %v", err)
	}

	records, _ := oplog.ReadAll(path)
	out, err := oplog.RenderLog(records, true)
	if err != nil {
		t.Fatalf("render JSON: %v", err)
	}

	// Verify valid JSON
	var parsed []*oplog.Record
	if err := json.Unmarshal([]byte(out), &parsed); err != nil {
		t.Fatalf("parse JSON output: %v", err)
	}
	if len(parsed) != 1 {
		t.Errorf("parsed %d records, want 1", len(parsed))
	}
}
