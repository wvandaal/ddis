package cli

// ddis:maintains APP-INV-028 (spec-as-trunk — crystallization feeds discoveries into spec)
// ddis:maintains APP-INV-033 (absorption format parity — generated content indistinguishable from hand-authored)
// ddis:implements APP-INV-025 (discovery provenance chain — records decision_crystallized event linking artifact to discovery thread)
// ddis:maintains APP-INV-061
// ddis:implements APP-INV-088 (single write path — crystallize emits events as primary write, file is projection)
// ddis:implements APP-ADR-049
// ddis:implements APP-ADR-069 (crystallize event-only path — events emitted alongside file write)

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/parser"
)

var (
	crystallizeModule    string
	crystallizeFile      string
	crystallizeNoProject bool
)

// CrystallizeInput defines the JSON schema for element crystallization.
type CrystallizeInput struct {
	Type string `json:"type"` // "invariant" or "adr"

	// Invariant fields
	ID                string `json:"id"`
	Title             string `json:"title"`
	Statement         string `json:"statement"`
	SemiFormal        string `json:"semi_formal"`
	ViolationScenario string `json:"violation_scenario"`
	ValidationMethod  string `json:"validation_method"`
	WhyThisMatters    string `json:"why_this_matters"`

	// ADR fields
	Problem    string `json:"problem"`
	Options    string `json:"options"`
	Decision   string `json:"decision"`
	Rationale  string `json:"rationale"`
	Tests      string `json:"tests"`

	// Manifest registry fields
	Owner       string `json:"owner"`
	Domain      string `json:"domain"`
	Description string `json:"description"`
}

var crystallizeCmd = &cobra.Command{
	Use:   "crystallize",
	Short: "Crystallize a discovery into a spec element",
	Long: `Writes a new invariant or ADR to the correct module file and updates
the manifest registry. Input is JSON on stdin.

This is the bilateral lifecycle's code→spec return path for NEW elements.
The CLI mediates spec authoring: exemplars provide format, crystallize writes.

Examples:
  echo '{"type":"invariant","id":"APP-INV-042",...}' | ddis discover crystallize --module auto-prompting
  ddis discover crystallize --module query-validation < element.json`,
	RunE:          runCrystallize,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	crystallizeCmd.Flags().StringVar(&crystallizeModule, "module", "", "Target module name (required)")
	crystallizeCmd.Flags().StringVar(&crystallizeFile, "manifest", "manifest.yaml", "Path to manifest.yaml")
	crystallizeCmd.Flags().BoolVar(&crystallizeNoProject, "no-project", false, "Skip auto-project (emit events only, no markdown regeneration)")
}

func runCrystallize(cmd *cobra.Command, args []string) error {
	// Read JSON from stdin
	var input CrystallizeInput
	dec := json.NewDecoder(os.Stdin)
	if err := dec.Decode(&input); err != nil {
		return fmt.Errorf("parse input JSON: %w", err)
	}

	if input.Type == "" {
		return fmt.Errorf("input must have 'type' field (invariant or adr)")
	}
	if input.ID == "" {
		return fmt.Errorf("input must have 'id' field")
	}
	if input.Title == "" {
		return fmt.Errorf("input must have 'title' field")
	}

	// Load manifest to find module file path
	manifest, _, err := parser.ParseManifestFile(crystallizeFile)
	if err != nil {
		return fmt.Errorf("parse manifest: %w", err)
	}

	// Auto-resolve target module if --module not provided
	if crystallizeModule == "" {
		resolved, resolveErr := resolveTargetModule(manifest, input.ID, input.Domain)
		if resolveErr != nil {
			return resolveErr
		}
		crystallizeModule = resolved
		fmt.Fprintf(os.Stderr, "Auto-resolved module: %s\n", crystallizeModule)
	}

	if _, ok := manifest.Modules[crystallizeModule]; !ok {
		available := make([]string, 0, len(manifest.Modules))
		for k := range manifest.Modules {
			available = append(available, k)
		}
		return fmt.Errorf("module %q not found in manifest (available: %s)", crystallizeModule, strings.Join(available, ", "))
	}

	// Validate element type
	switch input.Type {
	case "invariant", "adr":
		// valid
	default:
		return fmt.Errorf("unknown type %q (expected invariant or adr)", input.Type)
	}

	// --- Step 1: Emit events (APP-INV-088: event log is the primary write) ---

	// Emit decision_crystallized event to Stream 1 (Discovery).
	emitEvent(crystallizeFile, events.StreamDiscovery, events.TypeDecisionCrystallized, "", map[string]interface{}{
		"element_type":  input.Type,
		"element_id":    input.ID,
		"title":         input.Title,
		"module":        crystallizeModule,
		"artifact_refs": []string{input.ID},
	})

	// Emit content-bearing event to Stream 2 (Specification).
	// ddis:implements APP-INV-072 (event content completeness — crystallize emits structured payloads)
	switch input.Type {
	case "invariant":
		emitEvent(crystallizeFile, events.StreamSpecification, events.TypeInvariantCrystallized, "", events.InvariantPayload{
			ID:                input.ID,
			Title:             input.Title,
			Statement:         input.Statement,
			SemiFormal:        input.SemiFormal,
			ViolationScenario: input.ViolationScenario,
			ValidationMethod:  input.ValidationMethod,
			WhyThisMatters:    input.WhyThisMatters,
			Module:            crystallizeModule,
		})
	case "adr":
		emitEvent(crystallizeFile, events.StreamSpecification, events.TypeADRCrystallized, "", events.ADRPayload{
			ID:           input.ID,
			Title:        input.Title,
			Problem:      input.Problem,
			Options:      input.Options,
			Decision:     input.Decision,
			Consequences: input.Rationale,
			Tests:        input.Tests,
			Module:       crystallizeModule,
		})
	}

	fmt.Printf("Crystallized %s %s: %s (event emitted to stream)\n", input.Type, input.ID, input.Title)

	// --- Step 2: Update manifest registry (metadata, not content) ---
	if input.Type == "invariant" && input.Owner != "" {
		if err := updateManifestRegistry(crystallizeFile, input); err != nil {
			return fmt.Errorf("update manifest registry: %w", err)
		}
	}

	// --- Step 3: Auto-project (APP-ADR-069: materialize + project) ---
	if !crystallizeNoProject {
		if err := crystallizeAutoProject(crystallizeFile); err != nil {
			// Auto-project failure is non-fatal — the event was already emitted
			fmt.Fprintf(os.Stderr, "Auto-project warning: %v\n", err)
			fmt.Fprintf(os.Stderr, "  Events are recorded. Run manually: ddis materialize && ddis project\n")
		}
	}

	if !NoGuidance {
		fmt.Println("\nNext: ddis parse manifest.yaml && ddis validate")
	}
	return nil
}

// crystallizeAutoProject runs materialize + project after event emission.
// This is the auto-project path (APP-ADR-069): events → SQLite → markdown.
// Failures are non-fatal since the event was already recorded.
func crystallizeAutoProject(manifestPath string) error {
	manifestDir := filepath.Dir(manifestPath)
	wsRoot := manifestDir
	streamPath := events.StreamPath(wsRoot, events.StreamSpecification)

	// Check that the event stream exists
	if _, err := os.Stat(streamPath); os.IsNotExist(err) {
		return fmt.Errorf("no event stream at %s (run 'ddis import' first to populate)", streamPath)
	}

	// Materialize: JSONL → SQLite
	dbPath := filepath.Join(wsRoot, ".ddis", "index.db")
	result, err := runMaterializeInternal(streamPath, dbPath, true)
	if err != nil {
		return fmt.Errorf("materialize: %w", err)
	}
	fmt.Fprintf(os.Stderr, "  Auto-materialized %d events → %s\n", result.EventsProcessed, dbPath)

	// Project: SQLite → markdown module files
	// Derive module output directory from manifest module paths
	moduleDir := filepath.Join(manifestDir, "modules")
	if err := runProjectInternal(dbPath, moduleDir, ""); err != nil {
		return fmt.Errorf("project: %w", err)
	}

	return nil
}

// formatInvariant generates the canonical invariant markdown format.
func formatInvariant(in CrystallizeInput) string {
	var b strings.Builder

	fmt.Fprintf(&b, "**%s: %s**\n\n", in.ID, in.Title)

	if in.Statement != "" {
		fmt.Fprintf(&b, "*%s*\n\n", in.Statement)
	}

	if in.SemiFormal != "" {
		fmt.Fprintf(&b, "```\n%s\n```\n\n", in.SemiFormal)
	}

	if in.ViolationScenario != "" {
		fmt.Fprintf(&b, "Violation scenario: %s\n\n", in.ViolationScenario)
	}

	if in.ValidationMethod != "" {
		fmt.Fprintf(&b, "Validation: %s\n\n", in.ValidationMethod)
	}

	if in.WhyThisMatters != "" {
		fmt.Fprintf(&b, "// WHY THIS MATTERS: %s\n\n", in.WhyThisMatters)
	}

	b.WriteString("---\n")
	return b.String()
}

// formatADR generates the canonical ADR markdown format.
// Uses #### level-4 headings per the DDIS element specification format.
func formatADR(in CrystallizeInput) string {
	var b strings.Builder

	fmt.Fprintf(&b, "### %s: %s\n\n", in.ID, in.Title)

	if in.Problem != "" {
		fmt.Fprintf(&b, "#### Problem\n\n%s\n\n", in.Problem)
	}

	if in.Options != "" {
		fmt.Fprintf(&b, "#### Options\n\n%s\n\n", in.Options)
	}

	if in.Decision != "" {
		fmt.Fprintf(&b, "#### Decision\n\n%s\n\n", in.Decision)
	}

	if in.Rationale != "" {
		fmt.Fprintf(&b, "#### Consequences\n\n%s\n\n", in.Rationale)
	}

	if in.Tests != "" {
		fmt.Fprintf(&b, "#### Tests\n\n%s\n\n", in.Tests)
	}

	b.WriteString("---\n")
	return b.String()
}

// resolveTargetModule auto-resolves the target module for crystallization.
// Priority: 1) manifest invariant_registry mapping, 2) domain match from input, 3) error with candidates.
func resolveTargetModule(manifest *parser.ManifestData, elementID, inputDomain string) (string, error) {
	// Priority 1: Check invariant_registry for explicit mapping
	if entry, ok := manifest.InvariantRegistry[elementID]; ok {
		// Find module by domain match
		for name, mod := range manifest.Modules {
			if mod.Domain == entry.Domain {
				return name, nil
			}
		}
	}

	// Priority 2: Check manifest maintains lists
	for name, mod := range manifest.Modules {
		for _, m := range mod.Maintains {
			if m == elementID {
				return name, nil
			}
		}
	}

	// Priority 3: Domain match from input
	if inputDomain != "" {
		for name, mod := range manifest.Modules {
			if mod.Domain == inputDomain {
				return name, nil
			}
		}
	}

	// No match — error with candidates
	candidates := make([]string, 0, len(manifest.Modules))
	for name, mod := range manifest.Modules {
		candidates = append(candidates, fmt.Sprintf("%s (domain: %s)", name, mod.Domain))
	}
	return "", fmt.Errorf("cannot auto-resolve target module for %s; use --module to specify (candidates: %s)",
		elementID, strings.Join(candidates, ", "))
}

// updateManifestRegistry appends an invariant to the manifest.yaml invariant_registry.
func updateManifestRegistry(manifestPath string, in CrystallizeInput) error {
	data, err := os.ReadFile(manifestPath)
	if err != nil {
		return err
	}

	content := string(data)

	// Find the invariant_registry section and append the new entry
	entry := fmt.Sprintf("  %s: { owner: %s, domain: %s, description: \"%s\" }\n",
		in.ID, in.Owner, in.Domain, in.Description)

	// Append before the last line if registry exists, or at end
	if idx := strings.LastIndex(content, "\n"); idx >= 0 {
		content = content[:idx+1] + entry
	} else {
		content += "\n" + entry
	}

	return os.WriteFile(manifestPath, []byte(content), 0644)
}
