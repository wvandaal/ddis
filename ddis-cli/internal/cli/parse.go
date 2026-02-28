package cli

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:implements APP-ADR-001 (monolith-first parsing)
// ddis:implements APP-ADR-068 (phased migration — parse handles legacy markdown, new changes via crystallize events)
// ddis:implements APP-INV-089 (deprecation compatibility bridge — parse wraps import+materialize path)
// ddis:maintains APP-INV-001 (round-trip fidelity)
// ddis:maintains APP-INV-009 (monolith-modular equivalence)
// ddis:maintains APP-INV-041 (witness auto-invalidation — triggers InvalidateWitnesses on re-parse)

var (
	parseOutput string
)

var parseCmd = &cobra.Command{
	Use:   "parse <spec.md|manifest.yaml>",
	Short: "Parse a DDIS spec into a structured index",
	Args:  cobra.ExactArgs(1),
	RunE:  runParse,
}

func init() {
	parseCmd.Flags().StringVarP(&parseOutput, "output", "o", "", "Output database path (default: <spec>.ddis.db)")
}

func runParse(cmd *cobra.Command, args []string) error {
	// APP-INV-089: deprecation bridge — parse still works but suggests event pipeline
	fmt.Fprintln(os.Stderr, "Note: 'ddis parse' is the legacy path. For new content, prefer: ddis crystallize → ddis materialize → ddis project")

	specPath := args[0]

	// Determine output path
	dbPath := parseOutput
	if dbPath == "" {
		ext := filepath.Ext(specPath)
		dbPath = strings.TrimSuffix(specPath, ext) + ".ddis.db"
	}

	// Open database
	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	// Reset parser diagnostics before parsing
	parser.ResetDiagnostics()

	// Detect monolith vs modular
	var specID int64
	basename := filepath.Base(specPath)
	if basename == "manifest.yaml" || basename == "manifest.yml" {
		fmt.Printf("Parsing modular spec from %s...\n", specPath)
		specID, err = parser.ParseModularSpec(specPath, db)
	} else {
		fmt.Printf("Parsing monolith spec from %s...\n", specPath)
		specID, err = parser.ParseDocument(specPath, db)
	}

	if err != nil {
		return fmt.Errorf("parse: %w", err)
	}

	// Print parser diagnostics (incomplete elements) to stderr
	if diags := parser.GlobalDiagnostics.All(); len(diags) > 0 {
		for _, d := range diags {
			d.FilePath = specPath
			fmt.Fprintln(os.Stderr, parser.FormatDiagnostic(d))
		}
	}

	// Auto-invalidate stale witnesses
	// Witnesses are stored against the first (canonical) spec_id; compare against fresh parse
	firstSpecID, _ := storage.GetFirstSpecID(db)
	if firstSpecID == 0 {
		firstSpecID = specID
	}
	if staleCount, err := storage.InvalidateWitnesses(db, firstSpecID, specID); err == nil && staleCount > 0 {
		fmt.Printf("  Witnesses invalidated: %d (spec changed)\n", staleCount)
	}

	// Print summary
	printParseSummary(db, specID)

	// Build search index (FTS5 + LSI + PageRank)
	fmt.Println("\nBuilding search index...")
	if err := search.BuildIndex(db, specID); err != nil {
		fmt.Printf("  Warning: search index build failed: %v\n", err)
	} else {
		printSearchSummary(db, specID)
	}

	fmt.Printf("\nIndex written to %s\n", dbPath)

	// Emit spec_parsed event to Stream 2 (Specification).
	emitEvent(dbPath, events.StreamSpecification, events.TypeSpecParsed, specHashFromDB(db, specID), map[string]interface{}{
		"spec_path":   specPath,
		"spec_id":     specID,
		"source_type": func() string { if basename == "manifest.yaml" || basename == "manifest.yml" { return "modular" }; return "monolith" }(),
	})

	// Guidance postscript
	if !NoGuidance {
		// Check for unresolved cross-refs
		var unresolved int
		_ = db.QueryRow("SELECT COUNT(*) FROM cross_references WHERE spec_id = ? AND resolved = 0", specID).Scan(&unresolved)
		if unresolved > 0 {
			fmt.Printf("\nNext: ddis validate --checks 1\n")
			fmt.Printf("  %d unresolved cross-references detected.\n", unresolved)
		} else {
			fmt.Println("\nNext: ddis validate && ddis coverage")
		}
	}
	return nil
}

func printSearchSummary(db storage.DB, specID int64) {
	var ftsCount int
	if err := db.QueryRow("SELECT COUNT(*) FROM fts_index").Scan(&ftsCount); err == nil {
		fmt.Printf("  FTS5 documents:   %d\n", ftsCount)
	}
	var authCount int
	if err := db.QueryRow("SELECT COUNT(*) FROM search_authority WHERE spec_id = ?", specID).Scan(&authCount); err == nil {
		fmt.Printf("  Authority nodes:  %d\n", authCount)
	}
	var modelCount int
	if err := db.QueryRow("SELECT COUNT(*) FROM search_model WHERE spec_id = ?", specID).Scan(&modelCount); err == nil && modelCount > 0 {
		var k, terms, docs int
		_ = db.QueryRow("SELECT k_dimensions, term_count, doc_count FROM search_model WHERE spec_id = ? AND model_type = 'lsi'", specID).Scan(&k, &terms, &docs)
		fmt.Printf("  LSI model:        k=%d, %d terms, %d docs\n", k, terms, docs)
	}
}

func printParseSummary(db storage.DB, specID int64) {
	type countQuery struct {
		label string
		query string
	}

	queries := []countQuery{
		{"Sections", "SELECT COUNT(*) FROM sections WHERE spec_id = ?"},
		{"Invariants", "SELECT COUNT(*) FROM invariants WHERE spec_id = ?"},
		{"ADRs", "SELECT COUNT(*) FROM adrs WHERE spec_id = ?"},
		{"Quality Gates", "SELECT COUNT(*) FROM quality_gates WHERE spec_id = ?"},
		{"Negative Specs", "SELECT COUNT(*) FROM negative_specs WHERE spec_id = ?"},
		{"Verification Prompts", "SELECT COUNT(*) FROM verification_prompts WHERE spec_id = ?"},
		{"Meta-Instructions", "SELECT COUNT(*) FROM meta_instructions WHERE spec_id = ?"},
		{"Worked Examples", "SELECT COUNT(*) FROM worked_examples WHERE spec_id = ?"},
		{"WHY NOT Annotations", "SELECT COUNT(*) FROM why_not_annotations WHERE spec_id = ?"},
		{"Comparison Blocks", "SELECT COUNT(*) FROM comparison_blocks WHERE spec_id = ?"},
		{"Performance Budgets", "SELECT COUNT(*) FROM performance_budgets WHERE spec_id = ?"},
		{"State Machines", "SELECT COUNT(*) FROM state_machines WHERE spec_id = ?"},
		{"Glossary Entries", "SELECT COUNT(*) FROM glossary_entries WHERE spec_id = ?"},
		{"Cross-References", "SELECT COUNT(*) FROM cross_references WHERE spec_id = ?"},
		{"  Resolved", "SELECT COUNT(*) FROM cross_references WHERE spec_id = ? AND resolved = 1"},
		{"  Unresolved", "SELECT COUNT(*) FROM cross_references WHERE spec_id = ? AND resolved = 0"},
	}

	fmt.Println("\nParse Summary:")
	fmt.Println("─────────────────────────────")
	for _, q := range queries {
		var count int
		if err := db.QueryRow(q.query, specID).Scan(&count); err != nil {
			continue
		}
		fmt.Printf("  %-22s %d\n", q.label, count)
	}
}
