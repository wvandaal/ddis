// Package projector implements synthetic markdown rendering from materialized
// SQLite state (APP-INV-076, APP-INV-077, APP-ADR-061).
package projector

// ddis:implements APP-INV-076 (projection purity — pure functions of SQL state)
// ddis:implements APP-INV-077 (synthetic render — structured fields, not raw_text)
// ddis:implements APP-ADR-061 (field synthesis for projections)

import (
	"fmt"
	"strings"
)

// Invariant represents an invariant's structured fields for rendering.
type Invariant struct {
	ID                string
	Title             string
	Statement         string
	SemiFormal        string
	ViolationScenario string
	ValidationMethod  string
	WhyThisMatters    string
}

// ADR represents an ADR's structured fields for rendering.
type ADR struct {
	ID           string
	Title        string
	Problem      string
	Options      string
	Decision     string
	Consequences string
	Tests        string
}

// Section represents a section's structured fields for rendering.
type Section struct {
	Path  string
	Title string
	Body  string
	Level int
}

// ModuleSpec represents a complete module for rendering.
type ModuleSpec struct {
	Name        string
	Domain      string
	Maintains   []string
	Interfaces  []string
	Implements  []string
	Adjacent    []string
	NegSpecs    []string
	Sections    []Section
	Invariants  []Invariant
	ADRs        []ADR
}

// RenderInvariant renders a single invariant from structured fields (APP-INV-077).
func RenderInvariant(inv Invariant) string {
	var b strings.Builder
	fmt.Fprintf(&b, "**%s: %s**\n\n", inv.ID, inv.Title)
	if inv.Statement != "" {
		fmt.Fprintf(&b, "*%s*\n\n", inv.Statement)
	}
	if inv.SemiFormal != "" {
		fmt.Fprintf(&b, "```\n%s\n```\n\n", inv.SemiFormal)
	}
	if inv.ViolationScenario != "" {
		fmt.Fprintf(&b, "Violation scenario: %s\n\n", inv.ViolationScenario)
	}
	if inv.ValidationMethod != "" {
		fmt.Fprintf(&b, "Validation: %s\n\n", inv.ValidationMethod)
	}
	if inv.WhyThisMatters != "" {
		fmt.Fprintf(&b, "// WHY THIS MATTERS: %s\n\n", inv.WhyThisMatters)
	}
	b.WriteString("---\n")
	return b.String()
}

// RenderADR renders a single ADR from structured fields (APP-INV-077).
func RenderADR(adr ADR) string {
	var b strings.Builder
	fmt.Fprintf(&b, "### %s: %s\n\n", adr.ID, adr.Title)
	if adr.Problem != "" {
		fmt.Fprintf(&b, "#### Problem\n\n%s\n\n", adr.Problem)
	}
	if adr.Options != "" {
		fmt.Fprintf(&b, "#### Options\n\n%s\n\n", adr.Options)
	}
	if adr.Decision != "" {
		fmt.Fprintf(&b, "#### Decision\n\n%s\n\n", adr.Decision)
	}
	if adr.Consequences != "" {
		fmt.Fprintf(&b, "#### Consequences\n\n%s\n\n", adr.Consequences)
	}
	if adr.Tests != "" {
		fmt.Fprintf(&b, "#### Tests\n\n%s\n\n", adr.Tests)
	}
	b.WriteString("---\n")
	return b.String()
}

// RenderModule renders a complete module from structured data (APP-INV-076).
// This is a pure function of the module data — no I/O, no side effects.
func RenderModule(mod ModuleSpec) string {
	var b strings.Builder

	// Frontmatter
	b.WriteString("---\n")
	fmt.Fprintf(&b, "module: %s\n", mod.Name)
	fmt.Fprintf(&b, "domain: %s\n", mod.Domain)
	if len(mod.Maintains) > 0 {
		fmt.Fprintf(&b, "maintains: [%s]\n", strings.Join(mod.Maintains, ", "))
	}
	if len(mod.Interfaces) > 0 {
		fmt.Fprintf(&b, "interfaces: [%s]\n", strings.Join(mod.Interfaces, ", "))
	}
	if len(mod.Implements) > 0 {
		fmt.Fprintf(&b, "implements: [%s]\n", strings.Join(mod.Implements, ", "))
	}
	if len(mod.Adjacent) > 0 {
		fmt.Fprintf(&b, "adjacent: [%s]\n", strings.Join(mod.Adjacent, ", "))
	}
	if len(mod.NegSpecs) > 0 {
		b.WriteString("negative_specs:\n")
		for _, ns := range mod.NegSpecs {
			fmt.Fprintf(&b, "  - %q\n", ns)
		}
	}
	b.WriteString("---\n\n")

	// Module title
	fmt.Fprintf(&b, "# %s Module\n\n", strings.Title(strings.ReplaceAll(mod.Name, "-", " ")))

	// Invariants
	if len(mod.Invariants) > 0 {
		b.WriteString("## Invariants\n\n")
		for _, inv := range mod.Invariants {
			b.WriteString(RenderInvariant(inv))
			b.WriteString("\n")
		}
	}

	// ADRs
	if len(mod.ADRs) > 0 {
		b.WriteString("## Architecture Decision Records\n\n")
		for _, adr := range mod.ADRs {
			b.WriteString(RenderADR(adr))
			b.WriteString("\n")
		}
	}

	// Negative specs
	if len(mod.NegSpecs) > 0 {
		b.WriteString("## Negative Specifications\n\n")
		for _, ns := range mod.NegSpecs {
			fmt.Fprintf(&b, "**DO NOT** %s\n\n", strings.TrimPrefix(ns, "Must NOT "))
		}
	}

	return b.String()
}
