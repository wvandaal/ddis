package events

// ddis:implements APP-ADR-015 (three-stream event sourcing — typed event schemas)
// ddis:maintains APP-INV-020 (event stream append-only — version field enables forward-compatible evolution)
// ddis:maintains APP-INV-053 (event stream completeness — 22 typed event types covering all 3 streams)

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
)

// Stream 3 (Implementation) event types.
const (
	TypeIssueCreated        = "issue_created"
	TypeStatusChanged       = "status_changed"
	TypeDependencyResolved  = "dependency_resolved"
	TypeImplementationFinding = "implementation_finding"
	TypeChallengeIssued      = "challenge_issued"
	TypeChallengeBatch       = "challenge_batch"
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
	},
	StreamImplementation: {
		TypeIssueCreated:        true,
		TypeStatusChanged:       true,
		TypeDependencyResolved:  true,
		TypeImplementationFinding: true,
		TypeChallengeIssued:      true,
		TypeChallengeBatch:       true,
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
