package absorb

// ddis:implements APP-ADR-024 (bilateral specification)
// ddis:maintains APP-INV-032 (symmetric reconciliation)

// Pattern represents a code pattern extracted during scanning.
type Pattern struct {
	File       string  `json:"file"`
	Line       int     `json:"line"`
	Type       string  `json:"type"` // assertion, error_return, interface_def, annotation, state_transition, guard_clause
	Text       string  `json:"text"`
	Confidence float64 `json:"confidence"` // 0.0-1.0
	Language   string  `json:"language"`
}

// AbsorbOptions controls absorption behavior.
type AbsorbOptions struct {
	CodeRoot   string // directory to scan
	AgainstDB  string // spec database for reconciliation (or empty)
	OutputPath string // output path for draft spec (or empty for stdout)
	PromptOnly bool   // emit prompt without side effects
	Depth      int    // conversation depth for k* budget
}

// ReconciliationReport describes the bidirectional gap analysis.
type ReconciliationReport struct {
	Correspondences      []Correspondence    `json:"correspondences"`
	UndocumentedBehavior []UndocumentedItem  `json:"undocumented_behavior"`
	UnimplementedSpec    []UnimplementedItem `json:"unimplemented_spec"`
}

// Correspondence maps a code pattern to a spec element.
type Correspondence struct {
	Pattern     Pattern `json:"pattern"`
	SpecElement string  `json:"spec_element"` // element ID
	ElementType string  `json:"element_type"` // invariant, adr, gate
	Score       float64 `json:"score"`        // similarity score
}

// UndocumentedItem is a code pattern with no spec correspondence.
type UndocumentedItem struct {
	Pattern    Pattern `json:"pattern"`
	Suggestion string  `json:"suggestion"` // suggested spec element type
}

// UnimplementedItem is a spec element with no code evidence.
type UnimplementedItem struct {
	ElementID   string `json:"element_id"`
	ElementType string `json:"element_type"`
	Title       string `json:"title"`
}

// AbsorbResult holds the complete absorption output.
type AbsorbResult struct {
	Patterns       []Pattern             `json:"patterns"`
	TotalFiles     int                   `json:"total_files"`
	TotalPatterns  int                   `json:"total_patterns"`
	Reconciliation *ReconciliationReport `json:"reconciliation,omitempty"`
}
