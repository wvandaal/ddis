# SPIKE: COMPARISON_TARGET — How drift Analyze() Compares Spec vs. Implementation

## Summary

This document describes the **mechanical** (no LLM required) comparison logic for the `drift` command. It specifies which `storage.*` functions to call, in what order, and how to compute drift counts across four categories: **unspecified**, **unimplemented**, **contradiction**, and **coherence**.

---

## 1. Architecture Overview

### Input

The drift `Analyze()` function receives:
- `db *sql.DB` — the SQLite index produced by `ddis parse`
- `specID int64` — the spec to analyze
- `opts DriftOptions` — optional filters (domain, module, element type)

### Output

```go
type DriftReport struct {
    SpecName    string        `json:"spec_name"`
    SpecType    string        `json:"spec_type"` // "monolith" | "modular"
    HasParent   bool          `json:"has_parent"`
    Summary     DriftSummary  `json:"summary"`
    Findings    []DriftFinding `json:"findings"`
}

type DriftSummary struct {
    Unspecified   int     `json:"unspecified"`   // exists in impl, not in spec
    Unimplemented int     `json:"unimplemented"` // in spec, no coverage
    Contradictions int    `json:"contradictions"` // spec says X, evidence says Y
    Coherence     int     `json:"coherence"`     // orphan refs, decl-def gaps
    Total         int     `json:"total"`
    DriftScore    float64 `json:"drift_score"`   // 0.0 (no drift) to 1.0 (total drift)
}

type DriftFinding struct {
    Category  string `json:"category"`  // "unspecified"|"unimplemented"|"contradiction"|"coherence"
    Severity  string `json:"severity"`  // "error"|"warning"|"info"
    ElementID string `json:"element_id"`
    Message   string `json:"message"`
    Location  string `json:"location,omitempty"`
    Remedy    string `json:"remedy,omitempty"` // hint for Remediate()
}
```

### Design Principle

Every detection is a **set-difference** or **set-intersection** operation on data already in the SQLite index. No source code scanning, no LLM judgment. The drift command answers: "Does the spec's internal data model have gaps, contradictions, or structural inconsistencies?"

---

## 2. Detection Logic: Unimplemented

**Question**: Which spec-declared elements have no implementation coverage?

### 2a. Invariants with zero coverage (via `coverage.Analyze()` scores)

```go
// Step 1: Get coverage result
covResult, err := coverage.Analyze(db, specID, coverage.Options{})

// Step 2: Find invariants with 0.0 completeness
for invID, ic := range covResult.Invariants {
    if ic.Completeness == 0.0 {
        findings = append(findings, DriftFinding{
            Category:  "unimplemented",
            Severity:  "error",
            ElementID: invID,
            Message:   fmt.Sprintf("invariant %s has 0%% component coverage", invID),
            Remedy:    "add_invariant_components",
        })
        summary.Unimplemented++
    } else if ic.Completeness < 0.5 {
        findings = append(findings, DriftFinding{
            Category:  "unimplemented",
            Severity:  "warning",
            ElementID: invID,
            Message:   fmt.Sprintf("invariant %s has %.0f%% coverage (below 50%%)", invID, ic.Completeness*100),
            Remedy:    "complete_invariant_components",
        })
        summary.Unimplemented++
    }
}
```

**Storage calls**: `ListInvariants()`, `ListADRs()`, `GetInvariantRegistryEntries()` (all via `coverage.Analyze()`)

### 2b. Invariants in registry but not defined

```go
// Step 1: Get registry entries and defined invariants
registryEntries, _ := storage.GetInvariantRegistryEntries(db, specID)
invs, _ := storage.ListInvariants(db, specID)

// Step 2: Build defined set
definedIDs := make(map[string]bool)
for _, inv := range invs {
    definedIDs[inv.InvariantID] = true
}

// Step 3: Find declared-but-undefined
for _, entry := range registryEntries {
    if !definedIDs[entry.InvariantID] {
        findings = append(findings, DriftFinding{
            Category:  "unimplemented",
            Severity:  "error",
            ElementID: entry.InvariantID,
            Message:   fmt.Sprintf("%s declared in registry (owner=%s, domain=%s) but has no definition", entry.InvariantID, entry.Owner, entry.Domain),
            Remedy:    "add_invariant_definition",
        })
    }
}
```

**Note**: This reuses the same logic as validator Check 7 (`checkINV015DeclDef`). We can call it directly or reuse the pattern.

### 2c. Modules with zero maintained invariants

```go
modules, _ := storage.ListModules(db, specID)
rels, _ := storage.GetModuleRelationships(db, specID)

// Build module DB ID → name
moduleByID := make(map[int64]string)
for _, m := range modules {
    moduleByID[m.ID] = m.ModuleName
}

// Count maintains per module
maintainsCount := make(map[string]int)
for _, r := range rels {
    if r.RelType == "maintains" {
        modName := moduleByID[r.ModuleID]
        maintainsCount[modName]++
    }
}

for _, m := range modules {
    if maintainsCount[m.ModuleName] == 0 {
        findings = append(findings, DriftFinding{
            Category:  "unimplemented",
            Severity:  "warning",
            ElementID: m.ModuleName,
            Message:   fmt.Sprintf("module %s maintains 0 invariants (empty module)", m.ModuleName),
            Remedy:    "assign_invariants_to_module",
        })
    }
}
```

---

## 3. Detection Logic: Unspecified

**Question**: What elements exist in implementation artifacts but have no spec declaration?

### 3a. Invariants defined but not in registry

```go
for _, inv := range invs {
    if !registryIDs[inv.InvariantID] {
        findings = append(findings, DriftFinding{
            Category:  "unspecified",
            Severity:  "warning",
            ElementID: inv.InvariantID,
            Message:   fmt.Sprintf("%s defined in spec but not declared in invariant registry", inv.InvariantID),
            Remedy:    "add_to_registry",
        })
    }
}
```

### 3b. Module source files not in modules table (and vice versa)

This reuses Check 8 (`checkINV016ManifestSync`) logic:

```go
sourceFiles, _ := storage.GetSourceFiles(db, specID)
modules, _ := storage.ListModules(db, specID)

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

// Source files claiming to be modules but not in modules table
for name := range sfModules {
    if !modNames[name] {
        findings = append(findings, DriftFinding{
            Category:  "unspecified",
            Severity:  "warning",
            ElementID: name,
            Message:   fmt.Sprintf("source file module %q exists but not registered in modules table", name),
            Remedy:    "register_module",
        })
    }
}
```

### 3c. Cross-spec: Child spec elements not in parent (THE 13→23 problem)

For child specs with `parent_spec_id`, we compare declared elements:

```go
parentSpecID, _ := storage.GetParentSpecID(db, specID)
if parentSpecID != nil {
    // Get parent's element set
    parentInvs, _ := storage.ListInvariants(db, *parentSpecID)
    parentInvIDs := make(map[string]bool)
    for _, inv := range parentInvs {
        parentInvIDs[inv.InvariantID] = true
    }

    // Get parent's section paths
    parentSections, _ := storage.ListSections(db, *parentSpecID)
    parentSectionPaths := make(map[string]bool)
    for _, sec := range parentSections {
        parentSectionPaths[sec.SectionPath] = true
    }

    // Child-only elements (not necessarily drift — child specs extend parents)
    // But child element COUNTS diverging from parent's declarations IS drift
    // Example: parent declares "13 commands" but child spec defines 23
    childInvs, _ := storage.ListInvariants(db, specID)
    for _, inv := range childInvs {
        // APP-prefixed invariants are child-specific, not drift
        if strings.HasPrefix(inv.InvariantID, "APP-") {
            continue
        }
        if !parentInvIDs[inv.InvariantID] {
            findings = append(findings, DriftFinding{
                Category:  "unspecified",
                Severity:  "info",
                ElementID: inv.InvariantID,
                Message:   fmt.Sprintf("child spec defines %s which is not in parent spec", inv.InvariantID),
                Remedy:    "add_to_parent_or_mark_app_specific",
            })
        }
    }
}
```

**Key insight**: The 13→23 command divergence is detected NOT by comparing code to spec, but by comparing spec element counts to spec declarations. If the spec's registry/manifest says "13 commands" but the spec itself defines sections/invariants for 23, that's a spec-internal inconsistency.

### 3d. Sections with declared counts vs actual counts

This reuses Check 12 (`checkNamespaceConsistency`) logic — range claims like "INV-001 through INV-020" vs actual defined invariants.

```go
// Delegate to validator Check 12 result
check12 := validator.AllChecks()[11] // checkNamespaceConsistency
result := check12.Run(db, specID)
for _, f := range result.Findings {
    findings = append(findings, DriftFinding{
        Category:  "unspecified",
        Severity:  string(f.Severity),
        ElementID: f.InvariantID,
        Message:   f.Message,
        Location:  f.Location,
        Remedy:    "update_count_declarations",
    })
}
```

---

## 4. Detection Logic: Contradictions

**Question**: Where does the spec say one thing but evidence points to another?

### 4a. ADRs marked "active" that reference superseded patterns

```go
adrs, _ := storage.ListADRs(db, specID)
for _, adr := range adrs {
    if adr.Status == "superseded" && adr.SupersededBy == "" {
        findings = append(findings, DriftFinding{
            Category:  "contradiction",
            Severity:  "error",
            ElementID: adr.ADRID,
            Message:   fmt.Sprintf("ADR %s marked superseded but has no superseded_by reference", adr.ADRID),
            Remedy:    "set_superseded_by",
        })
    }
}
```

### 4b. Module relationship conflicts

```go
// Modules that "maintain" and "interface" the same invariant
for mod, maintained := range moduleMainInvs {
    interfaces := moduleInterfaceInvs[mod]
    for _, m := range maintained {
        for _, i := range interfaces {
            if m == i {
                findings = append(findings, DriftFinding{
                    Category:  "contradiction",
                    Severity:  "error",
                    ElementID: i,
                    Message:   fmt.Sprintf("module %s both maintains and interfaces %s (should be one or the other)", mod, i),
                    Remedy:    "fix_module_relationship",
                })
            }
        }
    }
}
```

### 4c. Multi-owned invariants (from Check 5 / INV-013)

```go
maintainsCounts := make(map[string][]string) // invID → [module1, module2, ...]
for _, r := range rels {
    if r.RelType == "maintains" {
        modName := moduleByID[r.ModuleID]
        maintainsCounts[r.Target] = append(maintainsCounts[r.Target], modName)
    }
}

for invID, owners := range maintainsCounts {
    if len(owners) > 1 {
        findings = append(findings, DriftFinding{
            Category:  "contradiction",
            Severity:  "error",
            ElementID: invID,
            Message:   fmt.Sprintf("%s has %d owners (%s) — INV-013 requires exactly 1", invID, len(owners), strings.Join(owners, ", ")),
            Remedy:    "reassign_invariant_ownership",
        })
    }
}
```

---

## 5. Detection Logic: Coherence

**Question**: Is the spec's internal cross-reference graph consistent?

### 5a. Unresolved cross-references (from Check 1)

```go
unresolved, _ := storage.GetUnresolvedRefs(db, specID)
for _, xr := range unresolved {
    if isTemplateRef(xr.RefTarget) {
        continue // Skip template patterns like INV-NNN
    }
    findings = append(findings, DriftFinding{
        Category:  "coherence",
        Severity:  "error",
        ElementID: xr.RefTarget,
        Message:   fmt.Sprintf("unresolved %s reference to %s (line %d)", xr.RefType, xr.RefTarget, xr.SourceLine),
        Location:  fmt.Sprintf("line %d", xr.SourceLine),
        Remedy:    "fix_cross_reference",
    })
}
```

### 5b. Orphan sections (from Check 3 / INV-006)

```go
refCounts, _ := storage.GetSectionRefCounts(db, specID)
sections, _ := storage.ListSections(db, specID)

for _, sec := range sections {
    if sec.HeadingLevel <= 1 || isExemptSection(sec.SectionPath) {
        continue
    }
    rc := refCounts[sec.ID]
    if rc.Incoming == 0 && rc.Outgoing == 0 {
        findings = append(findings, DriftFinding{
            Category:  "coherence",
            Severity:  "warning",
            ElementID: sec.SectionPath,
            Message:   fmt.Sprintf("orphan section %s (%s) has 0 incoming and 0 outgoing references", sec.SectionPath, sec.Title),
            Location:  sec.SectionPath,
            Remedy:    "add_cross_references",
        })
    }
}
```

### 5c. Invariants with no validation_method

```go
for _, inv := range invs {
    if inv.ValidationMethod == "" {
        findings = append(findings, DriftFinding{
            Category:  "coherence",
            Severity:  "warning",
            ElementID: inv.InvariantID,
            Message:   fmt.Sprintf("%s has no validation_method — untestable invariant", inv.InvariantID),
            Remedy:    "add_validation_method",
        })
    }
}
```

### 5d. Cross-spec coherence: Unresolved parent references

For child specs whose cross-references target parent elements:

```go
if parentSpecID != nil {
    // Already handled by parser's ResolveCrossReferences —
    // any remaining unresolved refs after parent fallback are genuine coherence issues.
    // The unresolved refs from 5a already capture these.
    // We add an annotation to cross-spec findings:
    for i, f := range findings {
        if f.Category == "coherence" && f.Remedy == "fix_cross_reference" {
            // Check if this ref target exists in parent
            _, parentErr := storage.GetSection(db, *parentSpecID, f.ElementID)
            if parentErr == nil {
                findings[i].Message += " (exists in parent spec — resolution may have failed)"
            }
        }
    }
}
```

---

## 6. Call Order (Pseudocode)

```go
func Analyze(db *sql.DB, specID int64, opts DriftOptions) (*DriftReport, error) {
    spec, err := storage.GetSpecIndex(db, specID)
    parentSpecID, _ := storage.GetParentSpecID(db, specID)

    var findings []DriftFinding
    var summary DriftSummary

    // === PHASE 1: Load all data (parallel-safe, read-only) ===
    invs, _ := storage.ListInvariants(db, specID)
    adrs, _ := storage.ListADRs(db, specID)
    modules, _ := storage.ListModules(db, specID)
    rels, _ := storage.GetModuleRelationships(db, specID)
    registry, _ := storage.GetInvariantRegistryEntries(db, specID)
    sourceFiles, _ := storage.GetSourceFiles(db, specID)
    unresolved, _ := storage.GetUnresolvedRefs(db, specID)
    refCounts, _ := storage.GetSectionRefCounts(db, specID)
    sections, _ := storage.ListSections(db, specID)

    // Build index maps
    definedIDs := buildSet(invs, func(i) string { return i.InvariantID })
    registryIDs := buildSet(registry, func(r) string { return r.InvariantID })
    moduleByID := buildIDMap(modules)
    moduleMainInvs, moduleInterfaceInvs := groupRelsByModule(rels, moduleByID)

    // === PHASE 2: Run coverage for unimplemented detection ===
    covResult, _ := coverage.Analyze(db, specID, coverage.Options{})

    // === PHASE 3: Compute findings per category ===
    // 3a. Unimplemented (§2a-2c)
    detectUnimplemented(&findings, &summary, covResult, registry, definedIDs, modules, rels, moduleByID)

    // 3b. Unspecified (§3a-3d)
    detectUnspecified(&findings, &summary, invs, registryIDs, sourceFiles, modules, parentSpecID, db, specID)

    // 3c. Contradictions (§4a-4c)
    detectContradictions(&findings, &summary, adrs, rels, moduleByID, moduleMainInvs, moduleInterfaceInvs)

    // 3d. Coherence (§5a-5d)
    detectCoherence(&findings, &summary, unresolved, refCounts, sections, invs, parentSpecID, db, specID)

    // === PHASE 4: Compute drift score ===
    totalElements := len(invs) + len(adrs) + len(modules) + len(sections)
    if totalElements > 0 {
        summary.DriftScore = float64(summary.Total) / float64(totalElements)
        if summary.DriftScore > 1.0 {
            summary.DriftScore = 1.0
        }
    }

    return &DriftReport{
        SpecName:  spec.SpecName,
        SpecType:  spec.SourceType,
        HasParent: parentSpecID != nil,
        Summary:   summary,
        Findings:  findings,
    }, nil
}
```

---

## 7. Storage Functions Used (Complete List)

| Function | Purpose | Phase |
|----------|---------|-------|
| `GetSpecIndex(db, specID)` | Spec metadata | 1 |
| `GetParentSpecID(db, specID)` | Cross-spec relationship | 1 |
| `ListInvariants(db, specID)` | All invariants | 1 |
| `ListADRs(db, specID)` | All ADRs | 1 |
| `ListModules(db, specID)` | All modules | 1 |
| `GetModuleRelationships(db, specID)` | Module→invariant edges | 1 |
| `GetInvariantRegistryEntries(db, specID)` | Registry declarations | 1 |
| `GetSourceFiles(db, specID)` | Source file manifest | 1 |
| `GetUnresolvedRefs(db, specID)` | Broken cross-refs | 1 |
| `GetSectionRefCounts(db, specID)` | Orphan detection | 1 |
| `ListSections(db, specID)` | All sections | 1 |
| `coverage.Analyze(db, specID, opts)` | Component completeness | 2 |
| `GetSection(db, parentSpecID, path)` | Cross-spec ref check | 3 |
| `ListInvariants(db, parentSpecID)` | Parent comparison | 3 |
| `ListSections(db, parentSpecID)` | Parent section set | 3 |

---

## 8. Relationship to Existing Validator Checks

The drift command **subsumes** several validator checks, reusing their logic but classifying results into drift categories:

| Validator Check | Drift Category | Reuse Strategy |
|----------------|---------------|----------------|
| Check 1 (cross-ref integrity) | Coherence | Reuse `GetUnresolvedRefs()` directly |
| Check 3 (INV-006 xref density) | Coherence | Reuse `GetSectionRefCounts()` directly |
| Check 5 (INV-013 ownership) | Contradiction | Reuse relationship grouping logic |
| Check 7 (INV-015 decl-def) | Unimplemented + Unspecified | Reuse set-difference logic |
| Check 8 (INV-016 manifest sync) | Unspecified | Reuse source file vs modules logic |
| Check 12 (namespace consistency) | Unspecified | Can delegate to check directly |

**Implementation note**: Do NOT import the validator package. Instead, reuse the same storage queries and reproduce the set-difference logic. This keeps the drift package decoupled and allows independent evolution.

---

## 9. Edge Cases and Limitations

### Edge Case 1: Monolith specs
Monolith specs have no `modules`, `module_relationships`, or `invariant_registry` tables populated. The drift command must handle empty results gracefully:
- Module-related checks (§2c, §3b, §4b, §4c) produce zero findings for monolith specs
- Coverage and cross-ref checks still work

### Edge Case 2: No parent spec
When `parent_spec_id` is NULL, all cross-spec checks (§3c, §5d) are skipped. The `HasParent` field signals this to the renderer.

### Edge Case 3: Parent spec not in same database
The current schema stores parent as `parent_spec_id INTEGER REFERENCES spec_index(id)` — both specs must be in the same SQLite DB. The parser's `ParseModularSpec()` handles this by recursively parsing the parent. If the parent wasn't parsed, `GetParentSpecID()` returns nil.

### Edge Case 4: APP-prefixed invariants
Child specs use `APP-INV-NNN` prefixed invariants for application-specific rules. These should NOT be flagged as "not in parent" since they're intentionally child-only.

### Edge Case 5: Template references
References like `INV-NNN`, `§N.M`, `ADR-NNN` are template patterns in the spec. The `isTemplateRef()` function from the validator must be reused to filter these out of coherence findings.

### Edge Case 6: Coverage.Analyze() includes ADRs
The coverage result includes both invariants and ADRs. For drift purposes, we check invariant coverage primarily. ADR coverage is informational (ADRs document decisions, not implementations).

### Limitation: No source code comparison
The drift command does NOT compare spec content to actual Go source code. That's the domain of Check 13 (implementation traceability), which requires `--code-root`. The drift command works purely on the parsed spec index. A future `--code-root` extension could add a "code-spec drift" category.

### Limitation: No semantic comparison
We cannot detect whether an invariant's *meaning* has drifted from its implementation. We only detect *structural* drift: missing elements, broken references, inconsistent declarations.

---

## 10. Drift Score Calculation

```
drift_score = min(1.0, total_findings / total_elements)

where:
  total_findings = unspecified + unimplemented + contradictions + coherence
  total_elements = len(invariants) + len(ADRs) + len(modules) + len(sections)
```

Weighted alternative (recommended):
```
weighted_score = (unspecified * 0.3 + unimplemented * 0.4 + contradictions * 0.5 + coherence * 0.2) / total_elements
```

The weights reflect impact:
- **Contradictions** (0.5): Most severe — the spec actively misleads
- **Unimplemented** (0.4): Spec promises unfulfilled
- **Unspecified** (0.3): Implementation beyond spec boundary
- **Coherence** (0.2): Internal consistency issues (often cosmetic)

---

## 11. Classify() Integration

The `DriftFinding.Remedy` field provides a hint to the `Classify()` function for generating remediation tasks. Each remedy string maps to a concrete action:

| Remedy | Action |
|--------|--------|
| `add_invariant_components` | Add missing statement/semi_formal/violation/validation |
| `complete_invariant_components` | Fill in weak components |
| `add_invariant_definition` | Write full invariant block for registry entry |
| `assign_invariants_to_module` | Add maintains relationships |
| `add_to_registry` | Add invariant_registry entry |
| `register_module` | Add module to manifest |
| `update_count_declarations` | Fix range claims in prose |
| `set_superseded_by` | Fill superseded_by field |
| `fix_module_relationship` | Change maintains↔interfaces |
| `reassign_invariant_ownership` | Deduplicate ownership |
| `fix_cross_reference` | Resolve or remove broken ref |
| `add_cross_references` | Connect orphan section |
| `add_validation_method` | Write testable validation |
| `add_to_parent_or_mark_app_specific` | Decide parent/child boundary |
