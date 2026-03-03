# ADRS.md — Design Decision Index

> **Purpose**: Canonical index of all design decisions made during Braid ideation and
> development. This is the distillation of decisions from the original design transcripts,
> plus any additions or revisions harvested during implementation.
>
> **Formalization**: These entries are intentionally lightweight — sufficient to recall
> the decision, its rationale, and where to find the full discussion. Formal invariants,
> falsification conditions, and cross-references will be added when these decisions are
> formalized in `SPEC.md` using DDIS methodology.
>
> **Lifecycle**: Decisions start here as harvested records from transcripts or sessions.
> They graduate to `SPEC.md` as formal ADR elements (with IDs like `ADR-STORE-001`).
> This file remains the human-readable index even after formalization.

---

## How to Use This Document

- **At session start**: Scan for decisions relevant to your task. Do not relitigate settled
  decisions (NEG-002) unless you find a formal contradiction with another decision.
- **During work**: If you make or discover a design decision, add it here immediately.
- **At session end**: Verify any new decisions are recorded. Include transcript references
  where available.

---

## Table of Contents

1. [Foundational Decisions (FD)](#foundational-decisions) — Core architectural axioms
2. [Algebraic Structure (AS)](#algebraic-structure-decisions) — G-Set CRDT, branching, commitment weight, lattice algebra
3. [Store & Runtime Architecture (SR)](#store--runtime-architecture-decisions) — Deployment, indexes, file layout, schema layers
4. [Protocol Decisions (PD)](#protocol-decisions) — Protocol-level design questions (PQ1–PQ4+)
5. [Protocol Operations (PO)](#protocol-operations-decisions) — TRANSACT, QUERY, ASSOCIATE, etc.
6. [Snapshot & Query Decisions (SQ)](#snapshot--query-decisions) — Query semantics, frontiers, strata, Datalog boundaries
7. [Uncertainty & Authority (UA)](#uncertainty--authority-decisions) — Tensor, spectral authority, delegation, measurement invariants
8. [Conflict & Resolution (CR)](#conflict--resolution-decisions) — Detection, routing, deliberation, precedent
9. [Agent Architecture (AA)](#agent-architecture-decisions) — Dual-process, metacognition, agent cycle
10. [Interface & Budget (IB)](#interface--budget-decisions) — Five layers, k*, output modes, budget enforcement
11. [Guidance System (GU)](#guidance-system-decisions) — Comonadic structure, spec-language, anti-drift mechanisms
12. [Lifecycle & Methodology (LM)](#lifecycle--methodology-decisions) — Harvest/seed, self-bootstrap, change management
13. [Coherence & Reconciliation (CO)](#coherence--reconciliation-decisions) — Verification framework, taxonomy, boundaries

---

## Foundational Decisions

These are the "why" decisions — fundamental architectural choices that shape everything downstream.

### FD-001: Append-Only Store

**Decision**: The datom store never deletes or mutates. Retractions are new datoms with `op=retract`.
**Rationale**: Mutable state is the root of most correctness bugs in distributed systems. Append-only plus content-addressing gives arbitrary time-travel for free.
**Rejected**: Mutable state (correctness risk, no time-travel); 2P-Set (once retracted, never re-assertable — gives any agent permanent veto power); OR-Set/Add-Wins (scoped retractions with unique tags — correct but adds identity complexity; unnecessary when retractions are modeled as facts).
**Source**: SEED.md §4 Axiom 2, §11; Transcript 01 (five locked axioms, Options 2A/2B/2C)
**Formalized as**: ADR-STORE-001 in `spec/01-store.md`

### FD-002: EAV Over Relational

**Decision**: Entity-Attribute-Value data model rather than normalized relational tables.
**Rationale**: The ontology of a project evolves as the project evolves. EAV handles schema evolution without migrations. The schema crystallizes from usage rather than being declared upfront.
**Rejected**: Relational tables (schema rigidity, migration burden), document stores (no graph traversal).
**Source**: SEED.md §4, §11; Transcript 01
**Formalized as**: ADR-STORE-002 in `spec/01-store.md`

### FD-003: Datalog for Queries

**Decision**: Datalog with stratified evaluation as the query language. Datomic-style dialect with bottom-up semi-naive evaluation strategy.
**Rationale**: Natural graph joins for traceability (goal → invariant → implementation → test). Stratified evaluation maps cleanly to the monotonic/non-monotonic distinction (Axiom 4). CALM theorem compliance. Semi-naive avoids redundant derivation.
**Rejected**: SQL (poor graph traversal), custom query language (wheel reinvention), top-down evaluation (worse for materialized views).
**Source**: SEED.md §4 Axiom 4, §11; Transcript 01; Transcript 02 (stratum analysis); Transcript 03 (Datomic dialect, semi-naive)
**Formalized as**: ADR-QUERY-001, ADR-QUERY-002 in `spec/03-query.md`

### FD-004: Datom Store Over Vector DB / RAG

**Decision**: The core substrate is a datom store, not a vector database.
**Rationale**: Vector similarity retrieval finds "related" content but does not verify logical coherence, detect contradictions, or trace causal dependencies. DDIS needs a verification substrate, not just a retrieval heuristic. The "filing cabinet vs. bigger desk" argument: the problem isn't retrieval — it's coherence verification.
**Rejected**: RAG/vector DB (no coherence verification, no contradiction detection).
**Source**: SEED.md §11; Transcript 06 (filing cabinet argument)

### FD-005: Per-Attribute Conflict Resolution

**Decision**: Each attribute declares its own resolution mode: lattice-resolved, last-writer-wins, or multi-value.
**Rationale**: Different attributes have different semantics. Task status has a natural lattice (`todo < in-progress < done`). Person names do not. Global policy either loses information or produces nonsense.
**Rejected**: Global resolution policy (information loss).
**Source**: SEED.md §4 Axiom 5; Transcript 01 (five locked axioms)
**Formalized as**: ADR-RESOLUTION-001 in `spec/04-resolution.md`

### FD-006: DDIS Specifies Itself

**Decision**: The DDIS specification is written using DDIS methodology. Spec elements become the first data the system manages.
**Rationale**: (1) Integrity — a coherence verification system specified incoherently is self-undermining. (2) Bootstrapping — spec elements are the first dataset, so Stage 0 has real data on day one. (3) Validation — if DDIS can't spec DDIS, it can't spec anything.
**Rejected**: Traditional documentation (can't verify own coherence).
**Source**: SEED.md §10, §11; Transcript 06–07

### FD-007: Content-Addressable Identity

**Decision**: A datom is `[e, a, v, tx, op]`. Two agents independently asserting the same fact produce one datom. Identity is by content (full five-tuple hash), not by sequential ID.
**Rationale**: Eliminates the "same fact, different ID" problem in multi-agent settings. Makes set-union merge naturally deduplicate. The entity `e` is derived from content hash, not assigned sequentially.
**Rejected**: Sequential IDs (merge conflicts, agent-local ID spaces).
**Source**: SEED.md §4 Axiom 1; Transcript 01 (five-tuple identity)
**Formalized as**: ADR-STORE-003 in `spec/01-store.md`

### FD-008: Schema-as-Data

**Decision**: Schema is defined as datoms in the store. Schema evolution is a transaction, not a migration.
**Rationale**: Aligns with EAV (FD-002). The schema should evolve with the project without requiring code changes or DDL migrations.
**Rejected**: Hardcoded DDL (the Go CLI approach — 39 `CREATE TABLE` statements in Go source).
**Source**: SEED.md §4 Constraint C3
**Formalized as**: ADR-SCHEMA-001 in `spec/02-schema.md`

### FD-009: Datom Store Replaces JSONL Event Stream

**Decision**: The datom store is the canonical substrate. It replaces the JSONL-based event stream used in the Go CLI.
**Rationale**: JSONL events are sequential and file-scoped. Datoms are content-addressed and set-scoped. The datom model subsumes event-sourcing (events become transactions) while adding CRDT merge, multi-agent coordination, and graph-structured querying.
**Rejected**: (B) JSONL as application layer with datom store underneath (dual source of truth, dual-write problem violating APP-INV-071); (C) JSONL as derived view projected from datom store (impedance mismatch between datom transactions and JSONL events; conceptual pollution of two mental models). Both rejected in favor of single canonical store.
**Source**: Transcript 01:762–860 (Options A/B/C analysis)

### FD-010: Embedded Deployment Model

**Decision**: Braid deploys as an embedded, single-process system (analogous to SQLite). No separate database server or daemon required.
**Rationale**: Minimizes operational complexity. Agents invoke Braid as a CLI tool or link it as a library. The VPS-local deployment model means all agents share a filesystem, making a database server unnecessary.
**Rejected**: Client-server database (unnecessary infrastructure at target scale); distributed database (overkill for single-VPS deployment).
**Source**: Transcript 01 (embedded SQLite-style deployment)
**Formalized as**: ADR-STORE-006 in `spec/01-store.md`

### FD-011: Rust as Implementation Language

**Decision**: Braid is implemented in Rust. The query engine targets a purpose-built Rust binary as the final form.
**Rationale**: Safety guarantees (ownership, lifetimes), performance (zero-cost abstractions for index operations), and ecosystem support for append-only file structures (redb, LMDB bindings). The user explicitly confirmed "I want the option a) approach" (Rust binary) for the query engine.
**Rejected**: Go (current CLI language — but substrate divergence is fundamental per LM-001); Python (performance insufficient for index operations at scale).
**Source**: Transcript 01 (Rust implementation); Transcript 04:2397 (user confirms Rust binary target)

### FD-012: Every DDIS Command Is a Store Transaction

**Decision**: Every DDIS command becomes a transaction against the datom store. The bilateral loop (discover → refine → crystallize → datoms; scan → absorb → drift → datoms) maps entirely to store operations.
**Rationale**: If any DDIS operation produces state outside the store, that state cannot be queried, conflict-detected, or coherence-verified. The store is the sole truth.
**Source**: Transcript 01:866–924 (architecture diagram, command-to-transaction mapping)
**Formalized as**: ADR-STORE-011 in `spec/01-store.md`

### FD-013: BLAKE3 for Content Hashing

**Decision**: BLAKE3 is the hash algorithm for all content-addressed identity in Braid (EntityId generation, datom identity, genesis hash). 256-bit output (`[u8; 32]`).
**Rationale**: BLAKE3 is ~14x faster than SHA-256 on modern hardware, ships as a pure Rust crate (`blake3`) with no C dependency for optimal performance, produces the same 256-bit collision resistance (2^{-128} birthday bound), and was designed specifically for content-addressed systems (used by IPFS, Bao, and other content-addressed projects). Performance matters for Braid because every datom insertion, every entity lookup, and every merge deduplication involves hashing.
**Rejected**: SHA-256 (slower, requires C dependency via `ring` or `openssl` for optimal performance); BLAKE2b (predecessor to BLAKE3, less optimized tree structure, no SIMD auto-detection); xxHash (not cryptographic — collision resistance is insufficient for content-addressed identity where collisions cause silent data corruption).
**Source**: SEED.md §4 (content-addressed identity, Axiom 1); Transcript 01 (BLAKE3 discussed for content hashing); FD-007 (content-addressable identity)
**Formalized as**: ADR-STORE-013 in `spec/01-store.md`

### FD-014: Private EntityId Inner Field

**Decision**: EntityId's inner `[u8; 32]` field is private. Construction only via `EntityId::from_content()`. Read access via `as_bytes()`.
**Rationale**: Content-addressable identity (C2) means EntityIds MUST be derived from content. A public constructor from raw bytes would bypass content addressing, allowing fabricated IDs that break set-union deduplication (INV-STORE-002).
**Rejected**: Public inner field (simpler pattern matching, but bypasses C2 at construction time).
**Source**: SEED §4 Axiom 1, C2
**Formalized as**: ADR-STORE-014 in `spec/01-store.md`

---

## Algebraic Structure Decisions

Formal algebraic properties of the datom store and its extensions.

### AS-001: G-Set CvRDT as Store Algebra

**Decision**: The datom store is formally a G-Set (grow-only set) CvRDT. The algebraic structure is `(P(D), ∪)` — power set of datoms under set union. Merge is commutative, associative, and idempotent by construction.
**Rationale**: G-Set is the simplest CRDT that provides strong eventual consistency. Set union requires no coordination. Content-addressable identity (FD-007) ensures the same fact has the same identity regardless of which agent asserted it.
**Rejected**: More complex CRDTs (OR-Set, LWW-Register at store level — unnecessary when per-attribute resolution handles conflicts); custom merge logic (breaks formal guarantees).
**Source**: Transcript 01 (algebraic foundations); SEED.md §4 Axiom 3
**Formalized as**: ADR-STORE-001 in `spec/01-store.md`

### AS-002: Commitment Weight Function w(d)

**Decision**: Each design decision `d` carries a commitment weight `w(d)` equal to the size of its forward causal cone — the count of downstream decisions that depend on it. `w(d) = |{d' ∈ S : d ∈ causes*(d')}|`. Commitment is continuous, not binary.
**Rationale**: Overturning a high-weight decision cascades more changes. The commitment function provides a continuous model replacing the binary provisional/irrevocable distinction. Three emergent states: provisional (w small, few dependents), load-bearing (w large, many dependents), effectively irrevocable (forward cone too large to retract). Transition is emergent from the causal graph — no explicit state change required. Monotonic by construction: w(d) can only increase as the store grows.
**Rejected**: Uniform weighting (ignores cascading cost of revision); subjective importance ranking (not computable from the graph); binary provisional/irrevocable (too coarse).
**Source**: Transcript 01:676–716 (commitment function, continuous model); Transcript 02 (crystallization stability)

### AS-003: Branching G-Set Extension

**Decision**: The pure G-Set is extended to a Branching G-Set `(S, B, ⊑, commit, combine)` where S is the trunk (shared store, a G-Set), B is a set of branches (each a G-Set over D), `⊑` is the ancestry relation, `commit` merges a branch into trunk, and `combine` merges two branches.
**Rationale**: Agents need isolated workspaces for competing implementations. The branching extension preserves trunk monotonicity while giving agents snapshot-isolated workspaces with full datom semantics.
**Properties**: (1) Monotonicity: `commit(b,S) ⊇ S`; (2) Branch isolation: branches cannot see each other; (3) Combination commutativity; (4) Commit-combine equivalence; (5) Fork snapshot: `b.base = S|_{frontier(t)}`.
**Rejected**: Pure G-Set only (no isolated workspaces); mutable branching (violates FD-001).
**Source**: Transcript 04:509–553 (formal properties)
**Formalized as**: ADR-MERGE-002 in `spec/07-merge.md`

### AS-004: Branch Visibility Formula

**Decision**: A query against branch `b` sees exactly: `visible(b) = {d ∈ trunk | d.tx ≤ b.base-tx} ∪ {d | d.tx.branch = b}`. Trunk commits after the fork point are NOT visible unless the branch rebases.
**Rationale**: Provides snapshot isolation. Formally consistent with fork snapshot property (AS-003 Property 5). Without this, branches would need coordination on which trunk commits to include.
**Rejected**: Branches see all trunk (destroys isolation); branches see no trunk (loses pre-existing knowledge).
**Source**: Transcript 04:592–603

### AS-005: Branch as First-Class Entity

**Decision**: Branches are entities in the datom store with schema: `:branch/ident`, `:branch/base-tx`, `:branch/agent`, `:branch/status` (lattice: `:active < :proposed < :committed < :abandoned`), `:branch/purpose`, `:branch/competing-with`.
**Rationale**: Branch metadata (who, why, competing with what) must be queryable via Datalog. The `:branch/competing-with` attribute prevents first-to-commit from winning by default.
**Invariant**: `INV-BRANCH-COMPETE-001`: competing branches MUST NOT commit until comparison or deliberation has occurred.
**Source**: Transcript 04:553–597

### AS-006: Bilateral Branch Duality

**Decision**: The diverge-compare-converge (DCC) pattern works identically in both directions: forward flow (spec → competing implementations → selection) and backward flow (implementation → competing spec updates → selection). Same algebraic structure, same comparison machinery.
**Rationale**: The bilateral principle is central to DDIS. Separate mechanisms for forward and backward flow would duplicate implementation and violate reconciliation taxonomy symmetry.
**Invariant**: `INV-BRANCH-SYMMETRY-001`: violation if system supports branching for implementation but requires linear spec modifications.
**Source**: Transcript 04:605–653

### AS-007: Hebbian Significance via Separate Access Log

**Decision**: Datom significance is computed from a separate access log: `significance(d) = Σ decay(now - t) × query_weight(q)` over all queries that returned `d`. The access log is separate from the main store to avoid unbounded positive feedback.
**Rationale**: Neural analogy: connections strengthened by repeated access. Significance feeds into ASSOCIATE (high-significance attributes surface first) and ASSEMBLE (significance is a selection criterion). Storing access events as datoms in the main store would create infinite loops.
**Formal**: Default assembly weights: α=0.5 (relevance), β=0.3 (significance), γ=0.2 (recency).
**Invariant**: `INV-QUERY-SIGNIFICANCE-001`: every query MUST generate an access event in the access log, NOT in the main store.
**Source**: Transcript 04:659–705

### AS-008: Projection Reification as Learning Mechanism

**Decision**: Projection patterns (entity sets + query combinations) are reified as first-class entities when their access-count exceeds a threshold (default: 3 accesses). Reified projections carry significance scores and are discoverable via ASSOCIATE.
**Rationale**: The system learns useful ways to look at data. An agent discovering a useful query propagates it to other agents via the store. Projections develop a shared vocabulary of "good ways to look at things."
**Invariant**: `INV-PROJECTION-LEARNING-001`: projection pattern MUST be stored when access-count exceeds reification threshold.
**Source**: Transcript 04:709–741

### AS-009: Diamond Lattice as Contradiction Signal

**Decision**: Several lattices (challenge-verdict, finding-lifecycle, proposal-lifecycle) use a diamond structure where two incomparable top elements join to produce an error/attention signal. E.g., challenge-verdict: `:confirmed` and `:refuted` are incomparable; their join is `:contradicted`. The CRDT merge of concurrent incomparable values produces a first-class error signal.
**Rationale**: Lattice structure is not just for conflict resolution — it is a signal-generation mechanism. The diamond pattern where incomparable values join to produce a "contradiction" or "contested" state connects the lattice algebra directly to the coordination layer's uncertainty detection.
**Source**: Transcript 02:628–651 (challenge-verdict diamond), 02:712–720 (pattern repeated across three lattices)

### AS-010: Branch Comparison Entity Type

**Decision**: Branch Comparisons are entities with schema: `:comparison/branches` (ref :many), `:comparison/criterion`, `:comparison/method` (`:automated-test | :fitness-score | :agent-review | :human-review`), `:comparison/scores` (json), `:comparison/winner` (ref), `:comparison/rationale`, `:comparison/agent`.
**Rationale**: Comparison outcomes need structured storage for the competing-branch workflow. Without it, the INV-BRANCH-COMPETE-001 enforcement has no place to record why a branch was selected.
**Source**: Transcript 04:570–580

---

## Store & Runtime Architecture Decisions

Deployment model, storage implementation, schema layers, and index design.

### SR-001: Four Core Index Sort Orders

**Decision**: The store maintains four core indexes (following Datomic): EAVT, AEVT, VAET, AVET. Each is a different sort order over the same datom set.
**Rationale**: Different query patterns require different access paths. EAVT for entity lookup, AEVT for attribute-centric queries, VAET for reverse reference traversal, AVET for value-range scans. Datomic's index design is proven at scale.
**Rejected**: Single index with secondary lookups (poor query performance); ad-hoc indexes per query (unpredictable performance, index explosion).
**Source**: Transcript 01 (four index sort orders); Transcript 04:2197–2235
**Formalized as**: ADR-STORE-005 in `spec/01-store.md`

### SR-002: LIVE Materialized Index as Fifth Index

**Decision**: A fifth index, LIVE, materializes the "current state" view — the set of all non-retracted datoms at the latest frontier. LIVE is incrementally maintained, not recomputed.
**Rationale**: The most common query pattern is "what is the current state of entity X?" Without LIVE, this requires scanning all datoms for entity X and applying retractions. The LIVE index answers this in O(1) per entity.
**Rejected**: (A) Accept stratified negation (leaves negation in every basic query); (B) Incremental materialized view — **selected** (cleanest separation: materialization handles non-monotonicity, all Datalog queries run over LIVE without negation, fully CALM-compliant); (C) Imperative fold (function, not a query, loses declarative benefits).
**Invariant**: `INV-LIVE-001`: The LIVE index MUST be the deterministic result of applying all assert and retract datoms in causal order with the declared resolution mode per attribute: `LIVE(S) = fold(causal-sort(S), apply-resolution)`. LWW = greatest HLC assertion; lattice = join over unretracted assertions; multi = set of all unretracted values. Falsification: LIVE shows a value whose retraction has no subsequent re-assertion.
**Source**: Transcript 02 (LIVE index); Transcript 03:100–160 (Option A/B/C analysis, INV-LIVE-001)

### SR-003: LMDB/redb for MVCC Storage Semantics

**Decision**: The storage layer uses LMDB or redb (Rust-native) for persistent storage with MVCC (multi-version concurrency control) semantics.
**Rationale**: MVCC enables concurrent readers without blocking writers. The append-only datom model maps naturally to MVCC — new datoms are new versions, never overwrites. redb is the preferred Rust-native option.
**Rejected**: Direct file I/O without MVCC (reader-writer conflicts); SQLite (viable intermediate step per SR-005 but not the target architecture).
**Source**: Transcript 01 (LMDB/redb MVCC)

### SR-004: HLC (Hybrid Logical Clocks) for Transaction IDs

**Decision**: Transaction IDs use Hybrid Logical Clocks combining physical wall-clock time with a logical counter.
**Rationale**: HLC preserves temporal ordering (critical for time-travel queries) while maintaining uniqueness across agents without centralized coordination. Purely logical clocks lose temporal information; purely physical clocks have clock skew issues.
**Rejected**: Sequential integers (require centralized counter, conflict across agents); UUIDs (no temporal ordering); pure Lamport clocks (no wall-clock correlation).
**Source**: Transcript 01 (HLC for tx IDs)
**Formalized as**: ADR-STORE-004 in `spec/01-store.md`

### SR-005: Query Engine Implementation Path — Shell → SQLite → Rust Binary

**Decision**: Three-phase implementation: (a) shell tools (grep/jq + Python) for bootstrap, (b) SQLite with EAV schema and Datalog-to-SQL compilation as intermediate, (c) purpose-built Rust binary as final target. User confirmed Rust binary target.
**Rationale**: Bootstrapping problem — you need the system to build the system. Shell tools enable immediate use. The three implementations are substitutable (same protocol interface, tested against same invariants).
**Rejected**: Starting directly with Rust (bootstrapping problem); staying with shell permanently (won't scale).
**Source**: Transcript 04:2293–2314, 04:2397 (user confirms Rust target)
**Formalized as**: ADR-STORE-012 in `spec/01-store.md`

### SR-006: File-Backed Store with Git as Temporal Index

**Decision**: Store is implemented as append-only files: `trunk.ednl`, `branches/{name}.ednl`, `access.log`, `frontier.json`, `indexes/` (gitignored). The store directory is in git — every trunk TRANSACT is a git commit.
**Rationale**: For single-VPS with single-digit agents and thousands of datoms, file-backed with in-memory indexes is sufficient. Git integration provides audit history and time-travel without implementing a separate temporal layer.
**Rejected**: Database server (unnecessary infrastructure); in-memory only (not durable, violates PD-003).
**Source**: Transcript 04:2197–2235
**Formalized as**: ADR-STORE-007 in `spec/01-store.md`

### SR-007: Multi-Agent Coordination via Shared Filesystem

**Decision**: Multiple agents coordinate through the shared filesystem. Each agent writes to its branch file; all read from trunk. File-locking (flock) handles concurrent writes. Append-only structure makes concurrent trunk appends filesystem-safe.
**Rationale**: For co-located VPS agents, the filesystem is a natural coordination mechanism. The datom store IS the communication channel — no separate IPC protocol needed.
**Rejected**: Direct inter-process communication (additional infrastructure; loses store-as-sole-truth property).
**Source**: Transcript 04:2316–2343

### SR-008: Axiomatic Meta-Schema — 17 Bootstrap Attributes

**Decision**: The meta-schema consists of exactly 17 axiomatic attributes hardcoded in the engine (not defined by datoms): `:db/ident`, `:db/valueType`, `:db/cardinality`, `:db/doc`, `:db/unique`, `:db/isComponent`, `:db/resolutionMode`, `:db/latticeOrder`, `:db/lwwClock`, plus lattice definition attributes (`:lattice/ident`, `:lattice/elements`, `:lattice/comparator`, `:lattice/bottom`, `:lattice/top`). Value types include non-standard `:db.type/json` and `:db.type/tuple`. Three LWW clock options: `:hlc`, `:wall`, `:agent-rank`.
**Rationale**: The meta-schema is the self-describing foundation — the only attributes not defined by datoms. Everything else in the store is defined by datoms that reference these 17 attributes.
**Source**: Transcript 02:379–420
**Formalized as**: ADR-SCHEMA-002 in `spec/02-schema.md`

### SR-009: Six-Layer Schema Architecture

**Decision**: Schema organized into 6 layers: Layer 0 (Meta-schema, 17 axiomatic attributes), Layer 1 (Agent & Provenance, 2 types, 16 attributes, 1 lattice), Layer 2 (DDIS Core, 12 types, 72 attributes, 5 lattices), Layer 3 (Discovery & Exploration, 5 types, 28 attributes, 3 lattices), Layer 4 (Coordination & Uncertainty, 7 types, 35 attributes, 2 lattices), Layer 5 (Workflow & Task, 5 types, 27 attributes, 1 lattice). Total: 31 base entity types, ~195 attributes, 12 lattice definitions.
**Rationale**: The user chose "Approach 2: Full domain model" over minimal schema. The schema IS the ontology — it determines what the system can think about. The 6-layer structure enables incremental implementation.
**Note**: Protocol extensions (Transcript 04) add 15 more entity types, bringing total to ~46 types, ~300 attributes, ~16 lattices.
**Source**: Transcript 02:356–942 (user choice at 369, full schema)
**Formalized as**: ADR-SCHEMA-003 in `spec/02-schema.md`

### SR-010: Twelve Named Lattice Definitions

**Decision**: Exactly 12 lattices defined, several with non-trivial diamond structure: (1) agent-lifecycle, (2) confidence-level, (3) adr-lifecycle, (4) witness-lifecycle, (5) challenge-verdict (diamond: `:confirmed`/`:refuted` incomparable, join=`:contradicted`), (6) thread-lifecycle, (7) finding-lifecycle (diamond), (8) proposal-lifecycle (three-way incomparable, join=`:contested`), (9) delegation-level, (10) conflict-lifecycle, (11) task-lifecycle, (12) numeric-max.
**Rationale**: The diamond lattice structures are formally significant — concurrent incomparable values join to produce a coordination signal (see AS-009). This connects lattice algebra directly to uncertainty detection.
**Source**: Transcript 02:628–926 (lattice definitions throughout schema)
**Formalized as**: ADR-SCHEMA-004 in `spec/02-schema.md`

### SR-012: Owned Schema with Borrow API

**Decision**: Store owns a `Schema` field internally, derived from schema datoms on load. Exposed via `store.schema() -> &Schema` (zero-cost borrow). Schema is reconstructed after schema-modifying transactions.
**Rationale**: Avoids lifetime infection from Option A (borrow-based `Schema<'a>`), prevents divergence from Option B (copied data that can go stale). Maintains C3 because Schema is always derived from datoms — `Schema::from_store()` is the sole constructor.
**Rejected**: (A) Borrow-based `Schema<'a>` (lifetime-infectious in Rust); (B) Independent copy (can diverge from store after construction).
**Source**: C3, INV-SCHEMA-001
**Formalized as**: ADR-SCHEMA-005 in `spec/02-schema.md`

### SR-011: Session State File as Coordination Point

**Decision**: A session state file at `.ddis/session/context.json` serves as the coordination point between the Claude Code statusline hook and the CLI's budget system. Contains: `used_percentage`, `input_tokens`, `remaining_tokens`, `k_eff`, `quality_adjusted`, `output_budget`, `timestamp`, `session_id`.
**Rationale**: The statusline hook has direct access to Claude Code's context window data. Writing to a well-known file path allows the CLI to read budget data without needing MCP or transcript parsing.
**Invariant**: `INV-SESSION-STATE-001`: session state file must be updated on every statusline render cycle.
**Source**: Transcript 05:652–737

### SR-013: Free Functions Over Store Methods for Namespace Operations

**Decision**: Namespace operations (query, harvest, seed, merge, guidance) are free functions taking `&Store` rather than Store methods. Store methods are reserved for core datom operations: `genesis()`, `transact()`, `current()`, `as_of()`, `len()`, `datoms()`, `frontier()`, `schema()`.
**Rationale**: Store is a datom container, not an application framework. Free functions keep namespace logic independent of Store internals, prevent Store from becoming a god object, and enable testing with mock stores. Each namespace's free functions form a natural Rust module boundary.
**Source**: SEED.md §4, §10; ADRS FD-010, FD-012
**Formalized as**: ADR-STORE-015 in `spec/01-store.md`, ADR-ARCHITECTURE-001 in `guide/00-architecture.md`

---

## Protocol Decisions

These decisions govern the protocol layer above the axioms.

### PD-001: Agent Working Set (W_α) — Two-Tier Store

**Decision**: Each agent maintains a private working set W_α using the same datom structure as the shared store S. W_α is not merged during MERGE operations. Agents explicitly `commit` datoms from W_α to S (commit is a TRANSACT). Agents can query over W_α ∪ S locally.
**Rationale**: Preserves clean CRDT semantics on S (set union merge) while giving agents local scratchpad flexibility. W_α is agent-internal and opaque to the protocol.
**Formal**: `commit : W_α × S → S'` where `S' = S ∪ {d ∈ W_α | agent chose to commit d}`. Local queries: same engine over W_α ∪ S.
**Rejected**: (A) No private datoms — pure G-Set, agents self-censor (no scratchpad; ASSOCIATE returns noisy results from scratchwork). (C) Visibility as datom attribute (`:private < :team < :shared`) — breaks union semantics, creates dangling refs during merge.
**Source**: Transcript 04:210–240 (options), 04:373 (confirmed); Transcript 05:849–861
**User extension**: Patch branches — see AS-003–AS-006 for full formalization.

### PD-002: Provenance Typing Lattice

**Decision**: Each transaction carries `:tx/provenance-type` with values from a lattice: `:observed < :derived < :inferred < :hypothesized`. Agent declares provenance type; system can structurally audit the declaration.
**Rationale**: Different epistemic statuses require different trust levels in authority computation and conflict resolution. Self-authored associations are `:inferred`, not `:observed`. The lattice is auditable — a transaction labeled `:observed` without tool read operations is flagged as misclassified.
**Formal**: Provenance factors for authority computation: `:observed` = 1.0, `:derived` = 0.8, `:inferred` = 0.5, `:hypothesized` = 0.2. `contribution_weight(d) = base_weight × provenance_factor(d.tx.provenance)`.
**Rejected**: (A) No provenance typing — rely on challenge system post-hoc (slow convergence). (C) Structural inference only — imperfect classification.
**Source**: Transcript 04:244–310 (options, provenance factor weights), 04:373 (confirmed)
**Formalized as**: ADR-STORE-008 in `spec/01-store.md`

### PD-003: Crash-Recovery Model with Durable Frontiers

**Decision**: Agents follow a crash-recovery model. On restart, they rebuild local state from their last known frontier (stored durably). The protocol supports reconnection: a recovering agent announces its frontier and receives the delta.
**Rationale**: Crash-recovery is the realistic model for LLM agents. Byzantine tolerance is overkill for controlled environments.
**Invariant**: `INV-FRONTIER-DURABLE-001`: frontier MUST be durably stored after every TRANSACT and every MERGE.
**Rejected**: (A) Crash-stop (too rigid). (C) Byzantine model (overkill).
**Source**: Transcript 04:310–342, 04:373 (confirmed)
**Formalized as**: ADR-STORE-009 in `spec/01-store.md`

### PD-004: At-Least-Once Delivery with Idempotent Operations

**Decision**: Message delivery is at-least-once. All protocol operations are idempotent.
**Formal**: `merge(merge(S, R), R) = merge(S, R)` (idempotence). SYNC-BARRIER is the exception — requires stronger guarantees but is explicitly rare.
**Rejected**: Exactly-once (requires 2PC). At-most-once (risks data loss).
**Source**: Transcript 04:337–358, 04:373 (confirmed)
**Formalized as**: ADR-STORE-010 in `spec/01-store.md`

### PD-005: Protocol Is Topology-Agnostic

**Decision**: The protocol defines operations; topology emerges from how agents use them. Single-agent, bilateral, flat swarm, and hierarchy are all valid topologies using the same operations.
**Rationale**: Prescribing topology limits applicability. Topology-specific behavior is agents choosing SIGNAL targets and MERGE partners.
**Rejected**: Topology-prescribing protocol (locks to one coordination pattern).
**Source**: Transcript 04:114–127; Transcript 03
**Formalized as**: ADR-SYNC-002 in `spec/08-sync.md`

### PD-006: Bilateral Authority Principle

**Decision**: Authority flows bilaterally: (1) Forward — human initiates exploration; as findings stabilize, work flows outward to agents as uncertainty decreases. (2) Backward — agents surface divergence they cannot resolve; as contradiction severity increases, resolution flows inward toward agents/humans with broader context. (3) Fixpoint — the bilateral loop reaches fixpoint when forward and backward flows produce no further changes. (4) Authority is emergent from the contribution graph (UA-003), not structural. (5) Coordination topology emerges from spectral structure.
**Rationale**: This is the integrating principle connecting commitment weight, uncertainty tensor, spectral authority, and delegation thresholds into a coherent whole. Synthesized from the retracted "Delegation Inversion" (which captured only the backward flow) after user correction that it missed the forward flow.
**Source**: Transcript 01:1454–1488 (bilateral authority principle)

---

## Protocol Operations Decisions

Specifications for the canonical protocol operations.

### PO-001: TRANSACT — Seven-Field Type Signature

**Decision**: TRANSACT requires: `agent`, `branch` (None=trunk), `datoms`, `causal_parents` (set of TxIds), `provenance` (ProvenanceType), `rationale` (string), `operation` (keyword: `:op/observe | :op/infer | :op/deliberate | :op/crystallize | :op/resolve`).
**Invariants**: `INV-TX-APPEND-001` (S ⊆ S'), `INV-TX-CAUSAL-001` (causal parents must reference known tx), `INV-TX-BRANCH-001` (branch tx cannot affect trunk), `INV-TX-PROVENANCE-001` (provenance structurally consistent), `INV-TX-FRONTIER-DURABLE-001` (frontier stored before response).
**Source**: Transcript 04:1252–1342

### PO-002: ASSOCIATE — Two-Mode Schema Discovery

**Decision**: Two modes: `SemanticCue` (natural language) or `ExplicitSeeds` (entity IDs). Returns SchemaNeighborhood (entities, attributes, types — not values). Bounded by `depth × breadth`.
**Invariants**: `INV-ASSOCIATE-BOUND-001` (size ≤ depth × breadth), `INV-ASSOCIATE-SIGNIFICANCE-001` (high-significance preferred), `INV-ASSOCIATE-LEARNED-001` (learned associations traversed alongside structural edges).
**Source**: Transcript 04:1399–1451

### PO-003: ASSEMBLE — Rate-Distortion Context Construction

**Decision**: Takes query results + schema neighborhood + budget, produces assembled context using pyramid-level selection per entity. Priority: `score = α × relevance + β × significance + γ × recency` (defaults: 0.5, 0.3, 0.2).
**Invariants**: `INV-ASSEMBLE-BUDGET-001` (≤ budget), `INV-ASSEMBLE-PYRAMID-001` (structural dependency coherence), `INV-ASSEMBLE-INTENTION-001` (intentions pinned at level 0), `INV-ASSEMBLE-PROJECTION-001` (record projection), `INV-ASSEMBLE-FRESHNESS-001` (check staleness, apply freshness-mode).
**Source**: Transcript 04:1453–1522

### PO-004: SIGNAL — Coordination as Datoms

**Decision**: Every signal (Confusion, Conflict, UncertaintySpike, ResolutionProposal, DelegationRequest, GoalDrift, BranchReady, DeliberationTurn) is recorded as a datom.
**Invariant**: `INV-SIGNAL-DATOM-001`: signal history must be queryable.
**Source**: Transcript 04:1715–1769
**Formalized as**: ADR-SIGNAL-001 in `spec/09-signal.md`

### PO-005: Confusion Signal as First-Class Protocol Operation

**Decision**: `Confusion(cue)` with type (NeedMore, Contradictory, GoalUnclear, SchemaUnknown) triggers automatic re-ASSOCIATE + re-ASSEMBLE within one agent cycle — NOT a full round-trip.
**Invariant**: `INV-SIGNAL-CONFUSION-001`: confusion MUST trigger re-ASSOCIATE + re-ASSEMBLE within one cycle.
**Source**: Transcript 04:49–61, 04:1754–1762

### PO-006: MERGE — Epistemic Event with Cascade

**Decision**: MERGE propagates consequences: invalidated queries, new conflicts, uncertainty deltas, stale projections. All cascade effects recorded as datoms.
**Invariant**: `INV-MERGE-CASCADE-001`: after merge, MUST detect conflicts, invalidate caches, mark stale projections, recompute uncertainty, fire subscriptions.
**Source**: Transcript 04:64–78, 04:1642–1651
**Formalized as**: ADR-MERGE-001 in `spec/07-merge.md`

### PO-007: BRANCH — Six Sub-Operations with Competing-Branch Lock

**Decision**: Fork, Commit, Combine (strategies: Union, SelectiveUnion, ConflictToDeliberation), Rebase, Abandon, Compare (criteria: FitnessScore, TestSuite, UncertaintyReduction, AgentReview, Custom). Competing branches locked from commit until comparison/deliberation.
**Invariant**: `INV-BRANCH-COMPETE-001`, `INV-BRANCH-DELIBERATION-001` (ConflictToDeliberation opens deliberation).
**Source**: Transcript 04:1524–1591
**Formalized as**: ADR-MERGE-003 in `spec/07-merge.md`, ADR-MERGE-004 in `spec/07-merge.md`

### PO-008: SUBSCRIBE — Pattern-Based Push Notifications

**Decision**: Registers Datalog-like pattern filter with callback. Debounce parameter batches rapid-fire matches.
**Invariants**: `INV-SUBSCRIBE-COMPLETENESS-001` (fire for every match within refresh cycle), `INV-SUBSCRIBE-DEBOUNCE-001` (debounced notifications must batch within window).
**Source**: Transcript 04:1772–1814
**Formalized as**: ADR-SIGNAL-003 in `spec/09-signal.md`

### PO-009: GUIDANCE — Query over Guidance Graph

**Decision**: Queries available action topology by evaluating guidance nodes' state predicates. Returns actions + optional lookahead tree (1–5 steps). Includes system-default and learned guidance.
**Invariants**: `INV-GUIDANCE-ALIGNMENT-001` (actions scored higher if they advance active intentions: `if postconditions(a) ∩ goals(i) ≠ ∅: score(a) += intention_alignment_bonus`), `INV-GUIDANCE-LEARNED-001` (learned guidance ranked by effectiveness).
**Source**: Transcript 04:1816–1875

### PO-010: SYNC-BARRIER — Topology-Dependent Frontier Exchange

**Decision**: Topology-dependent (Option C): protocol provides primitives; deployment chooses topology. User confirmed "C for sure."
**Invariants**: `INV-BARRIER-TIMEOUT-001` (resolve within timeout), `INV-BARRIER-CRASH-RECOVERY-001` (recovering agents can query barrier record).
**Source**: Transcript 04:1960–1977
**Formalized as**: ADR-SYNC-001, ADR-SYNC-003 in `spec/08-sync.md`

### PO-011: Agent Cycle as Ten-Step Composition

**Decision**: (1) ASSOCIATE, (2) QUERY, (3) ASSEMBLE with guidance+intentions, (4) GUIDANCE lookahead=2, (5) agent policy evaluates, (6a) action → TRANSACT or (6b) confusion → re-ASSOCIATE/ASSEMBLE → retry, (7) learned association → TRANSACT(:inferred), (8) subtask → TRANSACT(intention update), (9) check incoming MERGE/signals, (10) repeat.
**Source**: Transcript 04:1880–1927

### PO-012: Genesis Transaction

**Decision**: Store begins with genesis transaction containing schema definitions. No causal parents. Root of the causal graph.
**Invariant**: `INV-GENESIS-001`: Transaction tx=0 MUST contain exactly the axiomatic meta-schema attributes and nothing else. All stores begin from identical genesis state. `∀ S1, S2: S1|_{tx=0} = S2|_{tx=0}`. Verified by constant hash of tx=0 datom set.
**Source**: Transcript 02:429–442

### PO-013: QUERY — Datalog Evaluation with Four Invariants

**Decision**: QUERY evaluates Datalog expressions against a specified frontier and branch. Four invariants: (1) `INV-QUERY-CALM-001`: monotonic-mode queries MUST NOT contain negation/aggregation; reject at parse time. (2) `INV-QUERY-BRANCH-001`: branch query visibility = `visible(b)`. (3) `INV-QUERY-SIGNIFICANCE-001`: every query generates access event in access log. (4) `INV-QUERY-DETERMINISM-001`: identical expressions at identical frontiers MUST return identical results.
**Source**: Transcript 04:1370–1397

### PO-014: GENERATE-CLAUDE-MD — Dynamic Instruction Generation

**Decision**: Formal operation with signature `(focus, agent, budget)`. Seven-step process: ASSOCIATE, QUERY active intentions, QUERY governing invariants, QUERY uncertainty, QUERY competing branches, QUERY drift patterns, QUERY guidance topology, ASSEMBLE at budget. Priority ordering: tools > task context > risks > drift corrections > seed context.
**Invariants**: `INV-CLAUDE-MD-RELEVANCE-001` (every section relevant to focus; falsified if removing a section wouldn't change behavior), `INV-CLAUDE-MD-IMPROVEMENT-001` (drift corrections derived from empirical data; corrections showing no effect after 5 sessions replaced).
**Source**: Transcript 06:147–207

---

## Snapshot & Query Decisions

### SQ-001: Local Frontier as Default Query Mode (Option 3C)

**Decision**: Default query mode is local frontier. Consistent cuts via optional sync barriers for non-monotonic queries.
**Rejected**: (A) Local frontier only. (B) Consistent cut only (too expensive for monotonic queries).
**Source**: Transcript 01:518–542, 01:645
**Formalized as**: ADR-QUERY-005 in `spec/03-query.md`

### SQ-002: Frontier as Datom Attribute

**Decision**: Frontier stored as `:tx/frontier`. Concrete type: `Frontier = Map<AgentId, TxId>` (vector-clock equivalent).
**Source**: Transcript 02:471–479; Transcript 04:1190–1243
**Formalized as**: ADR-QUERY-006 in `spec/03-query.md`

### SQ-003: Datalog Frontier Query Extension

**Decision**: Datalog extended with `[:frontier ?frontier-ref]` clause.
**Source**: Transcript 02:1004–1012

### SQ-004: Stratum Safety Classification

**Decision**: Strata 0–1 (monotonic) and Stratum 4 (conservatively monotonic) safe without coordination. Strata 2–3 (mixed/FFI) require frontier-specific evaluation (Stratified mode). Stratum 5 (non-monotonic bilateral loop) requires sync barriers (Barriered mode). `QueryMode = Monotonic | Stratified Frontier | Barriered BarrierId`.
**Source**: Transcript 02:2047; Transcript 04:1190–1243
**Formalized as**: ADR-QUERY-003 in `spec/03-query.md`

### SQ-005: Topology-Agnostic Resolution Invariant

**Decision**: Query results must be identical regardless of dissemination topology.
**Source**: Transcript 02

### SQ-006: Bilateral Query Layer Structure

**Decision**: Query layer is bilateral. Forward queries (spec → impl status) and backward queries (impl → spec alignment) use the same Datalog apparatus.
**Formal**: Queries naturally partition into: Forward-flow (planning: epistemic uncertainty, crystallization candidates, delegation, ready tasks), Backward-flow (assessment: conflict detection, drift candidates, aleatory uncertainty, absorption triggers), Bridge (both: commitment weight, consequential uncertainty, spectral authority). Spectral authority is the explicit bridge — updated by backward-flow observations, consumed by forward-flow decisions.
**Source**: Transcript 02; Transcript 03:1084–1094
**Formalized as**: ADR-QUERY-008 in `spec/03-query.md`

### SQ-007: Projection Pyramid — Level-Based Summarization

**Decision**: Pyramid `{π₀, π₁, π₂, π₃}`: π₀ = full datoms, π₁ = entity summaries, π₂ = type summaries, π₃ = store summary.
**Budget-driven selection**: >2000 tokens = π₀ for top/π₁ for others; 500–2000 = π₁/π₂; 200–500 = π₂ for top/omit others; ≤200 = single-line status + single guidance action.
**Invariant**: `INV-ASSEMBLE-PYRAMID-001`: structural dependency coherence.
**Source**: Transcript 04:966–1021; Transcript 05:1008–1019 (budget thresholds)
**Formalized as**: ADR-QUERY-007 in `spec/03-query.md`

### SQ-008: Complete Protocol Type Definitions

**Decision**: `Value = String | Keyword | Boolean | Long | Double | Instant | UUID | Ref EntityId | Bytes | URI | BigInt | BigDec | Tuple [Value] | Json String`; `Level = 0 | 1 | 2 | 3`; full Signal sum type.
**Source**: Transcript 04:1190–1243

### SQ-009: Six-Stratum Query Classification

**Decision**: Six strata with 17 named query patterns: Stratum 0 (primitive, monotonic — current-value over LIVE), Stratum 1 (graph traversal, monotonic — causal-ancestor, depends-on, cross-ref reachability), Stratum 2 (uncertainty, mixed — epistemic/aleatory/consequential), Stratum 3 (authority, not pure Datalog — linear algebra: SVD, delegation threshold), Stratum 4 (conflict detection, conservatively monotonic — detect-conflicts, route-conflict), Stratum 5 (bilateral loop, non-monotonic — spec-fitness, crystallization-candidates, drift-candidates).
**Rationale**: Systematic safety analysis. Strata 0–3 safe at any frontier. Strata 4–5 benefit from sync barriers for correctness-critical decisions.
**Source**: Transcript 03:1052–1081
**Formalized as**: ADR-QUERY-003 in `spec/03-query.md`

### SQ-011: Full Graph Engine in Kernel

**Decision**: Graph algorithms (PageRank, betweenness, critical path, SCC, k-core, etc.) are first-class kernel query operations alongside Datalog, with results stored as datoms.
**Rationale**: Graph algorithms are the foundation of task derivation (INV-GUIDANCE-009), work routing (INV-GUIDANCE-010), and topology fitness (INV-GUIDANCE-011). Externalizing them would break CRDT merge — graph results must be datoms to merge across agents. Graph metrics over the datom reference graph are monotonic (CALM-compliant).
**Rejected**: (A) External tools (breaks store-as-sole-truth); (B) FFI derived functions (forces unnatural Datalog encoding of results).
**Source**: ADRS SQ-004, FD-003
**Formalized as**: ADR-QUERY-009 in `spec/03-query.md`

### SQ-010: Datalog/Imperative Boundary for Derived Functions

**Decision**: Three core computations CANNOT be expressed in pure Datalog: σ_a (requires entropy — grouping, division, logarithm), σ_c (requires bottom-up DAG traversal with memoization), spectral authority (requires linear algebra — SVD). These are DERIVED FUNCTIONS: Datalog provides the input query, a Rust function computes the result. σ_e uses count-distinct aggregation (borderline). The query engine must support a foreign-function interface for derived computations.
**Rationale**: Establishes the boundary between declarative queries and imperative computation. Major architectural implication: three of four core coordination computations are derived functions.
**Source**: Transcript 02:1318–1321 (σ_a), 02:1391 (σ_c), 02:1475–1476 (authority); Transcript 03:346–392, 03:422–466
**Formalized as**: ADR-QUERY-004 in `spec/03-query.md`

---

## Uncertainty & Authority Decisions

### UA-001: Three-Dimensional Uncertainty Tensor

**Decision**: Uncertainty `σ = (σ_e, σ_a, σ_c)`: epistemic (reducible by observation), aleatory (inherent randomness — Shannon entropy), consequential (downstream risk — DAG traversal).
**Formal**: Scalar combination: `scalar = √(α·σ_e² + β·σ_a² + γ·σ_c²)`. Default weights: α=0.4 (epistemic), β=0.4 (aleatory), γ=0.2 (consequential). Weights stored as datoms, configurable per deployment.
**Rationale**: σ_e and σ_a weighted equally (both actionable). σ_c weighted lower (structural, depends on graph topology which changes slowly). Overweighting σ_c causes excessive caution about well-understood heavily-depended entities.
**Source**: Transcript 01; Transcript 02 (complete formalization, default weights at 1456–1469); Transcript 03:473–501

### UA-002: Epistemic Uncertainty Temporal Decay

**Decision**: Epistemic uncertainty increases over time. Exponential form with per-namespace lambda calibration: `age_factor(e) = 1 - e^{-λ × time_since_last_validation(e)}`. Code observations decay fast; architectural decisions decay slowly; invariants do not decay (normative, not descriptive).
**Source**: Transcript 02; Transcript 07:263–275 (exponential form, per-namespace lambda)

### UA-003: Spectral Authority via SVD

**Decision**: Authority computed via SVD of bipartite agent-entity contribution matrix. Captures TRANSITIVE authority — if agent α contributed to entities A, B, C related to D, α has authority over D even without directly touching D. Mathematically identical to LSI search applied to agent-entity matrix. Truncated SVD with k = min(50, agent_count, entity_count).
**Rationale**: Self-reported authority is unreliable. Raw contribution counting misses transitive authority. SVD projects agents and entities into shared latent space where proximity = structural similarity. "LSI finds relevant documents; spectral authority finds authoritative agents."
**Invariant**: `INV-AUTHORITY-001`: Agent authority MUST be derived from weighted spectral decomposition of contribution graph. MUST NOT be assigned by configuration. Exception: human authority is axiomatically unbounded. Falsification: authority granted by configuration rather than contribution.
**Source**: Transcript 01:1432–1448; Transcript 02:1523–1577; Transcript 03:604–608

### UA-004: Delegation Threshold Formula

**Decision**: `threshold = 0.3×betweenness + 0.2×in_degree + 0.3×σ_c + 0.2×conflict_surface`, where conflict_surface = fraction of entity's cardinality-one attributes. Delegation classification: `delegatable` (resolvers > 0 AND uncertainty < 0.2), `contested` (resolvers > 0 AND uncertainty ≥ 0.2), `escalated` (resolvers = 0 AND uncertainty < 0.5), `human-required` (resolvers = 0 AND uncertainty ≥ 0.5). Thresholds configurable as datoms.
**Source**: Transcript 03:610–689

### UA-005: Four-Class Delegation

**Decision**: Tasks classified: self-handle, consult, delegate, escalate to human.
**Source**: Transcript 03

### UA-006: Uncertainty Markers as First-Class Spec Elements

**Decision**: Specification uncertainty marked explicitly with confidence levels (0.0–1.0) and resolution criteria.
**Source**: Transcript 07; SEED.md §3

### UA-007: Observation Staleness Model

**Decision**: Observation datoms carry metadata: `:observation/entity` (ref), `:observation/source` (keyword: `:filesystem | :shell | :network | :git | :process`), `:observation/path` (string), `:observation/timestamp`, `:observation/hash`, `:observation/stale-after`. ASSEMBLE applies freshness-mode (`:warn` default, `:refresh`, `:accept`).
**Source**: Transcript 04:2155–2188

### UA-008: Self-Referential Measurement Exclusion (INV-MEASURE-001)

**Decision**: When computing σ_c for entity e, MUST exclude uncertainty measurements targeting e itself from the dependent set. Without this exclusion, the function diverges in self-referential loops. Revised from initial unconditional claim (which was retracted after analysis).
**Formal**: `σ_c(e)` computed over `dependents(e) \ {measurements of e}`.
**Rationale**: Self-referential feedback loops cause oscillation. The initial claim "measurement is always contractive" was self-corrected to a conditional version requiring exclusion.
**Source**: Transcript 02:819–858 (self-correction); Transcript 03:450–469

### UA-009: Query Stability Score (INV-COMMIT-001)

**Decision**: `stability(R) = min{w(d) : d ∈ F and d contributed to R}` for query result R from facts F. A result with stability ≥ threshold is safe for irrevocable decisions without sync barrier.
**Rationale**: Distinct from crystallization stability (CR-005) — this measures safety of acting on any query result, not just promoting datoms to stable spec.
**Falsification**: Agent makes irrevocable decision based on query result with stability = 0.
**Source**: Transcript 01:704–714

### UA-010: Contribution Weight by Verification Status

**Decision**: Contributions to the authority graph weighted: 1 (unverified), 2 (witnessed/valid), 3 (challenge-confirmed). Feeds into spectral authority computation (UA-003).
**Rationale**: Creates feedback loop: verified work → more authority → more delegation → more work.
**Source**: Transcript 02:1494–1519

### UA-011: Delegation Safety Invariant (INV-DELEGATE-001)

**Decision**: Agent MUST NOT begin work on spec element e unless `delegatable(e) = true` at agent's local frontier. `delegatable(e) = ∀ a ∈ attributes(e): no conflict, AND stability(e) ≥ delegation_threshold`.
**Falsification**: Agent begins implementing a function whose signature is contested by concurrent planning agents.
**Source**: Transcript 01:1036–1062

### UA-012: Resolution Capacity Monotonicity (INV-RESOLUTION-001)

**Decision**: When `uncertainty(e)` increases, the set of agents with authority to resolve conflicts on e MUST NOT shrink. `∀ t1 < t2: uncertainty(e,t1) < uncertainty(e,t2) ⟹ resolvers(e,t2) ⊇ resolvers(e,t1)`. Topology-agnostic: in hierarchy, higher-level agents added; in swarm, quorum increases; in market, reputation threshold decreases.
**Rationale**: Revised from retracted INV-CASCADE-001 (which mandated hierarchical escalation). The revision is topology-agnostic per PD-005.
**Source**: Transcript 01:1096–1113 (retracted), 01:1196–1215 (revised)

---

## Conflict & Resolution Decisions

### CR-001: Conservative Conflict Detection

**Decision**: Conservative — flags potential conflicts even when uncertain. `INV-CONFLICT-CONSERVATIVE-001`: detected conflicts at any local frontier MUST be a superset of conflicts at the global frontier. `conflicts(frontier_local) ⊇ conflicts(frontier_global)`.
**Proof sketch**: Causal-ancestor relation is monotonically growing. Learning about new causal paths can only resolve apparent concurrency, never create it. Agent may waste effort on phantom conflicts (safe) but never miss a real one (critical).
**Source**: Transcript 02; Transcript 03:742–761
**Formalized as**: ADR-RESOLUTION-003 in `spec/04-resolution.md`

### CR-002: Three-Tier Conflict Routing

**Decision**: (1) Automatic (low severity — lattice/LWW per attribute), (2) Agent-with-notification (medium), (3) Human-required (high — blocks). Severity = `max(w(d₁), w(d₂))`.
**Source**: Transcript 02; Transcript 04:1331–1342
**Formalized as**: ADR-RESOLUTION-004 in `spec/04-resolution.md`

### CR-003: Conflict Detection and Routing as Datom Cascade

**Decision**: System: (1) asserts Conflict entity, (2) computes severity, (3) routes, (4) fires TUI, (5) updates uncertainty, (6) invalidates caches. ALL steps produce datoms.
**Source**: Transcript 04:1331–1342
**Formalized as**: ADR-SIGNAL-002 in `spec/09-signal.md`

### CR-004: Deliberation, Position, and Decision as First-Class Entity Types

**Decision**: Deliberation (process), Position (stance: `:advocate | :oppose | :neutral | :synthesize`), Decision (method: `:consensus | :majority | :authority | :human-override | :automated`). Deliberation history is a case law system.
**Invariant**: `INV-DELIBERATION-BILATERAL-001`: supports both forward and backward flow with identical entity structure.
**Source**: Transcript 04:745–828
**Formalized as**: ADR-DELIBERATION-001, ADR-DELIBERATION-002 in `spec/11-deliberation.md`; ADR-RESOLUTION-005 in `spec/04-resolution.md`

### CR-005: Crystallization Stability Guard

**Decision**: Datom carries `:stability-min` guard. Cannot crystallize until stability score exceeds threshold (default 0.7). Conditions: status `:refined`, thread `:active`, parent confidence ≥ 0.6, coherence ≥ 0.6, no unresolved conflicts. Defense against premature crystallization: `:stability-min` as Datalog pre-filter.
**Source**: Transcript 02; Transcript 03:942–990
**Formalized as**: ADR-DELIBERATION-004 in `spec/11-deliberation.md`

### CR-006: Formal Conflict Predicate

**Decision**: `conflict(d1, d2) = d1 = [e a v1 t1 assert] ∧ d2 = [e a v2 t2 assert] ∧ v1 ≠ v2 ∧ cardinality(a) = :one ∧ ¬(t1 < t2) ∧ ¬(t2 < t1)`. Critical: conflict requires causal independence — if one tx precedes the other, it is an update, not a conflict.
**Source**: Transcript 01:998–1011

### CR-008: Resolution at Query Time, Not Merge Time

**Decision**: MERGE is pure set union; conflict resolution happens at query time in the LIVE index, not during MERGE.
**Rationale**: If MERGE resolves conflicts, then `MERGE(S1, S2)` depends on schema — but schema is itself data in the store. This creates a circular dependency that breaks the CRDT algebraic properties (L1–L3 assume set union). Resolution at query time avoids this: MERGE is always set union, and LIVE applies resolution modes.
**Rejected**: (A) Resolution at merge time (breaks C4, creates schema dependency in MERGE).
**Source**: C4, ADRS AS-001
**Formalized as**: ADR-RESOLUTION-002 in `spec/04-resolution.md`

### CR-009: BLAKE3 Hash Tie-Breaking for LWW Conflicts

**Decision**: When two concurrent LWW assertions have identical HLC timestamps, break ties by lexicographic comparison of `blake3([e, a, v, tx, op])` hash. The datom with the greater hash wins.
**Rationale**: Without a deterministic tie-breaker, two agents resolving the same conflict could pick different winners, violating INV-RESOLUTION-002 (commutativity). BLAKE3 hash comparison provides a total order that preserves commutativity, associativity, and idempotency. The hash is already computed for EntityId generation (ADR-STORE-013 / FD-013), so marginal cost is zero.
**Rejected**: (B) Agent ID comparison (creates implicit agent hierarchy, incentivizes ID manipulation); (C) Leave undefined (breaks CRDT convergence).
**Source**: INV-RESOLUTION-005, ADR-STORE-013
**Formalized as**: ADR-RESOLUTION-009 in `spec/04-resolution.md`

### CR-007: Precedent Query Pattern for Deliberations

**Decision**: Concrete Datalog query pattern `find-precedent` locates past deliberations relevant to a current conflict by matching entity type and contested attributes.
**Rationale**: Makes deliberation history a "case law system" — past decisions inform future conflicts.
**Source**: Transcript 04:798–828
**Formalized as**: ADR-DELIBERATION-003 in `spec/11-deliberation.md`

---

## Agent Architecture Decisions

### AA-001: Dual-Process Architecture Is Protocol-Level

**Decision**: System 1 (associative retrieval) + System 2 (LLM reasoning) is a protocol-level requirement. The two-phase retrieval (ASSOCIATE → QUERY → ASSEMBLE) is first-class.
**Formal**: `assemble ∘ query ∘ associate : SemanticCue → BudgetedContext`.
**Rejected**: Context assembly as application-level (flat-buffer pathology).
**Source**: Transcript 03; Transcript 04:31–46

### AA-002: Revised Agent System Formalism — D-Centric

**Decision**: `(D, Op_D, Obs_D, A, π, Σ, Γ)` — all operations reference the datom store D. POSIX runtime R is below protocol boundary.
**Source**: Transcript 04:461–476

### AA-003: Store-as-Protocol-Runtime — Three-Layer Architecture

**Decision**: (1) Protocol Layer — all coordination as datoms, (2) Observation Interface — tools translating R-state to datoms, (3) Execution Layer — POSIX runtime, opaque.
**Formal**: Observation functor `observe : R-State → [Datom]` with three properties: (1) Idempotent — same R-state produces equivalent datoms modulo tx-id; (2) Monotonic — only ADDs datoms (file deletion = retraction datom, not deletion); (3) Lossy — selective observation per information-value criterion. The strong-form invariant `INV-STORE-AS-RUNTIME-001` was revised to the weaker eventually-consistent `INV-STORE-AS-RUNTIME-002` in the three-layer architecture.
**Source**: Transcript 04:2086–2189

### AA-004: Metacognitive Layer — Four Entity Types

**Decision**: Belief, Intention, Learned Association (type: `:causal | :correlative | :architectural | :strategic | :analogical`), Strategic Heuristic.
**Invariant**: `INV-ASSOCIATE-LEARNED-001`: ASSOCIATE MUST traverse learned associations alongside structural edges.
**Source**: Transcript 04:1100–1179

### AA-005: Intention Anchoring — Anti-Goal-Dilution

**Decision**: Intentions pinned at level 0 regardless of budget pressure when `include_intentions=true`.
**Invariant**: `INV-ASSEMBLE-INTENTION-001`.
**Source**: Transcript 04:1506–1513

### AA-006: Ten Named Protocol Operations

**Decision**: TRANSACT, QUERY, ASSOCIATE, ASSEMBLE, BRANCH, MERGE, SYNC-BARRIER, SIGNAL, SUBSCRIBE, GUIDANCE (10 total).
**Source**: Transcript 03; Transcript 04

### AA-007: System 1/System 2 Diagnosis

**Decision**: Generic/hedging output diagnosed as S1/S2 mismatch — retrieval failure, not reasoning failure. Fix: better ASSOCIATE configuration.
**Source**: Transcript 06

---

## Interface & Budget Decisions

### IB-001: Five Interface Layers (Plus Layer 4.5)

**Decision**: Layer 0 (Ambient — CLAUDE.md, ~80 tokens, k*-exempt), Layer 1 (CLI — Rust binary, budget-aware), Layer 2 (MCP Server — thin wrapper), Layer 3 (Guidance — comonadic, spec-language), Layer 4 (TUI — subscription-driven, human-only), Layer 4.5 (Statusline — bridge between human display and agent budget, writes session state file).
**Rationale**: "Agents fail to invoke available tools 56% of the time without ambient awareness." Layer 0 is most important. Layer 4.5 (statusline hook) has zero cost to agent context but produces critical side effect consumed by agent-facing layers.
**Source**: Transcript 04:2444–2638; Transcript 05 (statusline as Layer 4.5, 05:1080–1163)
**Formalized as**: ADR-INTERFACE-001 in `spec/14-interface.md`

### IB-002: CLI Has Three Output Modes

**Decision**: (1) Structured/JSON, (2) Agent mode (100–300 tokens), (3) Human mode (TTY). Agent-mode: headline + entities (3–7) + signals (0–3) + guidance (1–3) + pointers (1–3).
**Principle**: "Demonstration, not constraint list" — agent output follows demonstration style (activates deep LLM substrate) rather than constraint style (activates surface substrate). Concrete BAD/GOOD examples in transcript.
**Source**: Transcript 04:2508–2574
**Formalized as**: ADR-INTERFACE-002 in `spec/14-interface.md`

### IB-003: MCP as Thin Wrapper with Six Tools (Stage 0)

**Decision**: MCP server calls CLI for all computation. Adds session state, budget adjustment, tool descriptions, notification queuing. Stage 0 exposes exactly six tools: `braid_transact` (meta), `braid_query` (moderate), `braid_status` (cheap), `braid_harvest` (meta), `braid_seed` (expensive), `braid_guidance` (cheap). Entity lookup and history are accessible via `braid_query`; CLAUDE.md generation is accessible via `braid_guidance`. Three additional tools (`braid_branch`, `braid_signal`, `braid_associate`) activate at Stage 2+ when branching and multi-agent coordination are available. On every call: reads context state, computes Q(t), passes `--budget`, appends notifications, updates session state, checks thresholds.
**Source**: Transcript 04:2578–2641; Transcript 05:792–900 (original nine tool schemas); revised to six tools for Stage 0 per INV-INTERFACE-003
**Formalized as**: ADR-INTERFACE-004 in `spec/14-interface.md`

### IB-004: CLI Output Budget as Hard Invariant

**Decision**: `--budget <tokens>` flag on every command. Five-level precedence for budget determination: (1) `--budget` flag, (2) `--context-used` flag, (3) session state file `.ddis/session/context.json`, (4) transcript tail-parse (fallback), (5) conservative default 500 tokens. Staleness threshold: 30 seconds.
**Invariant**: `INV-CLI-BUDGET-001`, `INV-INTERFACE-BUDGET-001` (output capped at `max(MIN_OUTPUT, Q(t) × W × budget_fraction)`).
**Source**: Transcript 04:2489–2506; Transcript 05:649–716, 05:988–1006
**Formalized as**: ADR-BUDGET-001 in `spec/13-budget.md`

### IB-005: k* Measurement — Measured Context Replaces Heuristic

**Decision**: k*_eff computed from MEASURED context consumption via Claude Code's `context_window.used_percentage`, NOT from heuristic decay. Claude Code exposes structured JSON via statusline hook. Measurement replaces the turn-count heuristic; heuristic becomes fallback only. Quality-adjusted budget: `Q(t) = k*_eff(t) × attention_decay(k*_eff(t))` where attention_decay is piecewise: 1.0 if k*_eff > 0.6; k*_eff/0.6 if 0.3–0.6; (k*_eff/0.3)² if ≤ 0.3. Output budget: `max(50, Q(t) × 200000 × 0.05)`.
**Rationale**: Heuristic inaccurate because conversation structure varies. Measured consumption gives ground truth. Attention quality degrades faster than context consumption above ~60–70%.
**Fallback**: Exponential decay `k*_eff = k*_base × e^{-αn}` (α=0.03) when measurement unavailable.
**Source**: Transcript 04:2870–2900 (heuristic); Transcript 05:510–648 (measured, Q(t) formula)
**Formalized as**: ADR-BUDGET-001, ADR-BUDGET-002, ADR-BUDGET-003 in `spec/13-budget.md`

### IB-006: k*-Parameterized Guidance Compression

**Decision**: >0.7 = full (100–200 tokens), 0.4–0.7 = compressed (30–60), ≤0.4 = minimal (10–20), ≤0.2 = harvest signal ("Run `ddis harvest`").
**Source**: Transcript 04:2678–2707

### IB-007: CLI Command Taxonomy by Attention Profile

**Decision**: CHEAP (≤50: status, guidance, frontier, branch ls), MODERATE (50–300: associate, query, assemble, diff), EXPENSIVE (300+: assemble --full, seed), META (side effects: harvest, transact, merge).
**Source**: Transcript 04:2819–2862

### IB-008: TUI as Subscription-Driven Push Projection

**Decision**: Continuously-updated projection via SUBSCRIBE. NOT k*-constrained.
**Invariant**: `INV-TUI-LIVENESS-001`: delegation changes and conflicts above threshold trigger notification.
**Source**: Transcript 04:1024–1096

### IB-009: Human-to-Agent Signal Injection via TUI

**Decision**: Human injects signal from TUI, delivered via MCP notification queue in agent's next tool response. Also recorded as datom.
**Source**: Transcript 04:2724–2737

### IB-010: Store-Mediated Trajectory Management

**Decision**: `ddis harvest` extracts durable facts; `ddis seed` generates carry-over. Agent lifecycle: SEED → work 20–30 turns → HARVEST → reset → GOTO SEED.
**Invariant**: `INV-TRAJECTORY-STORE-001`: Seed output five-part template: (1) Context (1–2 sentences), (2) Invariants established, (3) Artifacts, (4) Open questions from deliberations, (5) Active guidance. Formatted as spec-first seed turn.
**Source**: Transcript 04:2742–2812
**Formalized as**: ADR-INTERFACE-003 in `spec/14-interface.md`, ADR-SEED-004 in `spec/06-seed.md`

### IB-011: Rate-Distortion Interface Design

**Decision**: Interface is formally a rate-distortion channel: maximize information value while minimizing attention cost.
**Source**: Transcript 05
**Formalized as**: ADR-SEED-002 in `spec/06-seed.md`

### IB-012: Proactive Harvest Warning (INV-INTERFACE-HARVEST-001)

**Decision**: When Q(t) < 0.15 (~75% consumed), every response includes harvest warning. When Q(t) < 0.05 (~85%), CLI emits ONLY the harvest imperative.
**Rationale**: Continuing past harvest threshold produces diminishing returns — outputs become parasitic.
**Source**: Transcript 05:1037–1048

---

## Guidance System Decisions

### GU-001: Guidance Topology as Comonad

**Decision**: `(W, extract, extend)` where `W(A) = (StoreState, A)`. Guidance nodes are entities with query-driven lookup. Agents can write new guidance nodes.
**Invariant**: `INV-GUIDANCE-EVOLUTION-001`: learned guidance flagged, effectiveness updated empirically, below 0.3 threshold SHOULD be retracted.
**Source**: Transcript 04:857–962
**Formalized as**: ADR-GUIDANCE-001 in `spec/12-guidance.md`

### GU-002: Guidance Lookahead via Branch Simulation

**Decision**: Lookahead (1–5 steps) via virtual branch simulation. "Planning as branch simulation."
**Source**: Transcript 04:1843–1855

### GU-003: Guidance Is the Seed Turn — Spec-Language Phrasing

**Decision**: Guidance MUST use spec-language (invariants, formal structure), NOT instruction-language (steps, checklists).
**Invariant**: `INV-GUIDANCE-SEED-001`.
**Source**: Transcript 04:2647–2673; Transcript 05
**Formalized as**: ADR-GUIDANCE-004 in `spec/12-guidance.md`, ADR-SEED-003 in `spec/06-seed.md`

### GU-004: Dynamic CLAUDE.md Generation

**Decision**: CLAUDE.md dynamically generated from empirical drift patterns. Collapses three concerns: (1) Ambient awareness (Layer 0 — CLAUDE.md IS the ambient awareness), (2) Guidance (Layer 3 — seed context IS the first guidance, pre-computed, zero tool-call cost), (3) Trajectory management (CLAUDE.md IS the seed turn). "One mechanism, three problems solved."
**Source**: Transcript 05; Transcript 06:209–232
**Formalized as**: ADR-SEED-001 in `spec/06-seed.md` (three-concern collapse)

### GU-005: Guidance Injection — Recency Effect Exploitation (INV-GUIDANCE-INJECTION-001)

**Decision**: Every CLI/MCP response MUST include guidance footer specifying next methodologically-correct action. Footer MUST: (a) name specific ddis command, (b) reference active invariants, (c) note uncommitted observations, (d) warn if drifting. Token cost included in budget (high Q(t): ~100; low Q(t): ~15).
**Rationale**: Model's most recent tool output is strongest non-system-prompt influence. Guidance footer exploits recency effect for continuous Basin A re-seeding.
**Source**: Transcript 05:1302–1346

### GU-006: Basin Competition Model for Methodology Drift

**Decision**: Primary failure mode: attractor competition between Basin A (DDIS methodology — multi-step, token-expensive, learned in-conversation) and Basin B (pretrained coding pattern — single-step plan-to-code, deeply embedded). As k*_eff decreases, Basin B's pull increases. At crossover, Basin B captures trajectory and agent's own non-DDIS outputs reinforce it. Three non-fixes: longer CLAUDE.md (decays with k*), more reminders (accelerates k* depletion), simpler tools (can't beat zero friction).
**Rationale**: "Most important practical problem in the entire system." Understanding as basin competition (not memory problem) is essential for designing countermeasures.
**Source**: Transcript 05:1183–1260
**Formalized as**: ADR-GUIDANCE-002 in `spec/12-guidance.md`

### GU-007: Six Anti-Drift Mechanisms (Integrated Architecture)

**Decision**: (1) **Guidance Pre-emption** — CLAUDE.md rules: "Before writing code, MUST run `ddis guidance`." (2) **Guidance Injection** — every tool response includes next-action footer (GU-005). (3) **Drift Detection** — access log analysis: transact gap > 5 bash commands, tool absence > threshold. (4) **Pre-Implementation Gate** — `ddis pre-check --file <path>` before file writes, returns GO/CAUTION/STOP. (5) **Statusline Drift Alarm** — uncommitted count, time since last transact, warning indicator. (6) **Harvest Safety Net** — recovers un-transacted observations.
**Rationale**: No single mechanism sufficient. They compose: injection prevents, detection catches, gate forces, statusline makes visible, harvest recovers.
**Source**: Transcript 05:1262–1568
**Formalized as**: ADR-GUIDANCE-003 in `spec/12-guidance.md`

### GU-009: Unified Guidance as M(t) x R(t) x T(t)

**Decision**: Guidance composes three independently falsifiable scores: M(t) methodology adherence, R(t) graph-based work routing, and T(t) topology fitness. Each has its own invariant (INV-GUIDANCE-008, 010, 011), uses data-driven weights stored as datoms, and is computed at its designated stage (M(t) and R(t) at Stage 0, T(t) at Stage 2).
**Rationale**: Independent scores enable independent verification and independent evolution. A composite score hides which component failed. Hierarchical gating creates artificial dependencies — M(t) being low shouldn't prevent R(t) from routing to the right task. The tensor product preserves each component's information while enabling composition in the comonadic footer.
**Rejected**: (A) Single composite score (hides failure component); (C) Hierarchical gating (creates artificial dependencies).
**Source**: ADRS GU-006, GU-007, GU-008
**Formalized as**: ADR-GUIDANCE-005 in `spec/12-guidance.md`

### GU-008: Guidance-Intention Coherence (INV-GUIDANCE-ALIGNMENT-001)

**Decision**: Actions scored higher if they advance active intentions: `if postconditions(a) ∩ goals(i) ≠ ∅: score(a) += intention_alignment_bonus`.
**Source**: Transcript 04:1858–1875

---

## Lifecycle & Methodology Decisions

### LM-001: Braid Is a New Implementation, Not a Patch

**Decision**: Braid replaces the Go CLI; it does not extend or migrate it.
**Source**: SEED.md §9; HARVEST.md Session 001

### LM-002: Manual Harvest/Seed Before Tools Exist

**Decision**: Methodology precedes tooling.
**Source**: SEED.md §10; Transcript 07

### LM-003: Conversations Are Disposable, Knowledge Is Durable

**Decision**: Bounded conversation trajectories with durable knowledge extraction.
**Source**: SEED.md §5; Transcript 06
**Formalized as**: ADR-HARVEST-002 in `spec/05-harvest.md`

### LM-004: Reconciliation as Unified Taxonomy

**Decision**: All operations are: detect divergence → classify → resolve. Eight types.
**Source**: SEED.md §6; Transcript 07

### LM-005: Semi-Automated Harvest

**Decision**: System proposes harvests from transaction analysis; agent/human confirms.
**Source**: Transcript 05
**Formalized as**: ADR-HARVEST-001 in `spec/05-harvest.md`

### LM-006: Harvest Calibration with FP/FN Tracking (INV-HARVEST-LEARNING-001)

**Decision**: Track empirical quality: committed candidate later retracted = false positive; rejected candidate re-discovered = false negative. High FP → raise thresholds; high FN → lower; both → improve extractor. Harvest as drift metric: 0–2 uncommitted = excellent; 3–5 = minor drift; 6+ = significant.
**Invariant**: `INV-HARVEST-DIAGNOSTIC-001`: uncommitted count stored as datom per session.
**Source**: Transcript 05:1507–1520, 05:2011–2031
**Formalized as**: ADR-HARVEST-003 in `spec/05-harvest.md`

### LM-007: Datom-Exclusive Information

**Decision**: All durable information must exist as datoms. External representations are projections.
**Source**: Transcript 05

### LM-008: Self-Bootstrap Fixed-Point Property

**Decision**: When system manages its own spec, spec IS data. Converges when spec-as-data and spec-as-document agree. The specification PROCESS generates the first test data (invariants about the store become the store's test cases; contradictions caught during spec become contradiction-detection test cases).
**Source**: Transcript 07

### LM-009: Specification Documents Use DDIS Structure

**Decision**: Invariants, ADRs, negative cases, uncertainty markers.
**Source**: Transcript 07

### LM-010: Explicit Residual Divergence

**Decision**: Unresolvable divergence recorded explicitly with uncertainty marker.
**Source**: Transcript 07

### LM-011: Bounded Conversation Lifecycle — 20–30 Turn Cycle

**Decision**: Seven-step loop: (1) fresh conversation, (2) `ddis seed` carry-over, (3) work 20–30 turns transacting, (4) k*_eff drops below threshold, (5) `ddis harvest`, (6) conversation ends, (7) GOTO 1.
**Source**: Transcript 04:2794–2812

### LM-012: Harvest Delegation Topology

**Decision**: Five topologies: single-agent self-review, bilateral peer review, swarm broadcast+voting, hierarchical specialist delegation, human review. Conservative thresholds: auto=0.15, peer=0.40, human=0.70. "Fresh-Agent Self-Review" pattern: depleted agent proposes, fresh session reviews (maximum context asymmetry).
**Formal**: `w_harvest(candidate) = w_intrinsic(candidate) × confidence(extraction)`.
**Source**: Transcript 05:1676–2073
**Formalized as**: ADR-HARVEST-004 in `spec/05-harvest.md`

### LM-013: Harvest Entity Types

**Decision**: Two entity types: **Harvest Session** (`:harvest/session-id`, `:harvest/transcript-path`, `:harvest/agent`, `:harvest/review-topology`, `:harvest/candidate-count`, `:harvest/drift-score`) and **Harvest Candidate** (`:candidate/harvest` ref, `:candidate/datom-spec`, `:candidate/category`, `:candidate/extraction-confidence`, `:candidate/commitment-weight`, `:candidate/status` lattice: `:proposed < :under-review < :committed < :rejected`).
**Source**: Transcript 05:1967–2002

### LM-014: DDR (DDIS Decision Record) as Feedback Loop

**Decision**: When practical usage reveals spec gaps, recorded as DDR with sections: Observation, Impact on Spec, Resolution Options, Decision, Spec Update. DDRs are datoms. Feedback frequency: Stage 0 = every session, Stage 1 = every few sessions, Stage 2 = weekly.
**Source**: Transcript 05:2493–2532

### LM-015: Staged Alignment Strategy for Existing Codebase

**Decision**: Four strategies in preference order: (1) THIN WRAPPER — adapter for different interface, (2) SURGICAL EDIT — fix specific divergences, (3) PARALLEL IMPLEMENTATION — build alongside, migrate, remove, (4) REWRITE — replace entirely. Priority matrix: stable+working = optimize freely; stable+broken = fix now; changing-soon+working = leave alone; changing-soon+broken = defer. "Never rewrite what you can align incrementally."
**Source**: Transcript 05:2536–2626

### LM-016: Seed Document Eleven-Section Structure

**Decision**: SEED.md structured as eleven sections: (1) What DDIS Is, (2) The Problem (coherence leads, memory as subsection), (3) Specification Formalism (bridges why/how), (4) Core Abstraction, (5) Harvest/Seed Lifecycle, (6) Reconciliation Mechanisms, (7) Self-Improvement Loop, (8) Interface Principles, (9) Existing Codebase, (10) Staged Roadmap, (11) Design Rationale. Minimal formalism — no algebra in seed. Mathematical formalism flows to SPEC.md.
**Source**: Transcript 07:329–343

---

## Coherence & Reconciliation Decisions

### CO-001: Coherence Verification Is the Fundamental Problem

**Decision**: DDIS solves coherence verification — maintaining verifiable non-divergence between intent, specification, implementation, and observed behavior. NOT a memory system. The memory problem is the presenting symptom; divergence is the deeper disease. Framing hierarchy: coherence leads, memory subordinated.
**Rationale**: AI agents both amplify the problem (high-volume artifacts with zero durable memory = "divergence factory") and make it more solvable (fast, voluminous output directed at continuous automatic verification).
**Source**: Transcript 06 (coherence verification reframe); Transcript 07:9–17 (framing hierarchy)

### CO-002: Four-Type Divergence Taxonomy (Original)

**Decision**: Original taxonomy: epistemic, structural, consequential, aleatory.
**Note**: Expanded to eight types in CO-003.
**Source**: Transcript 06

### CO-003: Eight-Type Reconciliation Taxonomy (Final)

**Decision**: Eight types with detection and resolution mechanisms:

| Type | Boundary | Detection | Resolution |
|---|---|---|---|
| Epistemic | Store vs. agent knowledge | Harvest gap detection | Harvest |
| Structural | Implementation vs. spec | Bilateral scan / drift | Associate + reimplementation |
| Consequential | Current state vs. future risk | Uncertainty tensor | Guidance |
| Aleatory | Agent vs. agent | Merge conflict detection | Deliberation + Decision |
| Logical | Invariant vs. invariant | Contradiction detection (5-tier) | Deliberation + ADR |
| Axiological | Implementation vs. goals | Fitness function, goal-drift | Human review + ADR revision |
| Temporal | Agent frontier vs. frontier | Frontier comparison | Sync barrier |
| Procedural | Agent behavior vs. methodology | Drift detection (access log) | Dynamic CLAUDE.md |

**Source**: Transcript 07; SEED.md §6

### CO-004: Bilateral Loop Convergence Property

**Decision**: Each cycle reduces total divergence. Converges when forward and backward projections agree.
**Source**: Transcript 06

### CO-005: Specification Formalism as Divergence-Type-to-Mechanism Mapping

**Decision**: Each formalism element maps to a primary divergence detection role: Invariants = logical divergence (falsifiable claims), ADRs = axiological divergence (prevent reversing decisions without knowing why), Negative cases = structural divergence (prevent overspecification in one dimension and underspecification in another).
**Source**: Transcript 06:428–433

### CO-006: Structural vs. Procedural Coherence

**Decision**: Coherence is a structural property, not a procedural obligation. "Process obligations decay under pressure. Structural properties persist because they are enforced by architecture."
**Source**: Transcript 06

### CO-007: Four Recognized Taxonomy Gaps

**Decision**: Four coverage gaps identified in the eight-type reconciliation taxonomy: (1) Spec-to-intent divergence — addressed by intent validation sessions (CO-012), (2) Implementation-to-behavior divergence — addressed by test results as datoms (CO-011), (3) Cross-project coherence — deferred, addressed architecturally (CO-013), (4) Temporal degradation of observations — addressed by observation staleness model (UA-002).
**Source**: Transcript 07:248–275

### CO-008: Five-Point Coherence Statement

**Decision**: (1) Does the spec contradict itself? (2) Does implementation match spec? (3) Does spec still match intent? (4) Do agents agree? (5) Is methodology being followed?
**Source**: Transcript 06

### CO-009: Fitness Function F(S) with Seven Components

**Decision**: `F(S) = 0.18×V + 0.18×C + 0.18×(1-D) + 0.13×H + 0.13×(1-K) + 0.08×(1-I) + 0.12×(1-U)` where U = mean uncertainty. Target: F(S) → 1.0.
**Rationale**: Uncertainty weight 0.12 reflects importance as coordination metric without dominating fitness.
**Source**: Transcript 02:1905–1910; Transcript 03:933–941
**Formalized as**: ADR-BILATERAL-001 in `spec/10-bilateral.md`

### CO-010: Four-Boundary Chain

**Decision**: Divergence arises at four boundaries: Intent → Specification → Implementation → Observed Behavior. Each boundary has a specific divergence type. DDIS provides detection and resolution at EACH boundary.
**Source**: Transcript 06:413–420

### CO-011: Test Results as Datoms

**Decision**: Test results are datoms. "Test X passed at frontier F" is a fact about observed behavior. "Test X failed with error E" is implementation-to-behavior divergence. Extends bilateral loop to the behavior boundary.
**Source**: Transcript 07:255–258

### CO-012: Intent Validation Sessions

**Decision**: Periodic structured reviews where the system assembles current spec state for human review: "Does this still describe what I want?" Output is a datom. Something between deliberation and harvest.
**Source**: Transcript 07:249–253
**Formalized as**: ADR-BILATERAL-003 in `spec/10-bilateral.md`

### CO-013: Cross-Project Coherence (Deferred)

**Decision**: Axiological divergence can occur between projects. Store architecture supports it (multiple stores mergeable) but reconciliation machinery needs cross-store contradiction detection. Deferred to post-Stage-2.
**Source**: Transcript 07:259–261

### CO-015: Divergence Metric as Weighted Boundary Sum

**Decision**: Total divergence across the four-boundary chain (intent -> spec -> impl -> behavior) is quantified as `D(spec, impl) = Sigma_i w_i * |boundary_gap(i)|` where boundary weights reflect the cost of divergence at each boundary. Default: equal weights.
**Rationale**: Each boundary contributes independently to total divergence. Weighted sum is the simplest combination that captures per-boundary severity while remaining decomposable for targeted remediation.
**Uncertainty**: UNC-BILATERAL-002 — boundary weights may need per-project tuning. Confidence: 0.5.
**Source**: ADRS CO-010
**Formalized as**: ADR-BILATERAL-002 in `spec/10-bilateral.md`

### CO-014: Extensible Reconciliation Architecture

**Decision**: Taxonomy extensible by construction: new divergence types yield new detection queries and new deliberation patterns, all producing datoms in the same store. Resolution mechanism for all future types constrained to be datom-producing queries.
**Source**: Transcript 07:315–317

---

## Implementation Decisions

Decisions made during the specification-to-implementation transition, based on research
findings from `audits/stage-0/research/`.

### IMPL-001: Custom Datalog Engine Over Existing Crates

**Decision**: Build a custom Datalog evaluator (~2100-3400 LOC) rather than using Datafrog, Crepe, Ascent, or DDlog. None of the existing Rust Datalog engines support runtime query construction, which is required for `braid query '[:find ...]'`. Crepe and Ascent are compile-time macro systems. DDlog is archived. Datafrog is a fixpoint engine without a Datalog layer.
**Rationale**: The CLI's runtime query requirement (`braid query` accepts arbitrary Datalog strings) fundamentally disqualifies compile-time-only engines. Custom is the only option satisfying all spec requirements (semi-naive, frontier scoping, EAV pattern matching, runtime construction).
**Rejected**: Datafrog (no Datalog parser, no stratification — only primitives), Crepe (compile-time only), Ascent (compile-time only), DDlog (archived, Haskell toolchain).
**Source**: D2-datalog-engines.md; spec/03-query.md; formalized as ADR-IMPL-QUERY-001 in guide/03-query.md.

### IMPL-002: Tiered Tokenization (chars/4 at Stage 0, tiktoken-rs at Stage 1)

**Decision**: Use chars/4 with content-type correction at Stage 0. Graduate to tiktoken-rs (cl100k_base) at Stage 1 when token efficiency tracking needs cross-session comparability. Behind a `TokenCounter` trait for swappability.
**Rationale**: The budget system uses coarse bands (200/500/2000 tokens). A 15-20% approximation error from chars/4 rarely changes band selection. Zero-dependency at Stage 0 avoids complexity during critical foundation work.
**Rejected**: tiktoken-rs at Stage 0 (unnecessary dependency), HuggingFace tokenizers (~40 deps, no Claude model), bpe (no model-specific encoding).
**Source**: D5-tokenizer-survey.md; spec/13-budget.md.

### IMPL-003: Three-Tier Kani CI Pipeline

**Decision**: Split Kani verification into three CI tiers: Fast (every PR, <5 min, ~13 trivial+simple harnesses), Full (nightly, <30 min, all 24 Stage 0 harnesses), Extended (weekly, <2 hours, higher unwind bounds). CaDiCaL as default solver.
**Rationale**: 24 Stage 0 Kani harnesses cannot all run within the 15-minute PR gate. The three-tier split keeps PR verification fast while still running comprehensive verification on a schedule. CaDiCaL shows 10-200x speedup over MiniSat for structural properties.
**Source**: D3-kani-feasibility.md; spec/16-verification.md section 16.2.

---

*This document is maintained alongside `SEED.md` and `HARVEST.md` as part of the manual
bootstrap methodology. When `SPEC.md` exists, each decision here should have a corresponding
formal ADR element. When the datom store exists, each decision becomes a set of datoms and
this file becomes a projection.*
