package renderer

import (
	"fmt"
	"os"

	"github.com/wvandaal/ddis/internal/storage"
)

// RenderMonolith writes the stored raw_text back to a file for round-trip fidelity.
func RenderMonolith(db storage.DB, specID int64, outputPath string) error {
	var rawText string
	err := db.QueryRow(
		`SELECT raw_text FROM source_files WHERE spec_id = ? AND file_role IN ('monolith', 'system_constitution')
		 ORDER BY CASE file_role WHEN 'monolith' THEN 0 ELSE 1 END LIMIT 1`,
		specID,
	).Scan(&rawText)
	if err != nil {
		return fmt.Errorf("query source file: %w", err)
	}

	// Write to temp file then atomic rename
	tmpPath := outputPath + ".tmp"
	if err := os.WriteFile(tmpPath, []byte(rawText), 0644); err != nil {
		return fmt.Errorf("write temp file: %w", err)
	}

	if err := os.Rename(tmpPath, outputPath); err != nil {
		os.Remove(tmpPath)
		return fmt.Errorf("rename to output: %w", err)
	}

	return nil
}
