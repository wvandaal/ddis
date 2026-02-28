package events

// ddis:implements APP-ADR-015 (three-stream event sourcing — typed event schemas)
// ddis:implements APP-ADR-066 (event-sourced architecture — JSONL as canonical record, events are source of truth)
// ddis:maintains APP-INV-020 (event stream append-only — version field enables forward-compatible evolution)
// ddis:maintains APP-INV-053 (event stream completeness — 28 typed event types covering all 3 streams)

import "fmt"

// Stream 1 (Discovery) event types.
const (
	TypeQuestionOpened        = "question_opened"
	TypeAnswerRecorded        = "answer_recorded"
	TypeConfidenceChanged     = "confidence_changed"
	TypeDecisionCrystallized  = "decision_crystallized"
	TypeArtifactWritten       = "artifact_written"
	TypeImplementationFeedback = "implementation_feedback"
	TypeThreadBranched        = "thread_branched"
	TypeThreadMerged          = "thread_merged"
	TypeThreadParked          = "thread_parked"
	TypeModeObserved          = "mode_observed"
	TypeFindingRecorded       = "finding_recorded"
)

// Stream 2 (Specification) event types.
const (
	TypeSpecParsed            = "spec_parsed"
	TypeValidationRun         = "validation_run"
	TypeDriftMeasured         = "drift_measured"
	TypeContradictionDetected = "contradiction_detected"
	TypeAmendmentApplied      = "amendment_applied"

	// Content-bearing event types (APP-INV-072: event content completeness)
	TypeSpecSectionDefined    = "spec_section_defined"
	TypeSpecSectionUpdated    = "spec_section_updated"
	TypeSpecSectionRemoved    = "spec_section_removed"
	TypeInvariantCrystallized = "invariant_crystallized"
	TypeInvariantUpdated      = "invariant_updated"
	TypeInvariantRemoved      = "invariant_removed"
	TypeADRCrystallized       = "adr_crystallized"
	TypeADRUpdated            = "adr_updated"
	TypeADRSuperseded         = "adr_superseded"
	TypeNegativeSpecAdded     = "negative_spec_added"
	TypeQualityGateDefined    = "quality_gate_defined"
	TypeCrossRefAdded         = "cross_ref_added"
	TypeGlossaryTermDefined   = "glossary_term_defined"
	TypeModuleRegistered      = "module_registered"
	TypeManifestUpdated       = "manifest_updated"
	TypeSnapshotCreated       = "snapshot_created"
)

// Stream 3 (Implementation) event types.
const (
	TypeIssueCreated         = "issue_created"
	TypeStatusChanged        = "status_changed"
	TypeDependencyResolved   = "dependency_resolved"
	TypeImplementationFinding = "implementation_finding"
	TypeChallengeIssued      = "challenge_issued"
	TypeChallengeBatch       = "challenge_batch"

	// Triage lifecycle events (APP-ADR-053: event-sourced issue state machine)
	TypeIssueTriaged      = "issue_triaged"
	TypeIssueSpecified    = "issue_specified"
	TypeIssueImplementing = "issue_implementing"
	TypeIssueVerified     = "issue_verified"
	TypeIssueClosed       = "issue_closed"
	TypeIssueWontfix      = "issue_wontfix"

	// Content-bearing witness/challenge events (APP-INV-071: log canonicality)
	TypeWitnessRecorded    = "witness_recorded"
	TypeWitnessRevoked     = "witness_revoked"
	TypeWitnessInvalidated = "witness_invalidated"
	TypeChallengeCompleted = "challenge_completed"
)

// streamEventTypes maps each stream to its valid event types.
var streamEventTypes = map[Stream]map[string]bool{
	StreamDiscovery: {
		TypeQuestionOpened:        true,
		TypeAnswerRecorded:        true,
		TypeConfidenceChanged:     true,
		TypeDecisionCrystallized:  true,
		TypeArtifactWritten:       true,
		TypeImplementationFeedback: true,
		TypeThreadBranched:        true,
		TypeThreadMerged:          true,
		TypeThreadParked:          true,
		TypeModeObserved:          true,
		TypeFindingRecorded:       true,
	},
	StreamSpecification: {
		TypeSpecParsed:            true,
		TypeValidationRun:         true,
		TypeDriftMeasured:         true,
		TypeContradictionDetected: true,
		TypeAmendmentApplied:      true,
		TypeSpecSectionDefined:    true,
		TypeSpecSectionUpdated:    true,
		TypeSpecSectionRemoved:    true,
		TypeInvariantCrystallized: true,
		TypeInvariantUpdated:      true,
		TypeInvariantRemoved:      true,
		TypeADRCrystallized:       true,
		TypeADRUpdated:            true,
		TypeADRSuperseded:         true,
		TypeNegativeSpecAdded:     true,
		TypeQualityGateDefined:    true,
		TypeCrossRefAdded:         true,
		TypeGlossaryTermDefined:   true,
		TypeModuleRegistered:      true,
		TypeManifestUpdated:       true,
		TypeSnapshotCreated:       true,
	},
	StreamImplementation: {
		TypeIssueCreated:         true,
		TypeStatusChanged:        true,
		TypeDependencyResolved:   true,
		TypeImplementationFinding: true,
		TypeChallengeIssued:      true,
		TypeChallengeBatch:       true,
		TypeIssueTriaged:        true,
		TypeIssueSpecified:      true,
		TypeIssueImplementing:   true,
		TypeIssueVerified:       true,
		TypeIssueClosed:         true,
		TypeIssueWontfix:        true,
		TypeWitnessRecorded:     true,
		TypeWitnessRevoked:      true,
		TypeWitnessInvalidated:  true,
		TypeChallengeCompleted:  true,
	},
}

// ValidateEvent checks that the event type is valid for its stream.
func ValidateEvent(e *Event) error {
	if e.Stream < 1 || e.Stream > 3 {
		return fmt.Errorf("invalid stream: %d (must be 1, 2, or 3)", e.Stream)
	}
	types, ok := streamEventTypes[e.Stream]
	if !ok {
		return fmt.Errorf("unknown stream: %d", e.Stream)
	}
	if !types[e.Type] {
		return fmt.Errorf("event type %q not valid for stream %d", e.Type, e.Stream)
	}
	if e.ID == "" {
		return fmt.Errorf("event ID is required")
	}
	if e.Timestamp == "" {
		return fmt.Errorf("event timestamp is required")
	}
	return nil
}
