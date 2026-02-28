package cli

// ddis:implements APP-INV-073 (fold determinism — CLI wiring for materialize command)
// ddis:implements APP-INV-075 (materialization idempotency — replay produces identical state)
// ddis:implements APP-ADR-059 (deterministic fold over incremental mutation)

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/materialize"
	"github.com/wvandaal/ddis/internal/storage"
)

var (
	materializeOutput   string
	materializeFromSnap bool
	materializeJSON     bool
)

var materializeCmd = &cobra.Command{
	Use:   "materialize [stream-path]",
	Short: "Replay JSONL event log into SQLite state",
	Long: `Replays a JSONL event stream through the deterministic fold engine,
producing a SQLite materialized view.

The fold function is pure: same event sequence always produces identical SQLite
state (APP-INV-073). The resulting database is disposable — it can always be
recreated from the event log (APP-INV-075).

Events are causally sorted before folding: the causes field on each event
defines a partial order, and Kahn's algorithm produces a deterministic
topological ordering (APP-INV-074).

Examples:
  ddis materialize .ddis/events/stream-2.jsonl -o index.db
  ddis materialize .ddis/events/stream-2.jsonl -o index.db --json
  ddis materialize .ddis/events/stream-2.jsonl  # default: .ddis/index.db`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runMaterialize,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	materializeCmd.Flags().StringVarP(&materializeOutput, "output", "o", "", "Output database path (default: .ddis/index.db)")
	materializeCmd.Flags().BoolVar(&materializeFromSnap, "from-snapshot", false, "Resume from latest snapshot checkpoint")
	materializeCmd.Flags().BoolVar(&materializeJSON, "json", false, "Output result as JSON")
}

func runMaterialize(cmd *cobra.Command, args []string) error {
	// Determine stream path
	var streamPath string
	if len(args) > 0 {
		streamPath = args[0]
	} else {
		// Auto-discover: look for .ddis/events/stream-2.jsonl
		wsRoot := "."
		streamPath = events.StreamPath(wsRoot, events.StreamSpecification)
		if _, err := os.Stat(streamPath); os.IsNotExist(err) {
			return fmt.Errorf("no event stream found at %s\nTip: specify a stream path or ensure .ddis/events/stream-2.jsonl exists", streamPath)
		}
	}

	// Read all content events from the stream
	evts, err := events.ReadStream(streamPath, events.EventFilters{})
	if err != nil {
		return fmt.Errorf("read stream %s: %w", streamPath, err)
	}

	if len(evts) == 0 {
		return fmt.Errorf("no events found in %s", streamPath)
	}

	// Filter to content-bearing event types only
	var contentEvts []*events.Event
	for _, e := range evts {
		if isContentEvent(e.Type) {
			contentEvts = append(contentEvts, e)
		}
	}

	if len(contentEvts) == 0 {
		fmt.Fprintf(os.Stderr, "No content-bearing events in %s (%d total events skipped)\n", streamPath, len(evts))
		return nil
	}

	// Determine output path
	dbPath := materializeOutput
	if dbPath == "" {
		dbPath = ".ddis/index.db"
	}

	// Create fresh database
	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("create database %s: %w", dbPath, err)
	}
	defer db.Close()

	// Create the SQL applier
	applier := &sqlApplier{db: db}

	// Run the fold
	result, err := materialize.Fold(applier, contentEvts)
	if err != nil {
		return fmt.Errorf("fold: %w", err)
	}

	// Report results
	if materializeJSON {
		fmt.Printf(`{"events_processed":%d,"events_skipped":%d,"errors":%d}`,
			result.EventsProcessed, result.EventsSkipped, len(result.Errors))
		fmt.Println()
	} else {
		fmt.Printf("Materialized %d events into %s\n", result.EventsProcessed, dbPath)
		if result.EventsSkipped > 0 {
			fmt.Printf("  Skipped: %d events with errors\n", result.EventsSkipped)
			for _, fe := range result.Errors {
				fmt.Fprintf(os.Stderr, "  %s (%s): %v\n", fe.EventID, fe.EventType, fe.Err)
			}
		}
	}

	if !NoGuidance {
		fmt.Fprintln(os.Stderr, "\nNext: ddis validate "+dbPath)
	}

	return nil
}

// isContentEvent returns true for event types that carry spec content mutations.
func isContentEvent(t string) bool {
	switch t {
	case events.TypeSpecSectionDefined,
		events.TypeSpecSectionUpdated,
		events.TypeSpecSectionRemoved,
		events.TypeInvariantCrystallized,
		events.TypeInvariantUpdated,
		events.TypeInvariantRemoved,
		events.TypeADRCrystallized,
		events.TypeADRUpdated,
		events.TypeADRSuperseded,
		events.TypeNegativeSpecAdded,
		events.TypeCrossRefAdded,
		events.TypeGlossaryTermDefined,
		events.TypeModuleRegistered,
		events.TypeWitnessRecorded,
		events.TypeWitnessRevoked,
		events.TypeWitnessInvalidated,
		events.TypeChallengeCompleted:
		return true
	}
	return false
}

// sqlApplier implements materialize.Applier using the storage package.
type sqlApplier struct {
	db storage.DB
}

func (a *sqlApplier) InsertSection(p events.SectionPayload) error {
	// Minimal stub: insert section into sections table
	_, err := a.db.Exec(`INSERT OR REPLACE INTO sections (spec_id, source_file_id, section_path, title, heading_level, line_start, line_end, raw_text, content_hash)
		VALUES (1, 1, ?, ?, ?, 0, 0, ?, '')`,
		p.Path, p.Title, p.Level, p.Body)
	return err
}

func (a *sqlApplier) UpdateSection(p events.SectionUpdatePayload) error {
	_, err := a.db.Exec(`UPDATE sections SET title = ?, raw_text = ? WHERE spec_id = 1 AND section_path = ?`,
		p.Title, p.Body, p.Path)
	return err
}

func (a *sqlApplier) RemoveSection(p events.SectionRemovePayload) error {
	_, err := a.db.Exec(`DELETE FROM sections WHERE spec_id = 1 AND section_path = ?`, p.Path)
	return err
}

func (a *sqlApplier) InsertInvariant(p events.InvariantPayload) error {
	_, err := a.db.Exec(`INSERT OR REPLACE INTO invariants (spec_id, source_file_id, section_id, invariant_id, title, statement, semi_formal, violation_scenario, validation_method, why_this_matters, line_start, line_end, raw_text, content_hash)
		VALUES (1, 1, 1, ?, ?, ?, ?, ?, ?, ?, 0, 0, '', '')`,
		p.ID, p.Title, p.Statement, p.SemiFormal, p.ViolationScenario, p.ValidationMethod, p.WhyThisMatters)
	return err
}

func (a *sqlApplier) UpdateInvariant(p events.InvariantUpdatePayload) error {
	for field, val := range p.NewValues {
		switch field {
		case "title":
			a.db.Exec(`UPDATE invariants SET title = ? WHERE spec_id = 1 AND invariant_id = ?`, val, p.ID)
		case "statement":
			a.db.Exec(`UPDATE invariants SET statement = ? WHERE spec_id = 1 AND invariant_id = ?`, val, p.ID)
		case "semi_formal":
			a.db.Exec(`UPDATE invariants SET semi_formal = ? WHERE spec_id = 1 AND invariant_id = ?`, val, p.ID)
		}
	}
	return nil
}

func (a *sqlApplier) RemoveInvariant(p events.InvariantRemovePayload) error {
	_, err := a.db.Exec(`DELETE FROM invariants WHERE spec_id = 1 AND invariant_id = ?`, p.ID)
	return err
}

func (a *sqlApplier) InsertADR(p events.ADRPayload) error {
	_, err := a.db.Exec(`INSERT OR REPLACE INTO adrs (spec_id, source_file_id, section_id, adr_id, title, problem, decision_text, consequences, tests, line_start, line_end, raw_text, content_hash)
		VALUES (1, 1, 1, ?, ?, ?, ?, ?, ?, 0, 0, '', '')`,
		p.ID, p.Title, p.Problem, p.Decision, p.Consequences, p.Tests)
	return err
}

func (a *sqlApplier) UpdateADR(p events.ADRUpdatePayload) error {
	for field, val := range p.NewValues {
		switch field {
		case "title":
			a.db.Exec(`UPDATE adrs SET title = ? WHERE spec_id = 1 AND adr_id = ?`, val, p.ID)
		case "decision":
			a.db.Exec(`UPDATE adrs SET decision_text = ? WHERE spec_id = 1 AND adr_id = ?`, val, p.ID)
		}
	}
	return nil
}

func (a *sqlApplier) SupersedeADR(p events.ADRSupersededPayload) error {
	_, err := a.db.Exec(`UPDATE adrs SET status = 'superseded', superseded_by = ? WHERE spec_id = 1 AND adr_id = ?`,
		p.SupersededBy, p.ID)
	return err
}

func (a *sqlApplier) InsertWitness(p events.WitnessPayload) error {
	_, err := a.db.Exec(`INSERT OR REPLACE INTO invariant_witnesses (spec_id, invariant_id, spec_hash, code_hash, evidence_type, evidence, proven_by, model)
		VALUES (1, ?, ?, ?, ?, ?, ?, ?)`,
		p.InvariantID, p.SpecHash, p.CodeHash, p.EvidenceType, p.Evidence, p.By, p.Model)
	return err
}

func (a *sqlApplier) RevokeWitness(p events.WitnessRevokePayload) error {
	_, err := a.db.Exec(`UPDATE invariant_witnesses SET status = 'invalidated', notes = ? WHERE spec_id = 1 AND invariant_id = ?`,
		p.Reason, p.InvariantID)
	return err
}

func (a *sqlApplier) InsertChallenge(p events.ChallengePayload) error {
	_, err := a.db.Exec(`INSERT OR REPLACE INTO challenge_results (spec_id, invariant_id, verdict, challenged_by)
		VALUES (1, ?, ?, 'system')`,
		p.InvariantID, p.Verdict)
	return err
}

func (a *sqlApplier) InsertModule(p events.ModulePayload) error {
	_, err := a.db.Exec(`INSERT OR REPLACE INTO modules (spec_id, source_file_id, module_name, domain, line_count)
		VALUES (1, 1, ?, ?, 0)`,
		p.Name, p.Domain)
	return err
}

func (a *sqlApplier) InsertGlossaryTerm(p events.GlossaryTermPayload) error {
	_, err := a.db.Exec(`INSERT OR REPLACE INTO glossary_entries (spec_id, section_id, term, definition, line_number)
		VALUES (1, 1, ?, ?, 0)`,
		p.Term, p.Definition)
	return err
}

func (a *sqlApplier) InsertCrossRef(p events.CrossRefPayload) error {
	_, err := a.db.Exec(`INSERT INTO cross_references (spec_id, source_file_id, source_line, ref_type, ref_target, ref_text, resolved)
		VALUES (1, 1, 0, 'section', ?, ?, 0)`,
		p.Target, p.Source)
	return err
}

func (a *sqlApplier) InsertNegativeSpec(p events.NegativeSpecPayload) error {
	_, err := a.db.Exec(`INSERT INTO negative_specs (spec_id, source_file_id, section_id, constraint_text, reason, line_number, raw_text)
		VALUES (1, 1, 1, ?, ?, 0, '')`,
		p.Pattern, p.Rationale)
	return err
}
