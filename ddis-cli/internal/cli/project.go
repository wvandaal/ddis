package cli

// ddis:implements APP-INV-076 (projection purity — CLI wiring for project command)
// ddis:implements APP-INV-077 (synthetic render — structured fields, not raw_text)
// ddis:implements APP-ADR-061 (field synthesis for projections)

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/projector"
	"github.com/wvandaal/ddis/internal/storage"
)

var (
	projectOutput string
	projectModule string
	projectFormat string
)

var projectCmd = &cobra.Command{
	Use:   "project [db-path]",
	Short: "Render SQLite state into markdown projections",
	Long: `Projects materialized SQLite state into markdown specifications.

Projections are pure functions of SQL state (APP-INV-076): no I/O beyond
reading the database and writing output files. All content is synthesized
from structured fields (title, statement, problem, etc.), never from
raw_text blobs (APP-INV-077).

This is the final stage of the JSONL → SQLite → Markdown pipeline.

Examples:
  ddis project index.db -o ./output/
  ddis project index.db --module parse-pipeline -o parse-pipeline.md
  ddis project index.db --format bundle -o ./bundle/`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runProject,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	projectCmd.Flags().StringVarP(&projectOutput, "output", "o", "", "Output directory or file path")
	projectCmd.Flags().StringVar(&projectModule, "module", "", "Project only this module")
	projectCmd.Flags().StringVar(&projectFormat, "format", "markdown", "Output format: markdown, bundle")
}

func runProject(cmd *cobra.Command, args []string) error {
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

	// Get modules to project
	modules, err := queryModulesForProject(db, specID)
	if err != nil {
		return fmt.Errorf("query modules: %w", err)
	}

	if projectModule != "" {
		filtered := make([]projector.ModuleSpec, 0)
		for _, m := range modules {
			if m.Name == projectModule {
				filtered = append(filtered, m)
			}
		}
		if len(filtered) == 0 {
			return fmt.Errorf("module %q not found", projectModule)
		}
		modules = filtered
	}

	// Determine output
	outDir := projectOutput
	if outDir == "" {
		outDir = "."
	}

	// Render each module
	for _, mod := range modules {
		rendered := projector.RenderModule(mod)

		if projectModule != "" && projectOutput != "" {
			// Single module to single file
			if err := os.WriteFile(projectOutput, []byte(rendered), 0o644); err != nil {
				return fmt.Errorf("write %s: %w", projectOutput, err)
			}
			fmt.Printf("Projected %s → %s\n", mod.Name, projectOutput)
		} else {
			// Each module to its own file in output dir
			if err := os.MkdirAll(outDir, 0o755); err != nil {
				return fmt.Errorf("create output dir: %w", err)
			}
			outPath := filepath.Join(outDir, mod.Name+".md")
			if err := os.WriteFile(outPath, []byte(rendered), 0o644); err != nil {
				return fmt.Errorf("write %s: %w", outPath, err)
			}
			fmt.Printf("Projected %s → %s\n", mod.Name, outPath)
		}
	}

	if !NoGuidance {
		fmt.Fprintln(os.Stderr, "\nNext: ddis validate "+dbPath)
	}

	return nil
}

// resolveDBPath resolves the database path from args or auto-discovery.
func resolveDBPath(args []string) (string, error) {
	if globalDBPath != "" {
		return globalDBPath, nil
	}
	if len(args) > 0 {
		return args[0], nil
	}
	return FindDB()
}

// queryModulesForProject queries modules and their invariants/ADRs for projection.
func queryModulesForProject(db storage.DB, specID int64) ([]projector.ModuleSpec, error) {
	rows, err := db.Query(`SELECT module_name, domain FROM modules WHERE spec_id = ?`, specID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var modules []projector.ModuleSpec
	for rows.Next() {
		var mod projector.ModuleSpec
		if err := rows.Scan(&mod.Name, &mod.Domain); err != nil {
			return nil, err
		}

		// Load invariants for this module
		invRows, err := db.Query(`SELECT invariant_id, title, statement, semi_formal, violation_scenario, validation_method, why_this_matters
			FROM invariants WHERE spec_id = ? AND invariant_id LIKE 'APP-INV-%'
			ORDER BY invariant_id`, specID)
		if err == nil {
			for invRows.Next() {
				var inv projector.Invariant
				invRows.Scan(&inv.ID, &inv.Title, &inv.Statement, &inv.SemiFormal,
					&inv.ViolationScenario, &inv.ValidationMethod, &inv.WhyThisMatters)
				mod.Invariants = append(mod.Invariants, inv)
			}
			invRows.Close()
		}

		// Load ADRs for this module
		adrRows, err := db.Query(`SELECT adr_id, title, problem, decision_text, consequences, tests
			FROM adrs WHERE spec_id = ? AND adr_id LIKE 'APP-ADR-%'
			ORDER BY adr_id`, specID)
		if err == nil {
			for adrRows.Next() {
				var adr projector.ADR
				adrRows.Scan(&adr.ID, &adr.Title, &adr.Problem, &adr.Decision, &adr.Consequences, &adr.Tests)
				mod.ADRs = append(mod.ADRs, adr)
			}
			adrRows.Close()
		}

		modules = append(modules, mod)
	}

	return modules, nil
}
