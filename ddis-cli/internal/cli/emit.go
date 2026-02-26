package cli

// ddis:implements APP-ADR-015 (three-stream event sourcing — CLI emission wiring)
// ddis:maintains APP-INV-020 (event stream append-only — best-effort, never blocks commands)
// ddis:implements APP-INV-053 (event stream completeness — emitEvent wires CLI commands to event streams)

import (
	"fmt"
	"os"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/storage"
)

// emitEvent is a best-effort event emitter. It appends a single event to the
// appropriate stream file. Errors are logged to stderr but never propagated —
// event emission must not block the primary command.
func emitEvent(dbPath string, stream events.Stream, eventType string, specHash string, payload interface{}) {
	wsRoot := events.WorkspaceRoot(dbPath)
	dir := events.StreamDir(wsRoot)

	// Ensure events directory exists.
	if err := os.MkdirAll(dir, 0o755); err != nil {
		fmt.Fprintf(os.Stderr, "event: mkdir %s: %v\n", dir, err)
		return
	}

	streamPath := events.StreamPath(wsRoot, stream)
	evt, err := events.NewEvent(stream, eventType, specHash, payload)
	if err != nil {
		fmt.Fprintf(os.Stderr, "event: new %s: %v\n", eventType, err)
		return
	}

	if err := events.AppendEvent(streamPath, evt); err != nil {
		fmt.Fprintf(os.Stderr, "event: append %s: %v\n", eventType, err)
	}
}

// specHashFromDB retrieves the content hash for a spec from the database.
// Returns empty string on error (best-effort).
func specHashFromDB(db storage.DB, specID int64) string {
	spec, err := storage.GetSpecIndex(db, specID)
	if err != nil {
		return ""
	}
	return spec.ContentHash
}
