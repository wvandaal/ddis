package challenge

// ddis:implements APP-INV-050 (challenge-witness adjunction fidelity)
// ddis:implements APP-ADR-037 (challenge as right adjoint of witness)

import (
	"bufio"
	"bytes"
	"context"
	"database/sql"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"time"
	"unicode"

	"github.com/wvandaal/ddis/internal/annotate"
	"github.com/wvandaal/ddis/internal/consistency"
	"github.com/wvandaal/ddis/internal/storage"
)

// Verdict represents the outcome of a challenge.
type Verdict string

const (
	Confirmed    Verdict = "confirmed"
	Provisional  Verdict = "provisional"
	Refuted      Verdict = "refuted"
	Inconclusive Verdict = "inconclusive"
)

// Options controls challenge behavior.
type Options struct {
	CodeRoot     string // root directory to scan for annotations/tests
	ChallengedBy string // agent/session ID
	Model        string // model used (e.g., "claude-opus-4-6")
	AsJSON       bool
	MaxLevel     int // max verification level (1-5, default 5)
}

// Result holds the outcome of challenging a single invariant's witness.
type Result struct {
	InvariantID        string             `json:"invariant_id"`
	Verdict            Verdict            `json:"verdict"`
	LevelFormal        *FormalResult      `json:"level_formal"`
	LevelUncertainty   *UncertaintyResult `json:"level_uncertainty"`
	LevelCausal        *CausalResult      `json:"level_causal,omitempty"`
	LevelPractical     *PracticalResult   `json:"level_practical,omitempty"`
	LevelMeta          *MetaResult        `json:"level_meta,omitempty"`
	WitnessInvalidated bool               `json:"witness_invalidated"`
}

// FormalResult is Level 1: SAT-based formal consistency check.
type FormalResult struct {
	Parsed         bool   `json:"parsed"`
	SelfConsistent bool   `json:"self_consistent"`
	Detail         string `json:"detail,omitempty"`
}

// UncertaintyResult is Level 2: evidence type confidence scoring.
type UncertaintyResult struct {
	EvidenceType string  `json:"evidence_type"`
	Confidence   float64 `json:"confidence"`
}

// CausalResult is Level 3: annotation-based causal link.
// Tracks both ddis:tests annotations (strongest) and ddis:implements/maintains
// annotations (weaker but still causal evidence).
type CausalResult struct {
	TestFound        bool     `json:"test_found"`
	Annotations      []string `json:"annotations,omitempty"`
	TestName         string   `json:"test_name,omitempty"`
	TestFile         string   `json:"test_file,omitempty"`
	CodeAnnotations  int      `json:"code_annotations"`  // count of implements/maintains/interfaces
	AnnotationVerbs  []string `json:"annotation_verbs,omitempty"`
}

// PracticalResult is Level 4: test execution.
type PracticalResult struct {
	Ran    bool   `json:"ran"`
	Passed bool   `json:"passed"`
	Output string `json:"output,omitempty"`
}

// MetaResult is Level 5: keyword overlap between invariant and evidence.
type MetaResult struct {
	Overlap   float64 `json:"overlap"`
	InvTerms  int     `json:"inv_terms"`
	EvidTerms int     `json:"evid_terms"`
	Shared    int     `json:"shared"`
}

// confidenceMap maps evidence types to confidence scores.
var confidenceMap = map[string]float64{
	"test":        0.9,
	"scan":        0.7,
	"review":      0.8,
	"annotation":  0.6,
	"attestation": 0.3,
}

// testFuncRe matches Go test function declarations.
var testFuncRe = regexp.MustCompile(`func\s+(Test\w+)\s*\(`)

// Challenge mechanically verifies a witness claim for a single invariant.
func Challenge(db *sql.DB, specID int64, invariantID string, opts Options) (*Result, error) {
	if opts.MaxLevel <= 0 {
		opts.MaxLevel = 5
	}

	// Look up the invariant.
	inv, err := storage.GetInvariant(db, specID, invariantID)
	if err != nil {
		return nil, fmt.Errorf("get invariant %s: %w", invariantID, err)
	}

	// Look up the witness (may not exist — that's inconclusive).
	w, err := storage.GetWitness(db, specID, invariantID)
	if err != nil {
		// No witness → inconclusive
		result := &Result{
			InvariantID:      invariantID,
			Verdict:          Inconclusive,
			LevelFormal:      &FormalResult{Detail: "no witness recorded"},
			LevelUncertainty: &UncertaintyResult{EvidenceType: "none", Confidence: 0},
		}
		return result, nil
	}

	// Skip non-valid witnesses (stale, invalidated, etc.).
	if w.Status != "valid" {
		result := &Result{
			InvariantID:      invariantID,
			Verdict:          Inconclusive,
			LevelFormal:      &FormalResult{Detail: fmt.Sprintf("witness status=%s (not valid)", w.Status)},
			LevelUncertainty: &UncertaintyResult{EvidenceType: w.EvidenceType, Confidence: 0},
		}
		return result, nil
	}

	result := &Result{InvariantID: invariantID}

	// Level 1: Formal — SAT consistency of semi-formal expression.
	result.LevelFormal = levelFormal(inv)

	// Level 2: Uncertainty — evidence type confidence.
	result.LevelUncertainty = levelUncertainty(w)

	// Level 3: Causal — annotation lookup.
	if opts.MaxLevel >= 3 && opts.CodeRoot != "" {
		result.LevelCausal = levelCausal(inv, opts.CodeRoot)
	}

	// Level 4: Practical — test execution.
	if opts.MaxLevel >= 4 && opts.CodeRoot != "" && result.LevelCausal != nil && result.LevelCausal.TestFound {
		result.LevelPractical = levelPractical(result.LevelCausal, opts.CodeRoot)
	}

	// Level 5: Meta — keyword overlap.
	if opts.MaxLevel >= 5 {
		result.LevelMeta = levelMeta(inv, w)
	}

	// Compute verdict.
	result.Verdict = computeVerdict(result)

	// On refutation, auto-invalidate the witness.
	if result.Verdict == Refuted && w.ID > 0 {
		if err := storage.InvalidateWitnessByID(db, w.ID); err != nil {
			return nil, fmt.Errorf("invalidate witness %s (id=%d): %w", invariantID, w.ID, err)
		}
		result.WitnessInvalidated = true
	}

	// Store the challenge result.
	cr := &storage.ChallengeResult{
		SpecID:           specID,
		InvariantID:      invariantID,
		Verdict:          string(result.Verdict),
		LevelFormal:      fmtFormal(result.LevelFormal),
		LevelUncertainty: fmtUncertainty(result.LevelUncertainty),
		LevelCausal:      fmtCausal(result.LevelCausal),
		LevelPractical:   fmtPractical(result.LevelPractical),
		LevelMeta:        fmtMeta(result.LevelMeta),
		ChallengedBy:     opts.ChallengedBy,
		Model:            opts.Model,
	}
	if w.ID > 0 {
		cr.WitnessID = &w.ID
	}
	if _, err := storage.InsertChallengeResult(db, cr); err != nil {
		return nil, fmt.Errorf("store challenge result for %s: %w", invariantID, err)
	}

	return result, nil
}

// ChallengeAll runs challenge on all witnessed invariants.
func ChallengeAll(db *sql.DB, specID int64, opts Options) ([]Result, error) {
	witnesses, err := storage.ListWitnesses(db, specID)
	if err != nil {
		return nil, fmt.Errorf("list witnesses: %w", err)
	}

	var results []Result
	for _, w := range witnesses {
		if w.Status != "valid" {
			continue
		}
		r, err := Challenge(db, specID, w.InvariantID, opts)
		if err != nil {
			continue
		}
		results = append(results, *r)
	}
	return results, nil
}

// levelFormal checks if the invariant's semi-formal expression is self-consistent.
func levelFormal(inv *storage.Invariant) *FormalResult {
	if inv.SemiFormal == "" {
		return &FormalResult{Parsed: false, SelfConsistent: true, Detail: "no semi-formal expression"}
	}

	vm := consistency.NewVarMap()
	cnf := consistency.ParseSemiFormal(inv.SemiFormal, vm)
	if len(cnf) == 0 {
		return &FormalResult{Parsed: false, SelfConsistent: true, Detail: "could not parse semi-formal"}
	}

	sat := consistency.Satisfiable(cnf, vm)
	return &FormalResult{
		Parsed:         true,
		SelfConsistent: sat,
		Detail:         fmt.Sprintf("parsed %d clauses, %d variables", len(cnf), vm.Count()),
	}
}

// levelUncertainty scores the witness evidence type.
func levelUncertainty(w *storage.InvariantWitness) *UncertaintyResult {
	conf, ok := confidenceMap[w.EvidenceType]
	if !ok {
		conf = 0.1
	}
	return &UncertaintyResult{
		EvidenceType: w.EvidenceType,
		Confidence:   conf,
	}
}

// levelCausal looks for annotations targeting the invariant.
// Tracks both ddis:tests (strongest causal link) and ddis:implements/maintains/interfaces
// (weaker but still meaningful — code explicitly declares it upholds the invariant).
func levelCausal(inv *storage.Invariant, codeRoot string) *CausalResult {
	result := &CausalResult{}

	scanResult, err := annotate.Scan(annotate.ScanOptions{Root: codeRoot})
	if err != nil {
		return result
	}

	verbsSeen := make(map[string]bool)
	for _, ann := range scanResult.Annotations {
		if ann.Target != inv.InvariantID {
			continue
		}

		if ann.Verb == "tests" {
			result.TestFound = true
			result.Annotations = append(result.Annotations, fmt.Sprintf("%s:%d", ann.FilePath, ann.Line))
			result.TestFile = ann.FilePath
			absPath := ann.FilePath
			if !filepath.IsAbs(absPath) {
				absPath = filepath.Join(codeRoot, absPath)
			}
			if name := extractTestName(absPath, ann.Line); name != "" {
				result.TestName = name
			}
		} else {
			// Track implements/maintains/interfaces annotations.
			result.CodeAnnotations++
			result.Annotations = append(result.Annotations, fmt.Sprintf("%s:%d (%s)", ann.FilePath, ann.Line, ann.Verb))
		}
		verbsSeen[ann.Verb] = true
	}

	for v := range verbsSeen {
		result.AnnotationVerbs = append(result.AnnotationVerbs, v)
	}
	return result
}

// levelPractical runs the test if we found one via causal analysis.
func levelPractical(causal *CausalResult, codeRoot string) *PracticalResult {
	result := &PracticalResult{}
	if causal.TestName == "" {
		result.Output = "no test function name extracted"
		return result
	}

	// Check that `go` is available before attempting test execution.
	if _, err := exec.LookPath("go"); err != nil {
		result.Output = "go binary not found in PATH"
		return result
	}

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	cmd := exec.CommandContext(ctx, "go", "test", "-run", "^"+causal.TestName+"$", "-count=1", "-v", "./...")
	cmd.Dir = codeRoot
	var buf bytes.Buffer
	cmd.Stdout = &buf
	cmd.Stderr = &buf

	err := cmd.Run()
	result.Ran = true
	result.Output = truncate(buf.String(), 2000)
	result.Passed = err == nil

	return result
}

// levelMeta computes keyword overlap between invariant statement and witness evidence.
func levelMeta(inv *storage.Invariant, w *storage.InvariantWitness) *MetaResult {
	invKeywords := extractKeywords(inv.Statement + " " + inv.Title)
	evidKeywords := extractKeywords(w.Evidence)

	shared := 0
	for k := range invKeywords {
		if evidKeywords[k] {
			shared++
		}
	}

	total := len(invKeywords) + len(evidKeywords) - shared
	overlap := 0.0
	if total > 0 {
		overlap = float64(shared) / float64(total)
	}

	return &MetaResult{
		Overlap:   overlap,
		InvTerms:  len(invKeywords),
		EvidTerms: len(evidKeywords),
		Shared:    shared,
	}
}

// computeVerdict synthesizes all level results into a verdict.
//
// The verdict taxonomy (gestalt-informed — evidence synthesis over short-circuit):
//   Refuted:      hard negative evidence (SAT contradiction or test failure)
//   Confirmed:    ddis:tests annotation found, test ran, test passed
//   Provisional:  no ddis:tests, but code annotations exist + semi-formal consistent
//                 + evidence confidence > attestation level
//   Inconclusive: insufficient evidence to make any determination
func computeVerdict(r *Result) Verdict {
	// === Hard refutation signals (categorical — override everything) ===

	// Refuted if: semi-formal is self-contradictory
	if r.LevelFormal != nil && r.LevelFormal.Parsed && !r.LevelFormal.SelfConsistent {
		return Refuted
	}

	// Refuted if: test ran and failed
	if r.LevelPractical != nil && r.LevelPractical.Ran && !r.LevelPractical.Passed {
		return Refuted
	}

	// === Full confirmation: test-backed evidence ===

	// Confirmed if: test ran AND passed AND no refutation signals
	if r.LevelPractical != nil && r.LevelPractical.Ran && r.LevelPractical.Passed {
		return Confirmed
	}

	// === Provisional: code annotations provide causal grounding without test execution ===
	// Requires: (a) at least one code annotation (implements/maintains/interfaces/tests),
	//           (b) semi-formal is consistent (or absent),
	//           (c) evidence confidence above attestation-only threshold
	if r.LevelCausal != nil && hasCodeGrounding(r.LevelCausal) {
		formalOK := r.LevelFormal == nil || !r.LevelFormal.Parsed || r.LevelFormal.SelfConsistent
		confAboveAttestation := r.LevelUncertainty != nil && r.LevelUncertainty.Confidence > 0.3
		if formalOK && confAboveAttestation {
			return Provisional
		}
	}

	// === Inconclusive: everything else ===
	return Inconclusive
}

// hasCodeGrounding returns true if any code annotation exists for this invariant
// (ddis:tests, ddis:implements, ddis:maintains, or ddis:interfaces).
func hasCodeGrounding(c *CausalResult) bool {
	return c.TestFound || c.CodeAnnotations > 0
}

// extractTestName scans a file near the annotation line for a Test function.
// Looks both forward (annotation above func) and backward (annotation inside func)
// within a 10-line window.
func extractTestName(filePath string, annotationLine int) string {
	f, err := os.Open(filePath)
	if err != nil {
		return ""
	}
	defer f.Close()

	scanner := bufio.NewScanner(f)
	lineNum := 0
	windowStart := annotationLine - 5
	if windowStart < 1 {
		windowStart = 1
	}
	windowEnd := annotationLine + 10
	for scanner.Scan() {
		lineNum++
		if lineNum < windowStart || lineNum > windowEnd {
			continue
		}
		if m := testFuncRe.FindStringSubmatch(scanner.Text()); m != nil {
			return m[1]
		}
	}
	return ""
}

// stopWords are common words to exclude from keyword extraction.
var stopWords = map[string]bool{
	"the": true, "a": true, "an": true, "is": true, "are": true, "was": true,
	"were": true, "be": true, "been": true, "being": true, "have": true,
	"has": true, "had": true, "do": true, "does": true, "did": true,
	"will": true, "would": true, "could": true, "should": true, "may": true,
	"might": true, "must": true, "shall": true, "can": true, "need": true,
	"to": true, "of": true, "in": true, "for": true, "on": true, "with": true,
	"at": true, "by": true, "from": true, "as": true, "into": true,
	"through": true, "that": true, "which": true, "this": true, "these": true,
	"those": true, "it": true, "its": true, "or": true, "and": true, "but": true,
	"if": true, "not": true, "no": true, "all": true, "each": true, "every": true,
}

// extractKeywords splits text into lowercase tokens, removes stop words.
func extractKeywords(text string) map[string]bool {
	words := make(map[string]bool)
	for _, word := range strings.FieldsFunc(strings.ToLower(text), func(r rune) bool {
		return !unicode.IsLetter(r) && !unicode.IsDigit(r)
	}) {
		if len(word) > 2 && !stopWords[word] {
			words[word] = true
		}
	}
	return words
}

// truncate limits a string to approximately maxLen runes.
func truncate(s string, maxLen int) string {
	runes := []rune(s)
	if len(runes) <= maxLen {
		return s
	}
	return string(runes[:maxLen]) + "..."
}

// Format helpers for storing level results as strings.
func fmtFormal(f *FormalResult) string {
	if f == nil {
		return ""
	}
	return fmt.Sprintf("parsed=%v consistent=%v %s", f.Parsed, f.SelfConsistent, f.Detail)
}

func fmtUncertainty(u *UncertaintyResult) string {
	if u == nil {
		return ""
	}
	return fmt.Sprintf("type=%s confidence=%.1f", u.EvidenceType, u.Confidence)
}

func fmtCausal(c *CausalResult) string {
	if c == nil {
		return ""
	}
	if c.TestFound {
		return fmt.Sprintf("test_found=%v test=%s file=%s code_annotations=%d", c.TestFound, c.TestName, c.TestFile, c.CodeAnnotations)
	}
	return fmt.Sprintf("test_found=false code_annotations=%d", c.CodeAnnotations)
}

func fmtPractical(p *PracticalResult) string {
	if p == nil {
		return ""
	}
	return fmt.Sprintf("ran=%v passed=%v", p.Ran, p.Passed)
}

func fmtMeta(m *MetaResult) string {
	if m == nil {
		return ""
	}
	return fmt.Sprintf("overlap=%.2f inv=%d evid=%d shared=%d", m.Overlap, m.InvTerms, m.EvidTerms, m.Shared)
}

