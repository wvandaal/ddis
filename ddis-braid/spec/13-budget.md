> **DEPRECATED**: This file is bootstrap scaffolding. The canonical source of truth is the braid datom store. Use `braid spec show` and `braid query` to access spec elements. See ADR-STORE-019.

---

> **Namespace**: BUDGET | **Wave**: 3 (Intelligence) | **Stage**: 1
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §13. BUDGET — Attention Budget Management

> **Purpose**: The attention budget is the fundamental constraint on agent output quality.
> Budget management ensures that high-priority information is never displaced by
> low-priority output, and that tool responses degrade gracefully as context fills.
>
> **Traces to**: SEED.md §8 (Interface Principles), ADRS IB-004–007, IB-011,
> SQ-007, UA-001

### §13.1 Level 0: Algebraic Specification

The attention budget is a **monotonically decreasing resource**:

```
k*_eff : Time → [0, 1]
  — effective remaining attention at time t, measured from actual context consumption

Q(t) = k*_eff(t) × attention_decay(k*_eff(t))
  — quality-adjusted budget incorporating attention degradation

attention_decay(k) =
  | 1.0           if k > 0.6      (full quality)
  | k / 0.6       if 0.3 ≤ k ≤ 0.6 (linear degradation)
  | (k / 0.3)²    if k < 0.3      (quadratic degradation)
```

**Five-level output precedence**:
```
System > Methodology > UserRequested > Speculative > Ambient

Truncation order: Ambient first, System last.
Lower-priority output is truncated before higher-priority output is touched.
```

**Projection pyramid** (SQ-007):
```
π₀ = full datoms           (> 2000 tokens available)
π₁ = entity summaries      (500–2000 tokens)
π₂ = type summaries         (200–500 tokens)
π₃ = store summary          (≤ 200 tokens — single-line status + single guidance action)
```

**Laws**:
- **L1 (Budget monotonicity)**: `k*_eff(t+1) ≤ k*_eff(t)` — effective attention never increases within a session
- **L2 (Precedence ordering)**: Truncation always follows the five-level ordering — no level N content is truncated while level N+1 content remains
- **L3 (Minimum output)**: `output_size ≥ MIN_OUTPUT` (50 tokens) — even at critical budget, a harvest signal is always emitted

### §13.2 Level 1: State Machine Specification

**State**: `Σ_budget = (k_eff: f64, q: f64, output_budget: u32, precedence_stack: [Level; 5])`

**Transitions**:

```
MEASURE(Σ, context_data) → Σ' where:
  POST: Σ'.k_eff computed from measured context consumption
  POST: Σ'.q = Q(t) formula applied
  POST: Σ'.output_budget = max(50, Σ'.q × 200000 × 0.05)

ALLOCATE(Σ, content, priority) → output where:
  POST: content truncated to fit output_budget
  POST: truncation follows precedence: lowest priority first
  POST: guidance compression follows IB-006:
        k > 0.7: full (100–200 tokens)
        0.4–0.7: compressed (30–60 tokens)
        ≤ 0.4: minimal (10–20 tokens)
        ≤ 0.2: harvest signal only

PROJECT(Σ, entities, budget) → projection where:
  POST: pyramid level selected based on budget:
        > 2000: π₀ for top, π₁ for others
        500–2000: π₁/π₂
        200–500: π₂ for top, omit others
        ≤ 200: π₃ (single-line)
```

**Budget source precedence** (IB-004):
1. `--budget` flag (explicit)
2. `--context-used` flag (from caller)
3. Session state file `.ddis/session/context.json` (from statusline hook)
4. Transcript tail-parse (fallback)
5. Conservative default: 500 tokens

Staleness threshold: 30 seconds. Sources older than 30s are deprioritized.

### §13.3 Level 2: Implementation Contract

```rust
pub struct BudgetManager {
    pub k_eff: f64,
    pub q: f64,
    pub output_budget: u32,
}

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum OutputPrecedence {
    Ambient = 0,
    Speculative = 1,
    UserRequested = 2,
    Methodology = 3,
    System = 4,
}

impl BudgetManager {
    /// Measure k*_eff from context data
    pub fn measure(&mut self, context_used_pct: f64) {
        self.k_eff = 1.0 - context_used_pct;
        self.q = self.k_eff * self.attention_decay(self.k_eff);
        self.output_budget = (50.0_f64).max(self.q * 200_000.0 * 0.05) as u32;
    }

    fn attention_decay(&self, k: f64) -> f64 {
        if k > 0.6 { 1.0 }
        else if k >= 0.3 { k / 0.6 }
        else { (k / 0.3).powi(2) }
    }

    /// Project entities to the appropriate pyramid level
    pub fn project(&self, entities: &[EntitySummary]) -> Projection {
        match self.output_budget {
            b if b > 2000 => Projection::Full(entities),
            b if b > 500  => Projection::EntitySummary(entities),
            b if b > 200  => Projection::TypeSummary(entities),
            _             => Projection::StoreSummary,
        }
    }
}
```

### §13.4 Invariants

### INV-BUDGET-001: Output Budget as Hard Cap

**Traces to**: ADRS IB-004
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
∀ tool_response r in non-emergency mode:
  |r| ≤ max(MIN_OUTPUT, Q(t) × W × budget_fraction)

where W = context window size, budget_fraction = 0.05 (5% of remaining capacity).

Exception: harvest-imperative mode (Q(t) < 0.05, INV-INTERFACE-007) is exempt
from MIN_OUTPUT. In harvest-imperative mode, the goal is behavioral steering
(emit only the harvest imperative, ~10 tokens), not information delivery.
The MIN_OUTPUT floor exists to ensure useful output; harvest-imperative mode
has a different purpose — it tells the agent to stop and harvest.
```

#### Level 1 (State Invariant)
The ALLOCATE transition enforces the cap. Content exceeding the budget is
truncated according to precedence ordering. The MIN_OUTPUT floor (50 tokens)
applies to all non-emergency modes.

In harvest-imperative mode (Q(t) < 0.05), MIN_OUTPUT does not apply. The
MEASURE transition detects this condition and switches the output pipeline
to harvest-imperative mode, which emits only the harvest imperative.

#### Level 2 (Implementation Contract)
```rust
#[kani::ensures(|output| output.len() <= self.output_budget as usize)]
fn allocate(&self, content: &[OutputBlock]) -> Vec<u8> { ... }
```

**Falsification**: A tool response in non-emergency mode exceeds the computed
output budget, OR a tool response in harvest-imperative mode emits content
other than the harvest imperative.

---

### INV-BUDGET-002: Precedence-Ordered Truncation

**Traces to**: ADRS IB-004
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
`∀ content blocks b₁, b₂ where priority(b₁) < priority(b₂):
  truncated(b₂) ⟹ truncated(b₁)`

Higher-priority content is never truncated while lower-priority content remains.

#### Level 1 (State Invariant)
The ALLOCATE transition sorts content by precedence and fills from highest to lowest.
When budget is exhausted, remaining lower-priority content is truncated.

**Falsification**: System output truncates a Methodology-level block while
Speculative-level blocks remain in the output.

---

### INV-BUDGET-003: Quality-Adjusted Degradation

**Traces to**: ADRS IB-005
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 1

#### Level 0 (Algebraic Law)
The Q(t) formula accounts for attention quality degradation:
```
Q(t) = k*_eff(t) × attention_decay(k*_eff(t))

Q(t) degrades faster than k*_eff(t) when k*_eff < 0.6
  because attention quality drops before context fills.
```

#### Level 1 (State Invariant)
The MEASURE transition computes Q(t) using the piecewise attention_decay function.
Output budget is derived from Q(t), not raw k*_eff.

**Falsification**: Output budget is computed from raw k*_eff without applying
the attention_decay quality adjustment.

---

### INV-BUDGET-004: Guidance Compression by Budget

**Traces to**: ADRS IB-006
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
Guidance footer size is a function of k*_eff:
```
k > 0.7:    full (100–200 tokens)
0.4–0.7:    compressed (30–60 tokens)
≤ 0.4:      minimal (10–20 tokens)
≤ 0.2:      harvest signal only ("Run ddis harvest")
```

#### Level 1 (State Invariant)
The INJECT transition (from GUIDANCE namespace) selects footer size
based on the current k*_eff from the budget manager.

**Falsification**: At k*_eff = 0.1, the guidance footer is 100+ tokens instead
of a minimal harvest signal.

---

### INV-BUDGET-005: Command Attention Profile

**Traces to**: ADRS IB-007
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
Commands are classified by attention cost:
```
CHEAP    (≤ 50 tokens):  guidance, config, log
MODERATE (50–300):        status, query, bilateral, schema, trace, verify, spec, task, next, done, go
EXPENSIVE (300+):         seed, session
META     (side effects):  harvest, transact, merge, init, observe, write
```

**Note**: `next`, `done`, and `go` produce confirmation + guidance footer (~80 tokens).
Classified as MODERATE because the guidance footer (with M(t) sub-metrics and
paste-ready commands) is the primary methodology steering signal — truncating it
defeats the purpose of the guidance system (INV-GUIDANCE-001).

The budget manager adjusts output to stay within the allocated cost.

#### Level 1 (State Invariant)
Each CLI command has a declared attention profile. The output pipeline
respects the profile ceiling, truncating to fit.

**Falsification**: A CHEAP command produces 300+ tokens of output.

---

### INV-BUDGET-006: Token Efficiency as Testable Property

**Traces to**: ADRS IB-011
**Verification**: `V:PROP`, `V:KANI`
**Stage**: 1

#### Level 0 (Algebraic Law)

Information density function `ρ(output) = semantic_units(output) / |output|`.
For a budget constraint `b`, the output `O(cmd, b)` satisfies:

1. `|O(cmd, b)| ≤ b` — hard budget cap (already in INV-BUDGET-001)
2. `ρ(O(cmd, b₁)) ≤ ρ(O(cmd, b₂))` for `b₁ > b₂` — density monotonicity
   (tighter budgets produce denser output, not truncated output)
3. Mode-specific ceilings:
   - `agent_mode ≤ 300 tokens`
   - `guidance_footer ≤ 50 tokens`
   - `error_message ≤ 100 tokens`

#### Level 1 (State Invariant)

The output pipeline evaluates density at each projection level. When budget
forces projection from a higher level (Full) to a lower level (Summary), the
density of the lower projection is verified to be ≥ the density of the higher.
The pipeline never produces output that is merely truncated — it always
re-projects at a lower level.

#### Level 2 (Implementation Contract)

```rust
pub struct TokenEfficiency {
    pub semantic_units: usize,
    pub token_count: usize,
}

impl TokenEfficiency {
    pub fn density(&self) -> f64 {
        self.semantic_units as f64 / self.token_count.max(1) as f64
    }
}

pub const AGENT_MODE_CEILING: usize = 300;
pub const GUIDANCE_FOOTER_CEILING: usize = 50;
pub const ERROR_MESSAGE_CEILING: usize = 100;

// proptest: for any command at budget b1 > b2,
//   density(output(cmd, b2)) >= density(output(cmd, b1))
```

**Falsification**: Reducing the budget by 50% reduces information value by more
than 50% (distortion exceeds rate-distortion bound), OR agent-mode output
exceeds 300 tokens, OR a guidance footer exceeds 50 tokens, OR an error message
exceeds 100 tokens.

---

### INV-BUDGET-007: Action-Centric Projection Completeness

**Traces to**: ADR-BUDGET-005
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
At every budget level B >= MIN_OUTPUT, the projected output contains a complete, actionable recommendation:
forall B >= 50: exists action A in project(store, B) such that A.command is executable and A.rationale is non-empty.

The action is structurally first — it occupies the first line of every projected output.

#### Level 1 (State Invariant)
The ActionProjection type enforces non-optional action fields. The project() algorithm emits the action before processing any context blocks.

**Falsification**: Any budget level >= MIN_OUTPUT produces output without an executable action command, OR the action is not the first line of the projected output.

---

### INV-BUDGET-008: Context Fill Monotonicity

**Traces to**: ADR-BUDGET-005
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
For budgets B1 > B2, context_blocks(project(store, B1)) is a superset of context_blocks(project(store, B2)). Context is additive — increasing budget adds blocks, never removes or reorders them.

#### Level 1 (State Invariant)
The project() algorithm iterates context blocks in fixed precedence order. A block included at budget B2 is always included at budget B1 > B2 because its token cost <= B2 <= B1.

**Falsification**: Increasing budget removes a context block that was present at lower budget, OR changes the relative order of included blocks.

---

### INV-BUDGET-009: Guidance-Projection Unification

**Traces to**: ADR-BUDGET-005, INV-GUIDANCE-001
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
The guidance system R(t) and the output projection function share the same action computation. There is exactly ONE code path (compute_action_from_store) that determines the recommended action.

#### Level 1 (State Invariant)
Both the guidance footer and the ACP action layer call compute_action_from_store(). The output is identical for the same store state.

**Falsification**: The action in a command's ACP output differs from the R(t) recommendation at the same store state, OR the guidance footer is computed by a different code path than the projection action.

---

### §13.5 ADRs

### ADR-BUDGET-001: Measured Context Over Heuristic

**Traces to**: ADRS IB-005
**Stage**: 1

#### Problem
Should attention budget be estimated heuristically or measured from actual consumption?

#### Decision
Measured. Claude Code exposes `context_window.used_percentage` via the statusline hook.
This gives ground truth. The heuristic `k*_eff = k*_base × e^{-0.03n}` becomes fallback
only when measurement is unavailable.

#### Formal Justification
Heuristic is inaccurate because conversation structure varies — a session with many
long tool outputs consumes context faster than one with short exchanges. Measured
consumption eliminates this source of error.

---

### ADR-BUDGET-002: Piecewise Attention Decay

**Traces to**: ADRS IB-005
**Verification**: Used in Q(t) computation
**Stage**: 1

#### Problem
How should attention quality degrade with context consumption?

#### Decision
Piecewise: full quality above 60% remaining, linear degradation 30–60%,
quadratic degradation below 30%.

#### Formal Justification
Empirical observation: LLM attention quality degrades faster than a simple linear
model would predict. The piecewise function captures three regimes: comfortable
(no degradation), pressured (graceful degradation), critical (rapid degradation).
The quadratic regime below 30% reflects the observed cliff in output quality.

---

### ADR-BUDGET-003: Rate-Distortion Framework

**Traces to**: ADRS IB-011
**Stage**: 1

#### Problem
What theoretical framework governs the budget-information tradeoff?

#### Decision
Rate-distortion theory. The interface is a channel with rate constraint (budget).
The system maximizes information value while minimizing distortion (loss of
important facts) at the given rate. The projection pyramid (π₀–π₃) is the
codebook with decreasing rate requirements.

#### Formal Justification
Rate-distortion is the information-theoretic framework for lossy compression
with quality guarantees. It formalizes the intuition that "less budget = less
detail, but the most important things survive." The precedence ordering defines
what "most important" means.

---

### ADR-BUDGET-004: Tokenization via Chars/4 Approximation

**Traces to**: SEED §8, ADRS IMPL-002
**Stage**: 0

#### Problem
The budget system requires token counting to enforce output caps, select
projection levels, and compute Q(t). At Stage 0, what tokenization strategy
should be used? Accurate tokenization requires model-specific vocabulary tables
(e.g., tiktoken cl100k_base for Claude/GPT-4), which add dependencies and
complexity.

#### Options
A) **tiktoken-rs at Stage 0** — use the cl100k_base tokenizer from day one for accurate counts.
B) **HuggingFace tokenizers** — use the HuggingFace tokenizers crate with a Claude-specific model.
C) **bpe crate** — use a generic BPE implementation.
D) **chars/4 approximation** — estimate tokens as character count divided by 4, with content-type correction factors. Behind a `TokenCounter` trait for future swappability.

#### Decision
**Option D.** Use chars/4 with content-type correction factors at Stage 0.
Graduate to tiktoken-rs (cl100k_base) at Stage 1 when token efficiency tracking
needs cross-session comparability. The implementation is behind a `TokenCounter`
trait so the approximation can be swapped for an accurate tokenizer without
changing any calling code.

The budget system operates on coarse bands (200/500/2000 tokens for projection
pyramid levels). A 15-20% approximation error from chars/4 rarely changes band
selection. The error is systematic (consistently overestimates for code,
underestimates for prose) and can be partially corrected with content-type factors.

#### Formal Justification
At Stage 0, zero external dependencies is a design goal — the system should be
self-contained and buildable without network access. tiktoken-rs (Option A) adds
a non-trivial dependency with a model vocabulary file. HuggingFace tokenizers
(Option B) pulls ~40 transitive dependencies and has no Claude-specific model.
bpe (Option C) provides no model-specific encoding. The chars/4 approximation
(Option D) has zero dependencies, is trivially correct to implement, and the
budget system's coarse bands (200/500/2000) provide sufficient margin for a
15-20% error. The `TokenCounter` trait ensures the graduation path to accurate
tokenization is frictionless.

#### Consequences
- Zero dependencies for tokenization at Stage 0
- 15-20% approximation error, which is acceptable for coarse band selection
- `TokenCounter` trait abstracts the strategy: `fn count(&self, text: &str) -> usize`
- Content-type correction: `code_factor = 0.85`, `prose_factor = 1.1`, `mixed_factor = 1.0`
- Stage 1 replaces the implementation behind the trait with tiktoken-rs cl100k_base
- Cross-session token comparisons are not reliable until Stage 1

#### Falsification
The chars/4 approximation error causes incorrect projection level selection in
more than 10% of cases (measured empirically), OR the `TokenCounter` trait is
not used (making the graduation path to accurate tokenization require widespread
code changes), OR Stage 0 adds tiktoken-rs as a dependency.

---

### ADR-BUDGET-005: Action-Centric Over Content-Centric Projection

**Traces to**: SEED.md §8, ADR-BUDGET-003
**Stage**: 1

#### Problem
How should the output pipeline manage budget constraints?

#### Options
A) Content-centric with pyramid summaries — compress content at decreasing detail levels
B) Action-centric — organize output around what the agent should DO, with context scaling

#### Decision
**Option B.** The output pipeline is organized around the recommended action, not the store content. Every output = Action (never truncated) + Context (scales with budget) + Evidence (on-demand).

#### Formal Justification
From prompt-optimization theory: every output is a field configuration over the agent's activation manifold. The action is the minimal configuration that activates the correct reasoning basin. Content-centric compression (Option A) solves the token problem but not the activation problem — the agent still needs to extract the action from compressed content.

---

### ADR-BUDGET-006: Activation Density Over Information Density

**Traces to**: SEED.md §8, ADR-BUDGET-003
**Stage**: 1

#### Problem
What metric should govern output quality under budget constraints?

#### Decision
Activation density: correct-basin-activating tokens per context unit consumed. A 10-token action that activates the task-execution basin has higher activation density than a 50-token summary that activates the observation basin.

---

### ADR-BUDGET-007: Four Activation Strategies for Four k* Regimes

**Traces to**: INV-BUDGET-004, ADR-BUDGET-002
**Stage**: 1

#### Problem
How should output detail vary with remaining attention?

#### Decision
Four k*-adaptive strategies: Demonstrate (k*>=0.7, ~300 tokens context), Navigate (0.4-0.7, ~100 tokens), Imperative (0.2-0.4, ~20 tokens), Signal (k*<0.2, ~5 tokens). Each is a different cognitive activation strategy, not just a compression level.

---

### §13.6 Negative Cases

### NEG-BUDGET-001: No Budget Overflow

**Traces to**: ADRS IB-004
**Verification**: `V:PROP`, `V:KANI`

**Safety property**: `□ ¬(output_size > output_budget ∧ output_budget > MIN_OUTPUT)`

No tool response exceeds the computed output budget (except at the minimum
floor of 50 tokens).

**proptest strategy**: Generate random tool outputs at various budget levels.
Verify truncation to budget ceiling in all cases.

**Kani harness**: Verify `allocate()` output size ≤ budget for all inputs.

---

### NEG-BUDGET-002: No High-Priority Truncation Before Low

**Traces to**: ADRS IB-004
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ b_high, b_low: priority(b_high) > priority(b_low) ∧ truncated(b_high) ∧ ¬truncated(b_low))`

Precedence ordering is inviolable. System and Methodology content is never
truncated while Speculative or Ambient content remains.

**proptest strategy**: Generate output with blocks at all five precedence levels.
Apply budget pressure. Verify truncation order matches precedence.

---

