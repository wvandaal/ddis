# D2 — Datalog Engine Selection

> **Thread**: R3.2a — Evaluate Rust Datalog engines for Braid
> **Date**: 2026-03-03
> **Sources**: spec/03-query.md, crates.io, GitHub repositories

---

## Research Questions

1. What Rust Datalog engines exist?
2. Can we use an existing one or must we build custom?
3. What features does Braid need (semi-naive, stratified negation, FFI)?
4. Does any engine support runtime (dynamic) query construction?

---

## Braid Requirements (from spec/03-query.md)

The spec mandates:

| Requirement | Source | Priority |
|-------------|--------|----------|
| Semi-naive bottom-up evaluation | ADR-QUERY-002, INV-QUERY-006 | MUST |
| Stratified negation | INV-QUERY-001 (CALM), INV-QUERY-005 | MUST (Stage 2+) |
| Aggregation | Stratum 2-3 queries | MUST (Stage 1+) |
| FFI for derived functions | ADR-QUERY-004, INV-QUERY-008 | MUST (Stage 1) |
| Runtime query construction | `braid query '[:find ...]'` CLI | MUST |
| EAV triple pattern matching | Core data model | MUST |
| Frontier-scoped evaluation | INV-QUERY-007 | MUST |
| Query monotonicity classification | INV-QUERY-001 | MUST |
| Query determinism | INV-QUERY-002 | MUST |
| Six-stratum classification | ADR-QUERY-003, INV-QUERY-005 | MUST |
| Graph algorithms as kernel operations | ADR-QUERY-009 | MUST |
| Content-addressed identity dedup | INV-STORE-002 | MUST |

**Critical constraint**: The CLI command `braid query '[:find ?e ?name :where [?e :db/ident ?name]]'`
requires **runtime query parsing and evaluation**. This rules out any compile-time-only solution.

---

## Candidate Engines

### 1. Datafrog

- **Repository**: https://github.com/rust-lang/datafrog
- **Architecture**: Lightweight runtime library. No compile-time macros. You build
  relations and variables, then call `while_not_empty()` to iterate to fixpoint.
- **Semi-naive**: YES — core design principle. Uses delta relations internally.
- **Negation**: Partial — supports anti-joins via `FilterAnti` and `ExtendAnti` leapers,
  but no stratified negation framework. You must manually sequence strata.
- **Aggregation**: NO — no built-in aggregation. Must be done post-fixpoint.
- **FFI**: YES (trivially) — it is a Rust library, so any Rust function is callable.
- **Runtime queries**: YES — relations and rules are built at runtime via API calls.
- **Activity**: Low. Last meaningful update ~2021. Used in rustc (Polonius borrow checker).
- **Lines of code**: ~1500 LOC. Very small.

**Verdict**: Good foundation for building on. Provides the low-level primitives (leapjoin,
semi-naive iteration) but requires building the Datalog layer (parser, stratification,
query planning) on top. No "Datalog" in the traditional sense — it is a fixpoint engine.

### 2. Crepe

- **Repository**: https://github.com/ekzhang/crepe
- **Architecture**: Procedural macro. Datalog rules defined at compile time in `crepe!{}` blocks.
- **Semi-naive**: YES — explicitly documented feature.
- **Negation**: YES — stratified negation supported.
- **Aggregation**: NO — not documented.
- **FFI**: Limited — can call Rust functions from within rules, but not as formal FFI.
- **Runtime queries**: **NO** — compile-time only. Rules are macro-expanded to Rust code
  at compile time. Cannot parse and evaluate a query string at runtime.
- **Activity**: Moderate. Last release 0.1.8. Inspired by Souffle.
- **Performance**: Comparable to compiled Souffle for transitive closure.

**Verdict**: DISQUALIFIED for Braid. The compile-time-only constraint makes it impossible
to implement `braid query '[:find ...]'`. There is no way to construct queries at runtime.

### 3. Ascent

- **Repository**: https://github.com/s-arash/ascent
- **Architecture**: Macro-based. Logic programs defined at compile time in `ascent!{}` blocks.
- **Semi-naive**: Not explicitly documented but likely used internally.
- **Negation**: YES — stratified negation supported.
- **Aggregation**: YES — sum, min, max, count, mean built-in, plus custom aggregators.
- **FFI**: NO — no formal FFI mechanism.
- **Runtime queries**: **NO** — compile-time only. All relations and rules are macro-expanded.
- **Lattice operations**: YES — can compute fixpoints over user-defined lattices.
- **Parallel execution**: YES — via `ascent_par!` macros (Rayon-based).
- **Activity**: Active. v0.8.0 released 2025.

**Verdict**: DISQUALIFIED for Braid. Same fundamental issue as Crepe: compile-time only.
The aggregation and lattice features are appealing, but without runtime query construction,
it cannot serve as the Braid query engine. Could potentially be used for specific
compile-time-known query patterns (e.g., graph algorithms) as an optimization.

### 4. Differential Datalog (DDlog)

- **Repository**: https://github.com/vmware/differential-datalog (ARCHIVED)
- **Architecture**: Standalone language compiled to Rust. Separate DDlog compiler (Haskell-based).
- **Semi-naive**: YES — based on differential dataflow (Frank McSherry).
- **Negation**: YES — stratified.
- **Aggregation**: YES — rich aggregation support.
- **FFI**: YES — compiled to Rust library, callable from Rust/C/C++/Java/Go.
- **Runtime queries**: Partial — input facts can be added/removed at runtime. Rules are
  compiled, not dynamically constructed.
- **Activity**: **ARCHIVED** (VMware). No active development.
- **Incremental**: YES — fully incremental computation.

**Verdict**: DISQUALIFIED. Archived project, Haskell toolchain dependency for compilation,
and rules are still compile-time. The incremental computation model is interesting but
the project is dead.

### 5. Custom Engine

- **Architecture**: Build Braid's own Datalog evaluator from scratch.
- **Approach**: Parse Datalog expressions at runtime, classify into strata, evaluate
  via semi-naive iteration over the datom store's indexes.
- **All requirements**: Achievable by design.

**Verdict**: The only option that satisfies all requirements.

---

## Decision Matrix

| Feature | Datafrog | Crepe | Ascent | DDlog | Custom |
|---------|----------|-------|--------|-------|--------|
| Semi-naive | YES | YES | Likely | YES | Build |
| Stratified negation | Manual | YES | YES | YES | Build |
| Aggregation | NO | NO | YES | YES | Build |
| FFI | Trivial | Limited | NO | YES | Build |
| Runtime queries | YES | **NO** | **NO** | Partial | YES |
| Active maintenance | Low | Moderate | Active | DEAD | N/A |
| EAV pattern matching | Manual | NO | NO | NO | Build |
| Frontier scoping | NO | NO | NO | NO | Build |

---

## Recommendation: Hybrid Approach

### Primary: Custom Datalog Evaluator

Build a custom evaluator for Braid's specific needs:
- Runtime query parsing (Datomic-style `:find/:where` syntax)
- Semi-naive evaluation over the datom store's EAVT/AEVT/AVET/VAET indexes
- Stratum classification and mode enforcement (INV-QUERY-005)
- Frontier-scoped evaluation (INV-QUERY-007)
- Query provenance recording (INV-STORE-014)

### Consider: Datafrog as Foundation

Datafrog's core primitives (leapjoin, semi-naive iteration infrastructure) could
serve as the low-level engine beneath the custom Datalog layer:
- Use `Relation` and `Variable` types for the evaluation core
- Build the Datalog parser and stratum classifier on top
- Benefit from Datafrog's proven correctness (used in rustc's Polonius)

**Risk**: Datafrog is lightly maintained. At ~1500 LOC, it may be simpler to
reimplement the core primitives directly rather than taking a dependency.

### Reject: Compile-Time Engines

Crepe and Ascent are both excellent for their intended use case (static analysis,
compile-time-known rules) but fundamentally incompatible with Braid's requirement
for runtime query construction.

---

## Implementation Estimate

Building a custom Datalog evaluator for Stage 0:

| Component | LOC Estimate | Complexity |
|-----------|-------------|------------|
| Query parser (Datomic-style) | 500-800 | Medium |
| Semi-naive evaluator | 400-600 | High |
| Stratum classifier | 200-300 | Medium |
| EAV index integration | 300-500 | Medium |
| Frontier scoping | 100-200 | Low |
| Graph algorithms (6 for Stage 0) | 600-1000 | Medium |
| **Total** | **2100-3400** | |

This is achievable but represents a significant portion of Stage 0 work.
The evaluator is the most critical path item — everything else (harvest, seed,
guidance) depends on it.

---

## Open Questions

1. Should we prototype with datafrog's leapjoin primitives and upgrade to custom
   if needed, or build from scratch?
2. The spec references Datomic-style query syntax (`[:find ?e :where ...]`). Should
   we commit to this syntax or consider a simpler initial syntax for Stage 0?
3. How much of the six-stratum classification is needed at Stage 0? Only Strata 0-1
   are used (monotonic queries). Strata 2-5 can be stubbed until later stages.
