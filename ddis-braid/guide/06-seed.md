# §6. SEED — Build Plan

> **Spec reference**: [spec/06-seed.md](../spec/06-seed.md) — read FIRST
> **Stage 0 elements**: INV-SEED-001–006 (6 INV), ADR-SEED-001–004, NEG-SEED-001–002
> **Dependencies**: STORE (§1), SCHEMA (§2), QUERY (§3), RESOLUTION (§4), MERGE (§7), HARVEST (§5)
> **Cognitive mode**: Retrieval-theoretic — relevance, compression, trajectory seeds

---

## §6.1 Module Structure

```
braid-kernel/src/
└── seed.rs     ← SeedAssembly, associate/assemble/compress, dynamic CLAUDE.md
```

### Public API Surface

```rust
/// Assemble a session seed from the store, given a task description.
pub fn assemble_seed(
    store: &Store,
    task: &str,
    budget: usize,       // token budget for seed output
) -> SeedOutput;

/// The five-part trajectory seed (ADR-SEED-004 canonical template).
/// Field names match spec/06-seed.md ADR-SEED-004. The spec's AssembledContext
/// is the lower-level entity-scored representation; SeedOutput is the
/// agent-facing formatted template produced from it.
pub struct SeedOutput {
    pub orientation:  String,  // Project identity, phase, active spec section
    pub constraints:  String,  // Relevant INVs, settled ADRs, negative cases
    pub state:        String,  // Recent transactions, frontier, drift score, uncertainties
    pub warnings:     String,  // Drift signals, open questions, uncertainties, harvest alerts
    pub directive:    String,  // Next task, acceptance criteria, active guidance corrections
}

/// Compute relevance score for a datom w.r.t. a task description.
pub fn relevance_score(datom: &Datom, store: &Store, task: &str) -> f64;

/// Generate dynamic CLAUDE.md from store state.
/// Free function: generates formatted CLAUDE.md by querying the store.
pub fn generate_claude_md(
    store: &Store,
    focus: &str,
    agent: AgentId,
    budget: usize,
) -> Result<String, SeedError>;

/// Cue for ASSOCIATE — determines how schema neighborhood discovery starts.
/// `depth` and `breadth` enforce INV-SEED-003 (ASSOCIATE Boundedness).
/// Defaults: depth=3, breadth=10.
pub enum AssociateCue {
    Semantic { text: String, depth: usize, breadth: usize },
    Explicit { seeds: Vec<EntityId>, depth: usize, breadth: usize },
}

/// Associate: discover schema neighborhood relevant to the task (INV-SEED-003).
pub fn associate(store: &Store, cue: AssociateCue) -> SchemaNeighborhood;

/// Compress seed to fit within budget.
pub fn compress_seed(seed: &SeedOutput, budget: usize) -> SeedOutput;
```

---

## §6.2 Three-Box Decomposition

### Seed Assembly (Associate → Assemble → Compress)

**Black box** (contract):
- INV-SEED-001: Seed as Store Projection — seed contains only information from the store,
  nothing fabricated. All content is queryable and traceable to datoms.
- INV-SEED-002: Budget Compliance — seed output fits within the specified token budget.
  Compression preserves activation-critical content over verbose detail.
- INV-SEED-003: ASSOCIATE Boundedness — graph expansion is bounded to `depth × breadth`,
  preventing unbounded traversal.
- INV-SEED-004: Section Compression Priority — compress State first, Directive last.
- INV-SEED-005: Demonstration Density — at least one demonstration per constraint cluster.
- INV-SEED-006: Intention Anchoring — active intentions pinned at π₀ (full detail)
  regardless of budget pressure.

**State box** (internal design):
- Four-stage pipeline: **associate → query → assemble → compress** (spec/06-seed.md §6.1).
  The formal composition is `SEED = assemble . query . associate` with compress as a
  post-processing step on the assembled output.

  **Stage 1 — ASSOCIATE**: Discover the schema neighborhood relevant to the task.
  - Input: store + task description (or explicit entity seeds).
  - Operation: keyword matching + graph proximity expansion.
  - Output: `SchemaNeighborhood` — a set of relevant entities, attributes, and types.
  - Bounded by `depth x breadth` (INV-SEED-003, defaults: depth=3, breadth=10).
  - Traverses both structural schema edges and learned associations (ADR-SEED-005, Stage 1).

  **Stage 2 — QUERY**: Retrieve the actual datom values for the discovered neighborhood.
  - Input: schema neighborhood from ASSOCIATE.
  - Operation: Datalog queries against the store for each relevant entity.
  - Output: `QueryResult` — scored entity-datom pairs with relevance rankings.
  - Scoring function: `score(e) = α*relevance(e, task) + β*significance(e) + γ*recency(e)`
    where α=0.5, β=0.3, γ=0.2 (defaults, configurable as datoms).

  **Stage 3 — ASSEMBLE**: Build the five-part seed structure from query results (ADR-SEED-004).
  Note: The assembled context uses `ContextSection` and `ProjectionLevel` types defined in
  spec/06-seed.md (lines 187-196). Stage 0 implementation should include minimal definitions
  -- see types.md.
  - Pre-allocate pinned intentions at π_0 (INV-SEED-006).
  - Distribute remaining budget across sections:
    - Orientation: project metadata datoms + current phase.
    - Constraints: relevant INVs, settled ADRs, negative cases for current task.
    - State: transactions since last seed, frontier state, drift score.
    - Warnings: drift signals, open questions, uncertainties, harvest alerts.
    - Directive: next task, acceptance criteria, active guidance corrections.
  - Select projection level per entity based on remaining budget:
    - \> 2000 tokens: π_0 (full datoms) for top entities, π_1 for others.
    - 500-2000 tokens: π_1/π_2 for all entities.
    - 200-500 tokens: π_2 for top entities, omit others.
    - <= 200 tokens: single-line status + single guidance action.
  - Insert demonstrations for constraint clusters when budget > 1000 tokens (INV-SEED-005).
    A 30-token demonstration activates pattern-completion more effectively than invariant
    statements alone.
  - Check observation staleness (ADR-HARVEST-005): flag observations past their TTL.

  **Stage 4 — COMPRESS**: Fit the assembled seed within the token budget.
  - Section compression priority (INV-SEED-004, first to compress → last):
    1. **State** first (lowest marginal value; reconstructible from store queries)
    2. **Constraints** second (degrade to ID-only references, e.g., "INV-STORE-001")
    3. **Orientation** third (short, mostly fixed; compress but never omit entirely)
    4. **Warnings** fourth (safety-critical; high behavioral leverage per token)
    5. **Directive** last (directly controls agent behavior; most valuable per token)
  - Token allocation by remaining budget:
    - \> 2000 tokens: full detail in all sections; π_0 for State items
    - 500-2000 tokens: compress State to π_1; keep Constraints at full IDs
    - 200-500 tokens: Orientation (50 tok) + Directive (100 tok) + top-3 Warnings only
    - <= 200 tokens: single-line orientation + single-line directive + harvest warning if applicable
  - Iteratively remove lowest-scored items until within budget. Never remove warnings or directive.

**Clear box** (implementation):
- `associate`: Tokenize task description → extract keywords → Datalog query:
  `[:find ?e ?score :where [?e :spec/id ?id] [(keyword-match ?id task-keywords) ?score]]`.
  For Stage 0, keyword matching is simple substring/overlap. Significance tracking deferred to Stage 1.
- `query`: For each entity in the neighborhood, retrieve relevant datoms. Score by
  `α*relevance + β*significance + γ*recency`. Return sorted entity-datom pairs.
- `assemble`: For each section, format scored datoms into markdown strings.
  Token count estimated at 4 characters per token (rough approximation).
- `compress`: If total tokens > budget, iteratively remove lowest-scored items from state
  and constraints sections until within budget. Never remove warnings or directive.

### Dynamic CLAUDE.md Generation (INV-GUIDANCE-007)

**Black box**: Generate a CLAUDE.md file from store state that optimizes the agent's session.

**Clear box** (implementation):

The canonical seven-step pipeline (from spec/06-seed.md §6.1 Level 0, GENERATE-CLAUDE-MD):

| Step | Operation | What It Produces |
|------|-----------|------------------|
| 1 | **ASSOCIATE** with focus | Schema neighborhood relevant to the declared focus |
| 2 | **QUERY** active intentions | What the agent should be working on (pinned at π_0) |
| 3 | **QUERY** governing invariants | Which INVs constrain the current task |
| 4 | **QUERY** uncertainty markers | What is uncertain in the relevant neighborhood |
| 5 | **QUERY** competing branches | Are there parallel efforts on the same entities? |
| 6 | **QUERY** drift patterns | What drift has been observed in recent sessions? |
| 7 | **ASSEMBLE** at budget | Compress all query results into budget-aware output |

Priority ordering within budget: `tools > task_context > risks > drift_corrections > seed_context`

The output follows the unified five-part seed template (ADR-SEED-004):
Orientation, Constraints, State, Warnings, Directive.

Implementation-level mapping to the `generate_claude_md()` function:
1. **Step 1 (ASSOCIATE)**: `associate(store, SemanticCue { text: focus })` — discover relevant entities.
2. **Step 2 (active intentions)**: Query `:intention/*` entities with status `:active`.
3. **Step 3 (governing INVs)**: Query `:spec/type = "invariant"` entities in the schema neighborhood.
   Apply the removal test: only include constraints that would change agent behavior at current k*.
4. **Step 4 (uncertainty)**: Query `:uncertainty/*` entities in the schema neighborhood.
5. **Step 5 (competing branches)**: Query `:branch/*` entities with overlapping entity targets.
   (Stage 0: no branches exist, this step produces empty results.)
6. **Step 6 (drift patterns)**: Query `:drift/*` entities from recent harvest sessions.
   Select anti-drift footer based on the most significant current drift signal.
7. **Step 7 (ASSEMBLE)**: Format into CLAUDE.md template (see guide/00-architecture.md §0.3).
   Token budget: ≤1000 tokens initially, shrinks as conversation k* decays.

---

## §6.3 LLM-Facing Outputs

The seed output IS the primary LLM-facing surface. Each part is designed as a prompt component:

### Seed Output — Five-Part Trajectory Seed

```markdown
## Orientation
You are working on **Braid** (DDIS implementation). Current phase: Stage 0 implementation.
Active namespace: STORE. Spec: spec/01-store.md.

## Constraints
- ADR-STORE-002: BLAKE3 for content hashing (w=12, do not relitigate)
- ADR-STORE-004: HLC for transaction ordering (w=8, do not relitigate)
- ADR-STORE-009: redb for persistence (w=3, low commitment — revisable)
- INV-STORE-001: Append-only immutability
- NEG-001: No aspirational stubs

## State
Last 3 transactions: genesis (tx_0), schema extension (tx_1), spec-bootstrap (tx_2).
Frontier: {agent1: tx_2}. Drift: 0.0. Store: 147 datoms.
Active uncertainties: UNC-SCHEMA-001 (17 attributes sufficient? confidence=0.85).

## Warnings
None.

## Directive
Implement `Store::transact()` per INV-STORE-001 (append-only) and INV-STORE-002 (strict growth).
Traces to: SEED.md §4 Axiom 2. First action: write typestate Transaction impl.
```

### Dynamic CLAUDE.md — Excerpt

```markdown
# Braid — Session Context
<!-- Generated: 2026-03-02T10:00:00Z, frontier: {agent1: tx_2}, k*: 180000 -->

## Active Methodology
Prior session example: Agent transacted 31 spec elements, harvested 4 decisions,
seed picked up implementation context without re-explanation. Harvest drift: 0.0.

## Constraints
- C1: Append-only store (INV-STORE-001)
- C7: Self-bootstrap (INV-SCHEMA-001)
- NEG-001: No aspirational stubs

## Session Objective
Implement STORE namespace (guide/01-store.md). 13 INVs, 12 ADRs.

## Anti-Drift
↳ What algebraic law does each function preserve?
```

---

## §6.4 Verification

### Key Properties

```rust
proptest! {
    // INV-SEED-001: Seed as Store Projection (only info from store, nothing fabricated)
    fn inv_seed_001(store in arb_store(10), task in arb_task()) {
        let seed = assemble_seed(&store, &task, 10000);
        // Every entity referenced in seed must exist in store
        for eid in extract_entity_refs(&seed) {
            prop_assert!(store.contains_entity(eid));
        }
    }

    // INV-SEED-002: Budget Compliance (seed output fits within token budget)
    fn inv_seed_002(store in arb_store(10), task in arb_task(), budget in 100..5000usize) {
        let seed = assemble_seed(&store, &task, budget);
        prop_assert!(token_count(&seed) <= budget);
    }

    // INV-SEED-003: ASSOCIATE Boundedness (graph expansion bounded by depth × breadth)
    fn inv_seed_003(store in arb_store(10), task in arb_task()) {
        let neighborhood = associate(&store, &task);
        let depth = 3; // default depth
        let breadth = 10; // default breadth
        prop_assert!(neighborhood.entities.len() <= depth * breadth);
    }
}
```

### Additional Properties

```rust
proptest! {
    // INV-SEED-004: Section Compression Priority — State compresses before Directive
    fn inv_seed_004(
        store in arb_store(20),
        task in arb_task(),
        budget in 200..2000usize,
    ) {
        let full_seed = assemble_seed(&store, &task, 10000);
        let compressed_seed = assemble_seed(&store, &task, budget);
        // If directive was compressed, state must have been compressed first
        if token_count_section(&compressed_seed.directive) < token_count_section(&full_seed.directive) {
            prop_assert!(
                token_count_section(&compressed_seed.state) < token_count_section(&full_seed.state),
                "Directive compressed before State — violates compression priority"
            );
        }
    }

    // INV-SEED-005: Demonstration Density
    fn inv_seed_005_demonstration_density(
        clusters in prop::collection::vec(any::<KnowledgeCluster>(), 1..10),
    ) {
        let context = assemble_context(&store, &clusters);
        for cluster in &clusters {
            let demos = context.demonstrations_for(cluster);
            prop_assert!(demos.len() >= 1 && demos.len() <= 3,
                "Each cluster must have 1-3 demonstrations, got {}", demos.len());
        }
    }

    // INV-SEED-006: Active Intention Pinning — π₀ survives budget compression
    fn inv_seed_006(
        store in arb_store(10),
        budget in 100usize..2000,
    ) {
        let active_intentions = store.query_active_intentions();
        let seed = assemble_seed(&store, "test task", budget);
        // Active intentions must appear regardless of budget
        for intention in &active_intentions {
            prop_assert!(
                seed.warnings.contains(&intention.summary)
                    || seed.state.contains(&intention.summary),
                "Active intention {} missing from seed at budget {}",
                intention.id, budget
            );
        }
    }
}
```

INV-SEED-002 and INV-SEED-003 have V:KANI tags.

---

## §6.5 Implementation Checklist

- [ ] `SeedOutput` five-part structure defined
- [ ] `associate()` finds task-relevant entities via keyword matching
- [ ] `assemble_seed()` builds five-part output from store queries
- [ ] `compress_seed()` fits within token budget, preserves warnings + directive
- [ ] `relevance_score()` scores datoms by keyword + recency
- [ ] `generate_claude_md()` produces valid CLAUDE.md from store state
- [ ] Seed is budget-bounded (INV-SEED-002)
- [ ] ASSOCIATE is bounded by depth × breadth (INV-SEED-003)
- [ ] Section compression follows priority order: State > Constraints > Orientation > Warnings > Directive (INV-SEED-004)
- [ ] At least one demonstration per constraint cluster when budget > 1000 tokens (INV-SEED-005)
- [ ] Active intentions pinned at π₀ regardless of budget (INV-SEED-006)
- [ ] Integration: genesis → transact specs → seed → readable context for new session

---
