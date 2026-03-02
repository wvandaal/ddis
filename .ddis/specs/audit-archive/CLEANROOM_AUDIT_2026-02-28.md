# Cleanroom Software Engineering Audit — 2026-02-28

**Thread**: `t-cleanroom-audit-2026-02-28`
**Methodology**: Formal methods, spec-driven design, abstract algebra
**Scope**: 238 Go files, 37 packages, 61K+ LOC, 55 test files
**Approach**: Six parallel deep-dive exploration threads

---

## 1. Verified Findings (Code-Level Evidence)

### Finding F-01: Snapshot Position Semantic Error [CRITICAL]

**Location**: `internal/cli/snapshot.go:104`
**Category**: Axiological divergence — violates event-sourcing semantics

The snapshot `position` field (APP-INV-094) is supposed to represent the
number of events processed in the fold, enabling `FoldFrom()` to resume
from the correct position. Instead, it queries the invariant count:

```go
db.QueryRow("SELECT COUNT(*) FROM invariants WHERE spec_id = ?", specID).Scan(&eventCount)
```

**Algebraic diagnosis**: The snapshot position forms a monotone counter
in the event stream's ordinal space (N → N, strictly increasing with
each applied event). By substituting invariant count (a projection of
the state, not the stream position), the monotonicity invariant breaks:
- Invariants can be deleted (position decreases)
- Upserts don't increment count (position stalls)
- Round-trip `CreateSnapshot → FoldFrom(snap.Position)` skips or replays
  wrong events

**Spec elements violated**: APP-INV-094 (snapshot monotonicity)
**Spec elements implicated**: APP-INV-075 (idempotency via FoldFrom)

---

### Finding F-02: Manifest YAML String Manipulation [CRITICAL]

**Location**: `internal/cli/crystallize.go:330-338`
**Category**: Structural divergence — bypasses bilateral specification loop

The `crystallize` command updates the manifest's invariant registry by
appending a raw string before the last newline, with zero YAML parsing:

```go
content := string(data)
entry := fmt.Sprintf("  %s: { owner: %s, ... }\n", ...)
if idx := strings.LastIndex(content, "\n"); idx >= 0 {
    content = content[:idx+1] + entry
}
return os.WriteFile(manifestPath, []byte(content), 0644)
```

**Algebraic diagnosis**: YAML is a context-free grammar. String concat
operates on the character monoid (free monoid over byte alphabet), which
does not preserve the CFG structure. The operation is NOT a homomorphism
from YAML AST to YAML AST — it can produce malformed output if:
- The file lacks a trailing newline
- The registry section has inline comments
- A concurrent crystallize interleaves writes (no file locking)

**Spec elements violated**: APP-INV-001 (round-trip fidelity), APP-ADR-009
**Spec elements implicated**: APP-INV-073 (fold determinism — events are
emitted BEFORE manifest update succeeds)

---

### Finding F-03: Materialize Applier Hardcodes section_id=0 [CRITICAL]

**Location**: `internal/cli/materialize.go:224-351`
**Category**: Structural divergence — breaks hierarchical integrity

The `sqlApplier` used during event fold hardcodes `section_id = 0` for
all content insertions (invariants, ADRs, glossary, negative specs, gates):

```go
// Line 242 (InsertInvariant):
VALUES (?, ?, 0, ?, ?, ...  // section_id = 0
// Line 272 (InsertADR):
VALUES (?, ?, 0, ?, ?, ...  // section_id = 0
```

**Algebraic diagnosis**: The section tree is a rooted forest (DAG with
unique parents). Each content element's section_id is a morphism
mapping content → its containing section. Setting section_id=0 collapses
this morphism to a constant function (everything maps to root/null),
destroying the forest structure.

Consequences:
- Coverage analysis cannot compute per-section completeness
- Validation Check 5 (INV-003) cannot verify invariants are in correct sections
- Section-scoped queries return empty (broken FK relationship)
- `project` command's section reconstruction has no structural data

**Spec elements violated**: APP-INV-076 (projection is pure function of
structured data — but structured data is incomplete)
**Spec elements implicated**: APP-INV-001 (parse → fold → project round-trip
is lossy because section hierarchy is not in events)

---

### Finding F-04: Cross-Reference Diff Key Collision [HIGH]

**Location**: `internal/materialize/diff.go:423`
**Category**: Logic error — silent data loss in structural diff

The `diffCrossRefs` function builds a map keyed by `target + "|" + text`,
excluding `ref_type`:

```go
key := r.target + "|" + r.text   // ref_type NOT in key
```

Two cross-references with identical target and text but different ref_type
(e.g., `invariant` vs `app_invariant`) collide. The second overwrites the
first, and the structural diff only detects the ref_type change for one of
them, silently dropping the other.

**Algebraic diagnosis**: The map key should be a tuple from the product
type (ref_type × target × text). Using only (target × text) projects
away the discriminant, collapsing distinct elements in the quotient.

**Spec elements violated**: APP-INV-093 (StateHash determinism relies on
StructuralDiff correctness)

---

### Finding F-05: LLM Confidence Constant Mismatch [MEDIUM]

**Location**: `internal/witness/eval.go:93` vs `internal/consistency/llm.go:166`
**Category**: Axiological divergence — inconsistent confidence semantics

Two independent implementations of LLM majority voting use different
confidence values for the same agreement level:

| Agreement | eval.go (witness) | llm.go (consistency) |
|-----------|-------------------|----------------------|
| 3/3       | 0.95              | 0.95                 |
| 2/3       | **0.75**          | **0.80**             |

**Algebraic diagnosis**: Confidence should be a function from
(agreement_count, total_runs) → [0, 1], forming a consistent lattice.
Having two incompatible functions violates the homomorphism property:
deduplication logic that compares confidence across tiers will mis-order
findings depending on which code path produced them.

**Spec elements implicated**: APP-INV-055 (statistical soundness)

---

### Finding F-06: Witness Re-Attachment Multi-Witness Orphaning [MEDIUM]

**Location**: `internal/parser/manifest.go:274-285`
**Category**: Logic error — FK integrity violation on re-parse

During modular spec re-parse, witnesses are saved and re-attached using
a map keyed by `InvariantID`:

```go
witnessIDMap := make(map[string]int64)
// ...
witnessIDMap[w.InvariantID] = newID  // Last witness wins
```

If an invariant has multiple witnesses (multiple evidence runs), only the
last witness's new ID is stored. Challenges pointing to earlier witnesses
get `WitnessID = nil` (orphaned).

**Algebraic diagnosis**: The map models a function InvariantID → WitnessID,
but the actual relationship is a multimap (one-to-many). The function
projection loses cardinality.

**Spec elements implicated**: APP-INV-041 (witness auto-invalidation)

---

### Finding F-07: Governance Overlap False Negatives [MEDIUM]

**Location**: `internal/consistency/graph.go:127`
**Category**: Design limitation — incomplete contradiction detection

Governance overlap detection requires BOTH Jaccard overlap > 0.6 AND
opposing polarity signals (`hasOpposingPolarity()`). This misses subtle
conflicts where two invariants share high subject-matter overlap without
explicit polarity markers (e.g., "cache all requests for 24h" vs "respect
user privacy boundaries in caching").

**Algebraic diagnosis**: The polarity function is a partial function over
the statement space — it's undefined for statements that conflict through
implication rather than surface-level polarity. The AND gate makes the
detector a conjunction of two partial predicates, whose domain intersection
is strictly smaller than the union (false negatives).

**Spec elements implicated**: APP-INV-019 (contradiction detection)

---

### Finding F-08: isDerivedEvent Convention-Based Heuristic [LOW]

**Location**: `internal/materialize/fold.go:378-395`
**Category**: Design fragility — convention vs type safety

Derived event detection relies on three conventions:
1. Payload is valid JSON
2. Event has at least one cause
3. Payload contains a `"derived_by"` field

A user-created event with a `"derived_by"` field would be misclassified
as processor output, skipping processor invocation.

**Algebraic diagnosis**: Event classification should be a total function
from Event → {Primary, Derived}. The current implementation is partial
(heuristic can err in both directions), making it a partial function with
undefined behavior at the boundary.

**Spec elements implicated**: APP-INV-090 (derived event dedup)

---

## 2. Structural Analysis: Cross-Cutting Concerns

### 2.1 Event Emission Decoupling [HIGH — Pervasive]

**Pattern across 10+ commands**: Events are emitted AFTER command execution
completes, decoupled from the command's transaction boundary.

Example: `parse` runs parser → builds search index → emits TypeSpecParsed.
If event emission fails (disk full, permission), the command succeeds but
the event stream is inconsistent with the actual state.

**Algebraic diagnosis**: The bilateral cycle requires events as the primary
write path (APP-INV-071: log canonicality). But the implementation treats
events as secondary audit records. This is an architectural inversion.

### 2.2 FindDB Auto-Discovery Ambiguity [MEDIUM — Pervasive]

Used by 30+ commands. If multiple `.ddis.db` files exist in the directory
tree, behavior depends on glob ordering — could silently operate on the
wrong spec.

### 2.3 GetFirstSpecID Fragility [MEDIUM — Pervasive]

All commands assume `GetFirstSpecID()` returns the canonical spec. If the
DB has 2+ specs (legacy data, re-parse artifacts), the fallback query
returns any spec, including parent specs.

---

## 3. Test Infrastructure Findings

### 3.1 Shared Database State (29+ tests)
- `sharedModularDB` in `integration_helpers_test.go` is shared across
  all behavioral tests without isolation
- Any mutation accumulates, making tests order-dependent

### 3.2 Vacuous Test Passing (~6-8 tests)
- Tests skip when oplog/authority/glossary tables are empty
- Example: `TestAPPINV006_TransactionStateMachine` passes with "vacuously true"
  without testing the actual invariant

### 3.3 Test Coverage vs Claims
- 97/97 invariants claimed witnessed
- ~88-90 actually meaningfully tested
- 6-8 pass vacuously, 1-2 lack verification logic

---

## 4. Prioritized Fix Plan

### P0 — Critical (Breaks core invariants)
1. **F-01**: Fix snapshot position to count events, not invariants
2. **F-02**: Replace string manipulation with proper YAML marshal/unmarshal
3. **F-03**: Add section hierarchy to event schema and applier

### P1 — High (Silent data corruption)
4. **F-04**: Include ref_type in cross-ref diff composite key
5. **Event emission**: Move to pre-commit or make events the primary write path
6. **FindDB**: Error on ambiguity (>1 candidate)

### P2 — Medium (Inconsistency)
7. **F-05**: Centralize LLM confidence constants
8. **F-06**: Use multimap for witness re-attachment
9. **F-07**: Add implication-based governance detection (semantic tier)
10. **GetFirstSpecID**: Validate spec type before returning

### P3 — Low (Hardening)
11. **F-08**: Use typed event discriminant instead of convention
12. **Test isolation**: Per-test DB creation for behavioral tests
13. **Vacuous tests**: Convert skips to proper assertions or mocks
