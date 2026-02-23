package storage

// SpecIndex represents the top-level parsed spec.
type SpecIndex struct {
	ID          int64
	SpecPath    string
	SpecName    string
	DDISVersion string
	TotalLines  int
	ContentHash string
	ParsedAt    string
	SourceType  string // "monolith" or "modular"
}

// SourceFile represents one parsed source file.
type SourceFile struct {
	ID          int64
	SpecID      int64
	FilePath    string
	FileRole    string // monolith/manifest/system_constitution/domain_constitution/deep_context/module
	ModuleName  string // NULL for non-module files
	ContentHash string
	LineCount   int
	RawText     string
}

// Section represents a heading-delimited section.
type Section struct {
	ID           int64
	SpecID       int64
	SourceFileID int64
	SectionPath  string // "§0.5", "PART-0", "Chapter-3", "Appendix-A"
	Title        string
	HeadingLevel int
	ParentID     *int64
	LineStart    int
	LineEnd      int
	RawText      string
	ContentHash  string
}

// Invariant represents an INV-NNN block with up to 6 components.
type Invariant struct {
	ID                int64
	SpecID            int64
	SourceFileID      int64
	SectionID         int64
	InvariantID       string // "INV-006" or "APP-INV-017"
	Title             string
	Statement         string
	SemiFormal        string
	ViolationScenario string
	ValidationMethod  string
	WhyThisMatters    string
	ConditionalTag    string
	LineStart         int
	LineEnd           int
	RawText           string
	ContentHash       string
}

// ADR represents an ADR block.
type ADR struct {
	ID           int64
	SpecID       int64
	SourceFileID int64
	SectionID    int64
	ADRID        string
	Title        string
	Problem      string
	DecisionText string
	ChosenOption string
	Consequences string
	Tests        string
	Confidence   string
	Status       string
	SupersededBy string
	LineStart    int
	LineEnd      int
	RawText      string
	ContentHash  string
}

// ADROption represents one option within an ADR.
type ADROption struct {
	ID          int64
	ADRID       int64
	OptionLabel string // "A", "B", "C"
	OptionName  string
	Pros        string
	Cons        string
	IsChosen    bool
	WhyNot      string
}

// QualityGate represents a quality gate.
type QualityGate struct {
	ID        int64
	SpecID    int64
	SectionID int64
	GateID    string // "Gate-1", "Gate-M-3"
	Title     string
	Predicate string
	IsModular bool
	LineStart int
	LineEnd   int
	RawText   string
}

// NegativeSpec represents a DO NOT constraint.
type NegativeSpec struct {
	ID             int64
	SpecID         int64
	SourceFileID   int64
	SectionID      int64
	ConstraintText string
	Reason         string
	InvariantRef   string
	LineNumber     int
	RawText        string
}

// VerificationPrompt represents a verification prompt block.
type VerificationPrompt struct {
	ID          int64
	SpecID      int64
	SectionID   int64
	ChapterName string
	LineStart   int
	LineEnd     int
	RawText     string
}

// VerificationCheck is an individual check within a prompt.
type VerificationCheck struct {
	ID           int64
	PromptID     int64
	CheckType    string // "positive", "negative", "integration"
	CheckText    string
	InvariantRef string
	Ordinal      int
}

// MetaInstruction represents a META-INSTRUCTION block.
type MetaInstruction struct {
	ID        int64
	SpecID    int64
	SectionID int64
	Directive string
	Reason    string
	LineStart int
	LineEnd   int
	RawText   string
}

// WorkedExample represents a worked example block.
type WorkedExample struct {
	ID        int64
	SpecID    int64
	SectionID int64
	Title     string
	LineStart int
	LineEnd   int
	RawText   string
}

// WhyNotAnnotation represents a WHY NOT inline annotation.
type WhyNotAnnotation struct {
	ID          int64
	SpecID      int64
	SectionID   int64
	Alternative string
	Explanation string
	ADRRef      string
	LineNumber  int
	RawText     string
}

// ComparisonBlock represents a comparison block.
type ComparisonBlock struct {
	ID                 int64
	SpecID             int64
	SectionID          int64
	SuboptimalApproach string
	ChosenApproach     string
	SuboptimalReasons  string
	ChosenReasons      string
	ADRRef             string
	LineStart          int
	LineEnd            int
	RawText            string
}

// PerformanceBudget represents a performance budget table.
type PerformanceBudget struct {
	ID          int64
	SpecID      int64
	SectionID   int64
	DesignPoint string
	LineStart   int
	LineEnd     int
	RawText     string
}

// BudgetEntry is one row in a performance budget table.
type BudgetEntry struct {
	ID                int64
	BudgetID          int64
	MetricID          string
	Operation         string
	Target            string
	MeasurementMethod string
	Ordinal           int
}

// StateMachine represents a state machine table.
type StateMachine struct {
	ID        int64
	SpecID    int64
	SectionID int64
	Title     string
	LineStart int
	LineEnd   int
	RawText   string
}

// StateMachineCell is one cell in a state machine table.
type StateMachineCell struct {
	ID         int64
	MachineID  int64
	StateName  string
	EventName  string
	Transition string
	Guard      string
	IsInvalid  bool
}

// GlossaryEntry represents a glossary term.
type GlossaryEntry struct {
	ID         int64
	SpecID     int64
	SectionID  int64
	Term       string
	Definition string
	SectionRef string
	LineNumber int
}

// CrossReference represents a cross-reference link.
type CrossReference struct {
	ID              int64
	SpecID          int64
	SourceFileID    int64
	SourceSectionID *int64
	SourceLine      int
	RefType         string // "section", "invariant", "adr", "gate", etc.
	RefTarget       string
	RefText         string
	Resolved        bool
}

// Module represents a modular spec module.
type Module struct {
	ID              int64
	SpecID          int64
	SourceFileID    int64
	ModuleName      string
	Domain          string
	DeepContextPath string
	LineCount       int
}

// ModuleRelationship represents an inter-module relationship.
type ModuleRelationship struct {
	ID       int64
	ModuleID int64
	RelType  string // "maintains", "interfaces", "implements", "adjacent"
	Target   string
}

// ModuleNegativeSpec represents a module-level negative spec.
type ModuleNegativeSpec struct {
	ID             int64
	ModuleID       int64
	ConstraintText string
}

// Manifest represents parsed manifest.yaml data.
type Manifest struct {
	ID               int64
	SpecID           int64
	DDISVersion      string
	SpecName         string
	TierMode         string
	TargetLines      int
	HardCeilingLines int
	ReasoningReserve float64
	RawYAML          string
}

// InvariantRegistryEntry represents one invariant in the registry.
type InvariantRegistryEntry struct {
	ID          int64
	SpecID      int64
	InvariantID string
	Owner       string
	Domain      string
	Description string
}

// RefCounts holds incoming/outgoing cross-reference counts for a section.
type RefCounts struct {
	Incoming int
	Outgoing int
}

// Transaction represents a spec modification transaction.
type Transaction struct {
	ID          int64
	SpecID      int64
	TxID        string
	Description string
	Status      string // "pending", "committed", "rolled_back"
	CreatedAt   string
	CommittedAt *string
	ParentTxID  *string
}

// TxOperation represents one operation within a transaction.
type TxOperation struct {
	ID            int64
	TxID          string
	Ordinal       int
	OperationType string
	OperationData string
	ImpactSet     *string
	AppliedAt     *string
}

// FormattingHint records formatting details for round-trip fidelity.
type FormattingHint struct {
	ID           int64
	SpecID       int64
	SourceFileID int64
	LineNumber   int
	HintType     string
	HintValue    string
}
