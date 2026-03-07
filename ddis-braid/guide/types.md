# Canonical Type Catalog

> **Purpose**: Single source of truth for all Rust types across `spec/` and `guide/`.
> Every type definition in the Braid system is listed here with its defining file(s),
> full signature, spec/guide agreement status, namespace, and stage.
>
> **Downstream consumers**: R1.2 (resolve divergences), R1.3 (audit completeness),
> R1.7 (reconcile field-level mismatches).
>
> **Convention**: `[AGREE]` = spec and guide definitions match. `[DIVERGENCE]` = mismatch
> between spec and guide that requires reconciliation. `[STAGED]` = intentional difference
> where guide implements a Stage 0 subset of the spec's full definition.
> `[SPEC-ONLY]` = defined in spec but absent from guide.
> `[GUIDE-ONLY]` = defined in guide but absent from spec.

---

## Table of Contents

- [STORE (spec/01, guide/01, guide/00)](#store)
- [LAYOUT (spec/01b, guide/01b)](#layout)
- [SCHEMA (spec/02, guide/02)](#schema)
- [QUERY (spec/03, guide/03, guide/00)](#query)
- [RESOLUTION (spec/04, guide/04)](#resolution)
- [HARVEST (spec/05, guide/05, guide/00)](#harvest)
- [SEED (spec/06, guide/06, guide/00)](#seed)
- [MERGE (spec/07, guide/07)](#merge)
- [SYNC (spec/08)](#sync)
- [SIGNAL (spec/09)](#signal)
- [BILATERAL (spec/10)](#bilateral)
- [DELIBERATION (spec/11)](#deliberation)
- [GUIDANCE (spec/12, guide/08, guide/00)](#guidance)
- [BUDGET (spec/13)](#budget)
- [INTERFACE (spec/14, guide/09, guide/00)](#interface)
- [TRILATERAL (spec/18, guide/13)](#trilateral)
- [Appendix A: Divergence Summary](#appendix-a-divergence-summary)

---

## STORE

### Datom

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L0 definition, L2 contract) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/01-store.md` (section 1.1, 1.2) |
| **Status** | `[AGREE]` |

```rust
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Datom {
    pub entity:    EntityId,
    pub attribute: Attribute,
    pub value:     Value,
    pub tx:        TxId,
    pub op:        Op,
}
```

**Notes**: Spec defines the five-tuple algebraically as `(e, a, v, tx, op)`. Guide provides the
full Rust derive attributes. Content-addressed: identity = hash(e, a, v, tx, op). Immutable after
construction.

---

### EntityId

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L0, L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/01-store.md` (section 1.1, 1.2) |
| **Status** | `[AGREE]` |

```rust
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct EntityId([u8; 32]);

impl EntityId {
    pub fn from_content(content: &[u8]) -> Self;
    pub fn from_ident(keyword: &str) -> Self;
    pub fn as_bytes(&self) -> &[u8; 32];
}
// No: pub fn new(raw: [u8; 32]) — bypasses content addressing (NEG-STORE-002)
```

**Notes**: BLAKE3 hash of semantic content. No public raw-byte constructor. Both spec and guide
agree on the content-addressing constraint.

---

### Attribute

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L0) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/01-store.md` (section 1.1) |
| **Status** | `[AGREE]` |

```rust
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Attribute(String);

impl Attribute {
    pub fn new(keyword: &str) -> Result<Self, AttributeError>;
    pub fn namespace(&self) -> &str;
    pub fn name(&self) -> &str;
}
```

**Notes**: Keyword-style namespaced string (`:db/ident`, `:spec/type`). Must start with `:`,
contain exactly one `/`. Guide adds `from_keyword` const constructor for genesis constants.

---

### Value

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L0 value domain: 14 types) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/01-store.md` (section 1.1) |
| **Status** | `[STAGED]` -- Intentional: spec defines 14 value types, guide implements 9 at Stage 0 |

```rust
// Guide definition (Stage 0):
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Value {
    String(String),
    Keyword(String),
    Boolean(bool),
    Long(i64),
    Double(OrderedFloat<f64>),
    Instant(u64),        // millis since epoch
    Uuid([u8; 16]),
    Ref(EntityId),
    Bytes(Vec<u8>),
    // Deferred to later stages: BigInt, BigDec, Tuple, Json, URI
}
```

**Divergence detail**: Spec section 1.1 defines 14 value types in the value domain:
String, Keyword, Boolean, Long, Double, Instant, Uuid, Ref, Bytes, BigInt, BigDec,
Tuple, Json, URI. Guide explicitly defers 5 (BigInt, BigDec, Tuple, Json, URI) to
later stages. This is an intentional Stage 0 scope reduction, documented in both
spec and guide. Not a reconciliation defect -- tracked as staged feature.

---

### TxId

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L0, L2: HLC specification) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/01-store.md` (section 1.1) |
| **Status** | `[AGREE]` -- Spec silent on derive attributes; guide intentionally omits Hash on TxId (only AgentId needs Hash) |

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TxId {
    pub wall_time: u64,   // millis since epoch
    pub logical:   u32,   // counter for same-millisecond ordering
    pub agent:     AgentId,
}
```

**Divergence detail**: Guide specifies `TxId` without `Hash` derive (it does derive
`Ord`/`PartialOrd`). Since `TxId` is used as a key in `HashMap<AgentId, TxId>` (via
the frontier), the `Hash` derive is not needed on `TxId` itself -- only on `AgentId`.
Spec is silent on derive attributes. Minor -- guide is more precise.

---

### AgentId

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L0) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/01-store.md` (section 1.1) |
| **Status** | `[AGREE]` |

```rust
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct AgentId([u8; 16]);  // UUID or hash of agent name
```

---

### Op

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L0: `assert | retract`) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/01-store.md` (section 1.1) |
| **Status** | `[AGREE]` |

```rust
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Op { Assert, Retract }
```

---

### Store

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/01-store.md` (section 1.2) |
| **Status** | `[AGREE]` |

```rust
pub struct Store {
    datoms:   BTreeSet<Datom>,
    indexes:  Indexes,
    frontier: HashMap<AgentId, TxId>,
    schema:   Schema,
}
```

**Notes**: Spec defines Store as `(P(D), union)` algebraically, with methods `genesis`, `transact`,
`merge`, `current`, `as_of`, `len`, `datoms`, `frontier`. Guide mirrors this exactly with
additional `schema` field.

---

### Transaction (Typestate Pattern)

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L1, L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/01-store.md` (section 1.2) |
| **Status** | `[AGREE]` |

```rust
pub struct Building;
pub struct Committed;
pub struct Applied;

pub trait TxState: sealed::Sealed {}
impl TxState for Building {}
impl TxState for Committed {}
impl TxState for Applied {}

pub struct Transaction<S: TxState> {
    datoms:   Vec<Datom>,
    tx_data:  TxData,
    _state:   PhantomData<S>,
}

pub struct TxData {
    pub tx_entity:           EntityId,
    pub provenance:          ProvenanceType,
    pub causal_predecessors: Vec<TxId>,
    pub agent:               AgentId,
    pub rationale:           String,
}
```

**Notes**: Three-state typestate: Building -> Committed -> Applied. Spec uses opaque
`tx_data: TxData`; guide inlines the fields. Both are valid representations.

---

### ProvenanceType

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L1: provenance typing lattice) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/01-store.md` (section 1.1) |
| **Status** | `[AGREE]` |

```rust
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum ProvenanceType {
    Hypothesized,  // 0.2
    Inferred,      // 0.5
    Derived,       // 0.8
    Observed,      // 1.0
}
```

---

### TxReceipt

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2 cross-namespace types) |
| **Status** | `[AGREE]` |

```rust
pub struct TxReceipt {
    pub tx_id: TxId,
    pub datom_count: usize,
    pub new_entities: Vec<EntityId>,
}
```

**Notes**: Formalized in spec via R4.1b. Return type of `Transaction<Applied>::receipt()` and
`Store::transact()`. Motivating invariants: INV-STORE-001, INV-STORE-002.

---

### TxValidationError

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2 cross-namespace types) |
| **Status** | `[AGREE]` |

```rust
pub enum TxValidationError {
    UnknownAttribute(Attribute),
    SchemaViolation { attr: Attribute, expected: ValueType, got: ValueType },
    InvalidRetraction(EntityId, Attribute),
}
```

**Notes**: Formalized in spec via R4.1b. Error return of `Transaction<Building>::commit()`.
Gates transaction correctness per INV-SCHEMA-004. Spec uses `Attribute` type (not `Keyword`).

---

### TxApplyError

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L2) |
| **Guide files** | `guide/01-store.md` (section 1.1: transact signature) |
| **Status** | `[AGREE]` |

```rust
pub enum TxApplyError {
    DuplicateTransaction(TxId),
    StorageFailure(String),
}
```

**Notes**: Formalized in spec via R4.1b. Error return of `Transaction<Committed>::apply()` and
`Store::transact()`. Distinct from `TxValidationError` (schema-level vs. store-level errors).

---

### Frontier

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L2) |
| **Guide files** | `guide/01-store.md` (Store::frontier field) |
| **Status** | `[AGREE]` |

```rust
pub type Frontier = HashMap<AgentId, TxId>;
```

**Notes**: Formalized in spec via R4.1b. Used as parameter type in `Store::as_of()`,
`QueryMode::Stratified`, and `detect_conflicts()`. Equivalent to a vector clock. Frontier
is also a queryable datom attribute (:tx/frontier) per INV-QUERY-007.

---

### EntityView

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L2) |
| **Guide files** | `guide/01-store.md` (Store::current return type) |
| **Status** | `[AGREE]` |

```rust
pub struct EntityView {
    pub entity: EntityId,
    pub attributes: HashMap<Attribute, Value>,
    pub as_of: TxId,
}
```

**Notes**: Formalized in spec via R4.1b. Return type of `Store::current()`. Provides resolved
attribute values after per-attribute resolution (RESOLUTION namespace).

---

### SnapshotView

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Spec file** | `spec/01-store.md` (L2) |
| **Guide files** | `guide/01-store.md` (Store::as_of return type) |
| **Status** | `[AGREE]` |

```rust
pub struct SnapshotView<'a> {
    store: &'a Store,
    frontier: Frontier,
}

impl<'a> SnapshotView<'a> {
    pub fn current(&self, entity: EntityId) -> EntityView;
    pub fn len(&self) -> usize;
}
```

**Notes**: Formalized in spec via R4.1b. Return type of `Store::as_of()`. Read-only view of the
store restricted to datoms visible at the given frontier.

---

## LAYOUT

### LayoutError

| Field | Value |
|-------|-------|
| **Namespace** | LAYOUT |
| **Stage** | 0 |
| **Spec file** | `spec/01b-storage-layout.md` (INV-LAYOUT-005 L2) |
| **Guide files** | `guide/01b-storage-layout.md` (§1b.2) |
| **Status** | `[AGREE]` |

```rust
pub enum LayoutError {
    Io(std::io::Error),
    Deserialize(String),
    HashMismatch { expected: Blake3Hash, actual: Blake3Hash, path: PathBuf },
    CorruptedTransaction { path: PathBuf, reason: String },
    MissingGenesis,
}
```

**Notes**: Used by all persistence functions in `braid/src/persistence.rs` and serialization
functions in `braid-kernel/src/layout.rs`. `HashMismatch` supports INV-LAYOUT-005 (integrity
self-verification).

---

### Blake3Hash

| Field | Value |
|-------|-------|
| **Namespace** | LAYOUT |
| **Stage** | 0 |
| **Spec file** | `spec/01b-storage-layout.md` (INV-LAYOUT-001) |
| **Guide files** | `guide/01b-storage-layout.md` (§1b.2) |
| **Status** | `[AGREE]` |

```rust
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Blake3Hash([u8; 32]);
```

**Notes**: Content-addressed file identity. File name = hex(Blake3Hash). Used by
`transaction_hash()` and all content-addressed operations.

---

## SCHEMA

### Schema

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Spec file** | `spec/02-schema.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/02-schema.md` (section 2.1) |
| **Status** | `[AGREE]` |

```rust
pub struct Schema { /* opaque — fields extracted from schema datoms */ }

impl Schema {
    pub fn from_store(datoms: &BTreeSet<Datom>) -> Schema;
    pub fn attribute(&self, ident: &Keyword) -> Option<&AttributeDef>;
    pub fn validate_datom(&self, datom: &Datom) -> Result<(), SchemaValidationError>;
    pub fn new_attribute(&self, spec: AttributeSpec) -> Vec<Datom>;
    pub fn attributes(&self) -> impl Iterator<Item = (&Attribute, &AttributeDef)>;
    pub fn resolution_mode(&self, attr: &Attribute) -> ResolutionMode;
}
```

**Notes**: Owned by Store internally (ADR-SCHEMA-005). `from_store` is the sole constructor
(enforces C3). Guide adds `get()` as alias for `attribute()`.

**Concurrency**: Schema is part of Store's MVCC snapshot (ADR-STORE-016). No independent
versioning needed or permitted. Each `ArcSwap` load returns a Store whose Schema matches
its datoms exactly — consistency is structural, not coordinated. See ADR-SCHEMA-005
Stage 3 Concurrency Analysis for Option B rejection rationale.

---

### AttributeSpec

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Spec file** | `spec/02-schema.md` (L2: attribute algebra) |
| **Guide files** | `guide/02-schema.md` (section 2.1) |
| **Status** | `[AGREE]` |

```rust
pub struct AttributeSpec {
    pub ident:           Attribute,
    pub value_type:      ValueType,
    pub cardinality:     Cardinality,
    pub doc:             String,
    pub resolution_mode: ResolutionMode,
    pub unique:          Option<Uniqueness>,
    pub is_component:    bool,
}
```

---

### AttributeDef

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Spec file** | `spec/02-schema.md` (L2: schema query result) |
| **Guide files** | `guide/02-schema.md` (section 2.1) |
| **Status** | `[AGREE]` |

```rust
pub struct AttributeDef {
    pub entity:          EntityId,
    pub ident:           Attribute,
    pub value_type:      ValueType,
    pub cardinality:     Cardinality,
    pub resolution_mode: ResolutionMode,
    pub doc:             String,
    pub unique:          Option<Uniqueness>,
    pub is_component:    bool,
}
```

---

### ValueType

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Spec file** | `spec/02-schema.md` (L0: 14 value types) |
| **Guide files** | `guide/02-schema.md` (section 2.1) |
| **Status** | `[AGREE]` (Stage 0 subset) |

```rust
pub enum ValueType {
    String, Keyword, Boolean, Long, Double, Instant, Uuid, Ref, Bytes,
}
```

---

### Cardinality

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Spec file** | `spec/02-schema.md` (L0: `:one | :many`) |
| **Guide files** | `guide/02-schema.md` (section 2.1) |
| **Status** | `[AGREE]` |

```rust
pub enum Cardinality { One, Many }
```

---

### Uniqueness

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Spec file** | `spec/02-schema.md` (L0: `:identity | :value`) |
| **Guide files** | `guide/02-schema.md` (section 2.1) |
| **Status** | `[AGREE]` |

```rust
pub enum Uniqueness { Identity, Value }
```

---

### SchemaLayer

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Spec file** | `spec/02-schema.md` (L2, INV-SCHEMA-006) |
| **Guide files** | `guide/02-schema.md` (section 2.2) |
| **Status** | `[AGREE]` |

```rust
pub enum SchemaLayer {
    MetaSchema,       // Layer 0
    AgentProvenance,  // Layer 1
    DdisCore,         // Layer 2
    Discovery,        // Layer 3
    Coordination,     // Layer 4
    Workflow,         // Layer 5
}
```

**Notes**: Rust enum formalized in spec via R4.2b. Previously described in prose only
(INV-SCHEMA-006 L0). Each layer depends only on layers below it.

---

### SchemaError

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Spec file** | `spec/02-schema.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2 cross-namespace types) |
| **Status** | `[AGREE]` |

```rust
pub enum SchemaError {
    DuplicateAttribute(Attribute),
    InvalidCardinality,
    LayerDependencyViolation { attr: Attribute, attr_layer: SchemaLayer, ref_layer: SchemaLayer },
}
```

**Notes**: Formalized in spec via R4.1b/R4.2b. Spec variant `LayerDependencyViolation` replaces
guide's `CyclicDependency` -- more precise (traces to NEG-SCHEMA-003). Guide definition should
adopt spec variants.

---

### SchemaValidationError

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Spec file** | `spec/02-schema.md` (INV-SCHEMA-004 L2) |
| **Guide files** | `guide/02-schema.md` (implied by `validate_datom` return) |
| **Status** | `[AGREE]` |

```rust
pub enum SchemaValidationError {
    UnknownAttribute(Attribute),
    TypeMismatch { attr: Attribute, expected: ValueType, got: ValueType },
    CardinalityViolation { attr: Attribute, cardinality: Cardinality },
    InvalidLatticeValue { attr: Attribute, value: Value, lattice: String },
    InvalidRetraction { entity: EntityId, attr: Attribute },
}
```

**Notes**: Covers datom-level type checking (INV-SCHEMA-004). Distinct from `SchemaError`
(schema definition operations) and `TxValidationError` (transaction-level wrapper in
`guide/00-architecture.md`).

---

### LatticeValidationError

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Spec file** | `spec/02-schema.md` (INV-SCHEMA-007 L2) |
| **Guide files** | `guide/02-schema.md` (section 2.2) |
| **Status** | `[SPEC-ONLY]` -- Detailed in spec L2 code block |

```rust
// From spec/02-schema.md INV-SCHEMA-007 L2:
pub enum LatticeValidationError {
    NotReflexive(Keyword),
    NotAntisymmetric(Keyword, Keyword),
    NotTransitive(Keyword, Keyword, Keyword),
    NoJoin(Keyword, Keyword),
    NonUniqueJoin(Keyword, Keyword),
    InvalidBottom(Keyword, Keyword),
}
```

---

## QUERY

### QueryExpr

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (L2: `QueryExpr { Find(ParsedQuery), Pull }`) |
| **Guide files** | `guide/03-query.md` (section 3.1) |
| **Status** | `[AGREE]` |

```rust
pub enum QueryExpr {
    Find(ParsedQuery),
    Pull { pattern: PullPattern, entity: EntityRef },
}
```

**Notes**: Spec updated (R6.7b) to adopt guide's `Find(ParsedQuery)` form. `ParsedQuery`
subsumes the original `{variables, clauses}` with richer AST support for all four Datomic
find forms, rules, and inputs.

---

### ParsedQuery

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (L2: explicit struct in QueryExpr) |
| **Guide files** | `guide/03-query.md` (section 3.1), `guide/00-architecture.md` (section 0.2) |
| **Status** | `[AGREE]` |

```rust
pub struct ParsedQuery {
    pub find_spec:     FindSpec,
    pub where_clauses: Vec<Clause>,
    pub rules:         Vec<Rule>,
    pub inputs:        Vec<Input>,
}
```

---

### QueryResult

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (L2) |
| **Guide files** | `guide/03-query.md` (section 3.1), `guide/00-architecture.md` (section 0.2) |
| **Status** | `[AGREE]` |

```rust
pub type BindingSet = HashMap<Variable, Value>;

pub struct QueryResult {
    pub bindings: Vec<BindingSet>,
    pub stratum:  Stratum,
    pub mode:     QueryMode,
    pub provenance_tx: TxId,
}
```

**Notes**: Spec updated (R6.7b) to adopt guide's `BindingSet` (preserves variable names) and
`Stratum` enum (type-safe). Both spec and guide now agree.

---

### FindSpec

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (L2: FindSpec enum in ParsedQuery) |
| **Guide files** | `guide/03-query.md` (section 3.1), `guide/00-architecture.md` (section 0.2) |
| **Status** | `[AGREE]` |

```rust
pub enum FindSpec {
    Relation(Vec<Variable>),    // [:find ?x ?y]
    Scalar(Variable),           // [:find ?x .]
    Collection(Variable),       // [:find [?x ...]]
    Tuple(Vec<Variable>),       // [:find [?x ?y]]
}
```

---

### Clause

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (L2) |
| **Guide files** | `guide/03-query.md` (section 3.1) |
| **Status** | `[AGREE]` |

```rust
pub enum Clause {
    DataPattern(EntityPattern, AttributePattern, ValuePattern),
    RuleApplication(RuleName, Vec<Term>),
    NotClause(Box<Clause>),
    OrClause(Vec<Vec<Clause>>),
    Frontier(FrontierRef),
    // Stage 1+: Aggregate(Variable, AggregateFunc)
    // Stage 1+: Ffi(FfiCall)
}
```

**Notes**: Spec updated (R6.7b) to adopt guide's Datalog-standard naming (DataPattern,
RuleApplication, NotClause, OrClause) and stage-defer Aggregate/Ffi to Stage 1+.
Both spec and guide now agree on Stage 0 variant set.

---

### QueryMode

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (L0, L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/03-query.md` (section 3.1) |
| **Status** | `[AGREE]` |

```rust
pub enum QueryMode {
    Monotonic,
    Stratified(Frontier),
    Barriered(BarrierId),
}
```

**Notes**: Spec updated (R6.7b) to adopt guide's tuple variant form. Semantically identical
to the original named-field form; syntactically simpler and consistent.

---

### Stratum

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (L2) |
| **Guide files** | `guide/03-query.md` (section 3.1) |
| **Status** | `[STAGED]` -- Intentional: guide defines Stage 0 only; spec defines all 6 |

```rust
// Spec definition (all 6 strata):
pub enum Stratum {
    S0_Primitive,           // Stage 0: Monotonic. Current-value over LIVE index.
    S1_MonotonicJoin,       // Stage 0: Monotonic. Multi-hop joins, transitive closure.
    S2_Uncertainty,         // Stage 1+: Mixed (Stratified). Epistemic/aleatory/consequential.
    S3_Authority,           // Stage 1+: Stratified (FFI). SVD, spectral authority.
    S4_ConflictDetection,   // Stage 1+: Conservatively monotonic.
    S5_BilateralLoop,       // Stage 1+: Barriered. Fitness, drift, crystallization.
}

// Guide definition (Stage 0 only):
pub enum Stratum {
    S0_Primitive,
    S1_MonotonicJoin,
    // S2-S5 deferred to Stage 1+
}
```

**Notes**: Intentional stage scoping, not a conflict. Spec defines the full enum for reference;
guide implements only the Stage 0 subset. Both documents agree on Stage 0 behavior.

---

### BindingSet / FrontierRef / QueryStats (cross-namespace)

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (L2: BindingSet in QueryResult) |
| **Guide files** | `guide/00-architecture.md` (section 0.2) |
| **Status** | `[AGREE]` (BindingSet), `[GUIDE-ONLY]` (FrontierRef, QueryStats) |

```rust
pub type BindingSet = HashMap<Variable, Value>;
pub struct FrontierRef(pub AgentId);
pub struct QueryStats { pub datoms_scanned: usize, pub bindings_produced: usize }
```

---

### DirectedGraph

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (INV-QUERY-012-021, implicit) |
| **Guide files** | `guide/03-query.md` (section 3.3) |
| **Status** | `[GUIDE-ONLY]` -- Internal data structure for graph algorithms |

```rust
pub struct DirectedGraph {
    pub vertices: BTreeSet<EntityId>,
    pub adj: HashMap<EntityId, Vec<EntityId>>,
    pub in_degree: HashMap<EntityId, usize>,
}
```

---

### SCCResult

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (INV-QUERY-013 L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/03-query.md` (section 3.1) |
| **Status** | `[AGREE]` |

```rust
pub struct SCCResult {
    pub components: Vec<Vec<EntityId>>,
    pub condensation: Vec<Vec<usize>>,
    pub has_cycles: bool,
}
```

---

### PageRankConfig

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (INV-QUERY-014 L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/03-query.md` (section 3.1) |
| **Status** | `[AGREE]` |

```rust
pub struct PageRankConfig {
    pub damping: f64,         // default: 0.85
    pub epsilon: f64,         // convergence: 1e-6
    pub max_iterations: u32,  // safety bound: 100
}
```

---

### CriticalPathResult

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (INV-QUERY-017 L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/03-query.md` (section 3.1) |
| **Status** | `[AGREE]` |

```rust
pub struct CriticalPathResult {
    pub path: Vec<EntityId>,
    pub total_weight: f64,
    pub slack: HashMap<EntityId, f64>,
    pub earliest_start: HashMap<EntityId, f64>,
    pub latest_start: HashMap<EntityId, f64>,
}
```

---

### GraphDensityMetrics

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (INV-QUERY-021 L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/03-query.md` (section 3.1) |
| **Status** | `[AGREE]` |

```rust
pub struct GraphDensityMetrics {
    pub vertex_count: usize,
    pub edge_count: usize,
    pub density: f64,
    pub avg_degree: f64,
    pub avg_clustering: f64,
    pub components: usize,
}
```

---

### QueryError

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (L2) |
| **Guide files** | `guide/03-query.md` (section 3.1) |
| **Status** | `[AGREE]` |

```rust
pub enum QueryError {
    ParseError(String),
    NonMonotonicInMonotonicMode,
    UnsafeProgram { variable: Variable },
    BarrierNotResolved(BarrierId),
    FfiError { function: String, message: String },
    Graph(GraphError),
}
```

**Notes**: Formalized in spec via R4.2b. Return type of `query()`. Encompasses parse errors
(INV-QUERY-006), monotonicity violations (INV-QUERY-001), safety violations, and graph errors.

---

### GraphError

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Spec file** | `spec/03-query.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/03-query.md` (section 3.1) |
| **Status** | `[AGREE]` |

```rust
pub enum GraphError {
    CycleDetected(SCCResult),
    EmptyGraph,
    NonConvergence(u32),
}
```

**Notes**: Formalized in spec via R4.1b. Unified error type for graph algorithms
(INV-QUERY-012-021). Embedded in QueryError::Graph variant.

---

### HITSResult

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 1 |
| **Spec file** | `spec/03-query.md` (INV-QUERY-016 L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` -- Stage 1, no guide coverage yet |

```rust
pub struct HITSResult {
    pub authorities: Vec<(EntityId, f64)>,
    pub hubs: Vec<(EntityId, f64)>,
    pub iterations: u32,
}
```

---

### ArticulationResult

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 2 |
| **Spec file** | `spec/03-query.md` (INV-QUERY-020 L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` -- Stage 2 |

```rust
pub struct ArticulationResult {
    pub articulation_points: Vec<EntityId>,
    pub bridges: Vec<(EntityId, EntityId)>,
    pub biconnected_components: Vec<Vec<EntityId>>,
}
```

---

## RESOLUTION

### ResolutionMode

| Field | Value |
|-------|-------|
| **Namespace** | RESOLUTION |
| **Stage** | 0 |
| **Spec file** | `spec/04-resolution.md` (L0, L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/04-resolution.md` (section 4.1) |
| **Status** | `[ALIGNED]` -- Reconciled: spec adopted guide naming (R1.10) |

```rust
pub enum ResolutionMode {
    LastWriterWins,                    // HLC default; clock variant via :db/lwwClock schema attr
    Lattice { lattice_id: EntityId },  // Join-semilattice — definition stored as datoms (C3)
    MultiValue,                        // Set of all unretracted values
}
```

**Resolution (R1.10)**: Spec updated to match guide naming. `Lww { clock: LwwClock }` became
`LastWriterWins` (fieldless); clock selection moved to schema attribute `:db/lwwClock`
(already defined in `spec/02-schema.md`). `Multi` renamed to `MultiValue` for clarity.
`LwwClock` enum removed from `ResolutionMode` -- it remains as a schema-level concept.

---

### LwwClock

| Field | Value |
|-------|-------|
| **Namespace** | RESOLUTION / SCHEMA |
| **Stage** | 0 |
| **Spec file** | `spec/02-schema.md` (L2) |
| **Guide files** | `guide/04-resolution.md` (section 4.1) |
| **Status** | `[AGREE]` |

```rust
pub enum LwwClock {
    Hlc,        // Hybrid Logical Clock (default, most precise)
    Wall,       // Wall-clock ordering
    AgentRank,  // Deterministic agent hierarchy
}
```

**Notes**: Formalized as Rust enum in spec via R4.1b/R4.3b. Added to guide/04-resolution.md
via R4.3b. Schema-level type read from `:db/lwwClock` keyword attribute; not embedded in
`ResolutionMode`. Determines tie-breaking behavior for INV-RESOLUTION-005.

---

### ConflictSet

| Field | Value |
|-------|-------|
| **Namespace** | RESOLUTION |
| **Stage** | 0 |
| **Spec file** | `spec/04-resolution.md` (L2) |
| **Guide files** | `guide/04-resolution.md` (section 4.1) |
| **Status** | `[ALIGNED]` -- Reconciled: spec adopted guide definition (R1.10) |

```rust
pub struct ConflictSet {
    pub entity:      EntityId,
    pub attribute:   Attribute,
    pub assertions:  Vec<(Value, TxId)>,
    pub retractions: Vec<(Value, TxId)>,
}
```

**Resolution (R1.10)**: Spec updated from `Conflict` (with embedded severity/tier/status) to
`ConflictSet` (detection-stage type with assertions + retractions). Routing metadata (severity,
tier, status) are computed during the conflict routing pipeline (INV-RESOLUTION-007) and stored
as separate datoms in the store, not embedded in the detection struct.

---

### RoutingTier

| Field | Value |
|-------|-------|
| **Namespace** | RESOLUTION |
| **Stage** | 0 |
| **Spec file** | `spec/04-resolution.md` (L2) |
| **Guide files** | `guide/04-resolution.md` (section 4.2) |
| **Status** | `[ALIGNED]` -- Reconciled: spec adopted guide name (R1.10) |

```rust
pub enum RoutingTier { Automatic, AgentNotification, HumanRequired }
```

**Resolution (R1.10)**: Spec updated from `ConflictTier` to `RoutingTier`. Same variants,
more descriptive name (describes what the enum is: a routing decision).

---

### ConflictStatus

| Field | Value |
|-------|-------|
| **Namespace** | RESOLUTION |
| **Stage** | 0 |
| **Spec file** | `spec/04-resolution.md` (L2: lattice `:detected < :routing < :resolving < :resolved`) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` -- Guide does not define a conflict status lattice |

**Notes**: Spec defines a four-step lattice for conflict lifecycle. Guide treats
conflict lifecycle as transient (detect -> resolve in one pass).

---

### ResolvedValue

| Field | Value |
|-------|-------|
| **Namespace** | RESOLUTION |
| **Stage** | 0 |
| **Spec file** | `spec/04-resolution.md` (implied by resolution function) |
| **Guide files** | `guide/04-resolution.md` (section 4.1) |
| **Status** | `[GUIDE-ONLY]` |

```rust
pub enum ResolvedValue {
    Single(Value),
    Multi(Vec<Value>),
    Conflict(Vec<(Value, TxId)>),
}
```

---

### Resolution

| Field | Value |
|-------|-------|
| **Namespace** | RESOLUTION |
| **Stage** | 0 |
| **Spec file** | `spec/04-resolution.md` (L2) |
| **Guide files** | `guide/04-resolution.md` (section 4.1) |
| **Status** | `[AGREE]` |

```rust
pub struct Resolution {
    pub conflict: EntityId,
    pub resolved_value: Value,
    pub method: ResolutionMethod,
    pub rationale: String,
}
```

**Notes**: Added to guide via R4.3b. Resolution provenance entity required by
NEG-RESOLUTION-003: every resolution must produce a datom trail.

---

## HARVEST

### HarvestCandidate

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Spec file** | `spec/05-harvest.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/05-harvest.md` (section 5.1) |
| **Status** | `[AGREE]` |

```rust
pub struct HarvestCandidate {
    pub id:                  usize,                  // Index for accept/reject referencing in CLI
    pub datom_spec:          Vec<Datom>,
    pub category:            HarvestCategory,
    pub confidence:          f64,                    // 0.0-1.0
    pub weight:              f64,                    // estimated commitment weight
    pub status:              CandidateStatus,        // lattice: proposed < under-review < committed | rejected
    pub extraction_context:  String,                 // why this was extracted
    pub reconciliation_type: ReconciliationType,     // traces to reconciliation taxonomy (spec section 15)
}
```

**Notes**: Spec updated (R6.7b) to include `id` and `reconciliation_type` fields from guide.
Both are needed: `id` for CLI accept/reject workflow, `reconciliation_type` for traceability
to the reconciliation taxonomy.

---

### HarvestCategory

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Spec file** | `spec/05-harvest.md` (L2) |
| **Guide files** | `guide/05-harvest.md` (section 5.1) |
| **Status** | `[AGREE]` |

```rust
pub enum HarvestCategory {
    Observation,
    Decision,
    Dependency,
    Uncertainty,
}
```

---

### CandidateStatus

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Spec file** | `spec/05-harvest.md` (L2) |
| **Guide files** | `guide/05-harvest.md` (section 5.1) |
| **Status** | `[AGREE]` |

```rust
pub enum CandidateStatus {
    Proposed,
    UnderReview,
    Committed,
    Rejected(String),
}
```

**Notes**: Formalized as Rust enum in spec via R4.1b. Spec now includes `Rejected(String)`
matching guide. Lattice ordering: proposed < under-review < committed | rejected.

---

### ReconciliationType

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Spec file** | -- |
| **Guide files** | `guide/05-harvest.md` (section 5.1) |
| **Status** | `[GUIDE-ONLY]` |

```rust
pub enum ReconciliationType {
    Epistemic,
    Structural,
    Consequential,
}
```

---

### HarvestSession

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Spec file** | `spec/05-harvest.md` (L2) |
| **Guide files** | `guide/05-harvest.md` (section 5.1) |
| **Status** | `[AGREE]` |

```rust
pub struct HarvestSession {
    pub session_id: EntityId,
    pub agent: AgentId,
    pub review_topology: ReviewTopology,
    pub candidates: Vec<HarvestCandidate>,
    pub drift_score: u32,
    pub timestamp: Instant,
}
```

**Notes**: Added to guide via R4.3b. Records harvest metadata per INV-HARVEST-002.
Distinct from `HarvestResult` (guide pipeline return); `HarvestSession` is the entity
that gets transacted into the store as a provenance record.

---

### ReviewTopology

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 (SelfReview), 2+ (other variants) |
| **Spec file** | `spec/05-harvest.md` (L2) |
| **Guide files** | `guide/05-harvest.md` (section 5.1) |
| **Status** | `[AGREE]` |

```rust
pub enum ReviewTopology {
    SelfReview,
    PeerReview { reviewer: AgentId },
    SwarmVote { quorum: u32 },
    HierarchicalDelegation { specialist: AgentId },
    HumanReview,
}
```

**Notes**: Added to guide via R4.3b. Stage 0 uses only `SelfReview`. Other variants
defined for forward compatibility per INV-HARVEST-008 (Stage 2).

---

### HarvestResult

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Spec file** | -- |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/05-harvest.md` (section 5.1) |
| **Status** | `[GUIDE-ONLY]` |

```rust
pub struct HarvestResult {
    pub candidates: Vec<HarvestCandidate>,
    pub drift_score: f64,
    pub quality: HarvestQuality,
}
```

---

### HarvestQuality

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Spec file** | -- |
| **Guide files** | `guide/05-harvest.md` (section 5.1) |
| **Status** | `[GUIDE-ONLY]` |

```rust
pub struct HarvestQuality {
    pub candidate_count:   usize,
    pub high_confidence:   usize,
    pub medium_confidence: usize,
    pub low_confidence:    usize,
}
```

---

### SessionContext

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Spec file** | -- |
| **Guide files** | `guide/05-harvest.md` (section 5.1) |
| **Status** | `[GUIDE-ONLY]` |

```rust
pub struct SessionContext {
    pub agent:               AgentId,
    pub session_start_tx:    TxId,
    pub recent_transactions: Vec<TxId>,
    pub task_description:    String,
}
```

---

## SEED

### SeedOutput

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Spec file** | `spec/06-seed.md` (ADR-SEED-004: five-part template) |
| **Guide files** | `guide/06-seed.md` (section 6.1) |
| **Status** | `[GUIDE-ONLY]` -- Spec describes template; guide defines struct |

```rust
pub struct SeedOutput {
    pub orientation:  String,
    pub constraints:  String,
    pub state:        String,
    pub warnings:     String,
    pub directive:    String,
}
```

---

### SchemaNeighborhood

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Spec file** | `spec/06-seed.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/06-seed.md` (section 6.1) |
| **Status** | `[AGREE]` |

```rust
pub struct SchemaNeighborhood {
    pub entities: Vec<EntityId>,
    pub attributes: Vec<Attribute>,
    pub entity_types: Vec<Keyword>,
}
```

---

### AssembledContext

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Spec file** | `spec/06-seed.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2) |
| **Status** | `[AGREE]` |

```rust
pub struct AssembledContext {
    pub sections: Vec<ContextSection>,
    pub total_tokens: usize,
    pub budget_remaining: usize,
    pub projection_pattern: ProjectionPattern,
}
```

**Notes**: Guide updated (R6.7b) to include `projection_pattern` field matching spec.
This field records which projection pyramid level was used during assembly.

---

### ContextSection

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Spec file** | `spec/06-seed.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct ContextSection {
    pub entity: EntityId,
    pub projection_level: ProjectionLevel,
    pub content: String,
    pub score: f64,
}
```

---

### ProjectionLevel

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Spec file** | `spec/06-seed.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum ProjectionLevel { Full, Summary, TypeLevel, Pointer }
```

---

### ClaudeMdGenerator

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Spec file** | `spec/06-seed.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` -- Guide uses free function `generate_claude_md()` instead |

```rust
pub struct ClaudeMdGenerator { pub store: Store }
```

**Notes**: Spec uses a struct wrapping the store. Guide uses a free function
`fn generate_claude_md(store: &Store, task: &str, budget: usize) -> String`
per ADR-ARCHITECTURE-001. Functionally equivalent.

---

### AssociateCue

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Spec file** | `spec/06-seed.md` (§6.3 L2) |
| **Guide files** | `guide/06-seed.md` (§6.1) |
| **Status** | `[AGREE]` |

```rust
pub enum AssociateCue {
    Semantic { text: String, depth: usize, breadth: usize },
    Explicit { seeds: Vec<EntityId>, depth: usize, breadth: usize },
}
```

**Notes**: `depth` and `breadth` enforce INV-SEED-003 (ASSOCIATE Boundedness). Defaults:
depth=3, breadth=10. `Semantic` mode uses text similarity for seed selection. `Explicit`
mode starts from specific entity IDs.

---

## MERGE

### Branch

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 2 |
| **Spec file** | `spec/07-merge.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` -- Stage 2, deferred from guide |

```rust
pub struct Branch {
    pub id: EntityId,
    pub ident: String,
    pub base_tx: TxId,
    pub agent: AgentId,
    pub status: BranchStatus,
    pub purpose: String,
    pub competing_with: Vec<EntityId>,
    pub datoms: BTreeSet<Datom>,
}
```

---

### BranchStatus

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 2 |
| **Spec file** | `spec/07-merge.md` (L2: lattice `:active < :proposed < :committed < :abandoned`) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

---

### MergeReceipt

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 0 |
| **Spec file** | `spec/07-merge.md` (section 7.3 L2, INV-MERGE-009 L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/07-merge-basic.md` (section 7.2) |
| **Status** | `[ALIGNED]` -- Reconciled: spec internal inconsistency resolved (R1.6) |

```rust
pub struct MergeReceipt {
    pub new_datoms:       usize,
    pub duplicate_datoms: usize,
    pub frontier_delta:   HashMap<AgentId, (Option<TxId>, TxId)>,
}
```

**Resolution (R1.6)**: Spec section 7.3 updated to match INV-MERGE-009 and guide. The old
definition 1 fields (`datoms_added`, `conflicts_detected`, `subscriptions_fired`,
`stale_projections`) were moved to `CascadeReceipt`. Spec `merge()` now returns
`(MergeReceipt, CascadeReceipt)`. MergeReceipt covers the pure set-union operation;
CascadeReceipt covers the 5-step cascade side-effects (INV-MERGE-002).

---

### CascadeReceipt

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 0 |
| **Spec file** | `spec/07-merge.md` (section 7.3 L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/07-merge-basic.md` (section 7.3) |
| **Status** | `[ALIGNED]` -- Reconciled: promoted from guide-only to spec+guide (R1.6) |

```rust
pub struct CascadeReceipt {
    pub conflicts_detected: usize,
    pub caches_invalidated: usize,
    pub projections_staled: usize,
    pub uncertainties_updated: usize,
    pub notifications_sent: usize,
    pub cascade_datoms: Vec<Datom>,
}
```

**Resolution (R1.6)**: Added to spec alongside MergeReceipt. Covers the cascade fields
from the old spec MergeReceipt definition 1. The guide's separation of MergeReceipt
(statistics) from CascadeReceipt (side-effects) is now the canonical decomposition.

---

### CombineStrategy

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 2 |
| **Spec file** | `spec/07-merge.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum CombineStrategy {
    Union,
    SelectiveUnion { selected: Vec<Datom> },
    ConflictToDeliberation,
}
```

---

### ComparisonCriterion

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 2 |
| **Spec file** | `spec/07-merge.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum ComparisonCriterion {
    FitnessScore,
    TestSuite,
    UncertaintyReduction,
    AgentReview,
    Custom(String),
}
```

---

### BranchComparison

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 2 |
| **Spec file** | `spec/07-merge.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct BranchComparison {
    pub branches: Vec<EntityId>,
    pub criterion: ComparisonCriterion,
    pub scores: HashMap<EntityId, f64>,
    pub winner: Option<EntityId>,
    pub rationale: String,
}
```

---

## SYNC

### Barrier

| Field | Value |
|-------|-------|
| **Namespace** | SYNC |
| **Stage** | 3 |
| **Spec file** | `spec/08-sync.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct Barrier {
    pub id: EntityId,
    pub participants: Vec<AgentId>,
    pub status: BarrierStatus,
    pub timeout: Duration,
    pub cut: Option<Frontier>,
    pub responses: HashMap<AgentId, Frontier>,
}
```

---

### BarrierResult

| Field | Value |
|-------|-------|
| **Namespace** | SYNC |
| **Stage** | 3 |
| **Spec file** | `spec/08-sync.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum BarrierResult {
    Resolved { cut: Frontier },
    TimedOut { responded: Vec<AgentId>, missing: Vec<AgentId> },
}
```

---

### BarrierStatus

| Field | Value |
|-------|-------|
| **Namespace** | SYNC |
| **Stage** | 3 |
| **Spec file** | `spec/08-sync.md` (L2: lattice `:initiated < :exchanging < :resolved | :timed-out`) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

---

## SIGNAL

### SignalType

| Field | Value |
|-------|-------|
| **Namespace** | SIGNAL |
| **Stage** | 3 |
| **Spec file** | `spec/09-signal.md` (L0, L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum SignalType {
    Confusion(ConfusionKind),
    Conflict { datom_a: DatomRef, datom_b: DatomRef },
    UncertaintySpike { entity: EntityId, delta: f64 },
    ResolutionProposal { deliberation: EntityId, position: EntityId },
    DelegationRequest { entity: EntityId, from: AgentId, to: AgentId },
    GoalDrift { intention: EntityId, observed_delta: f64 },
    BranchReady { branch: EntityId, comparison_criteria: Vec<Criterion> },
    DeliberationTurn { deliberation: EntityId, position: EntityId },
}
```

---

### ConfusionKind

| Field | Value |
|-------|-------|
| **Namespace** | SIGNAL |
| **Stage** | 3 |
| **Spec file** | `spec/09-signal.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum ConfusionKind { NeedMore, Contradictory, GoalUnclear, SchemaUnknown }
```

---

### Signal

| Field | Value |
|-------|-------|
| **Namespace** | SIGNAL |
| **Stage** | 3 |
| **Spec file** | `spec/09-signal.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct Signal {
    pub signal_type: SignalType,
    pub source: EntityId,
    pub target: EntityId,
    pub severity: Severity,
    pub timestamp: TxId,
}
```

---

### Severity

| Field | Value |
|-------|-------|
| **Namespace** | SIGNAL |
| **Stage** | 3 |
| **Spec file** | `spec/09-signal.md` (L0: total order Low < Medium < High < Critical) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum Severity { Low, Medium, High, Critical }
```

---

### Subscription

| Field | Value |
|-------|-------|
| **Namespace** | SIGNAL |
| **Stage** | 3 |
| **Spec file** | `spec/09-signal.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct Subscription {
    pub pattern: SignalPattern,
    pub callback: Box<dyn Fn(&Signal) -> Vec<Datom>>,
    pub debounce: Option<Duration>,
}
```

---

## BILATERAL

### BilateralLoop

| Field | Value |
|-------|-------|
| **Namespace** | BILATERAL |
| **Stage** | 2 |
| **Spec file** | `spec/10-bilateral.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct BilateralLoop {
    pub divergence_map: HashMap<Boundary, Vec<Gap>>,
    pub fitness: f64,
    pub cycle_count: u64,
    pub residuals: Vec<DocumentedResidual>,
}
```

---

### Boundary

| Field | Value |
|-------|-------|
| **Namespace** | BILATERAL |
| **Stage** | 2 |
| **Spec file** | `spec/10-bilateral.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum Boundary {
    IntentToSpec,
    SpecToSpec,
    SpecToImpl,
    ImplToBehavior,
}
```

---

### Gap

| Field | Value |
|-------|-------|
| **Namespace** | BILATERAL |
| **Stage** | 2 |
| **Spec file** | `spec/10-bilateral.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct Gap {
    pub boundary: Boundary,
    pub source: EntityId,
    pub target: Option<EntityId>,
    pub severity: Severity,
    pub description: String,
}
```

---

### DocumentedResidual

| Field | Value |
|-------|-------|
| **Namespace** | BILATERAL |
| **Stage** | 2 |
| **Spec file** | `spec/10-bilateral.md` (L2: referenced in BilateralLoop) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` -- Referenced but not structurally defined |

---

### CycleReport

| Field | Value |
|-------|-------|
| **Namespace** | BILATERAL |
| **Stage** | 2 |
| **Spec file** | `spec/10-bilateral.md` (L2: return type of `cycle()`) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` -- Referenced but not structurally defined |

---

## DELIBERATION

### Deliberation

| Field | Value |
|-------|-------|
| **Namespace** | DELIBERATION |
| **Stage** | 2 |
| **Spec file** | `spec/11-deliberation.md` (L0, L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct Deliberation {
    pub entity: EntityId,
    pub question: String,
    pub status: DeliberationStatus,
    pub positions: Vec<EntityId>,
    pub decision: Option<EntityId>,
}
```

---

### DeliberationStatus

| Field | Value |
|-------|-------|
| **Namespace** | DELIBERATION |
| **Stage** | 2 |
| **Spec file** | `spec/11-deliberation.md` (L0, L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeliberationStatus {
    Open,
    Active,
    Stalled,
    Decided,
    Contested,    // join(:stalled, :decided) — requires escalation
    Superseded,
}

impl DeliberationStatus {
    pub fn join(&self, other: &Self) -> Self;  // Custom — does NOT derive Ord
}

impl PartialOrd for DeliberationStatus { /* custom partial order */ }
// NOTE: Ord is NOT implemented — the order is partial (:stalled ∥ :decided)
```

**Notes**: Lifecycle partial lattice:
`:open < :active < :decided < :contested < :superseded` and
`:open < :active < :stalled < :contested < :superseded`.
`join(:stalled, :decided) = :contested` — requires DelegationRequest signal escalation.
`Ord` must NOT be derived — the order is partial.

---

### Position

| Field | Value |
|-------|-------|
| **Namespace** | DELIBERATION |
| **Stage** | 2 |
| **Spec file** | `spec/11-deliberation.md` (L0, L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct Position {
    pub entity: EntityId,
    pub deliberation: EntityId,
    pub stance: Stance,
    pub rationale: String,
    pub evidence: Vec<DatomRef>,
    pub agent: AgentId,
}
```

---

### Decision

| Field | Value |
|-------|-------|
| **Namespace** | DELIBERATION |
| **Stage** | 2 |
| **Spec file** | `spec/11-deliberation.md` (L0, L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct Decision {
    pub entity: EntityId,
    pub deliberation: EntityId,
    pub method: DecisionMethod,
    pub chosen_position: EntityId,
    pub rationale: String,
    pub commitment_weight: f64,
}
```

---

### Stance

| Field | Value |
|-------|-------|
| **Namespace** | DELIBERATION |
| **Stage** | 2 |
| **Spec file** | `spec/11-deliberation.md` (L0) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum Stance { Advocate, Oppose, Neutral, Synthesize }
```

---

### DecisionMethod

| Field | Value |
|-------|-------|
| **Namespace** | DELIBERATION |
| **Stage** | 2 |
| **Spec file** | `spec/11-deliberation.md` (L0) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum DecisionMethod { Consensus, Majority, Authority, HumanOverride, Automated }
```

---

## GUIDANCE

### GuidanceTopology

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/08-guidance.md` (section 8.1) |
| **Status** | `[AGREE]` |

```rust
pub struct GuidanceTopology {
    pub nodes: HashMap<EntityId, GuidanceNode>,
    pub edges: Vec<(EntityId, EntityId)>,
}
```

---

### GuidanceNode

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/08-guidance.md` (section 8.1) |
| **Status** | `[AGREE]` |

```rust
pub struct GuidanceNode {
    pub entity: EntityId,
    pub predicate: QueryExpr,
    pub actions: Vec<GuidanceAction>,
    pub learned: bool,
    pub effectiveness: f64,
}
```

---

### GuidanceAction

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/08-guidance.md` (section 8.1) |
| **Status** | `[AGREE]` |

```rust
pub struct GuidanceAction {
    pub command: String,
    pub invariant_refs: Vec<String>,
    pub postconditions: Vec<EntityId>,
    pub score: f64,
}
```

---

### GuidanceFooter

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/08-guidance.md` (section 8.1) |
| **Status** | `[AGREE]` |

```rust
pub struct GuidanceFooter {
    pub next_action: String,
    pub invariant_refs: Vec<String>,
    pub uncommitted_count: u32,
    pub drift_warning: Option<String>,
}
```

---

### MethodologyScore

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/08-guidance.md` (section 8.1) |
| **Status** | `[AGREE]` |

```rust
pub struct MethodologyScore {
    pub total: f64,
    pub components: [f64; 5],
    pub weights: [f64; 5],
    pub trend: Trend,
}
```

---

### Trend

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/08-guidance.md` (section 8.1) |
| **Status** | `[AGREE]` |

```rust
pub enum Trend { Up, Down, Stable }
```

---

### DerivationRule

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/08-guidance.md` (section 8.1) |
| **Status** | `[AGREE]` |

```rust
pub struct DerivationRule {
    pub entity: EntityId,
    pub artifact_type: String,
    pub task_template: TaskTemplate,
    pub dependency_fn: QueryExpr,
    pub priority_fn: PriorityFn,
}
```

---

### TaskTemplate

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/08-guidance.md` (section 8.1) |
| **Status** | `[AGREE]` |

```rust
pub struct TaskTemplate {
    pub task_type: String,
    pub title_pattern: String,
    pub attributes: Vec<(Attribute, ValueTemplate)>,
}
```

---

### RoutingDecision

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/08-guidance.md` (section 8.1) |
| **Status** | `[AGREE]` |

```rust
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

---

### DriftSignals

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | -- |
| **Guide files** | `guide/08-guidance.md` (section 8.1) |
| **Status** | `[GUIDE-ONLY]` |

```rust
pub struct DriftSignals {
    pub turns_without_ddis: usize,
    pub schema_changes_unvalidated: bool,
    pub high_confidence_unharvested: bool,
    pub approaching_budget_threshold: bool,
    pub using_pretrained_patterns: bool,
    pub missing_inv_references: bool,
    pub drift_score: f64,
}
```

---

### GuidanceOutput

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | -- |
| **Guide files** | `guide/08-guidance.md` (section 8.1) |
| **Status** | `[GUIDE-ONLY]` |

```rust
pub struct GuidanceOutput {
    pub recommendation: String,
    pub drift_assessment: String,
    pub relevant_invs: Vec<String>,
    pub next_action: String,
    pub footer: GuidanceFooter,
    pub methodology_score: MethodologyScore,
    pub routing: Option<RoutingDecision>,
}
```

---

### ClaudeMdConfig

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/08-guidance.md` (section 8.2) |
| **Status** | `[AGREE]` |

```rust
pub struct ClaudeMdConfig {
    pub ambient: AmbientSection,
    pub active: ActiveSection,
}
```

---

### AmbientSection

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/08-guidance.md` (section 8.2) |
| **Status** | `[AGREE]` |

```rust
pub struct AmbientSection {
    pub tool_awareness: String,
    pub identity: String,
}
```

---

### ActiveSection

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | `guide/08-guidance.md` (section 8.2) |
| **Status** | `[AGREE]` |

```rust
pub struct ActiveSection {
    pub demonstrations: Vec<Demonstration>,
    pub constraints: Vec<DriftCorrection>,
    pub context: SessionContext,
}
```

---

### Topology (multi-agent)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 3 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum Topology { Tree, Swarm, Market, Ring, Hybrid(Vec<Topology>) }
```

---

### TopologyRecommendation

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 3 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct TopologyRecommendation {
    pub recommended: Topology,
    pub fitness: f64,
    pub current: Topology,
    pub alternatives: Vec<(Topology, f64)>,
    pub phase: ProjectPhase,
}
```

---

### ProjectPhase

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 3 |
| **Spec file** | `spec/12-guidance.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub enum ProjectPhase { Ideation, Specification, Implementation, Verification, Reconciliation }
```

---

## BUDGET

### BudgetManager

| Field | Value |
|-------|-------|
| **Namespace** | BUDGET |
| **Stage** | 1 |
| **Spec file** | `spec/13-budget.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct BudgetManager {
    pub k_eff: f64,
    pub q: f64,
    pub output_budget: u32,
}
```

---

### OutputPrecedence

| Field | Value |
|-------|-------|
| **Namespace** | BUDGET |
| **Stage** | 1 |
| **Spec file** | `spec/13-budget.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum OutputPrecedence {
    Ambient = 0,
    Speculative = 1,
    UserRequested = 2,
    Methodology = 3,
    System = 4,
}
```

---

### TokenEfficiency

| Field | Value |
|-------|-------|
| **Namespace** | BUDGET |
| **Stage** | 1 |
| **Spec file** | `spec/13-budget.md` (INV-BUDGET-006 L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct TokenEfficiency {
    pub semantic_units: usize,
    pub token_count: usize,
}
```

---

## INTERFACE

### OutputMode

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Spec file** | `spec/14-interface.md` (L2) |
| **Guide files** | `guide/00-architecture.md` (section 0.2), `guide/09-interface.md` (section 9.2) |
| **Status** | `[AGREE]` |

```rust
pub enum OutputMode { Json, Agent, Human }
```

---

### ToolResponse

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Spec file** | -- (implements ADR-INTERFACE-002's 5-part output structure) |
| **Guide files** | `guide/00-architecture.md` (sections 0.2, 0.6), `guide/09-interface.md` (section 9.2) |
| **Status** | `[GUIDE-ONLY]` -- Implementation refinement (R1.12): concretizes spec's 5 semantic parts into 4-field struct. `agent_context`=headline, `agent_content`=entities+signals, footer (guidance+pointers) injected by `format_output()`. Both guide locations now agree on field names. |

```rust
pub struct ToolResponse {
    pub structured: serde_json::Value,  // Full data (JSON mode)
    pub agent_context: String,          // ≤50 tokens — headline
    pub agent_content: String,          // ≤200 tokens — entities + signals
    pub human_display: String,          // Formatted for terminal
}
// GuidanceFooter (guidance + pointers) injected during format_output(), not stored here.
```

---

### MCPServer / BraidMcpServer

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Spec file** | `spec/14-interface.md` (L2) |
| **Guide files** | `guide/09-interface.md` (section 9.2) |
| **Status** | `[ALIGNED]` -- Divergence resolved (R0.3c) |

```rust
// Spec definition:
pub struct MCPServer {
    pub store: ArcSwap<Store>,              // Loaded once at MCP_INIT; swapped on transact
    pub session_state: SessionState,
    pub notification_queue: Vec<Signal>,
    pub phase: MCPPhase,                    // Uninitialized → Initialized → Shutdown
}

// Guide definition (rmcp integration):
pub struct BraidMcpServer {
    store: ArcSwap<Store>,                  // Loaded once at init; swapped on write ops
    session_state: SessionState,
    notification_queue: Vec<Signal>,
}
// Guide omits `phase` field because rmcp manages lifecycle state externally.
// This is an intentional implementation refinement, not a divergence.
//
// ArcSwap<Store> implements the Datomic connection model: Store values are immutable
// (C1), the pointer swaps atomically after transact/harvest. Reads are lock-free
// via hazard pointers; in-flight queries see consistent snapshots.
```

---

### MCPTool

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Spec file** | `spec/14-interface.md` (L2) |
| **Guide files** | `guide/09-interface.md` (section 9.2) |
| **Status** | `[AGREE]` -- Resolved (R1.12): guide/09-interface.md defines identical enum + `const MCP_TOOLS: [MCPTool; 6]` |

```rust
pub enum MCPTool {
    Transact,   // meta: side effect — assert/retract datoms
    Query,      // moderate: 50–300 tokens — Datalog query
    Status,     // cheap: ≤50 tokens — store summary + M(t) + drift
    Harvest,    // meta: side effect — extract session knowledge
    Seed,       // expensive: 300+ tokens — session initialization
    Guidance,   // cheap: ≤50 tokens — methodology steering + R(t)
}
```

---

### SessionState

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 1 |
| **Spec file** | `spec/14-interface.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
#[derive(Serialize, Deserialize)]
pub struct SessionState {
    pub used_percentage: f64,
    pub input_tokens: u64,
    pub remaining_tokens: u64,
    pub k_eff: f64,
    pub quality_adjusted: f64,
    pub output_budget: u32,
    pub timestamp: u64,
    pub session_id: String,
}
```

---

### TUIState

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 4 |
| **Spec file** | `spec/14-interface.md` (L2) |
| **Guide files** | -- |
| **Status** | `[SPEC-ONLY]` |

```rust
pub struct TUIState {
    pub subscriptions: Vec<Subscription>,
    pub active_display: DisplayState,
}
```

---

### ToolDescription

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Spec file** | `spec/14-interface.md` (INV-INTERFACE-008 L2) |
| **Guide files** | `guide/09-interface.md` (section 9.2) |
| **Status** | `[AGREE]` |

```rust
pub struct ToolDescription {
    pub name: &'static str,
    pub purpose: &'static str,
    pub inputs: &'static [TypedParam],
    pub output: &'static str,
    pub example: &'static str,
}
```

---

### RecoveryHint

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Spec file** | `spec/14-interface.md` (INV-INTERFACE-009 L2) |
| **Guide files** | `guide/09-interface.md` (section 9.3) |
| **Status** | `[AGREE]` |

```rust
pub struct RecoveryHint {
    pub action: RecoveryAction,
    pub spec_ref: &'static str,
}
```

---

### RecoveryAction

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Spec file** | `spec/14-interface.md` (INV-INTERFACE-009 L2) |
| **Guide files** | `guide/09-interface.md` (section 9.3) |
| **Status** | `[AGREE]` |

```rust
pub enum RecoveryAction {
    RetryWith(String),
    CheckPrecondition(String),
    UseAlternative(String),
    EscalateToHuman(String),
}
```

---

## TRILATERAL

### AttrNamespace

| Field | Value |
|-------|-------|
| **Namespace** | TRILATERAL |
| **Stage** | 0 |
| **Spec file** | `spec/18-trilateral.md` (INV-TRILATERAL-005 L2) |
| **Guide files** | `guide/13-trilateral.md` (§13.2) |
| **Status** | `[AGREE]` |

```rust
pub enum AttrNamespace {
    Intent,
    Spec,
    Impl,
    Meta,   // db/*, tx/* — not counted in trilateral projections
}
```

**Notes**: Classifies attributes into the three trilateral boundaries (Intent, Spec, Impl)
plus a Meta category for infrastructure attributes. Exhaustive match enforced by V:TYPE
(INV-TRILATERAL-005).

---

### LiveView

| Field | Value |
|-------|-------|
| **Namespace** | TRILATERAL |
| **Stage** | 0 |
| **Spec file** | `spec/18-trilateral.md` (INV-TRILATERAL-001 L1) |
| **Guide files** | `guide/13-trilateral.md` (§13.2) |
| **Status** | `[AGREE]` |

```rust
pub struct LiveView {
    pub datoms: Vec<Datom>,
    pub entity_count: usize,
    pub namespace: AttrNamespace,
}
```

**Notes**: Result of a LIVE projection over one of the three attribute namespaces.
Monotone: `S₁ ⊆ S₂ ⟹ LiveView(S₁).datoms ⊆ LiveView(S₂).datoms`.

---

## Appendix A: Divergence Summary

This appendix catalogs all divergences between `spec/` and `guide/` for downstream
reconciliation beads (R1.2, R1.3, R1.7).

### Critical Divergences (Structural Mismatch)

| # | Type | Spec Source | Guide Source | Nature |
|---|------|------------|-------------|--------|
| ~~D1~~ | ~~`MergeReceipt`~~ | `spec/07-merge.md` | `guide/07-merge-basic.md` | **RESOLVED (R1.6)**: Spec section 7.3 updated to match INV-MERGE-009. Old cascade fields moved to `CascadeReceipt` (now in both spec and guide). `merge()` returns `(MergeReceipt, CascadeReceipt)`. |
| ~~D2~~ | ~~`Clause`~~ | `spec/03-query.md` | `guide/03-query.md` | **RESOLVED (R6.7b)**: Spec adopted guide's Datalog-standard naming (DataPattern, RuleApplication, NotClause, OrClause, Frontier) with Aggregate/Ffi stage-deferred. |
| ~~D3~~ | ~~`ConflictSet` / `Conflict`~~ | `spec/04-resolution.md` | `guide/04-resolution.md` | **RESOLVED (R1.10)**: Spec adopted `ConflictSet` with `assertions` + `retractions`. Routing metadata (severity, tier, status) are computed during routing, not embedded. |
| ~~D4~~ | ~~`ResolutionMode`~~ | `spec/04-resolution.md` | `guide/04-resolution.md` | **RESOLVED (R1.10)**: Spec adopted guide naming: `LastWriterWins`, `MultiValue`. LwwClock moved to schema layer. |

### Moderate Divergences (Field/Name Differences)

| # | Type | Nature |
|---|------|--------|
| ~~D5~~ | ~~`QueryResult`~~ | **RESOLVED (R6.7b)**: Spec adopted guide's `bindings: Vec<BindingSet>` and `stratum: Stratum` enum. More type-safe. |
| ~~D6~~ | ~~`QueryExpr`~~ | **RESOLVED (R6.7b)**: Spec adopted guide's `Find(ParsedQuery)` form. ParsedQuery subsumes inline fields. |
| ~~D7~~ | ~~`QueryMode`~~ | **RESOLVED (R6.7b)**: Spec adopted guide's tuple variant form `Stratified(Frontier)`, `Barriered(BarrierId)`. |
| ~~D8~~ | ~~`HarvestCandidate`~~ | **RESOLVED (R6.7b)**: Spec added `id: usize` and `reconciliation_type: ReconciliationType` from guide. |
| ~~D9~~ | ~~`CandidateStatus`~~ | **RESOLVED (R4.1b)**: Spec now defines Rust enum with `Rejected(String)` matching guide. |
| ~~D10~~ | ~~`AssembledContext`~~ | **RESOLVED (R6.7b)**: Guide added `projection_pattern: ProjectionPattern` field from spec. |
| ~~D11~~ | ~~`MCPServer`~~ | **RESOLVED**: Both now use `{store: ArcSwap<Store>, session_state, notification_queue, phase}`. Guide renames to `BraidMcpServer` for rmcp integration. ArcSwap implements the Datomic connection model (immutable Store values, atomic pointer swap on writes). |
| ~~D12~~ | ~~`ConflictTier` / `RoutingTier`~~ | **RESOLVED (R1.10)**: Spec adopted `RoutingTier`. |
| ~~D13~~ | ~~`TxId`~~ | **RESOLVED (R6.7d)**: Intentional — spec silent on derive attributes; guide correctly omits Hash on TxId (only AgentId needs Hash). Not a defect. |

### Intentional Stage Scoping (Not Defects)

| # | Type | Nature |
|---|------|--------|
| S1 | `Value` | Spec: 14 variants. Guide: 9 (BigInt, BigDec, Tuple, Json, URI deferred). |
| S2 | `Stratum` | Spec: 6 strata (S0-S5). Guide: 2 (S0, S1; rest deferred to Stage 1+). |
| S3 | `Clause` variants | Guide defers `Aggregate`, `Ffi` to Stage 1+. |

### Types Defined Only in Spec (No Guide Coverage)

These types are from namespaces not yet covered by guide files (SYNC, SIGNAL,
BILATERAL, DELIBERATION are Stage 2-3; BUDGET is Stage 1):

`Barrier`, `BarrierResult`, `BarrierStatus`, `SignalType`, `ConfusionKind`, `Signal`,
`Severity`, `Subscription`, `BilateralLoop`, `Boundary`, `Gap`, `DocumentedResidual`,
`CycleReport`, `Deliberation`, `DeliberationStatus`, `Position`, `Decision`, `Stance`,
`DecisionMethod`, `BudgetManager`, `OutputPrecedence`, `TokenEfficiency`, `Branch`,
`BranchStatus`, `CombineStrategy`, `ComparisonCriterion`, `BranchComparison`,
`HITSResult`, `ArticulationResult`, `Topology`, `TopologyRecommendation`,
`ProjectPhase`, `SessionState`, `TUIState`, `ConflictStatus`,
`ContextSection`, `ProjectionLevel`,
`ClaudeMdGenerator`, `AssociateCue`.

Note: `LwwClock` was removed from this list (R1.10) -- clock selection is now a schema-level
concept (`:db/lwwClock` attribute), not a standalone Resolution namespace type.

Note: `Resolution`, `HarvestSession`, `ReviewTopology` were removed from this list (R4.3b) --
now defined in both spec and guide.

Note: `MCPTool` was removed from this list (R1.12) -- guide/09-interface.md now defines the
identical enum and `const MCP_TOOLS: [MCPTool; 6]` array, matching spec/14-interface.md.

### Types Defined Only in Guide (No Spec Coverage)

These types are implementation refinements introduced by the guide:

`QueryStats`, `FrontierRef`, `DirectedGraph`, `ResolvedValue`,
`ReconciliationType`, `HarvestResult`, `HarvestQuality`, `SessionContext`,
`SeedOutput`, `DriftSignals`, `GuidanceOutput`, `ToolResponse`.

Note: `CascadeReceipt` was removed from this list (R1.6) -- now in both spec and guide.

Note: `TxReceipt`, `TxValidationError`, `SchemaError` were removed from this list (R4.1b/R4.2b)
-- now defined in both spec and guide.

Note: `ParsedQuery`, `FindSpec`, `BindingSet` were removed from this list (R6.7b) --
now defined in both spec and guide via QueryExpr reconciliation.

---

*Total types cataloged: 113 (+9 new: Frontier, EntityView, SnapshotView, TxApplyError, QueryError, CandidateStatus enum, LwwClock enum, SchemaLayer enum, Stratum enum).
Divergences: 0 remaining, 13 resolved (D1/R1.6, D2/R6.7b, D3/R1.10, D4/R1.10, D5/R6.7b, D6/R6.7b, D7/R6.7b, D8/R6.7b, D9/R4.1b, D10/R6.7b, D11, D12/R1.10, D13/R6.7d).
Intentional stage scoping: 3 (S1: Value, S2: Stratum, S3: Clause deferred variants). Spec-only: 33 (Resolution, HarvestSession, ReviewTopology moved to AGREE per R4.3b).
Guide-only: 12 (ParsedQuery, FindSpec, BindingSet moved to AGREE per R6.7b; TxReceipt, TxValidationError, SchemaError moved to AGREE per R4.1b/R4.2b).*
