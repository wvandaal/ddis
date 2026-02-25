package search

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"regexp"
	"strings"

	"github.com/wvandaal/ddis/internal/impact"
	"github.com/wvandaal/ddis/internal/oplog"
	"github.com/wvandaal/ddis/internal/query"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-005 (context self-containment)

// ContextBundle is the complete pre-flight briefing for a spec element.
type ContextBundle struct {
	Target          string               `json:"target"`
	ElementType     string               `json:"element_type"`
	Title           string               `json:"title"`
	Content         string               `json:"content"`
	LineStart       int                  `json:"line_start"`
	LineEnd         int                  `json:"line_end"`
	Constraints     []Constraint         `json:"constraints"`
	InvCompleteness []InvariantStatus    `json:"invariant_completeness,omitempty"`
	CoverageGaps    []CoverageGap        `json:"coverage_gaps,omitempty"`
	LocalValidation []LocalCheck         `json:"local_validation,omitempty"`
	ReasoningMode   []ReasoningModeItem  `json:"reasoning_mode_related,omitempty"`
	Related         []RelatedElement     `json:"related"`
	ImpactRadius    *impact.ImpactResult `json:"impact_radius"`
	RecentChanges   []ChangeRecord       `json:"recent_changes,omitempty"`
	EditingGuidance []string             `json:"editing_guidance"`
}

// Constraint is an invariant, gate, or negative spec that constrains the target.
type Constraint struct {
	Type        string `json:"type"` // invariant, gate, negative_spec
	ID          string `json:"id"`
	Description string `json:"description"`
}

// InvariantStatus tracks completeness of an invariant's components.
type InvariantStatus struct {
	ID            string   `json:"id"`
	HasStatement  bool     `json:"has_statement"`
	HasSemiFormal bool     `json:"has_semi_formal"`
	HasValidation bool     `json:"has_validation"`
	HasWhyMatters bool     `json:"has_why_matters"`
	HasViolation  bool     `json:"has_violation"`
	Complete      bool     `json:"complete"`
	MissingFields []string `json:"missing_fields,omitempty"`
}

// CoverageGap identifies something that should exist but doesn't.
type CoverageGap struct {
	Description  string `json:"description"`
	InvariantRef string `json:"invariant_ref,omitempty"`
	Severity     string `json:"severity"` // info, warning, error
}

// LocalCheck is a scoped validation result for the target region.
type LocalCheck struct {
	CheckName string `json:"check_name"`
	Passed    bool   `json:"passed"`
	Detail    string `json:"detail"`
}

// ReasoningModeItem tags a related element by its reasoning mode.
type ReasoningModeItem struct {
	Mode        string `json:"mode"` // Formal, Causal, Practical, Meta
	ElementType string `json:"element_type"`
	ElementID   string `json:"element_id"`
	Description string `json:"description"`
}

// RelatedElement is a semantically similar element found via LSI.
type RelatedElement struct {
	ElementType string  `json:"element_type"`
	ElementID   string  `json:"element_id"`
	Title       string  `json:"title"`
	Similarity  float64 `json:"similarity"`
}

// ChangeRecord represents a recent oplog change.
type ChangeRecord struct {
	Timestamp string `json:"timestamp"`
	Type      string `json:"type"`
	Summary   string `json:"summary"`
}

// BuildContext assembles a contextual intelligence bundle for the given target.
func BuildContext(db *sql.DB, specID int64, target string, lsi *LSIIndex, oplogPath string, depth int, relatedLimit int) (*ContextBundle, error) {
	if depth <= 0 {
		depth = 2
	}
	if relatedLimit <= 0 {
		relatedLimit = 5
	}

	// Get content via query package
	frag, err := query.QueryTarget(db, specID, target, query.QueryOptions{
		ResolveRefs: true,
		Backlinks:   true,
	})
	if err != nil {
		return nil, fmt.Errorf("query target: %w", err)
	}

	bundle := &ContextBundle{
		Target:      frag.ID,
		ElementType: string(frag.Type),
		Title:       frag.Title,
		Content:     frag.RawText,
		LineStart:   frag.LineStart,
		LineEnd:     frag.LineEnd,
	}

	// Find constraints (invariants, gates, negative specs connected via backlinks)
	bundle.Constraints = findConstraints(db, specID, frag)

	// Invariant completeness check
	bundle.InvCompleteness = checkInvariantCompleteness(db, specID, frag, bundle.Constraints)

	// Coverage gaps
	bundle.CoverageGaps = findCoverageGaps(db, specID, frag)

	// Local validation (scoped checks)
	bundle.LocalValidation = runLocalValidation(db, specID, frag)

	// Related elements by reasoning mode
	bundle.ReasoningMode = tagReasoningModes(db, specID, frag)

	// LSI-based semantic similarity
	if lsi != nil && lsi.Uk != nil {
		qVec := lsi.QueryVec(frag.Title + " " + frag.RawText)
		if qVec != nil {
			ranked := lsi.RankAll(qVec)
			for _, rd := range ranked {
				if rd.ElementID == frag.ID {
					continue // skip self
				}
				if rd.Similarity < 0.1 {
					continue
				}
				if len(bundle.Related) >= relatedLimit {
					break
				}
				// Look up element info
				eType, eTitle := lookupElement(db, specID, rd.ElementID)
				bundle.Related = append(bundle.Related, RelatedElement{
					ElementType: eType,
					ElementID:   rd.ElementID,
					Title:       eTitle,
					Similarity:  rd.Similarity,
				})
			}
		}
	}

	// Impact analysis
	impactResult, err := impact.Analyze(db, specID, target, impact.ImpactOptions{
		Direction: "both",
		MaxDepth:  depth,
	})
	if err == nil {
		bundle.ImpactRadius = impactResult
	}

	// Recent changes from oplog
	if oplogPath != "" {
		bundle.RecentChanges = getRecentChanges(oplogPath, frag.ID)
	}

	// Generate editing guidance
	bundle.EditingGuidance = generateGuidance(bundle)

	return bundle, nil
}

// findConstraints discovers invariants, gates, and negative specs constraining the target.
func findConstraints(db *sql.DB, specID int64, frag *query.Fragment) []Constraint {
	var constraints []Constraint

	// Find invariants mentioned in backlinks or whose section contains the target
	contentLower := strings.ToLower(frag.RawText)

	// Check all invariants for relevance
	invs, _ := storage.ListInvariants(db, specID)
	for _, inv := range invs {
		// Invariant is a constraint if:
		// 1. It's referenced from the target's backlinks
		// 2. The target's text mentions the invariant ID
		if strings.Contains(contentLower, strings.ToLower(inv.InvariantID)) {
			constraints = append(constraints, Constraint{
				Type:        "invariant",
				ID:          inv.InvariantID,
				Description: inv.Title + ": " + truncate(inv.Statement, 100),
			})
		}
	}

	// Also check backlinks for invariant references
	for _, bl := range frag.Backlinks {
		if strings.HasPrefix(bl.RefText, "INV-") || strings.HasPrefix(bl.RefText, "APP-INV-") {
			// Check if already added
			found := false
			for _, c := range constraints {
				if c.ID == bl.RefText {
					found = true
					break
				}
			}
			if !found {
				inv, err := storage.GetInvariant(db, specID, bl.RefText)
				if err == nil {
					constraints = append(constraints, Constraint{
						Type:        "invariant",
						ID:          inv.InvariantID,
						Description: inv.Title + ": " + truncate(inv.Statement, 100),
					})
				}
			}
		}
	}

	// Find quality gates mentioning the target
	gates, _ := storage.ListQualityGates(db, specID)
	for _, g := range gates {
		if strings.Contains(contentLower, strings.ToLower(g.GateID)) ||
			strings.Contains(strings.ToLower(g.RawText), strings.ToLower(frag.ID)) {
			constraints = append(constraints, Constraint{
				Type:        "gate",
				ID:          g.GateID,
				Description: g.Title + ": " + truncate(g.Predicate, 100),
			})
		}
	}

	// Find negative specs in the same section
	negSpecs, _ := storage.ListNegativeSpecs(db, specID)
	for _, ns := range negSpecs {
		if ns.LineNumber >= frag.LineStart && ns.LineNumber <= frag.LineEnd {
			invRef := ""
			if ns.InvariantRef != "" {
				invRef = " (→ " + ns.InvariantRef + ")"
			}
			constraints = append(constraints, Constraint{
				Type:        "negative_spec",
				ID:          fmt.Sprintf("DO NOT: %s", truncate(ns.ConstraintText, 60)),
				Description: ns.ConstraintText + invRef,
			})
		}
	}

	return constraints
}

// checkInvariantCompleteness verifies that each constraining invariant has all 4 components.
func checkInvariantCompleteness(db *sql.DB, specID int64, frag *query.Fragment, constraints []Constraint) []InvariantStatus {
	var statuses []InvariantStatus

	for _, c := range constraints {
		if c.Type != "invariant" {
			continue
		}
		inv, err := storage.GetInvariant(db, specID, c.ID)
		if err != nil {
			continue
		}

		status := InvariantStatus{
			ID:            inv.InvariantID,
			HasStatement:  strings.TrimSpace(inv.Statement) != "",
			HasSemiFormal: strings.TrimSpace(inv.SemiFormal) != "",
			HasValidation: strings.TrimSpace(inv.ValidationMethod) != "",
			HasWhyMatters: strings.TrimSpace(inv.WhyThisMatters) != "",
			HasViolation:  strings.TrimSpace(inv.ViolationScenario) != "",
		}
		status.Complete = status.HasStatement && status.HasSemiFormal &&
			status.HasValidation && status.HasWhyMatters

		if !status.HasStatement {
			status.MissingFields = append(status.MissingFields, "statement")
		}
		if !status.HasSemiFormal {
			status.MissingFields = append(status.MissingFields, "semi-formal predicate")
		}
		if !status.HasValidation {
			status.MissingFields = append(status.MissingFields, "validation method")
		}
		if !status.HasWhyMatters {
			status.MissingFields = append(status.MissingFields, "why-this-matters")
		}

		statuses = append(statuses, status)
	}

	return statuses
}

var boldTermRe = regexp.MustCompile(`\*\*([^*]+)\*\*`)

// findCoverageGaps identifies what should exist but doesn't.
func findCoverageGaps(db *sql.DB, specID int64, frag *query.Fragment) []CoverageGap {
	var gaps []CoverageGap

	// Check bold terms vs glossary (INV-009 style)
	boldMatches := boldTermRe.FindAllStringSubmatch(frag.RawText, -1)
	if len(boldMatches) > 0 {
		glossaryTerms, _ := storage.GetGlossaryTerms(db, specID)
		var missing []string
		for _, m := range boldMatches {
			term := strings.TrimSpace(m[1])
			if term == "" || len(term) < 3 {
				continue
			}
			if !glossaryTerms[term] && !glossaryTerms[strings.ToLower(term)] {
				missing = append(missing, term)
			}
		}
		if len(missing) > 0 {
			gaps = append(gaps, CoverageGap{
				Description:  fmt.Sprintf("%s uses %d bold terms not in glossary: %s", frag.ID, len(missing), strings.Join(missing, ", ")),
				InvariantRef: "INV-009",
				Severity:     "warning",
			})
		}
	}

	// Check verification prompt coverage (INV-017 style)
	// Only for chapter-level sections
	if frag.Type == "section" && (strings.HasPrefix(frag.ID, "Chapter-") || strings.HasPrefix(frag.ID, "PART-")) {
		vps, _ := storage.ListVerificationPrompts(db, specID)
		hasPrompt := false
		for _, vp := range vps {
			if vp.LineStart >= frag.LineStart && vp.LineEnd <= frag.LineEnd {
				hasPrompt = true
				break
			}
		}
		if !hasPrompt {
			gaps = append(gaps, CoverageGap{
				Description:  fmt.Sprintf("%s has 0 verification prompts", frag.ID),
				InvariantRef: "INV-017",
				Severity:     "info",
			})
		}
	}

	return gaps
}

// runLocalValidation runs scoped validation checks for the target region.
func runLocalValidation(db *sql.DB, specID int64, frag *query.Fragment) []LocalCheck {
	var checks []LocalCheck

	// Check 1: Cross-references in this region all resolve
	refs, _ := db.Query(
		`SELECT ref_target, resolved FROM cross_references
		 WHERE spec_id = ? AND source_line >= ? AND source_line <= ?`,
		specID, frag.LineStart, frag.LineEnd,
	)
	if refs != nil {
		defer refs.Close()
		total, resolved := 0, 0
		for refs.Next() {
			var target string
			var res int
			if err := refs.Scan(&target, &res); err == nil {
				total++
				if res != 0 {
					resolved++
				}
			}
		}
		if total > 0 {
			checks = append(checks, LocalCheck{
				CheckName: "Cross-references",
				Passed:    resolved == total,
				Detail:    fmt.Sprintf("%d/%d resolved", resolved, total),
			})
		}
	}

	// Check 2: Bold terms defined in glossary
	boldMatches := boldTermRe.FindAllStringSubmatch(frag.RawText, -1)
	if len(boldMatches) > 0 {
		glossaryTerms, _ := storage.GetGlossaryTerms(db, specID)
		defined, total := 0, 0
		for _, m := range boldMatches {
			term := strings.TrimSpace(m[1])
			if term == "" || len(term) < 3 {
				continue
			}
			total++
			if glossaryTerms[term] || glossaryTerms[strings.ToLower(term)] {
				defined++
			}
		}
		if total > 0 {
			checks = append(checks, LocalCheck{
				CheckName: "Glossary coverage",
				Passed:    defined == total,
				Detail:    fmt.Sprintf("%d/%d bold terms defined", defined, total),
			})
		}
	}

	return checks
}

// tagReasoningModes maps related elements to their reasoning modes.
func tagReasoningModes(db *sql.DB, specID int64, frag *query.Fragment) []ReasoningModeItem {
	var items []ReasoningModeItem

	// Formal: invariants and state machines in this region
	invs, _ := storage.ListInvariants(db, specID)
	for _, inv := range invs {
		if inv.LineStart >= frag.LineStart && inv.LineEnd <= frag.LineEnd {
			items = append(items, ReasoningModeItem{
				Mode:        "Formal",
				ElementType: "invariant",
				ElementID:   inv.InvariantID,
				Description: inv.Title,
			})
		}
	}

	sms, _ := storage.ListStateMachines(db, specID)
	for _, sm := range sms {
		if sm.LineStart >= frag.LineStart && sm.LineEnd <= frag.LineEnd {
			items = append(items, ReasoningModeItem{
				Mode:        "Formal",
				ElementType: "state_machine",
				ElementID:   fmt.Sprintf("sm:%d", sm.ID),
				Description: sm.Title,
			})
		}
	}

	// Causal: ADRs referenced from this region
	for _, ref := range frag.ResolvedRefs {
		if ref.RefType == "adr" || ref.RefType == "app_adr" {
			adr, err := storage.GetADR(db, specID, ref.Target)
			if err == nil {
				items = append(items, ReasoningModeItem{
					Mode:        "Causal",
					ElementType: "adr",
					ElementID:   adr.ADRID,
					Description: adr.Title,
				})
			}
		}
	}

	// Practical: worked examples in this region
	wes, _ := storage.ListWorkedExamples(db, specID)
	for _, we := range wes {
		if we.LineStart >= frag.LineStart && we.LineEnd <= frag.LineEnd {
			items = append(items, ReasoningModeItem{
				Mode:        "Practical",
				ElementType: "worked_example",
				ElementID:   fmt.Sprintf("we:%d", we.ID),
				Description: we.Title,
			})
		}
	}

	// Meta: negative specs and WHY NOT annotations in this region
	negSpecs, _ := storage.ListNegativeSpecs(db, specID)
	for _, ns := range negSpecs {
		if ns.LineNumber >= frag.LineStart && ns.LineNumber <= frag.LineEnd {
			items = append(items, ReasoningModeItem{
				Mode:        "Meta",
				ElementType: "negative_spec",
				ElementID:   fmt.Sprintf("neg:%d", ns.ID),
				Description: "DO NOT: " + truncate(ns.ConstraintText, 60),
			})
		}
	}

	whyNots, _ := storage.ListWhyNotAnnotations(db, specID)
	for _, wn := range whyNots {
		if wn.LineNumber >= frag.LineStart && wn.LineNumber <= frag.LineEnd {
			items = append(items, ReasoningModeItem{
				Mode:        "Meta",
				ElementType: "why_not",
				ElementID:   fmt.Sprintf("whynot:%d", wn.ID),
				Description: "WHY NOT: " + truncate(wn.Alternative, 60),
			})
		}
	}

	// Meta: comparison blocks in this region
	cbs, _ := storage.ListComparisonBlocks(db, specID)
	for _, cb := range cbs {
		if cb.LineStart >= frag.LineStart && cb.LineEnd <= frag.LineEnd {
			items = append(items, ReasoningModeItem{
				Mode:        "Meta",
				ElementType: "comparison",
				ElementID:   fmt.Sprintf("cmp:%d", cb.ID),
				Description: cb.ChosenApproach + " vs " + cb.SuboptimalApproach,
			})
		}
	}

	return items
}

// lookupElement returns (type, title) for an element ID.
func lookupElement(db *sql.DB, specID int64, elementID string) (string, string) {
	// Try section
	if sec, err := storage.GetSection(db, specID, elementID); err == nil {
		return "section", sec.Title
	}
	// Try invariant
	if inv, err := storage.GetInvariant(db, specID, elementID); err == nil {
		return "invariant", inv.Title
	}
	// Try ADR
	if adr, err := storage.GetADR(db, specID, elementID); err == nil {
		return "adr", adr.Title
	}
	// Try gate
	if gate, err := storage.GetQualityGate(db, specID, elementID); err == nil {
		return "gate", gate.Title
	}
	// Glossary
	if strings.HasPrefix(elementID, "glossary:") {
		return "glossary", strings.TrimPrefix(elementID, "glossary:")
	}
	return "unknown", elementID
}

// getRecentChanges extracts oplog changes relevant to the target.
func getRecentChanges(oplogPath string, targetID string) []ChangeRecord {
	records, err := oplog.ReadFiltered(oplogPath, oplog.FilterOpts{Limit: 50})
	if err != nil || len(records) == 0 {
		return nil
	}

	var changes []ChangeRecord
	targetLower := strings.ToLower(targetID)

	for _, rec := range records {
		summary := ""
		switch rec.Type {
		case oplog.RecordTypeDiff:
			d, err := rec.DecodeDiff()
			if err != nil {
				continue
			}
			for _, ch := range d.Changes {
				if strings.ToLower(ch.ElementID) == targetLower ||
					strings.ToLower(ch.SectionPath) == targetLower {
					summary = fmt.Sprintf("%s %s %s", ch.Action, ch.ElementType, ch.ElementID)
					break
				}
			}
		case oplog.RecordTypeTransaction:
			d, err := rec.DecodeTx()
			if err != nil {
				continue
			}
			if strings.Contains(strings.ToLower(d.Description), targetLower) {
				summary = fmt.Sprintf("%s: %s", d.Action, d.Description)
			}
		}

		if summary != "" {
			txInfo := ""
			if rec.TxID != "" {
				txInfo = " in " + rec.TxID
			}
			changes = append(changes, ChangeRecord{
				Timestamp: rec.Timestamp,
				Type:      string(rec.Type),
				Summary:   summary + txInfo,
			})
		}
	}

	return changes
}

// generateGuidance produces actionable editing guidance from constraints and gaps.
func generateGuidance(bundle *ContextBundle) []string {
	var guidance []string

	// From constraints
	for _, c := range bundle.Constraints {
		switch c.Type {
		case "invariant":
			guidance = append(guidance, fmt.Sprintf("Ensure compliance with %s (%s)", c.ID, truncate(c.Description, 60)))
		case "gate":
			guidance = append(guidance, fmt.Sprintf("Satisfy %s before proceeding", c.ID))
		case "negative_spec":
			guidance = append(guidance, c.Description)
		}
	}

	// From coverage gaps
	for _, gap := range bundle.CoverageGaps {
		guidance = append(guidance, gap.Description)
	}

	// From invariant completeness gaps
	for _, inv := range bundle.InvCompleteness {
		if !inv.Complete {
			guidance = append(guidance, fmt.Sprintf("Add missing %s to %s",
				strings.Join(inv.MissingFields, ", "), inv.ID))
		}
	}

	// Standard guidance
	guidance = append(guidance, "Run `ddis validate` after changes")

	return guidance
}

// RenderContext formats a ContextBundle for output.
func RenderContext(bundle *ContextBundle, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(bundle, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal context: %w", err)
		}
		return string(data), nil
	}

	return renderHumanContext(bundle), nil
}

func renderHumanContext(b *ContextBundle) string {
	var s strings.Builder

	lineCount := b.LineEnd - b.LineStart + 1
	fmt.Fprintf(&s, "CONTEXT BUNDLE: %s — %s\n", b.Target, b.Title)
	s.WriteString("══════════════════════════════════════════\n\n")

	// Content
	fmt.Fprintf(&s, "CONTENT (%d lines)\n", lineCount)
	// Show first 20 lines
	lines := strings.SplitN(b.Content, "\n", 22)
	for i, line := range lines {
		if i >= 20 {
			fmt.Fprintf(&s, "  ... (%d more lines)\n", lineCount-20)
			break
		}
		fmt.Fprintf(&s, "  %s\n", line)
	}
	s.WriteString("\n")

	// Constraints
	if len(b.Constraints) > 0 {
		fmt.Fprintf(&s, "CONSTRAINTS (%d)\n", len(b.Constraints))
		for _, c := range b.Constraints {
			fmt.Fprintf(&s, "  %s: %s\n", c.ID, c.Description)
		}
		s.WriteString("\n")
	}

	// Invariant Completeness
	if len(b.InvCompleteness) > 0 {
		s.WriteString("INVARIANT COMPLETENESS\n")
		for _, inv := range b.InvCompleteness {
			check := func(b bool) string {
				if b {
					return "+"
				}
				return "-"
			}
			status := "COMPLETE"
			if !inv.Complete {
				status = "GAP: " + strings.Join(inv.MissingFields, ", ")
			}
			fmt.Fprintf(&s, "  %s: %s Statement %s Semi-formal %s Validation %s Why-matters  (%s)\n",
				inv.ID, check(inv.HasStatement), check(inv.HasSemiFormal),
				check(inv.HasValidation), check(inv.HasWhyMatters), status)
		}
		s.WriteString("\n")
	}

	// Coverage Gaps
	if len(b.CoverageGaps) > 0 {
		fmt.Fprintf(&s, "COVERAGE GAPS (%d found)\n", len(b.CoverageGaps))
		for _, gap := range b.CoverageGaps {
			fmt.Fprintf(&s, "  %s\n", gap.Description)
		}
		s.WriteString("\n")
	}

	// Local Validation
	if len(b.LocalValidation) > 0 {
		passed, total := 0, len(b.LocalValidation)
		for _, lc := range b.LocalValidation {
			if lc.Passed {
				passed++
			}
		}
		fmt.Fprintf(&s, "LOCAL VALIDATION (scoped to %s)\n", b.Target)
		for _, lc := range b.LocalValidation {
			mark := "+"
			if !lc.Passed {
				mark = "-"
			}
			fmt.Fprintf(&s, "  %s %s: %s\n", mark, lc.CheckName, lc.Detail)
		}
		fmt.Fprintf(&s, "  Score: %d/%d checks passing\n", passed, total)
		s.WriteString("\n")
	}

	// Reasoning Mode Related
	if len(b.ReasoningMode) > 0 {
		s.WriteString("RELATED BY REASONING MODE\n")
		for _, rm := range b.ReasoningMode {
			fmt.Fprintf(&s, "  [%s] %s — %s\n", rm.Mode, rm.ElementID, rm.Description)
		}
		s.WriteString("\n")
	}

	// Related (LSI similarity)
	if len(b.Related) > 0 {
		s.WriteString("RELATED (via LSI cosine similarity)\n")
		for _, r := range b.Related {
			fmt.Fprintf(&s, "  %s — %s (similarity: %.2f)\n", r.ElementID, r.Title, r.Similarity)
		}
		s.WriteString("\n")
	}

	// Impact Radius
	if b.ImpactRadius != nil && b.ImpactRadius.TotalCount > 0 {
		fmt.Fprintf(&s, "IMPACT RADIUS (%d elements affected by changes)\n", b.ImpactRadius.TotalCount)
		for _, n := range b.ImpactRadius.Nodes {
			fmt.Fprintf(&s, "  [d=%d] %s: %s\n", n.Distance, n.ElementID, n.Title)
		}
		s.WriteString("\n")
	}

	// Recent Changes
	if len(b.RecentChanges) > 0 {
		s.WriteString("RECENT CHANGES (from oplog)\n")
		for _, ch := range b.RecentChanges {
			fmt.Fprintf(&s, "  %s: %s\n", ch.Timestamp, ch.Summary)
		}
		s.WriteString("\n")
	}

	// Editing Guidance
	if len(b.EditingGuidance) > 0 {
		s.WriteString("EDITING GUIDANCE\n")
		for _, g := range b.EditingGuidance {
			if strings.HasPrefix(g, "DO NOT") {
				fmt.Fprintf(&s, "  x %s\n", g)
			} else {
				fmt.Fprintf(&s, "  * %s\n", g)
			}
		}
		s.WriteString("\n")
	}

	return s.String()
}

func truncate(s string, maxLen int) string {
	s = strings.ReplaceAll(s, "\n", " ")
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen-3] + "..."
}
