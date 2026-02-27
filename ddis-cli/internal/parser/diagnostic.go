package parser

// ddis:maintains APP-INV-058
// ddis:implements APP-ADR-045

import "fmt"

// ParseDiagnostic represents a structured warning emitted during parsing
// when a spec element header is recognized but the element structure is
// incomplete (e.g., invariant header without statement, ADR without subheadings).
type ParseDiagnostic struct {
	ElementID  string // e.g. "APP-INV-042"
	FilePath   string // source file path
	Line       int    // 1-indexed line number
	Deficiency string // human-readable description of what's missing
}

// FormatDiagnostic formats a diagnostic in the standard compiler-style format:
// parse: warning: file:line: id missing component
func FormatDiagnostic(d ParseDiagnostic) string {
	return fmt.Sprintf("parse: warning: %s:%d: %s %s", d.FilePath, d.Line, d.ElementID, d.Deficiency)
}

// Diagnostics collects parse diagnostics during a parse session.
// It is safe to append concurrently from different extractors within
// a single-threaded parse (extractors run sequentially).
type Diagnostics struct {
	items []ParseDiagnostic
}

// Add appends a diagnostic.
func (d *Diagnostics) Add(diag ParseDiagnostic) {
	d.items = append(d.items, diag)
}

// All returns all collected diagnostics.
func (d *Diagnostics) All() []ParseDiagnostic {
	if d == nil {
		return nil
	}
	return d.items
}

// Len returns the number of diagnostics.
func (d *Diagnostics) Len() int {
	if d == nil {
		return 0
	}
	return len(d.items)
}

// GlobalDiagnostics collects diagnostics during a parse session.
// Reset at the start of each parse operation. Safe for single-threaded use.
var GlobalDiagnostics Diagnostics

// ResetDiagnostics clears the global diagnostics collector.
func ResetDiagnostics() {
	GlobalDiagnostics = Diagnostics{}
}
