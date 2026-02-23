package parser

import (
	"strings"
)

// Frontmatter represents parsed YAML frontmatter from a module file.
type Frontmatter struct {
	Module      string
	Domain      string
	Tier        int
	Description string
	DDISVersion string
	TierMode    string
	EndLine     int // 0-indexed line after closing ---
}

// ParseFrontmatter extracts YAML frontmatter from lines starting at line 0.
// Returns nil if no frontmatter is present.
func ParseFrontmatter(lines []string) *Frontmatter {
	if len(lines) == 0 || strings.TrimSpace(lines[0]) != "---" {
		return nil
	}

	endIdx := -1
	for i := 1; i < len(lines); i++ {
		if strings.TrimSpace(lines[i]) == "---" {
			endIdx = i
			break
		}
	}
	if endIdx < 0 {
		return nil
	}

	fm := &Frontmatter{EndLine: endIdx + 1}
	for _, line := range lines[1:endIdx] {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "module:") {
			fm.Module = strings.TrimSpace(strings.TrimPrefix(line, "module:"))
		} else if strings.HasPrefix(line, "domain:") {
			fm.Domain = strings.TrimSpace(strings.TrimPrefix(line, "domain:"))
		} else if strings.HasPrefix(line, "tier:") {
			val := strings.TrimSpace(strings.TrimPrefix(line, "tier:"))
			if val == "1" {
				fm.Tier = 1
			} else if val == "2" {
				fm.Tier = 2
			} else if val == "3" {
				fm.Tier = 3
			}
		} else if strings.HasPrefix(line, "ddis_version:") {
			fm.DDISVersion = strings.Trim(strings.TrimSpace(strings.TrimPrefix(line, "ddis_version:")), "\"")
		} else if strings.HasPrefix(line, "tier_mode:") {
			fm.TierMode = strings.TrimSpace(strings.TrimPrefix(line, "tier_mode:"))
		}
		// description is multi-line, handle the simple case
		if strings.HasPrefix(line, "description:") {
			desc := strings.TrimSpace(strings.TrimPrefix(line, "description:"))
			if desc == ">" || desc == "|" {
				// Multi-line — collect subsequent indented lines
				// This is simplified; we just skip the > and collect
			} else {
				fm.Description = desc
			}
		}
	}
	return fm
}
