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
| INV-STORE-002 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-STORE-003 | V:TYPE | V:PROP, V:KANI | rustc + proptest + kani | compile + test + kani | 0 |
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
| INV-SCHEMA-008 | V:PROP | — | proptest | test | 2 |

#### QUERY (21 INV)

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
| INV-QUERY-010 | V:MODEL | V:PROP | stateright + proptest | model + test | 3 |
| INV-QUERY-011 | V:PROP | — | proptest | test | 2 |
| INV-QUERY-012 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-QUERY-013 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-QUERY-014 | V:PROP | — | proptest | test | 0 |
| INV-QUERY-015 | V:PROP | — | proptest | test | 1 |
| INV-QUERY-016 | V:PROP | — | proptest | test | 1 |
| INV-QUERY-017 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-QUERY-018 | V:PROP | — | proptest | test | 1 |
| INV-QUERY-019 | V:PROP | — | proptest | test | 2 |
| INV-QUERY-020 | V:PROP | — | proptest | test | 2 |
| INV-QUERY-021 | V:PROP | — | proptest | test | 0 |

#### RESOLUTION (8 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-RESOLUTION-001 | V:TYPE | V:PROP | rustc + proptest | compile + test | 0 |
| INV-RESOLUTION-002 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-003 | V:PROP | V:MODEL | proptest + stateright | test + model | 0 |
| INV-RESOLUTION-004 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-005 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-006 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-RESOLUTION-007 | V:PROP | V:MODEL, V:KANI | proptest + stateright + kani | test + model + kani | 0 |
| INV-RESOLUTION-008 | V:PROP | V:MODEL | proptest + stateright | test + model | 0 |

#### HARVEST (8 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-HARVEST-001 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-HARVEST-002 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-003 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-004 | V:PROP | — | proptest | test | 1 |
| INV-HARVEST-005 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-006 | V:PROP | V:KANI | proptest + kani | test + kani | 1 |
| INV-HARVEST-007 | V:PROP | — | proptest | test | 0 |
| INV-HARVEST-008 | V:PROP | — | proptest | test | 2 |

#### SEED (8 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-SEED-001 | V:PROP | — | proptest | test | 0 |
| INV-SEED-002 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-SEED-003 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-SEED-004 | V:PROP | — | proptest | test | 0 |
| INV-SEED-005 | V:PROP | — | proptest | test | 0 |
| INV-SEED-006 | V:PROP | — | proptest | test | 0 |
| INV-SEED-007 | V:PROP | — | proptest | test | 1 |
| INV-SEED-008 | V:PROP | — | proptest | test | 1 |

#### MERGE (9 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-MERGE-001 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-MERGE-002 | V:PROP | V:MODEL | proptest + stateright | test + model | 0 |
| INV-MERGE-003 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-MERGE-004 | V:PROP | V:KANI, V:MODEL | proptest + kani + stateright | test + kani + model | 2 |
| INV-MERGE-005 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-MERGE-006 | V:PROP | — | proptest | test | 2 |
| INV-MERGE-007 | V:PROP | — | proptest | test | 2 |
| INV-MERGE-008 | V:PROP | V:KANI | proptest + kani | test + kani | 0 |
| INV-MERGE-009 | V:PROP | — | proptest | test | 0 |

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

#### GUIDANCE (11 INV)

| ID | Primary V:TAG | Secondary | Tool | CI Gate | Stage |
|----|---------------|-----------|------|---------|-------|
| INV-GUIDANCE-001 | V:PROP | — | proptest | test | 0 |
| INV-GUIDANCE-002 | V:PROP | — | proptest | test | 0 |
| INV-GUIDANCE-003 | V:PROP | — | proptest | test | 1 |
| INV-GUIDANCE-004 | V:PROP | — | proptest | test | 1 |
| INV-GUIDANCE-005 | V:PROP | — | proptest | test | 4 |
| INV-GUIDANCE-006 | V:PROP | V:KANI | proptest + kani | test + kani | 2 |
| INV-GUIDANCE-007 | V:PROP | — | proptest | test | 0 |
| INV-GUIDANCE-008 | V:PROP | — | proptest | test | 0 |
| INV-GUIDANCE-009 | V:PROP | — | proptest | test | 0 |
| INV-GUIDANCE-010 | V:PROP | — | proptest | test | 0 |
| INV-GUIDANCE-011 | V:PROP | — | proptest | test | 2 |

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

Gate 2: fmt               — cargo fmt --check
                            Checks: formatting consistency
                            Time: <5s

Gate 3: clippy            — cargo clippy --all-targets -- -D warnings
                            Checks: linting, common bug patterns
                            Time: <30s

Gate 4: test              — cargo test
                            Checks: V:PROP (all proptest properties hold)
                            Coverage: 121/124 INVs have proptest strategies (3 are V:TYPE-only)
                            Time: <5m (proptest default: 256 cases per property)

Gate 5: kani              — cargo kani
                            Checks: V:KANI (bounded model checking)
                            Coverage: 41 INVs with critical-path verification
                            Time: <15m (bounded; unwind limit configurable)

Gate 6: model             — cargo test --features stateright
                            Checks: V:MODEL (protocol model checking)
                            Coverage: 15 INVs with protocol safety/liveness
                            Time: <30m (state space exploration)

Gate 7: miri (optional)   — cargo +nightly miri test
                            Checks: V:MIRI (undefined behavior detection)
                            Coverage: all unsafe code paths (should be none: #![forbid(unsafe_code)])
                            Time: <10m
```

**Gate progression**: Gates 1–4 run on every commit. Gate 5 runs on PRs targeting main.
Gate 6 runs nightly or on protocol-affecting changes. Gate 7 runs only if `unsafe` code
appears (should never occur — `#![forbid(unsafe_code)]`).

**Failure handling**: A gate failure blocks merge. The implementing agent must fix the
failing invariant before proceeding. Gate failures are recorded as datoms (CO-011).

### §16.3 Typestate Encoding Catalog

Protocols enforced at compile time via Rust's type system (zero runtime cost):

| Protocol | Types | Transitions | INV |
|----------|-------|-------------|-----|
| Transaction lifecycle | `Building → Committed → Applied` | `commit()`, `apply()` | INV-STORE-001 |
| EntityId construction | `EntityId(hash)` — no public constructor from arbitrary bytes | content-addressed only | INV-STORE-003 |
| Store immutability | `&Store` for reads, `&mut Store` only via `transact`/`merge` | borrow checker | INV-STORE-001 |
| Schema attribute | `Attribute` newtype — cannot confuse with raw strings | type-safe attribute refs | INV-SCHEMA-003 |
| Schema monotonicity | `SchemaEvolution(datoms)` — no `DROP` or `ALTER DELETE` | append-only by type | INV-SCHEMA-004 |
| Query mode | `QueryMode::Monotonic \| Stratified(Frontier) \| Barriered(BarrierId)` | parse-time enforcement | INV-QUERY-005 |
| FFI boundary | `FfiFunction` trait with `pure` marker — host-language functions can't mutate store | type-level purity | INV-QUERY-006 |
| Resolution mode | `ResolutionMode` enum — exhaustive match required | compile-time completeness | INV-RESOLUTION-001 |
| MCP tool set | `const MCP_TOOLS: [MCPTool; 6]` — fixed-size array | compile-time tool count | INV-INTERFACE-003 |

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

### §16.5 Kani Feasibility Assurance

All 41 V:KANI-tagged invariants target **Level 2 implementation contracts** — bounded,
concrete Rust code operating on small inputs (3-5 datoms, <=8 graph vertices, <=20 operations).
Kani verifies these contracts exhaustively within the declared unwind bound. The Level 0
algebraic properties (which may involve unbounded domains) are covered by V:PROP (proptest),
not by Kani.

**Potential misconceptions resolved:**

- **INV-QUERY-001 (CALM Compliance)**: The Kani harness does NOT attempt to prove Datalog
  soundness in general. It verifies the **parser rejection path**: for all bounded AST
  combinations, Monotonic mode rejects expressions containing negation or aggregation.
  This is a finite-state property over a bounded enum tree — well within Kani's capabilities.

- **INV-QUERY-004 (Branch Visibility)**: The Kani harness does NOT model arbitrary branch
  topologies. It verifies **snapshot isolation for a single branch**: given a bounded store
  and one branch with a fork point, the visible set equals trunk datoms at fork plus
  branch-only datoms. Bounded to <=5 datoms and 1 branch — feasible.

**Why every V:KANI harness is feasible:**

| Category | INVs | Kani Strategy | Bound |
|----------|------|---------------|-------|
| Append-only / monotonicity | STORE-001/002/007/008, HARVEST-001, MERGE-001/008 | Verify datom count non-decreasing after bounded op sequences | <=20 ops |
| Content-addressing | STORE-003/010 | Stub hash with simpler function; verify structural properties | <=5 datoms |
| CRDT algebra | STORE-004/005/006, RESOLUTION-002/005/006 | Two/three bounded stores; verify algebraic law on merge | <=5 datoms/store |
| Schema validation | SCHEMA-001/002/004 | Bounded attribute set; verify rejection of invalid datoms | <=17 attributes |
| Graph algorithms | QUERY-012/013/017 | Bounded adjacency matrix; verify sort/SCC/critical path | <=8 vertices |
| Parser rejection | QUERY-001 | Enumerate AST node combinations; verify mode enforcement | <=10 clauses |
| Branch visibility | QUERY-004 | Bounded store + branch fork; verify visibility set | <=5 datoms, 1 branch |
| LIVE index / resolution | STORE-012, RESOLUTION-004/007 | Bounded attribute history; verify resolved value | <=5 values/attr |
| Budget / bounds | SEED-002/003, BUDGET-001/003/006 | Arithmetic on bounded numeric inputs | <=1000 tokens |
| Lifecycle guards | HARVEST-006, DELIBERATION-002/005, GUIDANCE-006 | Bounded state; verify guard enforcement | <=5 candidates |
| Signal system | SIGNAL-001/003/005 | Bounded subscription list + signal sequence | <=5 subscriptions |
| Merge isolation | MERGE-003/004/005 | Bounded branch pair; verify isolation/DCC | <=3 branches |

**Infeasible Kani count: 0.** Every V:KANI invariant has a concrete, bounded harness design.
The verification pipeline achieves **100% feasibility** — no invariant relies on a
verification method that cannot be practically executed.

### §16.6 Verification Statistics

| Metric | Count | Coverage |
|--------|-------|----------|
| Total invariants | 124 | — |
| With V:PROP | 121 | 97.6% |
| With V:TYPE (compile-time) | 10 | 8.1% |
| With V:PROP or V:TYPE (minimum) | 124 | 100% |
| With V:KANI | 41 | 33.1% |
| With V:MODEL | 15 | 12.1% |
| Stage 0 invariants | 64 | 51.6% |
| Stage 1 invariants | 25 | 20.2% |
| Stage 2 invariants | 22 | 17.7% |
| Stage 3 invariants | 11 | 8.9% |
| Stage 4 invariants | 2 | 1.6% |
| V:KANI feasibility | 41/41 | 100% |

---

