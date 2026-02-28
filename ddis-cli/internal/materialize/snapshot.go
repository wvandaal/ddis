// Package materialize — snapshot creation, verification, and accelerated fold.
//
// ddis:implements APP-INV-093 (snapshot creation determinism — same state always produces same hash)
// ddis:implements APP-INV-094 (snapshot monotonicity — later snapshots cover more events)
// ddis:implements APP-INV-095 (snapshot recovery graceful degradation — corrupt snapshot → full replay)
// ddis:implements APP-ADR-072 (snapshot as SQLite state hash)
// ddis:implements APP-ADR-073 (automatic snapshot interval)

package materialize

import (
	"database/sql"
	"fmt"
	"time"
)

// Snapshot represents a materialization checkpoint.
type Snapshot struct {
	ID        int64
	SpecID    int64
	Position  int    // number of events processed at snapshot time
	StateHash string // SHA-256 over canonicalized content tables
	CreatedAt string
}

// CreateSnapshot records a snapshot checkpoint after materialization.
// The state hash is computed deterministically from the current DB content (APP-INV-093).
func CreateSnapshot(db *sql.DB, specID int64, position int) (*Snapshot, error) {
	hash, err := StateHash(db, specID)
	if err != nil {
		return nil, fmt.Errorf("compute state hash: %w", err)
	}

	now := time.Now().UTC().Format(time.RFC3339)
	res, err := db.Exec(
		`INSERT INTO snapshots (spec_id, position, state_hash, created_at) VALUES (?, ?, ?, ?)`,
		specID, position, hash, now)
	if err != nil {
		return nil, fmt.Errorf("insert snapshot: %w", err)
	}

	id, _ := res.LastInsertId()
	return &Snapshot{
		ID:        id,
		SpecID:    specID,
		Position:  position,
		StateHash: hash,
		CreatedAt: now,
	}, nil
}

// LoadLatestSnapshot retrieves the most recent valid snapshot for a spec.
// Returns nil if no snapshots exist (triggering full replay per APP-INV-095).
func LoadLatestSnapshot(db *sql.DB, specID int64) (*Snapshot, error) {
	row := db.QueryRow(
		`SELECT id, spec_id, position, state_hash, created_at
		 FROM snapshots WHERE spec_id = ? ORDER BY position DESC LIMIT 1`, specID)

	var s Snapshot
	err := row.Scan(&s.ID, &s.SpecID, &s.Position, &s.StateHash, &s.CreatedAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("scan snapshot: %w", err)
	}
	return &s, nil
}

// VerifySnapshot checks that a snapshot's state hash matches the current DB state.
// Returns true if valid, false if corrupted (APP-INV-095: graceful degradation).
func VerifySnapshot(db *sql.DB, snap *Snapshot) (bool, error) {
	currentHash, err := StateHash(db, snap.SpecID)
	if err != nil {
		return false, fmt.Errorf("compute current hash: %w", err)
	}
	return currentHash == snap.StateHash, nil
}

// ListSnapshots returns all snapshots for a spec, ordered by position.
func ListSnapshots(db *sql.DB, specID int64) ([]Snapshot, error) {
	rows, err := db.Query(
		`SELECT id, spec_id, position, state_hash, created_at
		 FROM snapshots WHERE spec_id = ? ORDER BY position ASC`, specID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var snapshots []Snapshot
	for rows.Next() {
		var s Snapshot
		if err := rows.Scan(&s.ID, &s.SpecID, &s.Position, &s.StateHash, &s.CreatedAt); err != nil {
			return nil, err
		}
		snapshots = append(snapshots, s)
	}
	return snapshots, nil
}

// PruneSnapshots removes all but the latest N snapshots for a spec.
// APP-INV-094: monotonicity means we keep the most recent ones.
func PruneSnapshots(db *sql.DB, specID int64, keepN int) (int, error) {
	if keepN < 1 {
		keepN = 1
	}

	res, err := db.Exec(
		`DELETE FROM snapshots WHERE spec_id = ? AND id NOT IN (
			SELECT id FROM snapshots WHERE spec_id = ? ORDER BY position DESC LIMIT ?
		)`, specID, specID, keepN)
	if err != nil {
		return 0, fmt.Errorf("prune snapshots: %w", err)
	}
	n, _ := res.RowsAffected()
	return int(n), nil
}
