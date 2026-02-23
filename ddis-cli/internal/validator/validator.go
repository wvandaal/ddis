package validator

import (
	"database/sql"
	"fmt"
	"strconv"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// Severity indicates the importance of a finding.
type Severity string

const (
	SeverityError   Severity = "error"
	SeverityWarning Severity = "warning"
	SeverityInfo    Severity = "info"
)

// Finding is a single validation issue.
type Finding struct {
	CheckID     int      `json:"check_id"`
	CheckName   string   `json:"check_name"`
	Severity    Severity `json:"severity"`
	Message     string   `json:"message"`
	Location    string   `json:"location,omitempty"`
	InvariantID string   `json:"invariant_id,omitempty"`
}

// CheckResult is the outcome of running one check.
type CheckResult struct {
	CheckID   int       `json:"check_id"`
	CheckName string    `json:"check_name"`
	Passed    bool      `json:"passed"`
	Findings  []Finding `json:"findings"`
	Summary   string    `json:"summary"`
}

// Report is the full validation output.
type Report struct {
	SpecPath    string        `json:"spec_path"`
	SourceType  string        `json:"source_type"`
	TotalChecks int           `json:"total_checks"`
	Passed      int           `json:"passed"`
	Failed      int           `json:"failed"`
	Errors      int           `json:"errors"`
	Warnings    int           `json:"warnings"`
	Results     []CheckResult `json:"results"`
}

// Check is the interface all validation checks implement.
type Check interface {
	ID() int
	Name() string
	Applicable(sourceType string) bool
	Run(db *sql.DB, specID int64) CheckResult
}

// ValidateOptions controls which checks to run.
type ValidateOptions struct {
	CheckIDs []int  // empty = run all applicable
	CodeRoot string // Path to source code root for traceability check (Check 13)
}

// ParseCheckIDs parses a comma-separated list of check IDs (e.g. "1,2,3").
func ParseCheckIDs(s string) ([]int, error) {
	if s == "" {
		return nil, nil
	}
	parts := strings.Split(s, ",")
	ids := make([]int, 0, len(parts))
	for _, p := range parts {
		p = strings.TrimSpace(p)
		if p == "" {
			continue
		}
		id, err := strconv.Atoi(p)
		if err != nil {
			return nil, fmt.Errorf("invalid check ID %q: %w", p, err)
		}
		ids = append(ids, id)
	}
	return ids, nil
}

// AllChecks returns all registered validation checks.
func AllChecks() []Check {
	return []Check{
		&checkXRefIntegrity{},
		&checkINV003Falsifiability{},
		&checkINV006XRefDensity{},
		&checkINV009GlossaryCompleteness{},
		&checkINV013InvariantOwnership{},
		&checkINV014BundleBudget{},
		&checkINV015DeclDef{},
		&checkINV016ManifestSync{},
		&checkINV017NegSpecCoverage{},
		&checkGate1Structural{},
		&checkProportionalWeight{},
		&checkNamespaceConsistency{},
		&checkImplementationTraceability{}, // Check 13
	}
}

// Validate runs all applicable checks and builds a report.
func Validate(db *sql.DB, specID int64, opts ValidateOptions) (*Report, error) {
	spec, err := storage.GetSpecIndex(db, specID)
	if err != nil {
		return nil, fmt.Errorf("get spec: %w", err)
	}

	allChecks := AllChecks()

	// Inject CodeRoot into the traceability check
	for _, check := range allChecks {
		if tc, ok := check.(*checkImplementationTraceability); ok {
			tc.CodeRoot = opts.CodeRoot
		}
	}

	// Filter to requested checks if specified
	wantIDs := make(map[int]bool)
	for _, id := range opts.CheckIDs {
		wantIDs[id] = true
	}

	report := &Report{
		SpecPath:   spec.SpecPath,
		SourceType: spec.SourceType,
	}

	for _, check := range allChecks {
		if len(wantIDs) > 0 && !wantIDs[check.ID()] {
			continue
		}
		if !check.Applicable(spec.SourceType) {
			continue
		}

		result := check.Run(db, specID)
		report.Results = append(report.Results, result)
		report.TotalChecks++

		if result.Passed {
			report.Passed++
		} else {
			report.Failed++
		}

		for _, f := range result.Findings {
			switch f.Severity {
			case SeverityError:
				report.Errors++
			case SeverityWarning:
				report.Warnings++
			}
		}
	}

	return report, nil
}
