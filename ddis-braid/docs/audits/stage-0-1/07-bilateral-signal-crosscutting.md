# Bilateral + Signal + Cross-cutting — Stage 0/1 Audit
> Wave 1 Domain Audit | Date: 2026-03-17 | Agent: Opus 4.6 | Methodology: Fagan Inspection + IEEE Walkthrough

## Domain Inventory

### SIGNAL Namespace (spec/09-signal.md)
- **INVs**: INV-SIGNAL-001 through INV-SIGNAL-006 (6 invariants)
- **ADRs**: ADR-SIGNAL-001 through ADR-SIGNAL-005 (5 ADRs)
- **NEGs**: NEG-SIGNAL-001 through NEG-SIGNAL-003 (3 negative cases)

### BILATERAL Namespace (spec/10-bilateral.md)
- **INVs**: INV-BILATERAL-001 through INV-BILATERAL-005 (5 invariants)
- **ADRs**: ADR-BILATERAL-001 through ADR-BILATERAL-010 (10 ADRs)
- **NEGs**: NEG-BILATERAL-001, NEG-BILATERAL-002 (2 negative cases)

### DELIBERATION Namespace (spec/11-deliberation.md)
- **INVs**: INV-DELIBERATION-001 through INV-DELIBERATION-006 (6 invariants)
- **ADRs**: ADR-DELIBERATION-001 through ADR-DELIBERATION-004 (4 ADRs)
- **NEGs**: NEG-DELIBERATION-001 through NEG-DELIBERATION-003 (3 negative cases)

### TRILATERAL Namespace (spec/18-trilateral.md)
- **INVs**: INV-TRILATERAL-001 through INV-TRILATERAL-010 (10 invariants)
- **ADRs**: ADR-TRILATERAL-001 through ADR-TRILATERAL-006 (6 ADRs)
- **NEGs**: NEG-TRILATERAL-001 through NEG-TRILATERAL-003 (3 negative cases)

### TOPOLOGY Namespace (spec/19-topology.md)
- **INVs**: INV-TOPOLOGY-001 through INV-TOPOLOGY-016 (16 invariants)
- **ADRs**: ADR-TOPOLOGY-001 through ADR-TOPOLOGY-007 (7 ADRs)
- **NEGs**: NEG-TOPOLOGY-001 through NEG-TOPOLOGY-005 (5 negative cases)

### COHERENCE Namespace (spec/20-coherence.md)
- **INVs**: INV-COHERENCE-001 through INV-COHERENCE-013 (13 invariants)
- **ADRs**: ADR-COHERENCE-001 through ADR-COHERENCE-003 (3 ADRs)
- **NEGs**: NEG-COHERENCE-001 through NEG-COHERENCE-003 (3 negative cases)

---

## PART A: DOMAIN AUDIT

### A.1 SIGNAL Namespace (spec/09-signal.md)

**Spec elements**: 6 INV, 5 ADR, 3 NEG

### FINDING-001: Seven of eight SignalType variants are commented out
Severity: HIGH
Type: UNIMPLEMENTED
Sources: spec/09-signal.md:18-26 (8 SignalType variants) vs crates/braid-kernel/src/signal.rs:43-56
Evidence: The spec defines `SignalType = Confusion | Conflict | UncertaintySpike | ResolutionProposal | DelegationRequest | GoalDrift | BranchReady | DeliberationTurn`. The code defines only `Confusion` as constructible; the remaining seven are commented out (lines 49-56: `// Conflict,`, `// UncertaintySpike,` etc.).
Impact: INV-SIGNAL-006 (Taxonomy Completeness) is structurally unsatisfied -- the exhaustive signal dispatch match (line 138-140) covers only one of eight required dispatch targets. The entire signal routing subsystem is dormant.

### FINDING-002: Signal Severity type diverges from spec's discrete lattice
Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/09-signal.md:24-25 (`Severity = Low | Medium | High | Critical` with total order) vs crates/braid-kernel/src/signal.rs:64-92 (`Severity(f64)`)
Evidence: Spec defines a 4-element discrete lattice: `Low < Medium < High < Critical`. The implementation uses a continuous `f64` with classification by threshold (is_high >= 0.7, is_medium >= 0.3). The spec's `Critical` severity level is not distinguished from `High` in the code -- there is no `is_critical()` method.
Impact: INV-SIGNAL-004 (Severity-Ordered Routing) specifies that `Critical` triggers TUI alerts distinct from `High`. The implementation cannot distinguish these.

### FINDING-003: Signal struct missing `target` field required by spec
Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/09-signal.md:109-115 (`Signal { signal_type, source, target, severity, timestamp }`) vs crates/braid-kernel/src/signal.rs:99-110
Evidence: The spec Signal struct includes a `target: EntityId` field. The implementation struct omits it entirely -- `Signal` has `source` but no `target`. The `target` is needed for directed signal routing.
Impact: Signal routing cannot distinguish "who should receive this signal," which is required for INV-SIGNAL-003 (Subscription Completeness) pattern matching.

### FINDING-004: Subscription system entirely unimplemented
Severity: HIGH
Type: UNIMPLEMENTED
Sources: spec/09-signal.md:117-123 (Subscription struct) vs crates/braid-kernel/src/signal.rs (no Subscription type)
Evidence: The spec defines `Subscription { pattern, callback, debounce }` and the state machine includes `SUBSCRIBE(...)` transitions. No Subscription type, pattern matching, or callback system exists in the code. ADR-SIGNAL-003 (debounce policy) has no implementation counterpart.
Impact: INV-SIGNAL-003 (Subscription Completeness) is completely unsatisfied. The entire event-driven pub/sub architecture described in the spec does not exist.

**SIGNAL implementation summary**:
- INV: 6 total, 1 partially implemented (INV-SIGNAL-002 Confusion), 5 unimplemented
- ADR: 5 total, 1 reflected in code (ADR-SIGNAL-001 signal-as-datom), 4 unimplemented
- NEG: 3 total, 0 enforced

---

### A.2 BILATERAL Namespace (spec/10-bilateral.md)

**Spec elements**: 5 INV, 10 ADR, 2 NEG

### FINDING-005: Boundary enum has 2 variants instead of spec's 4
Severity: HIGH
Type: DIVERGENCE
Sources: spec/10-bilateral.md:26-33 (4 boundaries: IntentToSpec, SpecToSpec, SpecToImpl, ImplToBehavior) vs crates/braid-kernel/src/bilateral.rs:102-108 (2 variants: IntentSpec, SpecImpl)
Evidence: The spec's divergence measure D(spec, impl) = sum over 4 boundaries. The implementation has only 2 Boundary variants. The SpecToSpec boundary (logical contradictions within the spec) and ImplToBehavior boundary (behavioral testing) are absent as Boundary enum variants, although some functionality for these is handled elsewhere (coherence gate for SpecToSpec, test datoms for ImplToBehavior).
Impact: ADR-BILATERAL-002 (Divergence Metric as Weighted Boundary Sum) cannot produce a 4-boundary weighted sum. The fitness function partially compensates but the architectural mismatch limits future expansion.

### FINDING-006: F(S) fitness function fully implemented with 7 components
Severity: INFO
Type: GAP (positive finding)
Sources: spec/10-bilateral.md:43-67 vs crates/braid-kernel/src/bilateral.rs:77-92 (weight constants), 353-408 (compute_fitness)
Evidence: The 7 weights (V=0.18, C=0.18, D=0.18, H=0.13, K=0.13, I=0.08, U=0.12) match exactly between spec and code. The compute_fitness function calculates all 7 components. Depth-weighted V and C variants go beyond the spec.
Impact: This is the strongest implementation in this domain.

### FINDING-007: CC-3 (Intent alignment) staleness detection unimplemented
Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: spec/10-bilateral.md:218-240 (cc3_staleness_threshold, CC3StaleWarning signal) vs crates/braid-kernel/src/bilateral.rs (no cc3_staleness_threshold constant or warning emission)
Evidence: INV-BILATERAL-002 specifies that CC-3 is carried forward from the last FULL_CYCLE with a staleness threshold (default 10 AUTO_CYCLEs). After exceeding the threshold, a `CC3StaleWarning` signal must be emitted. The implementation evaluates CC-3 as a static pass-through with a note "single-agent vacuously true" but has no cycle counter, staleness tracking, or warning emission.
Impact: For multi-agent scenarios, CC-3 staleness could go undetected indefinitely.

### FINDING-008: Bilateral scan is forward+backward but only over SpecImpl boundary
Severity: MEDIUM
Type: GAP
Sources: spec/10-bilateral.md:76-88 (FORWARD_SCAN, BACKWARD_SCAN covering all boundaries) vs crates/braid-kernel/src/bilateral.rs:725-838 (forward_scan and backward_scan)
Evidence: Both forward_scan and backward_scan operate only on the SpecImpl boundary (`:impl/implements` references). The IntentSpec boundary (`:spec/traces-to`) is handled separately by the trilateral Phi metric but not within the bilateral scan result structure. The spec expects signals to be emitted for each gap found.
Impact: INV-BILATERAL-003 (Bilateral Symmetry) is partially satisfied -- forward and backward scans exist but only over one of the four boundaries.

### FINDING-009: Spectral certificate is a substantial extension beyond spec
Severity: INFO
Type: MISALIGNMENT (benign)
Sources: spec/10-bilateral.md (no spectral certificate mentioned) vs crates/braid-kernel/src/bilateral.rs:253-285 (SpectralCertificate with Fiedler, Cheeger, persistence, Ricci curvature, Renyi entropy)
Evidence: The implementation includes a rich spectral analysis pipeline (Fiedler value, Cheeger constant, persistent homology barcode, Ollivier-Ricci curvature, Renyi entropy spectrum) that is not specified anywhere in spec/10-bilateral.md. This appears to be an implementation-driven extension.
Impact: Positive -- the implementation exceeds the spec. But traceability (C5) is violated since these features lack spec elements.

**BILATERAL implementation summary**:
- INV: 5 total, 3 implemented (INV-BILATERAL-001 fitness monotonicity, INV-BILATERAL-002 partial CC evaluation, INV-BILATERAL-005 test-as-datoms partial), 2 partially (INV-BILATERAL-003 symmetry, INV-BILATERAL-004 residuals)
- ADR: 10 total, 5 reflected in code (ADR-BILATERAL-001 weights, ADR-BILATERAL-002 metric, ADR-BILATERAL-005 taxonomy, ADR-BILATERAL-006 coherence as problem, ADR-BILATERAL-007 formalism mapping), 5 not directly implemented
- NEG: 2 total, 1 partially enforced (NEG-BILATERAL-001 via fitness clamping), 1 unenforced (NEG-BILATERAL-002)

---

### A.3 DELIBERATION Namespace (spec/11-deliberation.md)

**Spec elements**: 6 INV, 4 ADR, 3 NEG

### FINDING-010: DeliberationStatus derives total Ord, contradicting spec's partial order
Severity: HIGH
Type: CONTRADICTION
Sources: spec/11-deliberation.md:150-172 (PartialOrd with Stalled/Decided incomparable) vs crates/braid-kernel/src/deliberation.rs:55 (`#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]`)
Evidence: The spec explicitly states: "Stalled and Decided are incomparable" (line 163: `(Stalled, Decided) | (Decided, Stalled) => None`) and provides a hand-written PartialOrd. The implementation derives total `Ord`, which makes `Stalled < Decided` because of enum variant ordering. The spec even warns "do NOT derive Ord" (line 132).
Impact: NEG-DELIBERATION-003 (No Backward Lifecycle Transition) relies on the partial order to detect invalid transitions. With total Ord, `Stalled < Decided` allows transitions that the spec considers incomparable (requiring escalation). The test at line 725-728 asserts `Decided < Stalled`, which contradicts the spec's incomparability.

### FINDING-011: Stability guard (INV-DELIBERATION-002) unimplemented
Severity: HIGH
Type: UNIMPLEMENTED
Sources: spec/11-deliberation.md:89-106 (6 stability guard conditions) vs crates/braid-kernel/src/deliberation.rs:255-329 (decide function)
Evidence: The spec requires 6 conditions before a DECIDE transition: status refined, thread active, parent confidence >= 0.6, coherence score >= 0.6, no unresolved conflicts, commitment weight >= 0.7. The `decide()` function performs no guard checks -- it unconditionally creates decision datoms. No `StabilityError` type exists.
Impact: NEG-DELIBERATION-001 (No Decision Without Stability Guard) is completely unenforced. Premature crystallization (FM-004) is undetectable.

### FINDING-012: Commitment weight (INV-DELIBERATION-005) not computed
Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: spec/11-deliberation.md:299-311 vs crates/braid-kernel/src/deliberation.rs
Evidence: The spec defines `w(decision) = |{d' in S : decision in causes*(d')}|` -- the size of the forward causal cone. No commitment weight computation exists in the code. The Decision struct in the spec includes `commitment_weight: f64`; the implementation's `decide()` function creates no such attribute.
Impact: Decision reversibility analysis is impossible without commitment weight tracking.

**DELIBERATION implementation summary**:
- INV: 6 total, 2 implemented (INV-DELIBERATION-001 lifecycle partial, INV-DELIBERATION-003 precedent), 4 unimplemented
- ADR: 4 total, 2 reflected in code (ADR-DELIBERATION-001 three entities, ADR-DELIBERATION-003 precedent), 2 unimplemented
- NEG: 3 total, 0 enforced

---

### A.4 TRILATERAL Namespace (spec/18-trilateral.md)

**Spec elements**: 10 INV, 6 ADR, 3 NEG

### FINDING-013: Trilateral implementation is the most complete in this domain
Severity: INFO
Type: GAP (positive finding)
Sources: spec/18-trilateral.md vs crates/braid-kernel/src/trilateral.rs (4000+ LOC)
Evidence: The trilateral module implements: LIVE projections (INV-TRILATERAL-001), Phi divergence metric (INV-TRILATERAL-002), formality gradient (INV-TRILATERAL-003), attribute namespace partition (INV-TRILATERAL-005), Datalog expressibility (INV-TRILATERAL-006), self-bootstrap (INV-TRILATERAL-007), ISP bypass detection (INV-TRILATERAL-008), coherence completeness with beta_1 (INV-TRILATERAL-009), persistent cohomology (INV-TRILATERAL-010). There are 127 proptest functions generated by the coherence compiler, plus dedicated tests.
Impact: This is the most thoroughly implemented module in the audited domain.

### FINDING-014: Verification matrix lists 10 TRILATERAL INVs but spec/17-crossref.md Appendix A lists 7
Severity: LOW
Type: STALE
Sources: spec/16-verification.md:247-260 (10 INV rows) vs spec/17-crossref.md:224 (TRILATERAL: 7 INV, 3 ADR, 3 NEG)
Evidence: The verification matrix (spec/16-verification.md) lists INV-TRILATERAL-001 through INV-TRILATERAL-010, which is 10 invariants. But the cross-reference summary (spec/17-crossref.md Appendix A) states TRILATERAL has only 7 INV. The total in the summary is 145 INV. If TRILATERAL has 10, the total should be 148.
Impact: Spec element counts are internally inconsistent. This triggers FM-008 (Derived Quantity Staleness).

---

### A.5 TOPOLOGY Namespace (spec/19-topology.md)

**Spec elements**: 16 INV, 7 ADR, 5 NEG

### FINDING-015: TOPOLOGY namespace entirely unimplemented
Severity: HIGH
Type: UNIMPLEMENTED
Sources: spec/19-topology.md (entire file, ~783 lines) vs crates/ (no topology.rs module)
Evidence: The spec defines 16 invariants (INV-TOPOLOGY-001 through INV-TOPOLOGY-016), 7 ADRs, 5 NEGs, complete algorithms (COLD_START, Tier NM Transition Protocol), and a datom schema. No `topology.rs` file exists anywhere in the crate. Topology concepts appear only in test fixtures (promote.rs line 446: `target_element_id: "INV-TOPOLOGY-001"`) and cross-namespace tests.
Impact: Expected -- spec/19-topology.md is Stage 3, and the project is at Stage 0. However, TOPOLOGY elements do not appear in spec/17-crossref.md Appendix A counts or the verification matrix, creating a spec completeness gap (see FINDING-017).

---

### A.6 COHERENCE Namespace (spec/20-coherence.md)

**Spec elements**: 13 INV, 3 ADR, 3 NEG

### FINDING-016: Coherence density matrix and Bures metric not implemented
Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: spec/20-coherence.md:29-137 (agreement function, density matrix, von Neumann entropy) vs crates/braid-kernel/src/trilateral.rs
Evidence: The spec defines a coherence density matrix rho with agreement functions per resolution mode, per-attribute density matrices, and the Bures distance metric. The implementation has `von_neumann_entropy()` (trilateral.rs:562) but computes it over the graph Laplacian (entity reference structure), not over the agreement-weighted density matrix specified in spec/20-coherence.md. There is no `agreement()` function, no `isp_density_matrix()`, no `bures_distance_to_pure()` as specified in the Stage 1 implementation path (spec/20-coherence.md:614-619).
Impact: INV-COHERENCE-001 through INV-COHERENCE-013 are all unimplemented. The von Neumann entropy in the code computes something structurally different from what the spec prescribes. The code's entropy is over graph connectivity; the spec's entropy is over perspective agreement.

---

## PART B: CROSS-CUTTING COHERENCE

### B.1 README.md Master Index Accuracy

### FINDING-017: spec/README.md does not include TOPOLOGY or COHERENCE in element counts
Severity: MEDIUM
Type: STALE
Sources: spec/README.md:77-83 ("14 namespaces" for INV/ADR/NEG) vs actual spec files (TOPOLOGY has 16 INV, 7 ADR, 5 NEG; COHERENCE has 13 INV, 3 ADR, 3 NEG)
Evidence: spec/README.md says "14 namespaces" with INV/ADR/NEG elements. The actual count is 22 namespaces (per braid-seed context: "358 elements, 22 namespaces"). The README element count section says "See 17-crossref.md for the complete element count summary" but that summary (Appendix A, 145 total INV) also excludes TOPOLOGY and COHERENCE. The preamble lists 16 namespaces (spec/00-preamble.md:30-35, naming only the original set without TOPOLOGY, COHERENCE, or FOUNDATION).
Impact: Master index is stale. An agent loading spec/README.md would not discover TOPOLOGY or COHERENCE namespaces.

### B.2 Preamble Convention Adherence

### FINDING-018: TOPOLOGY and COHERENCE namespaces not listed in preamble conventions
Severity: LOW
Type: STALE
Sources: spec/00-preamble.md:47-48 (namespace list: `STORE, LAYOUT, SCHEMA, QUERY, RESOLUTION, HARVEST, SEED, MERGE, SYNC, SIGNAL, BILATERAL, DELIBERATION, GUIDANCE, BUDGET, INTERFACE`) vs spec/19-topology.md, spec/20-coherence.md
Evidence: The preamble namespace list (spec/00-preamble.md:47-48) does not include TOPOLOGY, COHERENCE, TRILATERAL, FOUNDATION, UNCERTAINTY, or VERIFICATION. These were added in later sessions but the preamble was not updated.
Impact: Element ID format validation using the preamble's namespace list would reject IDs like INV-TOPOLOGY-001 or INV-COHERENCE-001.

### B.3 Cross-Reference Consistency (spec/17-crossref.md)

### FINDING-019: Cross-reference totals are inconsistent with actual spec file contents
Severity: MEDIUM
Type: STALE
Sources: spec/17-crossref.md Appendix A (Total: 145 INV, 136 ADR, 50 NEG = 331) vs actual
Evidence: Appendix A counts 145 INV and 331 total. But spec/19-topology.md adds 16 INV + 7 ADR + 5 NEG = 28 elements. spec/20-coherence.md adds 13 INV + 3 ADR + 3 NEG = 19 elements. If these were included, the total would be 145+16+13=174 INV, 136+7+3=146 ADR, 50+5+3=58 NEG = 378 total. The verification matrix (spec/16-verification.md) lists 145 INVs and does not include TOPOLOGY or COHERENCE invariants.
Impact: Any agent relying on the cross-reference for completeness tracking will under-count by 47 elements.

### B.4 ADRS.md Alignment

### FINDING-020: ADRS.md does not include TOPOLOGY or COHERENCE entries
Severity: LOW
Type: GAP
Sources: docs/design/ADRS.md (13 categories listed in TOC) vs spec/19-topology.md (7 ADRs), spec/20-coherence.md (3 ADRs)
Evidence: The ADRS.md Table of Contents lists 13 categories. No TOPOLOGY or COHERENCE category exists. The spec files define ADR-TOPOLOGY-001 through ADR-TOPOLOGY-007 and ADR-COHERENCE-001 through ADR-COHERENCE-003, none of which appear in ADRS.md with forward/backward annotations.
Impact: Traceability (C5) is broken for these 10 ADRs -- they exist in the spec files but have no design decision index entry.

### B.5 FAILURE_MODES.md Coverage

### FINDING-021: FM-004 (Cascading Incompleteness) mitigated but not fully verified
Severity: MEDIUM
Type: GAP
Sources: docs/design/FAILURE_MODES.md:88 (FM-004 mapped to INV-BILATERAL-001 and INV-DELIBERATION-002) vs implementation
Evidence: FM-004's acceptance criterion is "F(S) coverage component detects >= 99% of spec gaps within one bilateral cycle." F(S) is implemented and the bilateral scan detects gaps. However, INV-DELIBERATION-002 (stability guard), the second defense mechanism cited, is entirely unimplemented (FINDING-011). This means the "cascading" part of cascading incompleteness -- where premature decisions compound errors -- has no guard.
Impact: One of the two named defense mechanisms for the most severe failure mode (S0) is absent.

### B.6 Reconciliation Taxonomy Implementation Coverage

### FINDING-022: Only 2 of 8 divergence types have detection mechanisms in code
Severity: HIGH
Type: GAP
Sources: SEED.md section 6 (8 divergence types) vs code
Evidence: The Reconciliation Taxonomy defines 8 divergence types. Implementation status:
1. **Epistemic** (Store vs agent knowledge): IMPLEMENTED -- harvest gap detection works
2. **Structural** (Implementation vs spec): IMPLEMENTED -- bilateral forward/backward scan, trilateral Phi
3. **Consequential** (Current state vs future risk): NOT IMPLEMENTED -- no uncertainty tensor computation
4. **Aleatory** (Agent vs agent): NOT IMPLEMENTED -- merge conflict detection exists but conflict signals not emitted
5. **Logical** (Invariant vs invariant): PARTIAL -- coherence gate has Tier 1 (exact) and Tier 2 (logical pattern) but not the full 5-tier engine
6. **Axiological** (Implementation vs goals): NOT IMPLEMENTED -- no goal-drift signal
7. **Temporal** (Agent frontier vs agent frontier): NOT IMPLEMENTED -- no frontier comparison
8. **Procedural** (Agent behavior vs methodology): PARTIAL -- M(t) score tracks methodology adherence
Impact: The reconciliation taxonomy is the conceptual backbone of DDIS. Having only 2.5 of 8 types functionally implemented means the majority of divergence types go undetected.

### B.7 TOPOLOGY and COHERENCE Integration Assessment

### FINDING-023: TOPOLOGY and COHERENCE specs appear bolted-on without cross-reference integration
Severity: MEDIUM
Type: MISALIGNMENT
Sources: spec/19-topology.md, spec/20-coherence.md vs spec/17-crossref.md, spec/16-verification.md, spec/00-preamble.md
Evidence: Neither TOPOLOGY nor COHERENCE appears in: (a) the preamble namespace list, (b) the verification matrix, (c) the cross-reference element count summary, (d) the invariant dependency graph, (e) the stage mapping tables, or (f) the ADRS.md index. They have their own internal cross-reference sections (spec/19-topology.md:753-768 and spec/20-coherence.md:639-651) but are not woven into the integration fabric. Spec/19-topology.md references "INS-005" (line 142) and "INV-TOPOLOGY-015/016" (lines 744-749) which are defined inline in the spec but not in the verification matrix.
Impact: These namespaces are structurally disconnected from the specification's own coherence tracking machinery. An agent performing completeness analysis via spec/17-crossref.md would not know they exist.

### B.8 Contradiction Detection Tiers

### FINDING-024: Only 2 of 5 contradiction detection tiers implemented
Severity: HIGH
Type: UNIMPLEMENTED
Sources: SEED.md section 6 (5-tier contradiction engine) vs crates/braid-kernel/src/coherence.rs
Evidence: The spec (and SEED.md) describe a 5-tier contradiction engine: Tier 1 (exact), Tier 2 (logical), Tier 3 (semantic/SAT), Tier 4 (pragmatic), Tier 5 (axiological). The coherence.rs module implements only Tier 1 (exact duplicate value detection) and Tier 2 (pattern-based logical contradiction on spec statements). Tiers 3-5 are not present. The existing Go CLI had a 5-tier implementation; Braid's is 2-tier.
Impact: Logical contradictions that require SAT solving, semantic analysis, or axiological judgment are undetectable.

---

## Quantitative Summary

### Signal (spec/09-signal.md)
| Metric | Count |
|--------|-------|
| Total INVs | 6 |
| Implemented | 1 (partial: INV-SIGNAL-002) |
| Unimplemented | 5 |
| Total ADRs | 5 |
| Reflected in code | 1 (ADR-SIGNAL-001) |
| Total NEGs | 3 |
| Enforced | 0 |

### Bilateral (spec/10-bilateral.md)
| Metric | Count |
|--------|-------|
| Total INVs | 5 |
| Implemented | 2 (INV-BILATERAL-001 fitness, INV-BILATERAL-005 partial) |
| Partially implemented | 2 (INV-BILATERAL-002 CC evaluation, INV-BILATERAL-003 symmetry) |
| Unimplemented | 1 (INV-BILATERAL-004 residual documentation) |
| Total ADRs | 10 |
| Reflected in code | 5 |
| Total NEGs | 2 |
| Enforced | 1 (partial: NEG-BILATERAL-001) |

### Deliberation (spec/11-deliberation.md)
| Metric | Count |
|--------|-------|
| Total INVs | 6 |
| Implemented | 2 (INV-DELIBERATION-001 partial lifecycle, INV-DELIBERATION-003 precedent) |
| Unimplemented | 4 |
| Total ADRs | 4 |
| Reflected in code | 2 |
| Total NEGs | 3 |
| Enforced | 0 |

### Trilateral (spec/18-trilateral.md)
| Metric | Count |
|--------|-------|
| Total INVs | 10 |
| Implemented | 8 (001-003, 005-009) |
| Partially implemented | 1 (INV-TRILATERAL-004 convergence) |
| Unimplemented | 1 (INV-TRILATERAL-010 persistent cohomology stub) |
| Total ADRs | 6 |
| Reflected in code | 4 |
| Total NEGs | 3 |
| Enforced | 3 |

### Topology (spec/19-topology.md)
| Metric | Count |
|--------|-------|
| Total INVs | 16 |
| Implemented | 0 |
| Total ADRs | 7 |
| Reflected in code | 0 |
| Total NEGs | 5 |
| Enforced | 0 |

### Coherence (spec/20-coherence.md)
| Metric | Count |
|--------|-------|
| Total INVs | 13 |
| Implemented | 0 (von Neumann entropy exists but computes different thing) |
| Total ADRs | 3 |
| Reflected in code | 0 |
| Total NEGs | 3 |
| Enforced | 0 |

### Cross-cutting (Uncertainty, Verification, Crossref)
| Metric | Count |
|--------|-------|
| Uncertainty markers tracked | 13 (all documented in spec/15-uncertainty.md) |
| Verification matrix coverage | 145 INVs covered, excludes TOPOLOGY (16) and COHERENCE (13) |
| Cross-reference index | Stale -- excludes 2 namespaces, 47 elements |

### Grand Total for Audited Domain
| Metric | Count |
|--------|-------|
| Total INVs | 56 |
| Implemented | 13 |
| Partially implemented | 3 |
| Unimplemented | 40 |
| Total ADRs | 35 |
| Reflected in code | 12 |
| Total NEGs | 19 |
| Enforced | 4 |

---

## Domain Health Assessment

**Strongest aspect**: The **trilateral coherence model** (spec/18-trilateral.md) is thoroughly implemented with 8 of 10 invariants satisfied, 127 generated proptest functions, von Neumann entropy computation, beta_1 homology detection, and self-bootstrap verification. The **F(S) fitness function** in bilateral.rs is faithfully implemented with all 7 weights matching the spec, plus depth-weighted extensions that exceed spec requirements. The spectral certificate (Fiedler, Cheeger, Ricci curvature, Renyi entropy) is an impressive implementation-driven extension.

**Most concerning gap**: The **reconciliation taxonomy** -- the conceptual spine of DDIS -- has detection mechanisms for only 2.5 of its 8 divergence types. The signal system that routes detected divergence to resolution mechanisms has 7 of 8 signal types commented out. The deliberation system, which handles conflicts that automated mechanisms cannot resolve, lacks its primary safety mechanism (stability guard). The contradiction detection engine implements 2 of 5 tiers. Together, these gaps mean the system can detect structural divergence (spec vs impl) and epistemic divergence (harvest gaps) reasonably well, but is blind to consequential, aleatory, axiological, and temporal divergence -- and has no structured mechanism to resolve the conflicts it does detect beyond simple lattice merge. The TOPOLOGY and COHERENCE namespaces are entirely spec-only with zero code, and are structurally disconnected from the specification's own integration machinery (cross-references, verification matrix, preamble).
