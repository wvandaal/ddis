# SPEC.md — Braid Specification

> **Identity**: Cleanroom-grade specification for Braid, the Rust implementation of DDIS.
> Every invariant is formally falsifiable, every ADR grounded in algebraic properties,
> every negative case stated as a safety property. The specification enables formal
> verification at implementation time — type-level guarantees, property-based testing,
> bounded model checking, and protocol model checking.
>
> **Methodology**: Three-level cleanroom refinement (Mills). Each namespace proceeds:
> Level 0 (algebraic law) → Level 1 (state machine invariant) → Level 2 (implementation contract).
> Each level is verified against the level above it. Refinement is monotonic: Level 1 preserves
> Level 0 laws; Level 2 preserves Level 1 invariants.
>
> **Self-bootstrap**: This specification is the first dataset the system will manage (C7, FD-006).
> Every element has an ID, type, and traceability to SEED.md — structured for mechanical migration
> into the datom store at Stage 0.

---

## §0. Preamble

### §0.1 Scope and Purpose

This document specifies Braid — the Rust implementation of DDIS (Decision-Driven Implementation
Specification). Braid is an append-only datom store with CRDT merge semantics, a Datalog query
engine, a harvest/seed lifecycle for durable knowledge across conversation boundaries, and a
reconciliation framework that maintains verifiable coherence between intent, specification,
implementation, and observed behavior.

The specification covers 16 namespaces organized into four waves:

- **Foundation** (Wave 1): STORE, LAYOUT, SCHEMA, QUERY, RESOLUTION — the algebraic core
- **Lifecycle** (Wave 2): HARVEST, SEED, MERGE, SYNC — session and coordination mechanics
- **Intelligence** (Wave 3): SIGNAL, BILATERAL, DELIBERATION, GUIDANCE, BUDGET, INTERFACE — steering and adaptation
- **Integration** (Wave 4): TRILATERAL, Uncertainty register, verification plan, cross-reference index

### §0.2 Conventions

#### Element ID Format

```
INV-{NAMESPACE}-{NNN}    Invariant (falsifiable claim with violation condition)
ADR-{NAMESPACE}-{NNN}    Architectural Decision Record (choice with alternatives and rationale)
NEG-{NAMESPACE}-{NNN}    Negative Case (safety property: what must NOT happen)
```

Namespaces: `STORE`, `LAYOUT`, `SCHEMA`, `QUERY`, `RESOLUTION`, `HARVEST`, `SEED`, `MERGE`, `SYNC`,
`SIGNAL`, `BILATERAL`, `DELIBERATION`, `GUIDANCE`, `BUDGET`, `INTERFACE`.

#### Three-Level Refinement

Every invariant follows the cleanroom refinement chain:

| Level | Name | Content | Verification |
|-------|------|---------|--------------|
| 0 | Algebraic Law | Mathematical objects, operations, laws. No state, no time. | Proof by construction; proptest properties |
| 1 | State Machine | State, transitions, pre/postconditions, invariants over reachable states. | Stateright/TLA+ models; Kani function contracts |
| 2 | Implementation Contract | Rust types, function signatures, typestate patterns. | Type system; proptest; Kani harnesses; Miri |

#### Verification Tags

Every invariant is tagged with one or more verification methods:

| Tag | Method | Tool | Guarantee | Cost |
|-----|--------|------|-----------|------|
| `V:TYPE` | Type system | `rustc` | Compile-time state machine correctness | Free |
| `V:PROP` | Property-based testing | `proptest` | Holds for random inputs (probabilistic) | Low |
| `V:KANI` | Bounded model checking | `kani` | Holds for all inputs up to bound (exhaustive) | Moderate |
| `V:CONTRACT` | Function contracts | `kani::requires/ensures` | Modular correctness (compositional) | Moderate |
| `V:MODEL` | Protocol model checking | `stateright` or TLA+ | Protocol safety/liveness (all reachable states) | High |
| `V:DEDUCTIVE` | Deductive verification | `verus` or `creusot` | Full functional correctness (proof) | Very high |
| `V:MIRI` | UB detection | `cargo miri test` | No undefined behavior in test paths | Low |

**Minimum requirements**:
- Every invariant MUST have at least `V:PROP`.
- Critical invariants (STORE, MERGE, SCHEMA) MUST have `V:KANI`.
- Protocol invariants (SYNC, MERGE cascade, DELIBERATION) MUST have `V:MODEL`.

#### Traceability Notation

Every element traces to source documents:
- `SEED §N` — Section N of SEED.md
- `ADRS {CAT-NNN}` — Entry in ADRS.md (e.g., `ADRS FD-001`)
- `T{NN}:{line}` — Transcript line reference (e.g., `T01:328` = Transcript 01, line 328)
- `C{N}` — Hard constraint from CLAUDE.md (e.g., `C1` = append-only store)

#### Stage Assignment

Every element is assigned to an implementation stage:

| Stage | Scope | Dependencies |
|-------|-------|--------------|
| 0 | Harvest/Seed cycle — core store, query, schema, harvest, seed, guidance, dynamic CLAUDE.md | None |
| 1 | Budget-aware output + guidance injection | Stage 0 |
| 2 | Branching + deliberation | Stage 1 |
| 3 | Multi-agent coordination — CRDT merge, sync barriers, signal system | Stage 2 |
| 4 | Advanced intelligence — significance, spectral authority, learned guidance, TUI | Stage 3 |

### §0.3 Namespace Index

| § | Namespace | SEED.md §§ | ADRS.md Categories | Wave | Est. Elements |
|---|-----------|------------|---------------------|------|---------------|
| 1 | STORE | §4, §11 | FD-001–012, AS-001–010, SR-001–011 | 1 | ~15 INV, ~16 ADR, ~5 NEG |
| 1b | LAYOUT | §4, §11 | SR-006, SR-007, SR-014, FD-007 | 1 | 11 INV, 7 ADR, 5 NEG |
| 2 | SCHEMA | §4 | SR-008–009, FD-005, FD-008 | 1 | ~8 INV, ~4 ADR, ~3 NEG |
| 3 | QUERY | §4 | FD-003, SQ-001–010, PO-013 | 1 | ~12 INV, ~8 ADR, ~4 NEG |
| 4 | RESOLUTION | §4 | FD-005, CR-001–007 | 1 | ~8 INV, ~5 ADR, ~3 NEG |
| 5 | HARVEST | §5 | LM-005–006, LM-012–013 | 2 | ~9 INV, ~4 ADR, ~3 NEG |
| 6 | SEED | §5, §8 | IB-010, PO-014, GU-004 | 2 | ~6 INV, ~3 ADR, ~2 NEG |
| 7 | MERGE | §6 | AS-001, PD-004, PO-006 | 2 | ~10 INV, ~5 ADR, ~3 NEG |
| 8 | SYNC | §6 | PO-010, SQ-001, SQ-004 | 2 | ~5 INV, ~3 ADR, ~2 NEG |
| 9 | SIGNAL | §6 | PO-004–005, PO-008 | 3 | ~6 INV, ~3 ADR, ~2 NEG |
| 10 | BILATERAL | §3, §6 | SQ-006, CO-004 | 3 | ~5 INV, ~3 ADR, ~2 NEG |
| 11 | DELIBERATION | §6 | CR-004–005, CR-007, PO-007 | 3 | ~6 INV, ~4 ADR, ~2 NEG |
| 12 | GUIDANCE | §7, §8 | GU-001–008 | 3 | ~8 INV, ~5 ADR, ~3 NEG |
| 13 | BUDGET | §8 | IB-004–007 | 3 | ~6 INV, ~4 ADR, ~2 NEG |
| 14 | INTERFACE | §8 | IB-001–003, IB-008–012 | 3 | ~8 INV, ~5 ADR, ~3 NEG |
| 15 | — | — | — | 4 | Uncertainty Register |
| 16 | — | — | — | 4 | Verification Plan |
| 17 | — | — | — | 4 | Cross-Reference Index |

### §0.4 Hard Constraints (Non-Negotiable)

These constraints from CLAUDE.md are axiomatic. Every element in this specification must be
consistent with all seven. Violation of any constraint is a defect regardless of other merits.

| ID | Constraint | Source |
|----|-----------|--------|
| C1 | **Append-only store.** The datom store never deletes or mutates. Retractions are new datoms with `op=retract`. | SEED §4 Axiom 2, FD-001 |
| C2 | **Identity by content.** A datom is `[e, a, v, tx, op]`. Same fact = same datom. | SEED §4 Axiom 1, FD-007 |
| C3 | **Schema-as-data.** Schema is defined as datoms, not separate DDL. Schema evolution is a transaction. | SEED §4, FD-008 |
| C4 | **CRDT merge by set union.** Merging two stores = mathematical set union of datom sets. | SEED §4 Axiom 2, AS-001 |
| C5 | **Traceability.** Every artifact traces to spec; every spec element traces to SEED.md goals. | SEED §3 |
| C6 | **Falsifiability.** Every invariant has an explicit violation condition. | SEED §3 |
| C7 | **Self-bootstrap.** DDIS specifies itself. Spec elements are the first data the system manages. | SEED §10, FD-006 |

#### Constraint-Axiom Mapping

| Constraint | SEED.md Axiom | SEED.md Section |
|-----------|--------------|----------------|
| C1 (Append-only) | Axiom 2 (Store) | §4 |
| C2 (Identity by content) | Axiom 1 (Identity) | §4 |
| C3 (Schema-as-data) | — (design decision) | §4 |
| C4 (CRDT merge) | Axiom 2 (Store set-union) | §4 |
| C5 (Traceability) | — (methodology) | §3, §8 |
| C6 (Falsifiability) | — (methodology) | §3 |
| C7 (Self-bootstrap) | — (design commitment) | §1 |

---

### §0.5 Foundational ADRs

These ADRs capture foundational decisions about the project itself — its relationship to
predecessor systems, its methodology, its formalism, and its core design philosophy. They
belong in the preamble because they are prerequisites to every namespace specification.

### ADR-FOUNDATION-001: Braid Replaces Go CLI

**Traces to**: SEED §9, ADRS LM-001
**Stage**: 0

#### Problem
What is the relationship between Braid and the existing Go CLI (~62,500 LOC)?
Should Braid extend, migrate, or replace it?

#### Options
A) **Extend the Go CLI** — Add new capabilities to the existing codebase. Preserves
   investment in existing code, but inherits architectural decisions (30 SQLite tables,
   mutable storage, event-sourcing pipeline) that conflict with the datom store model.
B) **Migrate the Go CLI incrementally** — Replace modules one by one. Allows gradual
   transition but creates a long-lived hybrid state where two architectures coexist,
   with impedance mismatch at every boundary.
C) **Replace with a new implementation** — Build Braid from the specification, using the
   Go CLI as reference material only. Clean algebraic foundation, but discards existing
   tested code.

#### Decision
**Option C.** Braid is a new implementation built from the specification. The Go CLI
is reference material — consulted for design insights and to validate that DDIS concepts
work in practice — but not extended, patched, or migrated.

#### Formal Justification
The Go CLI's foundational architecture (mutable SQLite storage, sequential event streams,
file-scoped operations) is structurally incompatible with C1 (append-only), C2 (content-
addressed identity), and C4 (CRDT merge by set union). These are not surface differences
fixable by refactoring — they are algebraic incompatibilities. Extending or migrating would
require maintaining two incompatible invariant sets simultaneously, violating the coherence
the system is designed to enforce.

#### Consequences
- The Go CLI's 97 invariants and 74 ADRs inform but do not bind Braid's specification
- No code from `../ddis-cli/` is ported line-by-line; design patterns may be adopted after
  verification against the Braid specification
- GAP_ANALYSIS.md documents the relationship between Go CLI modules and Braid namespaces
- The Go CLI remains operational for existing workflows during Braid development

#### Falsification
This decision is wrong if: the Go CLI's architecture can be shown to satisfy C1–C4 and C7
without structural changes, making replacement unnecessary.

---

### ADR-FOUNDATION-002: Manual Harvest/Seed Before Tooling

**Traces to**: SEED §10, ADRS LM-002
**Stage**: 0

#### Problem
The harvest/seed lifecycle is the core mechanism for durable knowledge across conversation
boundaries. But the tooling that automates it (the datom store, the harvest pipeline, the
seed assembler) does not yet exist. Should we wait for the tooling or practice the
methodology manually?

#### Options
A) **Wait for tooling** — Build the datom store first, then start using harvest/seed.
   Clean automation from the start, but no knowledge durability during the critical
   specification and early implementation phases.
B) **Manual discipline first** — Practice harvest/seed by hand (HARVEST.md entries at
   session boundaries) before the tooling exists. Validates the methodology through use,
   generates the first dataset, but requires disciplined manual effort.
C) **Lightweight automation** — Build a minimal script-based harvest/seed before the
   full datom store. Intermediate automation, but creates throwaway tooling that may
   diverge from the eventual specification.

#### Decision
**Option B.** Methodology precedes tooling. The manual harvest/seed discipline
(HARVEST.md entries at session start and end) validates the methodology through direct
practice before any code is written. The manual entries become the first dataset when the
datom store is built — they are migrated into datoms as the system's first act.

#### Formal Justification
If the methodology requires tooling to function, it is not a methodology — it is a tool
dependency. By practicing harvest/seed manually, we validate that the core abstractions
(session boundary, knowledge extraction, context assembly) are coherent independent of
implementation. This also satisfies C7 (self-bootstrap): the specification process itself
generates the first test data for the harvest/seed pipeline.

#### Consequences
- Every session ends with a harvest entry in HARVEST.md (NEG-006 in CLAUDE.md)
- Every session starts by reading the latest HARVEST.md entry (the manual seed)
- The manual entries are structured for eventual migration into the datom store
- The discipline is non-negotiable even when it feels redundant — it IS the methodology

#### Falsification
This decision is wrong if: manual harvest/seed entries provide no measurable benefit to
cross-session knowledge continuity compared to sessions without them.

---

### ADR-FOUNDATION-003: D-Centric Agent System Formalism

**Traces to**: SEED §4, ADRS AA-002
**Stage**: 0

#### Problem
What is the formal model for an agent operating within DDIS? How do agents, operations,
observations, and the datom store compose into a coherent system formalism?

#### Options
A) **POSIX-centric formalism** — Model the agent as a process with filesystem state.
   Operations are system calls. The datom store is an application-level abstraction over
   the filesystem. Familiar but couples the protocol to an operating system model.
B) **D-centric formalism** — `(D, Op_D, Obs_D, A, pi, Sigma, Gamma)` where D (the datom
   store) is the central object. All operations reference D. The POSIX runtime R is below
   the protocol boundary — opaque and not modeled.
C) **Actor-model formalism** — Each agent is an actor with a mailbox. The datom store is
   a shared actor. Message-passing semantics govern coordination. Well-studied but
   introduces coordination channels outside the store.

#### Decision
**Option B.** The D-centric formalism places the datom store at the center of the universe.
Agents (A) are modeled as functions from store state to operations. Operations (Op_D) are
transformations on D. Observations (Obs_D) are functions from external state into datoms.
The policy function (pi), signal system (Sigma), and guidance function (Gamma) all operate
on and through D. The POSIX runtime R is below the protocol boundary — the system neither
models nor depends on specific OS abstractions.

#### Formal Justification
```
System S = (D, Op_D, Obs_D, A, pi, Sigma, Gamma)
  D     : Store                              — the datom set (G-Set CvRDT)
  Op_D  : D -> D                             — store operations (transact, merge)
  Obs_D : R-State -> [Datom]                 — observation functor (R -> D)
  A     : Set<Agent>                         — registered agents
  pi    : (Agent, D) -> Op_D                 — policy: agent + state -> action
  Sigma : D -> Set<Signal>                   — signal detection (divergence -> signal)
  Gamma : (Agent, D, Signal) -> Guidance     — guidance generation
```

All protocol-level state is in D. Anything not in D is either below the protocol boundary
(R-state) or a transient computation. This ensures C4 (CRDT merge of D merges everything
protocol-relevant) and C5 (traceability through D).

#### Consequences
- The observation functor `Obs_D : R-State -> [Datom]` is the only protocol-sanctioned
  interface between the external world and the datom store
- No protocol operation references files, processes, or OS state directly
- The formalism is independent of deployment model (embedded, client-server, distributed)
- Agent behavior is fully characterized by its policy function pi

#### Falsification
This decision is wrong if: a protocol-level operation is identified that cannot be expressed
as a function of D, requiring direct reference to R-state at the protocol layer.

> **Note**: The three properties of the observation functor Obs_D (idempotent, monotonic, lossy) are structural consequences of the append-only store (C1) and content addressing (C2). They are verified transitively through INV-STORE-001 and INV-STORE-003. Separate INVs would duplicate existing coverage.

---

### ADR-FOUNDATION-004: Specification Uses DDIS Formalism

**Traces to**: SEED §3, ADRS LM-009
**Stage**: 0

#### Problem
What structure should the specification documents follow? Should they use traditional
requirements document format, or the DDIS formalism itself?

#### Options
A) **Traditional requirements** — numbered requirements (REQ-001, REQ-002) with shall-
   statements. Familiar, tooling-supported, but lacks falsifiability, traceability, and
   the structural properties DDIS is designed to enforce.
B) **DDIS formalism** — Invariants (with falsification conditions), ADRs (with alternatives
   and rationale), negative cases (safety properties), uncertainty markers (with confidence
   levels). Every element individually addressable, typed, and traceable.
C) **Formal methods notation only** — TLA+, Alloy, or Z specifications. Maximum rigor but
   inaccessible to most contributors and difficult to maintain.

#### Decision
**Option B.** The specification uses the DDIS formalism: invariants (INV), ADRs, negative
cases (NEG), and uncertainty markers. Every element has an ID, type, traceability to
SEED.md, and (for invariants) an explicit falsification condition. This structure enables
mechanical migration into the datom store at Stage 0.

#### Formal Justification
Using the DDIS formalism for the specification satisfies C7 (self-bootstrap) directly:
the specification elements are structured identically to the data the system will manage.
Migration from document to datom store is a parsing operation, not a semantic
transformation. Traditional requirements (Option A) would require a lossy transformation
to fit the datom schema. Pure formal methods (Option C) would limit contributors and
create a separate truth source that must be kept in sync with implementation artifacts.

#### Consequences
- Every specification element has an ID following the pattern INV-{NS}-{NNN}, ADR-{NS}-{NNN},
  NEG-{NS}-{NNN}
- Every invariant has an explicit falsification condition
- Every ADR records alternatives considered and rationale for selection
- Uncertainty is marked explicitly with confidence levels, not hidden in ambiguous prose
- The preamble (this document) defines the conventions; all namespace files follow them

#### Falsification
This decision is wrong if: the DDIS formalism imposes so much overhead that specification
velocity drops below the rate needed to keep pace with implementation, making the formalism
a bottleneck rather than an accelerant.

---

### ADR-FOUNDATION-005: Structural Over Procedural Coherence

**Traces to**: SEED §6, ADRS CO-006
**Stage**: 0

#### Problem
How should the system maintain coherence between intent, specification, implementation,
and observed behavior? Through process obligations (code reviews, checklists, audits)
or through structural enforcement (type systems, invariants, algebraic properties)?

#### Options
A) **Procedural coherence** — Checklists, reviews, approval gates, periodic audits.
   Coherence depends on human/agent diligence. Familiar, widely practiced, but decays
   under time pressure, cognitive load, and personnel changes.
B) **Structural coherence** — Coherence is enforced by the architecture itself: type
   systems that prevent invalid states, algebraic properties that guarantee merge
   correctness, invariants with machine-verifiable falsification conditions. Coherence
   persists because it is a property of the structure, not of the process.
C) **Hybrid** — Structural enforcement for critical properties, procedural checks for
   everything else. Pragmatic but creates two tiers of reliability, with the procedural
   tier degrading over time.

#### Decision
**Option B.** Coherence is a structural property, not a procedural obligation. Process
obligations decay under pressure. Structural properties persist because they are enforced
by architecture.

#### Formal Justification
Every hard constraint (C1–C7) is stated as a structural property:
- C1 (append-only) is enforced by the Store API exposing no mutation operations
- C2 (content-addressed identity) is enforced by EntityId having a single constructor
- C3 (schema-as-data) is enforced by Schema having no external constructor
- C4 (CRDT merge) is enforced by set-union algebra (L1–L4)
- C5 (traceability) is enforced by the element ID and traces-to format
- C6 (falsifiability) is enforced by requiring falsification conditions in spec elements
- C7 (self-bootstrap) is enforced by the spec-element schema (Layer 2 attributes)

None of these depend on someone remembering to check. They are properties of the
artifacts themselves. This is the fundamental design philosophy of Braid.

#### Consequences
- Type-level enforcement is preferred over runtime checks where possible (V:TYPE)
- Runtime invariants use property-based testing (V:PROP) and bounded model checking (V:KANI)
  rather than manual test case enumeration
- Negative cases are stated as temporal safety properties, not process rules
- The system's own coherence verification (bilateral loop) is structural, not audit-driven

#### Falsification
This decision is wrong if: a critical coherence property is identified that cannot be
expressed structurally and requires procedural enforcement as the only viable mechanism.

---

### ADR-FOUNDATION-006: Self-Bootstrap Fixed-Point Property

**Traces to**: SEED §10, C7, ADRS LM-008
**Stage**: 0

#### Problem
DDIS specifies itself (C7). What does self-bootstrap convergence look like, and how do
we know when it has been achieved?

#### Options
A) **Spec as external document only** — The specification is a document that describes
   the system. The system manages other data, not its own spec. No self-reference.
B) **Spec as data, convergence by fiat** — The specification is loaded into the datom
   store, but convergence is declared rather than verified. The spec-as-document and
   spec-as-data may silently diverge.
C) **Spec as data with fixed-point convergence** — The specification is both a document
   and data in the store. Convergence is achieved when the specification-as-data and the
   specification-as-document agree. The specification process itself generates the first
   test data: invariants about the store become the store's test cases; contradictions
   caught during specification become contradiction-detection test cases.

#### Decision
**Option C.** When the system manages its own specification, the specification IS data.
The self-bootstrap fixed-point is reached when:
1. Every specification element exists as a datom in the store
2. The datom representation is faithful (no lossy transformation)
3. The system can verify its own specification for internal contradictions
4. The specification process generates test cases for the system's verification machinery

#### Formal Justification
Let `spec_doc` be the specification as a document and `spec_data` be the specification as
datoms in the store. The fixed-point condition is:
```
parse(spec_doc) = spec_data   AND   render(spec_data) = spec_doc
```
This is a round-trip property: parse and render are inverses. At the fixed point, there is
one specification with two representations (document and datoms) that agree exactly.

The specification process is self-validating: writing INV-STORE-001 (append-only) as a
datom tests whether the store can represent its own invariants. If the store cannot
represent INV-STORE-001, the invariant and the store are inconsistent — which is exactly
the kind of divergence DDIS is designed to detect.

#### Consequences
- The first act of Stage 0 is migrating specification elements into the datom store
- The migration is a test: if it fails, the spec-element schema is incomplete
- Round-trip fidelity (parse -> datom -> render -> document) is a verification target
- The contradiction detection engine's first test case is the specification itself
- Self-bootstrap is not a one-time event but an ongoing fixed-point maintenance

#### Falsification
This decision is wrong if: the specification cannot be faithfully represented as datoms
(some element type or relationship has no datom encoding), making the round-trip property
impossible to achieve.

---

