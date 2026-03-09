# 02 — Persistent Coherence Diagnostics

> **Status**: EXPLORATION
> **Date**: 2026-03-09
> **Traces to**: SEED.md §6 (reconciliation), §7 (self-improvement loop), §8 (interface principles)
> **Builds on**: 00-sheaf-cohomology-for-coherence.md (H¹ computation),
>   01-hodge-theory-and-spectral-connection.md (spectral connection),
>   spec/09-signal.md (signal system), spec/12-guidance.md (guidance injection)
>
> **Thesis**: Persistent cohomology tracks the birth and death of incoherence cycles over
> transaction history. The persistence diagram is a topological signature of project health —
> a "coherence EKG" that distinguishes routine work-in-progress from structural design problems.

---

## 1. The Filtration: Store Growth as Topological Evolution

### 1.1 The Transaction Filtration

The append-only store (C1) induces a natural filtration:

```
∅ = S₀ ⊂ S₁ ⊂ S₂ ⊂ ... ⊂ Sₜ
```

where Sᵢ is the store after transaction i. Each inclusion Sᵢ ↪ Sᵢ₊₁ adds datoms
(never removes — append-only). This is a **monotone filtration** — exactly the input
format for persistent homology.

At each step, we can compute the coherence complex and its cohomology:

```
H¹(S₀) → H¹(S₁) → H¹(S₂) → ... → H¹(Sₜ)
```

The maps between successive cohomology groups (induced by inclusion) track how
H¹ generators evolve: new generators appear (births), existing generators merge
or disappear (deaths).

### 1.2 The Persistence Module

The sequence of cohomology groups forms a **persistence module**:

```
M = {H¹(Sᵢ), φᵢ: H¹(Sᵢ) → H¹(Sᵢ₊₁)}
```

where φᵢ is the map induced by the inclusion Sᵢ ↪ Sᵢ₊₁.

By the structure theorem for persistence modules (over a PID), M decomposes into
**interval modules**:

```
M ≅ ⊕ⱼ k[bⱼ, dⱼ)
```

where each interval [bⱼ, dⱼ) represents an incoherence cycle that was born at
transaction bⱼ and died at transaction dⱼ. The multiset of intervals is the
**barcode** (equivalently, the **persistence diagram**).

### 1.3 Birth and Death Semantics for DDIS

| Event | Meaning | Example |
|-------|---------|---------|
| **Birth** of H¹ generator at tx_b | A new cyclic incoherence was introduced | Agent implements from intent, bypassing spec |
| **Death** of H¹ generator at tx_d | The cyclic incoherence was resolved | Spec updated to match implementation's intent interpretation |
| **Persistence** = d - b | How long the incoherence survived | Short: routine work. Long: structural problem |
| **Birth at tx_0** | Incoherence present from the beginning | Foundational design contradiction |
| **Still alive** (no death) | Incoherence not yet resolved | Ongoing structural issue requiring attention |

---

## 2. The Persistence Diagram as Project Health Metric

### 2.1 Reading the Diagram

```
death
  │
  │  ·  ·                         ← short-lived: work in progress (normal)
  │    · ·  ·
  │         ·
  │              ·                ← medium-lived: took a few sessions to resolve
  │
  │                         ·    ← LONG-LIVED: structural problem
  │                              ·
  │                                   · (still alive — on the diagonal at ∞)
  └──────────────────────────────── birth
```

**Healthy project signatures**:
- Most points near the diagonal (short persistence)
- Few or no points far from the diagonal
- No "still alive" points with old birth times

**Unhealthy project signatures**:
- Points far from the diagonal (persistent structural problems)
- Cluster of births at a specific time (a bad commit or design decision)
- Increasing birth rate without corresponding deaths (accumulating debt)

### 2.2 Derived Metrics

From the persistence diagram PD = {(bⱼ, dⱼ)}, derive:

```
Total persistence:      P_total = Σⱼ (dⱼ - bⱼ)
                        — total "incoherence-time" experienced

Maximum persistence:    P_max = maxⱼ (dⱼ - bⱼ)
                        — age of the longest-lived incoherence cycle

Active generators:      N_active(t) = |{j : bⱼ ≤ t < dⱼ}|
                        — number of unresolved cycles at time t

Birth rate:             R_birth(t) = |{j : bⱼ ∈ [t-w, t]}| / w
                        — rate of new incoherence introduction

Death rate:             R_death(t) = |{j : dⱼ ∈ [t-w, t]}| / w
                        — rate of incoherence resolution

Net accumulation:       R_net(t) = R_birth(t) - R_death(t)
                        — positive = incoherence accumulating
                        — negative = incoherence resolving
                        — zero = steady state
```

### 2.3 Stability Theorem Application

The stability theorem for persistence diagrams states:

```
d_B(PD(f), PD(g)) ≤ ||f - g||_∞
```

where d_B is the bottleneck distance and ||·||_∞ is the supremum norm on filtration
functions.

For DDIS, this means: **small changes to the store produce small changes to the
persistence diagram**. A single transaction cannot drastically change the topological
signature. Structural problems appear gradually and can be detected early.

Contrapositive: if the persistence diagram changes dramatically after a small commit,
that commit touched a topological nerve — it resolved (or created) a structural
incoherence cycle. This is **automatic detection of architecturally significant changes**.

---

## 3. Integration with the Signal System

### 3.1 New Signal Types

The signal system (spec/09-signal.md) currently defines signal types for operational
events. Persistent cohomology adds three new signal types:

```
SIGNAL_H1_BIRTH: CycleGenerator
  — emitted when a new H¹ generator appears
  — carries: cycle description, involved entities, involved agents
  — urgency: proportional to number of entities in cycle

SIGNAL_H1_DEATH: CycleGenerator
  — emitted when an H¹ generator is resolved
  — carries: resolution method, resolving transaction, persistence

SIGNAL_H1_CHRONIC: CycleGenerator
  — emitted when an H¹ generator exceeds the chronic threshold
  — carries: current age, involved entities, recommended resolution
  — urgency: HIGH (chronic incoherence requires deliberation)
```

### 3.2 Chronic Threshold Calibration

The threshold for SIGNAL_H1_CHRONIC is a function of the project's persistence
statistics:

```
chronic_threshold = median_persistence × k

where:
  median_persistence = median(dⱼ - bⱼ) over all resolved generators
  k = configurable multiplier (default: 3.0)
```

A generator that persists for 3× the median lifetime is "chronic" — it's not routine
work in progress but a structural problem that should trigger deliberation.

The threshold is stored as a datom (schema-as-data, C3), so it adapts to the project's
natural rhythm:
```
(config:cohomology, :config/chronic-threshold-multiplier, 3.0, tx:config, assert)
```

### 3.3 Signal-Driven Guidance

When SIGNAL_H1_BIRTH fires, the guidance system (INV-GUIDANCE-001) adjusts its
injection:

```
If active H¹ generators exist:
  guidance_header = "⚠ STRUCTURAL INCOHERENCE (β₁ = {count})"
  For each generator:
    guidance_item = "Cycle [{id}]: {entity_list} — {recommendation}"

  If any generator age > chronic_threshold:
    guidance_priority = URGENT
    guidance_action = "Resolve before adding new content. See `braid coherence --cycles`"
```

---

## 4. Integration with the Trilateral Model

### 4.1 Φ + β₁: The Complete Coherence Characterization

The trilateral coherence model (spec/18-trilateral.md) defines Φ as the divergence metric.
Adding β₁ gives the complete picture:

```
Coherence state: (Φ, β₁)

Target: Φ = 0, β₁ = 0

Interpretation:
  Φ > 0, β₁ = 0: "You have gaps. Fix them one at a time. Any order works."
  Φ = 0, β₁ > 0: "All links exist but are inconsistent. Coordinated fix needed."
  Φ > 0, β₁ > 0: "Gaps AND structural problems. Fix cycles first, then gaps."
  Φ = 0, β₁ = 0: "Fully coherent."
```

### 4.2 Extended Fitness Function

The bilateral fitness function F(S) from INV-BILATERAL-001 currently measures spec↔impl
convergence. The extended function:

```
F_extended(S) = w_Φ × (1 - Φ/Φ_max) + w_β × (1 - β₁/β₁_max) + w_P × (1 - P_max/P_limit)

where:
  Φ_max = maximum observed Φ (normalization)
  β₁_max = maximum observed β₁
  P_max = current maximum persistence
  P_limit = chronic threshold
  w_Φ + w_β + w_P = 1
```

This gives a scalar fitness in [0, 1] that accounts for both magnitude (Φ), structure
(β₁), and temporal evolution (P_max) of incoherence.

### 4.3 Convergence Monotonicity Extended

INV-TRILATERAL-004 states that adding convergence links never increases Φ. The
cohomological extension:

**Claim**: Adding a `:spec/traces-to` or `:spec/implements` link that resolves an H¹
generator strictly decreases β₁.

**Proof sketch**: An H¹ generator is a cycle of disagreements. Adding a link that makes
the cycle boundaries agree removes the cocycle from H¹. Since we're working over F₂,
removing a generator strictly decreases rank.

**Caveat**: Adding a link can also CREATE a new H¹ generator if it completes a previously
open cycle into a closed one with disagreements. So β₁ is NOT monotonically non-decreasing
under link additions — it can increase or decrease. This is a key difference from Φ (which
monotonically decreases under link additions).

**Implication**: The guidance system should compute H¹ BEFORE and AFTER a proposed link
addition, warning if the link would create a new cycle.

---

## 5. Visualization and CLI Interface

### 5.1 CLI Commands

```bash
# Basic coherence check (Φ + β₁)
braid coherence
# Output: Φ = 47, β₀ = 1, β₁ = 3

# Detailed cycle information
braid coherence --cycles
# Output: Lists each H¹ generator with entities, agents, and age

# Persistence diagram (ASCII art)
braid coherence --persistence
# Output: ASCII persistence diagram with birth/death pairs

# Persistence statistics
braid coherence --persistence --stats
# Output: P_total, P_max, N_active, R_birth, R_death, R_net

# JSON output for programmatic use
braid coherence --json
# Output: Full cohomology result as JSON

# ISP triangle check (single-agent mode)
braid coherence --isp
# Output: Lists specification bypass conflicts (if any)
```

### 5.2 Dashboard Integration

For continuous monitoring (Stage 2+):

```
braid watch --coherence
```

Streams coherence updates to stdout as transactions arrive:

```jsonl
{"tx": "abc123", "phi": 47, "beta_0": 1, "beta_1": 3, "births": [], "deaths": []}
{"tx": "def456", "phi": 46, "beta_0": 1, "beta_1": 3, "births": [], "deaths": []}
{"tx": "ghi789", "phi": 46, "beta_0": 1, "beta_1": 4, "births": [{"id": "gen-4", "entities": [...]}], "deaths": []}
{"tx": "jkl012", "phi": 44, "beta_0": 1, "beta_1": 3, "births": [], "deaths": [{"id": "gen-2", "persistence": 47}]}
```

---

## 6. The "Coherence EKG" — Temporal Signature

### 6.1 The β₁(t) Curve

Plot β₁ over transaction history:

```
β₁
 4│              ╭──╮
 3│    ╭─────╮  ╭╯  ╰╮    ╭╮
 2│╭──╯     ╰──╯    ╰╮  ╭╯╰─╮
 1│╯                   ╰──╯   ╰──
 0│
  └───────────────────────────────→ tx
    ↑       ↑    ↑    ↑       ↑
    bootstrap  sprint 1  sprint 2
```

**Pattern recognition**:
- **Sawtooth**: β₁ rises during implementation, drops during review/resolution. Healthy.
- **Monotonic rise**: β₁ only increases. Technical debt accumulating. Unhealthy.
- **Flat at zero**: No cyclic incoherence. Either very disciplined or not enough cross-linking.
- **Spike**: Sudden β₁ increase = bad merge or contradictory commit. Investigate immediately.

### 6.2 The Birth-Death Scatter Plot

```
                death tx
                  │
                  │  · ·                    ← resolved quickly
                  │    ·  ·                 ← resolved in same sprint
                  │         ·
                  │              ·          ← resolved in next sprint
                  │
                  │                    ·    ← persisted across 3+ sprints (STRUCTURAL)
                  │
  alive line ─ ─ ─│─ ─ ─ ─ ─ ─ ─ ─ ─ ─ × ← still alive (needs attention NOW)
                  │                     ×
                  └────────────────────────→ birth tx
```

Points above the "alive line" (death = current tx) are resolved. Points ON the alive
line are current structural problems. Their horizontal position shows when they were
introduced — old alive points are the highest priority.

---

## 7. Theoretical Limits and Practical Considerations

### 7.1 When H¹ Doesn't Help

H¹ detects *cyclic* incoherence. It does NOT detect:
- **Isolated gaps** (missing links with no cycle) — Φ catches these
- **Semantic contradictions within a single document** — the 5-tier contradiction engine
  catches these
- **Quality issues** (a spec element that's technically linked but poorly written) —
  fitness functions and LLM evaluation catch these
- **Temporal ordering issues** (two agents assert contradictory facts at different times
  but LWW resolves them) — the LWW resolution handles these transparently

### 7.2 False Positives

H¹ can report incoherence cycles that are semantically harmless:
- Two paths from I to P that produce "different" implementations that are actually
  equivalent (e.g., different variable names, equivalent algorithms)
- Resolution mode differences that look like disagreements but converge after merge

**Mitigation**: Use the weighted Hodge theory (doc 01 §2) to weight disagreements
by semantic significance. Low-weight H¹ generators (disagreements only on unimportant
attributes) are filtered from the diagnostic output.

### 7.3 Computational Overhead Budget

For Stage 0 (single agent, ~100 spec elements):
- ISP triangle check: O(|spec_elements|) — milliseconds
- No multi-agent cohomology needed (single agent → tree topology → H¹ = 0)

For Stage 2-3 (multi-agent, ~1000 elements):
- Full cohomology: O(n²m) where n ≤ 10, m ≤ 45 — microseconds
- Persistence update: O(1) amortized per transaction
- Dashboard streaming: negligible overhead (async, non-blocking)

**Conclusion**: The computational cost is negligible at all stages. The main cost is
conceptual — the team must understand what H¹ means to act on its diagnostics. The
CLI output should always include plain-language explanations alongside the mathematical
quantities.

---

*Persistent cohomology gives DDIS something no other specification framework has: a
topological signature of project health that distinguishes routine work from structural
problems, that tracks the birth and resolution of design contradictions over time, and
that provides formal justification for when pairwise fixes are sufficient vs. when
coordinated multi-stakeholder resolution is required. The mathematics is elementary
(linear algebra over finite fields), the computation is fast (microseconds), and the
insight is profound (the shape of incoherence matters more than its magnitude).*
