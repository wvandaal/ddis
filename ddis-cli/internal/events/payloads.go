package events

// ddis:implements APP-INV-072 (event content completeness — structured payloads for content events)
// ddis:implements APP-ADR-058 (JSONL as canonical representation — content payloads enable fold)
// ddis:implements APP-ADR-073 (snapshot format — SnapshotPayload carries position + SHA-256 state hash)

// SectionPayload carries the full content of a spec section.
type SectionPayload struct {
	Module string `json:"module"`
	Path   string `json:"path"`
	Title  string `json:"title"`
	Body   string `json:"body"`
	Level  int    `json:"level"`
}

// SectionUpdatePayload carries updates to a spec section.
type SectionUpdatePayload struct {
	Module  string            `json:"module"`
	Path    string            `json:"path"`
	Title   string            `json:"title"`
	Body    string            `json:"body"`
	Changes map[string]string `json:"changes,omitempty"`
}

// SectionRemovePayload carries a section removal.
type SectionRemovePayload struct {
	Module string `json:"module"`
	Path   string `json:"path"`
	Reason string `json:"reason"`
}

// InvariantPayload carries the full content of a crystallized invariant.
type InvariantPayload struct {
	ID                string `json:"id"`
	Title             string `json:"title"`
	Statement         string `json:"statement"`
	SemiFormal        string `json:"semi_formal"`
	ViolationScenario string `json:"violation_scenario"`
	ValidationMethod  string `json:"validation_method"`
	WhyThisMatters    string `json:"why_this_matters"`
	Module            string `json:"module"`
	Synthetic         bool   `json:"synthetic,omitempty"`
}

// InvariantUpdatePayload carries updates to an invariant.
type InvariantUpdatePayload struct {
	ID            string            `json:"id"`
	FieldsChanged []string          `json:"fields_changed"`
	NewValues     map[string]string `json:"new_values"`
}

// InvariantRemovePayload carries an invariant removal.
type InvariantRemovePayload struct {
	ID           string `json:"id"`
	Reason       string `json:"reason"`
	SupersededBy string `json:"superseded_by,omitempty"`
}

// ADRPayload carries the full content of a crystallized ADR.
type ADRPayload struct {
	ID           string `json:"id"`
	Title        string `json:"title"`
	Problem      string `json:"problem"`
	Options      string `json:"options"`
	Decision     string `json:"decision"`
	Consequences string `json:"consequences"`
	Tests        string `json:"tests"`
	Module       string `json:"module"`
	Synthetic    bool   `json:"synthetic,omitempty"`
}

// ADRUpdatePayload carries updates to an ADR.
type ADRUpdatePayload struct {
	ID            string            `json:"id"`
	FieldsChanged []string          `json:"fields_changed"`
	NewValues     map[string]string `json:"new_values"`
}

// ADRSupersededPayload carries an ADR supersession.
type ADRSupersededPayload struct {
	ID           string `json:"id"`
	SupersededBy string `json:"superseded_by"`
	Reason       string `json:"reason"`
}

// NegativeSpecPayload carries a negative specification addition.
type NegativeSpecPayload struct {
	Module    string `json:"module"`
	Pattern   string `json:"pattern"`
	Rationale string `json:"rationale"`
}

// QualityGatePayload carries a quality gate definition.
type QualityGatePayload struct {
	GateNumber int    `json:"gate_number"`
	Title      string `json:"title"`
	Predicate  string `json:"predicate"`
}

// CrossRefPayload carries a cross-reference addition.
type CrossRefPayload struct {
	Source  string `json:"source"`
	Target string `json:"target"`
	Context string `json:"context,omitempty"`
}

// GlossaryTermPayload carries a glossary term definition.
type GlossaryTermPayload struct {
	Term       string `json:"term"`
	Definition string `json:"definition"`
	Module     string `json:"module,omitempty"`
}

// ModulePayload carries a module registration.
type ModulePayload struct {
	Name       string   `json:"name"`
	Domain     string   `json:"domain"`
	Maintains  []string `json:"maintains"`
	Interfaces []string `json:"interfaces"`
	Implements []string `json:"implements"`
	Adjacent   []string `json:"adjacent"`
}

// ManifestUpdatePayload carries a manifest field update.
type ManifestUpdatePayload struct {
	Field    string `json:"field"`
	OldValue string `json:"old_value,omitempty"`
	NewValue string `json:"new_value"`
}

// SnapshotPayload carries snapshot creation metadata.
type SnapshotPayload struct {
	Position  int    `json:"position"`
	StateHash string `json:"state_hash"`
}

// WitnessPayload carries a witness recording.
type WitnessPayload struct {
	InvariantID  string `json:"invariant_id"`
	EvidenceType string `json:"evidence_type"`
	Evidence     string `json:"evidence"`
	By           string `json:"by"`
	Model        string `json:"model,omitempty"`
	CodeHash     string `json:"code_hash,omitempty"`
	SpecHash     string `json:"spec_hash"`
}

// WitnessRevokePayload carries a witness revocation.
type WitnessRevokePayload struct {
	InvariantID string `json:"invariant_id"`
	Reason      string `json:"reason"`
}

// ChallengePayload carries a challenge result.
type ChallengePayload struct {
	InvariantID string  `json:"invariant_id"`
	Verdict     string  `json:"verdict"`
	Score       float64 `json:"score"`
	Detail      string  `json:"detail,omitempty"`
}
