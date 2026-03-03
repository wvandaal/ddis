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

The specification covers 14 namespaces organized into four waves:

- **Foundation** (Wave 1): STORE, SCHEMA, QUERY, RESOLUTION — the algebraic core
- **Lifecycle** (Wave 2): HARVEST, SEED, MERGE, SYNC — session and coordination mechanics
- **Intelligence** (Wave 3): SIGNAL, BILATERAL, DELIBERATION, GUIDANCE, BUDGET, INTERFACE — steering and adaptation
- **Integration** (Wave 4): Uncertainty register, verification plan, cross-reference index

### §0.2 Conventions

#### Element ID Format

```
INV-{NAMESPACE}-{NNN}    Invariant (falsifiable claim with violation condition)
ADR-{NAMESPACE}-{NNN}    Architectural Decision Record (choice with alternatives and rationale)
NEG-{NAMESPACE}-{NNN}    Negative Case (safety property: what must NOT happen)
```

Namespaces: `STORE`, `SCHEMA`, `QUERY`, `RESOLUTION`, `HARVEST`, `SEED`, `MERGE`, `SYNC`,
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
| 1 | STORE | §4, §11 | FD-001–012, AS-001–010, SR-001–011 | 1 | ~15 INV, ~12 ADR, ~5 NEG |
| 2 | SCHEMA | §4 | SR-008–009, FD-005, FD-008 | 1 | ~8 INV, ~4 ADR, ~3 NEG |
| 3 | QUERY | §4 | FD-003, SQ-001–010, PO-013 | 1 | ~12 INV, ~8 ADR, ~4 NEG |
| 4 | RESOLUTION | §4 | FD-005, CR-001–007 | 1 | ~8 INV, ~5 ADR, ~3 NEG |
| 5 | HARVEST | §5 | LM-005–006, LM-012–013 | 2 | ~8 INV, ~4 ADR, ~3 NEG |
| 6 | SEED | §5, §8 | IB-010, PO-014, GU-004 | 2 | ~6 INV, ~3 ADR, ~2 NEG |
| 7 | MERGE | §6 | AS-001, PD-004, PO-006 | 2 | ~9 INV, ~4 ADR, ~3 NEG |
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

---

