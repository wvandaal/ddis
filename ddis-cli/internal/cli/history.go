package cli

// ddis:maintains APP-INV-020 (event stream append-only — unified timeline)

import (
	"encoding/json"
	"fmt"
	"os"
	"sort"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/storage"
)

var (
	historyJSON  bool
	historyLimit int
)

var historyCmd = &cobra.Command{
	Use:   "history [index.db]",
	Short: "Unified event timeline from spec index",
	Long: `Displays a unified timeline of all events recorded in the spec database:
parse events, transactions, witness recordings, and validation runs.

Unlike 'ddis log' (which reads the oplog JSONL), 'ddis history' reads directly
from the SQLite database to show the complete picture.

Examples:
  ddis history
  ddis history manifest.ddis.db
  ddis history --json
  ddis history --limit 20`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runHistory,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	historyCmd.Flags().BoolVar(&historyJSON, "json", false, "JSON output")
	historyCmd.Flags().IntVar(&historyLimit, "limit", 50, "Maximum events to display")
}

type historyEvent struct {
	Timestamp string `json:"timestamp"`
	Type      string `json:"type"`
	Summary   string `json:"summary"`
}

func runHistory(cmd *cobra.Command, args []string) error {
	var dbPath string
	if len(args) >= 1 {
		dbPath = args[0]
	}
	if dbPath == "" {
		var err error
		dbPath, err = FindDB()
		if err != nil {
			return err
		}
	}

	db, err := storage.OpenExisting(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	var events []historyEvent

	// 1. Parse events from spec_index.
	rows, err := db.Query("SELECT id, spec_name, parsed_at, source_type FROM spec_index ORDER BY id")
	if err == nil {
		defer rows.Close()
		for rows.Next() {
			var id int64
			var name, parsedAt, sourceType string
			if rows.Scan(&id, &name, &parsedAt, &sourceType) == nil {
				events = append(events, historyEvent{
					Timestamp: parsedAt,
					Type:      "parse",
					Summary:   fmt.Sprintf("Parsed %s (id=%d, type=%s)", name, id, sourceType),
				})
			}
		}
	}

	// 2. Transaction events.
	specID, _ := storage.GetFirstSpecID(db)
	if specID > 0 {
		txns, err := storage.ListTransactions(db, specID)
		if err == nil {
			for _, tx := range txns {
				events = append(events, historyEvent{
					Timestamp: tx.CreatedAt,
					Type:      "transaction",
					Summary:   fmt.Sprintf("TX %s: %s (status: %s)", tx.TxID, tx.Description, tx.Status),
				})
			}
		}

		// 3. Witness events.
		witnesses, err := storage.ListWitnesses(db, specID)
		if err == nil {
			for _, w := range witnesses {
				events = append(events, historyEvent{
					Timestamp: w.ProvenAt,
					Type:      "witness",
					Summary: fmt.Sprintf("Witnessed %s (%s by %s, status: %s)",
						w.InvariantID, w.EvidenceType, w.ProvenBy, w.Status),
				})
			}
		}
	}

	// Sort by timestamp descending (most recent first).
	sort.Slice(events, func(i, j int) bool {
		return events[i].Timestamp > events[j].Timestamp
	})

	// Apply limit.
	if historyLimit > 0 && len(events) > historyLimit {
		events = events[:historyLimit]
	}

	if historyJSON {
		enc := json.NewEncoder(os.Stdout)
		enc.SetIndent("", "  ")
		return enc.Encode(events)
	}

	fmt.Printf("Event History (%d events)\n", len(events))
	for _, e := range events {
		fmt.Printf("  %s  [%-11s]  %s\n", e.Timestamp, e.Type, e.Summary)
	}

	if !NoGuidance {
		fmt.Println("\nNext: ddis log .ddis/oplog.jsonl")
		fmt.Println("  See the full oplog for diffs and validation records.")
	}

	return nil
}
