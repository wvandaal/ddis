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
// --- Guidance Topology (spec §12.3, ADR-GUIDANCE-001: comonadic topology) ---
// Note: ADR-GUIDANCE-001 topology architecture is Stage 1 formal, but basic data
// structures are included proactively at Stage 0 per the build-forward principle.

pub struct GuidanceTopology {
    pub nodes: HashMap<EntityId, GuidanceNode>,
    pub edges: Vec<(EntityId, EntityId)>,
}

pub struct GuidanceNode {
    pub entity: EntityId,
    pub predicate: QueryExpr,      // Datalog predicate over store state
    pub actions: Vec<GuidanceAction>,
    pub learned: bool,
    pub effectiveness: f64,
}

pub struct GuidanceAction {
    pub command: String,               // specific braid command
    pub invariant_refs: Vec<String>,   // e.g., "INV-STORE-001"
    pub postconditions: Vec<EntityId>,
    pub score: f64,
}

pub struct GuidanceFooter {
    pub next_action: String,           // ≤50 tokens, navigative language
    pub invariant_refs: Vec<String>,   // e.g., ["INV-STORE-001"] (C5 traceability)
    pub uncommitted_count: u32,        // Harvest urgency signal
    pub drift_warning: Option<String>, // Active drift signal if any
    pub methodology_score: f64,        // M(t) ∈ [0,1], computed per INV-GUIDANCE-008
    pub turn_count: usize,             // Current turn number (budget awareness)
    pub datom_count: usize,            // Current store size (budget awareness)
}

// --- Public free functions (ADR-ARCHITECTURE-001) ---

/// Query guidance topology for current state with lookahead (spec §12.2 QUERY_GUIDANCE).
pub fn query_guidance(
    topology: &GuidanceTopology,
    store: &Store,
    agent: &AgentId,
    lookahead: u8,
) -> GuidanceResult;

/// Generate a guidance footer for a tool response (spec §12.2 INJECT).
/// Spec §12.3 L2 defines this as `GuidanceTopology::footer(&self, store, k_eff)`.
/// Guide uses free-function form per ADR-ARCHITECTURE-001; semantically equivalent.
pub fn guidance_footer(
    topology: &GuidanceTopology,
    store: &Store,
    k_eff: f64,
) -> GuidanceFooter;

/// Detect drift signals from the agent's recent behavior (spec §12.2 DETECT_DRIFT).
pub fn detect_drift(
    store: &Store,
    agent: AgentId,
    recent_commands: &[CommandRecord],
) -> DriftSignals;

/// Generate full guidance output (standalone `braid guidance` command).
pub fn full_guidance(
    store: &Store,
    agent: AgentId,
) -> GuidanceOutput;

// --- Drift Detection (internal, feeds into topology evaluation) ---

pub struct DriftSignals {
    pub turns_without_ddis: usize,  // Consecutive turns without braid commands
    pub schema_changes_unvalidated: bool,
    pub high_confidence_unharvested: bool,
    pub approaching_budget_threshold: bool,
    pub using_pretrained_patterns: bool,
    pub missing_inv_references: bool,
    pub drift_score: f64,           // Aggregate drift score (spec §12.2)
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

/// Priority function for task derivation — computes priority score from graph
/// metrics (PageRank, betweenness, etc.) for a given artifact in the store.
/// See types.md phantom types appendix for the full type hierarchy.
pub type PriorityFn = Box<dyn Fn(&Store, EntityId) -> f64>;

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
- `guidance_footer` (via topology, per ADR-GUIDANCE-001):
  ```rust
  pub fn guidance_footer(
      topology: &GuidanceTopology,
      store: &Store,
      k_eff: f64,
  ) -> GuidanceFooter {
      // Evaluate topology nodes: each node's Datalog predicate is tested
      // against current store state. Matching nodes produce scored actions.
      let mut scored_actions: Vec<(GuidanceAction, f64)> = Vec::new();
      for (_, node) in &topology.nodes {
          if evaluate_predicate(&node.predicate, store) {
              for action in &node.actions {
                  scored_actions.push((action.clone(), action.score));
              }
          }
      }
      // Select highest-scoring action (priority:
      //   budget warning > harvest prompt > drift correction > general guidance)
      scored_actions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
      let best = scored_actions.first();
      GuidanceFooter {
          next_action: best.map(|(a, _)| a.command.clone())
              .unwrap_or_else(|| default_next_action(store)),
          invariant_refs: best.map(|(a, _)| a.invariant_refs.clone())
              .unwrap_or_default(),
          uncommitted_count: count_uncommitted(store),
          drift_warning: detect_active_warning(store),
          methodology_score: compute_methodology_score(store),
          turn_count: current_turn_count(store),
          datom_count: store.len(),
      }
  }
  ```
- Token counting for footer budget uses `&dyn TokenCounter` (see guide/00-architecture.md
  section 0.6, from D5-tokenizer-survey.md). At Stage 0: chars/4 approximation. At Stage 1:
  tiktoken-rs. The 50-token footer budget is coarse enough that ~15-20% approximation error
  is acceptable.
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

> **ExternalizationObligation** is a Stage 2 concept — a directive in the dynamic CLAUDE.md
> that requires the agent to externalize specific knowledge before session end. The generated
> CLAUDE.md may include obligations like "record the ADR for X before harvest." These obligations
> feed into the harvest pipeline as pre-candidates with boosted confidence. See INV-HARVEST-009.

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

### Basin Competition Model (ADR-GUIDANCE-002)

The guidance system is designed around a dynamical systems model of agent behavior.
Two attractors compete for the agent's behavioral trajectory:

- **Basin A (DDIS methodology)**: spec-language, formal reasoning, transact/query/harvest cycle.
- **Basin B (pretrained patterns)**: generic coding advice, procedural checklist compliance.

Without intervention, Basin B captures the trajectory within 15-20 turns. This is not a
memory problem (bigger context only delays the crossover). It is a dynamical systems
problem requiring continuous energy injection.

**Formal model** (spec/12-guidance.md §12.1):
```
P(Basin_A, t) + P(Basin_B, t) = 1

Without intervention: P(Basin_B, t) → 1 as t → ∞
With guidance injection: P(Basin_A, t) maintained above threshold τ

E_drift = E_preemption + E_injection + E_detection + E_gate + E_alarm + E_harvest
Stable when: E_drift > E_decay (natural drift toward Basin B)
```

**Implementation**: Each anti-drift mechanism maps to a guidance topology node:

| Mechanism | Energy Source | Topology Node Predicate | Footer Example |
|-----------|-------------|------------------------|----------------|
| Pre-emption | Dynamic CLAUDE.md | `store_has_drift_patterns()` | (emitted in CLAUDE.md, not footer) |
| Injection | Guidance footer | `true` (every response) | `↳ Which INV does this satisfy?` |
| Detection | Drift detector | `turns_without_ddis >= 5` | `↳ What divergence type does this address?` |
| Gate | Schema validation | `schema_changed_unvalidated()` | `↳ Validate schema: INV-SCHEMA-004` |
| Alarm | Budget warning | `k_eff < 0.15` | `↳ Q(t) = 0.12. Harvest now.` |
| Harvest | Harvest prompt | `high_confidence_unharvested()` | `↳ Transact high-confidence candidate.` |

Defense in depth: each mechanism covers the failure modes of the others. Pre-emption fails
when agents skip the CLAUDE.md check. Injection fails when agents ignore the footer.
Detection fails for novel drift patterns. No mechanism is a single point of failure.

**Scoring**: The `guidance_footer` function evaluates all topology nodes and selects the
highest-scoring action. The priority order (`budget_warning > harvest_prompt > drift_correction
> general_guidance`) reflects the Basin A energy model: budget and harvest signals inject
the most anti-drift energy (urgent, context-loss prevention), while general guidance provides
continuous low-energy steering.

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
  **Note**: M(t) weights are loaded from store datoms at initialization, with code-level
  fallback to the defaults above. The genesis bootstrap datoms for these weights are
  defined in [spec/02-schema.md](../spec/02-schema.md) §2.2 (M(t) Default Weight Bootstrap Datoms).
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
Drift: Basin A (spec-driven), 0 signals. Namespace: STORE (8/13 Stage 0 INVs verified).
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
    fn inv_guidance_001(store in arb_store(5), topology in arb_guidance_topology()) {
        let footer = guidance_footer(&topology, &store, 1.0);
        prop_assert!(!footer.next_action.is_empty());
        prop_assert!(footer.next_action.len() <= 200);  // ≤50 tokens ≈ ≤200 chars
    }

    // INV-GUIDANCE-002: Spec-language (footer references INV/ADR/NEG)
    fn inv_guidance_002(store in arb_store_with_specs(5), topology in arb_guidance_topology()) {
        let footer = guidance_footer(&topology, &store, 1.0);
        // At least one spec element reference in non-default footers
        let signals = detect_drift(&store, test_agent(), &[]);
        if signals.turns_without_ddis >= 5 || signals.missing_inv_references {
            let has_ref = footer.invariant_refs.iter()
                .any(|r| r.contains("INV-") || r.contains("ADR-") || r.contains("NEG-"));
            prop_assert!(has_ref || footer.next_action.contains("SEED"));
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

### Core Guidance (spec §12.3, ADR-GUIDANCE-001)
- [ ] `GuidanceTopology`, `GuidanceNode`, `GuidanceAction` types defined (comonadic topology)
- [ ] `GuidanceFooter`, `DriftSignals` types defined
- [ ] `guidance_footer()` evaluates topology nodes, selects highest-scoring action
- [ ] `query_guidance()` evaluates predicates with optional lookahead
- [ ] `detect_drift()` computes drift signals from command history
- [ ] `full_guidance()` produces comprehensive guidance output
- [ ] Six anti-drift mechanisms represented as topology nodes (spec §12.2)
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
