package cli

// ddis:implements APP-ADR-051

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
)

var (
	renameOld      string
	renameNew      string
	renameDryRun   bool
	renameCodeRoot string
)

var renameCmd = &cobra.Command{
	Use:   "rename",
	Short: "Rename a spec ID across all spec and code files",
	Long: `Renames a DDIS element ID (e.g. APP-INV-001 → APP-INV-100) across
all spec source files (*.md, manifest.yaml) in the current directory tree,
and optionally across Go annotation files under --code-root.

Each occurrence is reported with file:line before replacement. Use --dry-run
to preview changes without writing any files.

Examples:
  ddis rename --old APP-INV-001 --new APP-INV-100
  ddis rename --old APP-ADR-005 --new APP-ADR-050 --dry-run
  ddis rename --old APP-INV-010 --new APP-INV-099 --code-root .`,
	RunE:          runRename,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	renameCmd.Flags().StringVar(&renameOld, "old", "", "The ID to rename from (required)")
	renameCmd.Flags().StringVar(&renameNew, "new", "", "The ID to rename to (required)")
	renameCmd.Flags().BoolVar(&renameDryRun, "dry-run", false, "Preview changes without writing")
	renameCmd.Flags().StringVar(&renameCodeRoot, "code-root", "", "Also scan Go source files under this path")
	renameCmd.GroupID = "utility"
}

func runRename(cmd *cobra.Command, args []string) error {
	if renameOld == "" {
		return fmt.Errorf("--old is required")
	}
	if renameNew == "" {
		return fmt.Errorf("--new is required")
	}
	if renameOld == renameNew {
		return fmt.Errorf("--old and --new are identical; nothing to change")
	}

	totalOccurrences := 0
	totalFiles := 0

	// Walk spec files: *.md and *.yaml in current directory tree.
	specFiles, err := collectFiles(".", []string{".md", ".yaml"})
	if err != nil {
		return fmt.Errorf("collect spec files: %w", err)
	}

	occurrences, files, err := processRenameFiles(specFiles, renameOld, renameNew, renameDryRun)
	if err != nil {
		return err
	}
	totalOccurrences += occurrences
	totalFiles += files

	// Walk Go source files under --code-root if provided.
	if renameCodeRoot != "" {
		goFiles, err := collectFiles(renameCodeRoot, []string{".go"})
		if err != nil {
			return fmt.Errorf("collect Go files: %w", err)
		}

		goOccurrences, goFiles2, err := processRenameFiles(goFiles, renameOld, renameNew, renameDryRun)
		if err != nil {
			return err
		}
		totalOccurrences += goOccurrences
		totalFiles += goFiles2
	}

	// Print summary.
	if renameDryRun {
		fmt.Printf("\n[dry-run] Would rename %d occurrence(s) across %d file(s): %s → %s\n",
			totalOccurrences, totalFiles, renameOld, renameNew)
	} else {
		fmt.Printf("\nRenamed %d occurrence(s) across %d file(s): %s → %s\n",
			totalOccurrences, totalFiles, renameOld, renameNew)
	}

	// Emit rename event to Stream 2 (Specification) — best-effort.
	if !renameDryRun && totalOccurrences > 0 {
		emitEvent(".", events.StreamSpecification, events.TypeAmendmentApplied, "", map[string]interface{}{
			"command":     "rename",
			"old_id":      renameOld,
			"new_id":      renameNew,
			"occurrences": totalOccurrences,
			"files":       totalFiles,
		})
	}

	if !NoGuidance && !renameDryRun && totalOccurrences > 0 {
		fmt.Println("\nNext: ddis parse manifest.yaml && ddis validate")
		fmt.Println("  Re-parse to verify the rename didn't break any cross-references.")
	}

	return nil
}

// collectFiles walks root and returns all files whose extension is in exts.
func collectFiles(root string, exts []string) ([]string, error) {
	var paths []string
	err := filepath.Walk(root, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.IsDir() {
			// Skip hidden directories (e.g. .git, .ddis) and vendor.
			base := info.Name()
			if base != "." && (strings.HasPrefix(base, ".") || base == "vendor") {
				return filepath.SkipDir
			}
			return nil
		}
		ext := strings.ToLower(filepath.Ext(path))
		for _, e := range exts {
			if ext == e {
				paths = append(paths, path)
				break
			}
		}
		return nil
	})
	return paths, err
}

// processRenameFiles scans files for oldID, reports occurrences, and replaces
// them with newID unless dryRun is true. Returns total occurrences and affected
// file count.
func processRenameFiles(files []string, oldID, newID string, dryRun bool) (occurrences int, fileCount int, err error) {
	for _, path := range files {
		fileOccurrences, fileErr := processRenameFile(path, oldID, newID, dryRun)
		if fileErr != nil {
			return occurrences, fileCount, fileErr
		}
		if fileOccurrences > 0 {
			occurrences += fileOccurrences
			fileCount++
		}
	}
	return occurrences, fileCount, nil
}

// processRenameFile handles a single file: finds occurrences of oldID, prints
// each with file:line, and replaces if not dry-run. Returns occurrence count.
func processRenameFile(path, oldID, newID string, dryRun bool) (int, error) {
	f, err := os.Open(path)
	if err != nil {
		return 0, fmt.Errorf("open %s: %w", path, err)
	}
	defer f.Close()

	// Scan line by line to record occurrence locations.
	type occurrence struct {
		line int
		text string
	}
	var found []occurrence
	var lines []string

	scanner := bufio.NewScanner(f)
	lineNum := 0
	for scanner.Scan() {
		lineNum++
		text := scanner.Text()
		lines = append(lines, text)
		if strings.Contains(text, oldID) {
			found = append(found, occurrence{line: lineNum, text: text})
		}
	}
	if err := scanner.Err(); err != nil {
		return 0, fmt.Errorf("read %s: %w", path, err)
	}

	if len(found) == 0 {
		return 0, nil
	}

	// Report each occurrence.
	for _, o := range found {
		if dryRun {
			fmt.Printf("[dry-run] %s:%d: %s\n", path, o.line, strings.TrimSpace(o.text))
		} else {
			fmt.Printf("%s:%d: %s\n", path, o.line, strings.TrimSpace(o.text))
		}
	}

	if dryRun {
		return len(found), nil
	}

	// Rebuild file content with replacements applied.
	var builder strings.Builder
	for i, line := range lines {
		if i > 0 {
			builder.WriteByte('\n')
		}
		builder.WriteString(strings.ReplaceAll(line, oldID, newID))
	}
	// Preserve trailing newline if the original file had one.
	// bufio.Scanner strips the final newline; add it back unconditionally
	// to match standard text file convention.
	builder.WriteByte('\n')

	if err := os.WriteFile(path, []byte(builder.String()), 0644); err != nil {
		return 0, fmt.Errorf("write %s: %w", path, err)
	}

	return len(found), nil
}
