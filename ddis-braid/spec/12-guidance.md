> **Namespace**: GUIDANCE | **Wave**: 3 (Intelligence) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §12. GUIDANCE — Methodology Steering

> **Purpose**: Guidance is the anti-drift mechanism — continuous methodology steering
> that counteracts the basin competition between DDIS methodology (Basin A) and pretrained
> coding patterns (Basin B). Without guidance, agents drift into Basin B within 15–20 turns.
>
> **Traces to**: SEED.md §7 (Self-Improvement Loop), §8 (Interface Principles),
> ADRS GU-001–008

### §12.1 Level 0: Algebraic Specification

The guidance system is a **comonad** (GU-001):

```
W(A) = (StoreState, A)

extract : W(A) → A
  — given the store state and a value, extract the value (current guidance)

extend : (W(A) → B) → W(A) → W(B)
  — given a function that uses store context to produce guidance,
    lift it to produce guidance at every store state
```

**Basin competition model** (GU-006):
```
P(Basin_A, t) = probability of methodology-adherent behavior at time t
P(Basin_B, t) = probability of pretrained-pattern behavior at time t

P(Basin_A, t) + P(Basin_B, t) = 1

Without intervention: P(Basin_B, t) → 1 as t → ∞ (pretrained patterns dominate)
With guidance injection: P(Basin_A, t) maintained above threshold τ
```

**Anti-drift energy** is injected via six mechanisms (GU-007) that collectively
maintain `P(Basin_A) > τ`:

```
E_drift = E_preemption + E_injection + E_detection + E_gate + E_alarm + E_harvest

Each Eᵢ > 0 is a positive contribution to Basin A probability.
The system is stable when E_drift > E_decay (natural drift toward Basin B).
```

**Laws**:
- **L1 (Continuous steering)**: Every tool response includes a guidance footer (GU-005)
- **L2 (Spec-language phrasing)**: Guidance uses invariant references and formal structure, not checklists (GU-003)
- **L3 (Intention coherence)**: Actions scored higher if they advance active intentions (GU-008)
- **L4 (Empirical improvement)**: Learned guidance is effectiveness-tracked and pruned below threshold (GU-001)

### §12.2 Level 1: State Machine Specification

**State**: `Σ_guidance = (topology: Graph<GuidanceNode>, learned: Map<EntityId, Effectiveness>, drift_score: f64, mechanisms: [Mechanism; 6])`

**Transitions**:

```
QUERY_GUIDANCE(Σ, agent_state, lookahead) → (actions, tree) where:
  POST: evaluates guidance node predicates against agent state
  POST: returns scored actions + optional lookahead tree (1–5 steps)
  POST: intention-aligned actions scored higher: if postconditions(a) ∩ goals(i) ≠ ∅:
        score(a) += intention_alignment_bonus

INJECT(Σ, tool_response) → tool_response' where:
  POST: tool_response' = tool_response + guidance_footer
  POST: footer contains: (a) specific ddis command, (b) active invariant refs,
        (c) uncommitted observation count, (d) drift warning if applicable
  POST: footer size determined by k*_eff (GU-005)

DETECT_DRIFT(Σ, access_log) → Σ' where:
  POST: analyze transact gap (> 5 bash commands without transact = drift signal)
  POST: analyze tool absence (key tools unused for > threshold turns)
  POST: Σ'.drift_score updated
  POST: if drift_score > threshold: emit GoalDrift signal

EVOLVE(Σ, outcome_data) → Σ' where:
  POST: update effectiveness scores for learned guidance based on outcomes
  POST: prune guidance below effectiveness threshold (0.3)
  POST: effective patterns promoted to higher confidence
```

**Six anti-drift mechanisms** (GU-007):
1. **Guidance Pre-emption**: CLAUDE.md rules require `ddis guidance` before code writing
2. **Guidance Injection**: Every tool response includes next-action footer
3. **Drift Detection**: Access log analysis for transact gap, tool absence
4. **Pre-Implementation Gate**: `ddis pre-check --file <path>` returns GO/CAUTION/STOP
5. **Statusline Drift Alarm**: Uncommitted count, time since last transact, warning indicator
6. **Harvest Safety Net**: Recovers un-transacted observations at session end

### §12.3 Level 2: Implementation Contract

```rust
pub struct GuidanceTopology {
    pub nodes: HashMap<EntityId, GuidanceNode>,
    pub edges: Vec<(EntityId, EntityId)>,
}

pub struct GuidanceNode {
    pub entity: EntityId,
    pub predicate: QueryExpr,  // Datalog predicate over store state
    pub actions: Vec<GuidanceAction>,
    pub learned: bool,
    pub effectiveness: f64,
}

pub struct GuidanceAction {
    pub command: String,          // specific ddis command
    pub invariant_refs: Vec<String>, // e.g., "INV-STORE-001"
    pub postconditions: Vec<EntityId>,
    pub score: f64,
}

pub struct GuidanceFooter {
    pub next_action: String,
    pub invariant_refs: Vec<String>,
    pub uncommitted_count: u32,
    pub drift_warning: Option<String>,
}

impl GuidanceTopology {
    /// Query guidance for current state with lookahead
    pub fn query(&self, store: &Store, agent: &AgentId, lookahead: u8)
        -> GuidanceResult { ... }

    /// Generate footer for tool response
    pub fn footer(&self, store: &Store, k_eff: f64) -> GuidanceFooter { ... }
}
```

### §12.4 Invariants

### INV-GUIDANCE-001: Continuous Injection

**Traces to**: ADRS GU-005
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
`∀ tool_response r: ∃ footer f: r' = r ⊕ f`
Every tool response includes a guidance footer.

#### Level 1 (State Invariant)
The INJECT transition always fires as post-processing on tool output.
No tool response reaches the agent without a guidance footer.

#### Level 2 (Implementation Contract)
The CLI output pipeline appends a footer to every response. The footer
is computed from current store state and k*_eff.

**Falsification**: Any tool response reaches the agent without a guidance footer.

---

### INV-GUIDANCE-002: Spec-Language Phrasing

**Traces to**: ADRS GU-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
Guidance text references invariant IDs, formal structures, and spec elements.
Never instruction-language ("do step 1, then step 2") — always spec-language
("INV-STORE-001 requires append-only; current operation would mutate").

#### Level 1 (State Invariant)
Guidance generation templates use invariant references. The template engine
pulls from the store's invariant index, not from hardcoded instruction strings.

**Falsification**: Guidance output contains a numbered checklist or imperative
instruction without invariant reference.

---

### INV-GUIDANCE-003: Intention-Action Coherence

**Traces to**: ADRS GU-008
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
`∀ action a, intention i:
  postconditions(a) ∩ goals(i) ≠ ∅ ⟹ score(a) += intention_alignment_bonus`

Actions that advance active intentions are scored higher in guidance output.

#### Level 1 (State Invariant)
The QUERY_GUIDANCE transition computes intersection between action postconditions
and active intention goals. Non-empty intersection adds a bonus to action score.

**Falsification**: An action that advances an active intention is scored
identically to an action that does not.

---

### INV-GUIDANCE-004: Drift Detection Responsiveness

**Traces to**: ADRS GU-007 (mechanism 3)
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
`transact_gap > 5 ⟹ drift_signal_emitted`
If an agent executes more than 5 bash commands without a transact, the drift
detection mechanism emits a GoalDrift signal.

#### Level 1 (State Invariant)
The DETECT_DRIFT transition monitors the access log for transact gaps and
tool absence patterns. When thresholds are exceeded, a signal is emitted.

**Falsification**: An agent executes 10+ bash commands without a transact
and no drift signal is emitted.

---

### INV-GUIDANCE-005: Learned Guidance Effectiveness Tracking

**Traces to**: ADRS GU-001
**Verification**: `V:PROP`
**Stage**: 4

#### Level 0 (Algebraic Law)
`∀ learned_guidance g: effectiveness(g) < 0.3 ⟹ ◇ retracted(g)`
Learned guidance below the effectiveness threshold is eventually retracted.

Effectiveness is computed from outcome data:
`effectiveness(g) = success_rate(actions_taken_following_g)`

#### Level 1 (State Invariant)
The EVOLVE transition updates effectiveness scores and prunes below-threshold
learned guidance. System-default guidance is never pruned.

**Falsification**: Learned guidance with effectiveness < 0.3 persists after
5+ sessions without being retracted.

---

### INV-GUIDANCE-006: Lookahead via Branch Simulation

**Traces to**: ADRS GU-002
**Verification**: `V:PROP`
**Stage**: 2

#### Level 0 (Algebraic Law)
Lookahead (1–5 steps) simulates action consequences by creating a virtual branch,
applying hypothetical actions, and evaluating the resulting store state.

`lookahead(actions, n) = evaluate(apply(fork(store), actions[0..n]))`

#### Level 1 (State Invariant)
Virtual branches created for lookahead are never committed to trunk.
Lookahead branches are ephemeral — created, evaluated, and discarded within
the QUERY_GUIDANCE transition.

**Falsification**: A lookahead branch persists after the guidance query completes
or its datoms leak into trunk.

---

### INV-GUIDANCE-007: Dynamic CLAUDE.md as Optimized Prompt

**Traces to**: ADRS GU-004, PO-014
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)

CLAUDE.md generation is a function `G: StoreState → CLAUDEmd` that produces a
field configuration over the agent's activation manifold, subject to:

1. **Constraint budget**: `|constraints(G(s))| ≤ k*(fresh_session)` — the total
   constraint count must not exceed the k* capacity of a fresh session
2. **Ambient/active partition**: `G(s) = ambient(s) ⊕ active(s)` where
   `|ambient(s)| ≤ 80 tokens` — ambient content is k*-exempt and always present
3. **Demonstration density**: `demonstrations(G(s)) / max(1, constraint_clusters(G(s))) ≥ 1.0`
   — at least one demonstration per cluster of constraints
4. **Effectiveness tracking**: `∀ correction c: sessions_without_effect(c) > 5 ⟹ ◇ replaced(c)`
   — corrections that show no measurable effect after 5 sessions are replaced

#### Level 1 (State Invariant)

The GENERATE-CLAUDE-MD operation follows a typestate pipeline:

```
MeasureDrift → DiagnoseDrift → SelectCorrections → ValidateBudget → Emit
```

- **MeasureDrift**: query store for recent drift patterns across sessions
- **DiagnoseDrift**: classify drift signals (basin competition, spec-language
  decay, tool avoidance)
- **SelectCorrections**: choose corrections from drift patterns, respecting
  k* budget
- **ValidateBudget**: verify `|constraints| ≤ k*`, verify demonstration ratio
  ≥ 1.0, verify ambient ≤ 80 tokens
- **Emit**: produce the CLAUDE.md content

Ineffective corrections are replaced by new corrections derived from recent
drift patterns. The pipeline cannot skip the ValidateBudget stage.

#### Level 2 (Implementation Contract)

```rust
pub struct ClaudeMdConfig {
    pub ambient: AmbientSection,     // |ambient| ≤ 80 tokens, k*-exempt
    pub active: ActiveSection,       // |active| ≤ k*(fresh_session) - |ambient|
}

pub struct AmbientSection {
    pub tool_awareness: String,      // Tool names + one-line purposes
    pub identity: String,            // Project identity
}
// Invariant: ambient.token_count() <= 80

pub struct ActiveSection {
    pub demonstrations: Vec<Demonstration>,
    pub constraints: Vec<DriftCorrection>,
    pub context: SessionContext,
}
// Invariant: demonstrations.len() >= constraints.chunks(3).len()
```

**Falsification**: A drift correction persists in generated CLAUDE.md for 10+
sessions with no measurable improvement in the targeted drift metric, OR
generated CLAUDE.md exceeds k* constraint budget for a fresh session, OR
ambient section exceeds 80 tokens, OR zero demonstrations accompany a cluster
of 3+ constraints, OR the generator emits without validating constraint count
(pipeline stage skipped).

---

### §12.5 ADRs

### ADR-GUIDANCE-001: Comonadic Topology Over Flat Rules

**Traces to**: ADRS GU-001
**Stage**: 1

#### Problem
How should guidance be structured — flat rules or a graph topology?

#### Decision
Comonadic topology: guidance nodes are entities with Datalog predicates.
The `(StoreState, A)` comonad means guidance is always contextualized by the
full store state. Nodes can be traversed, composed, and extended.

#### Formal Justification
Flat rules don't compose (interaction between rules is implicit and fragile).
The comonadic structure makes composition explicit: `extend` lifts a guidance
function to operate over the full topology. Agents can contribute new guidance
nodes that integrate with existing ones via graph edges.

---

### ADR-GUIDANCE-002: Basin Competition as Central Failure Model

**Traces to**: ADRS GU-006
**Stage**: 0

#### Problem
What is the primary failure mode in agent-methodology interaction?

#### Decision
Basin competition between DDIS methodology (Basin A) and pretrained coding patterns
(Basin B). As k*_eff decreases, Basin B's pull increases. At crossover, Basin B
captures the trajectory and the agent's own non-DDIS outputs reinforce it.

#### Formal Justification
This is not a memory problem (bigger context doesn't help — it just delays
the crossover). It is a dynamical systems problem: two attractors competing for
trajectory. The six anti-drift mechanisms are energy injections that maintain
Basin A dominance. Understanding this is prerequisite to designing effective
countermeasures.

---

### ADR-GUIDANCE-003: Six Integrated Mechanisms Over Single Solution

**Traces to**: ADRS GU-007
**Stage**: 1

#### Problem
How many anti-drift mechanisms are needed?

#### Decision
Six. No single mechanism is sufficient — they compose: pre-emption prevents,
injection steers, detection catches, gate forces, alarm makes visible, harvest
recovers. The failure mode of each mechanism is covered by the others.

#### Formal Justification
Defense in depth. Pre-emption fails when agents skip the CLAUDE.md check.
Injection fails when agents ignore footer. Detection fails for novel drift
patterns. Gate fails if agents don't call pre-check. Alarm fails if agent
doesn't read statusline. Harvest fails if session terminates abnormally.
No mechanism is single-point-of-failure because each covers the others' gaps.

---

### ADR-GUIDANCE-004: Spec-Language Over Instruction-Language

**Traces to**: ADRS GU-003
**Stage**: 0

#### Problem
What language register should guidance use?

#### Options
A) Instruction-language — "Do X, then Y, then Z" (checklists)
B) Spec-language — "INV-STORE-001 requires X; current state violates Y"

#### Decision
**Option B.** Spec-language activates the deep reasoning substrate of LLMs
(formal pattern matching, logical inference). Instruction-language activates
the surface substrate (compliance, procedure following). The deep substrate
produces more robust behavior under context pressure.

#### Formal Justification
This is empirically validated: demonstration-style prompts outperform
constraint-style prompts for LLMs. "Demonstration, not constraint list" (IB-002).
Spec-language is the formal analogue of demonstration style applied to methodology.

---

### §12.6 Negative Cases

### NEG-GUIDANCE-001: No Tool Response Without Footer

**Traces to**: ADRS GU-005
**Verification**: `V:PROP`

**Safety property**: `□ ¬(tool_response_sent ∧ ¬footer_appended)`

Every tool response includes a guidance footer. No response reaches the agent
without methodology steering.

**proptest strategy**: Invoke all CLI commands with random arguments. Verify
every output contains a guidance footer section.

---

### NEG-GUIDANCE-002: No Lookahead Branch Leak

**Traces to**: ADRS GU-002
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(lookahead_branch_committed_to_trunk)`

Virtual branches created for lookahead simulation must never be committed.
They are ephemeral evaluation contexts, not real branches.

**proptest strategy**: Run random lookahead sequences. After each, verify
trunk contains exactly the datoms it had before lookahead.

**Kani harness**: Verify that the `lookahead` function cannot call `commit`.

---

### NEG-GUIDANCE-003: No Ineffective Guidance Persistence

**Traces to**: ADRS GU-001
**Verification**: `V:PROP`

**Safety property**: `□ ¬(learned_guidance_effectiveness < 0.3 ∧ age > 5_sessions ∧ ¬retracted)`

Learned guidance that fails to improve outcomes must be pruned. The system
must not accumulate ineffective guidance that wastes agent attention budget.

**proptest strategy**: Create learned guidance with low effectiveness scores.
Run EVOLVE transitions. Verify pruning occurs within 5 sessions.

---

