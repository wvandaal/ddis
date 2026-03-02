> **Namespace**: SEED | **Wave**: 2 (Lifecycle) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §6. SEED — Start-of-Session Assembly

### §6.0 Overview

Seed is the complement of harvest: where harvest extracts knowledge at session end,
seed assembles relevant knowledge at session start. The seed provides a fresh agent
with full relevant context, zero irrelevant noise, and a fresh attention budget.

The seed collapses three concerns into one mechanism: ambient awareness (CLAUDE.md),
guidance (methodology steering), and trajectory management (carry-over from prior sessions).

**Traces to**: SEED.md §5, §8
**ADRS.md sources**: IB-010, PO-002, PO-003, PO-014, GU-004, SQ-007

---

### §6.1 Level 0: Algebraic Specification

#### Seed as Projection

```
SEED : Store × TaskContext × Budget → AssembledContext

SEED(S, task, k*) = ASSEMBLE(QUERY(ASSOCIATE(S, task)), k*)

The seed is a projection of the store onto the relevant subset,
compressed to fit the available attention budget.

Formally: SEED = assemble ∘ query ∘ associate
  where associate : Store × TaskContext → SchemaNeighborhood
        query     : SchemaNeighborhood → QueryResult
        assemble  : QueryResult × Budget → AssembledContext
```

#### Assembly Priority Function

```
For each entity e in the query result:
  score(e) = α × relevance(e, task) + β × significance(e) + γ × recency(e)
  where α = 0.5, β = 0.3, γ = 0.2 (defaults, configurable as datoms)

Assembly selects entities in score order until budget is exhausted.
Higher-priority entities get richer projections (π₀ → π₁ → π₂ → π₃).
```

#### Dynamic CLAUDE.md as Seed

```
GENERATE-CLAUDE-MD : Store × Focus × Agent × Budget → Markdown

The dynamic CLAUDE.md collapses three concerns:
  1. Ambient awareness (Layer 0) — CLAUDE.md IS the ambient context
  2. Guidance (Layer 3) — seed context IS the first guidance (zero tool-call cost)
  3. Trajectory management — CLAUDE.md IS the seed turn

Seven-step generation:
  (1) ASSOCIATE with focus
  (2) QUERY active intentions
  (3) QUERY governing invariants
  (4) QUERY uncertainty markers
  (5) QUERY competing branches
  (6) QUERY drift patterns
  (7) ASSEMBLE at budget

Priority ordering: tools > task_context > risks > drift_corrections > seed_context
```

---

### §6.2 Level 1: State Machine Specification

#### ASSOCIATE — Schema Discovery

```
ASSOCIATE(S, cue) → SchemaNeighborhood

Two modes:
  SemanticCue(text): natural language → schema search → graph expansion
  ExplicitSeeds([EntityId]): start from known entities → graph expansion

POST:
  |result| ≤ depth × breadth (bounded)
  high-significance entities preferred (AS-007)
  learned associations traversed alongside structural edges (AA-004)

SchemaNeighborhood = {entities, attributes, types} — NOT values
  (schema-level discovery, not data retrieval)
```

#### ASSEMBLE — Rate-Distortion Context

```
ASSEMBLE(query_results, schema_neighborhood, budget) → AssembledContext

PRE:
  budget > 0

PIPELINE:
  1. Score entities: score(e) = α×relevance + β×significance + γ×recency
  2. Sort by score (descending)
  3. For each entity in order:
     a. Select projection level based on remaining budget:
        >2000 tokens: π₀ (full datoms) for top entities, π₁ for others
        500–2000:     π₁/π₂
        200–500:      π₂ for top, omit others
        ≤200:         single-line status + single guidance action
     b. Subtract token cost from remaining budget
     c. If budget exhausted, stop
  4. Pin intentions at π₀ regardless of budget (INV-ASSEMBLE-INTENTION-001)
  5. Record projection pattern for reification learning (AS-008)
  6. Check staleness for observation entities (UA-007)

POST:
  |result| ≤ budget (token count)
  structural dependency coherence (no entity without its dependencies)
  all active intentions included
```

#### Seed Output Template

```
Seed output follows a five-part template:
  (1) Context — 1–2 sentences: what was last worked on, current project state
  (2) Invariants — active invariants governing the next task
  (3) Artifacts — files modified, decisions made, entities created
  (4) Open questions — from deliberations, uncertainties, pending crystallizations
  (5) Active guidance — next methodologically correct actions

Formatted as spec-language (INV-GUIDANCE-SEED-001): invariants and formal
structure, NOT instruction-language (steps, checklists).
```

---

### §6.3 Level 2: Interface Specification

```rust
/// Schema neighborhood — what ASSOCIATE discovers.
pub struct SchemaNeighborhood {
    pub entities: Vec<EntityId>,
    pub attributes: Vec<Attribute>,
    pub entity_types: Vec<Keyword>,
}

/// Assembled context — what ASSEMBLE produces.
pub struct AssembledContext {
    pub sections: Vec<ContextSection>,
    pub total_tokens: usize,
    pub budget_remaining: usize,
    pub projection_pattern: ProjectionPattern,
}

pub struct ContextSection {
    pub entity: EntityId,
    pub projection_level: ProjectionLevel,
    pub content: String,
    pub score: f64,
}

pub enum ProjectionLevel {
    Full,       // π₀ — all datoms
    Summary,    // π₁ — entity summary
    TypeLevel,  // π₂ — type summary
    Pointer,    // π₃ — single-line reference
}

/// Dynamic CLAUDE.md generator.
pub struct ClaudeMdGenerator {
    pub store: Store,
}

impl ClaudeMdGenerator {
    /// Generate dynamic CLAUDE.md for a session.
    pub fn generate(
        &self,
        focus: &str,
        agent: AgentId,
        budget: usize,
    ) -> Result<String, SeedError>;
}

impl Store {
    /// ASSOCIATE — discover relevant schema neighborhood.
    pub fn associate(&self, cue: AssociateCue) -> SchemaNeighborhood;

    /// ASSEMBLE — build budget-aware context.
    pub fn assemble(
        &self,
        query_results: &QueryResult,
        neighborhood: &SchemaNeighborhood,
        budget: usize,
    ) -> AssembledContext;

    /// SEED — full pipeline: associate → query → assemble.
    pub fn seed(&mut self, task: &str, budget: usize) -> Result<AssembledContext, SeedError>;
}
```

#### CLI Commands

```
braid seed --task "implement datom store"     # Full seed for task
braid seed --budget 2000                      # With explicit token budget
braid associate "conflict resolution"         # Schema neighborhood only
braid assemble --budget 500                   # Assemble from last query
braid claude-md --focus "stage 0"             # Generate dynamic CLAUDE.md
```

---

### §6.4 Invariants

### INV-SEED-001: Seed as Store Projection

**Traces to**: SEED §5, ADRS IB-010
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ seed operations: SEED(S, task, k*) ⊆ S
  (the seed contains only information from the store — nothing fabricated)
```

#### Level 1 (State Invariant)
Every datum in the seed output traces to a datom in the store.
The seed is a view, not a source of truth.

**Falsification**: Any claim in the seed output that does not correspond to a datom in the store.

---

### INV-SEED-002: Budget Compliance

**Traces to**: ADRS IB-004, PO-003
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ ASSEMBLE operations with budget B:
  |output| ≤ B (in tokens)
```

#### Level 1 (State Invariant)
The assembled context never exceeds the declared budget. If the relevant
information exceeds the budget, lower-priority content is dropped (projected
to coarser levels), never the budget exceeded.

**Falsification**: An ASSEMBLE output whose token count exceeds the budget parameter.

**proptest strategy**: Generate stores of varying sizes. Assemble with varying budgets.
Verify output token count ≤ budget for all combinations.

---

### INV-SEED-003: ASSOCIATE Boundedness

**Traces to**: ADRS PO-002
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ ASSOCIATE operations with depth d and breadth b:
  |result.entities| ≤ d × b
```

#### Level 1 (State Invariant)
ASSOCIATE graph expansion is bounded to prevent unbounded traversal.
The bound is `depth × breadth`, both configurable.

**Falsification**: An ASSOCIATE result with more entities than `depth × breadth`.

---

### INV-SEED-004: Intention Anchoring

**Traces to**: ADRS AA-005, PO-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ ASSEMBLE operations with include_intentions=true:
  ∀ active intentions I: I ∈ assembled_context at projection level π₀
  regardless of budget pressure
```

#### Level 1 (State Invariant)
Active intentions are pinned at full detail (π₀) even when the budget would
otherwise compress or omit them. Intentions are never sacrificed for budget.

**Falsification**: An active intention omitted from the assembled context when
`include_intentions=true`, or projected below π₀.

---

### INV-SEED-005: Dynamic CLAUDE.md Relevance

**Traces to**: ADRS PO-014
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ sections s in GENERATE-CLAUDE-MD output:
  removing s would change agent behavior
  (no irrelevant padding or boilerplate)
```

#### Level 1 (State Invariant)
Every section of the dynamic CLAUDE.md is relevant to the declared focus.
Irrelevant sections waste attention budget.

**Falsification**: A section in the generated CLAUDE.md that, if removed, would not
change agent behavior (deadweight content).

---

### INV-SEED-006: Dynamic CLAUDE.md Improvement

**Traces to**: ADRS PO-014
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ drift corrections in GENERATE-CLAUDE-MD:
  correction derived from empirical drift data (not speculation)
  corrections showing no effect after 5 sessions → replaced
```

#### Level 1 (State Invariant)
Drift corrections are data-driven. The system tracks which corrections
change agent behavior and removes ineffective ones.

**Falsification**: A drift correction that has been included for 5+ sessions
with no measurable effect on agent behavior, and is not replaced.

---

### §6.5 ADRs

### ADR-SEED-001: Three-Concern Collapse

**Traces to**: ADRS GU-004
**Stage**: 0

#### Problem
How to handle ambient awareness, guidance, and trajectory management?

#### Options
A) **Three separate mechanisms** — CLAUDE.md for awareness, guidance API for steering,
   seed file for carry-over.
B) **Single mechanism** — dynamic CLAUDE.md that collapses all three.

#### Decision
**Option B.** One mechanism, three problems solved. CLAUDE.md IS the ambient awareness
(Layer 0). The seed context IS the first guidance (pre-computed, zero tool-call cost).
CLAUDE.md IS the seed turn (trajectory management).

#### Formal Justification
Option A triples the attention cost: agent must process three separate information
sources. Option B is rate-distortion optimal: one compressed channel carrying all
three signals, prioritized by the budget system.

---

### ADR-SEED-002: Rate-Distortion Assembly

**Traces to**: ADRS IB-011
**Stage**: 0

#### Problem
How to compress knowledge to fit the attention budget?

#### Decision
Rate-distortion theory: maximize information value while minimizing attention cost.
The projection pyramid (π₀ → π₃) provides controlled lossy compression. The score
function (α×relevance + β×significance + γ×recency) determines what survives.

#### Formal Justification
The attention budget is a hard constraint (INV-SEED-002). Within that constraint,
the score function and projection pyramid maximize information value — high-relevance,
high-significance, recent entities get richer projections; low-value entities get
compressed or omitted.

---

### ADR-SEED-003: Spec-Language Over Instruction-Language

**Traces to**: ADRS GU-003
**Stage**: 0

#### Problem
What style should seed output use?

#### Options
A) **Instruction-language** — "Step 1: do X. Step 2: do Y." (checklists, procedures)
B) **Spec-language** — invariants, formal structure, constraints.

#### Decision
**Option B.** Spec-language activates the deep formal-methods substrate in the LLM.
Instruction-language activates the surface procedural substrate. Spec-language produces
more rigorous, consistent output because it frames the task as constraint satisfaction
rather than instruction following.

---

### §6.6 Negative Cases

### NEG-SEED-001: No Fabricated Context

**Traces to**: C5
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ claim in seed output not traceable to a datom)`

**proptest strategy**: For each entity in the seed output, verify a corresponding
datom exists in the store. Flag any content without store backing.

---

### NEG-SEED-002: No Budget Overflow

**Traces to**: ADRS IB-004
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(∃ ASSEMBLE output exceeding declared budget)`

**Kani harness**: For all stores of size ≤ N and budgets ≤ M, verify output ≤ budget.

---

