package discover

// ddis:maintains APP-INV-027 (thread topology primacy)
// ddis:maintains APP-INV-029 (convergent thread selection)

// Thread represents a discovery inquiry thread.
type Thread struct {
	ID             string   `json:"id"`
	Status         string   `json:"status"` // active, parked, merged
	Summary        string   `json:"summary"`
	SpecAttachment []string `json:"spec_attachment,omitempty"`
	CreatedAt      string   `json:"created_at"`
	LastEventAt    string   `json:"last_event_at"`
	EventCount     int      `json:"event_count"`
	Confidence     [5]int   `json:"confidence"`
}

// Event represents a discovery event in the JSONL stream.
type Event struct {
	Version   int                    `json:"version"`
	Type      string                 `json:"type"` // session_started, mode_observed, finding_recorded, question_opened, question_closed, challenge_posed, decision_crystallized, thread_parked, thread_resumed
	Timestamp string                 `json:"timestamp"`
	ThreadID  string                 `json:"thread_id"`
	SessionID string                 `json:"session_id,omitempty"`
	Sequence  int                    `json:"sequence"`
	Data      map[string]interface{} `json:"data"`
}

// DiscoverOptions controls discovery behavior.
type DiscoverOptions struct {
	SpecDB    string // path to spec database
	ThreadID  string // explicit thread override (or empty for auto-convergence)
	Content   string // user content for thread matching
	Depth     int    // conversation depth for k* budget
	EventsDir string // path to events directory (default: .ddis/events/)
}

// ThreadMatchResult describes how a thread was selected.
type ThreadMatchResult struct {
	ThreadID string  `json:"thread_id"`
	Score    float64 `json:"score"`
	Method   string  `json:"method"` // convergent, user_override, new_thread
}

// ModeClassification describes the observed cognitive mode.
type ModeClassification struct {
	Mode       string  `json:"mode"` // divergent, convergent, dialectical, abductive, metacognitive, incubation, crystallization
	Confidence float64 `json:"confidence"`
	Evidence   string  `json:"evidence"`
	DoFHint    string  `json:"dof_hint"` // very_low, low, mid, high, very_high
}
