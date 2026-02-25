package absorb

import (
	"bufio"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/wvandaal/ddis/internal/annotate"
)

// ddis:implements APP-ADR-024 (bilateral specification)
// ddis:implements APP-ADR-025 (heuristic scan over AST parsing)
// ddis:maintains APP-INV-032 (symmetric reconciliation)
// ddis:maintains APP-INV-033 (absorption format parity)

// DefaultExcludes are directory patterns excluded from absorption scanning by default.
var DefaultExcludes = []string{".git", "vendor", "node_modules", "bin", "testdata", ".ddis"}

// maxHeuristicPerFile caps heuristic patterns per file to avoid overwhelming output.
const maxHeuristicPerFile = 200

// Heuristic patterns for code analysis.
var (
	assertionRe    = regexp.MustCompile(`(?i)(assert|require|expect|must|shall)\s*[.(]`)
	errorReturnRe  = regexp.MustCompile(`(?i)return\s+.*(?:err\b|fmt\.Errorf|errors\.)`)
	guardClauseRe  = regexp.MustCompile(`^\s*if\s+.+\{\s*$`)
	stateTransRe   = regexp.MustCompile(`(?i)(?:state|status)\s*[=:]\s*["']?\w+["']?`)
	interfaceDefRe = regexp.MustCompile(`(?i)(?:interface\s*\{|type\s+\w+\s+interface)`)
)

// patternDef pairs a regex with its classification and confidence.
type patternDef struct {
	re         *regexp.Regexp
	patType    string
	confidence float64
}

var heuristicDefs = []patternDef{
	{assertionRe, "assertion", 0.8},
	{errorReturnRe, "error_return", 0.6},
	{guardClauseRe, "guard_clause", 0.7},
	{stateTransRe, "state_transition", 0.5},
	{interfaceDefRe, "interface_def", 0.7},
}

// ScanPatterns extracts both annotations and heuristic patterns from code.
func ScanPatterns(codeRoot string) (*AbsorbResult, error) {
	result := &AbsorbResult{}

	// 1. Run annotation scan to get ddis: annotations.
	annotResult, err := annotate.Scan(annotate.ScanOptions{Root: codeRoot})
	if err != nil {
		return nil, err
	}

	// Convert annotations to patterns (high confidence).
	for _, a := range annotResult.Annotations {
		result.Patterns = append(result.Patterns, Pattern{
			File:       a.FilePath,
			Line:       a.Line,
			Type:       "annotation",
			Text:       a.RawComment,
			Confidence: 1.0,
			Language:   a.Language,
		})
	}

	// 2. Walk code files and extract heuristic patterns.
	err = filepath.WalkDir(codeRoot, func(path string, d fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return nil // skip errors
		}

		// Skip excluded directories.
		if d.IsDir() {
			name := d.Name()
			for _, excl := range DefaultExcludes {
				if matched, _ := filepath.Match(excl, name); matched {
					return filepath.SkipDir
				}
			}
			return nil
		}

		// Skip symlinks.
		if d.Type()&fs.ModeSymlink != 0 {
			return nil
		}

		// Only scan files the annotate grammar recognizes.
		_, lang := annotate.LookupCommentPrefixes(d.Name())
		if lang == "" {
			return nil
		}

		relPath, relErr := filepath.Rel(codeRoot, path)
		if relErr != nil {
			relPath = path
		}

		patterns, scanErr := scanFileHeuristic(path, relPath, lang)
		if scanErr != nil {
			return nil // skip unreadable files
		}

		result.Patterns = append(result.Patterns, patterns...)
		result.TotalFiles++
		return nil
	})
	if err != nil {
		return nil, err
	}

	// TotalFiles from annotation scan contributes too.
	if annotResult.FilesScanned > result.TotalFiles {
		result.TotalFiles = annotResult.FilesScanned
	}

	result.TotalPatterns = len(result.Patterns)
	return result, nil
}

// scanFileHeuristic reads a single file and extracts heuristic code patterns.
func scanFileHeuristic(absPath, relPath, lang string) ([]Pattern, error) {
	f, err := os.Open(absPath)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	var patterns []Pattern
	scanner := bufio.NewScanner(f)
	lineNum := 0
	heuristicCount := 0

	// Track whether the previous line was a guard_clause opening to pair with
	// a return on the next line (Go style: if ... {\n  return ...\n}).
	prevGuard := false

	for scanner.Scan() {
		lineNum++
		line := scanner.Text()
		trimmed := strings.TrimSpace(line)

		// Skip blank lines and pure comment lines for heuristic detection.
		if trimmed == "" || isCommentOnly(trimmed, lang) {
			prevGuard = false
			continue
		}

		// Check guard clause completion: the line after an if-opening with a
		// return is the actual guard. We tag the opening line.
		if prevGuard {
			prevGuard = false
			if strings.HasPrefix(trimmed, "return ") || trimmed == "return" {
				// The previous if-line was already recorded if it matched.
				continue
			}
		}

		if heuristicCount >= maxHeuristicPerFile {
			break
		}

		for _, def := range heuristicDefs {
			if def.re.MatchString(line) {
				patterns = append(patterns, Pattern{
					File:       relPath,
					Line:       lineNum,
					Type:       def.patType,
					Text:       trimmed,
					Confidence: def.confidence,
					Language:   lang,
				})
				heuristicCount++

				if def.patType == "guard_clause" {
					prevGuard = true
				}

				// One match per line is enough.
				break
			}
		}
	}

	return patterns, scanner.Err()
}

// isCommentOnly returns true if the trimmed line is exclusively a comment
// (not a trailing comment on a code line).
func isCommentOnly(trimmed, lang string) bool {
	switch lang {
	case "Go", "Rust", "TypeScript", "JavaScript", "Java", "C", "C++", "C#",
		"Kotlin", "Swift":
		return strings.HasPrefix(trimmed, "//")
	case "Python", "Ruby", "Shell", "YAML", "TOML", "Perl":
		return strings.HasPrefix(trimmed, "#")
	case "SQL", "Lua", "Haskell":
		return strings.HasPrefix(trimmed, "--")
	case "Lisp", "Clojure", "Assembly":
		return strings.HasPrefix(trimmed, ";")
	case "LaTeX", "Erlang":
		return strings.HasPrefix(trimmed, "%")
	case "HTML", "XML", "Markdown":
		return strings.HasPrefix(trimmed, "<!--")
	}
	return false
}
