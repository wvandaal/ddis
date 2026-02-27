package validator

// ddis:implements APP-ADR-048

import (
	"database/sql"
	"fmt"
	"os"
	"os/exec"

	"github.com/wvandaal/ddis/internal/storage"
)

// Check 19: VCS tracking — every source file in the spec must be tracked by git.
// Governs APP-ADR-048.
type checkVCSTracking struct{}

func (c *checkVCSTracking) ID() int                { return 19 }
func (c *checkVCSTracking) Name() string           { return "VCS tracking" }
func (c *checkVCSTracking) Applicable(string) bool { return true }

func (c *checkVCSTracking) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	// Graceful degradation: skip if no .git directory present.
	if _, err := os.Stat(".git"); os.IsNotExist(err) {
		result.Summary = "no .git directory found — VCS check skipped"
		return result
	}

	// Graceful degradation: skip if git binary is not available.
	gitPath, err := exec.LookPath("git")
	if err != nil {
		result.Summary = "git binary not found — VCS check skipped"
		return result
	}

	// Query source files for the spec.
	rows, err := db.Query("SELECT file_path FROM source_files WHERE spec_id = ?", specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID:   c.ID(),
			CheckName: c.Name(),
			Severity:  SeverityError,
			Message:   fmt.Sprintf("query error: %v", err),
		})
		return result
	}
	defer rows.Close()

	var filePaths []string
	for rows.Next() {
		var fp string
		if scanErr := rows.Scan(&fp); scanErr != nil {
			result.Passed = false
			result.Findings = append(result.Findings, Finding{
				CheckID:   c.ID(),
				CheckName: c.Name(),
				Severity:  SeverityError,
				Message:   fmt.Sprintf("scan error: %v", scanErr),
			})
			return result
		}
		filePaths = append(filePaths, fp)
	}
	if err := rows.Err(); err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID:   c.ID(),
			CheckName: c.Name(),
			Severity:  SeverityError,
			Message:   fmt.Sprintf("rows error: %v", err),
		})
		return result
	}

	if len(filePaths) == 0 {
		result.Summary = "no source files found for spec"
		return result
	}

	// Check each source file against git tracking.
	untracked := 0
	for _, fp := range filePaths {
		cmd := exec.Command(gitPath, "ls-files", "--error-unmatch", fp)
		// Combine stdout and stderr so any git error message is captured but discarded.
		cmd.Stdout = nil
		cmd.Stderr = nil
		if runErr := cmd.Run(); runErr != nil {
			// Non-zero exit means the file is not tracked by git.
			untracked++
			result.Passed = false
			result.Findings = append(result.Findings, Finding{
				CheckID:   c.ID(),
				CheckName: c.Name(),
				Severity:  SeverityWarning,
				Message:   fmt.Sprintf("source file not tracked by git: %s", fp),
				Location:  fp,
			})
		}
	}

	if untracked > 0 {
		result.Summary = fmt.Sprintf("%d/%d source file(s) not tracked by git", untracked, len(filePaths))
	} else {
		result.Summary = fmt.Sprintf("all %d source file(s) are git-tracked", len(filePaths))
	}

	// Synthesize: use storage helper to provide richer path context if available.
	if sourceFiles, sfErr := storage.GetSourceFiles(db, specID); sfErr == nil {
		_ = sourceFiles // available for future enrichment (e.g., file_role context)
	}

	return result
}
