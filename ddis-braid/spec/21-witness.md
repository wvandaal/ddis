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

---

## §21.8 Closing the Five Verification Gaps

The verification stack audit (Session 024) identified five gaps where errors can
slip through ALL existing layers. This section specifies the solution for each gap,
with the principle: **every gap gets exactly one mechanism that closes it completely**.

### Gap 1: Test-to-Falsification Alignment → Level 0 Challenge (INV-WITNESS-002)

**Already specified above.** The falsification alignment score A(test, falsification)
verifies that the test's assertions exercise the invariant's violation condition.
Staged: keyword overlap (S1) → LLM-as-judge (S2) → SMT entailment (S3+).

No additional invariants needed — INV-WITNESS-002 fully closes this gap.

### Gap 2: Test Content Stability → Triple-Hash Lock (INV-WITNESS-001)

**Already specified above.** The test_body_hash component of the FBW triple detects
any modification to the test body, including weakening. Combined with INV-WITNESS-005
(stale witnesses reduce F(S)), weakened tests cause immediate fitness degradation.

No additional invariants needed — INV-WITNESS-001 + INV-WITNESS-005 fully close this gap.

### Gap 3: Harness/Property Correctness → Verification Regression Detection

**The problem**: A Kani harness or proptest property can verify the WRONG thing.
The INV-MERGE-002 Kani proof checked frontier monotonicity instead of cascade
completeness for months. No existing layer detects "proof verifies wrong property."

**The solution**: Extend the FBW to cover harnesses and properties, not just tests.
The falsification alignment check (Level 0) applies to ALL verification evidence:

#### INV-WITNESS-008: Harness-Falsification Binding

**Traces to**: SEED.md §3 (C6 Falsifiability), INV-VERIFICATION-001
**Type**: Invariant
**Stage**: 1
**Statement**: Every V:KANI harness and V:MODEL model check that claims to verify an
invariant must have a corresponding FBW with falsification alignment score ≥ 0.5
(property coverage level). The harness's assertions must be semantically related to
the invariant's falsification condition.

```
∀ harness h with spec_ref(h) = inv:
  ∃ fbw ∈ FBW(inv):
    fbw.depth ∈ {L3, L4}
    fbw.test_body_hash = BLAKE3(normalize(body(h)))
    A(h, inv.falsification) ≥ 0.5
```

**Falsification**: A Kani harness references INV-MERGE-002 (cascade completeness) but
its assertions only check `store.len() >= pre_len` (frontier monotonicity, which is
INV-STORE-001). Alignment score against cascade falsification = 0.1. This should be
detected and flagged.
**Verification**: V:PROP — construct harness with mismatched assertions, verify A < 0.5.

---

### Gap 4: Systematic Evaluation Bias → Cognitive Independence

**The problem**: The Go CLI's 3-run majority vote calls the SAME model 3 times within
the SAME conversation context. At turn 45 with depleted k*, this produces 3 correlated
sycophantic responses — not independence. The prompt-optimization theory (k* decay,
basin trapping) predicts this: a context-depleted agent cannot switch from implementation
mode to adversarial verification mode.

**The solution**: Challenge execution MUST occur in cognitively independent contexts.
In braid, this means Agent subagents with:
- **Fresh k*** — no shared conversation history (full attention budget)
- **No self-serving bias** — subagent never saw the implementation reasoning
- **Verification-specific basin** — prompt targeted at finding violations, not confirming

#### INV-WITNESS-009: Cognitive Independence of Challenge

**Traces to**: SEED.md §7 (Self-Improvement Loop), prompt-optimization k* theory
**Type**: Invariant
**Stage**: 1
**Statement**: Every challenge evaluation at Level 0 (falsification alignment) and
Level 4 (practical execution) MUST be executed in a context that is cognitively
independent from the context that produced the artifact being verified. Two contexts
are cognitively independent iff:

```
independent(ctx_producer, ctx_verifier) ⟺
  (1) ctx_verifier.conversation_history ∩ ctx_producer.conversation_history = ∅
      (no shared turns — fresh k*)
  (2) ctx_verifier.prompt does NOT contain ctx_producer.reasoning
      (no access to implementation rationale — prevents self-serving bias)
  (3) ctx_verifier.prompt.basin ∈ {adversarial, verification}
      (targeted at finding violations, not confirming expectations)
```

In the braid architecture, this is satisfied by spawning Agent subagents for challenge
evaluation. Each subagent:
- Starts with an empty conversation (full k* = 1.0)
- Receives ONLY: invariant statement, falsification condition, test source code
- Does NOT receive: implementation reasoning, prior conversation, other witnesses
- Has a verification-specific system prompt emphasizing adversarial evaluation

**Falsification**: A challenge evaluation is performed within the same conversation
context that produced the code or test being evaluated. The evaluator has access to
the implementation reasoning and may rubber-stamp due to self-serving bias or
k*-depleted sycophancy.
**Verification**: V:PROP — verify that challenge execution spawns a separate context
(Agent tool call with no shared state). V:ARCH — architectural review confirming no
shared mutable state between producer and verifier contexts.

---

#### INV-WITNESS-010: Decorrelated Multi-Verdict

**Traces to**: APP-INV-055 (Statistical Soundness), prompt-optimization basin theory
**Type**: Invariant
**Stage**: 1
**Statement**: When multiple challenge verdicts are aggregated (majority vote), each
verdict MUST come from a decorrelated cognitive context. Specifically:

```
∀ majority_vote(fbw) with verdicts v₁, v₂, ..., vₙ:
  ∀ i ≠ j: independent(ctx(vᵢ), ctx(vⱼ))
```

Three API calls to the same model within the same conversation are NOT decorrelated
(they share context, k*, and basin). Three independent Agent subagents ARE decorrelated
(each has fresh context, full k*, and can be prompted differently).

**Implementation**: At Stage 1, spawn 3 independent subagents using the Agent tool:
- Subagent 1: "Does this test verify the falsification condition? Respond: yes/no/unclear."
- Subagent 2: "What property does this test actually verify? Compare to the falsification."
- Subagent 3: "Can you construct an input that satisfies the falsification condition but passes the test?"

The three prompts are deliberately DIFFERENT (not the same prompt 3x) to further
decorrelate responses. The third prompt is adversarial — it asks the model to BREAK
the test, not confirm it.

**Falsification**: A majority vote aggregates verdicts from the same conversation
context (e.g., 3 sequential LLM calls within a single Agent), producing correlated
outputs.
**Verification**: V:PROP — verify that majority_vote implementation spawns N independent
Agents. V:ARCH — no shared mutable state between verdict contexts.

---

#### ADR-WITNESS-004: Subagent-Based Challenge over Same-Context Majority Vote

**Traces to**: prompt-optimization skill §trajectory-dynamics, APP-ADR-039
**Stage**: 1

##### Problem
The Go CLI's 3-run majority vote (APP-INV-055) calls the same model 3 times with the
same prompt in the same session. The prompt-optimization theory predicts that at high
turn counts (k* < 0.5), the 3 calls produce correlated outputs because they share
the same degraded context. This is systematic evaluation bias — the majority vote
provides statistical confidence that is illusory because the samples are not independent.

##### Options
A) **Same-context majority vote** (Go CLI pattern) — 3 API calls, same prompt, same session.
B) **Multi-model majority vote** — 3 API calls to DIFFERENT models (Haiku, Sonnet, Opus).
C) **Independent subagent vote** — 3 Agent subagents, each with fresh context, different
   prompts, no access to implementation reasoning.

##### Decision
**Option C.** Independent subagent vote. Each subagent is:
- A fresh Agent tool invocation (full k* = 1.0, empty conversation history)
- Given a DIFFERENT evaluation prompt (one confirmatory, one comparative, one adversarial)
- Provided ONLY the invariant, falsification, and test code (no implementation context)

This provides genuine statistical independence because the cognitive contexts are
structurally separated, not just API-call separated. The cost is higher (3 Agent
invocations vs 3 API calls) but the independence guarantee is absolute rather than
statistical.

##### Falsification
The subagent-based approach is too slow for bulk evaluation (>10 seconds per invariant
vs <1 second for API calls). Mitigation: use the fast API-call approach for Levels 1-3
(which are mechanical, not judgment-based) and reserve subagent evaluation for Levels 0
and 4-5 (which require judgment). This gives O(1) latency for structural checks and
O(N_subagents) latency only for the semantic checks that actually benefit from independence.

---

### Gap 5: Semantic Implementation Correctness → Behavioral Boundary Testing

**The problem**: All structural layers verify that code EXISTS and LINKS to spec, but
none verify that the code DOES WHAT THE SPEC SAYS. The bilateral scan sees "INV-STORE-001
is implemented in store.rs" but never runs a test to check append-only behavior.

**The solution**: The challenge protocol's Level 4 (Practical Execution) already addresses
this — it runs the actual test. But the gap is that Level 4 only runs if a test EXISTS.
The deeper issue is: what about invariants with no test at all?

#### INV-WITNESS-011: Verification Completeness Guard

**Traces to**: INV-BILATERAL-001 (Fitness Convergence), SEED.md §3 (C5 Traceability)
**Type**: Invariant
**Stage**: 1
**Statement**: Every Stage N invariant (where N ≤ current stage) MUST have at least one
FBW at depth L2+ (test or higher). An invariant at the current stage with only L1
witnesses (comments) or no witnesses at all is a verification gap that MUST be surfaced
as a signal.

```
∀ inv with stage(inv) ≤ current_stage:
  max_depth(FBW(inv)) ≥ L2
  ∨ ∃ signal(inv, type=:signal.type/verification-gap)
```

**Falsification**: An invariant at the current stage has only L1 witnesses but no
verification gap signal has been emitted.
**Verification**: V:PROP — construct store with L1-only invariants, verify signal emitted.

---

#### INV-WITNESS-012: Behavioral Boundary Test Generation

**Traces to**: SEED.md §3 (C6 Falsifiability), INV-VERIFICATION-001
**Type**: Invariant
**Stage**: 2+
**Statement**: For every invariant with a machine-parseable falsification condition, the
system CAN generate a behavioral boundary test — a test that exercises the exact
boundary between compliant and non-compliant behavior as defined by the falsification.

```
∀ inv with parseable(inv.falsification):
  ∃ test_template = generate_boundary_test(inv.falsification)
  test_template.positive_case satisfies inv.statement
  test_template.negative_case satisfies inv.falsification
```

At Stage 2, this uses the existing verification compiler (compiler.rs) pattern detection
plus LLM-as-judge generation. At Stage 3+, this uses SMT-guided test generation.

**Falsification**: An invariant has a parseable falsification condition but the system
cannot generate a test template that distinguishes compliant from non-compliant behavior.
**Verification**: V:PROP — use compiler pattern detection to generate tests, verify they
compile and exercise the boundary.

---

### §21.9 Negative Cases (Extended)

#### NEG-WITNESS-004: No Self-Witnessing

**Traces to**: INV-WITNESS-009 (Cognitive Independence)
**Type**: Negative Case
**Statement**: The agent that PRODUCES an implementation artifact (code, test, harness)
MUST NOT be the sole challenger of its own work. At least one challenge verdict must
come from a cognitively independent context.
**Violation**: All challenge verdicts for an FBW come from the same conversation context
that produced the code being verified.

---

#### NEG-WITNESS-005: No Tautological Verification

**Traces to**: INV-WITNESS-002 (Falsification Alignment), NEG-SEED-002
**Type**: Negative Case
**Statement**: A test that always passes regardless of implementation behavior (e.g.,
`assert!(true)`, `assert!(result.is_ok())` when result is always Ok) is NOT valid
verification evidence. The FBW alignment score for such tests must be 0.
**Violation**: An FBW exists with `alignment_score > 0` for a test whose assertions
are tautological (cannot fail for any implementation of the function under test).

---

#### NEG-WITNESS-006: No Correlated Majority

**Traces to**: INV-WITNESS-010 (Decorrelated Multi-Verdict)
**Type**: Negative Case
**Statement**: A majority vote where all verdicts come from the same cognitive context
(same conversation, same k*, same basin) is NOT a valid majority. The vote provides
no more confidence than a single evaluation.
**Violation**: A majority_vote result is recorded with `confidence = 0.95` (unanimous)
but all 3 verdicts were produced within the same Agent invocation.

---

### §21.10 Failure Modes Addressed

| FM ID | Gap | Failure Mode | Solution | Detection |
|-------|-----|-------------|----------|-----------|
| FM-027 | 1 | Test names invariant but verifies nothing relevant | INV-WITNESS-002: falsification alignment | Level 0 keyword/LLM/SMT check |
| FM-028 | 2 | Test weakened (assertions loosened) to hide bug | INV-WITNESS-001: test_body_hash change | Triple-hash auto-invalidation |
| FM-029 | 3 | Kani harness proves wrong property | INV-WITNESS-008: harness-falsification binding | Alignment score for harnesses |
| FM-030 | 4 | LLM rubber-stamps bulk witnesses (sycophancy) | INV-WITNESS-009 + 010: cognitive independence | Subagent decorrelated evaluation |
| FM-031 | 5 | Code exists, links to spec, but is semantically wrong | INV-WITNESS-011: verification completeness + Level 4 | Gap signal + practical execution |
| FM-032 | 5 | No test exists for current-stage invariant | INV-WITNESS-011: completeness guard | Signal emission for L1-only |
| FM-033 | 3+5 | Generated test is tautological (assert!(true)) | NEG-WITNESS-005 + INV-WITNESS-002 | Alignment score = 0 for tautologies |

---

### §21.11 Complete Gap Closure Matrix

| Gap | Invariants | ADRs | NEGs | Status |
|-----|-----------|------|------|--------|
| 1: Test-Falsification Alignment | INV-WITNESS-002 | ADR-WITNESS-002 | NEG-WITNESS-005 | CLOSED |
| 2: Test Content Stability | INV-WITNESS-001, 005, 006 | ADR-WITNESS-001 | NEG-WITNESS-002 | CLOSED |
| 3: Harness/Property Correctness | INV-WITNESS-008 | — | NEG-WITNESS-005 | CLOSED |
| 4: Systematic Evaluation Bias | INV-WITNESS-009, 010 | ADR-WITNESS-004 | NEG-WITNESS-004, 006 | CLOSED |
| 5: Semantic Impl Correctness | INV-WITNESS-011, 012 | — | — | CLOSED |

All five gaps have at least one invariant with a falsification condition, at least one
detection mechanism, and a clear implementation path. No gap relies solely on human
review or process discipline — each has a mechanical detection component.

---

### §21.12 Uncertainty Register (Extended)

| ID | Element | Confidence | Resolution Trigger |
|----|---------|------------|-------------------|
| UNC-WITNESS-001 | Keyword extraction quality for Level 0 alignment | 0.6 | Stage 2 LLM-as-judge replaces keyword matching |
| UNC-WITNESS-002 | Whitespace normalization completeness | 0.8 | Edge case testing with macro-heavy Rust code |
| UNC-WITNESS-003 | BLAKE3 collision resistance for test bodies | 0.99 | Theoretical — 2^128 collision resistance |
| UNC-WITNESS-004 | Subagent latency for bulk evaluation | 0.7 | Benchmark: 3 subagents × 145 invariants at Stage 1 |
| UNC-WITNESS-005 | Adversarial prompt effectiveness for decorrelated vote | 0.6 | Empirical testing of "break this test" prompt quality |
| UNC-WITNESS-006 | Tautology detection accuracy (assert!(true) patterns) | 0.8 | AST-level assertion analysis at Stage 2 |
| UNC-WITNESS-007 | Boundary test generation coverage for parseable falsifications | 0.5 | Depends on compiler.rs pattern detection maturity |
