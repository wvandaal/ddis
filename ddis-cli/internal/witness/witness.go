package witness

// ddis:maintains APP-INV-041 (witness auto-invalidation)
// ddis:implements APP-ADR-030 (persistent witnesses over ephemeral done flags)

import (
	"bufio"
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/wvandaal/ddis/internal/annotate"
	"github.com/wvandaal/ddis/internal/storage"
)

// Options controls witness recording behavior.
type Options struct {
	InvariantID   string
	EvidenceType  string // "test" | "annotation" | "scan" | "review" | "attestation"
	Evidence      string
	ProvenBy      string // session/agent ID
	Model         string // model type (e.g., "claude-opus-4-6")
	CodeHash      string
	CodeRoot      string // for --verify/--review-context
	Verify        bool   // --verify flag: require mechanical annotation proof (Level 3)
	ReviewContext bool   // --review-context flag: output review bundle (Level 4 phase A)
	Notes         string
	AsJSON        bool
}

// VerifyResult holds the result of a mechanical annotation check.
type VerifyResult struct {
	InvariantID      string            `json:"invariant_id"`
	AnnotationCount  int               `json:"annotation_count"`
	Annotations      []AnnotationMatch `json:"annotations"`
	ValidationMethod string            `json:"validation_method,omitempty"`
	Verified         bool              `json:"verified"`
}

// AnnotationMatch is one code annotation targeting the invariant.
type AnnotationMatch struct {
	FilePath string `json:"file_path"`
	Line     int    `json:"line"`
	Verb     string `json:"verb"`
	Raw      string `json:"raw_comment"`
}

// ReviewBundle is the Gestalt-Theory-framed review context for Level 4.
type ReviewBundle struct {
	InvariantID       string         `json:"invariant_id"`
	Title             string         `json:"title"`
	Statement         string         `json:"statement"`
	SemiFormal        string         `json:"semi_formal,omitempty"`
	ViolationScenario string         `json:"violation_scenario,omitempty"`
	ValidationMethod  string         `json:"validation_method,omitempty"`
	WhyThisMatters    string         `json:"why_this_matters,omitempty"`
	SpecHash          string         `json:"spec_hash"`
	CodeLocations     []CodeLocation `json:"code_locations"`
	ReviewCriteria    []string       `json:"review_criteria"`
}

// CodeLocation is a code file with annotation context.
type CodeLocation struct {
	FilePath    string `json:"file_path"`
	LineStart   int    `json:"line_start"`
	LineEnd     int    `json:"line_end"`
	Verb        string `json:"verb"`
	CodeSnippet string `json:"code_snippet"`
}

// CheckOptions controls witness check behavior.
type CheckOptions struct {
	AsJSON bool
}

// WitnessSummary is the full witness status report.
type WitnessSummary struct {
	Total    int           `json:"total_invariants"`
	Valid    int           `json:"valid_witnesses"`
	Stale    int           `json:"stale_witnesses"`
	Missing  int           `json:"missing_witnesses"`
	Coverage string        `json:"coverage"`
	Items    []WitnessItem `json:"items"`
}

// WitnessItem is one invariant's witness status.
type WitnessItem struct {
	InvariantID  string `json:"invariant_id"`
	Title        string `json:"title"`
	Status       string `json:"status"`
	ProvenBy     string `json:"proven_by,omitempty"`
	Model        string `json:"model,omitempty"`
	ProvenAt     string `json:"proven_at,omitempty"`
	EvidenceType string `json:"evidence_type,omitempty"`
	StaleReason  string `json:"stale_reason,omitempty"`
}

// Record creates or replaces a witness for an invariant.
func Record(db *sql.DB, specID int64, opts Options) (*storage.InvariantWitness, error) {
	// Validate invariant exists
	inv, err := storage.GetInvariant(db, specID, opts.InvariantID)
	if err != nil {
		return nil, fmt.Errorf("invariant %s not found: %w", opts.InvariantID, err)
	}

	// If --verify, run mechanical checks first
	if opts.Verify {
		vr, err := Verify(db, specID, opts.InvariantID, opts.CodeRoot)
		if err != nil {
			return nil, err
		}
		if !vr.Verified {
			return nil, fmt.Errorf("verification failed: no code annotations found for %s", opts.InvariantID)
		}
		// Override evidence with scan result
		opts.EvidenceType = "scan"
		evidence, err := json.Marshal(vr)
		if err != nil {
			return nil, fmt.Errorf("marshal verify result: %w", err)
		}
		opts.Evidence = string(evidence)
	}

	if opts.EvidenceType == "" {
		opts.EvidenceType = "attestation"
	}
	if opts.Evidence == "" {
		opts.Evidence = "agent attestation"
	}
	if opts.ProvenBy == "" {
		opts.ProvenBy = "unknown"
	}

	w := &storage.InvariantWitness{
		SpecID:       specID,
		InvariantID:  opts.InvariantID,
		SpecHash:     inv.ContentHash,
		CodeHash:     opts.CodeHash,
		EvidenceType: opts.EvidenceType,
		Evidence:     opts.Evidence,
		ProvenBy:     opts.ProvenBy,
		Model:        opts.Model,
		Status:       "valid",
		Notes:        opts.Notes,
	}

	_, err = storage.InsertWitness(db, w)
	if err != nil {
		return nil, err
	}

	// Re-read to get the proven_at timestamp
	return storage.GetWitness(db, specID, opts.InvariantID)
}

// Verify runs mechanical annotation checks for an invariant.
func Verify(db *sql.DB, specID int64, invariantID string, codeRoot string) (*VerifyResult, error) {
	if codeRoot == "" {
		codeRoot = "."
	}

	inv, err := storage.GetInvariant(db, specID, invariantID)
	if err != nil {
		return nil, fmt.Errorf("invariant %s not found: %w", invariantID, err)
	}

	scanResult, err := annotate.Scan(annotate.ScanOptions{Root: codeRoot})
	if err != nil {
		return nil, fmt.Errorf("scan code root: %w", err)
	}

	vr := &VerifyResult{
		InvariantID:      invariantID,
		ValidationMethod: inv.ValidationMethod,
	}

	for _, a := range scanResult.Annotations {
		if a.Target == invariantID {
			vr.Annotations = append(vr.Annotations, AnnotationMatch{
				FilePath: a.FilePath,
				Line:     a.Line,
				Verb:     a.Verb,
				Raw:      a.RawComment,
			})
		}
	}

	vr.AnnotationCount = len(vr.Annotations)
	vr.Verified = vr.AnnotationCount > 0
	return vr, nil
}

// BuildReviewContext creates a Gestalt-Theory-framed review bundle.
func BuildReviewContext(db *sql.DB, specID int64, invariantID string, codeRoot string) (*ReviewBundle, error) {
	if codeRoot == "" {
		codeRoot = "."
	}

	inv, err := storage.GetInvariant(db, specID, invariantID)
	if err != nil {
		return nil, fmt.Errorf("invariant %s not found: %w", invariantID, err)
	}

	// Find code annotations
	scanResult, err := annotate.Scan(annotate.ScanOptions{Root: codeRoot})
	if err != nil {
		return nil, fmt.Errorf("scan code root: %w", err)
	}

	bundle := &ReviewBundle{
		InvariantID:       invariantID,
		Title:             inv.Title,
		Statement:         inv.Statement,
		SemiFormal:        inv.SemiFormal,
		ViolationScenario: inv.ViolationScenario,
		ValidationMethod:  inv.ValidationMethod,
		WhyThisMatters:    inv.WhyThisMatters,
		SpecHash:          inv.ContentHash,
	}

	// Extract code locations with surrounding context
	for _, a := range scanResult.Annotations {
		if a.Target != invariantID {
			continue
		}
		snippet := readCodeSnippet(a.FilePath, codeRoot, a.Line, 25)
		lineStart := a.Line - 25
		if lineStart < 1 {
			lineStart = 1
		}
		bundle.CodeLocations = append(bundle.CodeLocations, CodeLocation{
			FilePath:    a.FilePath,
			LineStart:   lineStart,
			LineEnd:     a.Line + 25,
			Verb:        a.Verb,
			CodeSnippet: snippet,
		})
	}

	// Generate review criteria from invariant definition
	bundle.ReviewCriteria = generateCriteria(inv)

	return bundle, nil
}

// Check validates freshness of all witnesses and returns a summary.
func Check(db *sql.DB, specID int64, opts CheckOptions) (*WitnessSummary, error) {
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list invariants: %w", err)
	}

	witnesses, err := storage.ListWitnesses(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list witnesses: %w", err)
	}

	witnessMap := make(map[string]storage.InvariantWitness)
	for _, w := range witnesses {
		witnessMap[w.InvariantID] = w
	}

	summary := &WitnessSummary{
		Total: len(invs),
	}

	for _, inv := range invs {
		item := WitnessItem{
			InvariantID: inv.InvariantID,
			Title:       inv.Title,
		}

		w, hasWitness := witnessMap[inv.InvariantID]
		if !hasWitness {
			item.Status = "missing"
			summary.Missing++
		} else if w.Status == "valid" {
			item.Status = "valid"
			item.ProvenBy = w.ProvenBy
			item.Model = w.Model
			item.ProvenAt = w.ProvenAt
			item.EvidenceType = w.EvidenceType
			summary.Valid++
		} else {
			item.Status = w.Status
			item.ProvenBy = w.ProvenBy
			item.Model = w.Model
			item.ProvenAt = w.ProvenAt
			item.EvidenceType = w.EvidenceType
			item.StaleReason = w.Status
			summary.Stale++
		}

		summary.Items = append(summary.Items, item)
	}

	if summary.Total > 0 {
		summary.Coverage = fmt.Sprintf("%.0f%%", float64(summary.Valid)/float64(summary.Total)*100)
	} else {
		summary.Coverage = "0%"
	}

	return summary, nil
}

// Refresh re-validates witnesses by comparing spec hashes. Returns count of invalidated.
func Refresh(db *sql.DB, specID int64) (int, error) {
	return storage.InvalidateWitnesses(db, specID, specID)
}

// ValidDoneSet returns a set of invariant IDs with valid witnesses.
func ValidDoneSet(db *sql.DB, specID int64) (map[string]bool, error) {
	ids, err := storage.ListValidWitnessIDs(db, specID)
	if err != nil {
		return nil, err
	}
	result := make(map[string]bool, len(ids))
	for _, id := range ids {
		result[id] = true
	}
	return result, nil
}

// readCodeSnippet reads lines around a target line from a file.
func readCodeSnippet(filePath string, codeRoot string, targetLine int, radius int) string {
	fullPath := filePath
	if codeRoot != "" && codeRoot != "." {
		fullPath = codeRoot + "/" + filePath
	}

	f, err := os.Open(fullPath)
	if err != nil {
		return fmt.Sprintf("(could not read %s: %v)", filePath, err)
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	var lines []string
	lineNum := 0
	start := targetLine - radius
	if start < 1 {
		start = 1
	}
	end := targetLine + radius

	for scanner.Scan() {
		lineNum++
		if lineNum >= start && lineNum <= end {
			lines = append(lines, fmt.Sprintf("%4d: %s", lineNum, scanner.Text()))
		}
		if lineNum > end {
			break
		}
	}
	if err := scanner.Err(); err != nil {
		return fmt.Sprintf("(read error in %s: %v)\n%s", filePath, err, strings.Join(lines, "\n"))
	}

	return strings.Join(lines, "\n")
}

// generateCriteria produces review criteria from an invariant definition.
func generateCriteria(inv *storage.Invariant) []string {
	var criteria []string
	if inv.Statement != "" {
		criteria = append(criteria, fmt.Sprintf("Does the implementation faithfully satisfy: %q?", truncate(inv.Statement, 200)))
	}
	if inv.SemiFormal != "" {
		criteria = append(criteria, fmt.Sprintf("Does the code enforce the semi-formal predicate: %q?", truncate(inv.SemiFormal, 200)))
	}
	if inv.ViolationScenario != "" {
		criteria = append(criteria, fmt.Sprintf("Is the violation scenario prevented: %q?", truncate(inv.ViolationScenario, 200)))
	}
	if inv.ValidationMethod != "" {
		criteria = append(criteria, fmt.Sprintf("Is the validation method achievable: %q?", truncate(inv.ValidationMethod, 200)))
	}
	criteria = append(criteria, "Are there any gaps between spec intent and code behavior?")
	return criteria
}

func truncate(s string, maxLen int) string {
	s = strings.ReplaceAll(s, "\n", " ")
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen-3] + "..."
}
