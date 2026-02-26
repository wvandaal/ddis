package bundle

import (
	"database/sql"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// Default line budgets for bundle assembly. Overridden by manifest if set.
const (
	DefaultLineCeiling = 5000
	DefaultLineTarget  = 4000
)

// Options controls bundle assembly behavior.
type Options struct {
	ContentOnly bool
	AsJSON      bool
}

// BundleResult holds the assembled domain bundle.
type BundleResult struct {
	Domain            string             `json:"domain"`
	ConstitutionLines int                `json:"constitution_lines"`
	Modules           []BundleModule     `json:"modules"`
	ModuleLines       int                `json:"module_lines"`
	InterfaceElements []InterfaceElement `json:"interface_elements"`
	InterfaceLines    int                `json:"interface_lines"`
	TotalLines        int                `json:"total_lines"`
	Budget            Budget             `json:"budget"`
	Content           string             `json:"content,omitempty"`
}

// BundleModule represents one module included in the bundle.
type BundleModule struct {
	Name  string `json:"name"`
	Lines int    `json:"lines"`
}

// InterfaceElement represents a boundary invariant from another domain.
type InterfaceElement struct {
	ID          string `json:"id"`
	OwnerDomain string `json:"owner_domain"`
	Title       string `json:"title"`
}

// Budget tracks line budget usage.
type Budget struct {
	Target  int     `json:"target"`
	Ceiling int     `json:"ceiling"`
	Usage   float64 `json:"usage"`
}

// Assemble builds a domain context bundle: constitution + domain modules + interface stubs.
func Assemble(db *sql.DB, specID int64, domain string, opts Options) (*BundleResult, error) {
	result := &BundleResult{Domain: domain}
	var contentParts []string

	// 1. Load constitution text from source files with file_role = 'system_constitution'
	sourceFiles, err := storage.GetSourceFiles(db, specID)
	if err != nil {
		return nil, fmt.Errorf("get source files: %w", err)
	}

	var constitutionText string
	for _, sf := range sourceFiles {
		if sf.FileRole == "system_constitution" {
			text, err := storage.GetSourceFileContent(db, sf.ID)
			if err != nil {
				continue
			}
			if constitutionText != "" {
				constitutionText += "\n"
			}
			constitutionText += text
		}
	}
	result.ConstitutionLines = countLines(constitutionText)
	if constitutionText != "" {
		contentParts = append(contentParts, "# System Constitution\n\n"+constitutionText)
	}

	// 2. Load domain modules
	modules, err := storage.ListModulesByDomain(db, specID, domain)
	if err != nil {
		return nil, fmt.Errorf("list modules: %w", err)
	}

	var moduleTotalLines int
	for _, m := range modules {
		text, err := storage.GetSourceFileContent(db, m.SourceFileID)
		if err != nil {
			continue
		}
		lines := countLines(text)
		result.Modules = append(result.Modules, BundleModule{Name: m.ModuleName, Lines: lines})
		moduleTotalLines += lines
		contentParts = append(contentParts, text)
	}
	result.ModuleLines = moduleTotalLines

	// 3. Load interface invariants from other domains
	boundaryInvs, err := storage.GetDomainBoundaryInvariants(db, specID, domain)
	if err != nil {
		// Non-fatal: some specs might not have boundary invariants
		boundaryInvs = nil
	}

	var interfaceText string
	for _, entry := range boundaryInvs {
		inv, err := storage.GetInvariant(db, specID, entry.InvariantID)
		if err != nil || inv == nil {
			continue
		}
		result.InterfaceElements = append(result.InterfaceElements, InterfaceElement{
			ID:          entry.InvariantID,
			OwnerDomain: entry.Domain,
			Title:       inv.Title,
		})
		stub := renderInvariantStub(*inv, entry.Domain)
		interfaceText += stub + "\n"
	}
	result.InterfaceLines = countLines(interfaceText)
	if interfaceText != "" {
		contentParts = append(contentParts, "\n# Interface Invariants (from other domains)\n\n"+interfaceText)
	}

	// 4. Compute budget
	result.TotalLines = result.ConstitutionLines + result.ModuleLines + result.InterfaceLines
	manifest, _ := storage.GetManifest(db, specID)
	ceiling := DefaultLineCeiling
	target := DefaultLineTarget
	if manifest != nil {
		if manifest.HardCeilingLines > 0 {
			ceiling = manifest.HardCeilingLines
		}
		if manifest.TargetLines > 0 {
			target = manifest.TargetLines
		}
	}
	result.Budget = Budget{
		Target:  target,
		Ceiling: ceiling,
		Usage:   float64(result.TotalLines) / float64(ceiling),
	}

	// 5. Assemble content
	result.Content = strings.Join(contentParts, "\n\n---\n\n")

	// Ensure non-nil slices for JSON
	if result.Modules == nil {
		result.Modules = []BundleModule{}
	}
	if result.InterfaceElements == nil {
		result.InterfaceElements = []InterfaceElement{}
	}
	return result, nil
}

func renderInvariantStub(inv storage.Invariant, ownerDomain string) string {
	var b strings.Builder
	fmt.Fprintf(&b, "### %s: %s\n", inv.InvariantID, inv.Title)
	fmt.Fprintf(&b, "*Owner domain: %s*\n\n", ownerDomain)
	if inv.Statement != "" {
		fmt.Fprintf(&b, "**Statement:** %s\n\n", inv.Statement)
	}
	if inv.SemiFormal != "" {
		fmt.Fprintf(&b, "**Semi-formal:** %s\n", inv.SemiFormal)
	}
	return b.String()
}

func countLines(s string) int {
	if s == "" {
		return 0
	}
	return strings.Count(s, "\n") + 1
}
