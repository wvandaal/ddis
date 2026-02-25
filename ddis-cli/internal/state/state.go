package state

// ddis:maintains APP-INV-025 (discovery provenance chain)

import (
	"database/sql"
	"fmt"
)

// Entry represents a single key-value pair in the session state.
type Entry struct {
	Key       string `json:"key"`
	Value     string `json:"value"`
	UpdatedAt string `json:"updated_at"`
}

// Set inserts or updates a key-value pair for the given spec.
func Set(db *sql.DB, specID int64, key, value string) error {
	_, err := db.Exec(
		`INSERT INTO session_state (spec_id, key, value, updated_at)
		 VALUES (?, ?, ?, datetime('now'))
		 ON CONFLICT(spec_id, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at`,
		specID, key, value)
	return err
}

// Get retrieves the value for a key, or returns an error if not found.
func Get(db *sql.DB, specID int64, key string) (string, error) {
	var value string
	err := db.QueryRow(
		`SELECT value FROM session_state WHERE spec_id = ? AND key = ?`,
		specID, key).Scan(&value)
	if err == sql.ErrNoRows {
		return "", fmt.Errorf("key %q not found", key)
	}
	return value, err
}

// List returns all key-value pairs for the given spec, ordered by key.
func List(db *sql.DB, specID int64) ([]Entry, error) {
	rows, err := db.Query(
		`SELECT key, value, updated_at FROM session_state WHERE spec_id = ? ORDER BY key`,
		specID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var entries []Entry
	for rows.Next() {
		var e Entry
		if err := rows.Scan(&e.Key, &e.Value, &e.UpdatedAt); err != nil {
			return nil, err
		}
		entries = append(entries, e)
	}
	return entries, rows.Err()
}

// Delete removes a key-value pair. Returns an error if the key does not exist.
func Delete(db *sql.DB, specID int64, key string) error {
	result, err := db.Exec(
		`DELETE FROM session_state WHERE spec_id = ? AND key = ?`,
		specID, key)
	if err != nil {
		return err
	}
	n, _ := result.RowsAffected()
	if n == 0 {
		return fmt.Errorf("key %q not found", key)
	}
	return nil
}
