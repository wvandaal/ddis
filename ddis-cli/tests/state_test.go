package tests

import (
	"database/sql"
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	"github.com/wvandaal/ddis/internal/state"
	"github.com/wvandaal/ddis/internal/storage"
)

// setupStateDB creates a temp DB with schema and a minimal spec_index row.
func setupStateDB(t *testing.T) (*sql.DB, int64) {
	t.Helper()
	dbPath := filepath.Join(t.TempDir(), "state_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	res, err := db.Exec(
		`INSERT INTO spec_index (spec_path, total_lines, content_hash, parsed_at, source_type)
		 VALUES ('test.md', 100, 'abc123', datetime('now'), 'monolith')`)
	if err != nil {
		t.Fatalf("insert spec: %v", err)
	}
	specID, err := res.LastInsertId()
	if err != nil {
		t.Fatalf("last insert id: %v", err)
	}

	t.Cleanup(func() { db.Close() })
	return db, specID
}

// INV-STATE-PERSIST: save(k,v); load(k) = v
func TestINV_STATE_PERSIST(t *testing.T) {
	db, specID := setupStateDB(t)

	// Set and get
	if err := state.Set(db, specID, "author", "claude"); err != nil {
		t.Fatalf("set: %v", err)
	}
	val, err := state.Get(db, specID, "author")
	if err != nil {
		t.Fatalf("get: %v", err)
	}
	if val != "claude" {
		t.Errorf("got %q, want %q", val, "claude")
	}

	// Overwrite
	if err := state.Set(db, specID, "author", "human"); err != nil {
		t.Fatalf("overwrite: %v", err)
	}
	val, err = state.Get(db, specID, "author")
	if err != nil {
		t.Fatalf("get after overwrite: %v", err)
	}
	if val != "human" {
		t.Errorf("got %q after overwrite, want %q", val, "human")
	}

	// List with multiple entries
	if err := state.Set(db, specID, "phase", "7"); err != nil {
		t.Fatalf("set phase: %v", err)
	}
	entries, err := state.List(db, specID)
	if err != nil {
		t.Fatalf("list: %v", err)
	}
	if len(entries) != 2 {
		t.Fatalf("list returned %d entries, want 2", len(entries))
	}
	// Ordered by key: author < phase
	if entries[0].Key != "author" {
		t.Errorf("first entry key = %q, want %q", entries[0].Key, "author")
	}
	if entries[1].Key != "phase" {
		t.Errorf("second entry key = %q, want %q", entries[1].Key, "phase")
	}

	// Delete
	if err := state.Delete(db, specID, "author"); err != nil {
		t.Fatalf("delete: %v", err)
	}
	_, err = state.Get(db, specID, "author")
	if err == nil {
		t.Error("expected error after delete, got nil")
	}

	// Remaining entry
	entries, err = state.List(db, specID)
	if err != nil {
		t.Fatalf("list after delete: %v", err)
	}
	if len(entries) != 1 {
		t.Fatalf("list returned %d entries after delete, want 1", len(entries))
	}
	if entries[0].Key != "phase" {
		t.Errorf("remaining key = %q, want %q", entries[0].Key, "phase")
	}
}

func TestStateGetNotFound(t *testing.T) {
	db, specID := setupStateDB(t)

	_, err := state.Get(db, specID, "nonexistent")
	if err == nil {
		t.Fatal("expected error for nonexistent key")
	}
	if !strings.Contains(err.Error(), "not found") {
		t.Errorf("error = %q, want it to contain 'not found'", err.Error())
	}
}

func TestStateDeleteNotFound(t *testing.T) {
	db, specID := setupStateDB(t)

	err := state.Delete(db, specID, "nonexistent")
	if err == nil {
		t.Fatal("expected error for nonexistent key")
	}
	if !strings.Contains(err.Error(), "not found") {
		t.Errorf("error = %q, want it to contain 'not found'", err.Error())
	}
}

func TestStateListEmpty(t *testing.T) {
	db, specID := setupStateDB(t)

	entries, err := state.List(db, specID)
	if err != nil {
		t.Fatalf("list: %v", err)
	}
	if len(entries) != 0 {
		t.Errorf("got %d entries, want 0", len(entries))
	}
}

func TestStateUpdatedAt(t *testing.T) {
	db, specID := setupStateDB(t)

	if err := state.Set(db, specID, "key1", "val1"); err != nil {
		t.Fatalf("set: %v", err)
	}
	entries, err := state.List(db, specID)
	if err != nil {
		t.Fatalf("list: %v", err)
	}
	if len(entries) != 1 {
		t.Fatalf("got %d entries, want 1", len(entries))
	}
	if entries[0].UpdatedAt == "" {
		t.Error("updated_at is empty")
	}
}

func TestStateEntryJSON(t *testing.T) {
	e := state.Entry{Key: "author", Value: "claude", UpdatedAt: "2026-02-23 00:00:00"}
	data, err := json.Marshal(e)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	var parsed state.Entry
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if parsed != e {
		t.Errorf("round-trip mismatch: got %+v, want %+v", parsed, e)
	}
}

func TestStateSpecIsolation(t *testing.T) {
	db, specID1 := setupStateDB(t)

	// Insert a second spec
	res, err := db.Exec(
		`INSERT INTO spec_index (spec_path, total_lines, content_hash, parsed_at, source_type)
		 VALUES ('test2.md', 50, 'def456', datetime('now'), 'monolith')`)
	if err != nil {
		t.Fatalf("insert spec2: %v", err)
	}
	specID2, err := res.LastInsertId()
	if err != nil {
		t.Fatalf("last insert id: %v", err)
	}

	// Set same key in both specs
	if err := state.Set(db, specID1, "author", "alice"); err != nil {
		t.Fatalf("set spec1: %v", err)
	}
	if err := state.Set(db, specID2, "author", "bob"); err != nil {
		t.Fatalf("set spec2: %v", err)
	}

	// Each spec sees its own value
	val1, err := state.Get(db, specID1, "author")
	if err != nil {
		t.Fatalf("get spec1: %v", err)
	}
	if val1 != "alice" {
		t.Errorf("spec1 author = %q, want %q", val1, "alice")
	}

	val2, err := state.Get(db, specID2, "author")
	if err != nil {
		t.Fatalf("get spec2: %v", err)
	}
	if val2 != "bob" {
		t.Errorf("spec2 author = %q, want %q", val2, "bob")
	}
}
