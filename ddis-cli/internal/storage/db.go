package storage

// ddis:implements APP-ADR-002 (SQLite as sole storage backend)

import (
	"database/sql"
	"fmt"

	_ "modernc.org/sqlite"
)

// DB is the interface used by the parser for database operations.
// *sql.DB satisfies this interface.
type DB = *sql.DB

// Open creates or opens a SQLite database at the given path and applies the schema.
func Open(dbPath string) (*sql.DB, error) {
	db, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return nil, fmt.Errorf("open database %s: %w", dbPath, err)
	}

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

	// Apply schema
	if _, err := db.Exec(SchemaSQL); err != nil {
		db.Close()
		return nil, fmt.Errorf("apply schema: %w", err)
	}

	return db, nil
}
