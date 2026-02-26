package events

// ddis:maintains APP-INV-037 (workspace isolation — event streams within workspace)

import (
	"path/filepath"
	"strings"
)

// WorkspaceRoot derives the DDIS workspace root from a database path.
// If the path contains .ddis/index.db, the parent of .ddis is the workspace.
// Otherwise, the database's directory is treated as the workspace root.
func WorkspaceRoot(dbPath string) string {
	abs, err := filepath.Abs(dbPath)
	if err != nil {
		return "."
	}

	// .ddis/index.db → parent of .ddis is workspace
	if strings.HasSuffix(filepath.Dir(abs), ".ddis") {
		return filepath.Dir(filepath.Dir(abs))
	}

	// manifest.ddis.db or similar → same directory
	return filepath.Dir(abs)
}
