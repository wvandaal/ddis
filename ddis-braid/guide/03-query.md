# §3. QUERY — Build Plan

> **Spec reference**: [spec/03-query.md](../spec/03-query.md) — read FIRST
> **Stage 0 elements**: INV-QUERY-001–002, 005–007 (5 INV), ADR-QUERY-001–003, 005–006, NEG-QUERY-001–004
> **Dependencies**: STORE (§1), SCHEMA (§2)
> **Cognitive mode**: Language-theoretic — Datalog semantics, CALM theorem, fixpoint evaluation

---

## §3.1 Module Structure

```
braid-kernel/src/query/
├── mod.rs          ← Query engine entry: parse → classify → evaluate → return
├── parser.rs       ← Datalog parser ([:find ... :where ...] syntax)
├── clause.rs       ← Clause, Pattern, Binding, BindingSet
├── evaluator.rs    ← Semi-naive bottom-up fixpoint evaluation
└── strata.rs       ← Stratum classification (0–5), CALM analysis
```

### Public API Surface

```rust
/// Execute a Datalog query against the store.
pub fn query(store: &Store, q: &str) -> Result<QueryResult, QueryError>;

/// Parse without executing (for validation).
pub fn parse(q: &str) -> Result<ParsedQuery, ParseError>;

/// Classify the stratum of a parsed query.
pub fn classify_stratum(q: &ParsedQuery) -> Stratum;

pub struct ParsedQuery {
    pub find_spec:  FindSpec,
    pub where_clauses: Vec<Clause>,
    pub rules:      Vec<Rule>,
    pub inputs:     Vec<Input>,
}

pub struct QueryResult {
    pub bindings: Vec<BindingSet>,
    pub stratum:  Stratum,
}

pub enum FindSpec {
    Relation(Vec<Variable>),    // [:find ?x ?y]
    Scalar(Variable),           // [:find ?x .]
    Collection(Variable),       // [:find [?x ...]]
    Tuple(Vec<Variable>),       // [:find [?x ?y]]
}

pub enum Clause {
    DataPattern(EntityPattern, AttributePattern, ValuePattern),
    RuleApplication(RuleName, Vec<Term>),
    NotClause(Box<Clause>),       // Stratum 1+ only
    OrClause(Vec<Vec<Clause>>),
}

pub enum Stratum {
    S0_Ground,         // Pure data lookup, no joins
    S1_MonotonicJoin,  // Joins, recursion, no negation
    // S2–S5 deferred to Stage 1+
}
```

---

## §3.2 Three-Box Decomposition

### Query Engine

**Black box** (contract):
- INV-QUERY-001: CALM compliance — monotonic queries produce monotonic results. Adding datoms
  to the store can only add results, never remove them.
- INV-QUERY-002: Fixpoint termination — semi-naive evaluation terminates for all valid queries
  (guaranteed by Datalog semantics: finite domain, no function symbols).
- INV-QUERY-005: Mode-stratum compatibility — monotonic mode rejects Stratum 2+ queries.
- INV-QUERY-006: FFI boundary purity — any host-language functions referenced in queries are pure.
- INV-QUERY-007: Typed clause patterns — clause types enforce that only valid patterns compile.

**State box** (internal design):
- Parser: convert string → `ParsedQuery`. Datomic-style syntax: `[:find ?vars :where [clauses]]`.
- Stratum classifier: walk the AST → classify each clause → overall stratum is max of all clauses.
- Evaluator: semi-naive bottom-up fixpoint.
  - Initialize working set from data patterns (index lookups).
  - Iterate: apply rules, compute new bindings (delta), add to working set.
  - Terminate when delta is empty (fixpoint reached).
- For Stage 0: only Stratum 0 (ground) and Stratum 1 (monotonic join) supported.

**Clear box** (implementation):
- Parser: pest grammar or nom combinators. The syntax is:
  ```
  query = "[:find" find-spec ":where" clause+ "]"
  find-spec = var+ | var "." | "[" var "..." "]" | "[" var+ "]"
  clause = "[" pattern pattern pattern "]"
  pattern = var | literal | "_"
  var = "?" ident
  ```
- Index lookup: `[?e :db/ident ?name]` → scan AEVT index for attribute `:db/ident`.
- Join: Nested loop join for Stage 0. Hash join optimization deferred to Stage 1.
- Semi-naive delta: track new bindings per iteration → only join new bindings against old.

### Stratum Classification

**Black box**: Given a parsed query, classify it into a stratum.
- S0: No joins, no rules — pure index lookup.
- S1: Joins and/or recursive rules, but no negation/aggregation.
- S2+: Deferred. Query with negation in monotonic mode → `QueryError::StratumViolation`.

**Clear box**:
- Walk clauses: `NotClause` → Stratum 2+. `RuleApplication` → Stratum 1+.
  Multiple `DataPattern` with shared variables → Stratum 1 (join).

---

## §3.3 Type-Level Encoding

| INV | Compile-Time Guarantee | Mechanism |
|-----|----------------------|-----------|
| INV-QUERY-005 | `QueryMode::Monotonic` rejects Stratum 2+ | `match (mode, stratum)` at eval entry |
| INV-QUERY-006 | FFI purity marker trait | `trait FfiFunction: Pure` (Stage 0: no FFI, vacuously true) |
| INV-QUERY-007 | Typed clause patterns | `Clause` enum — only valid patterns expressible |

---

## §3.4 LLM-Facing Outputs

### Agent-Mode Output — `braid query`

```
[QUERY] 5 results (Stratum 1, monotonic). Attributes: ?id, ?type.
  INV-STORE-001  invariant
  INV-STORE-002  invariant
  INV-STORE-003  invariant
  INV-STORE-004  invariant
  INV-STORE-005  invariant
---
↳ All results are stable (CALM monotonic — adding datoms can only add results).
  Explore: `braid query '[:find ?id ?dep :where [?e :spec/id ?id] [?e :spec/depends-on ?d] [?d :spec/id ?dep]]'`
```

### Error Messages

- **Parse error**: `Query error: unexpected token at position {N} — expected {expected} — See Datalog syntax in spec/03-query.md §3.3`
- **Stratum violation**: `Query error: negation requires stratified mode — use --mode stratified — See: INV-QUERY-005`
- **No results**: `[QUERY] 0 results. Verify attribute names match schema: braid query '[:find ?a :where [_ :db/ident ?a]]'`

---

## §3.5 Verification

### Key Properties

```rust
proptest! {
    // INV-QUERY-001: CALM monotonicity
    fn inv_query_001(store in arb_store(5), extra_datoms in arb_datoms(3), query in arb_monotonic_query()) {
        let r1 = query(&store, &query).bindings;
        let mut bigger_store = store.clone();
        bigger_store.add_datoms(extra_datoms);
        let r2 = query(&bigger_store, &query).bindings;
        // r1 ⊆ r2: every result in the smaller store appears in the bigger store
        for binding in &r1 {
            prop_assert!(r2.contains(binding));
        }
    }

    // INV-QUERY-002: Fixpoint termination
    fn inv_query_002(store in arb_store(10), query in arb_query()) {
        // Must terminate (no timeout). proptest will catch infinite loops via timeout.
        let _ = query(&store, &query);
    }
}
```

---

## §3.6 Implementation Checklist

- [ ] Datalog parser handles `[:find ... :where ...]` syntax
- [ ] Stratum classifier distinguishes S0 (ground) from S1 (monotonic join)
- [ ] Semi-naive evaluator reaches fixpoint
- [ ] Index lookups use EAVT/AEVT/VAET/AVET correctly
- [ ] Mode-stratum compatibility enforced (monotonic rejects S2+)
- [ ] CALM monotonicity holds (proptest)
- [ ] Fixpoint termination holds (proptest)
- [ ] Integration: genesis → schema query → spec-element query round-trip
- [ ] Error messages follow protocol (what + why + recovery + ref)

---
