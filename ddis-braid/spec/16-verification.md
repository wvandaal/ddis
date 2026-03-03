> **Section**: Verification Plan | **Wave**: 4 (Integration)
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §16. Verification Plan

> **Purpose**: Maps every invariant to its verification method(s), tool, implementation
> stage, and CI gate. This section is the implementor's guide to "how do I prove this
> invariant holds?"

### §16.1 Per-Invariant Verification Matrix

#### STORE (14 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-STORE-001 | V:TYPE | V:KANI | rustc + kani | compile + kani | 0 |
| INV-STORE-002 | V:TYPE | — | rustc | compile | 0 |
| INV-STORE-003 | V:TYPE | V:PROP | rustc + proptest | compile + test | 0 |
| INV-STORE-004 | V:PROP | V:KANI, V:MODEL | proptest + kani + stateright | test + kani + model | 0 |
| INV-STORE-005 | V:PROP | V:KANI, V:MODEL | proptest + kani + stateright | test + kani + model | 0 |
| INV-STORE-006 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-STORE-007 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-STORE-008 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-STORE-009 | V:PROP | — | proptest | test | 0 |
| INV-STORE-010 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-STORE-011 | V:PROP | — | proptest | test | 0 |
| INV-STORE-012 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-STORE-013 | V:PROP | V:MODEL | proptest + stateright | test + model | 2 |
| INV-STORE-014 | V:PROP | — | proptest | test | 0 |

#### SCHEMA (8 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-SCHEMA-001 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-SCHEMA-002 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-SCHEMA-003 | V:TYPE | — | rustc | compile | 0 |
| INV-SCHEMA-004 | V:TYPE | V:KANI, V:PROP | rustc + kani + proptest | compile + kani + test | 0 |
| INV-SCHEMA-005 | V:PROP | — | proptest | test | 0 |
| INV-SCHEMA-006 | V:PROP | — | proptest | test | 0 |
| INV-SCHEMA-007 | V:PROP | — | proptest | test | 0 |
| INV-SCHEMA-008 | V:PROP | — | proptest | test | 0 |

#### QUERY (11 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-QUERY-001 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-QUERY-002 | V:PROP | — | proptest | test | 0 |
| INV-QUERY-003 | V:PROP | — | proptest | test | 1 |
| INV-QUERY-004 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-QUERY-005 | V:TYPE | V:PROP | rustc + proptest | compile + test | 0 |
| INV-QUERY-006 | V:TYPE | — | rustc | compile | 0 |
| INV-QUERY-007 | V:TYPE | V:PROP | rustc + proptest | compile + test | 0 |
| INV-QUERY-008 | V:PROP | — | proptest | test | 1 |
| INV-QUERY-009 | V:PROP | — | proptest | test | 1 |
| INV-QUERY-010 | V:PROP | V:MODEL | proptest + stateright | test + model | 2 |
| INV-QUERY-011 | V:PROP | — | proptest | test | 2 |

#### RESOLUTION (8 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-RESOLUTION-001 | V:TYPE | V:PROP | rustc + proptest | compile + test | 0 |
| INV-RESOLUTION-002 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-003 | V:PROP | V:MODEL | proptest + stateright | test + model | 3 |
| INV-RESOLUTION-004 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-005 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-006 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-007 | V:PROP | V:MODEL, V:KANI | proptest + stateright + kani | test + model + kani | 2 |
| INV-RESOLUTION-008 | V:PROP | V:MODEL | proptest + stateright | test + model | 0 |

#### HARVEST (8 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-HARVEST-001 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-HARVEST-002 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-003 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-004 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-005 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-006 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-HARVEST-007 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-008 | V:PROP | — | proptest | test | 0 |

#### SEED (6 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-SEED-001 | V:PROP | — | proptest | test | 0 |
| INV-SEED-002 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-SEED-003 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-SEED-004 | V:PROP | — | proptest | test | 0 |
| INV-SEED-005 | V:PROP | — | proptest | test | 1 |
| INV-SEED-006 | V:PROP | — | proptest | test | 2 |

#### MERGE (8 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-MERGE-001 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-MERGE-002 | V:PROP | V:MODEL | proptest + stateright | test + model | 2 |
| INV-MERGE-003 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-MERGE-004 | V:PROP | V:KANI, V:MODEL | proptest + kani + stateright | test + kani + model | 2 |
| INV-MERGE-005 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-MERGE-006 | V:PROP | — | proptest | test | 2 |
| INV-MERGE-007 | V:PROP | — | proptest | test | 2 |
| INV-MERGE-008 | V:PROP | — | proptest | test | 0 |

#### SYNC (5 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-SYNC-001 | V:PROP | V:MODEL | proptest + stateright | test + model | 3 |
| INV-SYNC-002 | V:PROP | — | proptest | test | 3 |
| INV-SYNC-003 | V:PROP | V:MODEL | proptest + stateright | test + model | 3 |
| INV-SYNC-004 | V:PROP | V:MODEL | proptest + stateright | test + model | 3 |
| INV-SYNC-005 | V:PROP | — | proptest | test | 3 |

#### SIGNAL (6 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-SIGNAL-001 | V:PROP | V:KANI | proptest + kani | test + kani | 3 |
| INV-SIGNAL-002 | V:PROP | — | proptest | test | 1 |
| INV-SIGNAL-003 | V:PROP | V:KANI | proptest + kani | test + kani | 3 |
| INV-SIGNAL-004 | V:PROP | — | proptest | test | 3 |
| INV-SIGNAL-005 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-SIGNAL-006 | V:PROP | — | proptest | test | 3 |

#### BILATERAL (5 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-BILATERAL-001 | V:PROP | V:MODEL | proptest + stateright | test + model | 1 |
| INV-BILATERAL-002 | V:PROP | — | proptest | test | 1 |
| INV-BILATERAL-003 | V:PROP | — | proptest | test | 2 |
| INV-BILATERAL-004 | V:PROP | — | proptest | test | 1 |
| INV-BILATERAL-005 | V:PROP | — | proptest | test | 1 |

#### DELIBERATION (6 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-DELIBERATION-001 | V:PROP | V:MODEL | proptest + stateright | test + model | 2 |
| INV-DELIBERATION-002 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-DELIBERATION-003 | V:PROP | — | proptest | test | 2 |
| INV-DELIBERATION-004 | V:PROP | — | proptest | test | 2 |
| INV-DELIBERATION-005 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-DELIBERATION-006 | V:PROP | V:MODEL | proptest + stateright | test + model | 2 |

#### GUIDANCE (7 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-GUIDANCE-001 | V:PROP | — | proptest | test | 0 |
| INV-GUIDANCE-002 | V:PROP | — | proptest | test | 0 |
| INV-GUIDANCE-003 | V:PROP | — | proptest | test | 1 |
| INV-GUIDANCE-004 | V:PROP | — | proptest | test | 1 |
| INV-GUIDANCE-005 | V:PROP | — | proptest | test | 4 |
| INV-GUIDANCE-006 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-GUIDANCE-007 | V:PROP | — | proptest | test | 0 |

#### BUDGET (6 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-BUDGET-001 | V:PROP | V:KANI | proptest + kani | test + kani | 1 |
| INV-BUDGET-002 | V:PROP | — | proptest | test | 1 |
| INV-BUDGET-003 | V:PROP | V:KANI | proptest + kani | test + kani | 1 |
| INV-BUDGET-004 | V:PROP | — | proptest | test | 1 |
| INV-BUDGET-005 | V:PROP | — | proptest | test | 1 |
| INV-BUDGET-006 | V:PROP | V:KANI | proptest + kani | test + kani | 1 |

#### INTERFACE (9 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-INTERFACE-001 | V:PROP | — | proptest | test | 0 |
| INV-INTERFACE-002 | V:PROP | — | proptest | test | 0 |
| INV-INTERFACE-003 | V:PROP | V:TYPE | proptest + rustc | test + compile | 0 |
| INV-INTERFACE-004 | V:PROP | — | proptest | test | 1 |
| INV-INTERFACE-005 | V:PROP | — | proptest | test | 4 |
| INV-INTERFACE-006 | V:PROP | — | proptest | test | 3 |
| INV-INTERFACE-007 | V:PROP | — | proptest | test | 1 |
| INV-INTERFACE-008 | V:PROP | — | proptest | test | 0 |
| INV-INTERFACE-009 | V:PROP | V:TYPE | proptest + rustc | test + compile | 0 |

### §16.2 CI Pipeline Gates

Every commit runs through a staged verification pipeline:

```
Gate 1: compile           — cargo check --all-targets
                            Checks: V:TYPE (all typestate patterns compile)
                            Time: <30s

Gate 2: test              — cargo test
                            Checks: V:PROP (all proptest properties hold)
                            Coverage: 107/107 INVs have proptest strategies
                            Time: <5m (proptest default: 256 cases per property)

Gate 3: kani              — cargo kani
                            Checks: V:KANI (bounded model checking)
                            Coverage: 44 INVs with critical-path verification
                            Time: <15m (bounded; unwind limit configurable)

Gate 4: model             — cargo test --features stateright
                            Checks: V:MODEL (protocol model checking)
                            Coverage: 15 INVs with protocol safety/liveness
                            Time: <30m (state space exploration)

Gate 5: miri (optional)   — cargo +nightly miri test
                            Checks: V:MIRI (undefined behavior detection)
                            Coverage: all unsafe code paths
                            Time: <10m
```

**Gate progression**: Gates 1–2 run on every commit. Gate 3 runs on PRs targeting main.
Gate 4 runs nightly or on protocol-affecting changes. Gate 5 runs on any `unsafe` code changes.

**Failure handling**: A gate failure blocks merge. The implementing agent must fix the
failing invariant before proceeding. Gate failures are recorded as datoms (CO-011).

### §16.3 Typestate Encoding Catalog

Protocols enforced at compile time via Rust's type system (zero runtime cost):

| Protocol | Types | Transitions | INV |
|----------|-------|-------------|-----|
| Transaction lifecycle | `Building → Committed → Applied` | `commit()`, `apply()` | INV-STORE-001 |
| EntityId construction | `EntityId(hash)` — no public constructor from arbitrary bytes | content-addressed only | INV-STORE-002 |
| Store immutability | `&Store` for reads, `&mut Store` only via `transact`/`merge` | borrow checker | INV-STORE-005 |
| Schema attribute | `Attribute` newtype — cannot confuse with raw strings | type-safe attribute refs | INV-SCHEMA-003 |
| Schema monotonicity | `SchemaEvolution(datoms)` — no `DROP` or `ALTER DELETE` | append-only by type | INV-SCHEMA-004 |
| Query mode | `QueryMode::Monotonic \| Stratified(Frontier) \| Barriered(BarrierId)` | parse-time enforcement | INV-QUERY-005 |
| FFI boundary | `FfiFunction` trait with `pure` marker — host-language functions can't mutate store | type-level purity | INV-QUERY-006 |
| Resolution mode | `ResolutionMode` enum — exhaustive match required | compile-time completeness | INV-RESOLUTION-001 |
| MCP tool set | `const MCP_TOOLS: [MCPTool; 9]` — fixed-size array | compile-time tool count | INV-INTERFACE-003 |

### §16.4 Deductive Verification Candidates

Invariants where deductive verification (Verus/Creusot) would provide mathematical proof
of correctness, justifying the higher cost:

| INV | Property | Justification |
|-----|----------|---------------|
| INV-STORE-004 | CRDT commutativity: `S₁ ∪ S₂ = S₂ ∪ S₁` | Foundational — all merge correctness depends on this. Proof by construction (set union) but a formal proof would close the loop. |
| INV-STORE-005 | CRDT associativity: `(S₁ ∪ S₂) ∪ S₃ = S₁ ∪ (S₂ ∪ S₃)` | Same justification as commutativity. |
| INV-STORE-006 | CRDT idempotency: `S ∪ S = S` | Completes the CRDT law triad. |
| INV-MERGE-001 | Merge preserves all datoms: `S ⊆ merge(S, S')` | Critical safety — no data loss during merge. |
| INV-RESOLUTION-005 | LWW commutativity | Per-attribute resolution correctness. |

**Recommendation**: Defer deductive verification to post-Stage 2. The cost is high
and the properties are well-served by proptest + Kani during initial implementation.
Pursue deductive proofs when the implementation stabilizes.

### §16.5 Verification Statistics

| Metric | Count | Coverage |
|--------|-------|----------|
| Total invariants | 107 | — |
| With V:PROP (minimum) | 107 | 100% |
| With V:KANI | 44 | 41.1% |
| With V:MODEL | 15 | 14.0% |
| With V:TYPE | 12 | 11.2% |
| Stage 0 invariants | 64 | 59.8% |
| Stage 1 invariants | 18 | 16.8% |
| Stage 2 invariants | 17 | 15.9% |
| Stage 3 invariants | 6 | 5.6% |
| Stage 4 invariants | 2 | 1.9% |

---

