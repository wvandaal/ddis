> **Section**: §21. WITNESS — Falsification-Bound Verification | **Wave**: 4 (Integration)
> **Namespace**: WITNESS | **Stage**: 1 (core), 2+ (LLM-as-judge, SMT)
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)
> **Traces to**: SEED.md §3 (Specification Formalism), §6 (Reconciliation), §7 (Self-Improvement)

## §21. Falsification-Bound Witness System

> **Purpose**: Prevent the silent erosion of verification quality — the failure mode where
> tests are weakened to hide bugs rather than bugs being fixed to pass tests. The witness
> system binds each invariant's *falsification condition* to its *verification evidence*
> via content-addressed hashes, creating a structural coupling that breaks automatically
> when either side changes.

### §21.1 Core Abstraction: Falsification-Bound Witness (FBW)

A **Falsification-Bound Witness** is a datom-backed proof receipt that binds three
content-addressed artifacts into a verification triple:

```
FBW(inv) = (spec_hash, falsification_hash, test_body_hash, verdict, depth, tx)
```

| Component | Definition | Hash Input |
|-----------|-----------|------------|
| `spec_hash` | BLAKE3 of `:spec/statement` ∥ `:spec/falsification` | Full invariant text |
| `falsification_hash` | BLAKE3 of `:spec/falsification` alone | Verification contract |
| `test_body_hash` | BLAKE3 of test function source body | Verification evidence |
| `verdict` | Challenge result from §21.3 protocol | Adjudicated status |
| `depth` | Verification depth (L1–L4) from trace scan | Maturity level |
| `tx` | Transaction provenance | Who, when, why |

**Key property**: The FBW is automatically **invalidated** (marked stale) when ANY
of the three hashes changes. This creates a triple-lock that detects:

1. **Spec drift** → `spec_hash` changes → stale
2. **Test weakening** → `test_body_hash` changes → stale
3. **Contract mutation** → `falsification_hash` changes → stale

### §21.2 Invariants

#### INV-WITNESS-001: Triple-Hash Auto-Invalidation

**Traces to**: SEED.md §6 (Reconciliation), APP-INV-041 (Go CLI witness staleness)
**Type**: Invariant
**Stage**: 1
**Statement**: For every FBW in the store, if any of `spec_hash`, `falsification_hash`, or
`test_body_hash` differs from the current computed hash of the referenced artifact, the
FBW's `:witness/status` is `stale`. Formally:

```
∀ fbw ∈ FBW(S):
  let current_spec = BLAKE3(inv.statement ∥ inv.falsification)
  let current_fals = BLAKE3(inv.falsification)
  let current_test = BLAKE3(test_body(fbw.test_fn))

  (fbw.spec_hash ≠ current_spec ∨
   fbw.falsification_hash ≠ current_fals ∨
   fbw.test_body_hash ≠ current_test)
  → fbw.status = :witness.status/stale
```

**Falsification**: An FBW exists in the store with `status = :witness.status/valid` but one
or more of its three hashes does not match the current artifact content. This means a
spec change, test change, or falsification change went undetected.
**Verification**: V:PROP + V:KANI — proptest with random spec/test mutations; Kani harness
verifying staleness detection for all 3 hash paths.

---

#### INV-WITNESS-002: Falsification Alignment

**Traces to**: SEED.md §3 (C6 Falsifiability), INV-BILATERAL-005
**Type**: Invariant
**Stage**: 1 (keyword), 2 (LLM), 3+ (SMT)
**Statement**: A witness at depth L2+ must demonstrate alignment between the test's
verification behavior and the invariant's falsification condition. The alignment score
A(test, falsification) ∈ [0, 1] must exceed a per-depth minimum:

```
∀ fbw with depth ≥ L2:
  A(fbw.test_fn, fbw.inv.falsification) ≥ alignment_threshold(fbw.depth)

where:
  alignment_threshold(L2) = 0.3   (keyword overlap)
  alignment_threshold(L3) = 0.5   (property coverage)
  alignment_threshold(L4) = 0.7   (formal entailment)
```

**Falsification**: A witness at L2+ exists where the test contains no assertions that would
fail if the falsification condition were true. Example: falsification says "violated if
datom count decreases" but test only checks `assert!(result.contains("ok"))`.
**Verification**: V:PROP (keyword extraction + Jaccard similarity at Stage 1),
V:EVAL (LLM-as-judge alignment at Stage 2), V:SMT (predicate entailment at Stage 3+).

---

#### INV-WITNESS-003: Monotonic Formality Progression

**Traces to**: INV-TRILATERAL-003, SEED.md §3
**Type**: Invariant
**Stage**: 1
**Statement**: The verification depth of a witnessed invariant is monotonically
non-decreasing. An invariant witnessed at depth L_n cannot be re-witnessed at
depth L_m where m < n without a deliberation decision.

```
∀ inv ∈ S, ∀ fbw₁ fbw₂ ∈ FBW(inv):
  fbw₂.tx > fbw₁.tx ∧ fbw₁.status = valid
  → fbw₂.depth ≥ fbw₁.depth ∨ fbw₂.has_deliberation_override
```

**Falsification**: A new FBW for an invariant has lower depth than the existing valid FBW
without a deliberation record justifying the regression.
**Verification**: V:PROP — proptest generating FBW sequences with monotonicity check.

---

#### INV-WITNESS-004: Challenge Adjunction Completeness

**Traces to**: APP-ADR-037 (Challenge as Right Adjoint), SEED.md §6
**Type**: Invariant
**Stage**: 1
**Statement**: Every FBW with `status = valid` has been verified by at least one
challenge pass. The challenge is the right adjoint to witness — it mechanically
verifies what the witness claims. No witness transitions to `valid` without challenge.

```
∀ fbw with status = valid:
  ∃ challenge_result(fbw) with verdict ∈ {confirmed, provisional}
```

**Falsification**: An FBW has `status = valid` but no corresponding challenge result
datom exists in the store.
**Verification**: V:PROP + V:KANI — verify the witness→challenge→verdict pipeline.

---

#### INV-WITNESS-005: Stale Witnesses Reduce F(S)

**Traces to**: INV-BILATERAL-001 (Fitness Convergence), ADR-BILATERAL-001
**Type**: Invariant
**Stage**: 1
**Statement**: The bilateral fitness function F(S) validation component V treats stale
witnesses as unverified. An invariant with only stale FBWs contributes 0 to the
validation score, regardless of its former depth.

```
V(inv) = max(depth_weight(fbw.depth) for fbw in FBW(inv) if fbw.status = valid)
       = 0.0 if all FBWs are stale

F(S).validation = Σ V(inv) / (|INV| × depth_weight(L4))
```

**Falsification**: A stale witness contributes non-zero to F(S).validation.
**Verification**: V:PROP — construct store with stale witnesses, verify F(S).V = 0.

---

#### INV-WITNESS-006: Test Body Hash Extraction

**Traces to**: INV-TRACE-001, SEED.md §3
**Type**: Invariant
**Stage**: 1
**Statement**: The trace scanner extracts test function body hashes for every test that
references a spec element. The hash covers the complete function body (from opening `{`
to closing `}` at the same brace depth), excluding whitespace-only changes.

```
∀ trace_link with depth ≥ L2:
  ∃ test_body_hash(trace_link.test_fn) = BLAKE3(normalize(body(trace_link.test_fn)))

where normalize strips:
  - leading/trailing whitespace per line
  - blank lines
  - comment-only lines
```

**Falsification**: A trace link at L2+ exists but `test_body_hash` is None or empty.
**Verification**: V:PROP — scan source with known test functions, verify hashes are computed.

---

#### INV-WITNESS-007: Auto-Task Filing on Refutation

**Traces to**: INV-GUIDANCE-009 (Derived Tasks), APP-INV-051 (Go CLI challenge task derivation)
**Type**: Invariant
**Stage**: 1
**Statement**: When a challenge produces a `refuted` verdict, the system automatically
files a bug task tracing to the refuted invariant. The task title includes the invariant
ID and the refutation reason.

```
∀ challenge with verdict = refuted:
  ∃ task ∈ S with:
    task.type = bug
    task.title contains challenge.inv_id
    task.traces_to = challenge.inv_entity
```

**Falsification**: A challenge refutation exists but no corresponding bug task was filed.
**Verification**: V:PROP — mock a refutation, verify task datoms are generated.

---

### §21.3 Challenge Protocol (6-Level Progressive Verification)

The challenge protocol is the **right adjoint** to the witness (APP-ADR-037).
It mechanically verifies witness claims through 6 independent levels:

```
Challenge(fbw) → Verdict ∈ {confirmed, provisional, refuted, contradicted}
```

| Level | Name | What It Checks | Confidence | Stage |
|-------|------|---------------|------------|-------|
| 0 | **Falsification Alignment** | Test assertions align with falsification condition | 0.4–0.9 | 1 |
| 1 | **Formal Consistency** | Falsification is self-consistent (no tautology) | 0.85 | 2+ |
| 2 | **Evidence Type** | Witness depth matches claimed evidence | 0.3–0.9 | 1 |
| 3 | **Causal Annotation** | Code annotations reference the invariant | 0.5–0.9 | 1 |
| 4 | **Practical Execution** | Test runs and passes | 1.0 | 1 |
| 5 | **Semantic Overlap** | Test keywords align with invariant keywords | 0.4–0.7 | 1 |

**Level 0 (NEW — the key innovation)**: Extracts assertion expressions from the test body
and compares them against the falsification condition. At Stage 1, uses keyword extraction
and Jaccard similarity. At Stage 2+, uses LLM-as-judge (3-run majority vote per
APP-INV-055). At Stage 3+, translates both to first-order predicates and checks entailment.

**Evidence Accumulation Verdict** (APP-ADR-039): Independent signals compound:

```
Hard Refutation (categorical override):
  - Test ran and FAILED (Level 4)
  - Falsification is self-contradictory (Level 1)

Full Confirmation:
  - Test ran AND passed AND alignment score > threshold
  → Verdict: confirmed, Score: 1.0

Accumulation:
  base = evidence_type_confidence(fbw.depth)
  + boost(multi_package_annotations, 0.05–0.15)
  + boost(falsification_alignment > 0.3, 0.10)
  + boost(keyword_overlap > 0.15, 0.05)
  + boost(formal_consistency, 0.05)

Thresholds:
  score ≥ 0.85 → confirmed
  0.30 < score < 0.85 → provisional
  score ≤ 0.30 → inconclusive
```

---

### §21.4 Design Decisions

#### ADR-WITNESS-001: Triple-Hash Over Single-Hash

**Traces to**: SEED.md §6, APP-INV-041
**Stage**: 1

##### Problem
The Go CLI uses a single `spec_hash` for witness invalidation. This detects spec changes
but NOT test weakening (test changes without spec changes) or contract mutation
(falsification changes that don't affect the statement).

##### Options
A) **Single spec_hash** (Go CLI pattern) — detects spec drift only.
B) **Double hash (spec + test)** — detects spec drift and test changes.
C) **Triple hash (spec + falsification + test)** — detects spec drift, test changes,
AND falsification contract mutations independently.

##### Decision
**Option C.** The falsification condition is the verification *contract* — it defines
what the test MUST verify. Tracking it independently from the full spec text means we
detect when the contract changes even if the statement doesn't (e.g., weakening the
violation condition while keeping the positive statement intact).

##### Falsification
The triple-hash produces false-positive staleness (marks witnesses stale when the
actual verification relationship hasn't changed — e.g., whitespace-only edits to
falsification text). Mitigation: normalize before hashing (strip whitespace, comments).

---

#### ADR-WITNESS-002: Falsification Alignment as Challenge Level 0

**Traces to**: SEED.md §3 (C6 Falsifiability), APP-ADR-037
**Stage**: 1

##### Problem
The Go CLI's 5-level challenge checks whether a test EXISTS, PASSES, and has keyword
overlap — but never checks whether the test actually VERIFIES the falsification condition.
A test that `assert!(true)` would pass all 5 levels if it names the invariant.

##### Options
A) **No alignment check** — rely on human review to verify test quality.
B) **Keyword overlap only** — check if test contains words from the falsification.
C) **Graduated alignment** — keyword overlap at L2, property coverage at L3, formal
entailment at L4, with LLM-as-judge at Stage 2+.

##### Decision
**Option C.** The alignment check is staged:
- Stage 1: Extract assertion keywords from test body, compute Jaccard similarity
  against falsification keywords. Threshold: 0.3 (30% keyword overlap).
- Stage 2: LLM-as-judge evaluates "does this test verify the falsification condition?"
  using 3-run majority vote (APP-INV-055 pattern).
- Stage 3+: Translate falsification to first-order predicate, translate test assertions
  to postconditions, check entailment via SMT solver.

##### Falsification
The keyword-based alignment produces false negatives (test uses different vocabulary than
falsification to verify the same property). Mitigation: synonym expansion at Stage 2+
via LLM semantic understanding.

---

#### ADR-WITNESS-003: Witness as Datom (Not Database Row)

**Traces to**: SEED.md §4 (C1 Append-Only), ADR-STORE-017
**Stage**: 1

##### Problem
The Go CLI stores witnesses as SQL rows in a relational table. This creates a separate
state management concern (the SQLite DB) outside the datom store.

##### Options
A) **SQLite table** (Go CLI pattern) — separate witness DB, separate backup, separate merge.
B) **Datom-backed witnesses** — FBW as datoms in the main store, queryable via Datalog,
mergeable via CRDT set union, append-only by construction.

##### Decision
**Option B.** Witnesses are datoms. The FBW entity has attributes:
- `:witness/spec-hash` (Bytes)
- `:witness/falsification-hash` (Bytes)
- `:witness/test-body-hash` (Bytes)
- `:witness/verdict` (Keyword)
- `:witness/depth` (Long)
- `:witness/inv-ref` (Ref → invariant entity)
- `:witness/test-fn` (String)
- `:witness/source-file` (String)
- `:witness/alignment-score` (Double)
- `:witness/status` (Keyword: valid/stale/refuted/contradicted)
- `:witness/challenge-tx` (Ref → challenge transaction)

This means witnesses are:
- **Queryable**: `[:find ?inv ?status :where [?w :witness/inv-ref ?inv] [?w :witness/status ?status]]`
- **Mergeable**: Two agents witnessing the same invariant produce two FBWs (not conflicts)
- **Append-only**: Stale status is a new assertion, not a mutation
- **Traceable**: Full provenance via transaction metadata

##### Falsification
Datom-backed witnesses consume more storage than SQL rows (7+ datoms per FBW vs 1 row).
Storage is proportional to invariant count (bounded: currently 145 INVs × ~10 datoms = ~1450).

---

### §21.5 Negative Cases

#### NEG-WITNESS-001: No Unbound Witness

**Traces to**: SEED.md §3 (C5 Traceability)
**Type**: Negative Case
**Statement**: Every FBW must reference a valid invariant entity in the store. An FBW
with `:witness/inv-ref` pointing to a non-existent entity is a defect.
**Violation**: An FBW exists where `resolve(fbw.inv_ref)` returns no entity.

---

#### NEG-WITNESS-002: No Silent Weakening

**Traces to**: SEED.md §6 (Structural Divergence)
**Type**: Negative Case
**Statement**: A test body change that reduces the assertion count or removes a
falsification-relevant assertion MUST invalidate the FBW. The system cannot
silently accept a weakened test as still-valid verification.
**Violation**: A test body hash changes, the new test has fewer assertions or weaker
conditions, but the FBW status remains `valid`.

---

#### NEG-WITNESS-003: No Verdict Without Challenge

**Traces to**: APP-ADR-037 (Adjunction)
**Type**: Negative Case
**Statement**: An FBW cannot have `status = valid` without a recorded challenge result.
Witnesses that bypass the challenge protocol are ceremonial, not verification.
**Violation**: An FBW has `status = valid` and no `:witness/challenge-tx` ref.

---

### §21.6 Schema Attributes

New Layer 5 attributes for the witness system:

| Attribute | ValueType | Cardinality | Resolution | Description |
|-----------|-----------|-------------|------------|-------------|
| `:witness/spec-hash` | Bytes | One | LWW | BLAKE3 of statement ∥ falsification |
| `:witness/falsification-hash` | Bytes | One | LWW | BLAKE3 of falsification alone |
| `:witness/test-body-hash` | Bytes | One | LWW | BLAKE3 of normalized test body |
| `:witness/verdict` | Keyword | One | LWW | Challenge verdict |
| `:witness/depth` | Long | One | LWW | Verification depth (1–4) |
| `:witness/inv-ref` | Ref | One | LWW | Reference to invariant entity |
| `:witness/test-fn` | String | One | LWW | Test function name |
| `:witness/source-file` | String | One | LWW | Source file path |
| `:witness/alignment-score` | Double | One | LWW | Falsification alignment [0,1] |
| `:witness/status` | Keyword | One | LWW | valid/stale/refuted/contradicted |
| `:witness/challenge-tx` | Ref | One | LWW | Reference to challenge tx |

Status lattice:
```
valid → stale (any hash change)
valid → refuted (challenge Level 4 failure)
stale → valid (re-witnessed after challenge confirms)
{valid, stale} → contradicted (conflicting verdicts from different agents)
```

### §21.7 Uncertainty Register

| ID | Element | Confidence | Resolution Trigger |
|----|---------|------------|-------------------|
| UNC-WITNESS-001 | Keyword extraction quality for Level 0 alignment | 0.6 | Stage 2 LLM-as-judge replaces keyword matching |
| UNC-WITNESS-002 | Whitespace normalization completeness | 0.8 | Edge case testing with macro-heavy Rust code |
| UNC-WITNESS-003 | BLAKE3 collision resistance for test bodies | 0.99 | Theoretical — 2^128 collision resistance |
