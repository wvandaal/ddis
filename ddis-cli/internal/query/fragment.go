package query

import (
	"encoding/json"
	"fmt"
	"strings"
)

// FragmentType identifies what kind of spec element a fragment represents.
type FragmentType string

const (
	FragmentSection   FragmentType = "section"
	FragmentInvariant FragmentType = "invariant"
	FragmentADR       FragmentType = "adr"
	FragmentGate      FragmentType = "gate"
)

// OutputFormat controls how fragments are rendered.
type OutputFormat string

const (
	FormatMarkdown OutputFormat = "markdown"
	FormatJSON     OutputFormat = "json"
	FormatRaw      OutputFormat = "raw"
)

// Fragment is the assembled query result for a single spec element.
type Fragment struct {
	Type         FragmentType  `json:"type"`
	ID           string        `json:"id"`
	Title        string        `json:"title"`
	RawText      string        `json:"raw_text"`
	LineStart    int           `json:"line_start"`
	LineEnd      int           `json:"line_end"`
	SectionPath  string        `json:"section_path,omitempty"`
	ResolvedRefs []ResolvedRef `json:"resolved_refs,omitempty"`
	GlossaryDefs []GlossaryDef `json:"glossary_defs,omitempty"`
	Backlinks    []Backlink    `json:"backlinks,omitempty"`
}

// ResolvedRef is a cross-reference from this fragment to another element.
type ResolvedRef struct {
	RefType  string `json:"ref_type"`
	Target   string `json:"target"`
	RefText  string `json:"ref_text"`
	Resolved bool   `json:"resolved"`
	RawText  string `json:"raw_text,omitempty"`
}

// GlossaryDef is a glossary term found in the fragment text.
type GlossaryDef struct {
	Term       string `json:"term"`
	Definition string `json:"definition"`
}

// Backlink is a cross-reference from another element to this fragment.
type Backlink struct {
	SourceSection string `json:"source_section,omitempty"`
	SourceLine    int    `json:"source_line"`
	RefType       string `json:"ref_type"`
	RefText       string `json:"ref_text"`
}

// RenderFragment formats a fragment according to the specified output format.
func RenderFragment(f *Fragment, format OutputFormat) (string, error) {
	switch format {
	case FormatJSON:
		data, err := json.MarshalIndent(f, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal fragment: %w", err)
		}
		return string(data), nil

	case FormatRaw:
		return f.RawText, nil

	case FormatMarkdown:
		return renderMarkdown(f), nil

	default:
		return "", fmt.Errorf("unknown format: %s", format)
	}
}

func renderMarkdown(f *Fragment) string {
	var b strings.Builder

	// Header
	fmt.Fprintf(&b, "## %s: %s\n", f.ID, f.Title)
	fmt.Fprintf(&b, "*Type: %s | Lines: %d–%d", f.Type, f.LineStart, f.LineEnd)
	if f.SectionPath != "" {
		fmt.Fprintf(&b, " | Section: %s", f.SectionPath)
	}
	b.WriteString("*\n\n")

	// Raw text
	b.WriteString(f.RawText)
	if !strings.HasSuffix(f.RawText, "\n") {
		b.WriteString("\n")
	}

	// Resolved refs
	if len(f.ResolvedRefs) > 0 {
		b.WriteString("\n### Outgoing References\n\n")
		for _, ref := range f.ResolvedRefs {
			status := "resolved"
			if !ref.Resolved {
				status = "UNRESOLVED"
			}
			fmt.Fprintf(&b, "- **%s** → `%s` [%s] (%s)\n", ref.RefType, ref.Target, status, ref.RefText)
			if ref.RawText != "" {
				// Show first 3 lines of the referenced element
				lines := strings.SplitN(ref.RawText, "\n", 4)
				for i, line := range lines {
					if i >= 3 {
						b.WriteString("  > ...\n")
						break
					}
					fmt.Fprintf(&b, "  > %s\n", line)
				}
			}
		}
	}

	// Glossary
	if len(f.GlossaryDefs) > 0 {
		b.WriteString("\n### Glossary Terms\n\n")
		for _, g := range f.GlossaryDefs {
			fmt.Fprintf(&b, "- **%s**: %s\n", g.Term, g.Definition)
		}
	}

	// Backlinks
	if len(f.Backlinks) > 0 {
		b.WriteString("\n### Backlinks\n\n")
		for _, bl := range f.Backlinks {
			src := bl.SourceSection
			if src == "" {
				src = "(unknown section)"
			}
			fmt.Fprintf(&b, "- %s (line %d): %s [%s]\n", src, bl.SourceLine, bl.RefText, bl.RefType)
		}
	}

	return b.String()
}
