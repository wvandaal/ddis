# §0. Architecture, Type Catalog & LLM-Native Interface Design

> **Spec reference**: [spec/00-preamble.md](../spec/00-preamble.md)
> **SEED.md**: §4, §10, §11
> **ADRS.md**: FD-001–012, AS-001–010, SR-001–011
> **Read this file after the spec preamble, before any namespace guide.**

---

## §0.1 Crate Workspace Layout

```
braid/                          ← Cargo workspace root
├── Cargo.toml                  ← workspace manifest
├── crates/
│   ├── braid-kernel/           ← Pure computation library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              ← Public re-exports
│   │       ├── datom.rs            ← Datom, EntityId, TxId, Op, Value
│   │       ├── store.rs            ← Store, transact, merge
│   │       ├── layout.rs           ← Canonical serialization, transaction hashing (pure)
│   │       ├── schema.rs           ← Schema, genesis, attribute registry
│   │       ├── query/
│   │       │   ├── mod.rs          ← Query engine entry point
│   │       │   ├── parser.rs       ← Datalog parser (pest or nom)
│   │       │   ├── evaluator.rs    ← Semi-naive fixpoint evaluator
│   │       │   ├── clause.rs       ← Clause, Binding, Pattern
│   │       │   ├── strata.rs       ← Stratum classification (CALM)
│   │       │   └── graph.rs        ← Graph engine: topo sort, SCC, PageRank, critical path, density
│   │       │                         Note: graph.rs provides dependency graph operations used by
│   │       │                         multiple namespaces (GUIDANCE for R(t), BILATERAL for dependency
│   │       │                         analysis). Placed in query module as primary consumer.
│   │       ├── resolution.rs       ← ResolutionMode, ConflictSet, resolve
│   │       ├── harvest.rs          ← HarvestCandidate, HarvestPipeline, gap detection
│   │       ├── seed.rs             ← SeedAssembly, associate/assemble/compress
│   │       ├── guidance.rs         ← GuidanceFooter, drift detection, anti-drift
│   │       ├── methodology.rs      ← M(t) adherence score, component computation
│   │       ├── derivation.rs       ← Task derivation rules, template matching
│   │       ├── routing.rs          ← R(t) graph-based work routing, impact scoring
│   │       ├── merge.rs            ← Pure set-union merge (Stage 0 subset)
│   │       ├── trilateral.rs       ← Trilateral coherence model (LIVE projections, Phi, formality)
│   │       └── frontier.rs         ← Frontier, AgentId, HLC clock
│   └── braid/                  ← Binary crate (CLI + MCP + persistence)
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs             ← clap CLI entry point
│           ├── commands/
│           │   ├── mod.rs
│           │   ├── transact.rs     ← braid transact
│           │   ├── query.rs        ← braid query
│           │   ├── status.rs       ← braid status
│           │   │                     Note: `braid status` is a derived CLI presentation
│           │   │                     command, not a protocol operation. It aggregates
│           │   │                     STORE and GUIDANCE namespace state. No separate
│           │   │                     spec formalization is required.
│           │   ├── harvest.rs      ← braid harvest
│           │   ├── seed.rs         ← braid seed
│           │   ├── guidance.rs     ← braid guidance
│           │   └── entity.rs       ← braid entity, braid history
│           ├── persistence.rs      ← Layout ↔ kernel Store bridge (filesystem I/O)
│           ├── output.rs           ← OutputMode dispatch (json/agent/human)
│           ├── mcp.rs              ← MCP server (6 tools, rmcp-based, persistent process)
│           └── claude_md.rs        ← Dynamic CLAUDE.md generation
└── tests/
    ├── proptest/               ← Property-based tests per namespace
    ├── integration/            ← Cross-namespace integration tests
    └── kani/                   ← Bounded model checking harnesses
```

### Design Invariants

- **`braid-kernel`**: `#![forbid(unsafe_code)]`. No IO. No async. No file system access.
  No network. Pure functions from `Store → Store`. Every kernel function is deterministic:
  same inputs → same outputs. This is the verification surface.

- **`braid`**: Thin wrapper. All domain logic delegated to `braid-kernel`.
  IO boundary: reads files, writes to layout directory, serves MCP, prints output.
  The binary crate contains no invariant-bearing logic.

### Re-Exports (`crates/braid-kernel/src/lib.rs`)

`braid-kernel` re-exports all public types so downstream crates use a single import root.
The binary crate depends on `braid-kernel` and accesses all types via `braid_kernel::*`.

```rust
// crates/braid-kernel/src/lib.rs

pub mod datom;
pub mod store;
pub mod layout;
pub mod schema;
pub mod query;
pub mod resolution;
pub mod harvest;
pub mod seed;
pub mod guidance;
pub mod methodology;
pub mod derivation;
pub mod routing;
pub mod merge;
pub mod trilateral;
pub mod frontier;

// Re-export core types at crate root for ergonomic access
pub use datom::{Datom, EntityId, TxId, AgentId, Op, Value, Attribute, ProvenanceType};
pub use store::{Store, Transaction, TxState, Building, Committed, Applied, TxData, TxReceipt};
pub use schema::{Schema, AttributeSpec, AttributeDef, ValueType, Cardinality};
pub use query::{QueryExpr, QueryResult, QueryMode, ParsedQuery, FindSpec, Stratum};
pub use resolution::ResolutionMode;
pub use frontier::Frontier;
pub use merge::MergeReceipt;
```

### Cargo.toml — Workspace Root

```toml
[workspace]
resolver = "2"
members = ["braid-kernel", "braid"]

[workspace.dependencies]
blake3 = "1"
ordered-float = "4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
proptest = "1"
```

### Cargo.toml — braid-kernel

```toml
[package]
name = "braid-kernel"
version = "0.1.0"
edition = "2021"

[dependencies]
blake3 = { workspace = true }
ordered-float = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
```

### Cargo.toml — braid (binary)

```toml
[package]
name = "braid"
version = "0.1.0"
edition = "2021"

[dependencies]
braid-kernel = { path = "../braid-kernel" }
arc-swap = "1"
clap = { version = "4", features = ["derive"] }
blake3 = "1"
hex = "0.4"
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }

[dev-dependencies]
proptest = { workspace = true }
assert_cmd = "2"
predicates = "3"
```

---

## §0.2 Core Type Catalog

These are the exact Rust types. The implementing agent writes the bodies; the guide
specifies the signatures and invariant contracts.

### Datom (INV-STORE-001, INV-STORE-003)

```rust
/// The atomic unit of information. Content-addressed: identity = hash(e, a, v, tx, op).
/// Five-tuple. Immutable after construction. Clone is the only way to propagate.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Datom {
    pub entity:    EntityId,
    pub attribute: Attribute,
    pub value:     Value,
    pub tx:        TxId,
    pub op:        Op,
}
```

### EntityId (INV-STORE-003, ADR-STORE-002)

```rust
/// Content-addressed entity identifier. BLAKE3 hash of semantic content.
/// No public constructor from raw bytes — only from content hashing.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct EntityId([u8; 32]);

impl EntityId {
    /// Compute EntityId from arbitrary content bytes.
    pub fn from_content(content: &[u8]) -> Self {
        Self(blake3::hash(content).into())
    }

    /// Compute EntityId for a temp-id (schema bootstrap, genesis).
    pub fn from_ident(keyword: &str) -> Self {
        Self::from_content(keyword.as_bytes())
    }

    /// Raw bytes (for serialization only).
    pub fn as_bytes(&self) -> &[u8; 32] { &self.0 }
}
// No: pub fn new(raw: [u8; 32]) -> Self — this would bypass content addressing (NEG-STORE-002)
```

### Attribute (INV-SCHEMA-003)

```rust
/// Keyword-style attribute. Always namespaced: `:db/ident`, `:spec/type`, etc.
/// Newtype prevents confusion with raw strings.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Attribute(String);

impl Attribute {
    pub fn new(keyword: &str) -> Result<Self, AttributeError> {
        // Must start with ':', must contain exactly one '/'
        // e.g., ":db/ident", ":spec/statement"
        validate_keyword(keyword)?;
        Ok(Self(keyword.to_string()))
    }

    pub fn namespace(&self) -> &str { /* before '/' */ }
    pub fn name(&self) -> &str { /* after '/' */ }
}
```

### Value (spec §1.1 Value Domain)

```rust
/// Polymorphic value. Matches the datom value domain from the spec.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Value {
    String(String),
    Keyword(String),     // :namespace/name
    Boolean(bool),
    Long(i64),
    Double(OrderedFloat<f64>),
    Instant(u64),        // millis since epoch
    Uuid([u8; 16]),
    Ref(EntityId),
    Bytes(Vec<u8>),
    // Stage 0 scope: the above types. Extended in later stages:
    // BigInt, BigDec, Tuple, Json, URI
}
```

### TxId — Hybrid Logical Clock (INV-STORE-008)

```rust
/// Transaction identifier. HLC: causally ordered, globally unique.
/// Monotone by construction: new TxId always > all previously observed TxIds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TxId {
    pub wall_time: u64,   // millis since epoch
    pub logical:   u32,   // counter for same-millisecond ordering
    pub agent:     AgentId,
}

/// Agent identifier. Fixed-size for hashing efficiency.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct AgentId([u8; 16]);  // UUID or hash of agent name
```

### Op (INV-STORE-001)

```rust
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Op { Assert, Retract }
```

### Transaction Typestate (INV-STORE-001, §1.3)

```rust
/// Typestate markers — zero-sized, compile-time only.
pub struct Building;
pub struct Committed;
pub struct Applied;

pub trait TxState: sealed::Sealed {}
impl TxState for Building {}
impl TxState for Committed {}
impl TxState for Applied {}

/// Transaction metadata (spec §1.3 references this as `TxData`).
/// The spec uses an opaque `tx_data: TxData` field; here we inline the fields
/// for clarity. Either representation is valid for implementation.
pub struct TxData {
    pub tx_entity:           EntityId,
    pub provenance:          ProvenanceType,
    pub causal_predecessors: Vec<TxId>,
    pub agent:               AgentId,
    pub rationale:           String,
}

pub struct Transaction<S: TxState> {
    datoms:   Vec<Datom>,
    tx_data:  TxData,
    _state:   PhantomData<S>,
}

// Building → Committed: validates schema, seals datoms
// Committed → Applied:  appends to store, updates indexes
// Applied:              read-only receipt
```

### ProvenanceType (ADR-STORE-008)

```rust
/// Provenance lattice: Observed > Derived > Inferred > Hypothesized
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum ProvenanceType {
    Hypothesized,  // 0.2 — lowest confidence
    Inferred,      // 0.5
    Derived,       // 0.8
    Observed,      // 1.0 — highest confidence
}

impl ProvenanceType {
    pub fn factor(&self) -> f64 {
        match self {
            Self::Hypothesized => 0.2,
            Self::Inferred     => 0.5,
            Self::Derived      => 0.8,
            Self::Observed     => 1.0,
        }
    }
}
```

### Store (INV-STORE-001–014)

```rust
pub struct Store {
    datoms:   BTreeSet<Datom>,
    indexes:  Indexes,
    frontier: HashMap<AgentId, TxId>,
    schema:   Schema,
}

impl Store {
    pub fn genesis() -> Self;
    pub fn transact(&mut self, tx: Transaction<Committed>) -> Result<TxReceipt, TxApplyError>;
    pub fn current(&self, entity: EntityId) -> EntityView;
    pub fn as_of(&self, frontier: &Frontier) -> SnapshotView;
    pub fn len(&self) -> usize;
    pub fn datoms(&self) -> impl Iterator<Item = &Datom>;
    pub fn frontier(&self) -> &HashMap<AgentId, TxId>;
    pub fn schema(&self) -> &Schema;
}
// Note: merge is a free function (ADR-ARCHITECTURE-001):
// pub fn merge(target: &mut Store, source: &Store) -> (MergeReceipt, CascadeReceipt);
```

### QueryMode (INV-QUERY-005)

```rust
pub enum QueryMode {
    Monotonic,                     // Stratum 0–1 only, no barriers
    Stratified(Frontier),          // Up to Stratum 5, frontier-scoped
    Barriered(BarrierId),          // Requires sync barrier (Stage 3)
}
```

### ResolutionMode (INV-RESOLUTION-001)

```rust
pub enum ResolutionMode {
    Lattice { lattice_id: EntityId },  // Join-semilattice — definition stored as datoms (C3)
    LastWriterWins,                    // Greatest HLC assertion
    MultiValue,                        // Set of all unretracted values
}
```

### Graph Engine Types (INV-QUERY-012–021)

```rust
/// Strongly connected components result (INV-QUERY-013).
pub struct SCCResult {
    pub components: Vec<Vec<EntityId>>,  // SCCs in reverse topological order
    pub condensation: Vec<Vec<usize>>,   // DAG adjacency list over SCC indices
    pub has_cycles: bool,                // true if any |SCC| > 1
}

/// PageRank configuration (INV-QUERY-014).
pub struct PageRankConfig {
    pub damping: f64,         // default: 0.85
    pub epsilon: f64,         // convergence: 1e-6
    pub max_iterations: u32,  // safety bound: 100
}

/// Critical path analysis result (INV-QUERY-017).
pub struct CriticalPathResult {
    pub path: Vec<EntityId>,               // critical path vertices
    pub total_weight: f64,                 // critical path length
    pub slack: HashMap<EntityId, f64>,     // slack per vertex (0.0 = critical)
    pub earliest_start: HashMap<EntityId, f64>,
    pub latest_start: HashMap<EntityId, f64>,
}

/// Graph density and health metrics (INV-QUERY-021).
pub struct GraphDensityMetrics {
    pub vertex_count: usize,
    pub edge_count: usize,
    pub density: f64,           // ∈ [0, 1]
    pub avg_degree: f64,
    pub avg_clustering: f64,    // ∈ [0, 1]
    pub components: usize,      // weakly connected component count
}

/// Graph algorithm errors.
pub enum GraphError {
    CycleDetected(SCCResult),   // Graph has cycles — includes SCC details
    EmptyGraph,                 // No vertices match the entity_type filter
    NonConvergence(u32),        // PageRank/eigenvector did not converge in N iterations
}
```

### Guidance Expansion Types (INV-GUIDANCE-008–010)

```rust
/// M(t) methodology adherence score (INV-GUIDANCE-008).
pub struct MethodologyScore {
    pub total: f64,              // M(t) ∈ [0, 1]
    pub components: [f64; 5],    // [transact_freq, spec_lang, query_div, harvest_q, guidance_c]
    pub weights: [f64; 5],       // loaded from store as `:guidance/m-weight` datoms
    pub trend: Trend,
}

pub enum Trend { Up, Down, Stable }

/// Task derivation rule (INV-GUIDANCE-009). Rules are datoms.
pub struct DerivationRule {
    pub entity: EntityId,
    pub artifact_type: String,
    pub task_template: TaskTemplate,
    pub dependency_fn: QueryExpr,
    pub priority_fn: PriorityFn,
}

pub struct TaskTemplate {
    pub task_type: String,
    pub title_pattern: String,
    pub attributes: Vec<(Attribute, ValueTemplate)>,
}

/// R(t) work routing decision (INV-GUIDANCE-010).
pub struct RoutingDecision {
    pub selected: EntityId,
    pub impact_score: f64,
    pub components: HashMap<String, f64>,
    pub alternatives: Vec<(EntityId, f64)>,
    pub ready_count: usize,
    pub blocked_count: usize,
    pub critical_path_remaining: usize,
}
```

### Cross-Namespace Types

Types defined in namespace-specific guide files but referenced across boundaries:

```rust
// --- Transaction results (§1 STORE) ---
pub struct TxReceipt {
    pub tx_id: TxId,
    pub datom_count: usize,
    pub new_entities: Vec<EntityId>,
}

pub enum TxValidationError {
    SchemaViolation { attr: Attribute, expected: ValueType, got: ValueType },
    UnknownAttribute(Attribute),
    InvalidRetraction(EntityId, Attribute),
}

// --- Schema (§2 SCHEMA) ---
pub struct Schema { /* extracted from schema datoms — see guide/02-schema.md */ }
pub enum SchemaError {
    DuplicateAttribute(Attribute),
    InvalidCardinality,
    LayerDependencyViolation { attr: Attribute, attr_layer: SchemaLayer, ref_layer: SchemaLayer },
}

// --- Query (§3 QUERY) ---
pub struct QueryResult {
    pub bindings: Vec<BindingSet>,
    pub stratum:  Stratum,
    pub mode:     QueryMode,
    pub provenance_tx: TxId,
}
pub struct QueryStats { pub datoms_scanned: usize, pub bindings_produced: usize }
pub type BindingSet = HashMap<Variable, Value>;
pub enum QueryExpr {
    Find(ParsedQuery),                          // Datalog pattern matching with joins
    Pull { pattern: PullPattern, entity: EntityRef }, // Entity-centric attribute retrieval
}
pub struct ParsedQuery {
    pub find_spec:     FindSpec,                // Which of four Datomic find forms
    pub where_clauses: Vec<Clause>,             // Conjunction of pattern clauses
    pub rules:         Vec<Rule>,               // Named rule definitions
    pub inputs:        Vec<Input>,              // :in parameterized bindings
}
pub struct FrontierRef(pub AgentId);  // Clause::Frontier operand (INV-QUERY-007)

// --- Merge (§7 MERGE) ---
pub struct MergeReceipt {
    pub new_datoms: usize,
    pub duplicate_datoms: usize,
    pub frontier_delta: HashMap<AgentId, (Option<TxId>, TxId)>,
}
pub struct CascadeReceipt {
    pub conflicts_detected: usize,
    pub caches_invalidated: usize,
    pub projections_staled: usize,
    pub uncertainties_updated: usize,
    pub notifications_sent: usize,
    pub cascade_datoms: Vec<Datom>,
}

// --- Harvest (§5 HARVEST) ---
pub struct HarvestCandidate {
    pub id: usize, pub datom_spec: Vec<Datom>, pub category: HarvestCategory,
    pub confidence: f64, pub weight: f64, pub status: CandidateStatus,
    pub extraction_context: String, pub reconciliation_type: ReconciliationType,
}
pub struct HarvestResult { pub candidates: Vec<HarvestCandidate>, pub drift_score: f64, pub quality: HarvestQuality }
pub enum CandidateStatus { Proposed, UnderReview, Committed, Rejected(String) }

// --- Seed (§6 SEED) ---
pub struct SchemaNeighborhood { pub entities: Vec<EntityId>, pub attributes: Vec<Attribute>, pub entity_types: Vec<Keyword> }
pub struct AssembledContext { pub sections: Vec<ContextSection>, pub total_tokens: usize, pub budget_remaining: usize, pub projection_pattern: ProjectionPattern }

// --- Guidance (§8 GUIDANCE) ---
pub struct GuidanceFooter {
    pub next_action: String, pub invariant_refs: Vec<String>,
    pub uncommitted_count: u32, pub drift_warning: Option<String>,
}

// --- Interface (§9 INTERFACE) ---
pub enum OutputMode { Json, Agent, Human }
pub struct ToolResponse { pub structured: Value, pub agent_context: String, pub agent_content: String, pub human_display: String }
```

---

## §0.3 File Formats

### Datom EDNL Interchange

One EDN datom per line. Used for `braid transact --file` and export (ADR-TRILATERAL-002):

```clojure
{:e #blake3 "a1b2c3..." :a :db/ident :v :spec/type :tx #hlc "1709000000000-0-agent1" :op :assert}
{:e #blake3 "d4e5f6..." :a :spec/statement :v "The store never deletes" :tx #hlc "1709000000000-0-agent1" :op :assert}
```

**Key conventions**:
- `e`: `#blake3 "hex..."` (tagged literal, 32 bytes hex-encoded)
- `a`: keyword (`:namespace/name`)
- `v`: EDN value — strings (`"..."`), keywords (`:keyword`), longs (`42`), refs (`#blake3 "..."`)
- `tx`: `#hlc "wall_time-logical-agent_hex"` (tagged literal)
- `op`: keyword (`:assert` or `:retract`)

### Layout Directory Schema

The on-disk layout uses **content-addressed per-transaction files** (see `spec/01b-storage-layout.md`).
The in-memory `Store` (BTreeSet of datoms) is authoritative. The layout directory provides
durable persistence; `.cache/` indexes provide accelerated lookups. Both are derived from
the canonical transaction files in `txns/`.

```
.braid/
├── txns/{hash[0..2]}/{full_blake3_hex}.edn  ← immutable, content-addressed transaction files
├── heads/{agent_id}.ref                      ← mutable frontier caches (derivable from txns/)
├── genesis.edn                               ← compile-time constant
├── .cache/                                   ← derived indexes (gitignored, rebuilt from txns/)
│   ├── eavt.idx, aevt.idx, vaet.idx, avet.idx, live.idx
└── .gitignore                                ← ignores .cache/
```

### Seed Output Template (ADR-SEED-004, spec/06-seed.md)

Five-part structure, each designed as a prompt component:

```markdown
## Orientation
{project_identity, current_phase, active_spec_section}

## Constraints
{relevant_INVs, settled_ADRs, negative_cases, commitment_weights}

## State
{recent_transactions, frontier_state, drift_score, active_uncertainties}

## Warnings
{drift_signals, open_questions, uncertainties, harvest_alerts}

## Directive
{next_task, acceptance_criteria, active_guidance_corrections}
```

### Dynamic CLAUDE.md Template (from spec/12-guidance.md, INV-GUIDANCE-007)

Seven-step generation pipeline. Each step applies prompt-optimization:

```markdown
# Braid — Session Context
<!-- Generated: {timestamp}, frontier: {agent_frontier}, k*: {remaining_budget} -->

## Active Methodology
{demonstrations_from_prior_sessions — one worked example showing harvest/seed}

## Constraints
{only_constraints_surviving_removal_test_at_current_k*}

## Current State
- Drift score: {F_S_score}
- Active basin: {A_or_B}
- Relevant INVs: {INVs_for_current_task}
- Unresolved: {uncertainty_markers_for_active_namespace}

## Session Objective
{task_description_with_traceability}

## Anti-Drift
{guidance_footer_appropriate_to_current_drift_signal}
```

---

## §0.3b Bootstrap Path (C7, SR-005)

The system initializes itself in three phases, implementing C7 (self-bootstrap):

### Phase 1: Empty → Schema-Enabled

```
braid init .braid/
```

1. Create empty store (BTreeSet = ∅)
2. Transact genesis datoms: 19 axiomatic meta-schema attributes (INV-SCHEMA-002)
3. Store now recognizes `:db/ident`, `:db/valueType`, etc.
4. Schema module can validate subsequent transactions

### Phase 2: Schema-Enabled → Spec-Enabled

```
braid transact --file spec-datoms.ednl
```

1. Load the specification elements from `spec/` as datoms
2. INVs, ADRs, NEGs become entities with `:spec/type`, `:spec/id`, `:spec/statement`
3. Cross-references (`:spec/traces-to`, `:spec/depends-on`) become ref datoms
4. **Dependency edges are REQUIRED, not optional (INV-SCHEMA-009)**: The bootstrap EDNL
   generator parses cross-references from spec markdown and emits corresponding
   `:spec/depends-on`, `:spec/affects`, `:spec/constrains`, and `:spec/tests` ref datoms.
   A bootstrap that transacts content without dependency relationships produces a flat
   store that cannot support impact analysis, contradiction chain tracing, or topology
   compilation (NEG-BOOTSTRAP-001).
5. Store now contains its own specification as queryable data with a connected dependency graph

### Phase 3: Spec-Enabled → Self-Verified

```
braid query '[:find ?inv :where [?inv :spec/type "invariant"] [?inv :spec/falsification ?f] [(missing? $ ?inv :spec/verified)]]'
```

1. Query the store for internal contradictions (INV-QUERY-001)
2. Verify all invariants have falsification conditions (C6)
3. Verify traceability: all INVs trace to SEED.md sections (C5)
4. Verify no orphans: all spec elements are reachable from the root goal

The system's first act of coherence verification is checking its own specification.
This is not ceremonial — it validates that the store, schema, query, and resolution
layers compose correctly before any user data enters the system.

---

## §0.4 CLI Command Signatures

All commands follow the pattern: parse args → load store from layout directory → call kernel function →
format output → write result. The binary crate is a thin adapter between IO and pure kernel.

### Output Modes (INV-INTERFACE-001)

Every command supports `--format {json,agent,human}`:

| Mode | Audience | Structure | Token Budget |
|------|----------|-----------|--------------|
| `json` | Programs, MCP | Full structured data, semantic keys | Unbounded |
| `agent` | LLM agents | `context + content + footer` | ≤300 tokens |
| `human` | Terminal users | Tables, colors, abbreviated | Unbounded |

Default: `agent` when `$BRAID_AGENT=1` (set by dynamic CLAUDE.md), `human` otherwise.

### Command Specifications

```rust
/// Top-level CLI (clap derive)
#[derive(Parser)]
#[command(name = "braid", about = "DDIS datom store")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format
    #[arg(long, default_value = "human")]
    format: OutputFormat,

    /// Store path (layout directory)
    #[arg(long, default_value = ".braid/")]
    store: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// Apply a transaction from file or inline
    Transact {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        inline: Option<String>,
        #[arg(long, default_value = "observed")]
        provenance: ProvenanceType,
    },
    /// Execute a Datalog query
    Query {
        /// Datalog query string
        query: String,
        /// Query mode
        #[arg(long, default_value = "monotonic")]
        mode: QueryMode,
    },
    /// Store summary
    Status,
    /// Extract session knowledge to datoms
    Harvest {
        /// Auto-accept high-confidence candidates
        #[arg(long)]
        auto: bool,
    },
    /// Assemble session context
    Seed {
        /// Task description for relevance scoring
        #[arg(long)]
        task: String,
    },
    /// Methodology guidance
    Guidance,
    /// Show entity datoms
    Entity {
        id: String,
    },
    /// Show attribute history
    History {
        entity: String,
        attribute: String,
    },
}
```

### Agent-Mode Output Structure

Every agent-mode response follows this three-part structure:

```
{context}     ← ≤50 tokens: activates the right cognitive mode
{content}     ← ≤200 tokens: the payload
{footer}      ← ≤50 tokens: guidance micro-prompt steering methodology
```

**Example** — `braid transact --file spec-bootstrap.jsonl --format agent`:

```
[STORE] Transacted 31 datoms (INV-STORE-001..014) in tx hlc:1709000000000-0-agent1.
Store: 48 datoms, frontier: {agent1: hlc:1709000000000-0-agent1}.
Genesis + spec bootstrap complete. Schema: 19 axiomatic + 14 spec attributes.
---
↳ What divergence type does this address? (C7: self-bootstrap)
  Next: `braid query '[:find ?id :where [?e :spec/type "invariant"] [?e :spec/id ?id]]'`
```

---

## §0.5 MCP Tool Definitions

Six tools at Stage 0 (INV-INTERFACE-003). The MCP server is a persistent process
using the `rmcp` crate for transport (ADR-INTERFACE-004). The store is loaded once
at initialization and held via `ArcSwap<Store>` for the session lifetime — the
Datomic connection model where Store values are immutable (C1) and the pointer swaps
atomically after write operations (transact, harvest). Reads are lock-free; in-flight
queries see a consistent snapshot. Schema consistency is structural under this model:
because Schema is owned by Store (ADR-SCHEMA-005, Option C), every MVCC snapshot
contains a Schema derived from exactly the datoms in that snapshot. No split-brain
between Schema and datoms is possible — no coordination protocol, no independent
Schema versioning, no stale-schema hazards. This is formalized in ADR-STORE-016
(ArcSwap MVCC Concurrency Model) and ADR-SCHEMA-005 Stage 3 Concurrency Analysis.
CLI and MCP both dispatch to the same kernel
functions (INV-INTERFACE-010) — MCP is an optimization, not a replacement for the
universal CLI interface. Tool handlers are annotated with rmcp's `#[tool]` macro,
which generates the `tools/list` response.

Each description is an optimized prompt: navigative purpose, semantic types, one
micro-example. Entity lookup, history, and CLAUDE.md generation are accessible via
`braid_query` and `braid_guidance` respectively.

```json
{
  "tools": [
    {
      "name": "braid_transact",
      "description": "Assert or retract datoms in the append-only store. Use when you have facts to record — decisions made, observations noted, specifications changed. Returns tx receipt with datom count and new frontier.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "datoms": { "type": "array", "items": { "$ref": "#/definitions/DatomInput" } },
          "provenance": { "enum": ["observed", "derived", "inferred", "hypothesized"] },
          "rationale": { "type": "string" }
        },
        "required": ["datoms"]
      }
    },
    {
      "name": "braid_query",
      "description": "Run a Datalog query or graph algorithm against the store. Use to find facts, entity details, history, dependencies, PageRank, critical path. Returns binding sets or graph metrics. Note: braid_query is a composite MCP tool that routes to query, entity, and history CLI operations via INV-INTERFACE-006 tool routing.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "query": { "type": "string", "description": "Datalog query or graph command (topo_sort, pagerank, critical_path, etc.)" },
          "mode": { "enum": ["monotonic", "stratified"], "default": "monotonic" },
          "entity": { "type": "string", "description": "Optional: entity lookup by id or :db/ident" },
          "history": { "type": "object", "description": "Optional: {entity, attribute} for value-over-time" }
        },
        "required": ["query"]
      }
    },
    {
      "name": "braid_status",
      "description": "Store summary: datom count, frontier, schema stats, M(t) adherence score, drift signals, graph density. Use for orientation at session start.",
      "inputSchema": { "type": "object", "properties": {} }
    },
    {
      "name": "braid_harvest",
      "description": "Extract session knowledge into datoms. Use near session end or when Q(t) is low. Presents candidates for accept/reject with confidence scores.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "auto": { "type": "boolean", "default": false }
        }
      }
    },
    {
      "name": "braid_seed",
      "description": "Assemble session context from store state. Use at session start with task description. Returns five-part trajectory seed (orientation, decisions, context, warnings, task).",
      "inputSchema": {
        "type": "object",
        "properties": {
          "task": { "type": "string", "description": "What you intend to do this session" }
        },
        "required": ["task"]
      }
    },
    {
      "name": "braid_guidance",
      "description": "Get methodology guidance: M(t) score, R(t) next task routing, drift signals, spec-language corrections. Use when uncertain about next step or to generate dynamic CLAUDE.md.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "generate_claude_md": { "type": "boolean", "default": false, "description": "If true, generate dynamic CLAUDE.md from store state" }
        }
      }
    }
  ]
}
```

Token budget per description: ≤100 tokens. Each follows: purpose (navigative) + use-when
(activation trigger) + returns (what to expect).

---

## §0.6 LLM-Native Interface Design

### The Principle

Every surface consumed by an LLM is an **optimized prompt**. This is not a feature — it is
a structural invariant. The data substrate (datoms) and the interface substrate (LLM-facing
outputs) are co-designed for coherence.

### Output Format Algebra

Three modes form a projection algebra over tool responses:

```
ToolResponse = {
    structured: Value,          // Full data (JSON mode)
    agent_context: String,      // ≤50 tokens — headline (agent mode)
    agent_content: String,      // ≤200 tokens — entities + signals (agent mode)
    human_display: String,      // Formatted for terminal (human mode)
}
```

Agent mode maps the spec's 5 semantic parts (ADR-INTERFACE-002: headline + entities +
signals + guidance + pointers) into 3 display zones during formatting:
- `agent_context` = headline
- `agent_content` = entities + signals
- footer (guidance + pointers) = injected by `format_output()` via `GuidanceFooter`

### Error Message Protocol

Every error is actionable by an LLM:

```
{what_failed} — {why} — {recovery_action} — {spec_ref}
```

**Demonstration** — schema validation failure:

```
Schema error: attribute `:spec/bogus` not in schema
— Unknown attribute (not in genesis or any schema transaction)
— Check available attributes: `braid query '[:find ?a :where [_ :db/ident ?a]]'`
— See: INV-SCHEMA-003 (schema-as-data), INV-SCHEMA-001 (genesis completeness)
```

**Demonstration** — query stratum violation:

```
Query error: aggregation in monotonic mode
— Aggregation (?count) requires stratified evaluation (Stratum 2+)
— Use --mode stratified: `braid query '...' --mode stratified`
— See: INV-QUERY-005 (mode-stratum compatibility), ADR-QUERY-003 (CALM compliance)
```

### Guidance Footer Design

The footer is a micro-prompt. Uses activation language (navigative, not instructive):

| Drift Signal | Footer Content | Anti-Drift Mechanism |
|-------------|----------------|---------------------|
| No DDIS commands in 5+ turns | "What divergence type does this address?" | Continuous injection |
| Schema changes without validation | "Which INV does this schema evolution preserve?" | Spec-language |
| High-confidence harvest candidate | "This epistemic gap has 0.9 confidence — transact now?" | Proactive warning |
| Approaching k* threshold | "Q(t) = 0.20. Harvest soon. What knowledge is at risk?" | Budget-aware |
| Agent using pretrained patterns | "Trace this decision to a SEED.md section." | Basin competition |
| Generic output without INV refs | "Which invariant's falsification condition does this satisfy?" | Spec-language |

Footer selection: choose the highest-priority signal. One footer per response. Priority:
budget warning > harvest prompt > drift correction > general guidance.

### TokenCounter Trait

Token counting is abstracted behind a trait to allow swappable implementations
across stages without changing callers. At Stage 0, the implementation is zero-dependency.

```rust
/// Trait for token counting. Swappable between stages.
/// Stage 0: ApproxTokenCounter (chars/4 + content-type heuristic, 0 deps)
/// Stage 1: TiktokenCounter (tiktoken-rs cl100k_base, ~90-95% Claude accuracy)
/// Stage 2+: AnthropicApiCounter (messages.countTokens API, ~99% accuracy)
pub trait TokenCounter: Send + Sync {
    /// Count the number of tokens in the given text.
    fn count(&self, text: &str) -> usize;

    /// Name of the counting method (for diagnostics/logging).
    fn method(&self) -> &'static str;
}

/// Stage 0 implementation: chars/4 with content-type correction.
/// Average error: ~15-20% vs real tokenizer.
/// Sufficient for coarse budget band selection (bands are 4x apart).
pub struct ApproxTokenCounter;

impl TokenCounter for ApproxTokenCounter {
    fn count(&self, text: &str) -> usize {
        let byte_count = text.len();
        let base = byte_count / 4;
        if looks_like_code(text) {
            base * 5 / 4  // 25% uplift for code
        } else {
            base
        }
    }

    fn method(&self) -> &'static str { "chars/4" }
}

fn looks_like_code(text: &str) -> bool {
    let indicators: &[&str] = &["{", "}", "(", ")", ";", "fn ", "let ", "pub ", "impl "];
    let score: usize = indicators.iter()
        .map(|i| text.matches(i).count())
        .sum();
    score > text.len() / 200  // > 0.5% indicator density
}
```

**Where TokenCounter is used**: Output budget cap (INV-BUDGET-001), guidance footer
size selection, projection pyramid level, command attention profile, token efficiency
metric (INV-BUDGET-006), CLAUDE.md budget validation. All callers accept `&dyn TokenCounter`,
enabling stage-based upgrades without API changes.

**Why chars/4 is sufficient at Stage 0**: The budget system uses coarse thresholds
(pi_0: >2000, pi_1: 500-2000, pi_2: 200-500, pi_3: <=200). A 15-20% error rarely
changes band selection. The token efficiency metric (INV-BUDGET-006) is a ratio where
consistent bias cancels out. See `audits/stage-0/research/D5-tokenizer-survey.md` for the full analysis.

### Token Efficiency Targets

| Surface | Budget | Metric |
|---------|--------|--------|
| Agent-mode output | ≤300 tokens | Context + content + footer |
| Guidance footer | ≤50 tokens | Single navigative question |
| Seed output | ≤ k*/4 of remaining | Five parts, declining relevance |
| Error message | ≤100 tokens | What + why + recovery + ref |
| MCP tool description | ≤100 tokens | Purpose + use-when + returns |
| Dynamic CLAUDE.md | ≤1000 tokens | Shrinks as k* decays |

### Resolved Spec Gaps

All gaps identified during guide production are now closed with formal invariants:

1. **Tool description quality metric** — Specified in INV-INTERFACE-008 (MCP Tool Description Quality):
   navigative structure, ≤100 tokens, semantic types, micro-example required.
2. **Error message recovery-hint completeness** — Specified in INV-INTERFACE-009 (Error Recovery
   Protocol Completeness) and NEG-INTERFACE-004: total recovery function, four-part error protocol.
3. **Dynamic CLAUDE.md as formally optimized prompt** — Specified in INV-GUIDANCE-007 (augmented):
   k* constraint budget, ambient/active partition, demonstration density, typestate pipeline.
4. **Token efficiency as testable property** — Specified in INV-BUDGET-006 (Token Efficiency as
   Testable Property): density monotonicity, mode-specific ceilings, rate-distortion bound.

---

## §0.7 Uncertainty Resolution Protocol

Three high-urgency uncertainties (spec/15-uncertainty.md) share a resolution pattern:

| Uncertainty | Uncertain Value | Resolution |
|-------------|----------------|------------|
| UNC-SCHEMA-001 | 19 axiomatic attributes sufficient? | Make attribute list a configurable genesis template. Instrument: log any "attribute not found" errors during Stage 0. Resolve: if 0 failures over 50 sessions, confirm sufficiency. |
| UNC-HARVEST-001 | Q(t) < 0.15 and < 0.05 thresholds | Make thresholds configurable datoms (`:braid/harvest-warn-threshold`, `:braid/harvest-only-threshold`). Instrument: log Q(t) at harvest time. Resolve: compute optimal threshold from harvest outcome data. |
| UNC-GUIDANCE-001 | Basin B crossover at 15–20 turns | Make crossover point a configurable datom (`:braid/drift-crossover`). Instrument: log turn count when agent first skips DDIS step. Resolve: compute empirical crossover from instrumentation data. |

**Pattern**: Make the uncertain value a configurable datom → instrument from day one →
resolve via empirical data from Stage 0 usage. The datom store stores its own uncertainty
resolution data — self-bootstrap at the epistemological level.

---

## §0.8 ADRs

### ADR-ARCHITECTURE-001: Free Functions Over Store Methods for Namespace Operations

**Traces to**: SEED.md §4, §10; ADRS FD-010, FD-012
**Stage**: 0

#### Problem

Should namespace operations (harvest, seed, merge, guidance, derivation, routing,
drift detection, etc.) be implemented as Store methods or as free functions that
accept a `&Store` / `&mut Store` parameter?

#### Options

A) **Free functions** — `query(store, expr, mode)`, `harvest_pipeline(store, session)`,
   `assemble_seed(store, task, budget)`, `merge(target, source)`, `guidance_footer(store, signals)`.
   Store methods are reserved for core datom operations: `store.genesis()`, `store.transact(tx)`,
   `store.current(entity)`, `store.as_of(frontier)`, `store.len()`, `store.datoms()`,
   `store.frontier()`, `store.schema()`.

B) **Store methods** — `store.harvest(session)`, `store.seed(task, budget)`, `store.guidance()`.
   All operations are methods on Store, centralizing the API surface.

C) **Trait-based** — Define traits like `Harvestable`, `Seedable`, `Guidable` and implement
   them on Store. Namespace operations accessed via trait methods: `store.harvest(session)`.

#### Decision

**Option A.** Free functions for all namespace operations. Store methods are reserved
exclusively for the core datom operations defined in spec/01-store.md §1.3 (genesis,
transact, current, as_of, len, datoms, frontier, schema). Merge is also a free function
since it is a set-algebraic operation spanning the MERGE namespace.

#### Formal Justification

1. **Keeps Store lean**: Store is the foundational abstraction (spec §1). Adding methods
   for every namespace would make Store a God-object that grows with every new feature.
   With 14 namespaces, Store would accumulate dozens of methods unrelated to its core
   responsibility (managing the datom set).

2. **Prevents God-object anti-pattern**: A Store with methods for harvest, seed, guidance,
   merge, deliberation, bilateral, sync, budget, and interface conflates data substrate
   with domain logic. Each namespace has its own invariants, types, and failure modes —
   these should live in their own modules, not on Store.

3. **Enables independent testing**: Free functions `fn harvest_pipeline(store: &Store, ...)`
   can be tested with a mock or minimal Store. Store methods would require constructing
   a full Store for every namespace test, coupling test infrastructure to Store internals.

4. **Matches Rust idioms**: Rust favors free functions for operations that don't need
   privileged access to private fields. Namespace operations only need Store's public API
   (`datoms()`, `current()`, `as_of()`, `schema()`, `transact()`). Free functions make
   this dependency explicit in the signature.

5. **Consistent with guide convention**: All guide files already use free functions for
   namespace APIs (guide/05-harvest.md, guide/06-seed.md, guide/07-merge-basic.md,
   guide/08-guidance.md). This ADR formalizes the existing pattern.

#### Consequences

- Store's `impl` block contains only: `genesis()`, `transact()`, `current()`,
  `as_of()`, `len()`, `datoms()`, `frontier()`, `schema()`.
  Note: `merge()` is also a free function (`pub fn merge(target: &mut Store, source: &Store)`)
  per the R5.2a audit, since merge is a set-algebraic operation spanning the MERGE namespace.
- Namespace operations are free functions in their respective modules: `query/mod.rs`,
  `harvest.rs`, `seed.rs`, `guidance.rs`, `merge.rs`, `methodology.rs`, `derivation.rs`,
  `routing.rs`.
- The binary crate (`crates/braid/src/commands/`) calls free functions, passing the loaded Store.
- Adding a new namespace never requires modifying `Store`'s API surface.

#### Falsification

Evidence this decision is wrong would be: a namespace operation that requires access to
Store's private fields and cannot be expressed through Store's public API. In that case,
the Store API needs extension (a new public method), not a namespace method on Store.

---
