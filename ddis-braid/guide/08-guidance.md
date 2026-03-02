# §8. GUIDANCE — Build Plan

> **Spec reference**: [spec/12-guidance.md](../spec/12-guidance.md) — read FIRST
> **Stage 0 elements**: INV-GUIDANCE-001–002, 007 (3 INV), ADR-GUIDANCE-002, 004, NEG-GUIDANCE-001
> **Dependencies**: STORE (§1), SCHEMA (§2), QUERY (§3), HARVEST (§5), SEED (§6)
> **Cognitive mode**: Control-theoretic — basin dynamics, anti-drift, feedback loops

---

## §8.1 Module Structure

```
braid-kernel/src/
└── guidance.rs   ← GuidanceFooter, DriftDetector, anti-drift mechanisms, spec-language
```

### Public API Surface

```rust
/// Generate a guidance footer for the current agent state.
pub fn guidance_footer(
    store: &Store,
    drift_signals: &DriftSignals,
) -> GuidanceFooter;

/// Detect drift signals from the agent's recent behavior.
pub fn detect_drift(
    store: &Store,
    agent: AgentId,
    recent_commands: &[CommandRecord],
) -> DriftSignals;

/// Generate full guidance output (standalone guidance command).
pub fn full_guidance(
    store: &Store,
    agent: AgentId,
) -> GuidanceOutput;

pub struct GuidanceFooter {
    pub text: String,       // ≤50 tokens, navigative language
    pub mechanism: AntiDriftMechanism,
    pub priority: DriftPriority,
}

pub struct DriftSignals {
    pub turns_without_ddis: usize,  // Consecutive turns without braid commands
    pub schema_changes_unvalidated: bool,
    pub high_confidence_unharvested: bool,
    pub approaching_budget_threshold: bool,
    pub using_pretrained_patterns: bool,
    pub missing_inv_references: bool,
}

pub enum AntiDriftMechanism {
    ContinuousInjection,     // Footer on every response
    SpecLanguage,            // INV references in guidance
    ProactiveWarning,        // Q(t) harvest prompt
    BudgetAware,             // Budget-triggered guidance
    BasinCompetition,        // Trace to SEED.md
    DriftDetection,          // Explicit drift alert
}

pub enum DriftPriority {
    Critical,   // Budget warning, harvest urgency
    High,       // Drift detected, active correction
    Normal,     // Standard guidance
}

pub struct GuidanceOutput {
    pub recommendation: String,
    pub drift_assessment: String,
    pub relevant_invs: Vec<String>,
    pub next_action: String,
    pub footer: GuidanceFooter,
}
```

---

## §8.2 Three-Box Decomposition

### Guidance System

**Black box** (contract):
- INV-GUIDANCE-001: Continuous injection — every tool response includes a guidance footer.
  NEG-GUIDANCE-001: no tool response without a footer.
- INV-GUIDANCE-002: Spec-language — guidance uses DDIS specification language (INV/ADR/NEG
  references), not generic programming advice. Activates the formal reasoning substrate.
- INV-GUIDANCE-007: Dynamic CLAUDE.md — generated from store state as an optimized prompt.

**State box** (internal design):
- Footer selection algorithm:
  1. Evaluate all drift signals.
  2. Select highest-priority signal.
  3. Generate footer using that signal's anti-drift mechanism.
  4. One footer per response. Priority order:
     budget warning > harvest prompt > drift correction > general guidance.
- Drift detection: query store for agent's recent command history.
  Count consecutive turns without `braid transact/query/harvest/seed`.
  If ≥ 5 → drift signal active.
- Spec-language: footers reference specific INVs.
  "What divergence type does this address?" not "You should check for divergence."

**Clear box** (implementation):
- `guidance_footer`:
  ```rust
  pub fn guidance_footer(store: &Store, signals: &DriftSignals) -> GuidanceFooter {
      if signals.approaching_budget_threshold {
          return budget_warning_footer(store);
      }
      if signals.high_confidence_unharvested {
          return harvest_prompt_footer(store);
      }
      if signals.turns_without_ddis >= 5 {
          return drift_correction_footer(store);
      }
      if signals.missing_inv_references {
          return spec_language_footer(store);
      }
      if signals.using_pretrained_patterns {
          return basin_competition_footer(store);
      }
      default_guidance_footer(store)
  }
  ```
- Each footer generator produces a ≤50 token navigative question:
  - `budget_warning_footer`: "Q(t) = {value}. Harvest soon. What knowledge is at risk?"
  - `harvest_prompt_footer`: "High-confidence candidate detected. Transact now?"
  - `drift_correction_footer`: "What divergence type does this address? (Reconciliation taxonomy)"
  - `spec_language_footer`: "Which invariant's falsification condition does this satisfy?"
  - `basin_competition_footer`: "Trace this decision to a SEED.md section."
  - `default_guidance_footer`: "Next: {highest-priority uncompleted INV for current namespace}."

### Drift Detection

**Black box**: Given an agent's recent command history, detect drift signals.

**Clear box**:
- Query store for agent's recent transactions: `[:find ?cmd ?tx :where [?t :tx/agent agent-id] [?t :cmd/name ?cmd] [?t :tx/time ?tx]]`.
- Count gap since last `braid` command → `turns_without_ddis`.
- Check for schema evolution without subsequent validation → `schema_changes_unvalidated`.
- Check for high-confidence harvest candidates not yet committed → `high_confidence_unharvested`.

---

## §8.3 LLM-Facing Outputs

### Footer Examples

All footers use **navigative** activation language (pointing at knowledge the model has)
rather than **instructive** language (teaching what it already knows):

| Signal | Footer |
|--------|--------|
| General | `↳ Next: implement Store::transact per INV-STORE-001.` |
| No DDIS in 5 turns | `↳ What divergence type does this address? (See: reconciliation taxonomy)` |
| Schema change | `↳ Which INV does this schema evolution preserve? (INV-SCHEMA-004)` |
| Budget low | `↳ Q(t) = 0.18. Harvest now. What knowledge would be lost?` |
| Pretrained patterns | `↳ Trace this decision to a SEED.md section. Which axiom governs?` |

### Agent-Mode Output — `braid guidance`

```
[GUIDANCE] Drift assessment: Basin A (spec-driven), 0 drift signals.
Current namespace: STORE. Progress: 8/13 INVs verified.
Relevant: INV-STORE-009 (frontier durability) — next to implement.
Recommendation: Implement frontier persistence in Store::transact.
---
↳ INV-STORE-009 requires frontier persisted BEFORE response. Which failure mode (FM-001)?
```

---

## §8.4 Verification

### Key Properties

```rust
proptest! {
    // INV-GUIDANCE-001: Every response has a footer
    fn inv_guidance_001(store in arb_store(5), signals in arb_drift_signals()) {
        let footer = guidance_footer(&store, &signals);
        prop_assert!(!footer.text.is_empty());
        prop_assert!(footer.text.len() <= 200);  // ≤50 tokens ≈ ≤200 chars
    }

    // INV-GUIDANCE-002: Spec-language (footer references INV/ADR/NEG)
    fn inv_guidance_002(store in arb_store_with_specs(5), signals in arb_drift_signals()) {
        let footer = guidance_footer(&store, &signals);
        // At least one spec element reference in non-default footers
        if signals.turns_without_ddis >= 5 || signals.missing_inv_references {
            prop_assert!(footer.text.contains("INV-") || footer.text.contains("ADR-")
                        || footer.text.contains("NEG-") || footer.text.contains("SEED"));
        }
    }
}
```

---

## §8.5 Implementation Checklist

- [ ] `GuidanceFooter`, `DriftSignals`, `AntiDriftMechanism` types defined
- [ ] `guidance_footer()` selects highest-priority signal
- [ ] `detect_drift()` computes drift signals from command history
- [ ] `full_guidance()` produces comprehensive guidance output
- [ ] Six anti-drift mechanism footer generators implemented
- [ ] Footer uses navigative language (not instructive)
- [ ] Footer ≤50 tokens
- [ ] Spec-language: footers reference INV/ADR/NEG IDs
- [ ] Integration with STORE: drift detection queries store
- [ ] Integration with INTERFACE: every tool response includes footer (NEG-GUIDANCE-001)

---
