package parser

import "regexp"

// Compiled regex patterns shared across all recognizers.
var (
	// Heading: # through ######
	HeadingRe = regexp.MustCompile(`^(#{1,6})\s+(.+)$`)

	// Canonical invariant header: **NS-INV-NNN: Title** with optional
	// [Conditional] tag and an optional (Key: Value) parenthetical suffix
	// (e.g. (Owner: foo) — convention shared with constitution registry entries).
	InvHeaderRe = regexp.MustCompile(
		`^\*\*(?P<id>(?:[A-Z]{2,5}-)?INV-\d{3}):\s+(?P<title>[^*]+?)\*\*(?:\s*\[(?P<cond>[^\]]+)\])?(?:\s*\([^)]+\))?\s*$`)

	// CMP-dialect invariant header: ### NS-INV-NNN — Title (h3, em-dash
	// or hyphen separator). Used by packages/components/docs/prompts/
	// CMP-constitution.md and any other namespace adopting the same form.
	InvHeaderH3Re = regexp.MustCompile(
		`^###\s+(?P<id>(?:[A-Z]{2,5}-)?INV-\d{3})\s+[—–-]\s+(?P<title>.+?)(?:\s*\[(?P<cond>[^\]]+)\])?(?:\s*\([^)]+\))?\s*$`)

	// *Italic statement*
	InvStatementRe = regexp.MustCompile(`^\*(.+)\*$`)

	// ADR header — accepts colon (canonical) or em-dash/hyphen (CMP dialect)
	// as the ID/title separator. Optional [Conditional] tag and trailing
	// (Implements: x) parenthetical suffix.
	ADRHeaderRe = regexp.MustCompile(
		`^###\s+(?P<id>(?:[A-Z]{2,5}-)?ADR-\d{3})(?:\s*:\s*|\s+[—–-]\s+)(?P<title>.+?)(?:\s*\[(?P<cond>[^\]]+)\])?(?:\s*\([^)]+\))?\s*$`)

	// **ADR-NNN: Title** — bold format (used in constitution declarations and legacy skeletons)
	ADRBoldHeaderRe = regexp.MustCompile(
		`^\*\*(?P<id>(?:[A-Z]{2,5}-)?ADR-\d{3}):\s+(?P<title>[^*]+?)\*\*`)

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

	// Violation prefix — accepts: "Violation scenario:", "Violation:" (synonym),
	// or any of those wrapped in **bold** (CMP dialect convention).
	ViolationRe = regexp.MustCompile(`^(?:\*\*)?Violation(?:\s+scenario)?(?:\*\*)?:\s*(?P<text>.+)$`)

	// Validation prefix — accepts: "Validation:", "Validation strategy:" (synonym),
	// or any of those wrapped in **bold** (CMP dialect convention).
	ValidationRe = regexp.MustCompile(`^(?:\*\*)?Validation(?:\s+strategy)?(?:\*\*)?:\s*(?P<text>.+)$`)

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

	// Inline code span — single-backtick-delimited content on a single line.
	// Used by ExtractCrossReferences to skip refs inside `inline code` (e.g.
	// `docs/foo.md §3.2` mentions a section in another doc, not in our spec).
	// Multi-backtick spans (``foo``) are rare in spec prose; covered well
	// enough by the simple form for our purpose.
	InlineCodeRe = regexp.MustCompile("`[^`\n]*`")

	// Table separator: |---|...|
	TableSepRe = regexp.MustCompile(`^\|[\s-]+\|`)

	// Table row: | ... | ... |
	TableRowRe = regexp.MustCompile(`^\|(.+)\|$`)

	// Cross-reference patterns
	// XRefInvRe / XRefADRRe accept any 2-5 uppercase letter namespace prefix
	// (APP, CMP, DOM, …) plus the legacy bare form. Single capture group is
	// preserved (m[1] = full ID); namespace can be derived via NamespaceOf().
	XRefSectionRe = regexp.MustCompile(`§(\d+(?:\.\d+)*)`)
	XRefInvRe     = regexp.MustCompile(`((?:[A-Z]{2,5}-)?INV-\d{3})`)
	XRefADRRe     = regexp.MustCompile(`((?:[A-Z]{2,5}-)?ADR-\d{3})`)
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

	// SUPERSEDED by ADR-NNN or SUPERSEDED BY APP-ADR-NNN (case-insensitive)
	supersededByRe = regexp.MustCompile(`(?i)SUPERSEDED\s+(?:by\s+)?(?P<id>(?:APP-)?ADR-\d{3})`)
)
