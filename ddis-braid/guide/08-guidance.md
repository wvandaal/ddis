# §8. GUIDANCE — Build Plan

> **Spec reference**: [spec/12-guidance.md](../spec/12-guidance.md) — read FIRST
> **Stage 0 elements**: INV-GUIDANCE-001–002, 007–010 (6 INV), ADR-GUIDANCE-002, 004–005, NEG-GUIDANCE-001
> **Dependencies**: STORE (§1), SCHEMA (§2), QUERY (§3), HARVEST (§5), SEED (§6)
> **Cognitive mode**: Control-theoretic — basin dynamics, anti-drift, feedback loops

---

## §8.1 Module Structure

```
braid-kernel/src/
├── guidance.rs    ← GuidanceFooter, DriftDetector, anti-drift mechanisms, spec-language
├── methodology.rs ← M(t) methodology adherence score, component computation
├── derivation.rs  ← Task derivation rules, template matching, priority computation
└── routing.rs     ← R(t) graph-based work routing, impact scoring, ready-set computation
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
    pub next_action: String,           // ≤50 tokens, navigative language
    pub invariant_refs: Vec<String>,   // e.g., ["INV-STORE-001"] (C5 traceability)
    pub uncommitted_count: u32,        // Harvest urgency signal
    pub drift_warning: Option<String>, // Active drift signal if any
}

pub struct DriftSignals {
    pub turns_without_ddis: usize,  // Consecutive turns without braid commands
    pub schema_changes_unvalidated: bool,
    pub high_confidence_unharvested: bool,
    pub approaching_budget_threshold: bool,
    pub using_pretrained_patterns: bool,
    pub missing_inv_references: bool,
}

pub enum GuidanceMechanism {
    CorrectionInsertion(DriftPattern),       // Insert correction for detected drift
    EffectivenessTracking(u32),              // Track sessions without improvement
    ConstraintBudget(usize),                 // k* limit enforcement
    AmbientPartition(usize),                 // ≤80 tokens ambient section
    DemonstrationDensity(f64),               // Ratio ≥1.0 (demos per constraint cluster)
    DynamicRegeneration,                     // Regenerate CLAUDE.md from store state
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
    pub methodology_score: MethodologyScore,
    pub routing: Option<RoutingDecision>,
}

// --- M(t) Methodology Adherence (INV-GUIDANCE-008) ---

/// Compute methodology adherence score from session state.
pub fn methodology_score(store: &Store, session: &SessionState) -> MethodologyScore;

pub struct MethodologyScore {
    pub total: f64,              // M(t) ∈ [0, 1]
    pub components: [f64; 5],    // [transact_freq, spec_lang, query_div, harvest_q, guidance_c]
    pub weights: [f64; 5],       // loaded from store as `:guidance/m-weight` datoms
    pub trend: Trend,            // Up, Down, Stable (last 5 measurements)
}

pub enum Trend { Up, Down, Stable }

// --- Task Derivation (INV-GUIDANCE-009) ---

/// Derive tasks from a newly transacted artifact.
pub fn derive_tasks(
    store: &Store,
    artifact: EntityId,
    rules: &[DerivationRule],
) -> Vec<Datom>;

/// Load derivation rules from the store.
pub fn load_derivation_rules(store: &Store) -> Vec<DerivationRule>;

pub struct DerivationRule {
    pub entity: EntityId,
    pub artifact_type: String,
    pub task_template: TaskTemplate,
    pub dependency_fn: QueryExpr,
    pub priority_fn: PriorityFn,
}

pub struct TaskTemplate {
    pub task_type: String,
    pub title_pattern: String,
    pub attributes: Vec<(Attribute, ValueTemplate)>,
}

// --- R(t) Work Routing (INV-GUIDANCE-010) ---

/// Select the highest-impact ready task via graph metrics.
pub fn route_work(
    store: &Store,
    weights: &[f64; 6],
) -> Option<RoutingDecision>;

pub struct RoutingDecision {
    pub selected: EntityId,
    pub impact_score: f64,
    pub components: HashMap<String, f64>,
    pub alternatives: Vec<(EntityId, f64)>,
    pub ready_count: usize,
    pub blocked_count: usize,
    pub critical_path_remaining: usize,
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
- INV-GUIDANCE-007: Dynamic CLAUDE.md as optimized prompt — the generator function
  `G: StoreState → CLAUDEmd` is subject to four formal constraints:
  1. **Constraint budget**: total constraints ≤ k*(fresh_session)
  2. **Ambient/active partition**: `G(s) = ambient(s) ⊕ active(s)`, ambient ≤ 80 tokens
  3. **Demonstration density**: ≥ 1 demonstration per constraint cluster
  4. **Effectiveness tracking**: corrections without effect after 5 sessions are replaced

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
- Dynamic CLAUDE.md generation follows a typestate pipeline (INV-GUIDANCE-007 L1):
  `MeasureDrift → DiagnoseDrift → SelectCorrections → ValidateBudget → Emit`.
  The pipeline cannot skip ValidateBudget — budget and demonstration ratio are
  verified before emission.

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

### Dynamic CLAUDE.md Generator (INV-GUIDANCE-007)

**Black box**: Generate CLAUDE.md from store state as an optimized prompt. Subject to
the four formal constraints defined in INV-GUIDANCE-007 L0: constraint budget, ambient/active
partition, demonstration density, effectiveness tracking.

**Clear box**:
- Typestate pipeline ensures no stage is skipped:
  ```rust
  pub struct ClaudeMdConfig {
      pub ambient: AmbientSection,     // |ambient| ≤ 80 tokens, k*-exempt
      pub active: ActiveSection,       // |active| ≤ k*(fresh_session) - |ambient|
  }

  pub struct AmbientSection {
      pub tool_awareness: String,      // Tool names + one-line purposes
      pub identity: String,            // Project identity
  }

  pub struct ActiveSection {
      pub demonstrations: Vec<Demonstration>,
      pub constraints: Vec<DriftCorrection>,
      pub context: SessionContext,
  }
  // Invariant: demonstrations.len() >= constraints.chunks(3).len()
  ```
- **MeasureDrift**: query store for recent drift patterns across sessions.
- **DiagnoseDrift**: classify drift signals (basin competition, spec-language decay, tool avoidance).
- **SelectCorrections**: choose corrections from drift patterns, respecting k* budget.
- **ValidateBudget**: verify `|constraints| ≤ k*`, demonstration ratio ≥ 1.0, ambient ≤ 80 tokens.
- **Emit**: produce the CLAUDE.md content. Ineffective corrections (no improvement after 5 sessions)
  are replaced by new corrections derived from recent drift patterns.

### Drift Detection

**Black box**: Given an agent's recent command history, detect drift signals.

**Clear box**:
- Query store for agent's recent transactions: `[:find ?cmd ?tx :where [?t :tx/agent agent-id] [?t :cmd/name ?cmd] [?t :tx/time ?tx]]`.
- Count gap since last `braid` command → `turns_without_ddis`.
- Check for schema evolution without subsequent validation → `schema_changes_unvalidated`.
- Check for high-confidence harvest candidates not yet committed → `high_confidence_unharvested`.

### M(t) Methodology Adherence (INV-GUIDANCE-008)

**Black box** (contract):
- INV-GUIDANCE-008: M(t) is a continuous function `M: SessionState → [0, 1]` decomposed
  into five independently measurable components with data-driven weights stored as datoms.
- M(t) is computed at every INJECT transition and appended to the guidance footer.
- `M(t) < 0.5` triggers a drift signal. Trend (↑/↓/→) computed from last 5 measurements.

**State box** (internal design):
- Five component computations, each querying the store:
  1. `transact_frequency`: `braid transact` calls / total commands in recent window
  2. `spec_language_ratio`: messages containing `INV-`/`ADR-`/`NEG-` / total messages
  3. `query_diversity`: distinct strata used / available strata
  4. `harvest_quality`: new datoms from harvest / estimated epistemic gap size
  5. `guidance_compliance`: followed suggestions / total suggestions
- Weights loaded from `:guidance/m-weight` datoms. Default: `(0.25, 0.20, 0.15, 0.25, 0.15)`.
- Trend: compare current M(t) to rolling average of last 5. >5% up = Up, >5% down = Down.
- Each measurement recorded as a datom for cross-session trend analysis.

**Clear box** (implementation):
```rust
impl MethodologyScore {
    pub fn compute(store: &Store, session: &SessionState) -> Self {
        // Load weights from store (`:guidance/m-weight` datoms)
        let weights = load_weights(store);

        // Compute each component
        let components = [
            transact_frequency(store, session),    // m₁
            spec_language_ratio(store, session),    // m₂
            query_diversity(store, session),        // m₃
            harvest_quality(store, session),        // m₄
            guidance_compliance(store, session),    // m₅
        ];

        let total = weights.iter().zip(&components)
            .map(|(w, m)| w * m)
            .sum();

        // Trend from last 5 M(t) datoms
        let trend = compute_trend(store);

        Self { total, components, weights, trend }
    }
}

fn transact_frequency(store: &Store, session: &SessionState) -> f64 {
    // [:find (count ?tx) :where [?tx :tx/provenance _] [?tx :tx/agent ?a]]
    // divided by total commands in session window
}

fn spec_language_ratio(store: &Store, session: &SessionState) -> f64 {
    // Count messages referencing INV-/ADR-/NEG- in recent window
}
```

### Task Derivation (INV-GUIDANCE-009)

**Black box** (contract):
- INV-GUIDANCE-009: Task derivation is a total function from specification artifacts to
  implementation tasks. Rules are datoms — the system can derive tasks to modify its own rules.
- 10 default derivation rules loaded in genesis:

| # | Artifact Type | Trigger | Derived Tasks | Example |
|---|--------------|---------|---------------|---------|
| 1 | Invariant | INV transacted | impl task + verification task | `INV-STORE-001` → "Implement INV-STORE-001" + "Test INV-STORE-001" |
| 2 | ADR | ADR transacted | impl task (apply decision) | `ADR-STORE-009` → "Apply ADR-STORE-009: redb persistence" |
| 3 | Negative case | NEG transacted | negative test task | `NEG-MERGE-001` → "Write negative test for NEG-MERGE-001" |
| 4 | Schema attr | Attribute registered | migration task | `:task/status` → "Register attribute :task/status" |
| 5 | Module boundary | Module ref created | integration test task | STORE↔SCHEMA → "Integration test: store+schema" |
| 6 | Entity type | Type definition transacted | CRUD impl task | `:spec/type "adr"` → "Implement ADR entity CRUD" |
| 7 | CLI command | Command spec transacted | handler + test task | `braid transact` → "Implement transact handler" |
| 8 | MCP tool | Tool spec transacted | MCP handler + description task | `braid_query` → "Implement MCP query handler" |
| 9 | Query pattern | Query pattern transacted | query test task | EAVT scan → "Test EAVT index scan" |
| 10 | Guidance node | Guidance rule transacted | evaluation task | Rule 1 → "Evaluate derivation rule 1 effectiveness" |

- Every derived task carries `:task/derived-from` and `:task/derived-by` traceability.

**State box** (internal design):
- Pipeline: `ArtifactTransacted → MatchRules → DeriveTask → ComputePriority → StoreTasks`
- Rule matching: query store for rules where `:rule/artifact-type` matches artifact type.
- Template expansion: replace `{artifact_id}`, `{artifact_type}`, etc. in title pattern.
- Priority computation: evaluate `:rule/priority-fn` using graph metrics (PageRank, betweenness).
- Self-bootstrap: rules themselves match "derivation_rule" artifact type, so a meta-rule
  can derive tasks to evaluate or improve other rules.

**Clear box** (implementation):
```rust
pub fn derive_tasks(
    store: &Store,
    artifact: EntityId,
    rules: &[DerivationRule],
) -> Vec<Datom> {
    let artifact_type = store.attribute_value(artifact, ":spec/type");
    let mut tasks = Vec::new();

    for rule in rules {
        if rule.artifact_type == artifact_type {
            // Expand template
            let task_datoms = rule.task_template.expand(artifact, store);
            // Compute dependencies from artifact's references
            let deps = evaluate_query(store, &rule.dependency_fn, artifact);
            // Compute priority from graph metrics
            let priority = rule.priority_fn.evaluate(store, artifact);
            // Add traceability datoms
            tasks.extend(task_datoms);
            tasks.push(datom(task_eid, ":task/derived-from", artifact));
            tasks.push(datom(task_eid, ":task/derived-by", rule.entity));
            tasks.push(datom(task_eid, ":task/priority", priority));
        }
    }
    tasks
}

pub fn load_derivation_rules(store: &Store) -> Vec<DerivationRule> {
    // [:find ?r :where [?r :rule/artifact-type _]]
    // Construct DerivationRule from datom attributes
}
```

### R(t) Graph-Based Work Routing (INV-GUIDANCE-010)

**Black box** (contract):
- INV-GUIDANCE-010: R(t) selects the highest-impact ready task via a weighted combination
  of six graph metrics: PageRank, betweenness, critical path position, blocker ratio,
  staleness, and priority boost.
- Routing decisions are recorded as datoms for traceability.
- Weights are datoms (`:guidance/r-weight`), enabling data-driven evolution.

**State box** (internal design):
- Ready set computation: topological sort (INV-QUERY-012) → filter to tasks with
  all dependencies completed → the ready set.
- For each ready task, compute six metrics:
  1. PageRank (INV-QUERY-014): normalized authority in dependency graph
  2. Betweenness (INV-QUERY-015, Stage 1; proxy via in/out-degree ratio at Stage 0)
  3. Critical path position (INV-QUERY-017): 1.0 if on critical path, 0.0 otherwise
  4. Blocker ratio: tasks blocked by this / total blocked tasks
  5. Staleness: time since task became ready, normalized
  6. Priority boost: human-assigned priority, normalized
- Combine with weights → select argmax → record decision.

**Clear box** (implementation):
```rust
pub fn route_work(
    store: &Store,
    weights: &[f64; 6],
) -> Option<RoutingDecision> {
    // 1. Compute ready set
    let topo = topo_sort(store, &task_type_attr, &dep_attr).ok()?;
    let ready: Vec<_> = topo.iter()
        .filter(|(eid, _)| all_deps_completed(store, *eid))
        .collect();

    if ready.is_empty() { return None; }

    // 2. Compute graph metrics for all ready tasks
    let pr = pagerank(store, &task_type_attr, &dep_attr, &PageRankConfig::default());
    let cp = critical_path(store, &task_type_attr, &dep_attr, None).ok();

    // 3. Score each ready task
    let mut scored: Vec<_> = ready.iter().map(|(eid, _)| {
        let components = [
            lookup_pagerank(&pr, eid),
            proxy_betweenness(store, eid),       // Stage 0: in/out degree ratio
            critical_path_membership(&cp, eid),
            blocker_ratio(store, eid),
            staleness(store, eid),
            priority_boost(store, eid),
        ];
        let score: f64 = weights.iter().zip(&components)
            .map(|(w, g)| w * g).sum();
        (*eid, score, components)
    }).collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // 4. Build decision
    let (selected, impact_score, comps) = scored[0];
    Some(RoutingDecision {
        selected,
        impact_score,
        components: metric_names().zip(comps).collect(),
        alternatives: scored[1..4.min(scored.len())]
            .iter().map(|(e, s, _)| (*e, *s)).collect(),
        ready_count: ready.len(),
        blocked_count: count_blocked(store),
        critical_path_remaining: cp.map(|c| c.path.len()).unwrap_or(0),
    })
}
```

---

## §8.3 LLM-Facing Outputs

### Footer Examples

All footers use **navigative** activation language (pointing at knowledge the model has)
rather than **instructive** language (teaching what it already knows):

| Signal | Footer |
|--------|--------|
| General | `↳ Next: implement Store::transact per INV-STORE-001.` |
| No DDIS in 5 turns | `↳ What divergence type does this address? (See: reconciliation taxonomy)` |
| Schema change | `↳ Which INV does this schema evolution preserve? (INV-SCHEMA-003)` |
| Budget low | `↳ Q(t) = 0.18. Harvest now. What knowledge would be lost?` |
| Pretrained patterns | `↳ Trace this decision to a SEED.md section. Which axiom governs?` |

### Agent-Mode Output — `braid guidance`

```
[GUIDANCE] M(t) = 0.82 ↑  [transact: 0.90, spec-lang: 0.85, query: 0.60, harvest: 0.90, guidance: 0.85]
Drift: Basin A (spec-driven), 0 signals. Namespace: STORE (8/13 INVs verified).
R(t): Next → INV-STORE-004 (impact: 0.87 — PR: 0.92, critical: yes, blockers: 4)
  Ready: 5 tasks | Blocked: 12 | Critical path: 8 remaining
---
↳ INV-STORE-004 is on the critical path and unblocks 4 tasks. Which CRDT law does it establish?
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

    // INV-GUIDANCE-007: Dynamic CLAUDE.md constraint budget and structure
    fn inv_guidance_007(store in arb_store_with_specs(5)) {
        let config = generate_claude_md(&store);
        // Ambient section ≤ 80 tokens
        prop_assert!(config.ambient.token_count() <= 80);
        // Active section within k* budget
        prop_assert!(config.active.constraint_count() <= k_star_fresh_session());
        // Demonstration density: ≥ 1 per constraint cluster
        let clusters = config.active.constraints.chunks(3).count().max(1);
        prop_assert!(config.active.demonstrations.len() >= clusters);
    }

    // INV-GUIDANCE-008: M(t) range and decomposability
    fn inv_guidance_008(store in arb_store_with_commands(10), session in arb_session_state()) {
        let score = methodology_score(&store, &session);
        // M(t) ∈ [0, 1]
        prop_assert!(score.total >= 0.0 && score.total <= 1.0);
        // All components ∈ [0, 1]
        for c in &score.components {
            prop_assert!(*c >= 0.0 && *c <= 1.0);
        }
        // Weights sum to 1.0
        let w_sum: f64 = score.weights.iter().sum();
        prop_assert!((w_sum - 1.0).abs() < 1e-10);
        // M(t) = weighted sum of components
        let expected: f64 = score.weights.iter().zip(&score.components)
            .map(|(w, m)| w * m).sum();
        prop_assert!((score.total - expected).abs() < 1e-10);
    }

    // INV-GUIDANCE-009: Task derivation produces tasks with traceability
    fn inv_guidance_009(store in arb_store_with_artifacts(5)) {
        let rules = load_derivation_rules(&store);
        let artifacts: Vec<_> = store.entities_of_type("invariant").collect();
        for artifact in artifacts {
            let tasks = derive_tasks(&store, artifact, &rules);
            // At least one task per artifact (if matching rule exists)
            if rules.iter().any(|r| r.artifact_type == "invariant") {
                prop_assert!(!tasks.is_empty());
            }
            // Every task has traceability
            for task_datom in &tasks {
                if task_datom.attribute.name() == "derived-from" {
                    prop_assert_eq!(task_datom.value, Value::Ref(artifact));
                }
            }
        }
    }

    // INV-GUIDANCE-010: R(t) selects only from ready set
    fn inv_guidance_010(store in arb_store_with_tasks(10)) {
        let weights = [0.25, 0.25, 0.20, 0.15, 0.10, 0.05];
        if let Some(decision) = route_work(&store, &weights) {
            // Selected task must be in ready set (all deps completed)
            prop_assert!(all_deps_completed(&store, decision.selected));
            // Impact score ≥ all alternative scores
            for (_, alt_score) in &decision.alternatives {
                prop_assert!(decision.impact_score >= *alt_score);
            }
            // Determinism: same store → same decision
            let decision2 = route_work(&store, &weights).unwrap();
            prop_assert_eq!(decision.selected, decision2.selected);
        }
    }
}
```

---

## §8.5 Implementation Checklist

### Core Guidance
- [ ] `GuidanceFooter`, `DriftSignals`, `AntiDriftMechanism` types defined
- [ ] `guidance_footer()` selects highest-priority signal
- [ ] `detect_drift()` computes drift signals from command history
- [ ] `full_guidance()` produces comprehensive guidance output
- [ ] Six anti-drift mechanism footer generators implemented
- [ ] Footer uses navigative language (not instructive)
- [ ] Footer ≤50 tokens
- [ ] Spec-language: footers reference INV/ADR/NEG IDs

### Dynamic CLAUDE.md (INV-GUIDANCE-007)
- [ ] `ClaudeMdConfig` with `AmbientSection` / `ActiveSection`
- [ ] Five-stage typestate pipeline (MeasureDrift→DiagnoseDrift→SelectCorrections→ValidateBudget→Emit)
- [ ] Ambient section ≤ 80 tokens, demonstration density ≥ 1 per cluster
- [ ] Ineffective correction replacement after 5 sessions

### M(t) Methodology Adherence (INV-GUIDANCE-008)
- [ ] `MethodologyScore` struct with total, components, weights, trend
- [ ] Five component computations: transact_freq, spec_lang, query_div, harvest_q, guidance_c
- [ ] Weights loaded from store as `:guidance/m-weight` datoms
- [ ] Default weights `(0.25, 0.20, 0.15, 0.25, 0.15)` in genesis
- [ ] Trend computation from last 5 M(t) datoms
- [ ] M(t) < 0.5 triggers drift signal
- [ ] M(t) recorded as datom at each measurement
- [ ] M(t) included in guidance footer

### Task Derivation (INV-GUIDANCE-009)
- [ ] `DerivationRule` struct with entity, artifact_type, task_template, dependency_fn, priority_fn
- [ ] `TaskTemplate` with type, title_pattern, attributes
- [ ] `load_derivation_rules()` from store
- [ ] `derive_tasks()` matching, expanding, prioritizing
- [ ] 10 default derivation rules in genesis store
- [ ] Traceability: `:task/derived-from` and `:task/derived-by` on every derived task
- [ ] Self-bootstrap: rules can derive tasks to modify rules
- [ ] Priority computation via graph metrics (PageRank, betweenness)

### R(t) Work Routing (INV-GUIDANCE-010)
- [ ] `RoutingDecision` struct with selected, impact_score, components, alternatives
- [ ] Ready set computation via topological sort (INV-QUERY-012)
- [ ] Six metric computations: PageRank, betweenness proxy, critical path, blocker ratio, staleness, priority
- [ ] Weights loaded from store as `:guidance/r-weight` datoms
- [ ] Default weights `(0.25, 0.25, 0.20, 0.15, 0.10, 0.05)` in genesis
- [ ] Routing decisions recorded as datoms
- [ ] R(t) included in guidance footer
- [ ] Determinism: same frontier → same routing decision

### Integration
- [ ] Integration with STORE: drift detection queries store
- [ ] Integration with QUERY: R(t) consumes graph engine results (topo_sort, pagerank, critical_path)
- [ ] Integration with INTERFACE: every tool response includes footer (NEG-GUIDANCE-001)
- [ ] Integration with INTERFACE: `braid guidance` returns M(t) + R(t) in output

---
