package parser

import "regexp"

// Compiled regex patterns shared across all recognizers.
var (
	// Heading: # through ######
	HeadingRe = regexp.MustCompile(`^(#{1,6})\s+(.+)$`)

	// **INV-NNN: Title** or **INV-NNN: Title** [Conditional — ...]
	InvHeaderRe = regexp.MustCompile(
		`^\*\*(?P<id>(?:APP-)?INV-\d{3}):\s+(?P<title>[^*]+?)\*\*(?:\s*\[(?P<cond>[^\]]+)\])?$`)

	// *Italic statement*
	InvStatementRe = regexp.MustCompile(`^\*(.+)\*$`)

	// ### ADR-NNN: Title  or  ### ADR-NNN: Title [Conditional — ...]
	ADRHeaderRe = regexp.MustCompile(
		`^###\s+(?P<id>(?:APP-)?ADR-\d{3}):\s+(?P<title>.+?)(?:\s*\[(?P<cond>[^\]]+)\])?\s*$`)

	// **ADR-NNN: Title** — bold format (used in constitution declarations and legacy skeletons)
	ADRBoldHeaderRe = regexp.MustCompile(
		`^\*\*(?P<id>(?:APP-)?ADR-\d{3}):\s+(?P<title>[^*]+?)\*\*`)

	// #### Problem / #### Options / #### Decision / #### Consequences / #### Tests
	ADRSubheadingRe = regexp.MustCompile(`^####\s+(?P<heading>Problem|Options|Decision|Consequences|Tests)\s*$`)

	// Bare text subheading: "Problem:" / "Options considered:" / "Decision:" / "Consequences:" / "Tests:"
	ADRBareSubheadingRe = regexp.MustCompile(`^(?P<heading>Problem|Options considered|Decision|Consequences|Tests):\s*(?P<rest>.*)$`)

	// Option line: A) **Name** or A) Name
	ADROptionRe = regexp.MustCompile(`^(?P<label>[A-Z])\)\s+\*\*(?P<name>[^*]+)\*\*`)

	// - Pros: / - Cons:
	ADRProsConsRe = regexp.MustCompile(`^-\s+(?P<type>Pros|Cons):\s+(?P<text>.+)$`)

	// **Option X: Name.** or **Option X.** within Decision section
	ADRChosenRe = regexp.MustCompile(`\*\*Option\s+(?P<label>[A-Z])(?::\s+[^*]+?)?\.\*\*`)

	// **Gate N: Title** or **Gate M-N: Title**
	GateRe = regexp.MustCompile(
		`^\*\*Gate\s+(?P<id>(?:M-)?[1-9]\d*)(?::\s+(?P<title>[^*]+?))?\*\*`)

	// **DO NOT** constraint (at start of line, possibly with leading - or *)
	NegSpecRe = regexp.MustCompile(`^(?:[-*]\s+)?\*\*DO NOT\*\*\s+(?P<text>.+)$`)

	// > **META-INSTRUCTION ...**: directive
	MetaInstrRe = regexp.MustCompile(
		`^>\s*\*\*META-INSTRUCTION(?:\s*\([^)]*\))?\*?\*?:?\*?\*?\s*(?P<dir>.+)$`)

	// // WHY NOT alternative? explanation  (or WHY NOT alternative — explanation)
	WhyNotRe = regexp.MustCompile(`^//\s*WHY NOT\s+(?P<alt>[^?—]+)[?—]\s*(?P<exp>.+)$`)

	// // WHY THIS MATTERS: explanation
	WhyMattersRe = regexp.MustCompile(`^//\s*WHY THIS MATTERS:\s*(?P<exp>.+)$`)

	// Violation scenario: ...
	ViolationRe = regexp.MustCompile(`^Violation scenario:\s*(?P<text>.+)$`)

	// Validation: ...
	ValidationRe = regexp.MustCompile(`^Validation:\s*(?P<text>.+)$`)

	// ### Verification Prompt for [Chapter]
	VerifPromptRe = regexp.MustCompile(
		`^###?\s*Verification Prompt for\s+(?P<ch>.+)$`)

	// Worked example heading: #### Worked Example or **Worked Example:**
	WorkedExampleRe = regexp.MustCompile(
		`(?i)^(?:#{2,4}\s+)?(?:\*\*)?Worked [Ee]xample`)

	// Module frontmatter delimiter
	FrontmatterRe = regexp.MustCompile(`^---\s*$`)

	// Glossary row: | **Term** | Definition | or | **Term** | Definition |
	GlossaryRowRe = regexp.MustCompile(
		`^\|\s*\*\*(?P<term>[^*]+)\*\*\s*\|\s*(?P<def>.+?)\s*\|$`)

	// Code fence
	CodeFenceRe = regexp.MustCompile("^(`{3,})")

	// Table separator: |---|...|
	TableSepRe = regexp.MustCompile(`^\|[\s-]+\|`)

	// Table row: | ... | ... |
	TableRowRe = regexp.MustCompile(`^\|(.+)\|$`)

	// Cross-reference patterns
	XRefSectionRe = regexp.MustCompile(`§(\d+(?:\.\d+)*)`)
	XRefInvRe     = regexp.MustCompile(`((?:APP-)?INV-\d{3})`)
	XRefADRRe     = regexp.MustCompile(`((?:APP-)?ADR-\d{3})`)
	XRefGateRe    = regexp.MustCompile(`Gate\s+((?:M-)?[1-9]\d*)`)

	// Confidence level in ADR decisions
	ConfidenceRe = regexp.MustCompile(`\*\*Confidence:\s*(Committed|Provisional|Speculative)\*\*`)

	// Section path patterns for normalizing headings to paths
	PartRe     = regexp.MustCompile(`(?i)^PART\s+([0-9IVXLC]+)`)
	SectionRe  = regexp.MustCompile(`^§?\s*(\d+(?:\.\d+)*)`)
	ChapterRe  = regexp.MustCompile(`(?i)^Chapter\s+(\d+)`)
	AppendixRe = regexp.MustCompile(`(?i)^Appendix\s+([A-Z])`)

	// Performance budget header
	PerfBudgetHeaderRe = regexp.MustCompile(`(?i)performance\s+budget`)

	// State machine - detect a table with "State" or "Event" in header
	StateMachineHeaderRe = regexp.MustCompile(`(?i)(?:state|event).*\|.*(?:state|event|transition)`)

	// Comparison block: ❌ or ✅ markers
	ComparisonBadRe  = regexp.MustCompile(`^❌`)
	ComparisonGoodRe = regexp.MustCompile(`^✅`)
)
