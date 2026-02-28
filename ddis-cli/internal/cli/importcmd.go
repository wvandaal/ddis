package cli

// ddis:implements APP-INV-078 (import equivalence — markdown to JSONL event conversion)
// ddis:implements APP-INV-085 (import content completeness — emits events for ALL content types)
// ddis:implements APP-ADR-062 (parse as import migration path)

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/storage"
)

var (
	importOutput  string
	importSynth   bool
	importJSON    bool
)

var importCmd = &cobra.Command{
	Use:   "import [db-path]",
	Short: "Convert parsed spec into JSONL events",
	Long: `Reads a parsed SQLite database and emits synthetic JSONL events that,
when materialized, would reproduce the same state.

This is the migration bridge: existing markdown specs parsed via 'ddis parse'
can be converted to event streams. The key invariant (APP-INV-078) is:

  materialize(import(parse(markdown))) ≈ parse(markdown)

Synthetic events carry the 'synthetic: true' flag for provenance tracking.

Examples:
  ddis import index.db -o .ddis/events/stream-2.jsonl
  ddis import index.db -o events.jsonl --json
  ddis import manifest.ddis.db -o .ddis/events/stream-2.jsonl`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runImport,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	importCmd.Flags().StringVarP(&importOutput, "output", "o", "", "Output JSONL path (default: .ddis/events/stream-2.jsonl)")
	importCmd.Flags().BoolVar(&importSynth, "synthetic", true, "Mark events as synthetic (default: true)")
	importCmd.Flags().BoolVar(&importJSON, "json", false, "Output summary as JSON")
}

func runImport(cmd *cobra.Command, args []string) error {
	// Resolve DB path
	dbPath, err := resolveDBPath(args)
	if err != nil {
		return err
	}

	db, err := storage.OpenExisting(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	specHash := specHashFromDB(db, specID)

	// Determine output path
	outPath := importOutput
	if outPath == "" {
		wsRoot := events.WorkspaceRoot(dbPath)
		dir := events.StreamDir(wsRoot)
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return fmt.Errorf("create events dir: %w", err)
		}
		outPath = events.StreamPath(wsRoot, events.StreamSpecification)
	}

	var count int

	// Emit module events
	modRows, err := db.Query(`SELECT module_name, domain FROM modules WHERE spec_id = ?`, specID)
	if err == nil {
		defer modRows.Close()
		for modRows.Next() {
			var name, domain string
			if err := modRows.Scan(&name, &domain); err != nil {
				continue
			}
			payload := events.ModulePayload{Name: name, Domain: domain}
			evt, err := events.NewEvent(events.StreamSpecification, events.TypeModuleRegistered, specHash, payload)
			if err != nil {
				continue
			}
			if err := events.AppendEvent(outPath, evt); err != nil {
				fmt.Fprintf(os.Stderr, "import: append module %s: %v\n", name, err)
				continue
			}
			count++
		}
	}

	// Emit invariant events
	invRows, err := db.Query(`SELECT invariant_id, title, statement, COALESCE(semi_formal,''), COALESCE(violation_scenario,''), COALESCE(validation_method,''), COALESCE(why_this_matters,'')
		FROM invariants WHERE spec_id = ? ORDER BY invariant_id`, specID)
	if err == nil {
		defer invRows.Close()
		for invRows.Next() {
			var p events.InvariantPayload
			if err := invRows.Scan(&p.ID, &p.Title, &p.Statement, &p.SemiFormal, &p.ViolationScenario, &p.ValidationMethod, &p.WhyThisMatters); err != nil {
				continue
			}
			p.Synthetic = importSynth
			evt, err := events.NewEvent(events.StreamSpecification, events.TypeInvariantCrystallized, specHash, p)
			if err != nil {
				continue
			}
			if err := events.AppendEvent(outPath, evt); err != nil {
				fmt.Fprintf(os.Stderr, "import: append invariant %s: %v\n", p.ID, err)
				continue
			}
			count++
		}
	}

	// Emit ADR events
	adrRows, err := db.Query(`SELECT adr_id, title, problem, decision_text, COALESCE(consequences,''), COALESCE(tests,'')
		FROM adrs WHERE spec_id = ? ORDER BY adr_id`, specID)
	if err == nil {
		defer adrRows.Close()
		for adrRows.Next() {
			var p events.ADRPayload
			if err := adrRows.Scan(&p.ID, &p.Title, &p.Problem, &p.Decision, &p.Consequences, &p.Tests); err != nil {
				continue
			}
			p.Synthetic = importSynth
			evt, err := events.NewEvent(events.StreamSpecification, events.TypeADRCrystallized, specHash, p)
			if err != nil {
				continue
			}
			if err := events.AppendEvent(outPath, evt); err != nil {
				fmt.Fprintf(os.Stderr, "import: append ADR %s: %v\n", p.ID, err)
				continue
			}
			count++
		}
	}

	// Emit glossary events
	glossRows, err := db.Query(`SELECT term, definition FROM glossary_entries WHERE spec_id = ?`, specID)
	if err == nil {
		defer glossRows.Close()
		for glossRows.Next() {
			var p events.GlossaryTermPayload
			if err := glossRows.Scan(&p.Term, &p.Definition); err != nil {
				continue
			}
			evt, err := events.NewEvent(events.StreamSpecification, events.TypeGlossaryTermDefined, specHash, p)
			if err != nil {
				continue
			}
			if err := events.AppendEvent(outPath, evt); err != nil {
				continue
			}
			count++
		}
	}

	// Emit section events (APP-INV-085: content completeness)
	secRows, err := db.Query(`SELECT section_path, title, heading_level, COALESCE(raw_text,'')
		FROM sections WHERE spec_id = ? ORDER BY section_path`, specID)
	if err == nil {
		defer secRows.Close()
		for secRows.Next() {
			var p events.SectionPayload
			if err := secRows.Scan(&p.Path, &p.Title, &p.Level, &p.Body); err != nil {
				continue
			}
			evt, err := events.NewEvent(events.StreamSpecification, events.TypeSpecSectionDefined, specHash, p)
			if err != nil {
				continue
			}
			if err := events.AppendEvent(outPath, evt); err != nil {
				continue
			}
			count++
		}
	}

	// Emit cross-reference events (APP-INV-085: content completeness)
	xrefRows, err := db.Query(`SELECT ref_text, ref_target
		FROM cross_references WHERE spec_id = ? ORDER BY id`, specID)
	if err == nil {
		defer xrefRows.Close()
		for xrefRows.Next() {
			var p events.CrossRefPayload
			if err := xrefRows.Scan(&p.Source, &p.Target); err != nil {
				continue
			}
			evt, err := events.NewEvent(events.StreamSpecification, events.TypeCrossRefAdded, specHash, p)
			if err != nil {
				continue
			}
			if err := events.AppendEvent(outPath, evt); err != nil {
				continue
			}
			count++
		}
	}

	// Emit negative spec events (APP-INV-085: content completeness)
	negRows, err := db.Query(`SELECT constraint_text, COALESCE(reason,'')
		FROM negative_specs WHERE spec_id = ? ORDER BY id`, specID)
	if err == nil {
		defer negRows.Close()
		for negRows.Next() {
			var p events.NegativeSpecPayload
			if err := negRows.Scan(&p.Pattern, &p.Rationale); err != nil {
				continue
			}
			evt, err := events.NewEvent(events.StreamSpecification, events.TypeNegativeSpecAdded, specHash, p)
			if err != nil {
				continue
			}
			if err := events.AppendEvent(outPath, evt); err != nil {
				continue
			}
			count++
		}
	}

	// Emit quality gate events (APP-INV-085: content completeness)
	gateRows, err := db.Query(`SELECT gate_id, title, predicate
		FROM quality_gates WHERE spec_id = ? ORDER BY gate_id`, specID)
	if err == nil {
		defer gateRows.Close()
		for gateRows.Next() {
			var gateID, title, predicate string
			if err := gateRows.Scan(&gateID, &title, &predicate); err != nil {
				continue
			}
			p := events.QualityGatePayload{Title: title, Predicate: predicate}
			evt, err := events.NewEvent(events.StreamSpecification, events.TypeQualityGateDefined, specHash, p)
			if err != nil {
				continue
			}
			if err := events.AppendEvent(outPath, evt); err != nil {
				continue
			}
			count++
		}
	}

	// Emit witness events
	witRows, err := db.Query(`SELECT invariant_id, spec_hash, COALESCE(code_hash,''), evidence_type, evidence, proven_by, COALESCE(model,'')
		FROM invariant_witnesses WHERE spec_id = ? AND status = 'valid'`, specID)
	if err == nil {
		defer witRows.Close()
		for witRows.Next() {
			var p events.WitnessPayload
			if err := witRows.Scan(&p.InvariantID, &p.SpecHash, &p.CodeHash, &p.EvidenceType, &p.Evidence, &p.By, &p.Model); err != nil {
				continue
			}
			evt, err := events.NewEvent(events.StreamImplementation, events.TypeWitnessRecorded, specHash, p)
			if err != nil {
				continue
			}
			if err := events.AppendEvent(outPath, evt); err != nil {
				continue
			}
			count++
		}
	}

	// Report
	if importJSON {
		fmt.Printf(`{"events_emitted":%d,"output":"%s"}`, count, outPath)
		fmt.Println()
	} else {
		fmt.Printf("Imported %d events → %s\n", count, outPath)
	}

	if !NoGuidance {
		fmt.Fprintln(os.Stderr, "\nNext: ddis materialize "+outPath)
	}

	return nil
}
