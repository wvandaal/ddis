package annotate

import (
	"bufio"
	"database/sql"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
)

// ddis:maintains APP-INV-017 (annotation portability)
// ddis:maintains APP-INV-018 (scan-spec correspondence)
// ddis:maintains APP-INV-032 (symmetric reconciliation — code-side evidence feeding absorb's bidirectional gap analysis)

// DefaultExcludes are directory patterns excluded from scanning by default.
var DefaultExcludes = []string{".git", "vendor", "node_modules", "bin", "testdata", ".ddis"}

// Scan walks the directory tree and extracts all DDIS annotations.
func Scan(opts ScanOptions) (*ScanResult, error) {
	if opts.Root == "" {
		return nil, fmt.Errorf("scan root directory is required")
	}

	excludes := opts.ExcludeGlobs
	if len(excludes) == 0 {
		excludes = DefaultExcludes
	}

	result := &ScanResult{
		ByVerb:     make(map[string]int),
		ByLanguage: make(map[string]int),
	}

	err := filepath.WalkDir(opts.Root, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return nil // skip errors
		}

		// Skip excluded directories
		if d.IsDir() {
			name := d.Name()
			for _, excl := range excludes {
				if matched, _ := filepath.Match(excl, name); matched {
					return filepath.SkipDir
				}
			}
			return nil
		}

		// Skip symlinks
		if d.Type()&fs.ModeSymlink != 0 {
			return nil
		}

		// Check if file extension is recognized
		prefixes, lang := LookupCommentPrefixes(d.Name())
		if prefixes == nil {
			result.FilesSkipped++
			return nil
		}

		// Scan the file
		annotations, err := scanFile(path, prefixes, lang)
		if err != nil {
			return nil // skip files that can't be read
		}

		result.FilesScanned++

		// Make paths relative to root for cleaner output
		relPath, relErr := filepath.Rel(opts.Root, path)
		if relErr != nil {
			relPath = path
		}

		for _, a := range annotations {
			a.FilePath = relPath
			result.Annotations = append(result.Annotations, a)
			result.ByVerb[a.Verb]++
			result.ByLanguage[a.Language]++
		}

		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("walk directory: %w", err)
	}

	result.TotalFound = len(result.Annotations)
	return result, nil
}

// scanFile reads a file and extracts all DDIS annotations.
func scanFile(path string, prefixes []string, lang string) ([]Annotation, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	var annotations []Annotation
	scanner := bufio.NewScanner(f)
	lineNum := 0

	for scanner.Scan() {
		lineNum++
		line := scanner.Text()

		commentText := ExtractComment(line, prefixes)
		if commentText == "" {
			continue
		}

		a := ParseAnnotation(commentText)
		if a == nil {
			continue
		}

		a.FilePath = path
		a.Line = lineNum
		a.Language = lang
		a.RawComment = strings.TrimSpace(line)
		annotations = append(annotations, *a)
	}

	return annotations, scanner.Err()
}

// Verify checks annotations against a spec database.
func Verify(result *ScanResult, db *sql.DB, specID int64) error {
	report := &VerifyReport{}

	// Check each annotation against the spec
	for _, a := range result.Annotations {
		exists, elemType := resolveTarget(db, specID, a.Target)
		if exists {
			report.Resolved = append(report.Resolved, ResolvedAnnotation{
				Annotation:  a,
				ElementType: elemType,
				ElementID:   a.Target,
			})
		} else {
			report.Orphaned = append(report.Orphaned, a)
		}
	}

	// Find spec elements with no annotations
	annotatedTargets := make(map[string]bool)
	for _, a := range result.Annotations {
		annotatedTargets[a.Target] = true
	}

	// Check invariants
	rows, err := db.Query(
		`SELECT invariant_id FROM invariants WHERE spec_id = ?`, specID)
	if err != nil {
		return fmt.Errorf("query invariants: %w", err)
	}
	defer rows.Close()
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return err
		}
		if !annotatedTargets[id] {
			report.Unimplemented = append(report.Unimplemented, id)
		}
	}

	// Check ADRs
	adrRows, err := db.Query(
		`SELECT adr_id FROM adrs WHERE spec_id = ?`, specID)
	if err != nil {
		return fmt.Errorf("query adrs: %w", err)
	}
	defer adrRows.Close()
	for adrRows.Next() {
		var id string
		if err := adrRows.Scan(&id); err != nil {
			return err
		}
		if !annotatedTargets[id] {
			report.Unimplemented = append(report.Unimplemented, id)
		}
	}

	result.VerifyReport = report
	return nil
}

// resolveTarget checks if a target exists in the spec database.
func resolveTarget(db *sql.DB, specID int64, target string) (bool, string) {
	// Try invariant
	var x int
	err := db.QueryRow(
		`SELECT 1 FROM invariants WHERE spec_id = ? AND invariant_id = ?`,
		specID, target).Scan(&x)
	if err == nil {
		return true, "invariant"
	}

	// Try ADR
	err = db.QueryRow(
		`SELECT 1 FROM adrs WHERE spec_id = ? AND adr_id = ?`,
		specID, target).Scan(&x)
	if err == nil {
		return true, "adr"
	}

	// Try gate
	err = db.QueryRow(
		`SELECT 1 FROM quality_gates WHERE spec_id = ? AND gate_id = ?`,
		specID, target).Scan(&x)
	if err == nil {
		return true, "gate"
	}

	return false, ""
}

// StoreAnnotations writes annotations to the code_annotations table.
func StoreAnnotations(db *sql.DB, specID int64, annotations []Annotation) error {
	// Clear existing annotations for this spec
	if _, err := db.Exec(
		`DELETE FROM code_annotations WHERE spec_id = ?`, specID); err != nil {
		return fmt.Errorf("clear annotations: %w", err)
	}

	stmt, err := db.Prepare(
		`INSERT INTO code_annotations (spec_id, file_path, line_number, verb, target, qualifier, language, raw_comment)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?)`)
	if err != nil {
		return fmt.Errorf("prepare insert: %w", err)
	}
	defer stmt.Close()

	for _, a := range annotations {
		if _, err := stmt.Exec(specID, a.FilePath, a.Line, a.Verb, a.Target, a.Qualifier, a.Language, a.RawComment); err != nil {
			return fmt.Errorf("insert annotation %s:%d: %w", a.FilePath, a.Line, err)
		}
	}

	return nil
}
