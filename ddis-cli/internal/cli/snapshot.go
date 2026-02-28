package cli

// ddis:implements APP-INV-093 (snapshot creation determinism — CLI wiring for snapshot create)
// ddis:implements APP-INV-094 (snapshot monotonicity — list shows ordered snapshots)
// ddis:implements APP-INV-095 (snapshot recovery graceful degradation — verify detects corruption)
// ddis:implements APP-ADR-073 (automatic snapshot interval — CLI controls for snapshot lifecycle)

import (
	"database/sql"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/materialize"
	"github.com/wvandaal/ddis/internal/storage"
)

var snapshotKeepN int

var snapshotCmd = &cobra.Command{
	Use:   "snapshot",
	Short: "Manage materialization snapshots",
	Long: `Create, list, verify, and prune snapshot checkpoints for the
materialized state. Snapshots accelerate re-materialization by allowing
the fold to skip already-processed events.

Each snapshot records a position in the event stream and a SHA-256 hash
of the materialized state at that position (APP-INV-093).

Examples:
  ddis snapshot create index.db
  ddis snapshot list index.db
  ddis snapshot verify index.db
  ddis snapshot prune index.db --keep 3`,
}

var snapshotCreateCmd = &cobra.Command{
	Use:   "create [db-path]",
	Short: "Create a snapshot of the current materialized state",
	Args:  cobra.MaximumNArgs(1),
	RunE:  runSnapshotCreate,
}

var snapshotListCmd = &cobra.Command{
	Use:   "list [db-path]",
	Short: "List all snapshots",
	Args:  cobra.MaximumNArgs(1),
	RunE:  runSnapshotList,
}

var snapshotVerifyCmd = &cobra.Command{
	Use:   "verify [db-path]",
	Short: "Verify the latest snapshot against current state",
	Args:  cobra.MaximumNArgs(1),
	RunE:  runSnapshotVerify,
}

var snapshotPruneCmd = &cobra.Command{
	Use:   "prune [db-path]",
	Short: "Remove old snapshots, keeping the latest N",
	Args:  cobra.MaximumNArgs(1),
	RunE:  runSnapshotPrune,
}

func init() {
	snapshotPruneCmd.Flags().IntVar(&snapshotKeepN, "keep", 3, "Number of latest snapshots to keep")

	snapshotCmd.AddCommand(snapshotCreateCmd)
	snapshotCmd.AddCommand(snapshotListCmd)
	snapshotCmd.AddCommand(snapshotVerifyCmd)
	snapshotCmd.AddCommand(snapshotPruneCmd)
}

func openDBForSnapshot(args []string) (*sql.DB, int64, error) {
	dbPath, err := resolveDBPath(args)
	if err != nil {
		return nil, 0, err
	}

	db, err := storage.OpenExisting(dbPath)
	if err != nil {
		return nil, 0, fmt.Errorf("open database: %w", err)
	}

	var specID int64
	if err := db.QueryRow("SELECT id FROM spec_index LIMIT 1").Scan(&specID); err != nil {
		db.Close()
		return nil, 0, fmt.Errorf("no spec found in database: %w", err)
	}

	return db, specID, nil
}

func runSnapshotCreate(cmd *cobra.Command, args []string) error {
	db, specID, err := openDBForSnapshot(args)
	if err != nil {
		return err
	}
	defer db.Close()

	// ddis:maintains APP-INV-098 (snapshot position is event-stream ordinal, not content count)
	// Count events in the canonical event stream (not materialized content).
	// Position represents the fold ordinal: how many events have been applied.
	var eventCount int
	streamPath := events.StreamPath(".", events.StreamSpecification)
	if evts, err := events.ReadStream(streamPath, events.EventFilters{}); err == nil {
		eventCount = len(evts)
	} else {
		// Fallback: count event_provenance rows if stream file not accessible from CWD.
		db.QueryRow("SELECT COUNT(*) FROM event_provenance WHERE spec_id = ?", specID).Scan(&eventCount)
	}

	snap, err := materialize.CreateSnapshot(db, specID, eventCount)
	if err != nil {
		return fmt.Errorf("create snapshot: %w", err)
	}

	fmt.Printf("Created snapshot #%d at position %d\n", snap.ID, snap.Position)
	fmt.Printf("  State hash: %s\n", snap.StateHash)
	return nil
}

func runSnapshotList(cmd *cobra.Command, args []string) error {
	db, specID, err := openDBForSnapshot(args)
	if err != nil {
		return err
	}
	defer db.Close()

	snaps, err := materialize.ListSnapshots(db, specID)
	if err != nil {
		return fmt.Errorf("list snapshots: %w", err)
	}

	if len(snaps) == 0 {
		fmt.Println("No snapshots found.")
		return nil
	}

	fmt.Printf("%-4s  %-10s  %-20s  %s\n", "ID", "Position", "Created", "State Hash")
	fmt.Println("────  ──────────  ────────────────────  " + "────────────────────────────────────────────────────────────────")
	for _, s := range snaps {
		fmt.Printf("%-4d  %-10d  %-20s  %s\n", s.ID, s.Position, s.CreatedAt, s.StateHash)
	}
	return nil
}

func runSnapshotVerify(cmd *cobra.Command, args []string) error {
	db, specID, err := openDBForSnapshot(args)
	if err != nil {
		return err
	}
	defer db.Close()

	snap, err := materialize.LoadLatestSnapshot(db, specID)
	if err != nil {
		return fmt.Errorf("load snapshot: %w", err)
	}
	if snap == nil {
		fmt.Println("No snapshots to verify.")
		return nil
	}

	valid, err := materialize.VerifySnapshot(db, snap)
	if err != nil {
		return fmt.Errorf("verify snapshot: %w", err)
	}

	if valid {
		fmt.Printf("Snapshot #%d at position %d is VALID\n", snap.ID, snap.Position)
		fmt.Printf("  State hash: %s\n", snap.StateHash)
	} else {
		fmt.Fprintf(os.Stderr, "Snapshot #%d at position %d is INVALID (state has changed)\n", snap.ID, snap.Position)
		fmt.Fprintf(os.Stderr, "  Recorded hash: %s\n", snap.StateHash)
		currentHash, _ := materialize.StateHash(db, specID)
		fmt.Fprintf(os.Stderr, "  Current hash:  %s\n", currentHash)
		fmt.Fprintln(os.Stderr, "  Recommendation: re-materialize from full event stream")
	}

	return nil
}

func runSnapshotPrune(cmd *cobra.Command, args []string) error {
	db, specID, err := openDBForSnapshot(args)
	if err != nil {
		return err
	}
	defer db.Close()

	pruned, err := materialize.PruneSnapshots(db, specID, snapshotKeepN)
	if err != nil {
		return fmt.Errorf("prune snapshots: %w", err)
	}

	fmt.Printf("Pruned %d snapshots (keeping latest %d)\n", pruned, snapshotKeepN)
	return nil
}
