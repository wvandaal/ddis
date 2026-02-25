# SPIKE: Intent Drift Semantics — Mechanical Definition of "Non-Negotiable Coverage"

> Status: RESOLVED
> Task: #4 (ddis-vls)
> Date: 2026-02-24

## 1. What Are Non-Negotiables?

Non-negotiables are the 7 engineering contract terms in **§0.1.2** of the system
constitution. They are NOT invariants — they are higher-level properties that the
spec MUST have. Invariants are the mechanism that enforces them.

The canonical list (from `constitution/system.md` lines 69-88):

| ID  | Title                                  | Key Text                                                        |
|-----|----------------------------------------|-----------------------------------------------------------------|
| NN1 | Causal chain is unbroken               | Every implementation detail traces back through a decision...   |
| NN2 | Decisions are explicit and locked       | Every design choice captured in an ADR with alternatives        |
| NN3 | Invariants are falsifiable              | Every invariant can be violated and detected by a test          |
| NN4 | No implementation detail is unsupported | pseudocode, complexity, worked example, test strategy           |
| NN5 | Cross-references form a web, not a list | ADRs ref invariants, invariants ref tests, etc.                 |
| NN6 | The document is self-contained          | Implementer with spec alone can build correct v1                |
| NN7 | Negative specifications prevent halluc. | Every implementation chapter states what subsystem must NOT do  |

## 2. How to Enumerate Non-Negotiables from the DB

Non-negotiables are not a first-class DB entity. They live in the raw_text of
section §0.1.2. Two strategies:

### Strategy A: Parse from section raw_text (RECOMMENDED for v1)

```go
// GetSection(db, specID, "§0.1.2") returns the section with raw_text
// containing the 7 bullet items. Parse with regex:
//   - **<Title>**\n  <Description>
// This works for DDIS meta-spec and any spec following the convention.

func ParseNonNegotiables(sectionText string) []NonNegotiable {
    // regex: `- \*\*(.+?)\*\*\n\s+(.+?)(?:\n\n|\z)`
    // Returns: [{Title: "Causal chain is unbroken", Description: "Every impl..."}]
}
```

Fallback: If §0.1.2 doesn't exist (monolith specs may use §0.11), try
`§0.11` as alternate path.

### Strategy B: Hard-coded canonical mapping (FALLBACK)

If parsing fails (no §0.1.2 section found, or section has unexpected format),
use a built-in table of the 7 DDIS non-negotiables. This is acceptable because
the non-negotiables are part of the DDIS standard itself — they don't change
per-spec.

## 3. How to Determine if a Non-Negotiable is "Covered"

A non-negotiable is **covered** when:
1. At least one invariant addresses it (via the mapping below), AND
2. That invariant has a non-empty `validation_method`, AND
3. That invariant's component completeness ≥ 0.5 (via `exemplar.WeakScore`)

### Non-Negotiable → Invariant Mapping

This mapping is derived from the spec's own cross-reference structure and the
invariant registry descriptions:

| NN  | Primary Invariants      | Validate Check | Matching Strategy                          |
|-----|-------------------------|----------------|--------------------------------------------|
| NN1 | INV-001                 | Check 13       | Registry desc contains "Causal Traceab"    |
| NN2 | INV-002                 | (partial Ch2)  | Registry desc contains "Decision Complete"  |
| NN3 | INV-003                 | Check 2        | Registry desc contains "Falsifiab"          |
| NN4 | INV-004, INV-005        | Check 10       | Registry desc contains "Algorithm" or "Performance Verif" |
| NN5 | INV-006                 | Check 1, 3     | Registry desc contains "Cross-Reference"    |
| NN6 | INV-008, INV-009        | Check 4, 10    | Registry desc contains "Self-Contain" or "Glossary" |
| NN7 | INV-017                 | Check 9        | Registry desc contains "Negative Spec"      |

### Mechanical Algorithm

```go
// IntentCoverage computes which non-negotiables are covered.
func IntentCoverage(db *sql.DB, specID int64) (*IntentResult, error) {
    // 1. Try to parse non-negotiables from §0.1.2 section
    nns := ParseNonNegotiablesFromDB(db, specID)
    if len(nns) == 0 {
        nns = CanonicalNonNegotiables() // fallback
    }

    // 2. Get invariant registry + all invariants
    registry, _ := storage.GetInvariantRegistryEntries(db, specID)
    invs, _ := storage.ListInvariants(db, specID)

    // 3. Build invariant completeness map (reuse coverage logic)
    invCompleteness := make(map[string]float64)
    for _, inv := range invs {
        fields := exemplar.ExtractInvariantFields(inv)
        components := exemplar.ComponentsForType("invariant")
        present := 0
        for _, comp := range components {
            if val := fields[comp]; val != "" {
                if exemplar.WeakScore(val, comp, "invariant") > 0 {
                    present++
                }
            }
        }
        if len(components) > 0 {
            invCompleteness[inv.InvariantID] = float64(present) / float64(len(components))
        }
    }

    // 4. Build inv lookup by ID for validation_method check
    invByID := make(map[string]storage.Invariant)
    for _, inv := range invs {
        invByID[inv.InvariantID] = inv
    }

    // 5. For each non-negotiable, find matching invariants
    result := &IntentResult{
        NonNegotiables: make([]NonNegotiableCoverage, 0, len(nns)),
    }

    for _, nn := range nns {
        nnc := NonNegotiableCoverage{
            Title:       nn.Title,
            Description: nn.Description,
        }

        // Match invariants: use keyword matching against registry descriptions
        for _, reg := range registry {
            if matchesNonNegotiable(nn, reg) {
                inv := invByID[reg.InvariantID]
                hasValidation := inv.ValidationMethod != ""
                completeness := invCompleteness[reg.InvariantID]
                covered := hasValidation && completeness >= 0.5

                nnc.MaintainingInvariants = append(nnc.MaintainingInvariants, InvariantMatch{
                    InvariantID:      reg.InvariantID,
                    Description:      reg.Description,
                    HasValidation:    hasValidation,
                    Completeness:     completeness,
                    MeetsCoverage:    covered,
                })

                if covered {
                    nnc.Covered = true
                }
            }
        }

        result.NonNegotiables = append(result.NonNegotiables, nnc)
    }

    // 6. Compute summary
    covered := 0
    for _, nnc := range result.NonNegotiables {
        if nnc.Covered {
            covered++
        }
    }
    result.CoveredCount = covered
    result.TotalCount = len(nns)
    if len(nns) > 0 {
        result.Score = float64(covered) / float64(len(nns))
    }

    return result, nil
}
```

### matchesNonNegotiable — Keyword Matching

```go
// nonNegotiableKeywords maps NN titles to keywords found in invariant
// registry descriptions. This is the "glue" between the high-level
// contract and the mechanical invariant system.
var nonNegotiableKeywords = map[string][]string{
    "Causal chain is unbroken":               {"causal", "traceab"},
    "Decisions are explicit and locked":       {"decision", "completeness"},
    "Invariants are falsifiable":              {"falsifiab"},
    "No implementation detail is unsupported": {"algorithm", "performance verif"},
    "Cross-references form a web":             {"cross-reference", "density"},
    "The document is self-contained":          {"self-contain", "glossary"},
    "Negative specifications prevent":         {"negative spec"},
}

func matchesNonNegotiable(nn NonNegotiable, reg storage.InvariantRegistryEntry) bool {
    keywords := nonNegotiableKeywords[nn.Title]
    descLower := strings.ToLower(reg.Description)
    for _, kw := range keywords {
        if strings.Contains(descLower, kw) {
            return true
        }
    }
    return false
}
```

## 4. Purposeless Elements (Orphan Detection)

An element is "purposeless" if it exists but serves no non-negotiable or stated
goal. Mechanically:

### Definition

An element is **purposeless** if:
- It is a section, invariant, or ADR, AND
- It has **zero incoming cross-references** from the constitution (source_file
  with file_role = 'system_constitution'), AND
- It is NOT in the invariant_registry (for invariants), AND
- It is NOT referenced by any module_relationship

### Algorithm

```go
func FindPurposelessElements(db *sql.DB, specID int64) ([]PurposelessElement, error) {
    // 1. Get all sections
    sections, _ := storage.ListSections(db, specID)

    // 2. Get ref counts per section
    refCounts, _ := storage.GetSectionRefCounts(db, specID)

    // 3. Get constitution source file IDs
    files, _ := storage.GetSourceFiles(db, specID)
    constitutionFileIDs := make(map[int64]bool)
    for _, f := range files {
        if f.FileRole == "system_constitution" || f.FileRole == "domain_constitution" {
            constitutionFileIDs[f.ID] = true
        }
    }

    // 4. Get all cross-refs originating FROM constitution files
    // (need a new query or filter existing ones)
    // For each element, check if any cross-ref targets it from a constitution file

    // 5. Invariants in registry are NOT purposeless (they have explicit ownership)
    registry, _ := storage.GetInvariantRegistryEntries(db, specID)
    registeredInvs := make(map[string]bool)
    for _, r := range registry {
        registeredInvs[r.InvariantID] = true
    }

    // 6. Module relationships provide purpose
    rels, _ := storage.GetModuleRelationships(db, specID)
    relTargets := make(map[string]bool)
    for _, r := range rels {
        relTargets[r.Target] = true
    }

    var purposeless []PurposelessElement
    for _, sec := range sections {
        rc := refCounts[sec.ID]
        if rc.Incoming == 0 && !constitutionFileIDs[sec.SourceFileID] {
            // Section in a module with zero incoming refs = potentially purposeless
            purposeless = append(purposeless, PurposelessElement{
                Type:       "section",
                ID:         sec.SectionPath,
                Title:      sec.Title,
                IncomingRefs: 0,
            })
        }
    }

    // Check invariants
    invs, _ := storage.ListInvariants(db, specID)
    for _, inv := range invs {
        if !registeredInvs[inv.InvariantID] && !relTargets[inv.InvariantID] {
            backlinks, _ := storage.GetBacklinks(db, specID, inv.InvariantID)
            if len(backlinks) == 0 {
                purposeless = append(purposeless, PurposelessElement{
                    Type:       "invariant",
                    ID:         inv.InvariantID,
                    Title:      inv.Title,
                    IncomingRefs: 0,
                })
            }
        }
    }

    return purposeless, nil
}
```

## 5. Data Types

```go
type NonNegotiable struct {
    Title       string
    Description string
}

type NonNegotiableCoverage struct {
    Title                 string            `json:"title"`
    Description           string            `json:"description"`
    Covered               bool              `json:"covered"`
    MaintainingInvariants []InvariantMatch  `json:"maintaining_invariants"`
}

type InvariantMatch struct {
    InvariantID   string  `json:"invariant_id"`
    Description   string  `json:"description"`
    HasValidation bool    `json:"has_validation"`
    Completeness  float64 `json:"completeness"`
    MeetsCoverage bool    `json:"meets_coverage"`
}

type IntentResult struct {
    NonNegotiables []NonNegotiableCoverage `json:"non_negotiables"`
    CoveredCount   int                     `json:"covered_count"`
    TotalCount     int                     `json:"total_count"`
    Score          float64                 `json:"score"`   // 0.0-1.0
    Purposeless    []PurposelessElement    `json:"purposeless,omitempty"`
}

type PurposelessElement struct {
    Type         string `json:"type"`     // "section", "invariant", "adr"
    ID           string `json:"id"`       // §X.Y, INV-NNN, ADR-NNN
    Title        string `json:"title"`
    IncomingRefs int    `json:"incoming_refs"`
}
```

## 6. Integration with Drift Command

The intent drift score feeds into the overall drift analysis:

```go
// In drift.Analyze():
if opts.Intent {
    intentResult, err := IntentCoverage(db, specID)
    if err != nil {
        return nil, fmt.Errorf("intent coverage: %w", err)
    }
    result.IntentDrift = 1.0 - intentResult.Score  // invert: 0 = no drift
    result.IntentDetail = intentResult
}
```

The `--intent` flag on the CLI enables this analysis. Without it,
`intent_drift` defaults to 0.0 (assumes full coverage).

## 7. Validate Check Reuse

Several existing validate checks already measure non-negotiable compliance.
Intent drift should REUSE these rather than duplicate:

| NN  | Validate Check | What it measures                              |
|-----|----------------|-----------------------------------------------|
| NN1 | Check 13       | Implementation traceability to INV/ADR/const  |
| NN3 | Check 2        | INV-003: 4 core components present per inv    |
| NN5 | Check 1, 3     | Xref resolution + density (no orphan sections)|
| NN6 | Check 4, 10    | Glossary completeness + structural conformance|
| NN7 | Check 9        | Negative spec count per impl chapter          |

For v2, intent drift could incorporate validate check results directly:
- Run the relevant checks
- If check passes → non-negotiable is "mechanically enforced"
- If check fails → non-negotiable has enforcement gaps

## 8. v1 Fallback

For the initial `drift` command (task #5), intent drift = 0.0 when `--intent`
is not specified. The v1 fallback is:

```go
intent_drift = count(non_negotiables with zero maintaining invariants) / total_non_negotiables
```

This is purely structural — it counts whether invariants exist for each
non-negotiable, not whether they pass validation. Good enough for v1;
the validate-check integration comes in v2.

## 9. New Storage Query Needed

One query is not currently available and would help:

```go
// ListCrossReferencesFromFile returns all cross-refs originating from
// a specific source file. Useful for finding what the constitution
// references vs what modules reference.
func ListCrossReferencesFromFile(db *sql.DB, specID, sourceFileID int64) ([]CrossReference, error) {
    rows, err := db.Query(
        `SELECT id, spec_id, source_file_id, source_section_id, source_line,
                ref_type, ref_target, ref_text, resolved
         FROM cross_references WHERE spec_id = ? AND source_file_id = ?`, specID, sourceFileID,
    )
    // ...
}
```

This enables the purposeless-element detection to efficiently find what the
constitution actually references.

## 10. Summary of Design Decisions

1. **Parse NN from §0.1.2 raw_text**, fall back to canonical table
2. **Keyword matching** between NN titles and invariant registry descriptions
3. **Coverage threshold**: validation_method present AND completeness ≥ 0.5
4. **Purposeless**: zero incoming refs from constitution AND not in registry
5. **v1 score**: simple ratio of covered non-negotiables
6. **v2 enhancement**: integrate validate check pass/fail status
7. **Integration**: `--intent` flag, defaults to 0.0 when omitted
