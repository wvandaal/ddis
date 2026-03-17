# Axiological Alignment -- Stage 0/1 Synthesis Audit
> Wave 2 Cross-Cutting Synthesis | Date: 2026-03-17 | Agent: Opus 4.6 | Methodology: Cross-domain synthesis of 124 Wave 1 findings

## 1. Goal-by-Goal Alignment Matrix

The goals are extracted from SEED.md sections 1 and 2. For each, I trace to spec elements and assess implementation status.

### SEED.md Section 1: "Verifiable Coherence"

| Goal | Spec Elements | Implementation Status | Assessment |
|------|--------------|----------------------|------------|
| **G1: Verifiable coherence between intent, spec, implementation, and behavior** | INV-BILATERAL-001..005, INV-TRILATERAL-001..010, ADR-BILATERAL-002..010 | bilateral.rs (950+ LOC), trilateral.rs (800+ LOC) compute F(S), Phi, spectral certificates, Renyi entropy. Coherence gate in coherence.rs prevents contradictions at transact time (Tier 1, Tier 2 only). | PARTIALLY IMPLEMENTED. Forward checking (spec->impl) works. Backward checking (impl->spec) depends on bilateral scan which exists as code but lacks the 5-tier contradiction engine (only Tiers 1-2 of 5 implemented). |
| **G2: Verifiable at any point, with formal justification** | F(S) fitness function, 7-component weighted sum | bilateral.rs computes F(S) with all 7 weights. `braid bilateral` command exposes it. Spectral certificates (Fiedler, Cheeger, Renyi entropy) implemented. | SUBSTANTIALLY IMPLEMENTED. F(S) is computable, but its inputs (especially Coverage and Contradiction components) depend on mechanisms that are partially stubbed. |
| **G3: True across people, AI agents, and time** | Harvest/seed lifecycle, CRDT merge, frontier tracking | harvest.rs, seed.rs, merge.rs all implemented. Store is a G-Set with set-union merge. Frontier tracking exists in Store::frontier(). | IMPLEMENTED FOR SINGLE-AGENT. Multi-agent coordination (merge across agents, frontier comparison, sync barriers) is structurally present but untested in multi-agent deployment. |
| **G4: True because of system structure, not process obligation** | Coherence gate (transact-time prevention), guidance injection, dynamic CLAUDE.md | coherence.rs gates at transact time. Guidance footer injected on every CLI command via `maybe_inject_footer()`. agent_md.rs generates dynamic CLAUDE.md. | SUBSTANTIALLY IMPLEMENTED. The structural enforcement exists (coherence gate, guidance footer). But the prevention is incomplete: only 2 of 5 contradiction tiers are implemented. |

### SEED.md Section 2: "The Divergence Problem" -- Eight Divergence Types

| Divergence Type | Detection Mechanism (spec) | Detection Mechanism (impl) | Resolution Mechanism (spec) | Resolution Mechanism (impl) | Gap |
|-----------------|---------------------------|---------------------------|----------------------------|----------------------------|-----|
| **Epistemic** (Store vs. agent knowledge) | Harvest gap detection | harvest.rs: `classify_candidates()`, FP/FN tracking, `compute_harvest_quality()` | Harvest (promote to datoms) | harvest.rs: `harvest_pipeline()` with 68 tests | LOW gap. Best-implemented divergence type. |
| **Structural** (Implementation vs. spec) | Bilateral scan / drift | bilateral.rs: forward/backward scan, F(S) computation | Associate + guided reimplementation | seed.rs: `associate()` + `assemble()`. guidance.rs: `derive_actions()` | MEDIUM gap. Detection works. Resolution guidance exists but is advisory, not enforced. |
| **Consequential** (Current state vs. future risk) | Uncertainty tensor | NOT IMPLEMENTED. No sigma_e/sigma_a/sigma_c computation anywhere. | Guidance (redirect before action) | guidance.rs provides steering but not risk-based -- steers by methodology score, not uncertainty tensor. | HIGH gap. Uncertainty tensor (UA-001, UA-002) is unimplemented. |
| **Aleatory** (Agent vs. agent) | Merge conflict detection | merge.rs: `detect_merge_conflicts()` works. resolution.rs: `has_conflict()` with 6-condition predicate. | Deliberation + Decision | deliberation.rs: types and datom creation exist. But NO stability guard (CR-005). No dispatch from merge to deliberation. | HIGH gap. Types exist but pipeline is unwired. |
| **Logical** (Invariant vs. invariant) | Contradiction detection (5-tier) | coherence.rs: Tiers 1-2 only. Tiers 3-5 (SAT, semantic, axiological) NOT IMPLEMENTED. | Deliberation + new ADR | deliberation.rs can create deliberation entities, but no automatic routing from contradiction detection. | HIGH gap. 2/5 tiers. No SAT. No semantic analysis. No axiological checking. |
| **Axiological** (Implementation vs. goals) | Fitness function / goal-drift signal | bilateral.rs: F(S) computed. But goal-drift signal (GoalDrift SignalType) is COMMENTED OUT. | Human review + ADR revision | No mechanism to surface axiological divergence to human. No GoalDrift signal dispatch. | SEVERE gap. This is the divergence type this audit specifically examines, and it has the weakest implementation. |
| **Temporal** (Agent frontier vs. agent frontier) | Frontier comparison | Store::frontier() exists. `verify_frontier_advancement()` in merge.rs. But no cross-agent frontier comparison. | Sync barrier | sync.rs referenced in ADRS but no sync.rs module exists. No SYNC-BARRIER implementation. | HIGH gap. Single-agent frontier works. Multi-agent: nothing. |
| **Procedural** (Agent behavior vs. methodology) | Drift detection (access log) | guidance.rs: `compute_methodology_score()` and `telemetry_from_store()`. But no access log (AS-007). | Dynamic CLAUDE.md | agent_md.rs generates dynamic CLAUDE.md. `braid seed --inject` works. | MEDIUM gap. Methodology scoring works. Access log not implemented. Dynamic CLAUDE.md works but without empirical drift data. |

**Summary**: Of 8 divergence types, 2 are well-covered (Epistemic, Structural), 2 are partially covered (Procedural, minor parts of Aleatory/Logical), and 4 have severe gaps (Consequential, Axiological, Temporal, full Aleatory+Logical).

---

## 2. Design Rationale Validity Check (SEED.md Section 11)

| Rationale | Still Valid? | Implementation Fidelity |
|-----------|-------------|------------------------|
| **"Why append-only?"** -- Mutable state causes bugs; append-only gives time-travel. | YES. Store is genuinely append-only. store.rs enforces Op::Assert/Op::Retract with no mutation. | HIGH. Datom, Store, Op all correct. |
| **"Why EAV?"** -- Ontology evolves; schema crystallizes from usage. | YES. Schema-as-data works. 86 distinct attributes emergent from usage. | HIGH. Schema derived from store datoms. |
| **"Why Datalog?"** -- Graph join semantics for traceability chains. | PARTIALLY INVALID. The evaluator is a single-pass nested-loop join, not actual Datalog with fixpoint. It cannot express recursive graph traversal (goal -> INV -> impl -> test). | LOW. The rationale specifically calls out traceability chain queries. The evaluator cannot express transitive closure, which is exactly what those chains require. |
| **"Why not vector DB?"** -- Need verification substrate, not retrieval heuristic. | YES. The datom store does provide structural verification (coherence gate, bilateral scan). | HIGH. Coherence gate is genuinely structural, not heuristic. |
| **"Why per-attribute resolution?"** -- Different attributes have different semantics. | PARTIALLY INVALID in practice. resolution.rs line 159: `ResolutionMode::Lattice => resolve_lww(&active)`. Lattice mode falls back to LWW silently. | LOW. The rationale says "forcing one policy loses information." But the implementation forces LWW on everything because lattice resolution is stubbed. |
| **"Why formalize at all?"** -- System promises verifiable coherence, so its own coherence must be verifiable. | YES. The formalization exists (358 spec elements, 22 namespaces). | MEDIUM. Formalization exists but verification is incomplete (2/5 contradiction tiers, no uncertainty tensor, axiological detection absent). |
| **"Why self-bootstrap?"** -- Integrity, bootstrapping data, validation. | YES. Spec elements are transacted as datoms (9314 datoms, 358 spec elements in store). | HIGH. The self-bootstrap genuinely works: spec is data. |

---

## 3. Failure Mode Mitigation Coverage Matrix

| FM-ID | Severity | Mitigation Mechanism | Implemented? | Status |
|-------|----------|---------------------|--------------|--------|
| FM-001 | S0 | Harvest gap detection | YES. harvest.rs with FP/FN calibration. | TESTABLE. 68 tests. |
| FM-002 | S1 | Provenance typing lattice | PARTIAL. ProvenanceType enum exists. Structural audit of provenance claims NOT implemented. | MAPPED but not VERIFIED. |
| FM-003 | S2 | Single-substrate store + Associate | YES. All data in one store. associate() queries full store. | TESTABLE. |
| FM-004 | S0 | Bilateral loop + fitness function | PARTIAL. F(S) computed. But coverage component depends on spec-impl traceability which requires recursive Datalog (not working). | MAPPED but verification incomplete. |
| FM-005 | S0 | Content-addressed identity | YES. EntityId from content hash. Same content = same entity. | TESTABLE. Strong. |
| FM-006 | S0 | Drift detection / frontier staleness | PARTIAL. No per-projection frontier tracking. Staleness detection is heuristic (observation timestamps), not structural (projection frontier comparison). | MAPPED but mechanism incomplete. |
| FM-007 | S0 | ADR-as-data + bilateral scan | PARTIAL. ADRs are datoms. But cross-layer consistency check not automated -- requires manual bilateral run. | MAPPED but not automatic. |
| FM-008 | S0 | Query-derived metrics | YES. Seed/status generate counts from store queries, not hardcoded numbers. | TESTABLE. This works well. |
| FM-009 | S1 | ADR traceability + contradiction detection | PARTIAL. ADRs in store. But contradiction engine (2/5 tiers) cannot detect semantic contradictions between an ADR and a guide implementation. | MAPPED but detection power insufficient. |
| FM-010 | S0 | 5-tier contradiction detection | PARTIAL. 2/5 tiers implemented. Cannot detect NEG-vs-ADR scope overlap (requires Tier 2 graph analysis -- partially present in coherence.rs, but only pattern-matching on statement text). | MAPPED but incomplete. |
| FM-011 | S1 | Verification tags as datom attributes | YES. Tags are stored as datoms. But no automated matrix-vs-body reconciliation. | PARTIALLY IMPLEMENTED. |
| FM-012 | S0 | Schema-as-data + bilateral scan | PARTIAL. Schema types in store. But no automated cross-document type name verification. | MAPPED but not automated. |
| FM-013 | S0 | Schema validation | PARTIAL. INV-SCHEMA-004 in spec. schema.rs validates on transact. But phantom types in guide prose are not checked against store schema. | MAPPED but scope limited. |
| FM-020 | S0 | Guidance injection + decision gates | PARTIAL. Guidance footer injected on CLI commands. But no DECIDE/EXPLORE classification. No decision-gate datoms. No compliance tracking. | MAPPED but mechanism not built. |

**Summary**: Of 14 documented failure modes: 3 have strong implementations (FM-001, FM-005, FM-008), 4 have partial implementations, and 7 have mechanisms that are mapped in theory but insufficiently implemented to actually prevent the failure.

---

## 4. "Axiological Drift" Assessment: Where Pragmatism Has Undermined Principle

### Drift Pattern 1: The Reconciliation Taxonomy Inversion

SEED.md Section 6 establishes the reconciliation taxonomy as the organizing principle of the entire system: "Every divergence the system detects falls into one of eight classes. Each has a characteristic boundary, detection mechanism, and resolution path." The taxonomy is not a feature list -- it is the *reason the system exists*.

The implementation inverts this: instead of building the 8-type detection/resolution pipeline, it built a single-path CLI with undifferentiated output. The signal system (signal.rs) has 7 of 8 signal types commented out. The dispatch function is a trivial match on one variant. The SEED says "if a feature under development doesn't address at least one of these divergence types, either a ninth type has been discovered or the feature doesn't belong." By this criterion, much of the implementation work (task management, graph algorithms, Renyi entropy, Ricci curvature) addresses divergence types indirectly at best.

**Severity**: This is the central axiological gap. The system's raison d'etre is divergence detection and resolution across 8 types. Only 2.5 types are functionally covered.

### Drift Pattern 2: The Datalog Promise

SEED.md Section 4 and Section 11 both emphasize Datalog as the query substrate because its join semantics "naturally express the graph queries needed for traceability (trace from goal -> invariant -> implementation -> test)." FD-003 is a foundational decision, and traceability chains are cited as the primary justification.

The evaluator in `evaluator.rs` is a single-pass sequential join over where-clauses. It has no recursion, no fixpoint, no delta tracking, no semi-naive optimization -- despite the module claiming "Semi-naive fixpoint Datalog evaluator" in its doc comment. The word "semi-naive" in the comment is aspirational, not descriptive.

This means the core traceability query -- "trace from goal through invariant through implementation to test" -- requires a transitive closure that the evaluator cannot compute. The entire rationale for choosing Datalog over SQL was this capability, and it is absent.

**Severity**: The query engine cannot deliver the specific capability cited as the justification for choosing Datalog. The design rationale for FD-003 is currently invalid given what was built.

### Drift Pattern 3: Lattice Resolution Collapse

FD-005 ("Why per-attribute conflict resolution?") argues that different attributes need different semantics: "Task status has a natural lattice. Person names do not. Forcing one resolution policy on all attributes either loses information or produces nonsense."

Yet `resolution.rs` line 158-162:
```rust
ResolutionMode::Lattice => {
    // Stage 0: lattice resolution falls back to LWW
    resolve_lww(&active)
}
```

Every attribute declared with Lattice resolution mode silently falls back to LWW. This means the diamond lattice structures documented in SR-010 (challenge-verdict, finding-lifecycle, proposal-lifecycle) -- which were specifically designed to produce coordination signals (AS-009) -- do not function. The signal-generation mechanism that was the motivation for the diamond lattice design is inert.

**Severity**: A settled ADR's core capability is stubbed. The lattice algebra that connects resolution to coordination signaling is not just deferred -- it is actively defeated by the fallback.

### Drift Pattern 4: The Anti-Drift Dead Zone

SEED.md Section 7 describes the basin competition model (GU-006): pretrained coding patterns (Basin B) constantly compete with DDIS methodology (Basin A), and without continuous corrective force the agent drifts toward Basin B within 15-20 turns. Six anti-drift mechanisms are specified.

The MCP interface (mcp.rs) -- the interface used by machine-to-machine integrations -- has no guidance injection. The `try_build_footer()` function exists in the CLI path but the MCP path returns raw JSON with no footer, no methodology reminder, no next-action guidance. An agent using the MCP interface operates in a guidance-free zone where Basin B capture is unimpeded.

**Severity**: The MCP interface creates a structural gap in the anti-drift architecture exactly where it matters most (machine-to-machine, where no human monitors the output).

---

## 5. Failure Mode Mitigation Summary

- **3 of 14 failure modes** have working mitigations (FM-001 harvest gap, FM-005 content-addressed identity, FM-008 query-derived metrics)
- **4 of 14** have partial mitigations that reduce but do not prevent the failure
- **7 of 14** have mitigations that exist only as type definitions or commented-out code

---

## VERDICT: PARTIALLY_ALIGNED

The implementation is **PARTIALLY_ALIGNED** with its stated goals. The core substrate (append-only datom store, content-addressed identity, schema-as-data, set-union merge, harvest/seed lifecycle) genuinely works and is well-tested. The self-bootstrap commitment is fulfilled: 358 spec elements are datoms in the store. The guidance injection in the CLI path is functional.

However, the system does not yet deliver on its central promise of verifiable coherence across all divergence types. It is more accurately described as "a datom store with harvest/seed lifecycle and partial coherence checking" rather than "a system that maintains verifiable coherence between intent, specification, implementation, and observed behavior."

### The Three Most Significant Axiological Gaps

1. **Reconciliation taxonomy hollowness**: The taxonomy of 8 divergence types is the system's organizing principle, but only 2.5 types have functional detection and resolution. The signal system's 7 commented-out variants are the visible symptom of an implementation that built infrastructure (store, schema, merge) without building the purpose-layer (detect divergence, classify it, resolve it). The system has the skeleton but not the nervous system.

2. **Datalog evaluator misrepresentation**: The query engine claims semi-naive fixpoint Datalog but implements single-pass nested-loop joins without recursion or fixpoint. This is not a staging issue (deferred for later) -- it is a capability gap that prevents the specific traceability queries cited as the primary justification for choosing Datalog over SQL. The design rationale in FD-003 is currently orphaned from the implementation.

3. **Axiological divergence detection is absent**: The divergence type named "axiological" -- implementation diverging from goals -- has the weakest implementation of all 8 types. The GoalDrift signal type is commented out. The fitness function exists but cannot detect goal-level misalignment (it measures structural properties, not axiological ones). The very audit this document represents -- "does what was built serve what was intended?" -- is a question the system cannot ask about itself. A system designed to detect all forms of divergence that cannot detect its own axiological drift has a first-order design gap.

### Key Distinction

The gaps are not quality failures -- the code that exists is well-structured, well-tested (1047 tests), and architecturally sound. The gaps are *priority inversions*: significant effort went into sophisticated mathematical machinery (spectral decomposition, Renyi entropy, Ricci curvature, persistent homology) while the purpose-layer mechanisms (signal routing, contradiction tiers 3-5, lattice resolution, deliberation stability guard, uncertainty tensor) remain stubbed. The implementation built excellent infrastructure for a coherence verification system while leaving the coherence verification itself incomplete.
