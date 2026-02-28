package cli

// ddis:implements APP-INV-084 (causal provenance — element-to-event tracing)
// ddis:implements APP-ADR-060 (event references for causal metadata)

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/causal"
	"github.com/wvandaal/ddis/internal/events"
)

var (
	blameStream string
	blameJSON   bool
)

var blameCmd = &cobra.Command{
	Use:   "blame <element-id>",
	Short: "Trace an element to its crystallization events",
	Long: `Traces a spec element (invariant, ADR, section, etc.) back to all events
that created or modified it (APP-INV-084).

The provenance chain shows the complete history of how an element reached
its current state, including crystallization, updates, and any supersessions.

Examples:
  ddis blame APP-INV-001
  ddis blame APP-ADR-058 --stream .ddis/events/stream-2.jsonl
  ddis blame APP-INV-073 --json`,
	Args:          cobra.ExactArgs(1),
	RunE:          runBlame,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	blameCmd.Flags().StringVar(&blameStream, "stream", "", "Path to event stream (default: .ddis/events/stream-2.jsonl)")
	blameCmd.Flags().BoolVar(&blameJSON, "json", false, "Output result as JSON")
}

func runBlame(cmd *cobra.Command, args []string) error {
	elementID := args[0]

	// Determine stream path
	streamPath := blameStream
	if streamPath == "" {
		streamPath = events.StreamPath(".", events.StreamSpecification)
	}

	// Read events from all relevant streams
	var allEvts []*events.Event

	// Stream 2 (specification events)
	evts2, err := events.ReadStream(streamPath, events.EventFilters{})
	if err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("read stream: %w", err)
	}
	allEvts = append(allEvts, evts2...)

	// Stream 3 (implementation events — witnesses, challenges)
	stream3Path := events.StreamPath(".", events.StreamImplementation)
	evts3, err := events.ReadStream(stream3Path, events.EventFilters{})
	if err == nil {
		allEvts = append(allEvts, evts3...)
	}

	if len(allEvts) == 0 {
		return fmt.Errorf("no events found; use --stream to specify event log location")
	}

	// Find provenance chain
	chain := causal.Provenance(allEvts, elementID)

	if len(chain) == 0 {
		fmt.Printf("No events found referencing %s\n", elementID)
		return nil
	}

	// Output
	if blameJSON {
		type blameEntry struct {
			EventID   string `json:"event_id"`
			Type      string `json:"type"`
			Timestamp string `json:"timestamp"`
			Stream    int    `json:"stream"`
		}
		var entries []blameEntry
		for _, e := range chain {
			entries = append(entries, blameEntry{
				EventID:   e.ID,
				Type:      e.Type,
				Timestamp: e.Timestamp,
				Stream:    int(e.Stream),
			})
		}
		data, _ := json.MarshalIndent(entries, "", "  ")
		fmt.Println(string(data))
	} else {
		fmt.Printf("Provenance for %s (%d events):\n\n", elementID, len(chain))
		for i, e := range chain {
			prefix := "  "
			if i == 0 {
				prefix = "→ "
			}
			fmt.Printf("%s%s  %s  [%s]  stream-%d\n", prefix, e.Timestamp, e.ID, e.Type, int(e.Stream))
		}
	}

	return nil
}
