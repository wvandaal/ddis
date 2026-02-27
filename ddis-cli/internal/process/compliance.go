package process

// ddis:maintains APP-INV-056 (process compliance observability)
// ddis:implements APP-ADR-043 (observational compliance over prescriptive gates)

import (
	"database/sql"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/wvandaal/ddis/internal/oplog"
	"github.com/wvandaal/ddis/internal/storage"
)

// Info summarizes process compliance for a spec element or feature scope.
type Info struct {
	Score           float64  `json:"score"`              // composite PC score 0.0-1.0
	SpecFirstRatio  float64  `json:"spec_first_ratio"`   // R_spec
	ToolUsage       float64  `json:"tool_usage"`         // R_tool
	WitnessCoverage float64  `json:"witness_coverage"`   // R_witness
	ValidationGate  float64  `json:"validation_gate"`    // R_validate
	Degraded        []string `json:"degraded,omitempty"` // which signals degraded
	Recommendation  string   `json:"recommendation"`     // workflow guidance
}

// Weights for the PC score formula.
const (
	WeightSpec     = 0.35
	WeightTool     = 0.20
	WeightWitness  = 0.25
	WeightValidate = 0.20
)

// Options controls process compliance computation.
type Options struct {
	OplogPath string // path to oplog.jsonl (empty = degrade)
	CodeRoot  string // path to code root for git analysis (empty = degrade)
	SpecRoot  string // path to spec root for git analysis (empty = degrade)
}

// Compute calculates the process compliance score for a spec.
// Follows APP-INV-030: graceful degradation when data sources are missing.
func Compute(db *sql.DB, specID int64, opts Options) *Info {
	info := &Info{}

	// R_spec: spec-first ordering (git log analysis)
	info.SpecFirstRatio = computeSpecFirst(opts)
	if info.SpecFirstRatio == 0.5 && opts.CodeRoot == "" {
		info.Degraded = append(info.Degraded, "spec_first_ratio (no git)")
	}

	// R_tool: auto-prompting tool intermediation
	info.ToolUsage = computeToolUsage(opts, specID)
	if info.ToolUsage == 0.5 && opts.OplogPath == "" {
		info.Degraded = append(info.Degraded, "tool_usage (no oplog)")
	}

	// R_witness: witness coverage (always computable from DB)
	info.WitnessCoverage = computeWitnessCoverage(db, specID)

	// R_validate: validation gate
	info.ValidationGate = computeValidationGate(opts)
	if info.ValidationGate == 0.5 && opts.OplogPath == "" {
		info.Degraded = append(info.Degraded, "validation_gate (no oplog)")
	}

	// Composite score
	info.Score = WeightSpec*info.SpecFirstRatio +
		WeightTool*info.ToolUsage +
		WeightWitness*info.WitnessCoverage +
		WeightValidate*info.ValidationGate

	// Generate recommendation based on weakest sub-score
	info.Recommendation = generateRecommendation(info)

	return info
}

// computeSpecFirst checks if spec files were modified before code files.
// Uses git log to compare commit timestamps.
// Degrades to 0.5 when git is unavailable.
func computeSpecFirst(opts Options) float64 {
	if opts.CodeRoot == "" || opts.SpecRoot == "" {
		return 0.5 // neutral degradation
	}

	// Check if git is available in code root
	cmd := exec.Command("git", "-C", opts.CodeRoot, "rev-parse", "--git-dir")
	if err := cmd.Run(); err != nil {
		return 0.5 // no git, neutral degradation
	}

	// Get last spec file change timestamp
	specTime := lastGitModTime(opts.CodeRoot, opts.SpecRoot)
	if specTime.IsZero() {
		return 0.5
	}

	// Get last code file change timestamp (Go files in internal/)
	codeDir := filepath.Join(opts.CodeRoot, "internal")
	codeTime := lastGitModTime(opts.CodeRoot, codeDir)
	if codeTime.IsZero() {
		return 0.5
	}

	// If spec was modified more recently than code, that's correct ordering
	// (spec changes come first, so spec's last change should be >= code's)
	if specTime.After(codeTime) || specTime.Equal(codeTime) {
		return 1.0
	}

	// Code was modified after spec - check how much
	// If code was modified within 1 hour of spec, consider it concurrent (0.5)
	delta := codeTime.Sub(specTime)
	if delta < time.Hour {
		return 0.5
	}

	// Code modified significantly after spec with no spec update
	return 0.0
}

// lastGitModTime returns the timestamp of the most recent git commit
// touching files under the given path.
func lastGitModTime(repoRoot, targetPath string) time.Time {
	relPath, err := filepath.Rel(repoRoot, targetPath)
	if err != nil {
		relPath = targetPath
	}

	cmd := exec.Command("git", "-C", repoRoot, "log", "-1",
		"--format=%aI", "--", relPath)
	out, err := cmd.Output()
	if err != nil || len(out) == 0 {
		return time.Time{}
	}

	t, err := time.Parse(time.RFC3339, strings.TrimSpace(string(out)))
	if err != nil {
		return time.Time{}
	}
	return t
}

// computeToolUsage measures auto-prompting tool intermediation.
// Checks oplog for validate/diff records and event streams for discovery events.
// Degrades to 0.5 when no oplog available.
func computeToolUsage(opts Options, specID int64) float64 {
	if opts.OplogPath == "" {
		return 0.5 // neutral degradation
	}

	records, err := oplog.ReadAll(opts.OplogPath)
	if err != nil || len(records) == 0 {
		// Check for event streams as fallback
		return computeEventStreamUsage(opts)
	}

	// Count auto-prompting-relevant records
	validateCount := 0
	diffCount := 0
	for _, rec := range records {
		switch rec.Type {
		case oplog.RecordTypeValidate:
			validateCount++
		case oplog.RecordTypeDiff:
			diffCount++
		}
	}

	// Also count event stream activity
	eventScore := computeEventStreamUsage(opts)

	// Expected: at least 1 validate + 1 diff per feature scope
	// Score: actual commands / expected baseline
	oplogScore := 0.0
	if validateCount > 0 {
		oplogScore += 0.4
	}
	if diffCount > 0 {
		oplogScore += 0.2
	}
	if validateCount >= 2 {
		oplogScore += 0.2 // extra credit for multiple validate runs
	}

	// Combine oplog evidence with event stream evidence
	combined := oplogScore + 0.2*eventScore
	if combined > 1.0 {
		combined = 1.0
	}
	return combined
}

// computeEventStreamUsage checks .ddis/events/ for discovery thread activity.
func computeEventStreamUsage(opts Options) float64 {
	if opts.CodeRoot == "" {
		return 0.0
	}

	eventsDir := filepath.Join(opts.CodeRoot, ".ddis", "events")
	info, err := os.Stat(eventsDir)
	if err != nil || !info.IsDir() {
		return 0.0
	}

	// Check threads.jsonl for active discovery threads
	threadsPath := filepath.Join(eventsDir, "threads.jsonl")
	data, err := os.ReadFile(threadsPath)
	if err != nil || len(data) == 0 {
		return 0.0
	}

	// Count threads with events
	lines := strings.Split(strings.TrimSpace(string(data)), "\n")
	activeThreads := 0
	for _, line := range lines {
		if strings.TrimSpace(line) == "" {
			continue
		}
		var thread struct {
			EventCount int `json:"event_count"`
		}
		if json.Unmarshal([]byte(line), &thread) == nil {
			if thread.EventCount > 0 {
				activeThreads++
			}
		}
		// Even a thread with 0 events counts as discovery usage
		activeThreads++
	}

	if activeThreads == 0 {
		return 0.0
	}
	if activeThreads >= 3 {
		return 1.0
	}
	return float64(activeThreads) / 3.0
}

// computeWitnessCoverage calculates the fraction of invariants with valid witnesses.
// Always computable — only requires the SQLite database.
func computeWitnessCoverage(db *sql.DB, specID int64) float64 {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil || len(invs) == 0 {
		return 0.0 // honest: nothing to witness
	}

	validIDs, err := storage.ListValidWitnessIDs(db, specID)
	if err != nil {
		return 0.0
	}

	validSet := make(map[string]bool)
	for _, id := range validIDs {
		validSet[id] = true
	}

	// Count: valid = 1.0, stale = 0.25, missing = 0.0
	total := 0.0
	for _, inv := range invs {
		if validSet[inv.InvariantID] {
			total += 1.0
		} else {
			// Check for stale witness
			w, err := storage.GetWitness(db, specID, inv.InvariantID)
			if err == nil && (w.Status == "stale_spec" || w.Status == "stale_code") {
				total += 0.25
			}
			// missing = 0.0, no addition
		}
	}

	return total / float64(len(invs))
}

// computeValidationGate checks if validation was run during the current workflow.
// Degrades to 0.5 when no oplog available.
func computeValidationGate(opts Options) float64 {
	if opts.OplogPath == "" {
		return 0.5 // neutral degradation
	}

	records, err := oplog.ReadFiltered(opts.OplogPath, oplog.FilterOpts{
		Types: []oplog.RecordType{oplog.RecordTypeValidate},
	})
	if err != nil {
		return 0.5
	}

	if len(records) == 0 {
		return 0.0 // no validation runs
	}

	// Check the most recent validate record for pass status
	latest := records[len(records)-1]
	vData, err := latest.DecodeValidate()
	if err != nil {
		return 0.5
	}

	if vData.Failed == 0 {
		return 1.0 // all checks passed
	}

	// Partial credit: validation was run but had failures
	return 0.5
}

// generateRecommendation produces a workflow guidance string based on weakest sub-score.
func generateRecommendation(info *Info) string {
	type subScore struct {
		name  string
		value float64
		hint  string
	}

	scores := []subScore{
		{"spec_first", info.SpecFirstRatio, "Run `ddis refine` on spec before implementing code changes."},
		{"tool_usage", info.ToolUsage, "Run `ddis discover` to establish context before editing."},
		{"witness_coverage", info.WitnessCoverage, "Run `ddis witness <INV-ID> --verify --code-root .` on modified invariants."},
		{"validation_gate", info.ValidationGate, "Run `ddis validate` between spec and code changes."},
	}

	// Find weakest non-degraded sub-score
	weakest := subScore{value: 2.0}
	for _, s := range scores {
		isDegraded := false
		for _, d := range info.Degraded {
			if strings.Contains(d, s.name) {
				isDegraded = true
				break
			}
		}
		if !isDegraded && s.value < weakest.value {
			weakest = s
		}
	}

	if weakest.value >= 0.8 {
		return "Process compliance is healthy."
	}

	return weakest.hint
}
