package exemplar

// Options configures an exemplar analysis run.
type Options struct {
	Target   string   // Element ID (e.g., "APP-INV-006")
	Gap      string   // Optional: focus on specific component
	Limit    int      // Max exemplars per gap (default 3)
	MinScore float64  // Quality threshold (default 0.3)
	AsJSON   bool     // JSON output mode
	Corpus   []string // Additional .ddis.db paths for cross-spec exemplars
}

// ExemplarResult is the top-level output of an exemplar analysis.
type ExemplarResult struct {
	Target      string         `json:"target"`
	ElementType string         `json:"element_type"`
	Title       string         `json:"title"`
	Gaps        []ComponentGap `json:"gaps"`
	Exemplars   []Exemplar     `json:"exemplars"`
	Guidance    string         `json:"guidance"`
}

// ComponentGap describes a missing or weak component.
type ComponentGap struct {
	Component string  `json:"component"`
	Severity  string  `json:"severity"`   // "missing" or "weak"
	Detail    string  `json:"detail"`
	WeakScore float64 `json:"weak_score"` // 0.0=missing, (0,0.6)=weak
}

// Exemplar is a corpus element that demonstrates a strong version of a gap component.
type Exemplar struct {
	ElementType           string          `json:"element_type"`
	ElementID             string          `json:"element_id"`
	Title                 string          `json:"title"`
	QualityScore          float64         `json:"quality_score"`
	Signals               ExemplarSignals `json:"signals"`
	DemonstratedComponent string          `json:"demonstrated_component"`
	Content               string          `json:"content"`
	SubstrateCue          string          `json:"substrate_cue"`
}

// ExemplarSignals are the 4 quality signals composing the score.
type ExemplarSignals struct {
	Completeness float64 `json:"completeness"`
	Substance    float64 `json:"substance"`
	Authority    float64 `json:"authority"`
	Similarity   float64 `json:"similarity"`
}

// invariantComponents lists the 5 checkable components for invariant elements.
var invariantComponents = []string{
	"statement",
	"semi_formal",
	"violation_scenario",
	"validation_method",
	"why_this_matters",
}

// adrComponents lists the 5 checkable components for ADR elements.
var adrComponents = []string{
	"problem",
	"decision_text",
	"chosen_option",
	"consequences",
	"tests",
}

// ComponentsForType returns the checkable components for the given element type.
func ComponentsForType(elementType string) []string {
	switch elementType {
	case "invariant":
		return invariantComponents
	case "adr":
		return adrComponents
	default:
		return nil
	}
}
