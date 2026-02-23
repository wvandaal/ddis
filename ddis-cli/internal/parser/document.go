package parser

import (
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/wvandaal/ddis/internal/storage"
)

// ParseDocument reads a monolith markdown file and populates the index.
func ParseDocument(filePath string, db storage.DB) (int64, error) {
	content, err := os.ReadFile(filePath)
	if err != nil {
		return 0, fmt.Errorf("read spec: %w", err)
	}

	text := string(content)
	lines := splitLines(text)

	// Create spec index
	fm := ParseFrontmatter(lines)
	specIndex := &storage.SpecIndex{
		SpecPath:    filePath,
		TotalLines:  len(lines),
		ContentHash: sha256Hex(text),
		ParsedAt:    nowISO8601(),
		SourceType:  "monolith",
	}
	if fm != nil {
		specIndex.DDISVersion = fm.DDISVersion
	}

	specID, err := storage.InsertSpecIndex(db, specIndex)
	if err != nil {
		return 0, fmt.Errorf("insert spec_index: %w", err)
	}

	// Insert the single source file
	sf := &storage.SourceFile{
		SpecID:      specID,
		FilePath:    filePath,
		FileRole:    "monolith",
		ContentHash: sha256Hex(text),
		LineCount:   len(lines),
		RawText:     text,
	}
	sfID, err := storage.InsertSourceFile(db, sf)
	if err != nil {
		return 0, fmt.Errorf("insert source_file: %w", err)
	}

	// Pass 1: Build section tree
	sections := BuildSectionTree(lines)
	if err := InsertSectionsDB(db, sections, specID, sfID, lines); err != nil {
		return 0, fmt.Errorf("insert sections: %w", err)
	}

	// Pass 2: Extract DDIS elements
	if err := extractElementsFromLines(lines, sections, specID, sfID, db); err != nil {
		return 0, fmt.Errorf("extract elements: %w", err)
	}

	// Pass 3: Extract cross-references
	if err := ExtractCrossReferences(lines, sections, specID, sfID, db); err != nil {
		return 0, fmt.Errorf("extract cross-references: %w", err)
	}

	// Pass 4: Resolve cross-references
	if err := ResolveCrossReferences(db, specID); err != nil {
		return 0, fmt.Errorf("resolve cross-references: %w", err)
	}

	// Record formatting hints for round-trip fidelity
	if err := extractFormattingHints(lines, specID, sfID, db); err != nil {
		return 0, fmt.Errorf("extract formatting hints: %w", err)
	}

	return specID, nil
}

// extractElementsFromLines runs all element recognizers on the given lines.
func extractElementsFromLines(lines []string, sections []*SectionNode, specID, sfID int64, db storage.DB) error {
	if err := ExtractInvariants(lines, sections, specID, sfID, db); err != nil {
		return fmt.Errorf("invariants: %w", err)
	}
	if err := ExtractADRs(lines, sections, specID, sfID, db); err != nil {
		return fmt.Errorf("ADRs: %w", err)
	}
	if err := ExtractGates(lines, sections, specID, db); err != nil {
		return fmt.Errorf("gates: %w", err)
	}
	if err := ExtractNegativeSpecs(lines, sections, specID, sfID, db); err != nil {
		return fmt.Errorf("negative specs: %w", err)
	}
	if err := ExtractVerificationPrompts(lines, sections, specID, db); err != nil {
		return fmt.Errorf("verification prompts: %w", err)
	}
	if err := ExtractMetaInstructions(lines, sections, specID, db); err != nil {
		return fmt.Errorf("meta-instructions: %w", err)
	}
	if err := ExtractWorkedExamples(lines, sections, specID, db); err != nil {
		return fmt.Errorf("worked examples: %w", err)
	}
	if err := ExtractWhyNots(lines, sections, specID, db); err != nil {
		return fmt.Errorf("WHY NOT annotations: %w", err)
	}
	if err := ExtractComparisonBlocks(lines, sections, specID, db); err != nil {
		return fmt.Errorf("comparison blocks: %w", err)
	}
	if err := ExtractPerformanceBudgets(lines, sections, specID, db); err != nil {
		return fmt.Errorf("performance budgets: %w", err)
	}
	if err := ExtractStateMachines(lines, sections, specID, db); err != nil {
		return fmt.Errorf("state machines: %w", err)
	}
	if err := ExtractGlossaryEntries(lines, sections, specID, db); err != nil {
		return fmt.Errorf("glossary entries: %w", err)
	}
	return nil
}

// extractElementsFromFile is used by the modular parser to extract elements from a single file.
func extractElementsFromFile(lines []string, specID, sfID int64, db storage.DB) error {
	sections := BuildSectionTree(lines)
	// Re-insert sections for this file (they were already inserted in parseAndInsertFile,
	// but we need the in-memory nodes with DBIDs for element extraction).
	// Actually, we need to query them back. For simplicity, rebuild and use the in-memory sections.
	// The sections are already in the DB from parseAndInsertFile, so we query their IDs.
	if err := loadSectionDBIDs(db, sections, specID, sfID); err != nil {
		return err
	}

	if err := extractElementsFromLines(lines, sections, specID, sfID, db); err != nil {
		return err
	}

	if err := ExtractCrossReferences(lines, sections, specID, sfID, db); err != nil {
		return err
	}

	return nil
}

// loadSectionDBIDs queries the DB to populate DBID fields on in-memory section nodes.
func loadSectionDBIDs(db storage.DB, sections []*SectionNode, specID, sfID int64) error {
	rows, err := db.Query(
		`SELECT id, section_path, line_start FROM sections WHERE spec_id = ? AND source_file_id = ?`,
		specID, sfID)
	if err != nil {
		return err
	}
	defer rows.Close()

	type secRow struct {
		id        int64
		path      string
		lineStart int
	}
	var dbSections []secRow
	for rows.Next() {
		var r secRow
		if err := rows.Scan(&r.id, &r.path, &r.lineStart); err != nil {
			return err
		}
		dbSections = append(dbSections, r)
	}

	for _, dbSec := range dbSections {
		for _, node := range sections {
			if node.SectionPath == dbSec.path && node.LineStart+1 == dbSec.lineStart {
				node.DBID = dbSec.id
				break
			}
		}
	}
	return nil
}

// extractFormattingHints records blank lines and horizontal rules for round-trip fidelity.
func extractFormattingHints(lines []string, specID, sfID int64, db storage.DB) error {
	for i, line := range lines {
		trimmed := strings.TrimSpace(line)

		if trimmed == "" {
			fh := &storage.FormattingHint{
				SpecID:       specID,
				SourceFileID: sfID,
				LineNumber:   i + 1,
				HintType:     "blank_line",
			}
			if _, err := storage.InsertFormattingHint(db, fh); err != nil {
				return err
			}
		} else if trimmed == "---" && !FrontmatterRe.MatchString(trimmed) {
			fh := &storage.FormattingHint{
				SpecID:       specID,
				SourceFileID: sfID,
				LineNumber:   i + 1,
				HintType:     "hr",
				HintValue:    "---",
			}
			if _, err := storage.InsertFormattingHint(db, fh); err != nil {
				return err
			}
		}
	}
	return nil
}

func splitLines(s string) []string {
	// Split but preserve the exact content for each line
	if s == "" {
		return nil
	}
	return strings.Split(s, "\n")
}

func nowISO8601() string {
	return time.Now().UTC().Format(time.RFC3339)
}
