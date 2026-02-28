package discovery

// ddis:maintains APP-INV-039 (task derivation completeness)

// DiscoveryEvent represents a single event from a discovery JSONL stream.
type DiscoveryEvent struct {
	Timestamp string                 `json:"timestamp"`
	Type      string                 `json:"type"`
	ThreadID  string                 `json:"thread_id,omitempty"`
	Data      map[string]interface{} `json:"data"`
}

// DiscoveryState is the reduced state from replaying discovery events.
type DiscoveryState struct {
	ArtifactMap   map[string]*ArtifactEntry `json:"artifact_map"`
	Findings      map[string]interface{}    `json:"findings"`
	OpenQuestions map[string]interface{}    `json:"open_questions"`
	Threads       map[string]*ThreadState   `json:"threads"`
}

// ArtifactEntry is a crystallized spec artifact in the artifact map.
type ArtifactEntry struct {
	ArtifactID       string                   `json:"artifact_id"`
	ArtifactType     string                   `json:"artifact_type"` // adr, invariant, negative_spec, glossary, gate, cross_ref, worked_example
	Title            string                   `json:"title"`
	Domain           string                   `json:"domain,omitempty"`
	Status           string                   `json:"status"` // active, deleted
	Tests            string                   `json:"tests,omitempty"`
	ValidationMethod string                   `json:"validation_method,omitempty"`
	Text             string                   `json:"text,omitempty"`
	Amendments       []map[string]interface{} `json:"amendments,omitempty"`
	Data             map[string]interface{}   `json:"data,omitempty"`
}

// ThreadState tracks discovery thread lifecycle.
type ThreadState struct {
	ThreadID string `json:"thread_id"`
	Status   string `json:"status"` // active, parked, merged
}

// DerivedTask represents a task generated from the artifact map via derivation rules.
type DerivedTask struct {
	ID                 string       `json:"id"`
	Title              string       `json:"title"`
	Type               string       `json:"type"`     // task, test
	Priority           int          `json:"priority"` // 1-3
	Labels             []string     `json:"labels"`
	AcceptanceCriteria string       `json:"acceptance"`
	DependsOn          []string     `json:"depends_on"`
	Metadata           TaskMetadata `json:"metadata"`
	WitnessStatus      string       `json:"witness_status,omitempty"` // ddis:maintains APP-INV-104
}

// TaskMetadata tracks the provenance of a derived task.
type TaskMetadata struct {
	SourceArtifact string `json:"source_artifact"`
	DerivationRule int    `json:"derivation_rule"`
	Phase          string `json:"phase,omitempty"`
}

// TasksOptions controls task derivation.
type TasksOptions struct {
	DiscoveryPath string // path to discovery JSONL
	SpecDB        string // optional spec DB for cross-validation
	Format        string // beads, json, markdown (default: beads)
}

// TasksResult holds the derivation output.
type TasksResult struct {
	Tasks        []DerivedTask `json:"tasks"`
	TotalTasks   int           `json:"total_tasks"`
	ByRule       map[int]int   `json:"by_rule"`
	OrphanedRefs []string      `json:"orphaned_refs,omitempty"`
}
