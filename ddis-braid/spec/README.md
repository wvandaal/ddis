# Braid Specification — Master Index

> Cleanroom-grade specification for Braid, the Rust implementation of DDIS.
> Every invariant is formally falsifiable, every ADR grounded in algebraic properties,
> every negative case stated as a safety property.
>
> **Self-bootstrap**: This specification is the first dataset the system will manage (C7, FD-006).

---

## Shared Definitions

**[00-preamble.md](00-preamble.md)** — Conventions, element ID format, three-level refinement,
verification tags, traceability notation, stage assignments, hard constraints C1–C7.

Read the preamble first. All namespace files reference it for shared definitions.

---

## Namespace Index

### Wave 1: Foundation (algebraic core)

| File | Namespace | Lines | Stage | SEED.md §§ |
|------|-----------|-------|-------|------------|
| [01-store.md](01-store.md) | STORE | 1,175 | 0 | §4, §11 |
| [02-schema.md](02-schema.md) | SCHEMA | 561 | 0 | §4 |
| [03-query.md](03-query.md) | QUERY | 720 | 0 | §4 |
| [04-resolution.md](04-resolution.md) | RESOLUTION | 547 | 0 | §4 |

### Wave 2: Lifecycle (session and coordination mechanics)

| File | Namespace | Lines | Stage | SEED.md §§ |
|------|-----------|-------|-------|------------|
| [05-harvest.md](05-harvest.md) | HARVEST | 582 | 0 | §5 |
| [06-seed.md](06-seed.md) | SEED | 438 | 0 | §5, §8 |
| [07-merge.md](07-merge.md) | MERGE | 521 | 3 | §6 |
| [08-sync.md](08-sync.md) | SYNC | 369 | 3 | §6 |

### Wave 3: Intelligence (steering and adaptation)

| File | Namespace | Lines | Stage | SEED.md §§ |
|------|-----------|-------|-------|------------|
| [09-signal.md](09-signal.md) | SIGNAL | 416 | 3 | §6 |
| [10-bilateral.md](10-bilateral.md) | BILATERAL | 362 | 2 | §3, §6 |
| [11-deliberation.md](11-deliberation.md) | DELIBERATION | 403 | 2 | §6 |
| [12-guidance.md](12-guidance.md) | GUIDANCE | 425 | 0 | §7, §8 |
| [13-budget.md](13-budget.md) | BUDGET | 352 | 1 | §8 |
| [14-interface.md](14-interface.md) | INTERFACE | 410 | 0 | §8 |

### Wave 4: Integration (cross-cutting references)

| File | Section | Lines |
|------|---------|-------|
| [15-uncertainty.md](15-uncertainty.md) | Uncertainty Register | 290 |
| [16-verification.md](16-verification.md) | Verification Plan | 271 |
| [17-crossref.md](17-crossref.md) | Cross-Reference Index + Appendices A–C | 229 |

---

## Reading Order

**For implementation (Stage 0):** preamble → STORE → SCHEMA → QUERY → HARVEST → SEED → GUIDANCE → INTERFACE

**For full understanding:** preamble → Wave 1 → Wave 2 → Wave 3 → Wave 4

**For a specific namespace:** preamble → that namespace file (each is self-contained with the preamble)

---

## Element Counts

| Type | Count | Namespaces |
|------|-------|------------|
| INV (Invariants) | 14 namespaces | All with falsification conditions and verification tags |
| ADR (Decisions) | 14 namespaces | All with alternatives and rationale |
| NEG (Negative Cases) | 14 namespaces | All with violation conditions |

See [17-crossref.md](17-crossref.md) for the complete element count summary and cross-reference tables.

---

*Modularized from the monolithic SPEC.md to enable per-namespace loading during implementation.
Content is byte-for-byte identical to the original sections.*
