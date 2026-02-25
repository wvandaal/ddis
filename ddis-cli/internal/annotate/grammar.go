package annotate

import (
	"path/filepath"
	"regexp"
	"strings"
)

// ddis:maintains APP-INV-017 (annotation portability)

// AnnotationRe captures the DDIS annotation pattern from comment text.
// Groups: 1=verb, 2=target, 3=rest (may contain qualifier)
var AnnotationRe = regexp.MustCompile(
	`ddis:(maintains|implements|interfaces|tests|validates-via|postcondition|relates-to|satisfies)\s+` +
		`((?:APP-)?(?:INV|ADR)-\d{3}|Gate-\d+|S\d+(?:\.\d+)*|@[\w-]+)(.*)$`)

// QualifierRe extracts a parenthesized qualifier from the rest of the match.
var QualifierRe = regexp.MustCompile(`\(\s*([^)]+)\s*\)`)

// CommentFamily maps file extensions to their comment prefix(es).
var CommentFamily = map[string][]string{
	// C-style
	".go":    {"//"},
	".rs":    {"//"},
	".ts":    {"//"},
	".tsx":   {"//"},
	".js":    {"//"},
	".jsx":   {"//"},
	".java":  {"//"},
	".c":     {"//"},
	".h":     {"//"},
	".cpp":   {"//"},
	".cc":    {"//"},
	".cs":    {"//"},
	".kt":    {"//"},
	".swift": {"//"},
	// Hash
	".py":   {"#"},
	".rb":   {"#"},
	".sh":   {"#"},
	".bash": {"#"},
	".zsh":  {"#"},
	".yaml": {"#"},
	".yml":  {"#"},
	".toml": {"#"},
	".pl":   {"#"},
	// SQL
	".sql": {"--"},
	".lua": {"--"},
	".hs":  {"--"},
	// Semicolon
	".lisp": {";"},
	".clj":  {";"},
	".asm":  {";"},
	// Percent
	".tex": {"%"},
	".erl": {"%"},
	// HTML-style
	".html": {"<!--"},
	".xml":  {"<!--"},
	".md":   {"<!--"},
}

// LanguageName maps extensions to human-readable language names.
var LanguageName = map[string]string{
	".go": "Go", ".rs": "Rust", ".ts": "TypeScript", ".tsx": "TypeScript",
	".js": "JavaScript", ".jsx": "JavaScript", ".java": "Java",
	".c": "C", ".h": "C", ".cpp": "C++", ".cc": "C++", ".cs": "C#",
	".kt": "Kotlin", ".swift": "Swift",
	".py": "Python", ".rb": "Ruby", ".sh": "Shell", ".bash": "Shell",
	".zsh": "Shell", ".yaml": "YAML", ".yml": "YAML", ".toml": "TOML",
	".pl": "Perl", ".sql": "SQL", ".lua": "Lua", ".hs": "Haskell",
	".lisp": "Lisp", ".clj": "Clojure", ".asm": "Assembly",
	".tex": "LaTeX", ".erl": "Erlang",
	".html": "HTML", ".xml": "XML", ".md": "Markdown",
}

// ParseAnnotation extracts an annotation from a comment line.
// Returns nil if the line doesn't contain a ddis annotation.
func ParseAnnotation(commentText string) *Annotation {
	m := AnnotationRe.FindStringSubmatch(commentText)
	if m == nil {
		return nil
	}

	a := &Annotation{
		Verb:   m[1],
		Target: m[2],
	}

	// Extract qualifier if present
	if rest := strings.TrimSpace(m[3]); rest != "" {
		if qm := QualifierRe.FindStringSubmatch(rest); qm != nil {
			a.Qualifier = strings.TrimSpace(qm[1])
		}
	}

	return a
}

// ExtractComment strips the comment prefix from a line and returns the comment body.
// Returns empty string if the line doesn't start with a recognized comment.
func ExtractComment(line string, prefixes []string) string {
	trimmed := strings.TrimSpace(line)
	for _, prefix := range prefixes {
		if prefix == "<!--" {
			// HTML-style: strip <!-- and optional -->
			if strings.HasPrefix(trimmed, "<!--") {
				body := strings.TrimPrefix(trimmed, "<!--")
				body = strings.TrimSuffix(body, "-->")
				return strings.TrimSpace(body)
			}
		} else {
			if strings.HasPrefix(trimmed, prefix) {
				return strings.TrimSpace(strings.TrimPrefix(trimmed, prefix))
			}
		}
	}
	return ""
}

// LookupCommentPrefixes returns the comment prefix(es) for a file extension.
func LookupCommentPrefixes(filename string) ([]string, string) {
	ext := strings.ToLower(filepath.Ext(filename))
	prefixes, ok := CommentFamily[ext]
	if !ok {
		return nil, ""
	}
	lang := LanguageName[ext]
	if lang == "" {
		lang = ext
	}
	return prefixes, lang
}
