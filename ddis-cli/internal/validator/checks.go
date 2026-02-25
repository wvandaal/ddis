package validator

// ddis:implements APP-ADR-014 (tiered contradiction detection)
// ddis:interfaces APP-INV-019 (contradiction graph soundness)

import (
	"database/sql"
	"fmt"
	"regexp"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-002 (validation determinism)
// ddis:maintains APP-INV-011 (check composability)
// ddis:maintains APP-INV-038 (cross-spec reference integrity)
// ddis:maintains APP-INV-040 (progressive validation monotonicity)

// Check 1: Cross-reference integrity — all refs should resolve.
type checkXRefIntegrity struct{}

func (c *checkXRefIntegrity) ID() int                { return 1 }
func (c *checkXRefIntegrity) Name() string           { return "Cross-reference integrity" }
func (c *checkXRefIntegrity) Applicable(string) bool { return true }

func (c *checkXRefIntegrity) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	unresolved, err := storage.GetUnresolvedRefs(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	if len(unresolved) > 0 {
		// Categorize: template/example refs are warnings, others are errors
		hasErrors := false
		for _, xr := range unresolved {
			sev := SeverityWarning
			// Common template patterns that are legitimately unresolved
			if isTemplateRef(xr.RefTarget) {
				sev = SeverityInfo
			} else {
				sev = SeverityError
				hasErrors = true
			}
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: sev,
				Message:  fmt.Sprintf("unresolved %s reference: %s", xr.RefType, xr.RefTarget),
				Location: fmt.Sprintf("line %d", xr.SourceLine),
			})
		}
		if hasErrors {
			result.Passed = false
		}
	}

	result.Summary = fmt.Sprintf("%d unresolved references", len(unresolved))
	return result
}

func isTemplateRef(target string) bool {
	lower := strings.ToLower(target)
	return strings.Contains(lower, "nnn") || strings.Contains(lower, "xxx") ||
		strings.Contains(lower, "n.m") || target == "§N.M" ||
		target == "INV-NNN" || target == "ADR-NNN"
}

// Check 2: INV-003 — Each invariant should have 4 core components.
type checkINV003Falsifiability struct{}

func (c *checkINV003Falsifiability) ID() int                { return 2 }
func (c *checkINV003Falsifiability) Name() string           { return "INV-003: Invariant falsifiability" }
func (c *checkINV003Falsifiability) Applicable(string) bool { return true }

func (c *checkINV003Falsifiability) Run(db *sql.DB, specID int64) CheckResult {
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

	missing := 0
	for _, inv := range invs {
		components := []struct {
			name  string
			value string
		}{
			{"statement", inv.Statement},
			{"semi_formal", inv.SemiFormal},
			{"violation_scenario", inv.ViolationScenario},
			{"validation_method", inv.ValidationMethod},
		}

		for _, comp := range components {
			if comp.value == "" {
				missing++
				result.Findings = append(result.Findings, Finding{
					CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityWarning,
					Message:     fmt.Sprintf("%s missing %s", inv.InvariantID, comp.name),
					InvariantID: inv.InvariantID,
				})
			}
		}
	}

	if missing > 0 {
		// Missing components are warnings, not errors (some invariants may legitimately lack fields)
		result.Summary = fmt.Sprintf("%d invariants checked, %d missing components", len(invs), missing)
	} else {
		result.Summary = fmt.Sprintf("all %d invariants have 4 core components", len(invs))
	}
	return result
}

// Check 3: INV-006 — Cross-reference density (no orphan sections).
type checkINV006XRefDensity struct{}

func (c *checkINV006XRefDensity) ID() int                { return 3 }
func (c *checkINV006XRefDensity) Name() string           { return "INV-006: Cross-reference density" }
func (c *checkINV006XRefDensity) Applicable(string) bool { return true }

func (c *checkINV006XRefDensity) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	sections, err := storage.ListSections(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	refCounts, err := storage.GetSectionRefCounts(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	orphans := 0
	for _, sec := range sections {
		// Skip top-level structural sections that are naturally orphaned
		if isExemptSection(sec.SectionPath) {
			continue
		}
		// Skip level 1 headings (PARTs)
		if sec.HeadingLevel <= 1 {
			continue
		}

		rc := refCounts[sec.ID]
		if rc.Incoming == 0 && rc.Outgoing == 0 {
			orphans++
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityWarning,
				Message:     fmt.Sprintf("orphan section: %s (%s) — 0 incoming, 0 outgoing refs", sec.SectionPath, sec.Title),
				Location:    sec.SectionPath,
				InvariantID: "INV-006",
			})
		}
	}

	if orphans > 0 {
		result.Summary = fmt.Sprintf("%d orphan sections out of %d total", orphans, len(sections))
	} else {
		result.Summary = fmt.Sprintf("all %d sections have cross-references", len(sections))
	}
	return result
}

func isExemptSection(path string) bool {
	exempts := []string{"PART-0", "Glossary", "Appendix-A", "Appendix-B", "Appendix-C", "Preamble"}
	for _, e := range exempts {
		if path == e || strings.HasPrefix(path, e+"/") {
			return true
		}
	}
	return false
}

// Check 4: INV-009 — Glossary completeness.
type checkINV009GlossaryCompleteness struct{}

func (c *checkINV009GlossaryCompleteness) ID() int                { return 4 }
func (c *checkINV009GlossaryCompleteness) Name() string           { return "INV-009: Glossary completeness" }
func (c *checkINV009GlossaryCompleteness) Applicable(string) bool { return true }

func (c *checkINV009GlossaryCompleteness) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	glossaryTerms, err := storage.GetGlossaryTerms(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	// Scan all section text for bold terms (**Term**) and count occurrences
	sections, err := storage.ListSections(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	boldTermRe := regexp.MustCompile(`\*\*([A-Z][A-Za-z\s-]{2,40})\*\*`)
	termCounts := make(map[string]int)

	for _, sec := range sections {
		matches := boldTermRe.FindAllStringSubmatch(sec.RawText, -1)
		for _, m := range matches {
			term := strings.TrimSpace(m[1])
			termCounts[term]++
		}
	}

	// Flag terms appearing >= 3 times that aren't in the glossary
	missing := 0
	for term, count := range termCounts {
		if count >= 3 && !glossaryTerms[term] {
			missing++
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityWarning,
				Message:     fmt.Sprintf("bold term %q appears %d times but is not in glossary", term, count),
				InvariantID: "INV-009",
			})
		}
	}

	result.Summary = fmt.Sprintf("%d glossary terms, %d frequent bold terms missing", len(glossaryTerms), missing)
	return result
}

// Check 5: INV-013 — Invariant ownership (modular only).
type checkINV013InvariantOwnership struct{}

func (c *checkINV013InvariantOwnership) ID() int      { return 5 }
func (c *checkINV013InvariantOwnership) Name() string { return "INV-013: Invariant ownership" }
func (c *checkINV013InvariantOwnership) Applicable(sourceType string) bool {
	return sourceType == "modular"
}

func (c *checkINV013InvariantOwnership) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	rels, err := storage.GetModuleRelationships(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	// Count "maintains" relationships per target invariant
	maintainsCounts := make(map[string]int)
	for _, rel := range rels {
		if rel.RelType == "maintains" {
			maintainsCounts[rel.Target]++
		}
	}

	multiOwned := 0
	for target, count := range maintainsCounts {
		if count > 1 {
			multiOwned++
			result.Passed = false
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
				Message:     fmt.Sprintf("%s has %d owners (expected exactly 1)", target, count),
				InvariantID: "INV-013",
			})
		}
	}

	// Also check for invariants with no owner
	invs, err := storage.ListInvariants(db, specID)
	if err == nil {
		for _, inv := range invs {
			if _, ok := maintainsCounts[inv.InvariantID]; !ok {
				result.Findings = append(result.Findings, Finding{
					CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityWarning,
					Message:     fmt.Sprintf("%s has no module owner", inv.InvariantID),
					InvariantID: "INV-013",
				})
			}
		}
	}

	result.Summary = fmt.Sprintf("%d invariants with multiple owners", multiOwned)
	return result
}

// Check 6: INV-014 — Bundle budget (modular only).
type checkINV014BundleBudget struct{}

func (c *checkINV014BundleBudget) ID() int      { return 6 }
func (c *checkINV014BundleBudget) Name() string { return "INV-014: Bundle budget" }
func (c *checkINV014BundleBudget) Applicable(sourceType string) bool {
	return sourceType == "modular"
}

func (c *checkINV014BundleBudget) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	manifest, err := storage.GetManifest(db, specID)
	if err != nil || manifest == nil {
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityInfo,
			Message: "no manifest found, cannot check bundle budget",
		})
		result.Summary = "skipped (no manifest)"
		return result
	}

	ceiling := manifest.HardCeilingLines
	if ceiling <= 0 {
		result.Summary = "no hard ceiling defined"
		return result
	}

	// Check each source file
	sourceFiles, err := storage.GetSourceFiles(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	overBudget := 0
	for _, sf := range sourceFiles {
		if sf.FileRole == "manifest" {
			continue
		}
		if sf.LineCount > ceiling {
			overBudget++
			result.Passed = false
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
				Message:     fmt.Sprintf("%s has %d lines (ceiling: %d)", sf.FilePath, sf.LineCount, ceiling),
				Location:    sf.FilePath,
				InvariantID: "INV-014",
			})
		}
	}

	result.Summary = fmt.Sprintf("%d files over budget (ceiling: %d lines)", overBudget, ceiling)
	return result
}

// Check 7: INV-015 — Declaration-definition consistency (modular only).
type checkINV015DeclDef struct{}

func (c *checkINV015DeclDef) ID() int      { return 7 }
func (c *checkINV015DeclDef) Name() string { return "INV-015: Declaration-definition consistency" }
func (c *checkINV015DeclDef) Applicable(sourceType string) bool {
	return sourceType == "modular"
}

func (c *checkINV015DeclDef) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	registryEntries, err := storage.GetInvariantRegistryEntries(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	// Build sets
	registryIDs := make(map[string]bool)
	for _, e := range registryEntries {
		registryIDs[e.InvariantID] = true
	}
	definedIDs := make(map[string]bool)
	for _, inv := range invs {
		definedIDs[inv.InvariantID] = true
	}

	// Check both directions
	mismatch := 0

	// In registry but not defined
	for id := range registryIDs {
		if !definedIDs[id] {
			mismatch++
			result.Passed = false
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
				Message:     fmt.Sprintf("%s declared in registry but not defined in spec", id),
				InvariantID: "INV-015",
			})
		}
	}

	// Defined but not in registry
	for id := range definedIDs {
		if !registryIDs[id] {
			mismatch++
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityWarning,
				Message:     fmt.Sprintf("%s defined in spec but not declared in registry", id),
				InvariantID: "INV-015",
			})
		}
	}

	result.Summary = fmt.Sprintf("%d registry entries, %d definitions, %d mismatches", len(registryIDs), len(definedIDs), mismatch)
	return result
}

// Check 8: INV-016 — Manifest-spec sync (modular only).
type checkINV016ManifestSync struct{}

func (c *checkINV016ManifestSync) ID() int      { return 8 }
func (c *checkINV016ManifestSync) Name() string { return "INV-016: Manifest-spec sync" }
func (c *checkINV016ManifestSync) Applicable(sourceType string) bool {
	return sourceType == "modular"
}

func (c *checkINV016ManifestSync) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	sourceFiles, err := storage.GetSourceFiles(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	modules, err := storage.ListModules(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	// Build sets: source files with role='module' and modules table
	sfModules := make(map[string]bool)
	for _, sf := range sourceFiles {
		if sf.FileRole == "module" && sf.ModuleName != "" {
			sfModules[sf.ModuleName] = true
		}
	}

	modNames := make(map[string]bool)
	for _, m := range modules {
		modNames[m.ModuleName] = true
	}

	mismatch := 0

	// Source file modules not in modules table
	for name := range sfModules {
		if !modNames[name] {
			mismatch++
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityWarning,
				Message:     fmt.Sprintf("source file module %q not in modules table", name),
				InvariantID: "INV-016",
			})
		}
	}

	// Modules not backed by source files
	for name := range modNames {
		if !sfModules[name] {
			mismatch++
			result.Passed = false
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
				Message:     fmt.Sprintf("module %q not backed by a source file", name),
				InvariantID: "INV-016",
			})
		}
	}

	result.Summary = fmt.Sprintf("%d source modules, %d modules, %d mismatches", len(sfModules), len(modNames), mismatch)
	return result
}

// Check 9: INV-017 — Negative spec coverage (>= 3 per implementation chapter).
type checkINV017NegSpecCoverage struct{}

func (c *checkINV017NegSpecCoverage) ID() int                { return 9 }
func (c *checkINV017NegSpecCoverage) Name() string           { return "INV-017: Negative spec coverage" }
func (c *checkINV017NegSpecCoverage) Applicable(string) bool { return true }

func (c *checkINV017NegSpecCoverage) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	negCounts, err := storage.GetNegativeSpecCountBySection(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	sections, err := storage.ListSections(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	// Build a map of section ID → section for lookup
	sectionMap := make(map[int64]*storage.Section)
	for i := range sections {
		sectionMap[sections[i].ID] = &sections[i]
	}

	// Accumulate neg spec counts per top-level implementation chapter (§N where N >= 1)
	chapterCounts := make(map[string]int)
	chapterTitles := make(map[string]string)

	for secID, count := range negCounts {
		sec, ok := sectionMap[secID]
		if !ok {
			continue
		}
		// Find the top-level chapter for this section
		chapterPath := findChapterPath(sec.SectionPath)
		if chapterPath == "" {
			continue
		}
		chapterCounts[chapterPath] += count
		if _, exists := chapterTitles[chapterPath]; !exists {
			for _, s := range sections {
				if s.SectionPath == chapterPath {
					chapterTitles[chapterPath] = s.Title
					break
				}
			}
		}
	}

	// Check implementation chapters (skip §0.x which are meta/preamble)
	underCovered := 0
	for path, count := range chapterCounts {
		if count < 3 {
			underCovered++
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
				Message:     fmt.Sprintf("%s (%s) has %d negative specs (need >= 3)", path, chapterTitles[path], count),
				Location:    path,
				InvariantID: "INV-017",
			})
		}
	}

	if underCovered > 0 {
		result.Passed = false
	}

	result.Summary = fmt.Sprintf("%d chapters checked, %d under-covered", len(chapterCounts), underCovered)
	return result
}

// findChapterPath extracts the top-level chapter from a section path.
// E.g. "§4.2" → "§4", "§4.2.1" → "§4", "PART-2" → "PART-2", "§0.5" → "" (skip preamble)
func findChapterPath(path string) string {
	if strings.HasPrefix(path, "§") {
		parts := strings.SplitN(path[len("§"):], ".", 2)
		if len(parts) == 0 {
			return ""
		}
		// Skip preamble §0.x
		if parts[0] == "0" {
			return ""
		}
		return "§" + parts[0]
	}
	if strings.HasPrefix(path, "Chapter-") {
		parts := strings.SplitN(path, "/", 2)
		return parts[0]
	}
	return ""
}

// Check 10: Gate-1 Structural conformance — required elements exist.
type checkGate1Structural struct{}

func (c *checkGate1Structural) ID() int                { return 10 }
func (c *checkGate1Structural) Name() string           { return "Gate-1: Structural conformance" }
func (c *checkGate1Structural) Applicable(string) bool { return true }

func (c *checkGate1Structural) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	// Check required sections exist
	requiredSections := []string{"§0.1", "§0.5", "§0.6", "§0.7"}
	for _, path := range requiredSections {
		_, err := storage.GetSection(db, specID, path)
		if err != nil {
			result.Passed = false
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
				Message:  fmt.Sprintf("required section %s not found", path),
				Location: path,
			})
		}
	}

	// Check element tables are non-empty
	counts, err := storage.CountElements(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	requiredElements := []struct {
		key   string
		label string
	}{
		{"invariants", "Invariants"},
		{"adrs", "ADRs"},
		{"quality_gates", "Quality Gates"},
		{"negative_specs", "Negative Specs"},
		{"glossary_entries", "Glossary Entries"},
		{"cross_references", "Cross-References"},
	}

	for _, req := range requiredElements {
		if counts[req.key] == 0 {
			result.Passed = false
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
				Message: fmt.Sprintf("no %s found", req.label),
			})
		}
	}

	result.Summary = fmt.Sprintf("checked %d required sections and %d element types", len(requiredSections), len(requiredElements))
	return result
}

// Check 11: Proportional weight — implementation chapter sizes.
type checkProportionalWeight struct{}

func (c *checkProportionalWeight) ID() int                { return 11 }
func (c *checkProportionalWeight) Name() string           { return "Proportional weight" }
func (c *checkProportionalWeight) Applicable(string) bool { return true }

func (c *checkProportionalWeight) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	sections, err := storage.ListSections(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	// Find implementation chapters (top-level §N where N >= 1, or Chapter-N)
	type chapter struct {
		path  string
		title string
		lines int
	}
	var chapters []chapter

	for _, sec := range sections {
		isImplChapter := false
		if strings.HasPrefix(sec.SectionPath, "§") && sec.HeadingLevel <= 2 {
			num := strings.TrimPrefix(sec.SectionPath, "§")
			if !strings.Contains(num, ".") && num != "0" {
				isImplChapter = true
			}
		} else if strings.HasPrefix(sec.SectionPath, "Chapter-") && !strings.Contains(sec.SectionPath, "/") {
			isImplChapter = true
		}

		if isImplChapter {
			lineCount := sec.LineEnd - sec.LineStart
			if lineCount > 0 {
				chapters = append(chapters, chapter{sec.SectionPath, sec.Title, lineCount})
			}
		}
	}

	if len(chapters) < 2 {
		result.Summary = "fewer than 2 implementation chapters found"
		return result
	}

	// Compute mean and check for >20% deviation
	total := 0
	for _, ch := range chapters {
		total += ch.lines
	}
	mean := float64(total) / float64(len(chapters))

	deviations := 0
	for _, ch := range chapters {
		deviation := float64(ch.lines)/mean - 1.0
		if deviation > 0.20 || deviation < -0.20 {
			deviations++
			sev := SeverityWarning
			if deviation > 0.50 || deviation < -0.50 {
				sev = SeverityError
				result.Passed = false
			}
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: sev,
				Message:  fmt.Sprintf("%s (%s): %d lines (%.0f%% deviation from mean %.0f)", ch.path, ch.title, ch.lines, deviation*100, mean),
				Location: ch.path,
			})
		}
	}

	result.Summary = fmt.Sprintf("%d chapters, mean %.0f lines, %d with >20%% deviation", len(chapters), mean, deviations)
	return result
}

// Check 12: Namespace consistency — counts match notes.
type checkNamespaceConsistency struct{}

func (c *checkNamespaceConsistency) ID() int                { return 12 }
func (c *checkNamespaceConsistency) Name() string           { return "Namespace consistency" }
func (c *checkNamespaceConsistency) Applicable(string) bool { return true }

func (c *checkNamespaceConsistency) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	// Get actual counts
	invs, err := storage.ListInvariants(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	adrs, err := storage.ListADRs(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	gates, err := storage.ListQualityGates(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	// Scan section text for "INV-NNN through INV-NNN" or "INV-001 through INV-020" patterns
	sections, err := storage.ListSections(db, specID)
	if err != nil {
		result.Passed = false
		result.Findings = append(result.Findings, Finding{
			CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityError,
			Message: fmt.Sprintf("query error: %v", err),
		})
		return result
	}

	rangeRe := regexp.MustCompile(`(INV|ADR|Gate)-(\d{1,3})\s+through\s+(?:INV|ADR|Gate)-(\d{1,3})`)

	mismatches := 0
	for _, sec := range sections {
		matches := rangeRe.FindAllStringSubmatch(sec.RawText, -1)
		for _, m := range matches {
			prefix := m[1]
			// Parse range bounds
			lo := parseInt(m[2])
			hi := parseInt(m[3])
			if lo <= 0 || hi <= 0 || hi < lo {
				continue
			}

			declared := hi - lo + 1
			var actual int

			switch prefix {
			case "INV":
				actual = countInRange(invs, lo, hi, func(inv storage.Invariant) string { return inv.InvariantID })
			case "ADR":
				actual = countADRsInRange(adrs, lo, hi)
			case "Gate":
				actual = countGatesInRange(gates, lo, hi)
			}

			if actual != declared {
				mismatches++
				result.Findings = append(result.Findings, Finding{
					CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityWarning,
					Message:  fmt.Sprintf("%s-%03d through %s-%03d: declared %d, found %d", prefix, lo, prefix, hi, declared, actual),
					Location: sec.SectionPath,
				})
			}
		}
	}

	result.Summary = fmt.Sprintf("%d INVs, %d ADRs, %d Gates; %d range mismatches", len(invs), len(adrs), len(gates), mismatches)
	return result
}

func parseInt(s string) int {
	n := 0
	for _, c := range s {
		if c >= '0' && c <= '9' {
			n = n*10 + int(c-'0')
		}
	}
	return n
}

var numRe = regexp.MustCompile(`(\d{1,3})$`)

func countInRange(invs []storage.Invariant, lo, hi int, getID func(storage.Invariant) string) int {
	count := 0
	for _, inv := range invs {
		id := getID(inv)
		m := numRe.FindStringSubmatch(id)
		if m == nil {
			continue
		}
		n := parseInt(m[1])
		if n >= lo && n <= hi {
			count++
		}
	}
	return count
}

func countADRsInRange(adrs []storage.ADR, lo, hi int) int {
	count := 0
	for _, a := range adrs {
		m := numRe.FindStringSubmatch(a.ADRID)
		if m == nil {
			continue
		}
		n := parseInt(m[1])
		if n >= lo && n <= hi {
			count++
		}
	}
	return count
}

// Check 14: Witness freshness — stale witnesses are warning findings.
type checkWitnessFreshness struct{}

func (c *checkWitnessFreshness) ID() int                { return 14 }
func (c *checkWitnessFreshness) Name() string           { return "Witness freshness" }
func (c *checkWitnessFreshness) Applicable(string) bool { return true }

func (c *checkWitnessFreshness) Run(db *sql.DB, specID int64) CheckResult {
	result := CheckResult{CheckID: c.ID(), CheckName: c.Name(), Passed: true}

	witnesses, err := storage.ListWitnesses(db, specID)
	if err != nil {
		// No witnesses table yet is fine — just skip
		result.Summary = "no witnesses recorded"
		return result
	}

	stale := 0
	valid := 0
	for _, w := range witnesses {
		if w.Status == "valid" {
			valid++
		} else {
			stale++
			result.Findings = append(result.Findings, Finding{
				CheckID: c.ID(), CheckName: c.Name(), Severity: SeverityWarning,
				Message:     fmt.Sprintf("witness %s is %s (proven by %s at %s)", w.InvariantID, w.Status, w.ProvenBy, w.ProvenAt),
				InvariantID: w.InvariantID,
			})
		}
	}

	if stale > 0 {
		result.Summary = fmt.Sprintf("%d stale witness(es) out of %d total", stale, len(witnesses))
	} else {
		result.Summary = fmt.Sprintf("%d witness(es), all valid", valid)
	}
	return result
}

func countGatesInRange(gates []storage.QualityGate, lo, hi int) int {
	count := 0
	for _, g := range gates {
		m := numRe.FindStringSubmatch(g.GateID)
		if m == nil {
			continue
		}
		n := parseInt(m[1])
		if n >= lo && n <= hi {
			count++
		}
	}
	return count
}
