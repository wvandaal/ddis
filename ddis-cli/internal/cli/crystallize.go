package cli

// ddis:maintains APP-INV-028 (spec-as-trunk — crystallization feeds discoveries into spec)
// ddis:maintains APP-INV-033 (absorption format parity — generated content indistinguishable from hand-authored)

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/parser"
)

var (
	crystallizeModule string
	crystallizeFile   string
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
}

func runCrystallize(cmd *cobra.Command, args []string) error {
	if crystallizeModule == "" {
		return fmt.Errorf("--module is required")
	}

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

	moduleDecl, ok := manifest.Modules[crystallizeModule]
	if !ok {
		available := make([]string, 0, len(manifest.Modules))
		for k := range manifest.Modules {
			available = append(available, k)
		}
		return fmt.Errorf("module %q not found in manifest (available: %s)", crystallizeModule, strings.Join(available, ", "))
	}

	modulePath := filepath.Join(filepath.Dir(crystallizeFile), moduleDecl.File)

	// Format the element
	var formatted string
	switch input.Type {
	case "invariant":
		formatted = formatInvariant(input)
	case "adr":
		formatted = formatADR(input)
	default:
		return fmt.Errorf("unknown type %q (expected invariant or adr)", input.Type)
	}

	// Read existing content to check for replacement
	existing, err := os.ReadFile(modulePath)
	if err != nil {
		return fmt.Errorf("read module file: %w", err)
	}
	content := string(existing)

	// Check if element already exists — find and replace if so
	var marker string
	if input.Type == "invariant" {
		marker = fmt.Sprintf("**%s:", input.ID)
	} else {
		marker = fmt.Sprintf("### %s:", input.ID)
	}

	if idx := strings.Index(content, marker); idx >= 0 {
		// Find the end of the existing element (next --- separator or next element header)
		endIdx := idx
		searchFrom := idx + len(marker)
		// Look for the terminating ---
		if dashes := strings.Index(content[searchFrom:], "\n---\n"); dashes >= 0 {
			endIdx = searchFrom + dashes + len("\n---\n")
		} else if dashes := strings.Index(content[searchFrom:], "\n---"); dashes >= 0 && searchFrom+dashes+4 >= len(content) {
			endIdx = len(content)
		} else {
			endIdx = len(content)
		}

		// Find the start (back up to include any leading blank lines)
		startIdx := idx
		for startIdx > 0 && content[startIdx-1] == '\n' {
			startIdx--
		}
		if startIdx > 0 {
			startIdx++ // Keep one newline
		}

		content = content[:startIdx] + "\n" + formatted + content[endIdx:]
		if err := os.WriteFile(modulePath, []byte(content), 0644); err != nil {
			return fmt.Errorf("write module file: %w", err)
		}
		fmt.Printf("Replaced %s %s: %s in %s\n", input.Type, input.ID, input.Title, modulePath)
	} else {
		// Append to end
		f, err := os.OpenFile(modulePath, os.O_APPEND|os.O_WRONLY, 0644)
		if err != nil {
			return fmt.Errorf("open module file: %w", err)
		}
		defer f.Close()

		if _, err := f.WriteString("\n" + formatted); err != nil {
			return fmt.Errorf("write to module file: %w", err)
		}
		fmt.Printf("Crystallized %s %s: %s → %s\n", input.Type, input.ID, input.Title, modulePath)
	}

	// Update manifest.yaml invariant registry (for invariants only)
	if input.Type == "invariant" && input.Owner != "" {
		if err := updateManifestRegistry(crystallizeFile, input); err != nil {
			return fmt.Errorf("update manifest registry: %w", err)
		}
	}

	fmt.Println("\nNext: ddis parse manifest.yaml && ddis validate")
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
