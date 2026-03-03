# R5.2a: Free Functions Audit

> **Bead**: R5.2a
> **Traces to**: ADR-ARCHITECTURE-001 (planned); guide/00-architecture.md SS0.1 (kernel/binary split)
> **Status**: Audit complete

---

## 1. Context

The Braid architecture places all domain logic in `braid-kernel`, a pure
computation library with `#![forbid(unsafe_code)]`, no IO, no async, and
deterministic functions. The binary crate `braid` is a thin wrapper for IO.

The architectural intent is clear: `braid-kernel` should expose **free functions**
that take `&Store` (or `&mut Store`) as an explicit parameter, rather than
methods on `impl Store`. This keeps the `Store` type focused on its core
responsibility (the datom set, indexes, frontier, and schema) and prevents it
from becoming a god object that accumulates every operation in every namespace.

However, the spec and guide files are inconsistent. Some functions appear as
`impl Store { fn xxx }` while others appear as `pub fn xxx(store: &Store, ...)`.
This audit catalogs every function signature, identifies the correct placement
per the planned ADR-ARCHITECTURE-001, and flags inconsistencies.

---

## 2. Audit Methodology

For each function, we record:
- **Current placement**: How it appears in spec and/or guide files
- **Recommended placement**: Whether it should be a Store method or a free function
- **Namespace**: Which braid-kernel module owns the function
- **Rationale**: Why it belongs where we recommend

The guiding principle: **Store methods are operations that directly mutate or
observe the core store invariants (datom set, indexes, frontier, schema). Everything
else is a free function that operates on a Store reference.**

---

## 3. Functions Currently on `impl Store` (Spec Files)

### 3.1 STORE Namespace (spec/01-store.md)

| Function | Signature | Recommended | Rationale |
|---|---|---|---|
| `genesis()` | `impl Store { pub fn genesis() -> Self }` | **KEEP as method** | Constructor. Creates the Store itself. |
| `transact()` | `impl Store { pub fn transact(&mut self, tx) -> Result<TxReceipt, TxApplyError> }` | **KEEP as method** | Core mutation. Directly modifies datom set, indexes, frontier. |
| `merge()` | `impl Store { pub fn merge(&mut self, other: &Store) -> MergeReceipt }` | **MOVE to free function** | Merge is a set-union operation that composes with cascade steps. Spec/01 defines it as a method; guide/07 already defines it as `pub fn merge(target: &mut Store, source: &Store)`. Free function is correct. |
| `current()` | `impl Store { pub fn current(&self, entity: EntityId) -> EntityView }` | **KEEP as method** | Direct read from indexes. Core store observation. |
| `as_of()` | `impl Store { pub fn as_of(&self, frontier: &Frontier) -> SnapshotView }` | **KEEP as method** | Core frontier-scoped read. |
| `len()` | `impl Store { pub fn len(&self) -> usize }` | **KEEP as method** | Trivial accessor. |
| `datoms()` | `impl Store { pub fn datoms(&self) -> impl Iterator<Item = &Datom> }` | **KEEP as method** | Trivial accessor. |
| `frontier()` | `impl Store { pub fn frontier(&self) -> &HashMap<AgentId, TxId> }` | **KEEP as method** | Trivial accessor. |
| `query()` | `impl Store { pub fn query(&mut self, q: &Query) -> QueryResult }` | **MOVE to free function** | Query is a complex operation spanning the QUERY namespace. It mutates store only for provenance tx recording (INV-STORE-014), which should be an explicit `transact` call by the query function rather than an implicit side effect inside Store. |

**Note on `query()`**: The spec defines `query()` on `impl Store` in two places
(spec/01-store.md line 852 and spec/03-query.md line 190-191). In both cases it
takes `&mut self`, which is needed for the provenance transaction
(INV-STORE-014). The guide (guide/03-query.md line 37) correctly defines it as a
free function: `pub fn query(store: &Store, expr: &QueryExpr, mode: QueryMode)`.
The free function form is preferred because query is a complex operation with its
own stratum classification, FFI boundary, and access log -- none of which are
core Store concerns. The provenance transaction should be a separate `transact`
call within the query function body.

---

### 3.2 SCHEMA Namespace (spec/02-schema.md)

| Function | Signature | Recommended | Rationale |
|---|---|---|---|
| `schema()` | `impl Store { pub fn schema(&self) -> &Schema }` | **KEEP as method** | Trivial accessor for derived cache. |

No issues. Schema is stored as a derived field on Store.

---

### 3.3 QUERY Namespace (spec/03-query.md)

| Function | Signature | Recommended | Rationale |
|---|---|---|---|
| `query()` | `impl Store { pub fn query(&mut self, expr: &QueryExpr, mode: QueryMode) -> Result<QueryResult, QueryError> }` | **MOVE to free function** | See analysis in SS3.1. Guide already uses free form. |

All graph algorithms in spec/03-query.md are already correctly defined as free
functions. No issues with:

- `topo_sort(store: &Store, ...)` (line 518)
- `tarjan_scc(store: &Store, ...)` (line 572)
- `pagerank(store: &Store, ...)` (line 623)
- `betweenness_centrality(store: &Store, ...)` (line 669)
- `hits(store: &Store, ...)` (line 722)
- `critical_path(store: &Store, ...)` (line 779)
- `k_core_decomposition(store: &Store, ...)` (line 824)
- `eigenvector_centrality(store: &Store, ...)` (line 867)
- `articulation_points(store: &Store, ...)` (line 922)
- `graph_density(store: &Store, ...)` (line 978)

---

### 3.4 HARVEST Namespace (spec/05-harvest.md)

| Function | Signature | Recommended | Rationale |
|---|---|---|---|
| `harvest_detect()` | `impl Store { pub fn harvest_detect(&self, agent: AgentId) -> Vec<HarvestCandidate> }` | **MOVE to free function** | Harvest detection is a pipeline operation. Guide already defines it as `pub fn harvest_pipeline(store: &Store, session_context: &SessionContext) -> HarvestResult`. |
| `harvest_commit()` | `impl Store { pub fn harvest_commit(&mut self, agent, candidates, topology) -> Result<HarvestSession, HarvestError> }` | **MOVE to free function** | Harvest commit should be decomposed into `accept_candidate()` (builds transaction) + `Store::transact()` (applies it). Guide correctly separates these: `pub fn accept_candidate(candidate, agent) -> Transaction<Building>`. |

**Inconsistency**: The spec (spec/05-harvest.md) defines both functions as Store
methods. The guide (guide/05-harvest.md) correctly decomposes them into free
functions. The guide's decomposition is architecturally superior because:

1. `harvest_pipeline()` is a pure function: `(Store, SessionContext) -> HarvestResult`.
   It only reads from the store.
2. `accept_candidate()` produces a `Transaction<Building>` which is then committed
   via the existing `Store::transact()`. This reuses the core mutation path
   rather than creating a parallel mutation path.
3. `harvest_session_entity()` produces another transaction for metadata. Same pattern.

---

### 3.5 SEED Namespace (spec/06-seed.md)

| Function | Signature | Recommended | Rationale |
|---|---|---|---|
| `associate()` | `impl Store { pub fn associate(&self, cue: AssociateCue) -> SchemaNeighborhood }` | **MOVE to free function** | Association is a query-layer operation. Guide defines it as `pub fn associate(store: &Store, cue: AssociateCue) -> SchemaNeighborhood`. |
| `assemble()` | `impl Store { pub fn assemble(&self, query_results, neighborhood, budget) -> AssembledContext }` | **MOVE to free function** | Assembly is a compression operation. Guide defines it as `pub fn assemble_seed(store: &Store, task: &str, budget: usize) -> SeedOutput`. |
| `seed()` | `impl Store { pub fn seed(&mut self, task: &str, budget: usize) -> Result<AssembledContext, SeedError> }` | **MOVE to free function** | Composite operation (associate + query + assemble). Guide uses `assemble_seed()` as the entry point. |

**Inconsistency**: The spec places all three seed functions on `impl Store`.
The guide correctly makes them all free functions. The guide additionally defines
helper free functions:
- `relevance_score(datom: &Datom, store: &Store, task: &str) -> f64`
- `generate_claude_md(store: &Store, task: &str, budget: usize) -> String`
- `compress_seed(seed: &SeedOutput, budget: usize) -> SeedOutput`

All of these are correctly free functions.

---

### 3.6 MERGE Namespace (spec/07-merge.md)

| Function | Signature | Recommended | Rationale |
|---|---|---|---|
| `merge()` | `impl Store { pub fn merge(&mut self, other: &Store) -> MergeReceipt }` | **MOVE to free function** | Guide already defines as `pub fn merge(target: &mut Store, source: &Store) -> MergeReceipt`. Merge is a set-algebraic operation that composes with cascade steps. |
| `fork()` | `impl Store { pub fn fork(&mut self, agent, purpose) -> Result<Branch, BranchError> }` | **MOVE to free function** (Stage 2) | Branch creation is a lifecycle operation. Mutation is via transact. |
| `commit_branch()` | `impl Store { pub fn commit_branch(&mut self, branch) -> Result<TxReceipt, BranchError> }` | **MOVE to free function** (Stage 2) | Branch commit decomposes to set union + transact. |
| `compare_branches()` | `impl Store { pub fn compare_branches(&mut self, branches, criterion) -> Result<BranchComparison, BranchError> }` | **MOVE to free function** (Stage 2) | Analysis operation; reads store, produces comparison entity. |

---

### 3.7 SYNC Namespace (spec/08-sync.md)

| Function | Signature | Recommended | Rationale |
|---|---|---|---|
| `sync_barrier()` | `impl Store { pub fn sync_barrier(&mut self, ...) }` | **MOVE to free function** (Stage 3) | Sync is a coordination protocol operation. Should compose with Store via explicit transact calls. |

---

### 3.8 GUIDANCE Namespace (spec/12-guidance.md)

All functions in the guidance spec are defined on other types
(`impl GuidanceTopology` and `impl MethodologyScore`) or as free functions
(`derive_tasks()`, `route_work()`, `topology_fitness()`). No Store methods.
This is correct.

However, `impl GuidanceTopology` has methods `query()` and `footer()` that take
`store: &Store` as a parameter, which is the correct pattern -- they are methods
on the topology, not on the store.

---

## 4. Functions Currently as Free Functions (Guide Files)

These are all correct and should remain free functions.

### 4.1 QUERY Namespace (guide/03-query.md)

| Function | Signature | Namespace |
|---|---|---|
| `query()` | `pub fn query(store: &Store, expr: &QueryExpr, mode: QueryMode) -> Result<QueryResult, QueryError>` | query/mod.rs |
| `parse()` | `pub fn parse(q: &str) -> Result<ParsedQuery, ParseError>` | query/parser.rs |
| `classify_stratum()` | `pub fn classify_stratum(q: &ParsedQuery) -> Stratum` | query/strata.rs |
| `topo_sort()` | `pub fn topo_sort(store: &Store, ...)` | query/graph.rs |
| `tarjan_scc()` | `pub fn tarjan_scc(store: &Store, ...)` | query/graph.rs |
| `pagerank()` | `pub fn pagerank(store: &Store, ...)` | query/graph.rs |
| `critical_path()` | `pub fn critical_path(store: &Store, ...)` | query/graph.rs |
| `graph_density()` | `pub fn graph_density(store: &Store, ...)` | query/graph.rs |

All correctly placed. Pure functions taking `&Store`.

### 4.2 RESOLUTION Namespace (guide/04-resolution.md)

| Function | Signature | Namespace |
|---|---|---|
| `resolve()` | `pub fn resolve(conflict: &ConflictSet, mode: &ResolutionMode) -> ResolvedValue` | resolution.rs |
| `has_conflict()` | `pub fn has_conflict(conflict: &ConflictSet, mode: &ResolutionMode) -> bool` | resolution.rs |
| `live_entity()` | `pub fn live_entity(store: &Store, entity: EntityId) -> HashMap<Attribute, ResolvedValue>` | resolution.rs |
| `detect_conflicts()` | `pub fn detect_conflicts(store: &Store, frontier: &...) -> Vec<ConflictSet>` | resolution.rs |
| `route_conflict()` | `pub fn route_conflict(conflict: &ConflictSet, mode: &ResolutionMode) -> RoutingTier` | resolution.rs |

All correctly placed. `resolve()` and `has_conflict()` do not even need Store --
they operate on a ConflictSet.

### 4.3 HARVEST Namespace (guide/05-harvest.md)

| Function | Signature | Namespace |
|---|---|---|
| `harvest_pipeline()` | `pub fn harvest_pipeline(store: &Store, session_context: &SessionContext) -> HarvestResult` | harvest.rs |
| `accept_candidate()` | `pub fn accept_candidate(candidate: &HarvestCandidate, agent: AgentId) -> Transaction<Building>` | harvest.rs |
| `harvest_session_entity()` | `pub fn harvest_session_entity(result: &HarvestResult, ...) -> Transaction<Building>` | harvest.rs |

All correctly placed. `accept_candidate()` does not need Store at all -- it builds
a transaction from a candidate. `harvest_pipeline()` reads from Store.

### 4.4 SEED Namespace (guide/06-seed.md)

| Function | Signature | Namespace |
|---|---|---|
| `assemble_seed()` | `pub fn assemble_seed(store: &Store, task: &str, budget: usize) -> SeedOutput` | seed.rs |
| `relevance_score()` | `pub fn relevance_score(datom: &Datom, store: &Store, task: &str) -> f64` | seed.rs |
| `generate_claude_md()` | `pub fn generate_claude_md(store: &Store, task: &str, budget: usize) -> String` | seed.rs |
| `associate()` | `pub fn associate(store: &Store, cue: AssociateCue) -> SchemaNeighborhood` | seed.rs |
| `compress_seed()` | `pub fn compress_seed(seed: &SeedOutput, budget: usize) -> SeedOutput` | seed.rs |

All correctly placed. `compress_seed()` is particularly noteworthy -- it does not
need Store at all, operating purely on the SeedOutput value.

### 4.5 MERGE Namespace (guide/07-merge-basic.md)

| Function | Signature | Namespace |
|---|---|---|
| `merge()` | `pub fn merge(target: &mut Store, source: &Store) -> MergeReceipt` | merge.rs |

Correctly placed. The guide form `merge(target: &mut Store, source: &Store)`
is architecturally superior to the spec's `impl Store { fn merge(&mut self, other: &Store) }`
because it makes both stores explicit parameters, which is clearer for reasoning
about the operation's semantics.

### 4.6 GUIDANCE Namespace (guide/08-guidance.md)

| Function | Signature | Namespace |
|---|---|---|
| `guidance_footer()` | `pub fn guidance_footer(store: &Store, drift_signals: &DriftSignals) -> GuidanceFooter` | guidance.rs |
| `detect_drift()` | `pub fn detect_drift(store: &Store, agent: AgentId, recent_commands: &[CommandRecord]) -> DriftSignals` | guidance.rs |
| `full_guidance()` | `pub fn full_guidance(store: &Store, agent: AgentId) -> GuidanceOutput` | guidance.rs |
| `methodology_score()` | `pub fn methodology_score(store: &Store, session: &SessionState) -> MethodologyScore` | methodology.rs |
| `derive_tasks()` | `pub fn derive_tasks(store: &Store, artifact: EntityId, rules: &[DerivationRule]) -> Vec<Datom>` | derivation.rs |
| `load_derivation_rules()` | `pub fn load_derivation_rules(store: &Store) -> Vec<DerivationRule>` | derivation.rs |
| `route_work()` | `pub fn route_work(store: &Store, weights: &[f64; 6]) -> Option<RoutingDecision>` | routing.rs |

All correctly placed. These are the most complex free functions in the system,
spanning multiple kernel modules.

### 4.7 INTERFACE Namespace (guide/09-interface.md)

| Function | Signature | Namespace |
|---|---|---|
| `format_output()` | `pub fn format_output(response: &ToolResponse, mode: OutputMode, footer: &GuidanceFooter) -> String` | braid/src/output.rs (binary crate) |
| `load_store()` | `pub fn load_store(path: &Path) -> Result<Store, PersistenceError>` | braid/src/persistence.rs (binary crate) |
| `save_store()` | `pub fn save_store(store: &Store, path: &Path) -> Result<(), PersistenceError>` | braid/src/persistence.rs (binary crate) |

Correctly placed in the binary crate (IO boundary).

---

## 5. Summary of Inconsistencies

### Functions That Need to Move: Spec Says Method, Guide Says Free Function

| Function | Spec file | Spec form | Guide file | Guide form | Recommended |
|---|---|---|---|---|---|
| `query()` | spec/01-store.md, spec/03-query.md | `impl Store` | guide/03-query.md | free fn | **Free function** |
| `merge()` | spec/01-store.md, spec/07-merge.md | `impl Store` | guide/07-merge-basic.md | free fn | **Free function** |
| `harvest_detect()` | spec/05-harvest.md | `impl Store` | guide/05-harvest.md | free fn (`harvest_pipeline`) | **Free function** |
| `harvest_commit()` | spec/05-harvest.md | `impl Store` | guide/05-harvest.md | decomposed to free fns | **Decompose** |
| `associate()` | spec/06-seed.md | `impl Store` | guide/06-seed.md | free fn | **Free function** |
| `assemble()` | spec/06-seed.md | `impl Store` | guide/06-seed.md | free fn (`assemble_seed`) | **Free function** |
| `seed()` | spec/06-seed.md | `impl Store` | guide/06-seed.md | free fn (`assemble_seed`) | **Free function** |
| `fork()` | spec/07-merge.md | `impl Store` | (Stage 2) | -- | **Free function** |
| `commit_branch()` | spec/07-merge.md | `impl Store` | (Stage 2) | -- | **Free function** |
| `compare_branches()` | spec/07-merge.md | `impl Store` | (Stage 2) | -- | **Free function** |
| `sync_barrier()` | spec/08-sync.md | `impl Store` | (Stage 3) | -- | **Free function** |

### Functions That Should Remain Store Methods

| Function | Namespace | Rationale |
|---|---|---|
| `genesis()` | STORE | Constructor |
| `transact()` | STORE | Core mutation of datom set, indexes, frontier |
| `current()` | STORE | Direct indexed read |
| `as_of()` | STORE | Frontier-scoped read |
| `len()` | STORE | Trivial accessor |
| `datoms()` | STORE | Trivial accessor |
| `frontier()` | STORE | Trivial accessor |
| `schema()` | SCHEMA | Trivial accessor (derived cache) |

### Functions Already Correctly Free (No Change Needed)

All graph algorithms (10 functions in spec/03-query.md): **Correct**.
All resolution functions (5 functions in guide/04-resolution.md): **Correct**.
All guidance functions (7 functions in guide/08-guidance.md): **Correct**.
All harvest helpers (3 functions in guide/05-harvest.md): **Correct**.
All seed helpers (5 functions in guide/06-seed.md): **Correct**.
All interface/persistence functions (3 functions in guide/09-interface.md): **Correct**.

---

## 6. The Store Public API After Reconciliation

After applying all recommendations, `impl Store` would contain only:

```rust
impl Store {
    // Constructors
    pub fn genesis() -> Self;

    // Core mutation (the ONLY mutation path)
    pub fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError>;

    // Read accessors
    pub fn current(&self, entity: EntityId) -> EntityView;
    pub fn as_of(&self, frontier: &Frontier) -> SnapshotView;
    pub fn len(&self) -> usize;
    pub fn datoms(&self) -> impl Iterator<Item = &Datom>;
    pub fn frontier(&self) -> &HashMap<AgentId, TxId>;
    pub fn schema(&self) -> &Schema;

    // Internal (pub(crate)) helpers needed by free functions
    pub(crate) fn insert_datom(&mut self, datom: Datom);
    pub(crate) fn rebuild_indexes_incremental(&mut self, from: usize);
}
```

Everything else is a free function in its respective namespace module.

---

## 7. Module-to-Function Mapping

| Module (braid-kernel/src/) | Functions |
|---|---|
| `store.rs` | `impl Store { genesis, transact, current, as_of, len, datoms, frontier }` |
| `schema.rs` | `impl Store { schema }` (or `impl Schema { ... }` with accessor on Store) |
| `query/mod.rs` | `query(store, expr, mode)` |
| `query/parser.rs` | `parse(q)`, `classify_stratum(q)` |
| `query/graph.rs` | `topo_sort(store, ...)`, `tarjan_scc(store, ...)`, `pagerank(store, ...)`, `betweenness_centrality(store, ...)`, `hits(store, ...)`, `critical_path(store, ...)`, `k_core_decomposition(store, ...)`, `eigenvector_centrality(store, ...)`, `articulation_points(store, ...)`, `graph_density(store, ...)` |
| `resolution.rs` | `resolve(conflict, mode)`, `has_conflict(conflict, mode)`, `live_entity(store, entity)`, `detect_conflicts(store, frontier)`, `route_conflict(conflict, mode)` |
| `harvest.rs` | `harvest_pipeline(store, context)`, `accept_candidate(candidate, agent)`, `harvest_session_entity(result, ...)` |
| `seed.rs` | `assemble_seed(store, task, budget)`, `associate(store, cue)`, `relevance_score(datom, store, task)`, `generate_claude_md(store, task, budget)`, `compress_seed(seed, budget)` |
| `merge.rs` | `merge(target, source)` |
| `guidance.rs` | `guidance_footer(store, signals)`, `detect_drift(store, agent, commands)`, `full_guidance(store, agent)` |
| `methodology.rs` | `methodology_score(store, session)` |
| `derivation.rs` | `derive_tasks(store, artifact, rules)`, `load_derivation_rules(store)` |
| `routing.rs` | `route_work(store, weights)` |

---

## 8. Action Items

1. **Spec files need updating** to move 11 functions from `impl Store` to free
   function form. The guide files already have the correct signatures. This is a
   documentation change, not a code change (no implementation exists yet).

2. **ADR-ARCHITECTURE-001 should be formalized** in spec/ to codify the principle:
   "Store methods are limited to constructors, core mutation (transact), and
   trivial accessors. All other operations are free functions taking `&Store`
   or `&mut Store` as an explicit parameter."

3. **The `query()` `&mut self` problem** needs resolution. The spec requires
   `&mut self` for provenance transaction recording (INV-STORE-014), but the
   guide uses `&Store`. Resolution: the free `query()` function takes `&Store`
   for the query itself, then calls `Store::transact()` for the provenance
   record. This requires the caller to have `&mut Store`, but the query
   function's signature reflects that it reads the store and returns results,
   with the provenance recording as a documented side-responsibility of the
   caller (or handled by a wrapper in the CLI layer).

---

*This audit finds the guide files are architecturally consistent with the free-function
principle. The spec files predate this decision and contain 11 functions incorrectly
placed on `impl Store`. The implementation should follow the guide signatures, and the
spec files should be updated to match.*
