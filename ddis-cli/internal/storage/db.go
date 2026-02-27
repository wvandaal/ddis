package storage

// ddis:implements APP-ADR-002 (SQLite as sole storage backend)

import (
	"database/sql"
	"fmt"
	"os"
	"strings"

	_ "modernc.org/sqlite"
)

// DB is the interface used by the parser for database operations.
// *sql.DB satisfies this interface.
type DB = *sql.DB

// OpenExisting opens an existing SQLite database, returning a clear error if
// the file does not exist. Use this for read-only commands that should never
// auto-create a database file.
// ddis:maintains APP-INV-059
// ddis:implements APP-ADR-046
func OpenExisting(dbPath string) (*sql.DB, error) {
	if _, err := os.Stat(dbPath); os.IsNotExist(err) {
		return nil, fmt.Errorf("database not found: %s (run ddis parse to create it)", dbPath)
	}
	return Open(dbPath)
}

// Open creates or opens a SQLite database at the given path and applies the schema.
func Open(dbPath string) (*sql.DB, error) {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return nil, fmt.Errorf("open database %s: %w", dbPath, err)
	}

	// SQLite PRAGMAs are per-connection. Minimize the connection pool so
	// the PRAGMA'd connection is reused for most operations. We cannot use
	// SetMaxOpenConns(1) because code that holds rows iterators open while
	// executing other queries would deadlock.
	db.SetMaxIdleConns(1)

	// WAL mode for better concurrent read performance
	if _, err := db.Exec("PRAGMA journal_mode=WAL"); err != nil {
		db.Close()
		return nil, fmt.Errorf("set WAL mode: %w", err)
	}

	// Foreign keys on
	if _, err := db.Exec("PRAGMA foreign_keys=ON"); err != nil {
		db.Close()
		return nil, fmt.Errorf("enable foreign keys: %w", err)
	}

	// Run migrations before schema application (CREATE TABLE IF NOT EXISTS
	// won't update existing tables with outdated CHECK constraints).
	if err := migrateSchema(db); err != nil {
		db.Close()
		return nil, fmt.Errorf("migrate schema: %w", err)
	}

	// Apply schema
	if _, err := db.Exec(SchemaSQL); err != nil {
		db.Close()
		return nil, fmt.Errorf("apply schema: %w", err)
	}

	return db, nil
}

// migrateSchema handles backward-incompatible schema changes.
func migrateSchema(db *sql.DB) error {
	// Migration 1: challenge_results CHECK constraint must include 'provisional'.
	// Added when the Provisional verdict tier was introduced between Confirmed
	// and Inconclusive. Old tables have CHECK(verdict IN ('confirmed','refuted','inconclusive')).
	var tableSql string
	err := db.QueryRow(`SELECT sql FROM sqlite_master WHERE type='table' AND name='challenge_results'`).Scan(&tableSql)
	if err == sql.ErrNoRows {
		return nil // Table doesn't exist yet — SchemaSQL will create it
	}
	if err != nil {
		return fmt.Errorf("check challenge_results schema: %w", err)
	}
	if !strings.Contains(tableSql, "'provisional'") {
		// Old schema — drop so SchemaSQL recreates with correct constraint.
		// Challenge results are transient and can be regenerated.
		if _, err := db.Exec(`DROP TABLE IF EXISTS challenge_results`); err != nil {
			return fmt.Errorf("drop old challenge_results: %w", err)
		}
	}
	return nil
}
