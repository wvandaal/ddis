package renderer

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/wvandaal/ddis/internal/storage"
)

// RenderModular writes each stored source file back to the output directory.
func RenderModular(db storage.DB, specID int64, outputDir string) error {
	rows, err := db.Query(
		`SELECT file_path, raw_text FROM source_files WHERE spec_id = ? AND file_role != 'manifest'
		 ORDER BY id`,
		specID,
	)
	if err != nil {
		return fmt.Errorf("query source files: %w", err)
	}
	defer rows.Close()

	for rows.Next() {
		var filePath, rawText string
		if err := rows.Scan(&filePath, &rawText); err != nil {
			return fmt.Errorf("scan source file: %w", err)
		}

		outPath := filepath.Join(outputDir, filePath)
		outDir := filepath.Dir(outPath)

		if err := os.MkdirAll(outDir, 0755); err != nil {
			return fmt.Errorf("create dir %s: %w", outDir, err)
		}

		if err := os.WriteFile(outPath, []byte(rawText), 0644); err != nil {
			return fmt.Errorf("write %s: %w", outPath, err)
		}
	}

	// Also write the manifest
	var rawYAML string
	err = db.QueryRow(
		`SELECT raw_yaml FROM manifest WHERE spec_id = ?`, specID,
	).Scan(&rawYAML)
	if err != nil {
		return fmt.Errorf("query manifest: %w", err)
	}

	manifestPath := filepath.Join(outputDir, "manifest.yaml")
	if err := os.WriteFile(manifestPath, []byte(rawYAML), 0644); err != nil {
		return fmt.Errorf("write manifest: %w", err)
	}

	return nil
}
