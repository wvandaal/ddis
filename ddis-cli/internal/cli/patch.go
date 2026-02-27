package cli

// ddis:maintains APP-INV-028 (spec-as-trunk — CLI mediates all spec modifications)

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/storage"
)

var (
	patchElement string
	patchSection string
	patchFile    string
	patchOld     string
	patchNew     string
	patchDryRun  bool
)

var patchCmd = &cobra.Command{
	Use:   "patch [index.db]",
	Short: "Surgical text replacement in spec source files",
	Long: `Replace exact text within a spec element, section, or file.
The CLI mediates all spec modifications — never edit spec files directly.

Three targeting modes (exactly one required):
  --element APP-INV-NNN  Scope to invariant/ADR/gate line range
  --section §N.M         Scope to section line range
  --file path            Scope to entire file (relative to spec root)

The old text must appear exactly once within the scope for safety.

Examples:
  ddis patch --element APP-ADR-005 --old "30-table" --new "39-table"
  ddis patch --section §0.3 --old "23 Commands" --new "30 Commands"
  ddis patch --file constitution/system.md --old "46 APP-INVs" --new "47 APP-INVs"
  ddis patch --file manifest.yaml --old "old text" --new "new text"
  ddis patch --element APP-INV-042 --old "text" --new "text" --dry-run`,
	RunE:          runPatch,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	patchCmd.Flags().StringVar(&patchElement, "element", "", "Target element (APP-INV-NNN, APP-ADR-NNN, Gate-N)")
	patchCmd.Flags().StringVar(&patchSection, "section", "", "Target section (§N.M)")
	patchCmd.Flags().StringVar(&patchFile, "file", "", "Target file path (relative to spec root)")
	patchCmd.Flags().StringVar(&patchOld, "old", "", "Text to find and replace (required)")
	patchCmd.Flags().StringVar(&patchNew, "new", "", "Replacement text (required)")
	patchCmd.Flags().BoolVar(&patchDryRun, "dry-run", false, "Preview changes without writing")
}

func runPatch(cmd *cobra.Command, args []string) error {
	if patchOld == "" {
		return fmt.Errorf("--old is required")
	}
	if patchNew == "" {
		return fmt.Errorf("--new is required")
	}
	if patchOld == patchNew {
		return fmt.Errorf("--old and --new are identical; nothing to change")
	}

	// Count target modes
	modes := 0
	if patchElement != "" {
		modes++
	}
	if patchSection != "" {
		modes++
	}
	if patchFile != "" {
		modes++
	}
	if modes == 0 {
		return fmt.Errorf("one of --element, --section, or --file is required")
	}
	if modes > 1 {
		return fmt.Errorf("specify exactly one of --element, --section, or --file")
	}

	// Resolve DB path for element/section modes
	dbPath := ""
	if len(args) >= 1 {
		dbPath = args[0]
	}

	if patchFile != "" {
		// File mode: use DB to resolve base directory, or resolve relative to cwd
		return runPatchFile(dbPath, patchFile, patchOld, patchNew, patchDryRun)
	}

	// Element/section mode: need DB
	if dbPath == "" {
		var err error
		dbPath, err = FindDB()
		if err != nil {
			return err
		}
	}

	db, err := storage.OpenExisting(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	target := patchElement
	if patchSection != "" {
		target = patchSection
	}

	return runPatchElement(db, specID, target, patchOld, patchNew, patchDryRun)
}

// runPatchFile replaces text in a file (no element scoping).
func runPatchFile(dbPath, relPath, oldText, newText string, dryRun bool) error {
	// Try to resolve against spec root from DB
	fullPath := relPath
	if dbPath != "" || fileExists(relPath) {
		// If file exists at given path, use it directly
		if !fileExists(relPath) {
			// Try to find DB for base dir resolution
			if dbPath == "" {
				var err error
				dbPath, err = FindDB()
				if err != nil {
					return fmt.Errorf("file %q not found and no DB to resolve against: %w", relPath, err)
				}
			}
			db, err := storage.OpenExisting(dbPath)
			if err != nil {
				return fmt.Errorf("open database: %w", err)
			}
			defer db.Close()
			specID, err := storage.GetFirstSpecID(db)
			if err != nil {
				return fmt.Errorf("no spec found: %w", err)
			}
			baseDir, err := specBaseDir(db, specID)
			if err != nil {
				return err
			}
			fullPath = filepath.Join(baseDir, relPath)
			if !fileExists(fullPath) {
				return fmt.Errorf("file not found: tried %q and %q", relPath, fullPath)
			}
		}
	}

	content, err := os.ReadFile(fullPath)
	if err != nil {
		return fmt.Errorf("read %s: %w", fullPath, err)
	}
	text := string(content)

	count := strings.Count(text, oldText)
	if count == 0 {
		return fmt.Errorf("text not found in %s:\n  %q", fullPath, oldText)
	}
	if count > 1 {
		return fmt.Errorf("text found %d times in %s — provide more context to make it unique", count, fullPath)
	}

	idx := strings.Index(text, oldText)
	lineNumber := 1 + strings.Count(text[:idx], "\n")

	if dryRun {
		fmt.Printf("Would replace in %s (line %d):\n", fullPath, lineNumber)
		fmt.Printf("  - %s\n", truncate(oldText, 120))
		fmt.Printf("  + %s\n", truncate(newText, 120))
		return nil
	}

	newContent := strings.Replace(text, oldText, newText, 1)
	if err := os.WriteFile(fullPath, []byte(newContent), 0644); err != nil {
		return fmt.Errorf("write %s: %w", fullPath, err)
	}

	fmt.Printf("Patched %s (line %d):\n", fullPath, lineNumber)
	fmt.Printf("  - %s\n", truncate(oldText, 120))
	fmt.Printf("  + %s\n", truncate(newText, 120))

	// Emit amendment_applied event
	emitEvent(dbPath, events.StreamSpecification, events.TypeAmendmentApplied, "", map[string]interface{}{
		"file": relPath,
		"line": lineNumber,
	})

	if !NoGuidance {
		fmt.Println("\nNext: ddis parse manifest.yaml && ddis validate")
		fmt.Println("  Re-parse to verify the patch didn't break anything.")
	}
	return nil
}

// runPatchElement replaces text within a spec element's line range.
func runPatchElement(db *sql.DB, specID int64, target, oldText, newText string, dryRun bool) error {
	relPath, lineStart, lineEnd, err := resolveTarget(db, specID, target)
	if err != nil {
		return err
	}

	baseDir, err := specBaseDir(db, specID)
	if err != nil {
		return err
	}
	fullPath := filepath.Join(baseDir, relPath)

	content, err := os.ReadFile(fullPath)
	if err != nil {
		return fmt.Errorf("read %s: %w", fullPath, err)
	}
	lines := strings.Split(string(content), "\n")

	// Scope to element's line range (1-indexed)
	scopeStart := 0
	scopeEnd := len(lines)
	if lineStart > 0 {
		scopeStart = lineStart - 1
	}
	if lineEnd > 0 && lineEnd <= len(lines) {
		scopeEnd = lineEnd
	}

	scope := strings.Join(lines[scopeStart:scopeEnd], "\n")

	count := strings.Count(scope, oldText)
	if count == 0 {
		return fmt.Errorf("text not found in %s %s (lines %d-%d):\n  %q", target, relPath, lineStart, lineEnd, oldText)
	}
	if count > 1 {
		return fmt.Errorf("text found %d times in %s (lines %d-%d) — provide more context", count, target, lineStart, lineEnd)
	}

	idx := strings.Index(scope, oldText)
	matchLine := scopeStart + 1 + strings.Count(scope[:idx], "\n")

	if dryRun {
		fmt.Printf("Would replace in %s at %s (line %d):\n", target, relPath, matchLine)
		fmt.Printf("  - %s\n", truncate(oldText, 120))
		fmt.Printf("  + %s\n", truncate(newText, 120))
		return nil
	}

	// Apply: rebuild file with modified scope
	newScope := strings.Replace(scope, oldText, newText, 1)
	var builder strings.Builder
	if scopeStart > 0 {
		builder.WriteString(strings.Join(lines[:scopeStart], "\n"))
		builder.WriteString("\n")
	}
	builder.WriteString(newScope)
	if scopeEnd < len(lines) {
		builder.WriteString("\n")
		builder.WriteString(strings.Join(lines[scopeEnd:], "\n"))
	}

	if err := os.WriteFile(fullPath, []byte(builder.String()), 0644); err != nil {
		return fmt.Errorf("write %s: %w", fullPath, err)
	}

	fmt.Printf("Patched %s in %s (line %d):\n", target, relPath, matchLine)
	fmt.Printf("  - %s\n", truncate(oldText, 120))
	fmt.Printf("  + %s\n", truncate(newText, 120))

	// Emit amendment_applied event
	specHash := specHashFromDB(db, specID)
	emitEvent(".", events.StreamSpecification, events.TypeAmendmentApplied, specHash, map[string]interface{}{
		"element": target,
		"file":    relPath,
		"line":    matchLine,
	})

	if !NoGuidance {
		fmt.Println("\nNext: ddis parse manifest.yaml && ddis validate")
		fmt.Println("  Re-parse to verify the patch didn't break anything.")
	}
	return nil
}

// resolveTarget looks up a spec element and returns its file path and line range.
var (
	patchInvRe  = regexp.MustCompile(`^((?:APP-)?INV-\d{3})$`)
	patchADRRe  = regexp.MustCompile(`^((?:APP-)?ADR-\d{3})$`)
	patchGateRe = regexp.MustCompile(`^Gate-?((?:M-)?[1-9]\d*)$`)
	patchSecRe  = regexp.MustCompile(`^§(\d+(?:\.\d+)*)$`)
)

func resolveTarget(db *sql.DB, specID int64, target string) (filePath string, lineStart, lineEnd int, err error) {
	target = strings.TrimSpace(target)

	if m := patchInvRe.FindStringSubmatch(target); m != nil {
		id := strings.ToUpper(m[1])
		// Prefer module definition (longer body) over constitution declaration
		err = db.QueryRow(`
			SELECT sf.file_path, i.line_start, i.line_end
			FROM invariants i JOIN source_files sf ON i.source_file_id = sf.id
			WHERE i.spec_id = ? AND i.invariant_id = ?
			ORDER BY (i.line_end - i.line_start) DESC LIMIT 1`, specID, id).Scan(&filePath, &lineStart, &lineEnd)
		if err != nil {
			return "", 0, 0, fmt.Errorf("invariant %s not found: %w", id, err)
		}
		return
	}

	if m := patchADRRe.FindStringSubmatch(target); m != nil {
		id := strings.ToUpper(m[1])
		err = db.QueryRow(`
			SELECT sf.file_path, a.line_start, a.line_end
			FROM adrs a JOIN source_files sf ON a.source_file_id = sf.id
			WHERE a.spec_id = ? AND a.adr_id = ?
			ORDER BY (a.line_end - a.line_start) DESC LIMIT 1`, specID, id).Scan(&filePath, &lineStart, &lineEnd)
		if err != nil {
			return "", 0, 0, fmt.Errorf("ADR %s not found: %w", id, err)
		}
		return
	}

	if m := patchGateRe.FindStringSubmatch(target); m != nil {
		id := "Gate-" + m[1]
		err = db.QueryRow(`
			SELECT sf.file_path, qg.line_start, qg.line_end
			FROM quality_gates qg JOIN source_files sf ON qg.source_file_id = sf.id
			WHERE qg.spec_id = ? AND qg.gate_id = ?
			LIMIT 1`, specID, id).Scan(&filePath, &lineStart, &lineEnd)
		if err != nil {
			return "", 0, 0, fmt.Errorf("gate %s not found: %w", id, err)
		}
		return
	}

	if m := patchSecRe.FindStringSubmatch(target); m != nil {
		sectionPath := "§" + m[1]
		err = db.QueryRow(`
			SELECT sf.file_path, s.line_start, s.line_end
			FROM sections s JOIN source_files sf ON s.source_file_id = sf.id
			WHERE s.spec_id = ? AND s.section_path = ?
			LIMIT 1`, specID, sectionPath).Scan(&filePath, &lineStart, &lineEnd)
		if err != nil {
			return "", 0, 0, fmt.Errorf("section %s not found: %w", sectionPath, err)
		}
		return
	}

	return "", 0, 0, fmt.Errorf("cannot parse target %q: expected APP-INV-NNN, APP-ADR-NNN, Gate-N, or §N.M", target)
}

// specBaseDir returns the directory containing the spec's manifest file.
func specBaseDir(db *sql.DB, specID int64) (string, error) {
	var specPath string
	if err := db.QueryRow("SELECT spec_path FROM spec_index WHERE id = ?", specID).Scan(&specPath); err != nil {
		return "", fmt.Errorf("get spec path: %w", err)
	}
	return filepath.Dir(specPath), nil
}

func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func truncate(s string, max int) string {
	// Replace newlines for display
	s = strings.ReplaceAll(s, "\n", "\\n")
	if len(s) > max {
		return s[:max-3] + "..."
	}
	return s
}
