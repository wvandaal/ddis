package cli

// ddis:implements APP-INV-079 (temporal query soundness — fold(log[0:t]) = valid spec at time t)
// ddis:implements APP-INV-083 (snapshot consistency — fold from snapshot)
// ddis:implements APP-ADR-064 (snapshot as fold checkpoint)

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/materialize"
	"github.com/wvandaal/ddis/internal/storage"
)

var (
	replayOutput string
	replayUntil  string
	replayPos    int
	replayJSON   bool
)

var replayCmd = &cobra.Command{
	Use:   "replay [stream-path]",
	Short: "Materialize spec state at a specific point in time",
	Long: `Replays the event log up to a specific position or timestamp, producing
a materialized view of the specification as it existed at that point
(APP-INV-079).

This enables temporal queries: "What did the spec look like at time T?"
The fold is deterministic, so replaying to the same position always
produces identical state (APP-INV-073).

Examples:
  ddis replay .ddis/events/stream-2.jsonl --until 2026-02-27T12:00:00Z -o past.db
  ddis replay .ddis/events/stream-2.jsonl --position 50 -o past.db
  ddis replay --until 2026-02-27T00:00:00Z --json`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runReplay,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	replayCmd.Flags().StringVarP(&replayOutput, "output", "o", "", "Output database path (required)")
	replayCmd.Flags().StringVar(&replayUntil, "until", "", "Replay events up to this RFC3339 timestamp")
	replayCmd.Flags().IntVar(&replayPos, "position", 0, "Replay events up to this position (1-indexed)")
	replayCmd.Flags().BoolVar(&replayJSON, "json", false, "Output result as JSON")
}

func runReplay(cmd *cobra.Command, args []string) error {
	if replayUntil == "" && replayPos == 0 {
		return fmt.Errorf("specify --until <timestamp> or --position <N>")
	}

	// Determine stream path
	var streamPath string
	if len(args) > 0 {
		streamPath = args[0]
	} else {
		streamPath = events.StreamPath(".", events.StreamSpecification)
		if _, err := os.Stat(streamPath); os.IsNotExist(err) {
			return fmt.Errorf("no event stream found; specify a stream path")
		}
	}

	// Read events
	evts, err := events.ReadStream(streamPath, events.EventFilters{})
	if err != nil {
		return fmt.Errorf("read stream: %w", err)
	}

	// Filter to content events
	var contentEvts []*events.Event
	for _, e := range evts {
		if isContentEvent(e.Type) {
			contentEvts = append(contentEvts, e)
		}
	}

	// Apply temporal filter
	var filtered []*events.Event
	if replayPos > 0 {
		if replayPos > len(contentEvts) {
			replayPos = len(contentEvts)
		}
		filtered = contentEvts[:replayPos]
	} else if replayUntil != "" {
		for _, e := range contentEvts {
			if e.Timestamp <= replayUntil {
				filtered = append(filtered, e)
			}
		}
	}

	if len(filtered) == 0 {
		return fmt.Errorf("no events match the temporal filter")
	}

	// Determine output path
	dbPath := replayOutput
	if dbPath == "" {
		dbPath = "replay.db"
	}

	// Create fresh database and fold
	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("create database: %w", err)
	}
	defer db.Close()

	applier := &sqlApplier{db: db}
	result, err := materialize.Fold(applier, filtered)
	if err != nil {
		return fmt.Errorf("fold: %w", err)
	}

	// Report
	if replayJSON {
		fmt.Printf(`{"events_replayed":%d,"total_available":%d,"output":"%s"}`,
			result.EventsProcessed, len(contentEvts), dbPath)
		fmt.Println()
	} else {
		fmt.Printf("Replayed %d/%d events → %s\n", result.EventsProcessed, len(contentEvts), dbPath)
		if replayUntil != "" {
			fmt.Printf("  Until: %s\n", replayUntil)
		}
		if replayPos > 0 {
			fmt.Printf("  Position: %d\n", replayPos)
		}
	}

	if !NoGuidance {
		fmt.Fprintln(os.Stderr, "\nNext: ddis validate "+dbPath)
	}

	return nil
}
