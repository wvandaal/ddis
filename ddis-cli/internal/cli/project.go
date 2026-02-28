package cli

// ddis:implements APP-INV-076 (projection purity — CLI wiring for project command)
// ddis:implements APP-INV-077 (synthetic render — structured fields, not raw_text)
// ddis:implements APP-INV-087 (projector section rendering — filters by module ownership)
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
// Filters invariants and ADRs by module ownership via module_relationships (APP-INV-087).
func queryModulesForProject(db storage.DB, specID int64) ([]projector.ModuleSpec, error) {
	rows, err := db.Query(`SELECT id, module_name, domain FROM modules WHERE spec_id = ?`, specID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var modules []projector.ModuleSpec
	for rows.Next() {
		var moduleID int64
		var mod projector.ModuleSpec
		if err := rows.Scan(&moduleID, &mod.Name, &mod.Domain); err != nil {
			return nil, err
		}

		// Get the list of maintained invariant IDs for this module
		maintainsIDs := getModuleRelationshipTargets(db, moduleID, "maintains")

		// Load invariants filtered by module ownership (APP-INV-087)
		if len(maintainsIDs) > 0 {
			for _, invID := range maintainsIDs {
				invRows, err := db.Query(`SELECT invariant_id, title, statement, COALESCE(semi_formal,''), COALESCE(violation_scenario,''), COALESCE(validation_method,''), COALESCE(why_this_matters,'')
					FROM invariants WHERE spec_id = ? AND invariant_id = ?`, specID, invID)
				if err == nil {
					for invRows.Next() {
						var inv projector.Invariant
						invRows.Scan(&inv.ID, &inv.Title, &inv.Statement, &inv.SemiFormal,
							&inv.ViolationScenario, &inv.ValidationMethod, &inv.WhyThisMatters)
						mod.Invariants = append(mod.Invariants, inv)
					}
					invRows.Close()
				}
			}
		}

		// Get the list of implemented ADR IDs for this module
		implementsIDs := getModuleRelationshipTargets(db, moduleID, "implements")

		// Load ADRs filtered by module ownership (APP-INV-087)
		if len(implementsIDs) > 0 {
			for _, adrID := range implementsIDs {
				adrRows, err := db.Query(`SELECT adr_id, title, COALESCE(problem,''), COALESCE(decision_text,''), COALESCE(consequences,''), COALESCE(tests,'')
					FROM adrs WHERE spec_id = ? AND adr_id = ?`, specID, adrID)
				if err == nil {
					for adrRows.Next() {
						var adr projector.ADR
						adrRows.Scan(&adr.ID, &adr.Title, &adr.Problem, &adr.Decision, &adr.Consequences, &adr.Tests)
						mod.ADRs = append(mod.ADRs, adr)
					}
					adrRows.Close()
				}
			}
		}

		// Load sections for this module
		secRows, err := db.Query(`SELECT section_path, title, heading_level, COALESCE(raw_text,'')
			FROM sections s
			WHERE s.spec_id = ? AND s.source_file_id IN (SELECT source_file_id FROM modules WHERE id = ?)
			ORDER BY s.section_path`, specID, moduleID)
		if err == nil {
			for secRows.Next() {
				var sec projector.Section
				secRows.Scan(&sec.Path, &sec.Title, &sec.Level, &sec.Body)
				mod.Sections = append(mod.Sections, sec)
			}
			secRows.Close()
		}

		// Load negative specs for this module
		negRows, err := db.Query(`SELECT constraint_text FROM module_negative_specs WHERE module_id = ?`, moduleID)
		if err == nil {
			for negRows.Next() {
				var constraint string
				negRows.Scan(&constraint)
				mod.NegSpecs = append(mod.NegSpecs, constraint)
			}
			negRows.Close()
		}

		modules = append(modules, mod)
	}

	return modules, nil
}

// getModuleRelationshipTargets returns all targets for a given module and relationship type.
func getModuleRelationshipTargets(db storage.DB, moduleID int64, relType string) []string {
	rows, err := db.Query(`SELECT target FROM module_relationships WHERE module_id = ? AND rel_type = ?`, moduleID, relType)
	if err != nil {
		return nil
	}
	defer rows.Close()

	var targets []string
	for rows.Next() {
		var target string
		if err := rows.Scan(&target); err != nil {
			continue
		}
		targets = append(targets, target)
	}
	return targets
}
