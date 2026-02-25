package annotate

// Annotation represents a single ddis: annotation found in source code.
type Annotation struct {
	FilePath   string `json:"file_path"`
	Line       int    `json:"line"`
	Verb       string `json:"verb"`
	Target     string `json:"target"`
	Qualifier  string `json:"qualifier,omitempty"`
	Language   string `json:"language"`
	RawComment string `json:"raw_comment"`
}

// ScanOptions controls scan behavior.
type ScanOptions struct {
	Root         string   // directory to scan
	SpecDB       string   // path to spec database (for --verify)
	Verify       bool     // verify annotations against spec
	Store        bool     // store annotations in spec DB
	ExcludeGlobs []string // patterns to skip (default: .git, vendor, node_modules)
	AsJSON       bool     // output as JSON
}

// ScanResult holds the complete scan output.
type ScanResult struct {
	Annotations    []Annotation    `json:"annotations"`
	FilesScanned   int             `json:"files_scanned"`
	FilesSkipped   int             `json:"files_skipped"`
	TotalFound     int             `json:"total_found"`
	ByVerb         map[string]int  `json:"by_verb"`
	ByLanguage     map[string]int  `json:"by_language"`
	VerifyReport   *VerifyReport   `json:"verify_report,omitempty"`
}

// VerifyReport shows annotation-spec correspondence.
type VerifyReport struct {
	Resolved      []ResolvedAnnotation `json:"resolved"`
	Orphaned      []Annotation         `json:"orphaned"`
	Unimplemented []string             `json:"unimplemented"`
}

// ResolvedAnnotation is an annotation that maps to an existing spec element.
type ResolvedAnnotation struct {
	Annotation
	ElementType string `json:"element_type"` // invariant, adr, gate
	ElementID   string `json:"element_id"`
}
