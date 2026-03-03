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
├── braid-kernel/               ← Pure computation library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs              ← Public re-exports
│       ├── datom.rs            ← Datom, EntityId, TxId, Op, Value
│       ├── store.rs            ← Store, transact, merge
│       ├── schema.rs           ← Schema, genesis, attribute registry
│       ├── query/
│       │   ├── mod.rs          ← Query engine entry point
│       │   ├── parser.rs       ← Datalog parser (pest or nom)
│       │   ├── evaluator.rs    ← Semi-naive fixpoint evaluator
│       │   ├── clause.rs       ← Clause, Binding, Pattern
│       │   └── strata.rs       ← Stratum classification (CALM)
│       ├── resolution.rs       ← ResolutionMode, ConflictSet, resolve
│       ├── harvest.rs          ← HarvestCandidate, HarvestPipeline, gap detection
│       ├── seed.rs             ← SeedAssembly, associate/assemble/compress
│       ├── guidance.rs         ← GuidanceFooter, drift detection, anti-drift
│       ├── merge.rs            ← Pure set-union merge (Stage 0 subset)
│       └── frontier.rs         ← Frontier, AgentId, HLC clock
├── braid/                      ← Binary crate (CLI + MCP + persistence)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             ← clap CLI entry point
│       ├── commands/
│       │   ├── mod.rs
│       │   ├── transact.rs     ← braid transact
│       │   ├── query.rs        ← braid query
│       │   ├── status.rs       ← braid status
│       │   ├── harvest.rs      ← braid harvest
│       │   ├── seed.rs         ← braid seed
│       │   ├── guidance.rs     ← braid guidance
│       │   └── entity.rs       ← braid entity, braid history
│       ├── persistence.rs      ← redb store ↔ kernel Store bridge
│       ├── output.rs           ← OutputMode dispatch (json/agent/human)
│       ├── mcp.rs              ← MCP JSON-RPC server (9 tools)
│       └── claude_md.rs        ← Dynamic CLAUDE.md generation
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
  IO boundary: reads files, writes to redb, serves MCP, prints output.
  The binary crate contains no invariant-bearing logic.

### Cargo.toml — Workspace Root

```toml
[workspace]
resolver = "2"
members = ["braid-kernel", "braid"]

[workspace.dependencies]
blake3 = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
proptest = "1"
```

### Cargo.toml — braid-kernel

```toml
[package]
name = "braid-kernel"
version = "0.1.0"
edition = "2024"

[dependencies]
blake3 = { workspace = true }
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
edition = "2024"

[dependencies]
braid-kernel = { path = "../braid-kernel" }
clap = { version = "4", features = ["derive"] }
redb = "2"
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

pub struct Transaction<S: TxState> {
    datoms:              Vec<Datom>,
    tx_entity:           EntityId,
    provenance:          ProvenanceType,
    causal_predecessors: Vec<TxId>,
    agent:               AgentId,
    rationale:           String,
    _state:              PhantomData<S>,
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
    Lattice(LatticeDef),    // Join over unretracted values
    LastWriterWins,         // Greatest HLC assertion
    MultiValue,             // Set of all unretracted values
}
```

---

## §0.3 File Formats

### Datom JSONL Interchange

One datom per line. Used for `braid transact --file` and export:

```jsonl
{"e":"blake3:a1b2c3...","a":":db/ident","v":{"String":":spec/type"},"tx":"hlc:1709000000000-0-agent1","op":"assert"}
{"e":"blake3:d4e5f6...","a":":spec/statement","v":{"String":"The store never deletes"},"tx":"hlc:1709000000000-0-agent1","op":"assert"}
```

**Key conventions**:
- `e`: prefixed with `blake3:` + hex-encoded 32 bytes
- `a`: keyword string (`:namespace/name`)
- `v`: tagged union matching `Value` enum — `{"String":"..."}`, `{"Long":42}`, `{"Ref":"blake3:..."}`, etc.
- `tx`: prefixed with `hlc:` + `{wall_time}-{logical}-{agent_hex}`
- `op`: `"assert"` or `"retract"`

### redb Table Schema

```
Table "datoms"     → (datom_hash: [u8; 32]) → (datom_bytes: Vec<u8>)
Table "eavt"       → (entity ++ attr ++ value ++ tx) → datom_hash
Table "aevt"       → (attr ++ entity ++ value ++ tx) → datom_hash
Table "vaet"       → (value ++ attr ++ entity ++ tx) → datom_hash
Table "avet"       → (attr ++ value ++ entity ++ tx) → datom_hash
Table "tx_log"     → (tx_id_bytes) → (tx_metadata_bytes)
Table "frontier"   → (agent_id_bytes) → (tx_id_bytes)
Table "schema"     → (attr_keyword) → (schema_entry_bytes)
```

### Seed Output Template (from spec/06-seed.md)

Five-part structure, each designed as a prompt component:

```markdown
## Orientation
{project_identity, current_phase, active_spec_section}

## Prior Decisions
{relevant_ADRs, commitment_weights, do_not_relitigate_list}

## Working Context
{recent_transactions, frontier_state, drift_score, active_uncertainties}

## Warnings
{unresolved_conflicts, stale_datoms, approaching_thresholds}

## Task
{assigned_task, traceability_to_SEED, relevant_INVs, recommended_first_action}
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

## §0.4 CLI Command Signatures

All commands follow the pattern: parse args → load store from redb → call kernel function →
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

    /// Store path (redb file)
    #[arg(long, default_value = ".braid/store.redb")]
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
Genesis + spec bootstrap complete. Schema: 17 axiomatic + 14 spec attributes.
---
↳ What divergence type does this address? (C7: self-bootstrap)
  Next: `braid query '[:find ?id :where [?e :spec/type "invariant"] [?e :spec/id ?id]]'`
```

---

## §0.5 MCP Tool Definitions

Nine tools (INV-INTERFACE-003). Each description is an optimized prompt: navigative purpose,
semantic types, one micro-example.

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
      "description": "Run a Datalog query against the store. Use when you need to find facts — which invariants exist, what depends on what, what changed since a frontier. Returns binding sets.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "query": { "type": "string", "description": "Datalog query in Braid syntax" },
          "mode": { "enum": ["monotonic", "stratified"], "default": "monotonic" }
        },
        "required": ["query"]
      }
    },
    {
      "name": "braid_status",
      "description": "Store summary: datom count, frontier, schema statistics, drift score. Use for orientation at session start.",
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
      "description": "Get methodology guidance based on current drift signals. Use when uncertain about next step. Returns spec-language guidance with INV references.",
      "inputSchema": { "type": "object", "properties": {} }
    },
    {
      "name": "braid_entity",
      "description": "All datoms for an entity. Use when examining a specific decision, invariant, or specification element.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "id": { "type": "string", "description": "EntityId (blake3 hash) or :db/ident keyword" }
        },
        "required": ["id"]
      }
    },
    {
      "name": "braid_history",
      "description": "Attribute value over time for an entity. Use when understanding how a decision evolved.",
      "inputSchema": {
        "type": "object",
        "properties": {
          "entity": { "type": "string" },
          "attribute": { "type": "string" }
        },
        "required": ["entity", "attribute"]
      }
    },
    {
      "name": "braid_claude_md",
      "description": "Generate dynamic CLAUDE.md from store state. Use to update session instructions after significant state changes.",
      "inputSchema": { "type": "object", "properties": {} }
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
    agent_summary: AgentOutput, // Compressed prompt (agent mode)
    human_display: String,      // Formatted for terminal (human mode)
}

AgentOutput = {
    context: String,            // ≤50 tokens — cognitive mode activation
    content: String,            // ≤200 tokens — the payload
    footer:  GuidanceFooter,    // ≤50 tokens — methodology steering
}
```

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
| UNC-SCHEMA-001 | 17 axiomatic attributes sufficient? | Make attribute list a configurable genesis template. Instrument: log any "attribute not found" errors during Stage 0. Resolve: if 0 failures over 50 sessions, confirm sufficiency. |
| UNC-HARVEST-001 | Q(t) < 0.15 and < 0.05 thresholds | Make thresholds configurable datoms (`:braid/harvest-warn-threshold`, `:braid/harvest-only-threshold`). Instrument: log Q(t) at harvest time. Resolve: compute optimal threshold from harvest outcome data. |
| UNC-GUIDANCE-001 | Basin B crossover at 15–20 turns | Make crossover point a configurable datom (`:braid/drift-crossover`). Instrument: log turn count when agent first skips DDIS step. Resolve: compute empirical crossover from instrumentation data. |

**Pattern**: Make the uncertain value a configurable datom → instrument from day one →
resolve via empirical data from Stage 0 usage. The datom store stores its own uncertainty
resolution data — self-bootstrap at the epistemological level.

---
