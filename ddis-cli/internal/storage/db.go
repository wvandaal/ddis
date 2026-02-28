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

	// Migrate witness data from old table if it exists (migration 2 data step).
	var hasOld int
	if db.QueryRow(`SELECT 1 FROM sqlite_master WHERE type='table' AND name='_witnesses_old'`).Scan(&hasOld) == nil {
		db.Exec(`INSERT INTO invariant_witnesses SELECT * FROM _witnesses_old`)
		db.Exec(`DROP TABLE _witnesses_old`)
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

	// Migration 2: invariant_witnesses CHECK constraint must include 'eval'.
	// Added when the eval evidence type was introduced for LLM-based evaluation.
	var witnessSql string
	err2 := db.QueryRow(`SELECT sql FROM sqlite_master WHERE type='table' AND name='invariant_witnesses'`).Scan(&witnessSql)
	if err2 == nil && !strings.Contains(witnessSql, "'eval'") {
		// Drop challenge_results first — it has a FK to invariant_witnesses.
		// Challenge results are transient and regenerable via `ddis challenge`.
		db.Exec(`DROP TABLE IF EXISTS challenge_results`)
		// Preserve existing witness data through temp table rename.
		// SchemaSQL will create the new table with updated CHECK.
		// Data is migrated after schema application in Open().
		if _, err := db.Exec(`ALTER TABLE invariant_witnesses RENAME TO _witnesses_old`); err != nil {
			return fmt.Errorf("rename old witnesses: %w", err)
		}
	}

	// Migration 3: challenge_results FK may reference stale "_witnesses_old" table.
	// ddis:implements APP-INV-107 (witness ID stability under upsert)
	// SQLite 3.25+ auto-updates FK references during ALTER TABLE RENAME.
	// When Migration 2 renamed invariant_witnesses → _witnesses_old, SQLite
	// rewrote challenge_results.witness_id FK to reference "_witnesses_old".
	// Drop challenge_results so SchemaSQL recreates it with correct FK target,
	// and clean up the migration artifact table.
	var crSql string
	err3 := db.QueryRow(`SELECT sql FROM sqlite_master WHERE type='table' AND name='challenge_results'`).Scan(&crSql)
	if err3 == nil && strings.Contains(crSql, "_witnesses_old") {
		db.Exec(`DROP TABLE IF EXISTS challenge_results`)
	}
	// Clean up _witnesses_old if new invariant_witnesses already exists with data.
	var oldExists int
	db.QueryRow(`SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='_witnesses_old'`).Scan(&oldExists)
	if oldExists > 0 {
		var newExists int
		db.QueryRow(`SELECT COUNT(*) FROM invariant_witnesses`).Scan(&newExists)
		if newExists > 0 {
			db.Exec(`DROP TABLE IF EXISTS _witnesses_old`)
		}
	}

	return nil
}
