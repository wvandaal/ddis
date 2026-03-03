# §6. SEED — Build Plan

> **Spec reference**: [spec/06-seed.md](../spec/06-seed.md) — read FIRST
> **Stage 0 elements**: INV-SEED-001–004 (4 INV), ADR-SEED-001–003, NEG-SEED-001–002
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

/// The five-part trajectory seed.
pub struct SeedOutput {
    pub orientation:   String,  // Project identity, phase, active spec section
    pub decisions:     String,  // Relevant ADRs, commitment weights, do-not-relitigate
    pub context:       String,  // Recent transactions, frontier, drift score, uncertainties
    pub warnings:      String,  // Unresolved conflicts, stale datoms, thresholds
    pub task:          String,  // Assigned task, traceability, relevant INVs, first action
}

/// Compute relevance score for a datom w.r.t. a task description.
pub fn relevance_score(datom: &Datom, store: &Store, task: &str) -> f64;

/// Generate dynamic CLAUDE.md from store state.
pub fn generate_claude_md(
    store: &Store,
    task: &str,
    budget: usize,
) -> String;

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
- INV-SEED-004: Intention Anchoring — active intentions pinned at π₀ (full detail)
  regardless of budget pressure.

**State box** (internal design):
- Three-stage pipeline: associate → assemble → compress.
- **Associate**: Query store for task-relevant entities using keyword matching and
  graph proximity. Score by relevance: α=0.5 (keyword match), β=0.3 (significance), γ=0.2 (recency).
- **Assemble**: Build five-part structure from associated entities:
  - Orientation: project metadata datoms + current phase.
  - Decisions: ADR entities with highest commitment weight relevant to task.
  - Context: transactions since last seed, frontier state, drift score.
  - Warnings: unresolved conflicts, high-urgency uncertainties.
  - Task: the task description with traceability to SEED.md sections and relevant INVs.
- **Compress**: If assembled seed exceeds budget, truncate least-relevant items per section.
  Preserve: all warnings (safety-critical), task (directive), top-K decisions.
  Truncate: older context items, lower-relevance ADRs.

**Clear box** (implementation):
- `associate`: Tokenize task description → extract keywords → Datalog query:
  `[:find ?e ?score :where [?e :spec/id ?id] [(keyword-match ?id task-keywords) ?score]]`.
  For Stage 0, keyword matching is simple substring/overlap. Significance tracking deferred to Stage 1.
- `assemble`: For each section, query store for relevant datoms. Format into markdown strings.
  Token count estimated at 4 characters per token (rough approximation).
- `compress`: If total tokens > budget, iteratively remove lowest-scored items from context
  and decisions sections until within budget. Never remove warnings or task.

### Dynamic CLAUDE.md Generation (INV-GUIDANCE-007)

**Black box**: Generate a CLAUDE.md file from store state that optimizes the agent's session.

**Clear box** (implementation):
Seven-step pipeline (from spec/12-guidance.md):
1. Load project metadata from store.
2. Query for prior session demonstrations (harvest/seed examples from history).
3. Query for constraints — only those passing the removal test at current k*.
4. Compute current state: F(S), drift score, active basin, relevant INVs.
5. Determine session objective from seed task.
6. Select anti-drift footer based on current drift signals.
7. Assemble into CLAUDE.md template (see guide/00-architecture.md §0.3).

Token budget: ≤1000 tokens initially, shrinks as conversation k* decays.

---

## §6.3 LLM-Facing Outputs

The seed output IS the primary LLM-facing surface. Each part is designed as a prompt component:

### Seed Output — Five-Part Trajectory Seed

```markdown
## Orientation
You are working on **Braid** (DDIS implementation). Current phase: Stage 0 implementation.
Active namespace: STORE. Spec: spec/01-store.md.

## Prior Decisions
- ADR-STORE-002: BLAKE3 for content hashing (w=12, do not relitigate)
- ADR-STORE-004: HLC for transaction ordering (w=8, do not relitigate)
- ADR-STORE-009: redb for persistence (w=3, low commitment — revisable)

## Working Context
Last 3 transactions: genesis (tx_0), schema extension (tx_1), spec-bootstrap (tx_2).
Frontier: {agent1: tx_2}. Drift: 0.0. Store: 147 datoms.
Active uncertainties: UNC-SCHEMA-001 (17 attributes sufficient? confidence=0.85).

## Warnings
None.

## Task
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

### Kani Harnesses

```rust
proptest! {
    // INV-SEED-004: Active Intention Pinning — π₀ survives budget compression
    fn inv_seed_004(
        store in arb_store(10),
        budget in 100usize..2000,
    ) {
        let active_intentions = store.query_active_intentions();
        let seed = assemble_seed(&store, "test task", budget);
        // Active intentions must appear regardless of budget
        for intention in &active_intentions {
            prop_assert!(
                seed.warnings.contains(&intention.summary)
                    || seed.context.contains(&intention.summary),
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
- [ ] `compress_seed()` fits within token budget, preserves warnings + task
- [ ] `relevance_score()` scores datoms by keyword + recency
- [ ] `generate_claude_md()` produces valid CLAUDE.md from store state
- [ ] Seed is budget-bounded (INV-SEED-002)
- [ ] ASSOCIATE is bounded by depth × breadth (INV-SEED-003)
- [ ] Active intentions pinned at π₀ regardless of budget (INV-SEED-004)
- [ ] Integration: genesis → transact specs → seed → readable context for new session

---
