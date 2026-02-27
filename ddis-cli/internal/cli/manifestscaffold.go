package cli

// ddis:maintains APP-INV-060
// ddis:implements APP-ADR-047

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/parser"
)

var manifestScaffoldCmd = &cobra.Command{
	Use:   "scaffold [manifest.yaml]",
	Short: "Generate stub module files for entries declared in manifest.yaml",
	Long: `Reads manifest.yaml and generates stub module files for any declared
modules or constitution files that do not yet exist on disk.

Each generated stub contains correct YAML frontmatter (domain, maintains,
interfaces) and a heading derived from the module name, followed by a TODO
placeholder. Running the command a second time is a no-op for files that
already exist (idempotent).

Examples:
  ddis manifest scaffold                              # Use manifest.yaml in cwd
  ddis manifest scaffold ddis-cli-spec/manifest.yaml # Explicit path`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runManifestScaffold,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	manifestCmd.AddCommand(manifestScaffoldCmd)
}

func runManifestScaffold(cmd *cobra.Command, args []string) error {
	manifestPath := "manifest.yaml"
	if len(args) >= 1 {
		manifestPath = args[0]
	}

	manifest, _, err := parser.ParseManifestFile(manifestPath)
	if err != nil {
		return fmt.Errorf("parse manifest: %w", err)
	}

	manifestDir := filepath.Dir(manifestPath)
	created := 0
	skipped := 0

	// Check constitution file
	if manifest.Constitution.System != "" {
		constitutionPath := filepath.Join(manifestDir, manifest.Constitution.System)
		if fileExists(constitutionPath) {
			fmt.Printf("  exists:  %s\n", constitutionPath)
			skipped++
		} else {
			if err := scaffoldConstitution(constitutionPath); err != nil {
				return fmt.Errorf("create constitution stub %s: %w", constitutionPath, err)
			}
			fmt.Printf("  created: %s\n", constitutionPath)
			created++
		}
	}

	// Check each module file
	for moduleName, moduleDecl := range manifest.Modules {
		if moduleDecl.File == "" {
			continue
		}
		modulePath := filepath.Join(manifestDir, moduleDecl.File)
		if fileExists(modulePath) {
			fmt.Printf("  exists:  %s\n", modulePath)
			skipped++
		} else {
			if err := scaffoldModule(modulePath, moduleName, moduleDecl); err != nil {
				return fmt.Errorf("create module stub %s: %w", modulePath, err)
			}
			fmt.Printf("  created: %s\n", modulePath)
			created++
		}
	}

	fmt.Printf("\n%d created, %d already exist.\n", created, skipped)
	if created > 0 && !NoGuidance {
		fmt.Println("\nNext: ddis manifest sync && ddis parse manifest.yaml")
		fmt.Println("  Fill TODO markers in the generated stubs, then parse to index.")
	}

	return nil
}

// scaffoldModule writes a minimal stub module file with correct YAML frontmatter.
func scaffoldModule(path, moduleName string, decl parser.ModuleDecl) error {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return err
	}

	title := moduleTitle(moduleName)
	content := fmt.Sprintf("---\ndomain: %s\nmaintains: []\ninterfaces: []\n---\n\n# %s\n\nTODO: specify this module.\n", decl.Domain, title)

	return os.WriteFile(path, []byte(content), 0644)
}

// scaffoldConstitution writes a minimal stub constitution file.
func scaffoldConstitution(path string) error {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return err
	}

	content := "---\ndomain: constitution\nmaintains: []\ninterfaces: []\n---\n\n# System Constitution\n\nTODO: specify this constitution.\n"

	return os.WriteFile(path, []byte(content), 0644)
}

// moduleTitle converts a module name (e.g. "parse-pipeline") to a title-cased heading.
func moduleTitle(name string) string {
	if name == "" {
		return "Module"
	}
	// Capitalise first letter; leave the rest as-is so names like "parse-pipeline"
	// become "Parse-pipeline" — readable and unambiguous without a word-splitting heuristic.
	runes := []rune(name)
	if runes[0] >= 'a' && runes[0] <= 'z' {
		runes[0] = runes[0] - 32
	}
	return string(runes)
}
