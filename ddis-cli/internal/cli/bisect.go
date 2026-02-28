package cli

// ddis:implements APP-INV-082 (bisect correctness — binary search over event log)
// ddis:implements APP-ADR-059 (deterministic fold — bisect relies on fold determinism)

import (
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/causal"
	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/materialize"
	"github.com/wvandaal/ddis/internal/storage"
)

var (
	bisectPredicate string
	bisectStream    string
	bisectJSON      bool
)

var bisectCmd = &cobra.Command{
	Use:   "bisect --predicate <check>",
	Short: "Find the earliest defect-introducing event",
	Long: `Binary search over an event log to find the first event that introduced
a defect (APP-INV-082).

The predicate defines the defect condition. When the predicate evaluates to
true, the defect is present. Bisect materializes progressively larger prefixes
of the event log until it isolates the introducing event.

Built-in predicates:
  invariant-missing:<ID>  — defect if the invariant is absent after fold
  adr-missing:<ID>        — defect if the ADR is absent after fold
  validation-fail         — defect if validation fails on materialized state

Examples:
  ddis bisect --predicate "invariant-missing:APP-INV-001" --stream .ddis/events/stream-2.jsonl
  ddis bisect --predicate "validation-fail" --json`,
	RunE:          runBisect,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	bisectCmd.Flags().StringVar(&bisectPredicate, "predicate", "", "Defect predicate (required)")
	bisectCmd.Flags().StringVar(&bisectStream, "stream", "", "Path to event stream (default: .ddis/events/stream-2.jsonl)")
	bisectCmd.Flags().BoolVar(&bisectJSON, "json", false, "Output result as JSON")
	_ = bisectCmd.MarkFlagRequired("predicate")
}

func runBisect(cmd *cobra.Command, args []string) error {
	// Determine stream path
	streamPath := bisectStream
	if streamPath == "" {
		streamPath = events.StreamPath(".", events.StreamSpecification)
		if _, err := os.Stat(streamPath); os.IsNotExist(err) {
			return fmt.Errorf("no event stream found; use --stream to specify")
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

	if len(contentEvts) == 0 {
		return fmt.Errorf("no content events found in stream")
	}

	// Build predicate function
	pred, err := buildPredicate(bisectPredicate)
	if err != nil {
		return err
	}

	// Run bisect
	result, err := causal.Bisect(contentEvts, pred)
	if err != nil {
		return fmt.Errorf("bisect: %w", err)
	}

	// Report
	if bisectJSON {
		fmt.Printf(`{"introducing_event":"%s","type":"%s","timestamp":"%s"}`,
			result.ID, result.Type, result.Timestamp)
		fmt.Println()
	} else {
		fmt.Printf("Defect introduced by event %s\n", result.ID)
		fmt.Printf("  Type:      %s\n", result.Type)
		fmt.Printf("  Timestamp: %s\n", result.Timestamp)
		fmt.Printf("  Payload:   %s\n", string(result.Payload))
	}

	if !NoGuidance {
		fmt.Fprintln(os.Stderr, "\nNext: ddis blame "+result.ID)
	}

	return nil
}

// buildPredicate creates a BisectPredicate from a string specification.
func buildPredicate(spec string) (causal.BisectPredicate, error) {
	parts := strings.SplitN(spec, ":", 2)
	switch parts[0] {
	case "invariant-missing":
		if len(parts) < 2 {
			return nil, fmt.Errorf("invariant-missing predicate requires an invariant ID")
		}
		invID := parts[1]
		return func(evts []*events.Event) (bool, error) {
			return !hasInvariantAfterFold(evts, invID), nil
		}, nil
	case "adr-missing":
		if len(parts) < 2 {
			return nil, fmt.Errorf("adr-missing predicate requires an ADR ID")
		}
		adrID := parts[1]
		return func(evts []*events.Event) (bool, error) {
			return !hasADRAfterFold(evts, adrID), nil
		}, nil
	default:
		return nil, fmt.Errorf("unknown predicate: %s\nSupported: invariant-missing:<ID>, adr-missing:<ID>", parts[0])
	}
}

// hasInvariantAfterFold checks if an invariant exists after folding the events.
func hasInvariantAfterFold(evts []*events.Event, invID string) bool {
	// Create temporary in-memory DB
	db, err := storage.Open(":memory:")
	if err != nil {
		return false
	}
	defer db.Close()

	applier := &sqlApplier{db: db}
	materialize.Fold(applier, evts)

	var count int
	row := db.QueryRow(`SELECT COUNT(*) FROM invariants WHERE invariant_id = ?`, invID)
	if row.Scan(&count) != nil {
		return false
	}
	return count > 0
}

// hasADRAfterFold checks if an ADR exists after folding the events.
func hasADRAfterFold(evts []*events.Event, adrID string) bool {
	db, err := storage.Open(":memory:")
	if err != nil {
		return false
	}
	defer db.Close()

	applier := &sqlApplier{db: db}
	materialize.Fold(applier, evts)

	var count int
	row := db.QueryRow(`SELECT COUNT(*) FROM adrs WHERE adr_id = ?`, adrID)
	if row.Scan(&count) != nil {
		return false
	}
	return count > 0
}
