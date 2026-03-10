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
**Formalized across**: ADR-STORE-001 in `spec/01-store.md`; ADR-TRILATERAL-002 in `spec/18-trilateral.md`

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
**Formalized as**: ADR-STORE-017 in `spec/01-store.md`

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
**Formalized across**: C7 in `spec/00-preamble.md`; INV-SCHEMA-001 in `spec/02-schema.md`; INV-STORE-014 in `spec/01-store.md`; INV-BILATERAL-005 in `spec/10-bilateral.md`

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
**Source**: SEED.md §4 (datom axioms); Transcript 01:762–860 (Options A/B/C analysis)
**Formalized as**: ADR-STORE-018 in `spec/01-store.md`

### FD-010: Embedded Deployment Model

**Decision**: Braid deploys as an embedded, single-process system (analogous to SQLite). No separate database server or daemon required.
**Rationale**: Minimizes operational complexity. Agents invoke Braid as a CLI tool or link it as a library. The VPS-local deployment model means all agents share a filesystem, making a database server unnecessary.
**Rejected**: Client-server database (unnecessary infrastructure at target scale); distributed database (overkill for single-VPS deployment).
**Source**: SEED.md §4 (store algebra); Transcript 01 (embedded SQLite-style deployment)
**Formalized as**: ADR-STORE-006 in `spec/01-store.md`

### FD-011: Rust as Implementation Language

**Decision**: Braid is implemented in Rust. The query engine targets a purpose-built Rust binary as the final form.
**Rationale**: Safety guarantees (ownership, lifetimes), performance (zero-cost abstractions for index operations), and ecosystem support for append-only file structures (redb, LMDB bindings). The user explicitly confirmed "I want the option a) approach" (Rust binary) for the query engine.
**Rejected**: Go (current CLI language — but substrate divergence is fundamental per LM-001); Python (performance insufficient for index operations at scale).
**Source**: SEED.md §8 (interface principles); Transcript 01 (Rust implementation); Transcript 04:2397 (user confirms Rust binary target)
**Formalized as**: ADR-INTERFACE-007 in `spec/14-interface.md`

### FD-012: Every DDIS Command Is a Store Transaction

**Decision**: Every DDIS command becomes a transaction against the datom store. The bilateral loop (discover → refine → crystallize → datoms; scan → absorb → drift → datoms) maps entirely to store operations.
**Rationale**: If any DDIS operation produces state outside the store, that state cannot be queried, conflict-detected, or coherence-verified. The store is the sole truth.
**Source**: SEED.md §10 (every command is a transaction); Transcript 01:866–924 (architecture diagram, command-to-transaction mapping)
**Formalized across**: INV-STORE-014, ADR-STORE-011 in `spec/01-store.md`
**Note**: Access events (query logging per INV-QUERY-003) go to a separate access log (AS-007), not the main datom store. This is a deliberate performance carve-out — the access log is still durable and queryable, preserving FD-012's spirit while avoiding store bloat from high-frequency queries.

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
**Formalized as**: INV-DELIBERATION-005 in `spec/11-deliberation.md`

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
**Formalized across**: INV-QUERY-004 in `spec/03-query.md`; INV-MERGE-003 in `spec/07-merge.md`

### AS-005: Branch as First-Class Entity

**Decision**: Branches are entities in the datom store with schema: `:branch/ident`, `:branch/base-tx`, `:branch/agent`, `:branch/status` (lattice: `:active < :proposed < :committed < :abandoned`), `:branch/purpose`, `:branch/competing-with`.
**Rationale**: Branch metadata (who, why, competing with what) must be queryable via Datalog. The `:branch/competing-with` attribute prevents first-to-commit from winning by default.
**Invariant**: `INV-BRANCH-COMPETE-001`: competing branches MUST NOT commit until comparison or deliberation has occurred.
**Source**: Transcript 04:553–597
**Formalized across**: INV-MERGE-004, INV-MERGE-006, ADR-MERGE-003 in `spec/07-merge.md`

### AS-006: Bilateral Branch Duality

**Decision**: The diverge-compare-converge (DCC) pattern works identically in both directions: forward flow (spec → competing implementations → selection) and backward flow (implementation → competing spec updates → selection). Same algebraic structure, same comparison machinery.
**Rationale**: The bilateral principle is central to DDIS. Separate mechanisms for forward and backward flow would duplicate implementation and violate reconciliation taxonomy symmetry.
**Invariant**: `INV-BRANCH-SYMMETRY-001`: violation if system supports branching for implementation but requires linear spec modifications.
**Source**: Transcript 04:605–653
**Formalized across**: INV-MERGE-007 in `spec/07-merge.md`; INV-BILATERAL-003 in `spec/10-bilateral.md`; INV-DELIBERATION-004 in `spec/11-deliberation.md`

### AS-007: Hebbian Significance via Separate Access Log

**Decision**: Datom significance is computed from a separate access log: `significance(d) = Σ decay(now - t) × query_weight(q)` over all queries that returned `d`. The access log is separate from the main store to avoid unbounded positive feedback.
**Rationale**: Neural analogy: connections strengthened by repeated access. Significance feeds into ASSOCIATE (high-significance attributes surface first) and ASSEMBLE (significance is a selection criterion). Storing access events as datoms in the main store would create infinite loops.
**Formal**: Default assembly weights: α=0.5 (relevance), β=0.3 (significance), γ=0.2 (recency).
**Invariant**: `INV-QUERY-SIGNIFICANCE-001`: every query MUST generate an access event in the access log, NOT in the main store.
**Source**: Transcript 04:659–705
**Formalized across**: INV-QUERY-003, NEG-QUERY-004 in `spec/03-query.md`

### AS-008: Projection Reification as Learning Mechanism

**Decision**: Projection patterns (entity sets + query combinations) are reified as first-class entities when their access-count exceeds a threshold (default: 3 accesses). Reified projections carry significance scores and are discoverable via ASSOCIATE.
**Rationale**: The system learns useful ways to look at data. An agent discovering a useful query propagates it to other agents via the store. Projections develop a shared vocabulary of "good ways to look at things."
**Invariant**: `INV-PROJECTION-LEARNING-001`: projection pattern MUST be stored when access-count exceeds reification threshold.
**Source**: Transcript 04:709–741
**Formalized as**: INV-QUERY-011 in `spec/03-query.md`

### AS-009: Diamond Lattice as Contradiction Signal

**Decision**: Several lattices (challenge-verdict, finding-lifecycle, proposal-lifecycle) use a diamond structure where two incomparable top elements join to produce an error/attention signal. E.g., challenge-verdict: `:confirmed` and `:refuted` are incomparable; their join is `:contradicted`. The CRDT merge of concurrent incomparable values produces a first-class error signal.
**Rationale**: Lattice structure is not just for conflict resolution — it is a signal-generation mechanism. The diamond pattern where incomparable values join to produce a "contradiction" or "contested" state connects the lattice algebra directly to the coordination layer's uncertainty detection.
**Source**: Transcript 02:628–651 (challenge-verdict diamond), 02:712–720 (pattern repeated across three lattices)
**Formalized across**: INV-SCHEMA-008 in `spec/02-schema.md`; INV-SIGNAL-005 in `spec/09-signal.md`

### AS-010: Branch Comparison Entity Type

**Decision**: Branch Comparisons are entities with schema: `:comparison/branches` (ref :many), `:comparison/criterion`, `:comparison/method` (`:automated-test | :fitness-score | :agent-review | :human-review`), `:comparison/scores` (json), `:comparison/winner` (ref), `:comparison/rationale`, `:comparison/agent`.
**Rationale**: Comparison outcomes need structured storage for the competing-branch workflow. Without it, the INV-BRANCH-COMPETE-001 enforcement has no place to record why a branch was selected.
**Source**: Transcript 04:570–580
**Formalized as**: ADR-MERGE-006 in `spec/07-merge.md`

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
**Formalized across**: INV-STORE-012, ADR-STORE-005 in `spec/01-store.md`

### SR-003: LMDB/redb for MVCC Storage Semantics

**Decision**: The storage layer uses LMDB or redb (Rust-native) for persistent storage with MVCC (multi-version concurrency control) semantics.
**Rationale**: MVCC enables concurrent readers without blocking writers. The append-only datom model maps naturally to MVCC — new datoms are new versions, never overwrites. redb is the preferred Rust-native option.
**Rejected**: Direct file I/O without MVCC (reader-writer conflicts); SQLite (viable intermediate step per SR-005 but not the target architecture).
**Source**: Transcript 01 (LMDB/redb MVCC)
**Formalized as**: ADR-STORE-016 in `spec/01-store.md`
**Note**: redb target for primary store is superseded by SR-014 / ADR-LAYOUT-005 (pure filesystem). redb may still serve as an optional index cache.

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
**Note**: trunk.ednl layout superseded by SR-014 / ADR-LAYOUT-001 (per-transaction content-addressed files). See `spec/01b-storage-layout.md`.
**Note**: Both SR-006 and SR-007 map to ADR-STORE-007 (now superseded by ADR-STORE-014). SR-006 covered file layout decisions; SR-007 covered coordination mechanism.

### SR-007: Multi-Agent Coordination via Shared Filesystem

**Decision**: Multiple agents coordinate through the shared filesystem. Each agent writes to its branch file; all read from trunk. File-locking (flock) handles concurrent writes. Append-only structure makes concurrent trunk appends filesystem-safe.
**Rationale**: For co-located VPS agents, the filesystem is a natural coordination mechanism. The datom store IS the communication channel — no separate IPC protocol needed.
**Rejected**: Direct inter-process communication (additional infrastructure; loses store-as-sole-truth property).
**Source**: Transcript 04:2316–2343
**Formalized as**: ADR-STORE-007 in `spec/01-store.md`
**Note**: flock coordination superseded by SR-014 / ADR-LAYOUT-006 (O_CREAT|O_EXCL structural concurrency). See `spec/01b-storage-layout.md`.
**Note**: Both SR-006 and SR-007 map to ADR-STORE-007 (now superseded by ADR-STORE-014). SR-006 covered file layout decisions; SR-007 covered coordination mechanism.

### SR-008: Axiomatic Meta-Schema — 17 Bootstrap Attributes

**Decision**: The meta-schema consists of exactly 17 axiomatic attributes hardcoded in the engine (not defined by datoms): `:db/ident`, `:db/valueType`, `:db/cardinality`, `:db/doc`, `:db/unique`, `:db/isComponent`, `:db/resolutionMode`, `:db/latticeOrder`, `:db/lwwClock`, plus lattice definition attributes (`:lattice/ident`, `:lattice/elements`, `:lattice/comparator`, `:lattice/bottom`, `:lattice/top`), plus transaction provenance attributes (`:tx/time` (Instant), `:tx/agent` (Ref->AgentId), `:tx/provenance` (String)). Value types include non-standard `:db.type/json` and `:db.type/tuple`. Three LWW clock options: `:hlc`, `:wall`, `:agent-rank`.
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

### SR-011: Session State File as Coordination Point

**Decision**: A session state file at `.ddis/session/context.json` serves as the coordination point between the Claude Code statusline hook and the CLI's budget system. Contains: `used_percentage`, `input_tokens`, `remaining_tokens`, `k_eff`, `quality_adjusted`, `output_budget`, `timestamp`, `session_id`.
**Rationale**: The statusline hook has direct access to Claude Code's context window data. Writing to a well-known file path allows the CLI to read budget data without needing MCP or transcript parsing.
**Invariant**: `INV-SESSION-STATE-001`: session state file must be updated on every statusline render cycle.
**Source**: Transcript 05:652–737
**Formalized as**: INV-INTERFACE-004 in `spec/14-interface.md`

### SR-012: Owned Schema with Borrow API

**Decision**: Store owns a `Schema` field internally, derived from schema datoms on load. Exposed via `store.schema() -> &Schema` (zero-cost borrow). Schema is reconstructed after schema-modifying transactions.
**Rationale**: Avoids lifetime infection from Option A (borrow-based `Schema<'a>`), prevents divergence from Option B (copied data that can go stale). Maintains C3 because Schema is always derived from datoms — `Schema::from_store()` is the sole constructor.
**Rejected**: (A) Borrow-based `Schema<'a>` (lifetime-infectious in Rust); (B) Independent copy (can diverge from store after construction).
**Source**: C3, INV-SCHEMA-001
**Formalized as**: ADR-SCHEMA-005 in `spec/02-schema.md`

### SR-013: Free Functions Over Store Methods for Namespace Operations

**Decision**: Namespace operations (query, harvest, seed, merge, guidance) are free functions taking `&Store` rather than Store methods. Store methods are reserved for core datom operations: `genesis()`, `transact()`, `current()`, `as_of()`, `len()`, `datoms()`, `frontier()`, `schema()`.
**Rationale**: Store is a datom container, not an application framework. Free functions keep namespace logic independent of Store internals, prevent Store from becoming a god object, and enable testing with mock stores. Each namespace's free functions form a natural Rust module boundary.
**Source**: SEED.md §4, §10; ADRS FD-010, FD-012
**Formalized as**: ADR-STORE-015 in `spec/01-store.md`, ADR-ARCHITECTURE-001 in `docs/guide/00-architecture.md`

### SR-014: Per-Transaction Content-Addressed Storage Layout

**Decision**: The physical storage layout uses per-transaction content-addressed files organized under 256-way hash-prefix sharding. Each transaction is a single EDN file named by its BLAKE3 hash. Merge is directory union. Verification is blake3sum. No database backend for the primary store.
**Rationale**: The G-Set CvRDT axioms (L1–L5) require that merge is set union. A single append-only file (trunk.ednl) creates git merge conflicts because the physical format doesn't reflect the algebraic structure. Per-transaction files make the filesystem isomorphic to the algebra — the CRDT axioms become filesystem tautologies.
**Supersedes**: SR-006 Option A (trunk.ednl), SR-007 (flock coordination), SR-003 redb as primary target (ADR-STORE-007 Options A and B).
**Source**: Coherence analysis of trunk.ednl scaling under concurrent agent writes.
**Formalized as**: INV-LAYOUT-001–011, ADR-LAYOUT-001–007, NEG-LAYOUT-001–005 in `spec/01b-storage-layout.md`
**Note**: Range notation covers all 7 ADRs, 11 INVs, and 5 NEGs in the LAYOUT namespace (spec/01b-storage-layout.md).

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
**Formalized across**: INV-STORE-013 in `spec/01-store.md`; NEG-MERGE-003 in `spec/07-merge.md`

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
**Rationale**: The datom store is a G-Set CRDT where adding the same datom twice is a no-op, making duplicate delivery harmless for TRANSACT and MERGE operations. This exploits the CRDT's inherent idempotence to achieve exactly-once semantics with at-least-once delivery mechanics. At-most-once was rejected as too lossy, and exactly-once was rejected as prohibitively expensive (requiring 2PC). The exception is SYNC-BARRIER, which requires stronger coordination, but sync barriers are rare by design.
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
**Formalized as**: ADR-BILATERAL-004 in `spec/10-bilateral.md`

---

## Protocol Operations Decisions

Specifications for the canonical protocol operations.

### PO-001: TRANSACT — Seven-Field Type Signature

**Decision**: TRANSACT requires: `agent`, `branch` (None=trunk), `datoms`, `causal_parents` (set of TxIds), `provenance` (ProvenanceType), `rationale` (string), `operation` (keyword: `:op/observe | :op/infer | :op/deliberate | :op/crystallize | :op/resolve`).
**Rationale**: Rich transaction metadata is essential for causal tracing, provenance auditing, and conflict detection. Requiring causal parents makes the partial order explicit; requiring provenance type enables structural auditing of epistemic claims; requiring operation kind enables stratum-aware query classification.
**Invariants**: `INV-TX-APPEND-001` (S ⊆ S'), `INV-TX-CAUSAL-001` (causal parents must reference known tx), `INV-TX-BRANCH-001` (branch tx cannot affect trunk), `INV-TX-PROVENANCE-001` (provenance structurally consistent), `INV-TX-FRONTIER-DURABLE-001` (frontier stored before response).
**Source**: Transcript 04:1252–1342
**Formalized across**: INV-STORE-002, INV-STORE-010 in `spec/01-store.md`; INV-SCHEMA-004 in `spec/02-schema.md`

### PO-002: ASSOCIATE — Two-Mode Schema Discovery

**Decision**: Two modes: `SemanticCue` (natural language) or `ExplicitSeeds` (entity IDs). Returns SchemaNeighborhood (entities, attributes, types — not values). Bounded by `depth × breadth`.
**Rationale**: Schema discovery must support both human-initiated exploration (natural language cues) and programmatic traversal (explicit entity seeds). Returning schema structure rather than values keeps ASSOCIATE in System 1 (fast, associative) and defers value retrieval to QUERY (System 2), preserving the dual-process architecture.
**Invariants**: `INV-ASSOCIATE-BOUND-001` (size ≤ depth × breadth), `INV-ASSOCIATE-SIGNIFICANCE-001` (high-significance preferred), `INV-ASSOCIATE-LEARNED-001` (learned associations traversed alongside structural edges).
**Source**: Transcript 04:1399–1451
**Formalized as**: INV-SEED-003 in `spec/06-seed.md`

### PO-003: ASSEMBLE — Rate-Distortion Context Construction

**Decision**: Takes query results + schema neighborhood + budget, produces assembled context using pyramid-level selection per entity. Priority: `score = α × relevance + β × significance + γ × recency` (defaults: 0.5, 0.3, 0.2).
**Rationale**: Context assembly is a rate-distortion problem: maximize information value delivered to the agent while respecting the finite attention budget. Pyramid-level selection ensures structural coherence (dependencies included before dependents), while the weighted scoring prevents recency bias from dominating over relevance and accumulated significance.
**Invariants**: `INV-ASSEMBLE-BUDGET-001` (≤ budget), `INV-ASSEMBLE-PYRAMID-001` (structural dependency coherence), `INV-ASSEMBLE-INTENTION-001` (intentions pinned at level 0), `INV-ASSEMBLE-PROJECTION-001` (record projection), `INV-ASSEMBLE-FRESHNESS-001` (check staleness, apply freshness-mode).
**Source**: Transcript 04:1453–1522
**Formalized across**: INV-SEED-002, INV-SEED-004, INV-SEED-006 in `spec/06-seed.md`

### PO-004: SIGNAL — Coordination as Datoms

**Decision**: Every signal (Confusion, Conflict, UncertaintySpike, ResolutionProposal, DelegationRequest, GoalDrift, BranchReady, DeliberationTurn) is recorded as a datom.
**Rationale**: Coordination events must be durable and queryable, not ephemeral messages. Recording signals as datoms means signal history participates in merge, conflict detection, and causal tracing like any other fact, and agents can reason over coordination patterns via the same Datalog apparatus used for domain queries.
**Invariant**: `INV-SIGNAL-DATOM-001`: signal history must be queryable.
**Source**: Transcript 04:1715–1769
**Formalized across**: INV-SIGNAL-001, ADR-SIGNAL-001 in `spec/09-signal.md`

### PO-005: Confusion Signal as First-Class Protocol Operation

**Decision**: `Confusion(cue)` with type (NeedMore, Contradictory, GoalUnclear, SchemaUnknown) triggers automatic re-ASSOCIATE + re-ASSEMBLE within one agent cycle — NOT a full round-trip.
**Rationale**: Confusion is the most common failure mode in agent-store interaction. Handling it within a single cycle (rather than requiring a full round-trip through the human) turns retrieval failures into self-correcting loops, dramatically reducing latency and preventing agents from proceeding with insufficient context.
**Invariant**: `INV-SIGNAL-CONFUSION-001`: confusion MUST trigger re-ASSOCIATE + re-ASSEMBLE within one cycle.
**Source**: Transcript 04:49–61, 04:1754–1762
**Formalized across**: INV-SIGNAL-002, NEG-SIGNAL-002 in `spec/09-signal.md`

### PO-006: MERGE — Epistemic Event with Cascade

**Decision**: MERGE propagates consequences: invalidated queries, new conflicts, uncertainty deltas, stale projections. All cascade effects recorded as datoms. The cascade is a **deterministic function of the merged datom set** — it does not depend on which agent executes it, when, or in what order the input stores were supplied. This determinism property (INV-MERGE-010) is what restores L1 (commutativity) and L2 (associativity) at the total post-merge state level.
**Rationale**: Bare set union (C4) is necessary but insufficient for coordination; agents need to know what changed and what broke. Making the cascade deterministic ensures that any agent running the same merge produces identical cascade datoms, preserving the CRDT algebraic properties even though MERGE produces secondary effects beyond the union itself.
**Invariant**: `INV-MERGE-002` (cascade completeness), `INV-MERGE-010` (cascade determinism).
**Source**: Transcript 04:64–78, 04:1642–1651; V1 Audit (Agent 10, R2.2)
**Formalized across**: ADR-MERGE-001, ADR-MERGE-005, ADR-MERGE-007 in `spec/07-merge.md`

### PO-007: BRANCH — Six Sub-Operations with Competing-Branch Lock

**Decision**: Fork, Commit, Combine (strategies: Union, SelectiveUnion, ConflictToDeliberation), Rebase, Abandon, Compare (criteria: FitnessScore, TestSuite, UncertaintyReduction, AgentReview, Custom). Competing branches locked from commit until comparison/deliberation.
**Rationale**: The competing-branch lock prevents first-to-commit from winning by default, which would undermine the deliberation process. Requiring explicit comparison before commit ensures that the selection between competing approaches is an evaluated decision rather than a race condition.
**Invariant**: `INV-BRANCH-COMPETE-001`, `INV-BRANCH-DELIBERATION-001` (ConflictToDeliberation opens deliberation).
**Source**: Transcript 04:1524–1591
**Formalized across**: INV-MERGE-004, ADR-MERGE-003, ADR-MERGE-004 in `spec/07-merge.md`

### PO-008: SUBSCRIBE — Pattern-Based Push Notifications

**Decision**: Registers Datalog-like pattern filter with callback. Debounce parameter batches rapid-fire matches.
**Rationale**: Polling-based coordination wastes agent attention budget and introduces latency. Push notifications via Datalog pattern matching allow agents to react to relevant store changes without continuously querying, while debouncing prevents cascading reactions during high-frequency transaction bursts.
**Invariants**: `INV-SUBSCRIBE-COMPLETENESS-001` (fire for every match within refresh cycle), `INV-SUBSCRIBE-DEBOUNCE-001` (debounced notifications must batch within window).
**Source**: Transcript 04:1772–1814
**Formalized across**: INV-SIGNAL-003, ADR-SIGNAL-003 in `spec/09-signal.md`

### PO-009: GUIDANCE — Query over Guidance Graph

**Decision**: Queries available action topology by evaluating guidance nodes' state predicates. Returns actions + optional lookahead tree (1–5 steps). Includes system-default and learned guidance.
**Rationale**: Agents need methodologically-correct next actions, not just data. By querying a guidance graph whose nodes are state-predicate-gated, the system provides context-sensitive direction that adapts to the current store state, preventing methodology drift (Basin B capture) by continuously re-seeding the agent toward the DDIS workflow.
**Invariants**: `INV-GUIDANCE-ALIGNMENT-001` (actions scored higher if they advance active intentions: `if postconditions(a) ∩ goals(i) ≠ ∅: score(a) += intention_alignment_bonus`), `INV-GUIDANCE-LEARNED-001` (learned guidance ranked by effectiveness).
**Source**: Transcript 04:1816–1875
**Formalized as**: ADR-GUIDANCE-006 in `spec/12-guidance.md`

### PO-010: SYNC-BARRIER — Topology-Dependent Frontier Exchange

**Decision**: Topology-dependent (Option C): protocol provides primitives; deployment chooses topology. User confirmed "C for sure."
**Rationale**: Hardcoding a sync topology would violate protocol topology-agnosticism (PD-005). By providing sync primitives rather than a fixed topology, deployments can choose the coordination pattern (star, mesh, hierarchical) that fits their agent configuration without protocol changes.
**Invariants**: `INV-BARRIER-TIMEOUT-001` (resolve within timeout), `INV-BARRIER-CRASH-RECOVERY-001` (recovering agents can query barrier record).
**Source**: Transcript 04:1960–1977
**Formalized as**: ADR-SYNC-001, ADR-SYNC-003 in `spec/08-sync.md`

### PO-011: Agent Cycle as Ten-Step Composition

**Decision**: (1) ASSOCIATE, (2) QUERY, (3) ASSEMBLE with guidance+intentions, (4) GUIDANCE lookahead=2, (5) agent policy evaluates, (6a) action → TRANSACT or (6b) confusion → re-ASSOCIATE/ASSEMBLE → retry, (7) learned association → TRANSACT(:inferred), (8) subtask → TRANSACT(intention update), (9) check incoming MERGE/signals, (10) repeat.
**Rationale**: Defining the canonical agent cycle as a fixed composition of protocol operations ensures that every agent interaction follows the dual-process pattern (ASSOCIATE/QUERY/ASSEMBLE for System 1, then policy evaluation for System 2) and that confusion recovery, learned associations, and signal processing are never skipped.
**Source**: Transcript 04:1880–1927
**Formalized as**: ADR-INTERFACE-008 in `spec/14-interface.md`

### PO-012: Genesis Transaction

**Decision**: Store begins with genesis transaction containing schema definitions. No causal parents. Root of the causal graph.
**Rationale**: A deterministic, content-identical genesis ensures that all stores share a common root, which is a prerequisite for set-union merge (C4) to produce consistent results. Without identical genesis, independently created stores would diverge in their meta-schema, making merge undefined.
**Invariant**: `INV-GENESIS-001`: Transaction tx=0 MUST contain exactly the axiomatic meta-schema attributes and nothing else. All stores begin from identical genesis state. `∀ S1, S2: S1|_{tx=0} = S2|_{tx=0}`. Verified by constant hash of tx=0 datom set.
**Source**: Transcript 02:429–442
**Formalized across**: INV-STORE-008 in `spec/01-store.md`; INV-SCHEMA-002 in `spec/02-schema.md`

### PO-013: QUERY — Datalog Evaluation with Four Invariants

**Decision**: QUERY evaluates Datalog expressions against a specified frontier and branch. Four invariants: (1) `INV-QUERY-CALM-001`: monotonic-mode queries MUST NOT contain negation/aggregation; reject at parse time. (2) `INV-QUERY-BRANCH-001`: branch query visibility = `visible(b)`. (3) `INV-QUERY-SIGNIFICANCE-001`: every query generates access event in access log. (4) `INV-QUERY-DETERMINISM-001`: identical expressions at identical frontiers MUST return identical results.
**Rationale**: The four invariants enforce CALM compliance (monotonic queries safe without coordination), branch isolation (snapshot semantics), Hebbian learning (access log feeds significance computation), and reproducibility (deterministic results enable caching and verification). Together they make QUERY both safe for concurrent execution and self-improving through usage tracking.
**Source**: Transcript 04:1370–1397
**Formalized across**: INV-QUERY-001, INV-QUERY-002, INV-QUERY-003, NEG-QUERY-001 in `spec/03-query.md`

### PO-014: GENERATE-CLAUDE-MD — Dynamic Instruction Generation

**Decision**: Formal operation with signature `(focus, agent, budget)`. Seven-step process: ASSOCIATE, QUERY active intentions, QUERY governing invariants, QUERY uncertainty, QUERY competing branches, QUERY drift patterns, QUERY guidance topology, ASSEMBLE at budget. Priority ordering: tools > task context > risks > drift corrections > seed context.
**Rationale**: Static CLAUDE.md cannot adapt to the agent's current task, observed drift patterns, or remaining attention budget. Dynamic generation collapses ambient awareness (Layer 0), guidance (Layer 3), and trajectory management into a single mechanism, ensuring that every session starts with instructions calibrated to what actually matters now rather than a generic document that decays in relevance.
**Invariants**: `INV-CLAUDE-MD-RELEVANCE-001` (every section relevant to focus; falsified if removing a section wouldn't change behavior), `INV-CLAUDE-MD-IMPROVEMENT-001` (drift corrections derived from empirical data; corrections showing no effect after 5 sessions replaced).
**Source**: Transcript 06:147–207
**Formalized across**: INV-SEED-007, INV-SEED-008 in `spec/06-seed.md`; INV-GUIDANCE-007 in `spec/12-guidance.md`

---

## Snapshot & Query Decisions

### SQ-001: Local Frontier as Default Query Mode (Option 3C)

**Decision**: Default query mode is local frontier. Consistent cuts via optional sync barriers for non-monotonic queries.
**Rejected**: (A) Local frontier only. (B) Consistent cut only (too expensive for monotonic queries).
**Rationale**: When transactions form a partial order (a DAG) across multiple concurrent writers rather than Datomic's single-writer total order, three snapshot semantics were considered. Option 3A (local frontier only) provides maximum performance but cannot support non-monotonic queries that require agreement. Option 3B (consistent cuts) provides strong semantics but imposes coordination cost on all queries. Option 3C (both, layered) was chosen because it maps directly to the cascading intelligence model: storage and agent layers operate uncoordinated on local frontiers, while the swarm/meta-agent layer invokes sync barriers to establish shared consistent cuts only when coordinated reasoning is required.
**Source**: Transcript 01:518–542, 01:645
**Formalized as**: ADR-QUERY-005 in `spec/03-query.md`

### SQ-002: Frontier as Datom Attribute

**Decision**: Frontier stored as `:tx/frontier`. Concrete type: `Frontier = Map<AgentId, TxId>` (vector-clock equivalent).
**Rationale**: The frontier is stored as a multi-valued ref attribute on the transaction entity itself — the vector-clock equivalent expressed in datom form. Unlike fixed-size vector clocks, the set of refs grows dynamically as an agent learns about more peers, solving the dynamic agent count problem. By recording frontier on the transaction, snapshot semantics (Axiom A3) fall out naturally: any query can be restricted to the down-set in the causal partial order defined by a given frontier, without coordination infrastructure external to the store.
**Source**: Transcript 02:471–479; Transcript 04:1190–1243
**Formalized as**: ADR-QUERY-006 in `spec/03-query.md`

### SQ-003: Datalog Frontier Query Extension

**Decision**: Datalog extended with `[:frontier ?frontier-ref]` clause.
**Rationale**: Standard Datomic-style Datalog has no mechanism to express "show me the state as of frontier X" — the frontier would have to be enforced outside the query engine, breaking composability. The `[:frontier ?frontier-ref]` clause keeps causal-cut semantics within the query language. Two co-designed extensions (`:stability-min ?threshold` for commitment-weight pre-filtering and `:barrier :required` for sync barrier enforcement) provide CALM theorem boundary enforcement for non-monotonic queries, keeping coordination semantics within the query language rather than scattered across imperative code.
**Source**: Transcript 02:1004–1012
**Formalized across**: INV-QUERY-007, ADR-QUERY-006 in `spec/03-query.md`

### SQ-004: Stratum Safety Classification

**Decision**: Strata 0–1 (monotonic) and Stratum 4 (conservatively monotonic) safe without coordination. Strata 2–3 (mixed/FFI) require frontier-specific evaluation (Stratified mode). Stratum 5 (non-monotonic bilateral loop) requires sync barriers (Barriered mode). `QueryMode = Monotonic | Stratified Frontier | Barriered BarrierId`.
**Rationale**: The three-mode classification was chosen over a binary safe/unsafe split because the intermediate "conservatively monotonic" category — where false positives are harmless and self-correcting on merge — covers the majority of coordination queries and would be unnecessarily restricted by a blanket barrier requirement. Strata 0–1 are fully monotonic, Stratum 4 (conflict detection) over-detects but never under-detects, and only Stratum 5 (negation, aggregation) requires sync barriers for correctness-critical decisions.
**Source**: Transcript 02:2047; Transcript 04:1190–1243
**Formalized across**: INV-QUERY-005, ADR-QUERY-003 in `spec/03-query.md`

### SQ-005: Topology-Agnostic Resolution Invariant

**Decision**: Query results must be identical regardless of dissemination topology.
**Rationale**: The original invariant (INV-CASCADE-001) assumed a hierarchical coordination topology where delegation level must not decrease when uncertainty increases. This was retracted after recognizing that the space of coordination topologies includes trees, flat swarms, markets, rings, and hybrids, and baking in a hierarchical assumption would exclude valid patterns. The replacement invariant states that when uncertainty increases, the set of agents with authority to resolve must not shrink — expressed in terms of resolver set size rather than structural position, holding regardless of topology.
**Source**: Transcript 02
**Formalized across**: INV-QUERY-010 in `spec/03-query.md`; INV-SYNC-003 in `spec/08-sync.md`

### SQ-006: Bilateral Query Layer Structure

**Decision**: Query layer is bilateral. Forward queries (spec → impl status) and backward queries (impl → spec alignment) use the same Datalog apparatus.
**Formal**: Queries naturally partition into: Forward-flow (planning: epistemic uncertainty, crystallization candidates, delegation, ready tasks), Backward-flow (assessment: conflict detection, drift candidates, aleatory uncertainty, absorption triggers), Bridge (both: commitment weight, consequential uncertainty, spectral authority). Spectral authority is the explicit bridge — updated by backward-flow observations, consumed by forward-flow decisions.
**Rationale**: After defining the complete query pattern set across all five strata, the queries naturally partitioned into forward-flow (planning) and backward-flow (assessment), with commitment weight, consequential uncertainty, and spectral authority as the bridge. This bilateral structure was not designed in but fell out of the formalization, mirroring the bilateral structure in the DDIS workflow. Making the partition explicit ensures the query apparatus serves both directions with equal fidelity and that spectral authority is recognized as the explicit bridge between observation and decision.
**Source**: Transcript 02; Transcript 03:1084–1094
**Formalized as**: ADR-QUERY-008 in `spec/03-query.md`

### SQ-007: Projection Pyramid — Level-Based Summarization

**Decision**: Pyramid `{π₀, π₁, π₂, π₃}`: π₀ = full datoms, π₁ = entity summaries, π₂ = type summaries, π₃ = store summary.
**Budget-driven selection**: >2000 tokens = π₀ for top/π₁ for others; 500–2000 = π₁/π₂; 200–500 = π₂ for top/omit others; ≤200 = single-line status + single guidance action.
**Invariant**: `INV-ASSEMBLE-PYRAMID-001`: structural dependency coherence.
**Rationale**: The projection pyramid defines a family of projection functions indexed by level, where each level is a lossy compression of the one below, enabling ASSEMBLE to select the appropriate detail level per entity based on relevance and significance scores. Summaries at each level are themselves datoms with staleness tracking, recomputed lazily or eagerly as underlying entities change. This enables rate-distortion optimization: an agent debugging a specific invariant gets full detail (L0) for that invariant, entity summaries (L1) for neighbors, type summaries (L2) for the module, and store summary (L3) for the rest.
**Source**: Transcript 04:966–1021; Transcript 05:1008–1019 (budget thresholds)
**Formalized as**: ADR-QUERY-007 in `spec/03-query.md`

### SQ-008: Complete Protocol Type Definitions

**Decision**: `Value = String | Keyword | Boolean | Long | Double | Instant | UUID | Ref EntityId | Bytes | URI | BigInt | BigDec | Tuple [Value] | Json String`; `Level = 0 | 1 | 2 | 3`; full Signal sum type.
**Rationale**: The complete type definitions formalize every protocol-visible concept as algebraic types, providing a single unambiguous type catalog that every operation's signature references, ensuring type safety across the full protocol. The Signal type enumerates all inter-agent communication cases, and QueryMode distinguishes monotonic, stratified, and barriered queries to enforce CALM-compliance boundaries at the type level.
**Source**: Transcript 04:1190–1243
**Formalized as**: ADR-SCHEMA-006 in `spec/02-schema.md`

### SQ-009: Six-Stratum Query Classification

**Decision**: Six strata with 17 named query patterns: Stratum 0 (primitive, monotonic — current-value over LIVE), Stratum 1 (graph traversal, monotonic — causal-ancestor, depends-on, cross-ref reachability), Stratum 2 (uncertainty, mixed — epistemic/aleatory/consequential), Stratum 3 (authority, not pure Datalog — linear algebra: SVD, delegation threshold), Stratum 4 (conflict detection, conservatively monotonic — detect-conflicts, route-conflict), Stratum 5 (bilateral loop, non-monotonic — spec-fitness, crystallization-candidates, drift-candidates).
**Rationale**: Systematic safety analysis. Strata 0–3 safe at any frontier. Strata 4–5 benefit from sync barriers for correctness-critical decisions.
**Source**: Transcript 03:1052–1081
**Formalized as**: ADR-QUERY-003 in `spec/03-query.md`

### SQ-010: Datalog/Imperative Boundary for Derived Functions

**Decision**: Three core computations CANNOT be expressed in pure Datalog: σ_a (requires entropy — grouping, division, logarithm), σ_c (requires bottom-up DAG traversal with memoization), spectral authority (requires linear algebra — SVD). These are DERIVED FUNCTIONS: Datalog provides the input query, a Rust function computes the result. σ_e uses count-distinct aggregation (borderline). The query engine must support a foreign-function interface for derived computations.
**Rationale**: Establishes the boundary between declarative queries and imperative computation. Major architectural implication: three of four core coordination computations are derived functions.
**Source**: Transcript 02:1318–1321 (σ_a), 02:1391 (σ_c), 02:1475–1476 (authority); Transcript 03:346–392, 03:422–466
**Formalized as**: ADR-QUERY-004 in `spec/03-query.md`

### SQ-011: Full Graph Engine in Kernel

**Decision**: Graph algorithms (PageRank, betweenness, critical path, SCC, k-core, etc.) are first-class kernel query operations alongside Datalog, with results stored as datoms.
**Rationale**: Graph algorithms are the foundation of task derivation (INV-GUIDANCE-009), work routing (INV-GUIDANCE-010), and topology fitness (INV-GUIDANCE-011). Externalizing them would break CRDT merge — graph results must be datoms to merge across agents. Graph metrics over the datom reference graph are monotonic (CALM-compliant).
**Rejected**: (A) External tools (breaks store-as-sole-truth); (B) FFI derived functions (forces unnatural Datalog encoding of results).
**Source**: ADRS SQ-004, FD-003
**Formalized as**: ADR-QUERY-009 in `spec/03-query.md`

### SQ-012: QueryExpr as Two-Variant Enum (Find + Pull)

**Decision**: The top-level query expression `QueryExpr` is a two-variant enum (`Find(ParsedQuery)` and `Pull { pattern, entity }`), not a flat struct. `ParsedQuery` captures the full Datomic AST: `find_spec` (four Datomic find forms), `where_clauses`, `rules`, and `inputs`.
**Rationale**: Pull queries and Find queries are categorically different operations with different evaluation strategies (O(1) entity lookup vs join-based Datalog evaluation). An enum models this distinction at the type level. `ParsedQuery` with four fields is the complete Datomic AST — the flat struct (`{find_spec, where_clauses}`) omitted rules and inputs, which are needed for stratum classification and parameterized queries. Choosing the flat struct would require a breaking API change later when Pull support is added.
**Source**: Transcript 02:982–1008 (Datomic dialect definition); spec/03-query.md (R6.7b reconciliation adopted guide's richer form)
**Formalized as**: QueryExpr, ParsedQuery, FindSpec, Clause types in `spec/03-query.md` and `docs/guide/03-query.md`

---

## Uncertainty & Authority Decisions

### UA-001: Three-Dimensional Uncertainty Tensor

**Decision**: Uncertainty `σ = (σ_e, σ_a, σ_c)`: epistemic (reducible by observation), aleatory (inherent randomness — Shannon entropy), consequential (downstream risk — DAG traversal).
**Formal**: Scalar combination: `scalar = √(α·σ_e² + β·σ_a² + γ·σ_c²)`. Default weights: α=0.4 (epistemic), β=0.4 (aleatory), γ=0.2 (consequential). Weights stored as datoms, configurable per deployment.
**Rationale**: σ_e and σ_a weighted equally (both actionable). σ_c weighted lower (structural, depends on graph topology which changes slowly). Overweighting σ_c causes excessive caution about well-understood heavily-depended entities.
**Source**: Transcript 01; Transcript 02 (complete formalization, default weights at 1456–1469); Transcript 03:473–501
**Formalized as**: ADR-UNCERTAINTY-001 in `spec/15-uncertainty.md`

### UA-002: Epistemic Uncertainty Temporal Decay

**Decision**: Epistemic uncertainty increases over time. Exponential form with per-namespace lambda calibration: `age_factor(e) = 1 - e^{-λ × time_since_last_validation(e)}`. Code observations decay fast; architectural decisions decay slowly; invariants do not decay (normative, not descriptive).
**Rationale**: The original epistemic uncertainty formulation captured information gaps at a point in time but missed the degradation problem: facts become stale as the world changes, and an observation about a codebase made months ago may no longer be accurate. The exponential decay term makes epistemic uncertainty increase proactively with time since last validation. Per-namespace lambda calibration is critical: code observations decay fast (code changes frequently), architectural decisions decay slowly (they are stable), and invariants do not decay (they are normative statements, not descriptive observations).
**Source**: Transcript 02; Transcript 07:263–275 (exponential form, per-namespace lambda)
**Formalized as**: ADR-UNCERTAINTY-002 in `spec/15-uncertainty.md`

### UA-003: Spectral Authority via SVD

**Decision**: Authority computed via SVD of bipartite agent-entity contribution matrix. Captures TRANSITIVE authority — if agent α contributed to entities A, B, C related to D, α has authority over D even without directly touching D. Mathematically identical to LSI search applied to agent-entity matrix. Truncated SVD with k = min(50, agent_count, entity_count).
**Rationale**: Self-reported authority is unreliable. Raw contribution counting misses transitive authority. SVD projects agents and entities into shared latent space where proximity = structural similarity. "LSI finds relevant documents; spectral authority finds authoritative agents."
**Invariant**: `INV-AUTHORITY-001`: Agent authority MUST be derived from weighted spectral decomposition of contribution graph. MUST NOT be assigned by configuration. Exception: human authority is axiomatically unbounded. Falsification: authority granted by configuration rather than contribution.
**Source**: Transcript 01:1432–1448; Transcript 02:1523–1577; Transcript 03:604–608
**Formalized as**: ADR-RESOLUTION-011 in `spec/04-resolution.md`

### UA-004: Delegation Threshold Formula

**Decision**: `threshold = 0.3×betweenness + 0.2×in_degree + 0.3×σ_c + 0.2×conflict_surface`, where conflict_surface = fraction of entity's cardinality-one attributes. Delegation classification: `delegatable` (resolvers > 0 AND uncertainty < 0.2), `contested` (resolvers > 0 AND uncertainty ≥ 0.2), `escalated` (resolvers = 0 AND uncertainty < 0.5), `human-required` (resolvers = 0 AND uncertainty ≥ 0.5). Thresholds configurable as datoms.
**Rationale**: The threshold formula weights four orthogonal risk factors: betweenness centrality (0.3, measuring cross-reference flow), in-degree (0.2, measuring dependents), consequential uncertainty σ_c (0.3, measuring downstream impact), and conflict surface (0.2, fraction of cardinality-one attributes susceptible to concurrent conflicts). All threshold values are stored as datoms rather than hardcoded, making the delegation policy itself queryable and evolvable. The design degrades conservatively: an agent that hasn't yet seen schema datoms from other agents will overestimate epistemic uncertainty, causing it to under-delegate rather than over-delegate.
**Source**: Transcript 03:610–689
**Formalized as**: ADR-RESOLUTION-006 in `spec/04-resolution.md`

### UA-005: Four-Class Delegation

**Decision**: Tasks classified: self-handle, consult, delegate, escalate to human.
**Rationale**: The four-class taxonomy arises from the `delegation-classify` function operating on two dimensions: whether qualified resolvers exist (resolver count) and the scalar uncertainty of the entity. When resolvers exist and uncertainty is low, the task is self-handleable; when resolvers exist but uncertainty is high, consultation is needed (contested); when no resolver has sufficient authority but uncertainty is manageable, the task escalates; when no resolver exists and uncertainty is high, human judgment is required. This integrates directly with the conflict routing pipeline, ensuring the system never silently resolves high-stakes conflicts without oversight.
**Source**: Transcript 03
**Formalized as**: ADR-RESOLUTION-007 in `spec/04-resolution.md`

### UA-006: Uncertainty Markers as First-Class Spec Elements

**Decision**: Specification uncertainty marked explicitly with confidence levels (0.0–1.0) and resolution criteria.
**Rationale**: Uncertainty markers were elevated to first-class status because writing aspirational prose that reads like certainty when the spec is not yet settled causes agents to implement uncertain claims as axioms — a critical failure mode. Each element declares a confidence level (0.0–1.0) and states what would resolve the uncertainty. This prevents downstream implementation work that may be discarded when the uncertainty resolves, and connects to the uncertainty tensor's role in the guidance mechanism for consequential divergence prevention.
**Source**: Transcript 07; SEED.md §3
**Formalized as**: ADR-UNCERTAINTY-003 in `spec/15-uncertainty.md`

### UA-007: Observation Staleness Model

**Decision**: Observation datoms carry metadata: `:observation/entity` (ref), `:observation/source` (keyword: `:filesystem | :shell | :network | :git | :process`), `:observation/path` (string), `:observation/timestamp`, `:observation/hash`, `:observation/stale-after`. ASSEMBLE applies freshness-mode (`:warn` default, `:refresh`, `:accept`).
**Rationale**: The observation staleness model addresses the fundamental boundary problem: the datom store claims to be the protocol runtime but POSIX-layer reality can diverge from the datoms that represent it. Each observation carries a configurable stale-after horizon, and three freshness modes provide graduated responses — warning by default, triggering re-observation when freshness is critical, or silently accepting for rarely-changing entities. This maintains the queryability benefits of the datom-store-as-runtime model while being honest about which parts of reality the store might be wrong about.
**Source**: Transcript 04:2155–2188
**Formalized as**: ADR-HARVEST-005 in `spec/05-harvest.md`

### UA-008: Self-Referential Measurement Exclusion (INV-MEASURE-001)

**Decision**: When computing σ_c for entity e, MUST exclude uncertainty measurements targeting e itself from the dependent set. Without this exclusion, the function diverges in self-referential loops. Revised from initial unconditional claim (which was retracted after analysis).
**Formal**: `σ_c(e)` computed over `dependents(e) \ {measurements of e}`.
**Rationale**: Self-referential feedback loops cause oscillation. The initial claim "measurement is always contractive" was self-corrected to a conditional version requiring exclusion.
**Source**: Transcript 02:819–858 (self-correction); Transcript 03:450–469
**Formalized as**: ADR-UNCERTAINTY-004 in `spec/15-uncertainty.md`

### UA-009: Query Stability Score (INV-COMMIT-001)

**Decision**: `stability(R) = min{w(d) : d ∈ F and d contributed to R}` for query result R from facts F. A result with stability ≥ threshold is safe for irrevocable decisions without sync barrier.
**Rationale**: Distinct from crystallization stability (CR-005) — this measures safety of acting on any query result, not just promoting datoms to stable spec.
**Falsification**: Agent makes irrevocable decision based on query result with stability = 0.
**Source**: Transcript 01:704–714
**Formalized as**: ADR-QUERY-011 in `spec/03-query.md`

### UA-010: Contribution Weight by Verification Status

**Decision**: Contributions to the authority graph weighted: 1 (unverified), 2 (witnessed/valid), 3 (challenge-confirmed). Feeds into spectral authority computation (UA-003).
**Rationale**: Creates feedback loop: verified work → more authority → more delegation → more work.
**Source**: Transcript 02:1494–1519
**Formalized as**: ADR-RESOLUTION-012 in `spec/04-resolution.md`

### UA-011: Delegation Safety Invariant (INV-DELEGATE-001)

**Decision**: Agent MUST NOT begin work on spec element e unless `delegatable(e) = true` at agent's local frontier. `delegatable(e) = ∀ a ∈ attributes(e): no conflict, AND stability(e) ≥ delegation_threshold`.
**Falsification**: Agent begins implementing a function whose signature is contested by concurrent planning agents.
**Rationale**: In multi-agent swarms, an implementation agent may begin work on a spec element whose attributes are still contested by concurrent planning agents, only to have the other version win resolution — forcing the implementation to be discarded. The predicate `delegatable(e)` requires both the absence of unresolved conflicts on all attributes of entity `e` and a stability score exceeding a configurable threshold, ensuring implementation only proceeds on settled spec elements. This is a monotonic query (checking for absence of conflict datoms in the LIVE index), so it runs at each agent's local frontier without coordination — aligning with the Option 3A default query mode.
**Source**: Transcript 01:1036–1062
**Formalized as**: ADR-RESOLUTION-008 in `spec/04-resolution.md`

### UA-012: Resolution Capacity Monotonicity (INV-RESOLUTION-001)

**Decision**: When `uncertainty(e)` increases, the set of agents with authority to resolve conflicts on e MUST NOT shrink. `∀ t1 < t2: uncertainty(e,t1) < uncertainty(e,t2) ⟹ resolvers(e,t2) ⊇ resolvers(e,t1)`. Topology-agnostic: in hierarchy, higher-level agents added; in swarm, quorum increases; in market, reputation threshold decreases.
**Rationale**: Revised from retracted INV-CASCADE-001 (which mandated hierarchical escalation). The revision is topology-agnostic per PD-005.
**Source**: Transcript 01:1096–1113 (retracted), 01:1196–1215 (revised)
**Formalized as**: ADR-RESOLUTION-010 in `spec/04-resolution.md`

---

## Conflict & Resolution Decisions

### CR-001: Conservative Conflict Detection

**Decision**: Conservative — flags potential conflicts even when uncertain. `INV-CONFLICT-CONSERVATIVE-001`: detected conflicts at any local frontier MUST be a superset of conflicts at the global frontier. `conflicts(frontier_local) ⊇ conflicts(frontier_global)`.
**Proof sketch**: Causal-ancestor relation is monotonically growing. Learning about new causal paths can only resolve apparent concurrency, never create it. Agent may waste effort on phantom conflicts (safe) but never miss a real one (critical).
**Rationale**: Conflict detection is conservative by design because the causal-ancestor relation is monotonically growing — learning about new causal paths can only resolve apparent concurrency (turning concurrent pairs into causally-ordered pairs), never create new concurrency. An agent may waste effort resolving a phantom conflict (a pair that appears concurrent locally but is actually causally ordered), but the resolution produces a redundant datom that is harmlessly deduplicated on merge. Under-detection — where real conflicts are silently missed — would be catastrophic, so the conservative bias ensures the system never misses a real conflict at the cost of occasional false positives.
**Source**: Transcript 02; Transcript 03:742–761
**Formalized as**: ADR-RESOLUTION-003 in `spec/04-resolution.md`

### CR-002: Three-Tier Conflict Routing

**Decision**: (1) Automatic (low severity — lattice/LWW per attribute), (2) Agent-with-notification (medium), (3) Human-required (high — blocks). Severity = `max(w(d₁), w(d₂))`.
**Rationale**: A single resolution mechanism cannot serve all conflict types: lattice-resolvable conflicts (e.g., lifecycle state progressions) need no human involvement, while high-commitment-weight conflicts that cascade through the dependency graph require human judgment. The severity metric uses commitment weight rather than conflict count because commitment weight measures downstream impact — a conflict on an entity with many dependents is structurally more dangerous than a conflict on a leaf entity, regardless of how many agents disagree.
**Source**: Transcript 02; Transcript 04:1331–1342
**Formalized across**: INV-RESOLUTION-007, ADR-RESOLUTION-004 in `spec/04-resolution.md`

### CR-003: Conflict Detection and Routing as Datom Cascade

**Decision**: System: (1) asserts Conflict entity, (2) computes severity, (3) routes, (4) fires TUI, (5) updates uncertainty, (6) invalidates caches. ALL steps produce datoms.
**Rationale**: The critical design choice is that all six steps in the conflict cascade produce additional datoms — the conflict detection, severity assessment, and routing decision are themselves recorded in the store. This makes the entire resolution process auditable, queryable, and usable as precedent for future conflicts, embodying the principle that mediation metadata is itself data.
**Source**: Transcript 04:1331–1342
**Formalized as**: ADR-SIGNAL-002 in `spec/09-signal.md`

### CR-004: Deliberation, Position, and Decision as First-Class Entity Types

**Decision**: Deliberation (process), Position (stance: `:advocate | :oppose | :neutral | :synthesize`), Decision (method: `:consensus | :majority | :authority | :human-override | :automated`). Deliberation history is a case law system.
**Invariant**: `INV-DELIBERATION-BILATERAL-001`: supports both forward and backward flow with identical entity structure.
**Rationale**: Deliberation, Position, and Decision are modeled as first-class entity types because recording the reasoning process by which conflicts are resolved — not merely the resolution — captures valuable audit data and creates queryable precedent. The Position entity includes structured stance/evidence/rebuttal fields, while the Decision entity records method, rationale, dissents, and revisitation conditions. This makes deliberation history a case law system for spec development, enabling precedent queries that inform current decisions.
**Source**: Transcript 04:745–828
**Formalized across**: INV-DELIBERATION-001, ADR-DELIBERATION-001, ADR-DELIBERATION-002 in `spec/11-deliberation.md`; ADR-RESOLUTION-005 in `spec/04-resolution.md`

### CR-005: Crystallization Stability Guard

**Decision**: Datom carries `:stability-min` guard. Cannot crystallize until stability score exceeds threshold (default 0.7). Conditions: status `:refined`, thread `:active`, parent confidence ≥ 0.6, coherence ≥ 0.6, no unresolved conflicts. Defense against premature crystallization: `:stability-min` as Datalog pre-filter.
**Rationale**: The crystallization readiness query contains a `not` clause (no unresolved conflicts), making it non-monotonic — an agent that has not yet learned about a conflict might incorrectly conclude a finding is ready to crystallize. This premature crystallization failure mode is the most dangerous in the system because it promotes provisional knowledge to committed spec elements. The `:stability-min` guard operates as a Datalog pre-filter that excludes provisional facts from joins entirely (not just from output), ensuring only findings built on well-established facts are considered for crystallization.
**Source**: Transcript 02; Transcript 03:942–990
**Formalized across**: INV-DELIBERATION-002, ADR-DELIBERATION-004 in `spec/11-deliberation.md`

### CR-006: Formal Conflict Predicate

**Decision**: `conflict(d1, d2) = d1 = [e a v1 t1 assert] ∧ d2 = [e a v2 t2 assert] ∧ v1 ≠ v2 ∧ cardinality(a) = :one ∧ ¬(t1 < t2) ∧ ¬(t2 < t1)`. Critical: conflict requires causal independence — if one tx precedes the other, it is an update, not a conflict.
**Rationale**: The conflict predicate was formalized to enable structural mutual-exclusivity detection in the parallel planning scenario, where multiple agents independently crystallize spec additions that may contradict each other. The causal independence condition `¬(t1 < t2) ∧ ¬(t2 < t1)` distinguishes genuine conflicts from simple updates (where the later datom supersedes). This maps detection to the structural domain (matching entity-attribute pairs with divergent concurrent values) rather than the semantic domain (NLP over invariant text), making Tier 1 conflict detection exact rather than heuristic.
**Source**: Transcript 01:998–1011
**Formalized as**: INV-RESOLUTION-004 in `spec/04-resolution.md`

### CR-007: Precedent Query Pattern for Deliberations

**Decision**: Concrete Datalog query pattern `find-precedent` locates past deliberations relevant to a current conflict by matching entity type and contested attributes.
**Rationale**: Makes deliberation history a "case law system" — past decisions inform future conflicts.
**Source**: Transcript 04:798–828
**Formalized across**: INV-DELIBERATION-003, ADR-DELIBERATION-003 in `spec/11-deliberation.md`

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

### CR-010: Causal Independence via Predecessor Graph, Not HLC

**Decision**: Causal independence in the conflict predicate is determined by reachability in the `causal_predecessors` graph (BFS/DFS over the transitive closure), NOT by HLC timestamp comparison. Two transactions T1, T2 are causally independent iff neither is transitively reachable from the other via predecessor links: `¬(T1 ≺ T2) ∧ ¬(T2 ≺ T1)`.
**Rationale**: HLC provides a total order (wall_time, logical, agent). Under a total order, `¬(t1 < t2) ∧ ¬(t2 < t1)` reduces to `t1 = t2`, making the conflict predicate trivially unsatisfiable — it would never detect conflicts between different transactions. The causal predecessor graph is a partial order that correctly distinguishes concurrent transactions (conflicts) from causally ordered ones (updates). This is Lamport's happens-before relation applied to the datom store.
**Alternatives rejected**: (A) HLC comparison — broken, total order can't express concurrency; (C) Vector clocks — requires fixed agent set, incompatible with Braid's open-agent model; (D) Interval tree clocks — complex, unfamiliar, marginal benefit over predecessor graph.
**Source**: V1 Audit (Agent 10 algebraic audit), R2.3 (brai-2nl.3)
**Formalized as**: INV-STORE-010 clarification (R2.3) in `spec/01-store.md`, verification harness §10.7.9 in `docs/guide/10-verification.md`

### CR-011: Conflict Pipeline Progressive Activation

**Decision**: At Stage 0, the conflict pipeline is a simplified two-tier system: (1) detect conflicts via the causal predecessor graph, (2) route all conflicts to multi-value resolution. The full three-tier pipeline (conservative detection → severity scoring → routing to LWW/lattice/multi/delegation) activates progressively: severity scoring at Stage 1, delegation routing at Stage 2. This avoids building the full routing infrastructure before it's needed.
**Rationale**: The full conflict pipeline involves three tiers, but Stage 0 only needs to prove the core datom store and merge semantics work. Building the full routing infrastructure before it is exercised would produce speculative scaffolding that violates NEG-001 (no aspirational stubs). The simplified two-tier system (detect conflicts, route all to multi-value) is sufficient for Stage 0 correctness; severity scoring and delegation routing activate only when the use cases that require them actually exist.
**Source**: Session 014 (Stage 0 simplification decisions)
**Formalized as**: ADR-RESOLUTION-013 in `spec/04-resolution.md`

---

## Agent Architecture Decisions

### AA-001: Dual-Process Architecture Is Protocol-Level

**Decision**: System 1 (associative retrieval) + System 2 (LLM reasoning) is a protocol-level requirement. The two-phase retrieval (ASSOCIATE → QUERY → ASSEMBLE) is first-class.
**Formal**: `assemble ∘ query ∘ associate : SemanticCue → BudgetedContext`.
**Rejected**: Context assembly as application-level (flat-buffer pathology).
**Rationale**: Convergence analysis revealed a critical gap: the protocol defined how agents transact and query the store, but left context assembly — going from millions of datoms to the right 8,000 tokens — as an application-level concern. The key insight was "System 2 doesn't know what System 1 didn't surface; a brilliant reasoner with a bad retrieval policy is always solving the wrong problem." Leaving context assembly to each agent implementation was rejected because it would reproduce the flat-buffer pathology where agents load context without structural awareness. Making the two-phase retrieval a first-class protocol operation ensures every agent benefits from structured context assembly.
**Source**: Transcript 03; Transcript 04:31–46
**Formalized as**: ADR-QUERY-010 in `spec/03-query.md`

### AA-002: Revised Agent System Formalism — D-Centric

**Decision**: `(D, Op_D, Obs_D, A, π, Σ, Γ)` — all operations reference the datom store D. POSIX runtime R is below protocol boundary.
**Rationale**: The agent system formalism was revised from the original tripartite (E, R, A) decomposition to place the datom store D at the center. The key shift is that operations and observations no longer refer to the POSIX runtime but to the datom store — the agent's entire interface to the world is through store operations. If an agent needs to read a file, the contents are either already datoms or the agent issues a TRANSACT recording the read as an observation datom, pushing runtime mediation below the protocol boundary.
**Source**: Transcript 04:461–476
**Formalized as**: ADR-FOUNDATION-003 in `spec/00-preamble.md`

### AA-003: Store-as-Protocol-Runtime — Three-Layer Architecture

**Decision**: (1) Protocol Layer — all coordination as datoms, (2) Observation Interface — tools translating R-state to datoms, (3) Execution Layer — POSIX runtime, opaque.
**Formal**: Observation functor `observe : R-State → [Datom]` with three properties: (1) Idempotent — same R-state produces equivalent datoms modulo tx-id; (2) Monotonic — only ADDs datoms (file deletion = retraction datom, not deletion); (3) Lossy — selective observation per information-value criterion. The strong-form invariant `INV-STORE-AS-RUNTIME-001` was revised to the weaker eventually-consistent `INV-STORE-AS-RUNTIME-002` in the three-layer architecture.
**Rationale**: The three-layer architecture resolves the tension between the ideal that "the store is the runtime" and the practical reality that POSIX filesystem, processes, and networks lack CRDT semantics. The protocol layer is a closed algebraic structure with full CRDT and CALM guarantees. The execution layer is opaque except through the observation interface — a functor that translates R-state into datoms with idempotent, monotonic, and lossy properties. The strong version (all tool results must become datoms first) was rejected as operationally wasteful; instead, the boundary admits that observations are selective and may become stale.
**Source**: Transcript 04:2086–2189
**Formalized as**: NEG-INTERFACE-001 in `spec/14-interface.md`

### AA-004: Metacognitive Layer — Four Entity Types

**Decision**: Belief, Intention, Learned Association (type: `:causal | :correlative | :architectural | :strategic | :analogical`), Strategic Heuristic.
**Invariant**: `INV-ASSOCIATE-LEARNED-001`: ASSOCIATE MUST traverse learned associations alongside structural edges.
**Rationale**: The metacognitive layer introduces four entity types so agents write their own epistemic state into the store — not just what they observe but what they believe, intend, and have learned. Making intentions and beliefs first-class datoms enables structural detection of goal dilution: a Datalog query can identify when an agent's recent transactions are unrelated to its stated Intention entity, firing a GoalDrift signal. The Learned Association and Strategic Heuristic types accumulate transferable knowledge across agents and sessions with empirical success/failure feedback.
**Source**: Transcript 04:1100–1179
**Formalized as**: ADR-SEED-005 in `spec/06-seed.md`

### AA-005: Intention Anchoring — Anti-Goal-Dilution

**Decision**: Intentions pinned at level 0 regardless of budget pressure when `include_intentions=true`.
**Invariant**: `INV-ASSEMBLE-INTENTION-001`.
**Rationale**: Intention anchoring requires that active Intention entities be included at pyramid level 0 (full detail) regardless of budget pressure — the anti-goal-dilution mechanism. The agent's stated goals are pinned in the assembled context and cannot be evicted by lower-priority material during budget-constrained compression. This directly addresses the problem where agents progressively lose sight of their original goals as context fills with incidental material.
**Source**: Transcript 04:1506–1513
**Formalized as**: INV-SEED-006 in `spec/06-seed.md`

### AA-006: Ten Named Protocol Operations

**Decision**: TRANSACT, QUERY, ASSOCIATE, ASSEMBLE, BRANCH, MERGE, SYNC-BARRIER, SIGNAL, SUBSCRIBE, GUIDANCE (10 total).
**Rationale**: The initial protocol identified eight operations, but convergence analysis revealed ASSOCIATE and ASSEMBLE were just as fundamental as TRANSACT and QUERY — current agent systems get read/write right but context assembly is vestigial or absent, which is why agents plateau regardless of LLM capability. The list evolved to ten: BRANCH was added for the bilateral diverge-compare-converge pattern, SUBSCRIBE for push-model TUI notifications, and GUIDANCE for the queryable comonadic action topology. The final ten span the complete agent lifecycle: write, read, discover, assemble, branch, synchronize, coordinate, and plan.
**Source**: Transcript 03; Transcript 04
**Formalized as**: ADR-INTERFACE-006 in `spec/14-interface.md`

### AA-007: System 1/System 2 Diagnosis

**Decision**: Generic/hedging output diagnosed as S1/S2 mismatch — retrieval failure, not reasoning failure. Fix: better ASSOCIATE configuration.
**Rationale**: The diagnosis reframes generic or hedging agent output not as a reasoning failure but as a retrieval failure — the agent's deliberative capacity (System 2) is intact, but its fast-access substrate (System 1) cannot find relevant prior context within the current context window. The fix is improving ASSOCIATE configuration so the right knowledge is surfaced, giving System 2 the material it needs. This was preferred over treating the symptom (e.g., prompt-engineering for specificity) because it identifies the structural root cause: retrieval limits, not reasoning limits, are why agents plateau.
**Source**: Transcript 06
**Formalized as**: ADR-GUIDANCE-007 in `spec/12-guidance.md`

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
**Rationale**: The three output modes serve three distinct consumers. Structured mode (JSON) serves MCP/agent tool processing. Human mode (colored terminal) serves direct use. Agent mode returns output shaped for LLM consumption following prompt optimization principles: headline, relevant entities at budget-appropriate pyramid levels, signals, guidance items, and depth pointers. This maps to the dual-process architecture: items 1-2 are System 1 output (cheap, pattern-matchable), signals are the confusion channel, guidance is the comonad, and pointers are the depth escape hatch.
**Source**: Transcript 04:2508–2574
**Formalized as**: ADR-INTERFACE-002 in `spec/14-interface.md`

### IB-003: MCP as Thin Wrapper with Six Tools (Stage 0)

**Decision**: MCP server calls kernel functions directly via library-mode integration (ADR-INTERFACE-004). Not the subprocess model from Transcript 04 — the library model was chosen because C1/C4 make the Store safe to share in-process. Store held via `ArcSwap<Store>` (Datomic connection model): immutable Store values, atomic pointer swap on writes. Both CLI and MCP dispatch to the same kernel functions (INV-INTERFACE-010). CLI remains the universal interface; MCP is an optimization. Stage 0 exposes exactly six tools: `braid_transact` (meta), `braid_query` (moderate), `braid_status` (cheap), `braid_harvest` (meta), `braid_seed` (expensive), `braid_guidance` (cheap). Entity lookup and history are accessible via `braid_query`; CLAUDE.md generation is accessible via `braid_guidance`. Three additional tools (`braid_branch`, `braid_signal`, `braid_associate`) activate at Stage 2+ when branching and multi-agent coordination are available. On every call: loads Store snapshot from ArcSwap, reads context state, computes Q(t), passes budget to kernel, appends notifications, updates session state, checks thresholds.
**Rationale**: The MCP server is a thin wrapper over the CLI rather than an independent implementation because the system must work without MCP — an agent using only bash calls to `ddis` still gets budget-aware output, guidance, and the full protocol. MCP adds exactly one capability the CLI cannot: session state persisting across tool calls (k* tracking, proactive notifications, automatic budget adjustment). MCP-only was rejected (loses human accessibility, scriptability); CLI-only was insufficient (cannot track intra-conversation dynamics).
**Source**: Transcript 04:2578–2641; Transcript 05:792–900 (original nine tool schemas); revised to six tools for Stage 0 per INV-INTERFACE-003; library model per ADR-INTERFACE-004
**Formalized as**: ADR-INTERFACE-004 in `spec/14-interface.md`

### IB-004: CLI Output Budget as Hard Invariant

**Decision**: `--budget <tokens>` flag on every command. Five-level precedence for budget determination: (1) `--budget` flag, (2) `--context-used` flag, (3) session state file `.ddis/session/context.json`, (4) transcript tail-parse (fallback), (5) conservative default 500 tokens. Staleness threshold: 30 seconds.
**Invariant**: `INV-CLI-BUDGET-001`, `INV-INTERFACE-BUDGET-001` (output capped at `max(MIN_OUTPUT, Q(t) × W × budget_fraction)`).
**Rationale**: Every CLI command must accept a `--budget` flag that caps output size because the CLI's output directly enters the agent's context window, and unconstrained output consumes the agent's finite attention budget (k*). This is a hard invariant rather than advisory because budget enforcement is the structural mechanism that prevents attention exhaustion. The five-level precedence hierarchy ensures budget is always determinable even when the agent provides no explicit value.
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
**Rationale**: Guidance compression is parameterized by k*_eff to match output richness to the agent's remaining context capacity. At high k*_eff, guidance includes full invariant context, uncertainty details, and precedent references. At mid k*_eff, it compresses to headline plus recommended action. At critically low k*_eff, it instructs the agent to harvest and reset. This connects the comonadic guidance structure to the budget constraints: the `extend` operation is parameterized by k*_eff, producing deep lookahead when budget is available and single actions when it is not.
**Source**: Transcript 04:2678–2707
**Formalized as**: INV-BUDGET-004 in `spec/13-budget.md`

### IB-007: CLI Command Taxonomy by Attention Profile

**Decision**: CHEAP (≤50: status, guidance, frontier, branch ls), MODERATE (50–300: associate, query, assemble, diff), EXPENSIVE (300+: assemble --full, seed), META (side effects: harvest, transact, merge).
**Rationale**: Agents need to make informed decisions about which commands to invoke based on remaining k* budget. The taxonomy classifies commands by attention cost: cheap commands are always safe, moderate commands are safe early/mid conversation, expensive commands are budget-gated, and meta commands produce side effects without consuming context. The same query with different budget parameters returns differently compressed output, demonstrating budget-aware compression of identical operations.
**Source**: Transcript 04:2819–2862
**Formalized as**: INV-BUDGET-005 in `spec/13-budget.md`

### IB-008: TUI as Subscription-Driven Push Projection

**Decision**: Continuously-updated projection via SUBSCRIBE. NOT k*-constrained.
**Invariant**: `INV-TUI-LIVENESS-001`: delegation changes and conflicts above threshold trigger notification.
**Rationale**: The TUI is subscription-driven rather than polling-based because the human needs to see what agents are doing without asking — a push model where the TUI subscribes to datom event patterns and renders them in real-time. The INV-TUI-LIVENESS-001 guarantee prevents the failure mode where conflicts are resolved by automated means while the human is unaware they existed, ensuring delegation changes, high-severity conflicts, and deliberation decisions always trigger notifications.
**Source**: Transcript 04:1024–1096
**Formalized as**: INV-INTERFACE-005 in `spec/14-interface.md`

### IB-009: Human-to-Agent Signal Injection via TUI

**Decision**: Human injects signal from TUI, delivered via MCP notification queue in agent's next tool response. Also recorded as datom.
**Rationale**: Human-to-agent signaling completes the bidirectional feedback loop: subscriptions push store state to the human (agent-to-human), and signal injection pushes human intent back to agents (human-to-agent). When the human observes something requiring attention (an agent drifting, a bad resolution), they inject a signal that enters the relevant agent's next tool response, mediated by the MCP server which queues the signal for delivery.
**Source**: Transcript 04:2724–2737
**Formalized as**: INV-INTERFACE-006 in `spec/14-interface.md`

### IB-010: Store-Mediated Trajectory Management

**Decision**: `ddis harvest` extracts durable facts; `ddis seed` generates carry-over. Agent lifecycle: SEED → work 20–30 turns → HARVEST → reset → GOTO SEED.
**Invariant**: `INV-TRAJECTORY-STORE-001`: Seed output five-part template: (1) Context (1–2 sentences), (2) Invariants established, (3) Artifacts, (4) Open questions from deliberations, (5) Active guidance. Formatted as spec-first seed turn.
**Rationale**: The datom store solves trajectory management by construction: each conversation is a bounded trajectory that starts with a store-assembled seed and ends with a store-harvested assertion of durable facts. Carry-over is basin-neutral (datoms, not conversation fragments), spec-flavored (invariants and formal structure), minimal (budget-constrained), and free of conversation artifacts. The store grows monotonically across conversations while conversations remain short, fresh, and seed-activated.
**Source**: Transcript 04:2742–2812
**Formalized across**: ADR-INTERFACE-003 in `spec/14-interface.md`; ADR-SEED-004 in `spec/06-seed.md`; ADR-TRILATERAL-001, ADR-TRILATERAL-003 in `spec/18-trilateral.md`

### IB-011: Rate-Distortion Interface Design

**Decision**: Interface is formally a rate-distortion channel: maximize information value while minimizing attention cost.
**Rationale**: An LLM agent has a finite, decaying attention budget (k*) and the datom store may contain thousands of datoms — the naive "dump all results" approach would destroy k* within a few cycles. The formal constraint requires that every tool invocation's attention cost stay within a fraction of remaining k*_eff, while delivering enough information for the agent's next decision. No alternative interface model (fixed verbosity, human-length output, or separate verbose/terse modes) was considered adequate because none adapt continuously to the agent's remaining reasoning capacity.
**Source**: Transcript 05
**Formalized as**: ADR-SEED-002 in `spec/06-seed.md`

### IB-012: Proactive Harvest Warning (INV-INTERFACE-HARVEST-001)

**Decision**: When Q(t) < 0.15 (~75% consumed), every response includes harvest warning. When Q(t) < 0.05 (~85%), CLI emits ONLY the harvest imperative.
**Rationale**: Continuing past harvest threshold produces diminishing returns — outputs become parasitic.
**Source**: Transcript 05:1037–1048
**Formalized across**: INV-HARVEST-005, NEG-HARVEST-001 in `spec/05-harvest.md`; INV-INTERFACE-007, NEG-INTERFACE-003, ADR-INTERFACE-010 in `spec/14-interface.md`

---

### IB-013: Configurable Heuristic Parameters with Progressive Disclosure

**Decision**: All heuristically-driven parameters (betweenness proxy default, turn-count
harvest thresholds, M(t) weights, R(t) routing coefficients, cascade stub behavior,
Value/Stratum/Clause variant activation) are exposed as configurable values with:
1. **Smart defaults** — system works out of the box without configuration.
2. **Progressive disclosure** — casual users see nothing; `braid config show` shows
   current values; `braid config set` modifies them; expert users can tune all
   parameters via a typed configuration schema.
3. **Portability** — configuration is stored as datoms in the store (schema-as-data,
   C3), surviving harvest/seed cycles and merge operations.
4. **Ergonomic access** — CLI `braid config` subcommand with tab completion, MCP
   `braid_config` tool, and CLAUDE.md guidance footer references when parameters
   affect behavior.
**Rationale**: Stage 0 introduces multiple heuristic proxies (betweenness=0.5,
harvest-warn-at-turn=20, harvest-imperative-at-turn=40) that will need tuning during
real usage. Hard-coding these values forces spec edits for operational changes. Making
them configurable datoms means they participate in the store's append-only, mergeable,
queryable infrastructure. Progressive disclosure prevents casual-user overwhelm while
enabling expert optimization.
**Alternatives rejected**: (A) Hard-coded constants — no tuning without code changes.
(B) Environment variables — not portable across sessions, not stored in datoms.
(C) TOML/YAML config file — external to the store, doesn't participate in merge/harvest.
**Invariant**: `INV-GUIDANCE-007` (dynamic CLAUDE.md should reflect current config),
`INV-INTERFACE-001` (CLI modes expose configuration).
**Source**: User directive (Session 012, 2026-03-04) — "things which are heuristically
driven should be configurable, portable, exposed to the user, intuitive and ergonomic
with smart defaults and progressive disclosure."
**Formalized as**: ADR-INTERFACE-005 in `spec/14-interface.md`

---

## Guidance System Decisions

### GU-001: Guidance Topology as Comonad

**Decision**: `(W, extract, extend)` where `W(A) = (StoreState, A)`. Guidance nodes are entities with query-driven lookup. Agents can write new guidance nodes.
**Invariant**: `INV-GUIDANCE-EVOLUTION-001`: learned guidance flagged, effectiveness updated empirically, below 0.3 threshold SHOULD be retracted.
**Rationale**: Guidance is formalized as a comonad `(StoreState, [Action])` because the comonadic structure ensures guidance is always relative to a context — there is no "global best action," only "best action given the current store state." The `extend` operation applies a scoring function to every reachable state, producing a scored guidance map. Guidance nodes are stored as datoms rather than hardcoded, enabling agents to write new guidance nodes when they discover effective sequences and self-correct based on empirical outcomes, making the topology a learning, evolving structure.
**Source**: Transcript 04:857–962
**Formalized as**: ADR-GUIDANCE-001 in `spec/12-guidance.md`

### GU-002: Guidance Lookahead via Branch Simulation

**Decision**: Lookahead (1–5 steps) via virtual branch simulation. "Planning as branch simulation."
**Rationale**: The comonadic `extend` operation is implemented as branch simulation: for each available action, the system speculatively applies it to a hypothetical store state (a virtual branch) and recursively evaluates the resulting guidance. The simulation does not execute actions against the real runtime but explores hypothetical outcomes to score available moves. Recursion is bounded by a lookahead depth parameter, making the computation tractable while providing multi-step planning.
**Source**: Transcript 04:1843–1855
**Formalized across**: INV-GUIDANCE-006, NEG-GUIDANCE-002 in `spec/12-guidance.md`

### GU-003: Guidance Is the Seed Turn — Spec-Language Phrasing

**Decision**: Guidance MUST use spec-language (invariants, formal structure), NOT instruction-language (steps, checklists).
**Invariant**: `INV-GUIDANCE-SEED-001`.
**Rationale**: Guidance must use spec-language (invariants, formal structure, domain terms) rather than instruction-language (steps, checklists, procedures) because empirical research showed seed turns producing formal/principled reasoning improved subsequent output quality (p=0.029). Good guidance names invariants (activating formal reasoning), identifies uncertainty (focusing attention), and poses questions (high degrees of freedom) rather than prescribing steps (mid-DoF saddle zone where output becomes generic). This connects guidance generation directly to prompt optimization: the guidance output serves exactly as a seed turn.
**Source**: Transcript 04:2647–2673; Transcript 05
**Formalized as**: ADR-GUIDANCE-004 in `spec/12-guidance.md`, ADR-SEED-003 in `spec/06-seed.md`

### GU-004: Dynamic CLAUDE.md Generation

**Decision**: CLAUDE.md dynamically generated from empirical drift patterns. Collapses three concerns: (1) Ambient awareness (Layer 0 — CLAUDE.md IS the ambient awareness), (2) Guidance (Layer 3 — seed context IS the first guidance, pre-computed, zero tool-call cost), (3) Trajectory management (CLAUDE.md IS the seed turn). "One mechanism, three problems solved."
**Rationale**: Static CLAUDE.md was rejected because it wastes k* on irrelevant instructions and cannot adapt to observed drift patterns. With dynamic generation, the system learns from empirical drift data — which observations went un-transacted, which guidance was ignored — and generates targeted behavioral corrections specific to the current project, task, and agent. The self-improvement loop (measure drift, generate correction, measure effect, refine correction) means the system prompt converges on the minimal set of reminders that prevent the most common failure modes.
**Source**: Transcript 05; Transcript 06:209–232
**Formalized as**: ADR-SEED-001 in `spec/06-seed.md` (three-concern collapse)

### GU-005: Guidance Injection — Recency Effect Exploitation (INV-GUIDANCE-INJECTION-001)

**Decision**: Every CLI/MCP response MUST include guidance footer specifying next methodologically-correct action. Footer MUST: (a) name specific ddis command, (b) reference active invariants, (c) note uncommitted observations, (d) warn if drifting. Token cost included in budget (high Q(t): ~100; low Q(t): ~15).
**Rationale**: Model's most recent tool output is strongest non-system-prompt influence. Guidance footer exploits recency effect for continuous Basin A re-seeding.
**Source**: Transcript 05:1302–1346
**Formalized across**: INV-GUIDANCE-001, NEG-GUIDANCE-001 in `spec/12-guidance.md`

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

### GU-008: Guidance-Intention Coherence (INV-GUIDANCE-ALIGNMENT-001)

**Decision**: Actions scored higher if they advance active intentions: `if postconditions(a) ∩ goals(i) ≠ ∅: score(a) += intention_alignment_bonus`.
**Rationale**: The intention alignment bonus structurally links the guidance scoring function to the Intention entities in the store, ensuring the comonadic guidance topology inherently favors actions that advance declared objectives over tangential explorations. This directly prevents the goal dilution problem where an agent progressively drifts from its original purpose, by making intention-alignment a first-class term in the guidance score.
**Source**: Transcript 04:1858–1875
**Formalized across**: INV-GUIDANCE-003, INV-GUIDANCE-009, INV-GUIDANCE-010, ADR-GUIDANCE-005 in `spec/12-guidance.md`

### GU-009: Unified Guidance as M(t) x R(t) x T(t)

**Decision**: Guidance composes three independently falsifiable scores: M(t) methodology adherence, R(t) graph-based work routing, and T(t) topology fitness. Each has its own invariant (INV-GUIDANCE-008, 010, 011), uses data-driven weights stored as datoms, and is computed at its designated stage (M(t) and R(t) at Stage 0, T(t) at Stage 2).
**Rationale**: Independent scores enable independent verification and independent evolution. A composite score hides which component failed. Hierarchical gating creates artificial dependencies — M(t) being low shouldn't prevent R(t) from routing to the right task. The tensor product preserves each component's information while enabling composition in the comonadic footer.
**Rejected**: (A) Single composite score (hides failure component); (C) Hierarchical gating (creates artificial dependencies).
**Source**: ADRS GU-006, GU-007, GU-008
**Formalized as**: ADR-GUIDANCE-005 in `spec/12-guidance.md`

### GU-010: Guidance Footer Progressive Enrichment

**Decision**: At Stage 0, guidance footers use static patterns (methodology score M(t) with default weights, task list from LIVE store queries). Progressive enrichment activates at Stage 1 (budget-compressed sections via Q(t)) and Stage 2 (branch comparison summaries, topology fitness T(t)). This allows Stage 0 to ship guidance without the full Q(t)/T(t) machinery.
**Rationale**: The guidance system's full specification requires three signal sources — M(t), Q(t), and T(t) — but Q(t) depends on budget-aware output machinery (Stage 1) and T(t) depends on branching and multi-agent topology (Stage 2), neither of which exists at Stage 0. Rather than building stubs, Stage 0 ships guidance using only static patterns, validating the core hypothesis that guidance improves agent behavior without blocking on the full signal pipeline.
**Source**: Session 014 (Stage 0 simplification decisions)
**Formalized as**: ADR-GUIDANCE-008 in `spec/12-guidance.md`

### GU-011: Betweenness Proxy via Degree Product

**Decision**: At Stage 0, full betweenness centrality (O(V·E)) is replaced by a degree-product proxy: `proxy_betweenness(v) = in_degree(v) × out_degree(v)`. This captures the bridge-node intuition (nodes connecting many inputs to many outputs) at O(V) cost. Full Brandes algorithm activates at Stage 1 when the graph engine supports it.
**Rationale**: Full betweenness centrality (Brandes algorithm, O(V·E)) is specified for identifying bridge nodes in the guidance graph — datoms connecting many consumers to many producers. At Stage 0, the graph engine does not yet support the full algorithm, and the datom graph is small enough that the degree-product proxy captures the same structural intuition at O(V) cost, deferring the full Brandes implementation to Stage 1 when graph size justifies the computational expense.
**Source**: Session 014 (Stage 0 simplification decisions)
**Formalized as**: ADR-GUIDANCE-009 in `spec/12-guidance.md`

---

## Lifecycle & Methodology Decisions

### LM-001: Braid Is a New Implementation, Not a Patch

**Decision**: Braid replaces the Go CLI; it does not extend or migrate it.
**Rationale**: The Go CLI's gap analysis revealed a fundamental substrate divergence: the existing 62,500 LOC implementation is built on a relational substrate (39-table normalized SQLite, three-stream JSONL events, global LWW merge) whereas Braid requires a datom substrate (content-addressable tuples, schema-as-data, per-attribute resolution, Datalog queries, set-union merge). This divergence pervades every layer — identity, mutability, schema, queries, and merge — making adaptation more costly than rebuilding. The Go CLI proves that DDIS concepts work at scale (it reached F(S) = 1.0), but the implementation strategy is to build the new substrate first, then port proven behavioral concepts.
**Source**: SEED.md §9; docs/HARVEST.md Session 001
**Formalized as**: ADR-FOUNDATION-001 in `spec/00-preamble.md`

### LM-002: Manual Harvest/Seed Before Tools Exist

**Decision**: Methodology precedes tooling.
**Rationale**: DDIS is a methodology first and a toolset second — the tools automate a way of working that can be practiced before the tools exist. The concrete instruction is to harvest manually between sessions by writing down key decisions and carrying them into the next session. The alternative of waiting for tooling was rejected because methodology precedes tooling: establishing the discipline of harvest/seed as a human practice ensures the eventual automation codifies a proven workflow rather than an untested theory.
**Source**: SEED.md §10; Transcript 07
**Formalized as**: ADR-FOUNDATION-002 in `spec/00-preamble.md`

### LM-003: Conversations Are Disposable, Knowledge Is Durable

**Decision**: Bounded conversation trajectories with durable knowledge extraction.
**Rationale**: The current paradigm — fighting to keep conversations alive because losing them means losing knowledge — is the source of most pain: long conversations, manual context management, repetition, and drift. DDIS inverts this by making knowledge live in the datom store rather than inside conversations, so conversations become lightweight, disposable reasoning sessions that can be harvested and discarded without loss. The alternative (bigger context windows, longer conversations) was rejected as "solving a filing problem by getting a bigger desk" — the desk always fills up.
**Source**: SEED.md §5; Transcript 06
**Formalized as**: ADR-HARVEST-002 in `spec/05-harvest.md`

### LM-004: Reconciliation as Unified Taxonomy

**Decision**: All operations are: detect divergence → classify → resolve. Eight types.
**Rationale**: The reconciliation taxonomy emerged from the realization that all protocol operations (harvest, merge, deliberation, guidance, sync, etc.) are not a grab-bag of features but instances of one fundamental operation: detect divergence, classify it, resolve it back to coherence. The taxonomy was constructed by mapping each protocol mechanism to a specific divergence type at a specific boundary, revealing that eight distinct divergence types provide complete coverage of the reconciliation space. This unified framing both validated the protocol design and exposed four specific gaps (CO-007).
**Source**: SEED.md §6; Transcript 07
**Formalized as**: ADR-BILATERAL-005 in `spec/10-bilateral.md`

### LM-005: Semi-Automated Harvest

**Decision**: System proposes harvests from transaction analysis; agent/human confirms.
**Rationale**: Fully automated harvest would assert everything the agent said or did — including tentative hypotheses and debugging tangents — violating the carry-over template's principle of "no conversation fragments." Fully manual harvest breaks down under k* pressure as the agent drifts from transact discipline. Semi-automated gives the best of both: the CLI mechanically extracts candidates by pattern-matching decisions, invariants, and dependencies, then a human or agent reviews before anything is committed. The two-step pipeline (propose then confirm) ensures the quality gate prevents noise from becoming "facts."
**Source**: Transcript 05
**Formalized as**: ADR-HARVEST-001 in `spec/05-harvest.md`

### LM-006: Harvest Calibration with FP/FN Tracking (INV-HARVEST-LEARNING-001)

**Decision**: Track empirical quality: committed candidate later retracted = false positive; rejected candidate re-discovered = false negative. High FP → raise thresholds; high FN → lower; both → improve extractor. Harvest as drift metric: 0–2 uncommitted = excellent; 3–5 = minor drift; 6+ = significant.
**Invariant**: `INV-HARVEST-DIAGNOSTIC-001`: uncommitted count stored as datom per session.
**Rationale**: Harvest thresholds cannot be set correctly a priori — they must be tuned empirically from observed outcomes. A committed candidate later retracted is a false positive (review too permissive); a rejected candidate later re-discovered and committed is a false negative (review too conservative). Tracking these rates enables principled threshold adjustment, and the uncommitted observation count per session serves as a drift metric revealing methodology adherence trends. This feedback loop means the harvest system improves with use.
**Source**: Transcript 05:1507–1520, 05:2011–2031
**Formalized as**: ADR-HARVEST-003 in `spec/05-harvest.md`

### LM-007: Datom-Exclusive Information

**Decision**: All durable information must exist as datoms. External representations are projections.
**Rationale**: The honest mitigation for the agent compliance problem is not technical but structural: if specification invariants, design decisions, dependency graphs, and task assignments exist ONLY as datoms and are never duplicated in markdown files, then the agent genuinely cannot do its job without querying the store. Allowing knowledge to exist in both datoms and filesystem files provides a parallel path (Basin B escape route) that bypasses DDIS entirely. Implementation code lives in files, but knowledge ABOUT that code must live exclusively as datoms.
**Source**: Transcript 05
**Formalized as**: ADR-STORE-019 in `spec/01-store.md`

### LM-008: Self-Bootstrap Fixed-Point Property

**Decision**: When system manages its own spec, spec IS data. Converges when spec-as-data and spec-as-document agree. The specification PROCESS generates the first test data (invariants about the store become the store's test cases; contradictions caught during spec become contradiction-detection test cases).
**Rationale**: Three converging arguments drove the self-bootstrap decision: integrity (specifying a coherence system using a methodology that can't maintain coherence would violate its own thesis at the moment of articulation), bootstrapping (the specification elements become the first dataset the system manages), and validation (if DDIS can't spec DDIS, it can't spec anything). The alternative of using traditional markdown prose was rejected because it would decouple the plan from the data at exactly the moment they should be unified.
**Source**: Transcript 07
**Formalized as**: ADR-FOUNDATION-006 in `spec/00-preamble.md`

### LM-009: Specification Documents Use DDIS Structure

**Decision**: Invariants, ADRs, negative cases, uncertainty markers.
**Rationale**: Rather than producing a traditional monolithic SPEC.md, the specification mandates DDIS structure where every element has an ID, a type (invariant/ADR/negative-case/section), explicit traceability to the seed, and a falsification condition. The format is markdown because the datom store does not yet exist, but the structure is DDIS so migration to datoms is mechanical when the store exists. Driven by the self-bootstrap commitment: the specification process itself generates the first real test data for the system.
**Source**: Transcript 07
**Formalized as**: ADR-FOUNDATION-004 in `spec/00-preamble.md`

### LM-010: Explicit Residual Divergence

**Decision**: Unresolvable divergence recorded explicitly with uncertainty marker.
**Rationale**: When divergence cannot be fully resolved — because genuine tradeoffs exist, information is incomplete, or priorities conflict — the system must not pretend it does not exist. Instead, it records the residual, documents why it persists, and tracks what resolving it would require. The system's purpose is to drive divergence toward zero or toward an explicitly acknowledged and documented residual. This distinguishes DDIS from every other approach: residuals are visible and queryable rather than hidden and discovered by accident.
**Source**: Transcript 07
**Formalized as**: ADR-BILATERAL-008 in `spec/10-bilateral.md`

### LM-011: Bounded Conversation Lifecycle — 20–30 Turn Cycle

**Decision**: Seven-step loop: (1) fresh conversation, (2) `ddis seed` carry-over, (3) work 20–30 turns transacting, (4) k*_eff drops below threshold, (5) `ddis harvest`, (6) conversation ends, (7) GOTO 1.
**Rationale**: The bounded lifecycle was designed because agent output quality degrades as context fills with prior outputs, creating basin trapping where mediocre outputs self-reinforce. Rather than fighting k* depletion with longer conversations, the architecture embraces short conversations: the store grows monotonically across resets while each conversation remains fresh and seed-activated. Resets are cheap because no durable knowledge is lost (it's in the store) and only degraded ephemeral reasoning is discarded.
**Source**: Transcript 04:2794–2812
**Formalized across**: INV-HARVEST-007, ADR-HARVEST-007 in `spec/05-harvest.md`

### LM-012: Harvest Delegation Topology

**Decision**: Five topologies: single-agent self-review, bilateral peer review, swarm broadcast+voting, hierarchical specialist delegation, human review. Conservative thresholds: auto=0.15, peer=0.40, human=0.70. "Fresh-Agent Self-Review" pattern: depleted agent proposes, fresh session reviews (maximum context asymmetry).
**Formal**: `w_harvest(candidate) = w_intrinsic(candidate) × confidence(extraction)`.
**Rationale**: Harvest has a unique epistemic asymmetry: the harvesting agent has maximum context (lived through the session) but minimum reasoning quality (k* depleted, potentially basin-trapped), while a reviewing entity has full reasoning quality but zero context. This information asymmetry — not a hierarchy problem — motivated five topologies. Conservative thresholds (auto=0.15, peer=0.40, human=0.70) are biased toward rejection because false commits (Type I) are more expensive than false rejections (Type II, recoverable from transcript). The Fresh-Agent Self-Review pattern is particularly natural: depleted agent proposes, fresh session reviews.
**Source**: Transcript 05:1676–2073
**Formalized as**: ADR-HARVEST-004 in `spec/05-harvest.md`

### LM-013: Harvest Entity Types

**Decision**: Two entity types: **Harvest Session** (`:harvest/session-id`, `:harvest/transcript-path`, `:harvest/agent`, `:harvest/review-topology`, `:harvest/candidate-count`, `:harvest/drift-score`) and **Harvest Candidate** (`:candidate/harvest` ref, `:candidate/datom-spec`, `:candidate/category`, `:candidate/extraction-confidence`, `:candidate/commitment-weight`, `:candidate/status` lattice: `:proposed < :under-review < :committed < :rejected`).
**Rationale**: Two distinct entity types were formalized because harvest requires tracking both session-level metadata (who harvested, topology used, overall drift score) and individual candidate-level data (proposed datom, category, confidence, commitment weight, lattice-tracked status). Both entity types are themselves datoms, making the entire harvest history queryable — enabling analysis of which topologies produce the best decisions, which categories have highest false-positive rates, and whether drift scores trend downward over time.
**Source**: Transcript 05:1967–2002
**Formalized as**: INV-HARVEST-009 in `spec/05-harvest.md`

### LM-014: DDR (DDIS Decision Record) as Feedback Loop

**Decision**: When practical usage reveals spec gaps, recorded as DDR with sections: Observation, Impact on Spec, Resolution Options, Decision, Spec Update. DDRs are datoms. Feedback frequency: Stage 0 = every session, Stage 1 = every few sessions, Stage 2 = weekly.
**Rationale**: The DDR mechanism formalizes the feedback loop from practical usage back to the formal specification, addressing the inevitable gap between theory and implementation reality. DDRs are themselves datoms, making the chain of spec-evolving decisions queryable and forming institutional memory. The feedback frequency is calibrated to project maturity: DDR after every session in Stage 0 (learning fast), every few sessions in Stage 1 (patterns emerging), weekly in Stage 2 (stabilizing), and as-needed in Stage 3+ (mature).
**Source**: Transcript 05:2493–2532
**Formalized as**: ADR-HARVEST-006 in `spec/05-harvest.md`

### LM-015: Staged Alignment Strategy for Existing Codebase

**Decision**: Four strategies in preference order: (1) THIN WRAPPER — adapter for different interface, (2) SURGICAL EDIT — fix specific divergences, (3) PARALLEL IMPLEMENTATION — build alongside, migrate, remove, (4) REWRITE — replace entirely. Priority matrix: stable+working = optimize freely; stable+broken = fix now; changing-soon+working = leave alone; changing-soon+broken = defer. "Never rewrite what you can align incrementally."
**Rationale**: The core principle — never rewrite what you can align incrementally — was driven by the reality of a 60,000 LOC codebase where a full rewrite would deliver zero value until completion while the spec evolves underneath it. The priority matrix cross-references stability with correctness, ensuring most modules are GREEN (don't touch) or GREY (deferred) at any given time, keeping the blast radius of any change small. Strategies are in strict preference order because each successive strategy is higher-risk and higher-cost.
**Source**: Transcript 05:2536–2626
**Formalized as**: ADR-INTERFACE-009 in `spec/14-interface.md`

### LM-016: Seed Document Eleven-Section Structure

**Decision**: SEED.md structured as eleven sections: (1) What DDIS Is, (2) The Problem (coherence leads, memory as subsection), (3) Specification Formalism (bridges why/how), (4) Core Abstraction, (5) Harvest/Seed Lifecycle, (6) Reconciliation Mechanisms, (7) Self-Improvement Loop, (8) Interface Principles, (9) Existing Codebase, (10) Staged Roadmap, (11) Design Rationale. Minimal formalism — no algebra in seed. Mathematical formalism flows to SPEC.md.
**Rationale**: The eleven-section structure was chosen to be dense enough to capture the irreducible core, sparse enough that it can be read in ten minutes. Three key structural choices: leading with the coherence problem rather than the memory problem (since memory loss is the presenting symptom while divergence is the disease), placing the specification formalism as Section 3 between problem statement and core abstraction (because invariants, ADRs, and the bilateral loop are core to what DDIS is), and giving reconciliation mechanisms their own section with the unifying detect-classify-resolve frame. Formalism was kept minimal — axioms stated plainly, not algebraically.
**Source**: Transcript 07:329–343
**Formalized as**: ADR-SEED-007 in `spec/06-seed.md`

### LM-017: Dynamic CLAUDE.md Generation

**Decision**: CLAUDE.md is dynamically generated from the datom store at session start as part of the seed assembly pipeline. It replaces the static CLAUDE.md with a version that adapts to observed drift patterns, current task context, and recent harvest quality. At Stage 0, CLAUDE.md is static; at Stage 1, generate_claude_md(store, task, budget) produces a budget-constrained document from store queries.
**Rationale**: A dynamic CLAUDE.md makes every subsequent tool call more effective because the agent starts with task-specific invariants, empirically-derived behavioral corrections, and pre-loaded seed context rather than generic instructions. The generation pipeline queries the store for active intentions, governing invariants, uncertainty landscape, drift history, and guidance topology, then applies ASSEMBLE with priority ordering (tools > task context > risks > drift corrections > seed context) to fit within a token budget. The implementation cost is minimal but the compounding benefit is significant: each session collects drift data that improves the next session's CLAUDE.md.
**Source**: Transcript 05 (dynamic CLAUDE.md innovation); SEED.md §8
**Formalized as**: ADR-SEED-006 in `spec/06-seed.md`

---

## Coherence & Reconciliation Decisions

### CO-001: Coherence Verification Is the Fundamental Problem

**Decision**: DDIS solves coherence verification — maintaining verifiable non-divergence between intent, specification, implementation, and observed behavior. NOT a memory system. The memory problem is the presenting symptom; divergence is the deeper disease. Framing hierarchy: coherence leads, memory subordinated.
**Rationale**: AI agents both amplify the problem (high-volume artifacts with zero durable memory = "divergence factory") and make it more solvable (fast, voluminous output directed at continuous automatic verification).
**Source**: Transcript 06 (coherence verification reframe); Transcript 07:9–17 (framing hierarchy)
**Formalized as**: ADR-BILATERAL-006 in `spec/10-bilateral.md`

### CO-002: Four-Type Divergence Taxonomy (Original)

**Decision**: Original taxonomy: epistemic, structural, consequential, aleatory.
**Note**: Expanded to eight types in CO-003.
**Rationale**: The original four-type taxonomy was established from the insight that divergence is the fundamental disease behind the presenting symptom of memory loss — it happens to every project at every scale with or without AI agents. The four types were classified to ensure detection and resolution mechanisms could be precisely targeted to each divergence type rather than treated with a single generic process. This was later expanded to eight types (CO-003) as additional boundary cases were identified.
**Source**: Transcript 06
**Formalized as**: ADR-SIGNAL-004 in `spec/09-signal.md`

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

**Rationale**: The eight-type taxonomy was derived by systematically mapping each protocol mechanism to the specific divergence type it addresses at a specific boundary in the coherence chain. The eight types — epistemic, structural, consequential, aleatory, logical, axiological, temporal, and procedural — emerged from laying out what each mechanism (harvest, associate/assemble, guidance, merge, deliberation, signal, sync-barrier, dynamic CLAUDE.md) actually reconciles, revealing they collectively cover the complete space of divergence types across all boundaries. The architecture is extensible because every mechanism operates over the same datom substrate (CO-014).
**Source**: Transcript 07; SEED.md §6
**Formalized across**: INV-SIGNAL-006, ADR-SIGNAL-001 in `spec/09-signal.md`

### CO-004: Bilateral Loop Convergence Property

**Decision**: Each cycle reduces total divergence. Converges when forward and backward projections agree.
**Rationale**: Coherence verification must be bidirectional: a forward-only check (does implementation satisfy spec?) misses cases where the spec is silent or wrong about implementation realities, while a backward-only check (does spec describe implementation?) misses cases where the implementation violates stated constraints. The convergence property — that each bilateral cycle monotonically reduces total divergence — was chosen because it provides a well-founded progress guarantee, making the loop a process that terminates rather than oscillates.
**Source**: Transcript 06
**Formalized across**: INV-BILATERAL-001, NEG-BILATERAL-001 in `spec/10-bilateral.md`

### CO-005: Specification Formalism as Divergence-Type-to-Mechanism Mapping

**Decision**: Each formalism element maps to a primary divergence detection role: Invariants = logical divergence (falsifiable claims), ADRs = axiological divergence (prevent reversing decisions without knowing why), Negative cases = structural divergence (prevent overspecification in one dimension and underspecification in another).
**Rationale**: The one-to-one mapping was a deliberate design choice to ensure the specification formalism is not merely documentation but active coherence machinery, where each element has a precise diagnostic role. Invariants detect logical divergence (the implementation contradicts the specification), ADRs detect axiological divergence (preventing reversal of decisions without knowing why, which would undermine the goals motivating the design), and negative cases prevent structural divergence from overspecification/underspecification. Without this mapping, specification elements would be undifferentiated prose rather than targeted divergence detectors.
**Source**: Transcript 06:428–433
**Formalized as**: ADR-BILATERAL-007 in `spec/10-bilateral.md`

### CO-006: Structural vs. Procedural Coherence

**Decision**: Coherence is a structural property, not a procedural obligation. "Process obligations decay under pressure. Structural properties persist because they are enforced by architecture."
**Rationale**: A sharp distinction is drawn between process obligations ("keep the docs updated") and structural properties ("the specification and implementation are stored in the same substrate, verified by the same queries, and divergence detection runs automatically"). Existing tools (requirements documents, Jira, test suites, code review, documentation) all fail because they treat coherence as a process obligation rather than a structural property. DDIS makes coherence structural by co-locating spec, implementation facts, and verification in the same datom store with automated bilateral checking.
**Source**: Transcript 06
**Formalized as**: ADR-FOUNDATION-005 in `spec/00-preamble.md`

### CO-007: Four Recognized Taxonomy Gaps

**Decision**: Four coverage gaps identified in the eight-type reconciliation taxonomy: (1) Spec-to-intent divergence — addressed by intent validation sessions (CO-012), (2) Implementation-to-behavior divergence — addressed by test results as datoms (CO-011), (3) Cross-project coherence — deferred, addressed architecturally (CO-013), (4) Temporal degradation of observations — addressed by observation staleness model (UA-002).
**Rationale**: After constructing the eight-type taxonomy, four specific gaps were immediately identified rather than papered over, consistent with the principle that acknowledged residual divergence is better than hidden divergence. Each gap was accompanied by a sketch of a potential resolution mechanism, and the gaps were explicitly documented to ensure they would be addressed as the system matures rather than discovered later as unpleasant surprises.
**Source**: Transcript 07:248–275
**Formalized as**: ADR-SIGNAL-005 in `spec/09-signal.md`

### CO-008: Five-Point Coherence Statement

**Decision**: (1) Does the spec contradict itself? (2) Does implementation match spec? (3) Does spec still match intent? (4) Do agents agree? (5) Is methodology being followed?
**Rationale**: The five-point chain articulates formally verifiable claims that DDIS must enable: intent captured as traceable goals, spec checked for internal contradictions via five-tier detection, every goal tracing to at least one invariant and vice versa, every implementation artifact tracing to an invariant it satisfies, and the full chain from intent through observed behavior verified with the uncertainty tensor driven to zero or explicitly accepted residual. Each point addresses a distinct boundary in the coherence chain, and the crucial constraint is that these must be verifiable claims supported by evidence in the store — not subjective assessments.
**Source**: Transcript 06
**Formalized across**: INV-BILATERAL-002, NEG-BILATERAL-002 in `spec/10-bilateral.md`; ADR-TRILATERAL-001 in `spec/18-trilateral.md`

### CO-009: Fitness Function F(S) with Seven Components

**Decision**: `F(S) = 0.18×V + 0.18×C + 0.18×(1-D) + 0.13×H + 0.13×(1-K) + 0.08×(1-I) + 0.12×(1-U)` where U = mean uncertainty. Target: F(S) → 1.0.
**Rationale**: Uncertainty weight 0.12 reflects importance as coordination metric without dominating fitness.
**Source**: Transcript 02:1905–1910; Transcript 03:933–941
**Formalized as**: ADR-BILATERAL-001 in `spec/10-bilateral.md`

### CO-010: Four-Boundary Chain

**Decision**: Divergence arises at four boundaries: Intent → Specification → Implementation → Observed Behavior. Each boundary has a specific divergence type. DDIS provides detection and resolution at EACH boundary.
**Rationale**: The four-boundary decomposition was chosen because each boundary has a distinct divergence type requiring a distinct detection mechanism: Intent→Spec (performance invariants missing), Spec→Impl (idempotent refresh violated), Impl→Behavior (assumptions not documented). Conflating boundaries would produce an undifferentiated "check everything" approach that fails to target specific failure modes. This chain structure also enables the fitness function to be decomposed per-boundary for targeted remediation.
**Source**: Transcript 06:413–420
**Formalized across**: ADR-BILATERAL-002 in `spec/10-bilateral.md`; ADR-TRILATERAL-001 in `spec/18-trilateral.md`

### CO-011: Test Results as Datoms

**Decision**: Test results are datoms. "Test X passed at frontier F" is a fact about observed behavior. "Test X failed with error E" is implementation-to-behavior divergence. Extends bilateral loop to the behavior boundary.
**Rationale**: Implementation-to-behavior checking (does the code actually do what it claims?) is traditionally covered by testing which sits outside the DDIS formalism, identified as Gap 2 in the taxonomy. Making test results datoms closes this gap naturally: the forward path includes running tests and asserting results, while the backward path triggers drift detection on the tested invariant upon failure. This extends the bilateral loop to cover the behavior boundary without introducing machinery outside the datom substrate.
**Source**: Transcript 07:255–258
**Formalized as**: INV-BILATERAL-005 in `spec/10-bilateral.md`

### CO-012: Intent Validation Sessions

**Decision**: Periodic structured reviews where the system assembles current spec state for human review: "Does this still describe what I want?" Output is a datom. Something between deliberation and harvest.
**Rationale**: Specification-vs-intent divergence is the hardest boundary to check because intent is often tacit and evolves as the human learns more about the problem. While axiological contradiction detection and coverage metrics check structural alignment between goals and invariants, they check structure rather than meaning — an invariant can trace to a goal and still not capture what the human actually meant. Periodic intent validation sessions address Gap 1 in the taxonomy, with the output recorded as a datom for traceability.
**Source**: Transcript 07:249–253
**Formalized as**: ADR-BILATERAL-003 in `spec/10-bilateral.md`

### CO-013: Cross-Project Coherence (Deferred)

**Decision**: Axiological divergence can occur between projects. Store architecture supports it (multiple stores mergeable) but reconciliation machinery needs cross-store contradiction detection. Deferred to post-Stage-2.
**Rationale**: The store architecture supports cross-project operation since multiple stores can be merged, but the reconciliation machinery would need cross-store contradiction detection. The deferral was driven by scope management: the single-project coherence problem is the immediate priority, while cross-project coherence is architecturally supported but requires additional detection machinery that should wait for later stages when the single-project case is proven.
**Source**: Transcript 07:259–261
**Formalized as**: ADR-BILATERAL-009 in `spec/10-bilateral.md`

### CO-014: Extensible Reconciliation Architecture

**Decision**: Taxonomy extensible by construction: new divergence types yield new detection queries and new deliberation patterns, all producing datoms in the same store. Resolution mechanism for all future types constrained to be datom-producing queries.
**Rationale**: The reconciliation taxonomy's potential incompleteness is not a flaw because every reconciliation mechanism operates over the same datom substrate, so new mechanisms can be added without changing the foundation. If a new divergence type is identified, the detection is a new query and the resolution is a new deliberation pattern, both producing datoms in the same store. This means the taxonomy grows with the system's needs rather than requiring architectural changes when gaps are discovered.
**Source**: Transcript 07:315–317
**Formalized as**: ADR-BILATERAL-010 in `spec/10-bilateral.md`

### CO-015: Divergence Metric as Weighted Boundary Sum

**Decision**: Total divergence across the four-boundary chain (intent -> spec -> impl -> behavior) is quantified as `D(spec, impl) = Sigma_i w_i * |boundary_gap(i)|` where boundary weights reflect the cost of divergence at each boundary. Default: equal weights.
**Rationale**: Each boundary contributes independently to total divergence. Weighted sum is the simplest combination that captures per-boundary severity while remaining decomposable for targeted remediation.
**Uncertainty**: UNC-BILATERAL-002 — boundary weights may need per-project tuning. Confidence: 0.5.
**Source**: ADRS CO-010
**Formalized as**: ADR-BILATERAL-002 in `spec/10-bilateral.md`

---

## Implementation Decisions

Decisions made during the specification-to-implementation transition, based on research
findings from `audits/stage-0/research/`.

### IMPL-001: Custom Datalog Engine Over Existing Crates

**Decision**: Build a custom Datalog evaluator (~2100-3400 LOC) rather than using Datafrog, Crepe, Ascent, or DDlog. None of the existing Rust Datalog engines support runtime query construction, which is required for `braid query '[:find ...]'`. Crepe and Ascent are compile-time macro systems. DDlog is archived. Datafrog is a fixpoint engine without a Datalog layer.
**Rationale**: The CLI's runtime query requirement (`braid query` accepts arbitrary Datalog strings) fundamentally disqualifies compile-time-only engines. Custom is the only option satisfying all spec requirements (semi-naive, frontier scoping, EAV pattern matching, runtime construction).
**Rejected**: Datafrog (no Datalog parser, no stratification — only primitives), Crepe (compile-time only), Ascent (compile-time only), DDlog (archived, Haskell toolchain).
**Source**: D2-datalog-engines.md; spec/03-query.md; formalized as ADR-IMPL-QUERY-001 in docs/guide/03-query.md.
**Formalized as**: ADR-IMPL-QUERY-001 in `docs/guide/03-query.md`
**Note**: IMPL-001 is an implementation-level decision; docs/guide/ reference is canonical.

### IMPL-002: Tiered Tokenization (chars/4 at Stage 0, tiktoken-rs at Stage 1)

**Decision**: Use chars/4 with content-type correction at Stage 0. Graduate to tiktoken-rs (cl100k_base) at Stage 1 when token efficiency tracking needs cross-session comparability. Behind a `TokenCounter` trait for swappability.
**Rationale**: The budget system uses coarse bands (200/500/2000 tokens). A 15-20% approximation error from chars/4 rarely changes band selection. Zero-dependency at Stage 0 avoids complexity during critical foundation work.
**Rejected**: tiktoken-rs at Stage 0 (unnecessary dependency), HuggingFace tokenizers (~40 deps, no Claude model), bpe (no model-specific encoding).
**Source**: D5-tokenizer-survey.md; spec/13-budget.md.
**Formalized as**: ADR-BUDGET-004 in `spec/13-budget.md`

### IMPL-003: Three-Tier Kani CI Pipeline

**Decision**: Split Kani verification into three CI tiers: Fast (every PR, <5 min, ~13 trivial+simple harnesses), Full (nightly, <30 min, all 24 Stage 0 harnesses), Extended (weekly, <2 hours, higher unwind bounds). CaDiCaL as default solver.
**Rationale**: 24 Stage 0 Kani harnesses cannot all run within the 15-minute PR gate. The three-tier split keeps PR verification fast while still running comprehensive verification on a schedule. CaDiCaL shows 10-200x speedup over MiniSat for structural properties.
**Source**: D3-kani-feasibility.md; spec/16-verification.md section 16.2.
**Formalized as**: ADR-VERIFICATION-001 in `spec/16-verification.md`

---

### ADR-AGENT-MD-001: Provider-Neutral Agent Instructions

**Decision**: The kernel's agent instructions generator uses provider-neutral naming
(`agent_md`, `AgentMdConfig`, `AGENTS.md`) rather than provider-specific naming
(`claude_md`, `ClaudeMdConfig`, `CLAUDE.md`).

**Alternatives rejected**:
1. Keep `claude_md` — violates kernel neutrality principle; DDIS/Braid is a specification
   standard, not a Claude-specific tool.
2. Use `instructions` — too generic, loses the markdown-document semantic that `agent_md`
   preserves while paralleling the AGENTS.md open standard (https://agents.md/).

**Traces to**: SEED.md §8 (Interface Principles — the kernel must not be tied to any
specific agent provider); C7 (Self-Bootstrap — DDIS specifies itself, not Claude).

**Rationale**: DDIS/Braid is a specification standard and protocol. The kernel must be
provider-neutral so that any agent (Claude, Codex, Gemini, Cursor, etc.) can consume its
output. The AGENTS.md open standard provides an interoperable default filename.

**Bead**: brai-1usv

---

*This document is maintained alongside `SEED.md` and `docs/HARVEST.md` as part of the manual
bootstrap methodology. When `SPEC.md` exists, each decision here should have a corresponding
formal ADR element. When the datom store exists, each decision becomes a set of datoms and
this file becomes a projection.*
