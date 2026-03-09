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

> **Note**: The Dynamic CLAUDE.md mechanism spans three namespaces: SEED (generation, INV-SEED-007/008), GUIDANCE (content, INV-GUIDANCE-007), and INTERFACE (delivery via MCP). See SEED.md §7 for the unified concept.

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
  1. Pre-allocate pinned intentions (INV-SEED-002, INV-SEED-006):
     a. Compute B_pinned = Σ|intention_i at π₀| for all active intentions
     b. If B_pinned ≥ budget: emit BudgetExhaustedByIntentions signal,
        assemble only pinned intentions + harvest imperative, STOP
     c. B_available = budget - B_pinned
  2. Score non-pinned entities: score(e) = α×relevance + β×significance + γ×recency
  3. Sort by score (descending)
  4. For each non-pinned entity in order:
     a. Select projection level based on remaining B_available:
        >2000 tokens: π₀ (full datoms) for top entities, π₁ for others
        500–2000:     π₁/π₂
        200–500:      π₂ for top, omit others
        ≤200:         single-line status + single guidance action
     b. Subtract token cost from B_available
     c. If B_available exhausted, stop
  5. Apply section compression priority (INV-SEED-004):
     compress State before Constraints before Orientation before Warnings before Directive
  6. Insert demonstrations for constraint clusters if B_available permits (INV-SEED-005)
  7. Record projection pattern for reification learning (AS-008)
  8. Check staleness for observation entities (UA-007)

POST:
  |result| ≤ budget (token count)
  structural dependency coherence (no entity without its dependencies)
  all active intentions included at π₀
```

#### Seed Output Template (ADR-SEED-004)

```
Seed output follows a five-part template:
  (1) Orientation — project identity, current phase, recent session history
  (2) Constraints — relevant INVs, settled ADRs, negative cases for current task
  (3) State — relevant datoms, artifacts, frontier, recent changes
  (4) Warnings — drift signals, open questions, uncertainties, harvest alerts
  (5) Directive — next task, acceptance criteria, active guidance corrections

Formatted as spec-language (INV-GUIDANCE-SEED-001): invariants and formal
structure, NOT instruction-language (steps, checklists).

Section compression priority (INV-SEED-004):
  Under budget pressure, compress in this order (first to compress → last):
    State > Constraints > Orientation > Warnings > Directive
  State absorbs compression first (reconstructible from store).
  Directive absorbs compression last (directly controls behavior).

Token allocation by remaining budget:
  > 2000 tokens:  Full detail in all sections. π₀ for State items.
  500–2000:       Compress State to π₁. Keep Constraints at full IDs.
  200–500:        Orientation (50 tok) + Directive (100 tok) + top-3 Warnings.
  ≤ 200:          Single-line orientation + single-line directive + harvest warning.

Demonstration density (INV-SEED-005):
  Include ≥1 demonstration per constraint cluster when budget > 1000 tokens.
  A demonstration is a concrete 20–40 token example showing compliance.
```

---

### §6.3 Level 2: Interface Specification

```rust
/// Cue for ASSOCIATE — determines how schema neighborhood discovery starts.
/// `depth` and `breadth` enforce INV-SEED-003 (ASSOCIATE Boundedness).
/// Defaults: depth=3, breadth=10.
pub enum AssociateCue {
    Semantic { text: String, depth: usize, breadth: usize },
    Explicit { seeds: Vec<EntityId>, depth: usize, breadth: usize },
}

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

// --- Free functions (ADR-ARCHITECTURE-001) ---

/// ASSOCIATE — discover relevant schema neighborhood.
/// Free function: association is a query-layer operation that reads from the store.
pub fn associate(store: &Store, cue: AssociateCue) -> SchemaNeighborhood;

/// ASSEMBLE — build budget-aware context.
/// Free function: assembly is a compression operation that reads from the store
/// and produces formatted seed output.
pub fn assemble(
    store: &Store,
    query_results: &QueryResult,
    neighborhood: &SchemaNeighborhood,
    budget: usize,
) -> AssembledContext;

/// SEED — full pipeline: associate → query → assemble.
/// Composite free function. Provenance recording (INV-STORE-014) is handled
/// by the caller via a separate Store::transact() call.
pub fn assemble_seed(store: &Store, task: &str, budget: usize) -> SeedOutput;

/// Generate dynamic CLAUDE.md from store state.
/// Free function: generates formatted CLAUDE.md by querying the store.
pub fn generate_claude_md(
    store: &Store,
    focus: &str,
    agent: AgentId,
    budget: usize,
) -> Result<String, SeedError>;
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
  Let B_pinned = Σ|intention_i at π₀| for all active intentions (INV-SEED-006)
  Let B_available = B - B_pinned
  |output_non_pinned| ≤ B_available

  Degenerate case: if B_pinned ≥ B, emit BudgetExhaustedByIntentions signal,
  assemble only pinned intentions + harvest imperative.

  Total output: |output| ≤ B (in tokens)
```

#### Level 1 (State Invariant)
The assembled context never exceeds the declared budget. Pinned intentions
(INV-SEED-006) are pre-allocated from the budget before other content is
considered. If the relevant non-pinned information exceeds the remaining
budget, lower-priority content is dropped (projected to coarser levels).

When pinned intentions alone exhaust the budget (B_pinned ≥ B), the system
emits a BudgetExhaustedByIntentions signal and assembles only the pinned
intentions plus the harvest imperative — no other content is included.

**Falsification**: An ASSEMBLE output whose token count exceeds the budget parameter,
OR an ASSEMBLE operation that drops a pinned intention to make room for non-pinned
content.

**proptest strategy**: Generate stores of varying sizes with varying numbers of
active intentions. Assemble with varying budgets (including budgets smaller than
total pinned intention size). Verify: (1) output token count ≤ budget for all
combinations, (2) all active intentions present in output, (3) when B_pinned ≥ B,
only intentions and harvest imperative appear.

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

### INV-SEED-004: Section Compression Priority

**Traces to**: SEED §5, §8, ADRS IB-004, PO-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
Under budget pressure, sections compress in priority order (first-to-compress → last):
  1. State       — lowest marginal value per token; reconstructible via store queries
  2. Constraints — reconstructible from store; can degrade to ID-only references
  3. Orientation — short, mostly fixed across sessions; compress but never omit
  4. Warnings    — safety-critical, high behavioral leverage per token
  5. Directive   — directly controls agent behavior; last to compress

∀ ASSEMBLE operations under budget B:
  if tokens(full_seed) > B:
    compress(State) before compress(Constraints)
    compress(Constraints) before compress(Orientation)
    compress(Orientation) before compress(Warnings)
    compress(Warnings) before compress(Directive)
```

#### Level 1 (State Invariant)
The ASSEMBLE pipeline compresses sections in priority order, not by section
position. State absorbs compression first because it is fully reconstructible
from the store. Directive absorbs compression last because it directly controls
the agent's next action. Warnings are second-last because they are safety-critical
and have high behavioral leverage per token.

**Falsification**: An ASSEMBLE operation that compresses Warnings or Directive while
State still contains verbose detail, or that omits Directive before omitting State.

---

### INV-SEED-005: Demonstration Density

**Traces to**: ADRS GU-003, GU-004, INV-GUIDANCE-007
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
```
∀ constraint clusters C in the seed Constraints section:
  if |C| ≥ 2 and budget permits:
    ∃ at least one demonstration d showing compliance with C
    d is a concrete 20-40 token example, not prose

A constraint cluster is a set of related INVs/ADRs/NEGs that govern
the same behavioral domain (e.g., {INV-STORE-001, INV-STORE-003}
form an append-only + content-addressed cluster).
```

#### Level 1 (State Invariant)
The seed includes at least one worked example per constraint cluster when
budget permits. Demonstrations activate the LLM's pattern-completion substrate
far more effectively than invariant statements alone. A 30-token demonstration
is worth approximately 10x its token cost in behavioral activation.

Under budget pressure, demonstrations compress before their parent constraints
(constraints without demonstrations are still useful; demonstrations without
constraints lack context).

**Falsification**: A seed with 3+ related constraints and budget > 1000 tokens
that contains zero demonstrations for the cluster.

---

### INV-SEED-006: Intention Anchoring

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

### INV-SEED-007: Dynamic CLAUDE.md Relevance

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

### INV-SEED-008: Dynamic CLAUDE.md Improvement

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

### ADR-SEED-004: Unified Five-Part Seed Template

**Traces to**: ADRS IB-010, GU-003
**Stage**: 0

#### Problem
The spec (Context/Invariants/Artifacts/Open questions/Active guidance) and guide
(Orientation/Decisions/Context/Warnings/Task) used different five-part templates.
Which structure should the seed output follow?

#### Options
A) **Spec template** — knowledge-facing: what exists in the store.
B) **Guide template** — agent-facing: orient the agent and direct action.
C) **Unified template** — reconcile both into a single structure that is both
   knowledge-grounded and agent-directing.

#### Decision
**Option C.** Unified five-part template:
1. **Orientation** — project identity, current phase, recent session history
2. **Constraints** — relevant INVs, settled ADRs, negative cases
3. **State** — relevant datoms, artifacts, frontier, recent changes
4. **Warnings** — drift signals, open questions, uncertainties, harvest alerts
5. **Directive** — next task, acceptance criteria, active guidance corrections

The spec's parts map to the unified template: Context→Orientation+State,
Invariants→Constraints, Artifacts→State, Open questions→Warnings,
Active guidance→Directive. The guide's parts map similarly: Orientation→Orientation,
Decisions→Constraints, Context→State, Warnings→Warnings, Task→Directive.

#### Consequences
- All documents (spec, guide, ADRS.md IB-010) reference the same template
- "Constraints" is broader than "Invariants" — includes ADRs and negative cases
- "Directive" is more action-oriented than "Active guidance"
- The SeedOutput struct uses these five field names

### ADR-SEED-005: Four Knowledge Types

**Traces to**: SEED §5, §8, ADRS AA-004
**Stage**: 1

#### Problem
The ASSOCIATE operation (§6.2) traverses structural edges (entity references, schema
relationships) to discover relevant context. But agents also accumulate metacognitive
knowledge — beliefs about what is true, intentions about what to do, learned associations
between concepts, and strategic heuristics about how to work effectively. How should the
seed assembly system represent and traverse these different knowledge types?

#### Options
A) **Flat entity model** — all knowledge is just entities with attributes. No distinction
   between a fact about the codebase and a learned association between two concepts.
   Simple but loses the metacognitive structure that makes associations useful.
B) **Two-tier model** — distinguish between "ground facts" and "meta-knowledge." Too
   coarse: it lumps beliefs, intentions, associations, and heuristics into one bucket
   despite their different traversal and trust characteristics.
C) **Four knowledge types** — Belief, Intention, Learned Association, and Strategic
   Heuristic, each as a distinct entity type with type-specific schema and traversal
   behavior.

#### Decision
**Option C.** The metacognitive layer defines four entity types:

1. **Belief** — an agent's assessment that something is true or likely. Carries confidence
   level and evidential basis. Beliefs are defeasible (can be retracted when evidence
   changes). Used in ASSOCIATE to weight relevance of related entities.

2. **Intention** — an agent's commitment to a future action. Pinned at π₀ during assembly
   (INV-SEED-006). Intentions track progress state and have causal links to the entities
   they intend to modify.

3. **Learned Association** — a discovered relationship between entities that is not part
   of the structural schema. Typed as one of: `:causal` (A causes B), `:correlative`
   (A and B co-occur), `:architectural` (A and B share design pattern), `:strategic`
   (A informs approach to B), `:analogical` (A is structurally similar to B). ASSOCIATE
   MUST traverse learned associations alongside structural edges
   (INV-ASSOCIATE-LEARNED-001 from AA-004).

4. **Strategic Heuristic** — a reusable pattern or rule derived from experience. E.g.,
   "when uncertainty on entity X exceeds 0.5, check related invariants before proceeding."
   Heuristics are injected into guidance (§12) and have effectiveness tracking (was following
   this heuristic beneficial in past sessions?).

#### Formal Justification
The four types capture qualitatively different knowledge structures:
- Beliefs have truth values and confidence; they are epistemic.
- Intentions have completion states and causal targets; they are conative.
- Associations have type-classified edges; they extend the knowledge graph.
- Heuristics have effectiveness scores; they are procedural.

Collapsing these into a flat model (Option A) loses the type-specific traversal behavior
that makes ASSOCIATE effective. The ASSOCIATE operation traverses learned associations
alongside structural edges (AA-004), which means an agent searching for context about
entity X will discover not just X's structural neighbors but also entities that previous
agents found relevant to X through experience.

This is the "system learns useful ways to look at data" property (AS-008: projection
reification as learning mechanism). Learned associations are the fine-grained version of
reified projections — individual edges rather than entire query patterns.

#### Consequences
- ASSOCIATE discovers context through both structural and experiential paths
- Intentions are never lost during seed assembly (INV-SEED-006 anchoring)
- Strategic heuristics compose with the guidance system (§12) for empirically-grounded
  methodology steering
- The five association types (causal, correlative, architectural, strategic, analogical)
  provide rich traversal options without requiring agents to understand the full schema

#### Falsification
The four types are wrong if: (1) a significant category of metacognitive knowledge does
not fit any of the four types (missing type), or (2) one of the four types is never used
in practice (unnecessary type), or (3) the ASSOCIATE traversal of learned associations
produces more noise than signal (association quality is too low to be useful).

---

### ADR-SEED-006: Dynamic CLAUDE.md Generation

**Traces to**: SEED §5, §8, ADRS GU-004, PO-014
**Stage**: 1

#### Problem
The static CLAUDE.md file that steers agent behavior is written once and gradually diverges
from the project's actual state. It cannot adapt to observed drift patterns, changing
priorities, or the specific agent's context. How should the system generate contextually
relevant agent instructions that collapse ambient awareness, guidance, and trajectory
management into a single mechanism?

#### Options
A) **Static CLAUDE.md** — a hand-maintained file that agents read at session start.
   Works initially but decays: no adaptation to drift patterns, no personalization to
   the current task, no budget awareness. The current approach during pre-Stage-0.
B) **Template-based generation** — fill in a template with current store state (active
   tasks, recent changes, open issues). Better than static but cannot incorporate
   empirical drift corrections or adapt priority ordering to agent behavior patterns.
C) **Seven-step dynamic generation** — a formal pipeline that queries the store for
   relevant context, applies empirical drift corrections, and assembles a budget-aware
   CLAUDE.md tailored to the current focus, agent, and available attention budget.

#### Decision
**Option C.** Dynamic CLAUDE.md is generated by the GENERATE-CLAUDE-MD operation with
signature `(Store, focus, agent, budget) -> Markdown`. The pipeline follows seven steps:

```
GENERATE-CLAUDE-MD pipeline:
  (1) ASSOCIATE with focus     — discover relevant schema neighborhood
  (2) QUERY active intentions  — what the agent should be working on
  (3) QUERY governing INVs     — which invariants constrain the current task
  (4) QUERY uncertainty markers — what is uncertain in the relevant neighborhood
  (5) QUERY competing branches — are there parallel efforts on the same entities?
  (6) QUERY drift patterns     — what drift has been observed in recent sessions?
  (7) ASSEMBLE at budget       — compress into budget-aware output
```

The output follows the unified five-part seed template (ADR-SEED-004):
Orientation, Constraints, State, Warnings, Directive.

Priority ordering within budget:
`tools > task_context > risks > drift_corrections > seed_context`

This collapses three concerns (ADR-SEED-001):
1. **Ambient awareness** (Layer 0) — CLAUDE.md IS the ambient context
2. **Guidance** (Layer 3) — seed context IS the first guidance, pre-computed at zero
   tool-call cost
3. **Trajectory management** — CLAUDE.md IS the seed turn, carrying forward the relevant
   state from prior sessions

#### Formal Justification
The seven-step pipeline ensures that every section of the generated CLAUDE.md is
empirically grounded in store state (INV-SEED-007: relevance — removing any section
would change agent behavior). The pipeline queries are ordered by descending behavioral
leverage: active intentions direct the agent's next action; governing invariants
constrain it; uncertainty markers warn about risks; competing branches prevent
wasted parallel effort; drift patterns encode learned corrections.

The three-concern collapse (ADR-SEED-001) is rate-distortion optimal: one compressed
channel carrying all three signals, prioritized by the budget system. Three separate
mechanisms would triple the attention cost.

Dynamic generation enables the self-improvement property (INV-SEED-008): drift
corrections that show no effect after 5 sessions are automatically replaced. This
makes CLAUDE.md a living document that converges toward the most effective steering
instructions for the specific project and agent combination.

#### Consequences
- CLAUDE.md adapts to the project's current state every session
- Drift corrections are empirically derived, not speculatively authored
- Budget awareness prevents CLAUDE.md from consuming excessive attention
- The generation pipeline is itself queryable — the store records which queries
  produced which CLAUDE.md sections, enabling meta-analysis of instruction effectiveness
- Stage 0 uses a static CLAUDE.md (this file); Stage 1 transitions to dynamic generation

#### Falsification
Dynamic generation is wrong if: (1) the generated CLAUDE.md consistently performs worse
than a well-maintained static file (generation introduces noise or irrelevant context),
or (2) the seven-step pipeline is too slow for interactive use (agent waits too long for
CLAUDE.md generation at session start), or (3) drift corrections oscillate rather than
converge (the system cannot distinguish effective from ineffective corrections).

---

### ADR-SEED-007: Seed Document Eleven-Section Structure

**Traces to**: SEED §1–§11, ADRS LM-016
**Stage**: 0

#### Problem
SEED.md is the foundational design document from which all specification and implementation
flow. What structure should it follow? The structure determines how efficiently a new agent
(or a returning agent with a fresh context window) can orient to the project and begin
productive work.

#### Options
A) **Technical reference structure** — organized by component (Store, Query, Harvest, etc.).
   Good for lookup but poor for initial orientation: an agent reading about the Store
   has no context for why the Store exists or what problem it solves.
B) **Narrative structure** — a flowing document that tells the story of DDIS from problem
   to solution. Good for first read but poor for reference: finding a specific design
   decision requires reading linearly.
C) **Eleven-section progressive deepening** — starts with identity and problem statement,
   progresses through formalism and core abstractions, then covers lifecycle, reconciliation,
   self-improvement, interfaces, existing context, roadmap, and rationale. Each section is
   self-contained but builds on prior sections. Minimal formalism — mathematical notation
   flows to SPEC.md, not SEED.md.

#### Decision
**Option C.** SEED.md follows an eleven-section structure:

1. **What DDIS Is** — identity statement: coherence verification system
2. **The Problem** — divergence as the fundamental challenge; coherence leads, memory
   subordinated
3. **Specification Formalism** — bridges why (§2) and how (§4): invariants, ADRs,
   negative cases, uncertainty markers
4. **Core Abstraction** — the datom, EAV model, five axioms, content-addressable identity
5. **Harvest/Seed Lifecycle** — bounded conversation cycles, knowledge extraction and
   assembly
6. **Reconciliation Mechanisms** — conflict resolution, deliberation, bilateral specification
7. **Self-Improvement Loop** — DDR feedback, fitness function, convergence tracking
8. **Interface Principles** — five-layer interface, budget-aware output, guidance injection
9. **Existing Codebase** — the Go CLI as reference material, gap analysis approach
10. **Staged Roadmap** — Stage 0 through Stage 4, deliverables and success criteria
11. **Design Rationale** — meta-level "why these choices" connecting back to the problem

#### Formal Justification
The eleven sections follow a progressive deepening pattern:
- §1–§2: **What and Why** — orient the agent to identity and problem
- §3: **Formalism Bridge** — how the specification is structured (critical for agents
  that will write or read spec elements)
- §4–§5: **Core Mechanisms** — the datom store and the lifecycle that uses it
- §6–§7: **Coordination and Improvement** — how the system handles disagreement and
  evolves
- §8–§9: **Interface and Context** — how agents interact and what already exists
- §10–§11: **Roadmap and Rationale** — what to build next and why these choices

The key constraint is **minimal formalism in the seed**. Mathematical notation
(`σ = (σ_e, σ_a, σ_c)`, `LIVE(S) = fold(...)`) belongs in the spec, not in the seed.
The seed uses natural language and intuitive explanations. This is deliberate: the seed
must be accessible to any agent on first read without requiring mathematical background
that may not be in its training data. The spec contains the formal definitions; the seed
provides the conceptual grounding that makes the formal definitions interpretable.

#### Consequences
- New agents orient in a single read-through of SEED.md (progressive deepening)
- Each section is self-contained enough for targeted reference
- The separation of formalism (spec) from intuition (seed) serves different use cases:
  the seed is for understanding, the spec is for verification
- The eleven sections map naturally to spec namespaces: §4 -> STORE/SCHEMA/QUERY,
  §5 -> HARVEST/SEED, §6 -> RESOLUTION/MERGE/DELIBERATION, §7 -> BILATERAL,
  §8 -> INTERFACE/BUDGET/GUIDANCE, etc.

#### Falsification
The structure is wrong if: (1) agents consistently fail to orient after reading SEED.md
(the progressive deepening does not achieve conceptual grounding), or (2) agents
frequently cannot find the rationale for specific decisions (the narrative structure is
too poor for reference use), or (3) the "minimal formalism" constraint causes ambiguity
that the spec's formal definitions do not resolve (too little precision in the seed).

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

