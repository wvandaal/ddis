# DEFECT_SPEC.md — Session 008 Cleanroom Audit Findings

> **Purpose**: DDIS-structured specification of defects discovered during a systematic
> "fresh eyes" audit of the braid-kernel and braid binary crates. Each defect is formalized
> as either an invariant violation (with falsification condition), an ADR (with rationale),
> or a negative case — per DDIS methodology.
>
> **Audit scope**: All `.rs` files in `braid-kernel/src/`, `braid/src/`, and
> `braid-kernel/tests/`. 288 tests passing at audit start. Cargo clippy clean.
>
> **Methodology**: Four parallel exploration agents traced execution flows through every
> module. Each finding was independently verified against source. False positives rejected.

---

## Confirmed Defects

### DEF-001: Transaction Rationale Not Stored as Datom

**Type**: Bug — Feature broken
**Severity**: HIGH
**Files**: `braid-kernel/src/store.rs:489-527`, `braid/src/commands/log.rs:45-49`

**Description**: `Store::make_tx_metadata()` creates datoms for `:tx/time`, `:tx/agent`,
and `:tx/provenance`, but does NOT create a datom for `:tx/rationale`. The `braid log`
command searches for `:tx/rationale` datoms (log.rs:45-49) and always finds nothing,
displaying `"-"` for every transaction's rationale.

The rationale IS stored in the on-disk `TxFile` format (layout.rs:186, 201), but is lost
during the store reconstruction via `load_store()` → `collect_datoms()`, because `TxFile`
metadata fields are separate from the datom list.

**Traces to**: INV-STORE-014 (transaction provenance tracking). The invariant requires
"every transaction carries provenance metadata as datoms" — rationale is part of provenance.

**Falsification condition**: `braid log --datoms` on any store shows `rationale: -` for
transactions that were created with non-empty `--rationale` arguments.

**Fix**: Add `:tx/rationale` datom creation to `make_tx_metadata()` in store.rs, after the
`:tx/provenance` block (line 524). Use `Value::String(tx_data.rationale.clone())`.

**Constraint check**: Fix only ADDS datoms (C1 ✓). Uses content-addressed entity (C2 ✓).
Rationale attribute is already in Layer 1 schema (C3 ✓). Set union merge unaffected (C4 ✓).

---

### DEF-002: read_tx Panics on Short Hash Input

**Type**: Bug — Panic on invalid input
**Severity**: MEDIUM
**Files**: `braid/src/layout.rs:133`

**Description**: `DiskLayout::read_tx(hash_hex)` does `&hash_hex[..2]` without validating
that `hash_hex.len() >= 2`. If called with an empty or single-character string, this
panics with a slice bounds error.

Currently, all callers pass hashes from `list_tx_hashes()` (which reads filenames from disk),
so the panic is not triggered in normal operation. However, the API is public and the
contract is not enforced.

**Traces to**: NEG-001 (no aspirational stubs — the function signature promises to handle
any `&str` but panics on some inputs).

**Falsification condition**: `DiskLayout::read_tx("")` panics instead of returning `Err`.

**Fix**: Add validation at the top of `read_tx()`:
```rust
if hash_hex.len() < 2 {
    return Err(BraidError::Store(format!("invalid tx hash: too short ({})", hash_hex)));
}
```

---

### DEF-003: O(n²) Entity Deduplication in live_projections

**Type**: Performance defect
**Severity**: MEDIUM
**Files**: `braid-kernel/src/trilateral.rs:103-151`

**Description**: `live_projections()` uses `Vec::contains()` to deduplicate entities as
they are classified into intent/spec/impl projections. For each datom, `contains()` scans
the entire vector — O(n) per datom, O(n²) total. With a store of 100K datoms and 10K
unique entities, this performs ~50M comparisons.

The same pattern appears in `compute_phi_default()` (lines 178-189) which uses
`filter(|e| !live_s.entities.contains(e))` — another O(n*m) scan.

**Traces to**: INV-TRILATERAL-001 (LIVE projections must be monotone and efficiently
computable). The monotonicity is satisfied, but "efficiently" is not.

**Falsification condition**: `live_projections()` on a store with 100K datoms takes
>100ms (should be <10ms).

**Fix**: Replace `Vec::contains()` dedup with `BTreeSet` (matching the store's existing
type convention):
```rust
let mut intent_set = BTreeSet::new();
// ...
if intent_set.insert(datom.entity) {
    intent_entities.push(datom.entity);
}
```
Then convert `LiveView::entities` field to use `Vec` built from the set at the end (preserving insertion order for determinism, which BTreeSet provides via sorted order).

Similarly, convert `compute_phi_default()` set difference to use `BTreeSet::contains()`.

---

### DEF-004: serde_json Unwrap Inconsistency

**Type**: Code quality / Panic safety
**Severity**: LOW
**Files**: `braid-kernel/src/store.rs:356`, `braid-kernel/src/resolution.rs:153-154`

**Description**: Two locations use `.unwrap()` on `serde_json::to_vec()` without an
explanatory message, while the canonical pattern in `datom.rs:404` uses
`.expect("datom serialization cannot fail")`.

- `store.rs:356`: `serde_json::to_vec(&tx_id).unwrap()`
- `resolution.rs:153`: `serde_json::to_vec(v1).unwrap()`
- `resolution.rs:154`: `serde_json::to_vec(v2).unwrap()`

**Traces to**: Coding discipline — all panicking paths must have explanatory messages.

**Falsification condition**: Any `.unwrap()` on serialization in non-test code without
an `expect()` message.

**Fix**: Replace each `.unwrap()` with `.expect("serialization cannot fail")`.

---

## Confirmed Design Issues (Not Bugs — Document Only)

### DES-001: β₁ Proxy Always Returns Zero

**Type**: Known limitation
**Severity**: HIGH (architectural)
**Files**: `braid-kernel/src/trilateral.rs:349-355`

**Description**: `compute_beta_1_proxy()` returns `0` unconditionally. This means
INV-TRILATERAL-009 (Φ,β₁ duality) is unfalsifiable at Stage 0 — the coherence quadrants
`CyclesOnly` and `GapsAndCycles` are unreachable.

This is **documented** in the code and is a **Stage 0 design choice** — full eigendecomposition
is deferred to Stage 1. The proxy is conservative (no false positives).

**Action**: No code change. This is tracked as a Stage 1 deliverable. The comment at
line 348 is accurate. A bead should be created if one doesn't exist.

### DES-002: Agent Filtering Uses Debug Format

**Type**: UX limitation
**Severity**: MEDIUM
**Files**: `braid/src/commands/log.rs:32`

**Description**: `format!("{:?}", tx.agent())` produces the Debug representation of
`AgentId` (a 16-byte array), which is unreadable. Agent names are hashed during
construction and cannot be recovered.

**Action**: Stage 1 improvement — store original agent name as a `:agent/name` datom
alongside the hashed ID. No code change now.

---

## Rejected False Positives

| Claim | Verdict | Reason |
|---|---|---|
| store.rs new_entities detection logic error | **Correct code** | Post-insert scan with `d.tx != tx_id` correctly identifies pre-existing entities |
| `:tx/provenance` datom lookup broken | **Works** | `make_tx_metadata()` creates `:tx/provenance` datoms at line 518-523 |
| harvest.rs division by zero | **Guarded** | `is_empty()` check at line 139 prevents it |
| seed.rs token estimation violates budget | **By design** | Documented as heuristic; budget is soft limit |
| guidance.rs external dependencies | **Correct** | Missing deps correctly treated as "not ready" |

---

## Implementation Order

All four fixes (DEF-001 through DEF-004) are independent. Recommended order:

1. **DEF-001** (rationale datom) — most impactful, restores broken feature
2. **DEF-003** (O(n²) dedup) — correctness-preserving performance fix
3. **DEF-002** (read_tx bounds) — defensive hardening
4. **DEF-004** (unwrap→expect) — code quality cleanup

Single atomic commit per defect, or one combined commit since all are small and independent.

---

*This document follows DDIS methodology: each defect has an ID, type, traceability to
spec elements, falsification condition, and constraint verification. It is itself a
specification that the fixes must satisfy.*
