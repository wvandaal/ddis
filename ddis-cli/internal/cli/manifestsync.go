package cli

// ddis:maintains APP-INV-047 (frontmatter-manifest bijection)
// ddis:implements APP-ADR-035 (frontmatter-manifest cross-validation)

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/spf13/cobra"
	"gopkg.in/yaml.v3"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/parser"
)

var (
	manifestSyncApply  bool
	manifestSyncJSON   bool
)

var manifestCmd = &cobra.Command{
	Use:   "manifest",
	Short: "Manifest operations",
	Long:  `Operations on the manifest.yaml file.`,
	SilenceErrors: true,
	SilenceUsage:  true,
}

var manifestSyncCmd = &cobra.Command{
	Use:   "sync [manifest.yaml]",
	Short: "Synchronize manifest.yaml with module frontmatter",
	Long: `Reads YAML frontmatter from each module file and compares it to the
corresponding manifest.yaml declarations. Frontmatter is authoritative —
it is collocated with the module body and maintained by spec authors.

Reports discrepancies in: maintains, interfaces, implements, adjacent,
and negative_specs fields. With --apply, updates manifest.yaml to match.

This enforces APP-INV-047 (Frontmatter-Manifest Bijection).

Examples:
  ddis manifest sync                              # Report differences
  ddis manifest sync ddis-cli-spec/manifest.yaml  # Explicit path
  ddis manifest sync --apply                      # Fix manifest to match frontmatter
  ddis manifest sync --json                       # Machine-readable output`,
	Args: cobra.MaximumNArgs(1),
	RunE: runManifestSync,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	manifestSyncCmd.Flags().BoolVar(&manifestSyncApply, "apply", false, "Update manifest.yaml to match frontmatter")
	manifestSyncCmd.Flags().BoolVar(&manifestSyncJSON, "json", false, "JSON output")
	manifestCmd.AddCommand(manifestSyncCmd)
}

// ModuleFrontmatter is the YAML frontmatter from a module .md file.
type ModuleFrontmatter struct {
	Module        string   `yaml:"module"`
	Domain        string   `yaml:"domain"`
	Maintains     []string `yaml:"maintains"`
	Interfaces    []string `yaml:"interfaces"`
	Implements    []string `yaml:"implements"`
	Adjacent      []string `yaml:"adjacent"`
	NegativeSpecs []string `yaml:"negative_specs"`
}

// FieldDiff describes a difference in one field between frontmatter and manifest.
type FieldDiff struct {
	Field      string
	Frontmatter []string
	Manifest   []string
	Added      []string // In frontmatter but not manifest
	Removed    []string // In manifest but not frontmatter
}

// ModuleDiff describes all differences for one module.
type ModuleDiff struct {
	ModuleName string
	Diffs      []FieldDiff
}

func runManifestSync(cmd *cobra.Command, args []string) error {
	// Find manifest.yaml
	manifestPath := ""
	if len(args) >= 1 {
		manifestPath = args[0]
	} else {
		// Try common locations
		candidates := []string{
			"manifest.yaml",
			"ddis-cli-spec/manifest.yaml",
		}
		for _, c := range candidates {
			if fileExists(c) {
				manifestPath = c
				break
			}
		}
		if manifestPath == "" {
			return fmt.Errorf("manifest.yaml not found; specify path as argument")
		}
	}

	manifest, _, err := parser.ParseManifestFile(manifestPath)
	if err != nil {
		return fmt.Errorf("parse manifest: %w", err)
	}

	manifestDir := filepath.Dir(manifestPath)
	var allDiffs []ModuleDiff
	totalDiffs := 0

	// Compare each module's frontmatter against manifest
	for moduleName, moduleDecl := range manifest.Modules {
		modulePath := filepath.Join(manifestDir, moduleDecl.File)
		fm, err := parseFrontmatter(modulePath)
		if err != nil {
			fmt.Fprintf(os.Stderr, "warning: cannot parse frontmatter for %s: %v\n", moduleName, err)
			continue
		}

		var diffs []FieldDiff

		if d := diffStringSlice("maintains", fm.Maintains, moduleDecl.Maintains); d != nil {
			diffs = append(diffs, *d)
		}
		if d := diffStringSlice("interfaces", fm.Interfaces, moduleDecl.Interfaces); d != nil {
			diffs = append(diffs, *d)
		}
		if d := diffStringSlice("implements", fm.Implements, moduleDecl.Implements); d != nil {
			diffs = append(diffs, *d)
		}
		if d := diffStringSlice("adjacent", fm.Adjacent, moduleDecl.Adjacent); d != nil {
			diffs = append(diffs, *d)
		}
		if d := diffStringSlice("negative_specs", fm.NegativeSpecs, moduleDecl.NegativeSpecs); d != nil {
			diffs = append(diffs, *d)
		}

		if len(diffs) > 0 {
			allDiffs = append(allDiffs, ModuleDiff{
				ModuleName: moduleName,
				Diffs:      diffs,
			})
			totalDiffs += len(diffs)
		}
	}

	// Sort by module name for deterministic output
	sort.Slice(allDiffs, func(i, j int) bool {
		return allDiffs[i].ModuleName < allDiffs[j].ModuleName
	})

	if manifestSyncJSON {
		return renderManifestSyncJSON(allDiffs, totalDiffs)
	}

	if len(allDiffs) == 0 {
		fmt.Println("Manifest is in sync with all module frontmatter.")
		if !NoGuidance {
			fmt.Println("\nNext: ddis validate")
		}
		return nil
	}

	// Report
	fmt.Printf("Frontmatter-Manifest Sync Report (%d field(s) differ)\n", totalDiffs)
	fmt.Println(strings.Repeat("─", 60))

	for _, md := range allDiffs {
		fmt.Printf("\nModule: %s\n", md.ModuleName)
		for _, fd := range md.Diffs {
			fmt.Printf("  %s:\n", fd.Field)
			for _, a := range fd.Added {
				fmt.Printf("    + %s  (in frontmatter, missing from manifest)\n", a)
			}
			for _, r := range fd.Removed {
				fmt.Printf("    - %s  (in manifest, missing from frontmatter)\n", r)
			}
		}
	}

	if manifestSyncApply {
		if err := applyManifestSync(manifestPath, manifest, allDiffs); err != nil {
			return fmt.Errorf("apply sync: %w", err)
		}
		fmt.Printf("\nApplied %d field update(s) to %s\n", totalDiffs, manifestPath)
		// ddis:maintains APP-INV-053 (event stream completeness — emits amendment_applied to stream 2)
		emitEvent(manifestPath, events.StreamSpecification, events.TypeAmendmentApplied, "", map[string]interface{}{
			"manifest_path": manifestPath,
			"fields_synced": totalDiffs,
			"command":       "manifest-sync",
		})
		if !NoGuidance {
			fmt.Println("\nNext: ddis parse manifest.yaml && ddis validate")
			fmt.Println("  Re-parse to verify the sync didn't break anything.")
		}
	} else {
		if !NoGuidance {
			fmt.Println("\nNext: ddis manifest sync --apply")
			fmt.Println("  Apply these changes to make manifest match frontmatter.")
		}
	}

	return nil
}

// parseFrontmatter extracts YAML frontmatter from a markdown file.
func parseFrontmatter(path string) (*ModuleFrontmatter, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	text := string(content)

	// Find frontmatter fences (---)
	if !strings.HasPrefix(text, "---") {
		return nil, fmt.Errorf("no YAML frontmatter found (file must start with ---)")
	}

	// Find closing ---
	endIdx := strings.Index(text[3:], "\n---")
	if endIdx < 0 {
		return nil, fmt.Errorf("unclosed YAML frontmatter (missing closing ---)")
	}

	fmText := text[3 : 3+endIdx]

	var fm ModuleFrontmatter
	if err := yaml.Unmarshal([]byte(fmText), &fm); err != nil {
		return nil, fmt.Errorf("parse frontmatter YAML: %w", err)
	}

	return &fm, nil
}

// diffStringSlice compares two string slices (order-insensitive) and returns differences.
func diffStringSlice(field string, frontmatter, manifest []string) *FieldDiff {
	fmSet := make(map[string]bool)
	mfSet := make(map[string]bool)

	for _, s := range frontmatter {
		fmSet[s] = true
	}
	for _, s := range manifest {
		mfSet[s] = true
	}

	var added, removed []string
	for _, s := range frontmatter {
		if !mfSet[s] {
			added = append(added, s)
		}
	}
	for _, s := range manifest {
		if !fmSet[s] {
			removed = append(removed, s)
		}
	}

	if len(added) == 0 && len(removed) == 0 {
		return nil
	}

	return &FieldDiff{
		Field:       field,
		Frontmatter: frontmatter,
		Manifest:    manifest,
		Added:       added,
		Removed:     removed,
	}
}

// applyManifestSync updates manifest.yaml using yaml.Node to preserve comments.
func applyManifestSync(manifestPath string, manifest *parser.ManifestData, diffs []ModuleDiff) error {
	data, err := os.ReadFile(manifestPath)
	if err != nil {
		return err
	}

	var doc yaml.Node
	if err := yaml.Unmarshal(data, &doc); err != nil {
		return fmt.Errorf("parse manifest as yaml.Node: %w", err)
	}

	if doc.Kind != yaml.DocumentNode || len(doc.Content) == 0 {
		return fmt.Errorf("unexpected YAML structure")
	}
	root := doc.Content[0]
	if root.Kind != yaml.MappingNode {
		return fmt.Errorf("root is not a mapping")
	}

	// Find "modules" value node
	modulesNode := findYAMLMapValue(root, "modules")
	if modulesNode == nil {
		return fmt.Errorf("no 'modules' section in manifest")
	}
	if modulesNode.Kind != yaml.MappingNode {
		return fmt.Errorf("'modules' is not a mapping")
	}

	for _, md := range diffs {
		moduleNode := findYAMLMapValue(modulesNode, md.ModuleName)
		if moduleNode == nil {
			fmt.Fprintf(os.Stderr, "warning: module %q not found in YAML tree\n", md.ModuleName)
			continue
		}

		for _, fd := range md.Diffs {
			updateYAMLSequence(moduleNode, fd.Field, fd.Frontmatter)
		}
	}

	// Re-encode preserving structure
	out, err := marshalYAMLNode(&doc)
	if err != nil {
		return fmt.Errorf("encode updated manifest: %w", err)
	}

	return os.WriteFile(manifestPath, out, 0644)
}

// findYAMLMapValue finds a value node by key in a mapping node.
func findYAMLMapValue(mapping *yaml.Node, key string) *yaml.Node {
	if mapping.Kind != yaml.MappingNode {
		return nil
	}
	for i := 0; i < len(mapping.Content)-1; i += 2 {
		if mapping.Content[i].Value == key {
			return mapping.Content[i+1]
		}
	}
	return nil
}

// updateYAMLSequence replaces a sequence field's content in a mapping node.
func updateYAMLSequence(mapping *yaml.Node, fieldName string, newValues []string) {
	if mapping.Kind != yaml.MappingNode {
		return
	}

	for i := 0; i < len(mapping.Content)-1; i += 2 {
		if mapping.Content[i].Value == fieldName {
			valueNode := mapping.Content[i+1]
			if valueNode.Kind != yaml.SequenceNode {
				// Convert to sequence
				valueNode.Kind = yaml.SequenceNode
				valueNode.Tag = "!!seq"
			}

			// Build new content nodes preserving style
			newContent := make([]*yaml.Node, len(newValues))
			for j, v := range newValues {
				newContent[j] = &yaml.Node{
					Kind:  yaml.ScalarNode,
					Tag:   "!!str",
					Value: v,
				}
				// Preserve quoting for negative_specs (strings with special chars)
				if fieldName == "negative_specs" {
					newContent[j].Style = yaml.DoubleQuotedStyle
				}
			}
			valueNode.Content = newContent
			return
		}
	}

	// Field doesn't exist in manifest — add it
	keyNode := &yaml.Node{
		Kind:  yaml.ScalarNode,
		Tag:   "!!str",
		Value: fieldName,
	}
	valueNode := &yaml.Node{
		Kind: yaml.SequenceNode,
		Tag:  "!!seq",
	}
	for _, v := range newValues {
		n := &yaml.Node{
			Kind:  yaml.ScalarNode,
			Tag:   "!!str",
			Value: v,
		}
		if fieldName == "negative_specs" {
			n.Style = yaml.DoubleQuotedStyle
		}
		valueNode.Content = append(valueNode.Content, n)
	}
	mapping.Content = append(mapping.Content, keyNode, valueNode)
}

// marshalYAMLNode encodes a yaml.Node tree back to YAML bytes.
func marshalYAMLNode(doc *yaml.Node) ([]byte, error) {
	var buf strings.Builder
	enc := yaml.NewEncoder(&buf)
	enc.SetIndent(2)
	if err := enc.Encode(doc); err != nil {
		return nil, err
	}
	if err := enc.Close(); err != nil {
		return nil, err
	}
	return []byte(buf.String()), nil
}

// renderManifestSyncJSON outputs the sync report as JSON.
func renderManifestSyncJSON(diffs []ModuleDiff, totalDiffs int) error {
	fmt.Println("{")
	fmt.Printf("  \"total_diffs\": %d,\n", totalDiffs)
	fmt.Printf("  \"in_sync\": %v,\n", totalDiffs == 0)
	fmt.Println("  \"modules\": {")

	for i, md := range diffs {
		fmt.Printf("    %q: {\n", md.ModuleName)
		for j, fd := range md.Diffs {
			fmt.Printf("      %q: {\n", fd.Field)
			fmt.Printf("        \"added\": [")
			for k, a := range fd.Added {
				if k > 0 {
					fmt.Print(", ")
				}
				fmt.Printf("%q", a)
			}
			fmt.Print("],\n")
			fmt.Printf("        \"removed\": [")
			for k, r := range fd.Removed {
				if k > 0 {
					fmt.Print(", ")
				}
				fmt.Printf("%q", r)
			}
			fmt.Print("]\n")
			if j < len(md.Diffs)-1 {
				fmt.Println("      },")
			} else {
				fmt.Println("      }")
			}
		}
		if i < len(diffs)-1 {
			fmt.Println("    },")
		} else {
			fmt.Println("    }")
		}
	}

	fmt.Println("  }")
	fmt.Println("}")
	return nil
}
