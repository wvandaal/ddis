package cli

// ddis:maintains APP-INV-034 (state monad universality — spec metadata as CommandResult)

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/storage"
)

var specJSON bool

var specCmd = &cobra.Command{
	Use:   "spec [index.db]",
	Short: "Show spec metadata summary",
	Long: `Displays a summary of the specification index: name, version, element counts,
module structure, and parse timestamp.

Examples:
  ddis spec
  ddis spec manifest.ddis.db
  ddis spec --json`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runSpec,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	specCmd.Flags().BoolVar(&specJSON, "json", false, "JSON output")
}

type specSummary struct {
	SpecName    string            `json:"spec_name"`
	DDISVersion string            `json:"ddis_version"`
	SpecPath    string            `json:"spec_path"`
	ParsedAt    string            `json:"parsed_at"`
	SourceType  string            `json:"source_type"`
	TotalLines  int               `json:"total_lines"`
	Elements    specElements      `json:"elements"`
	Modules     []specModuleInfo  `json:"modules"`
	ParentSpec  *string           `json:"parent_spec,omitempty"`
}

type specElements struct {
	Sections    int `json:"sections"`
	Invariants  int `json:"invariants"`
	ADRs        int `json:"adrs"`
	Gates       int `json:"quality_gates"`
	NegSpecs    int `json:"negative_specs"`
	Glossary    int `json:"glossary_entries"`
	CrossRefs   int `json:"cross_references"`
	Unresolved  int `json:"unresolved_refs"`
}

type specModuleInfo struct {
	Name   string `json:"name"`
	Domain string `json:"domain"`
}

func runSpec(cmd *cobra.Command, args []string) error {
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

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	spec, err := storage.GetSpecIndex(db, specID)
	if err != nil {
		return fmt.Errorf("get spec: %w", err)
	}

	// Count elements.
	invs, _ := storage.ListInvariants(db, specID)
	adrs, _ := storage.ListADRs(db, specID)
	gates, _ := storage.ListQualityGates(db, specID)
	sects, _ := storage.ListSections(db, specID)
	negs, _ := storage.ListNegativeSpecs(db, specID)
	gloss, _ := storage.ListGlossaryEntries(db, specID)
	unresolved, _ := storage.GetUnresolvedRefs(db, specID)
	modules, _ := storage.ListModules(db, specID)

	// Count total cross-refs.
	var totalRefs int
	err = db.QueryRow("SELECT COUNT(*) FROM cross_references WHERE spec_id = ?", specID).Scan(&totalRefs)
	if err != nil {
		totalRefs = 0
	}

	// Check for parent spec.
	var parentSpec *string
	parentID, err := storage.GetParentSpecID(db, specID)
	if err == nil && parentID != nil {
		ps, err := storage.GetSpecIndex(db, *parentID)
		if err == nil {
			parentSpec = &ps.SpecPath
		}
	}

	summary := specSummary{
		SpecName:    spec.SpecName,
		DDISVersion: spec.DDISVersion,
		SpecPath:    spec.SpecPath,
		ParsedAt:    spec.ParsedAt,
		SourceType:  spec.SourceType,
		TotalLines:  spec.TotalLines,
		Elements: specElements{
			Sections:   len(sects),
			Invariants: len(invs),
			ADRs:       len(adrs),
			Gates:      len(gates),
			NegSpecs:   len(negs),
			Glossary:   len(gloss),
			CrossRefs:  totalRefs,
			Unresolved: len(unresolved),
		},
		ParentSpec: parentSpec,
	}

	for _, m := range modules {
		summary.Modules = append(summary.Modules, specModuleInfo{
			Name:   m.ModuleName,
			Domain: m.Domain,
		})
	}

	if specJSON {
		enc := json.NewEncoder(os.Stdout)
		enc.SetIndent("", "  ")
		return enc.Encode(summary)
	}

	fmt.Printf("Spec: %s\n", summary.SpecName)
	if summary.DDISVersion != "" {
		fmt.Printf("  DDIS Version: %s\n", summary.DDISVersion)
	}
	fmt.Printf("  Source:        %s (%s)\n", summary.SpecPath, summary.SourceType)
	fmt.Printf("  Parsed:        %s\n", summary.ParsedAt)
	fmt.Printf("  Total lines:   %d\n", summary.TotalLines)
	if summary.ParentSpec != nil {
		fmt.Printf("  Parent spec:   %s\n", *summary.ParentSpec)
	}

	fmt.Printf("\nElements:\n")
	fmt.Printf("  Sections:        %d\n", summary.Elements.Sections)
	fmt.Printf("  Invariants:      %d\n", summary.Elements.Invariants)
	fmt.Printf("  ADRs:            %d\n", summary.Elements.ADRs)
	fmt.Printf("  Quality Gates:   %d\n", summary.Elements.Gates)
	fmt.Printf("  Negative Specs:  %d\n", summary.Elements.NegSpecs)
	fmt.Printf("  Glossary:        %d\n", summary.Elements.Glossary)
	fmt.Printf("  Cross-Refs:      %d (%d unresolved)\n", summary.Elements.CrossRefs, summary.Elements.Unresolved)

	if len(summary.Modules) > 0 {
		fmt.Printf("\nModules (%d):\n", len(summary.Modules))
		for _, m := range summary.Modules {
			fmt.Printf("  %-25s [%s]\n", m.Name, m.Domain)
		}
	}

	if !NoGuidance {
		fmt.Println("\nNext: ddis validate")
		fmt.Println("  Verify structural integrity of this spec.")
	}

	return nil
}
