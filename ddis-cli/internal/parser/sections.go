package parser

import (
	"crypto/sha256"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// SectionNode is an in-memory section before DB insertion.
type SectionNode struct {
	SectionPath  string
	Title        string
	HeadingLevel int
	LineStart    int // 0-indexed
	LineEnd      int // 0-indexed, exclusive
	ParentIdx    int // index into flat list, -1 for roots
	Children     []int
	DBID         int64 // set after insertion
}

// BuildSectionTree walks lines and builds a flat list of sections from headings.
// LineStart/LineEnd are 0-indexed. LineEnd of each section is set to the start of the
// next same-or-higher-level heading (or end of file).
func BuildSectionTree(lines []string) []*SectionNode {
	var nodes []*SectionNode
	// Stack tracks the most recent heading at each level for parent resolution
	stack := make([]*SectionNode, 7) // levels 1-6, index 0 unused
	pathCounts := make(map[string]int)

	for i, line := range lines {
		m := HeadingRe.FindStringSubmatch(line)
		if m == nil {
			continue
		}
		level := len(m[1])
		title := strings.TrimSpace(m[2])
		path := normalizeSectionPath(title, level)

		// Deduplicate paths by appending a counter suffix
		pathCounts[path]++
		if pathCounts[path] > 1 {
			path = fmt.Sprintf("%s~%d", path, pathCounts[path])
		}

		node := &SectionNode{
			SectionPath:  path,
			Title:        title,
			HeadingLevel: level,
			LineStart:    i,
			ParentIdx:    -1,
		}

		// Find parent: walk up the stack for the nearest lower level
		for l := level - 1; l >= 1; l-- {
			if stack[l] != nil {
				node.ParentIdx = indexOf(nodes, stack[l])
				break
			}
		}

		// Close the previous section at this level or higher
		if len(nodes) > 0 {
			closePreviousSections(nodes, i, level)
		}

		nodes = append(nodes, node)
		stack[level] = node
		// Clear deeper levels (a new h2 resets h3, h4, etc.)
		for l := level + 1; l <= 6; l++ {
			stack[l] = nil
		}
	}

	// Close all remaining open sections at EOF
	totalLines := len(lines)
	for _, n := range nodes {
		if n.LineEnd == 0 {
			n.LineEnd = totalLines
		}
	}

	return nodes
}

// closePreviousSections sets LineEnd for all unclosed sections at level >= targetLevel
// when a new heading at targetLevel is encountered at lineIdx.
func closePreviousSections(nodes []*SectionNode, lineIdx int, targetLevel int) {
	for j := len(nodes) - 1; j >= 0; j-- {
		n := nodes[j]
		if n.LineEnd != 0 {
			continue
		}
		if n.HeadingLevel >= targetLevel {
			n.LineEnd = lineIdx
		}
	}
}

// InsertSectionsDB inserts all section nodes into the DB using the storage layer.
func InsertSectionsDB(db storage.DB, sections []*SectionNode, specID, sourceFileID int64, lines []string) error {
	for _, sec := range sections {
		rawText := extractRawText(lines, sec.LineStart, sec.LineEnd)
		hash := sha256Hex(rawText)

		var parentID *int64
		if sec.ParentIdx >= 0 && sec.ParentIdx < len(sections) {
			pid := sections[sec.ParentIdx].DBID
			parentID = &pid
		}

		s := &storage.Section{
			SpecID:       specID,
			SourceFileID: sourceFileID,
			SectionPath:  sec.SectionPath,
			Title:        sec.Title,
			HeadingLevel: sec.HeadingLevel,
			ParentID:     parentID,
			LineStart:    sec.LineStart + 1, // 1-indexed in DB
			LineEnd:      sec.LineEnd,       // exclusive, 1-indexed
			RawText:      rawText,
			ContentHash:  hash,
		}

		id, err := storage.InsertSection(db, s)
		if err != nil {
			return fmt.Errorf("insert section %q: %w", sec.SectionPath, err)
		}
		sec.DBID = id
	}
	return nil
}

// normalizeSectionPath converts a heading title to a canonical section path.
func normalizeSectionPath(title string, level int) string {
	// PART N: ...
	if m := PartRe.FindStringSubmatch(title); m != nil {
		return "PART-" + m[1]
	}

	// §N.M or N.M at start
	if m := SectionRe.FindStringSubmatch(title); m != nil {
		return "§" + m[1]
	}

	// Chapter N: ...
	if m := ChapterRe.FindStringSubmatch(title); m != nil {
		return "Chapter-" + m[1]
	}

	// Appendix X: ...
	if m := AppendixRe.FindStringSubmatch(title); m != nil {
		return "Appendix-" + m[1]
	}

	// Fallback: slugify the title
	return slugify(title)
}

// slugify creates a URL-safe identifier from a title.
func slugify(s string) string {
	s = strings.ToLower(s)
	var b strings.Builder
	for _, r := range s {
		switch {
		case r >= 'a' && r <= 'z', r >= '0' && r <= '9':
			b.WriteRune(r)
		case r == ' ' || r == '-' || r == '_':
			b.WriteRune('-')
		}
	}
	result := b.String()
	// Collapse multiple dashes
	for strings.Contains(result, "--") {
		result = strings.ReplaceAll(result, "--", "-")
	}
	return strings.Trim(result, "-")
}

func indexOf(nodes []*SectionNode, target *SectionNode) int {
	for i, n := range nodes {
		if n == target {
			return i
		}
	}
	return -1
}

func extractRawText(lines []string, start, end int) string {
	if start >= len(lines) {
		return ""
	}
	if end > len(lines) {
		end = len(lines)
	}
	return strings.Join(lines[start:end], "\n")
}

func sha256Hex(s string) string {
	h := sha256.Sum256([]byte(s))
	return fmt.Sprintf("%x", h)
}

// FindSectionForLine returns the most specific (deepest) section containing the given 0-indexed line.
func FindSectionForLine(sections []*SectionNode, line int) *SectionNode {
	var best *SectionNode
	for _, s := range sections {
		if line >= s.LineStart && line < s.LineEnd {
			if best == nil || s.HeadingLevel > best.HeadingLevel {
				best = s
			}
		}
	}
	return best
}
