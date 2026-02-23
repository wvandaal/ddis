package validator

import (
	"bufio"
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// checkImplementationTraceability verifies spec-to-code traceability annotations.
// Only runs when CodeRoot is set (--code-root flag provided).
type checkImplementationTraceability struct {
	CodeRoot string
}

func (c *checkImplementationTraceability) ID() int      { return 13 }
func (c *checkImplementationTraceability) Name() string { return "Implementation traceability" }
func (c *checkImplementationTraceability) Applicable(_ string) bool {
	return c.CodeRoot != ""
}

// traceAnnotation represents a parsed Implementation Trace annotation.
type traceAnnotation struct {
	Kind     string // "Source", "Tests", "Validates-via"
	FilePath string
	FuncName string
}

// traceLineRe matches an Implementation Trace annotation line:
//
//	- Source: `path/to/file.go::FunctionName`
//	- Tests: `path/to/file_test.go::TestFunctionName`
//	- Validates-via: `path/to/file.go::ValidatorFunc`
var traceLineRe = regexp.MustCompile(`^\s*-\s*(Source|Tests|Validates-via):\s*` + "`" + `([^` + "`" + `]+)::(\w+)` + "`")

// parseTraceAnnotations extracts Implementation Trace annotations from invariant raw text.
func parseTraceAnnotations(rawText string) []traceAnnotation {
	var annotations []traceAnnotation
	scanner := bufio.NewScanner(strings.NewReader(rawText))
	for scanner.Scan() {
		line := scanner.Text()
		m := traceLineRe.FindStringSubmatch(line)
		if m == nil {
			continue
		}
		annotations = append(annotations, traceAnnotation{
			Kind:     m[1],
			FilePath: m[2],
			FuncName: m[3],
		})
	}
	return annotations
}

// funcExistsInFile checks whether a Go function, method, type, const, or var
// with the given name exists in the specified file. It handles:
//
//	func FunctionName(
//	func (receiver) FunctionName(
//	type TypeName struct
//	const ConstName =
//	const ConstName string =
//	var VarName =
//	var VarName Type =
func funcExistsInFile(filePath, funcName string) (bool, error) {
	f, err := os.Open(filePath)
	if err != nil {
		return false, err
	}
	defer f.Close()

	funcPattern := regexp.MustCompile(`func\s+(\([^)]+\)\s+)?` + regexp.QuoteMeta(funcName) + `\(`)
	typePattern := regexp.MustCompile(`type\s+` + regexp.QuoteMeta(funcName) + `\s+`)
	constVarPattern := regexp.MustCompile(`(const|var)\s+` + regexp.QuoteMeta(funcName) + `(\s+\S+)?\s*=`)

	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := scanner.Text()
		if funcPattern.MatchString(line) || typePattern.MatchString(line) || constVarPattern.MatchString(line) {
			return true, nil
		}
	}
	return false, scanner.Err()
}

func (c *checkImplementationTraceability) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	totalAnnotations := 0
	validAnnotations := 0
	brokenAnnotations := 0
	orphanedInvariants := 0

	for _, inv := range invs {
		annotations := parseTraceAnnotations(inv.RawText)

		if len(annotations) == 0 {
			// No Implementation Trace block — this is informational, not an error
			continue
		}

		hasAnyBroken := false

		for _, ann := range annotations {
			totalAnnotations++

			absPath := ann.FilePath
			if !filepath.IsAbs(absPath) {
				absPath = filepath.Join(c.CodeRoot, ann.FilePath)
			}

			// Check file existence
			if _, statErr := os.Stat(absPath); os.IsNotExist(statErr) {
				brokenAnnotations++
				hasAnyBroken = true
				result.Passed = false
				result.Findings = append(result.Findings, Finding{
					CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
					Message:     fmt.Sprintf("%s %s annotation: file not found: %s", inv.InvariantID, ann.Kind, ann.FilePath),
					Location:    inv.InvariantID,
					InvariantID: inv.InvariantID,
				})
				continue
			}

			// Determine expected function prefix
			expectedFunc := ann.FuncName
			if ann.Kind == "Tests" && !strings.HasPrefix(expectedFunc, "Test") {
				expectedFunc = "Test" + expectedFunc
			}

			found, scanErr := funcExistsInFile(absPath, expectedFunc)
			if scanErr != nil {
				brokenAnnotations++
				hasAnyBroken = true
				result.Passed = false
				result.Findings = append(result.Findings, Finding{
					CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
					Message:     fmt.Sprintf("%s %s annotation: error reading %s: %v", inv.InvariantID, ann.Kind, ann.FilePath, scanErr),
					Location:    inv.InvariantID,
					InvariantID: inv.InvariantID,
				})
				continue
			}

			if !found {
				brokenAnnotations++
				hasAnyBroken = true
				result.Passed = false
				result.Findings = append(result.Findings, Finding{
					CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
					Message:     fmt.Sprintf("%s %s annotation: function %s not found in %s", inv.InvariantID, ann.Kind, expectedFunc, ann.FilePath),
					Location:    inv.InvariantID,
					InvariantID: inv.InvariantID,
				})
				continue
			}

			validAnnotations++
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityInfo,
				Message:     fmt.Sprintf("%s %s annotation OK: %s::%s", inv.InvariantID, ann.Kind, ann.FilePath, expectedFunc),
				InvariantID: inv.InvariantID,
			})
		}

		if hasAnyBroken {
			orphanedInvariants++
		}
	}

	if totalAnnotations == 0 {
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityInfo,
			Message: "no Implementation Trace annotations found in any invariant",
		})
		result.Summary = "no annotations to verify"
		return result
	}

	result.Summary = fmt.Sprintf("%d annotations: %d valid, %d broken (%d invariants with broken refs)",
		totalAnnotations, validAnnotations, brokenAnnotations, orphanedInvariants)
	return result
}
