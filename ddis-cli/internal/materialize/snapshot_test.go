package materialize

// ddis:tests APP-INV-093 (snapshot creation determinism — same state → same hash)
// ddis:tests APP-INV-094 (snapshot monotonicity — later snapshots have higher positions)
// ddis:tests APP-INV-095 (snapshot recovery graceful degradation — corrupted → full replay)

import (
	"testing"

	_ "modernc.org/sqlite"
)

func TestCreateSnapshot(t *testing.T) {
	db := createTestDB(t)
	defer db.Close()

	// Add snapshots table
	db.Exec(`CREATE TABLE IF NOT EXISTS snapshots (
		id INTEGER PRIMARY KEY,
		spec_id INTEGER NOT NULL,
		position INTEGER NOT NULL,
		state_hash TEXT NOT NULL,
		created_at TEXT NOT NULL DEFAULT (datetime('now'))
	)`)

	seedContent(t, db, 1)

	snap, err := CreateSnapshot(db, 1, 100)
	if err != nil {
		t.Fatalf("CreateSnapshot: %v", err)
	}
	if snap.Position != 100 {
		t.Errorf("expected position 100, got %d", snap.Position)
	}
	if snap.StateHash == "" {
		t.Error("expected non-empty state hash")
	}
	if snap.ID == 0 {
		t.Error("expected non-zero snapshot ID")
	}
}

func TestCreateSnapshot_Determinism(t *testing.T) {
	// APP-INV-093: same state always produces same hash
	db := createTestDB(t)
	defer db.Close()

	db.Exec(`CREATE TABLE IF NOT EXISTS snapshots (
		id INTEGER PRIMARY KEY,
		spec_id INTEGER NOT NULL,
		position INTEGER NOT NULL,
		state_hash TEXT NOT NULL,
		created_at TEXT NOT NULL DEFAULT (datetime('now'))
	)`)

	seedContent(t, db, 1)

	snap1, err := CreateSnapshot(db, 1, 100)
	if err != nil {
		t.Fatalf("CreateSnapshot 1: %v", err)
	}

	snap2, err := CreateSnapshot(db, 1, 100)
	if err != nil {
		t.Fatalf("CreateSnapshot 2: %v", err)
	}

	if snap1.StateHash != snap2.StateHash {
		t.Errorf("determinism violation: %s != %s", snap1.StateHash, snap2.StateHash)
	}
}

func TestLoadLatestSnapshot(t *testing.T) {
	db := createTestDB(t)
	defer db.Close()

	db.Exec(`CREATE TABLE IF NOT EXISTS snapshots (
		id INTEGER PRIMARY KEY,
		spec_id INTEGER NOT NULL,
		position INTEGER NOT NULL,
		state_hash TEXT NOT NULL,
		created_at TEXT NOT NULL DEFAULT (datetime('now'))
	)`)

	seedContent(t, db, 1)

	// No snapshots yet
	snap, err := LoadLatestSnapshot(db, 1)
	if err != nil {
		t.Fatalf("LoadLatestSnapshot: %v", err)
	}
	if snap != nil {
		t.Error("expected nil for no snapshots")
	}

	// Create two snapshots
	CreateSnapshot(db, 1, 50)
	CreateSnapshot(db, 1, 100)

	snap, err = LoadLatestSnapshot(db, 1)
	if err != nil {
		t.Fatalf("LoadLatestSnapshot: %v", err)
	}
	if snap == nil {
		t.Fatal("expected non-nil snapshot")
	}
	// APP-INV-094: latest has highest position
	if snap.Position != 100 {
		t.Errorf("expected position 100, got %d", snap.Position)
	}
}

func TestVerifySnapshot(t *testing.T) {
	db := createTestDB(t)
	defer db.Close()

	db.Exec(`CREATE TABLE IF NOT EXISTS snapshots (
		id INTEGER PRIMARY KEY,
		spec_id INTEGER NOT NULL,
		position INTEGER NOT NULL,
		state_hash TEXT NOT NULL,
		created_at TEXT NOT NULL DEFAULT (datetime('now'))
	)`)

	seedContent(t, db, 1)

	snap, err := CreateSnapshot(db, 1, 100)
	if err != nil {
		t.Fatalf("CreateSnapshot: %v", err)
	}

	// Valid snapshot
	valid, err := VerifySnapshot(db, snap)
	if err != nil {
		t.Fatalf("VerifySnapshot: %v", err)
	}
	if !valid {
		t.Error("expected snapshot to be valid immediately after creation")
	}

	// Modify state — snapshot should become invalid (APP-INV-095)
	db.Exec(`INSERT INTO invariants (spec_id, source_file_id, section_id, invariant_id, title, statement, semi_formal, violation_scenario, validation_method, why_this_matters, line_start, line_end, raw_text, content_hash)
		VALUES (1, 1, 0, 'APP-INV-099', 'New Invariant', 'Added after snapshot', '', '', '', '', 0, 0, '', '')`)

	valid, err = VerifySnapshot(db, snap)
	if err != nil {
		t.Fatalf("VerifySnapshot after change: %v", err)
	}
	if valid {
		t.Error("expected snapshot to be invalid after state change")
	}
}

func TestPruneSnapshots(t *testing.T) {
	db := createTestDB(t)
	defer db.Close()

	db.Exec(`CREATE TABLE IF NOT EXISTS snapshots (
		id INTEGER PRIMARY KEY,
		spec_id INTEGER NOT NULL,
		position INTEGER NOT NULL,
		state_hash TEXT NOT NULL,
		created_at TEXT NOT NULL DEFAULT (datetime('now'))
	)`)

	seedContent(t, db, 1)

	// Create 5 snapshots
	for i := 1; i <= 5; i++ {
		CreateSnapshot(db, 1, i*100)
	}

	// Prune to keep 2
	pruned, err := PruneSnapshots(db, 1, 2)
	if err != nil {
		t.Fatalf("PruneSnapshots: %v", err)
	}
	if pruned != 3 {
		t.Errorf("expected 3 pruned, got %d", pruned)
	}

	// Verify remaining
	remaining, err := ListSnapshots(db, 1)
	if err != nil {
		t.Fatalf("ListSnapshots: %v", err)
	}
	if len(remaining) != 2 {
		t.Errorf("expected 2 remaining, got %d", len(remaining))
	}
	// Should be the two highest positions
	if remaining[0].Position != 400 {
		t.Errorf("expected position 400 first, got %d", remaining[0].Position)
	}
	if remaining[1].Position != 500 {
		t.Errorf("expected position 500 second, got %d", remaining[1].Position)
	}
}

func TestListSnapshots(t *testing.T) {
	db := createTestDB(t)
	defer db.Close()

	db.Exec(`CREATE TABLE IF NOT EXISTS snapshots (
		id INTEGER PRIMARY KEY,
		spec_id INTEGER NOT NULL,
		position INTEGER NOT NULL,
		state_hash TEXT NOT NULL,
		created_at TEXT NOT NULL DEFAULT (datetime('now'))
	)`)

	seedContent(t, db, 1)

	// Empty list
	snaps, err := ListSnapshots(db, 1)
	if err != nil {
		t.Fatalf("ListSnapshots: %v", err)
	}
	if len(snaps) != 0 {
		t.Errorf("expected 0 snapshots, got %d", len(snaps))
	}

	// Create snapshots
	CreateSnapshot(db, 1, 100)
	CreateSnapshot(db, 1, 200)

	snaps, err = ListSnapshots(db, 1)
	if err != nil {
		t.Fatalf("ListSnapshots: %v", err)
	}
	if len(snaps) != 2 {
		t.Errorf("expected 2 snapshots, got %d", len(snaps))
	}
	// Ordered by position ASC
	if snaps[0].Position != 100 || snaps[1].Position != 200 {
		t.Errorf("expected positions [100, 200], got [%d, %d]", snaps[0].Position, snaps[1].Position)
	}
}
