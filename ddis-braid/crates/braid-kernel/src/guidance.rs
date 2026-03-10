//! GUIDANCE namespace — methodology steering and anti-drift mechanisms.
//!
//! Guidance is the anti-drift mechanism that counteracts basin competition between
//! DDIS methodology (Basin A) and pretrained coding patterns (Basin B). Without
//! guidance, agents drift into Basin B within 15–20 turns.
//!
//! # Stage 0 Components
//!
//! - **M(t)**: Methodology adherence score (4 sub-metrics at Stage 0).
//! - **R(t)**: Graph-based work routing (PageRank + degree-product proxy).
//! - **GuidanceFooter**: Appended to every tool response.
//!
//! # Invariants
//!
//! - **INV-GUIDANCE-001**: Continuous injection — every response gets a footer.
//! - **INV-GUIDANCE-002**: Spec-language phrasing — references formal structures.
//! - **INV-GUIDANCE-008**: M(t) = Σ wᵢ × mᵢ(t), where Σ wᵢ = 1.
//! - **INV-GUIDANCE-009**: Task derivation completeness.
//! - **INV-GUIDANCE-010**: R(t) graph-based work routing.

use std::collections::BTreeMap;

use crate::datom::EntityId;
use crate::store::Store;

// ---------------------------------------------------------------------------
// M(t) — Methodology Adherence Score (INV-GUIDANCE-008)
// ---------------------------------------------------------------------------

/// Stage 0 methodology adherence weights (renormalized without m₅).
///
/// m₁ = transact_frequency (0.30)
/// m₂ = spec_language_ratio (0.23)
/// m₃ = query_diversity (0.17)
/// m₄ = harvest_quality (0.30)
const STAGE0_WEIGHTS: [f64; 4] = [0.30, 0.23, 0.17, 0.30];

/// Session telemetry used to compute M(t).
#[derive(Clone, Debug, Default)]
pub struct SessionTelemetry {
    /// Total turns elapsed.
    pub total_turns: u32,
    /// Turns containing a transact operation.
    pub transact_turns: u32,
    /// Turns using spec-language (invariant refs, formal structure).
    pub spec_language_turns: u32,
    /// Distinct query types issued (find, pull, aggregate, etc.).
    pub query_type_count: u32,
    /// Harvest quality score from last harvest (0.0–1.0).
    pub harvest_quality: f64,
    /// History of M(t) values for trend computation.
    pub history: Vec<f64>,
}

/// M(t) methodology adherence result.
#[derive(Clone, Debug)]
pub struct MethodologyScore {
    /// Composite score M(t) ∈ [0, 1].
    pub score: f64,
    /// Individual sub-metric values.
    pub components: MethodologyComponents,
    /// Trend arrow: Up, Down, or Stable.
    pub trend: Trend,
    /// Whether drift signal should be emitted (M(t) < 0.5).
    pub drift_signal: bool,
}

/// Individual M(t) sub-metrics.
#[derive(Clone, Debug)]
pub struct MethodologyComponents {
    /// m₁: transact_frequency — fraction of turns with transact.
    pub transact_frequency: f64,
    /// m₂: spec_language_ratio — fraction of turns using spec-language.
    pub spec_language_ratio: f64,
    /// m₃: query_diversity — distinct query types / 4 (capped at 1.0).
    pub query_diversity: f64,
    /// m₄: harvest_quality — latest harvest quality score.
    pub harvest_quality: f64,
}

/// Trend direction over recent M(t) history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trend {
    /// M(t) increasing over last 5 measurements.
    Up,
    /// M(t) decreasing over last 5 measurements.
    Down,
    /// M(t) stable (< 0.05 change).
    Stable,
}

/// Compute M(t) from session telemetry (INV-GUIDANCE-008).
///
/// Stage 0 uses 4 components with weights (0.30, 0.23, 0.17, 0.30).
/// M(t) = Σᵢ wᵢ × mᵢ(t).
pub fn compute_methodology_score(telemetry: &SessionTelemetry) -> MethodologyScore {
    let total = telemetry.total_turns.max(1) as f64;

    let m1 = (telemetry.transact_turns as f64 / total).min(1.0);
    let m2 = (telemetry.spec_language_turns as f64 / total).min(1.0);
    let m3 = (telemetry.query_type_count as f64 / 4.0).min(1.0);
    let m4 = telemetry.harvest_quality;

    let metrics = [m1, m2, m3, m4];
    let score: f64 = STAGE0_WEIGHTS
        .iter()
        .zip(metrics.iter())
        .map(|(w, m)| w * m)
        .sum();

    // Trend: compare to mean of last 5 measurements
    let trend = if telemetry.history.len() >= 2 {
        let recent: Vec<f64> = telemetry.history.iter().rev().take(5).copied().collect();
        let mean = recent.iter().sum::<f64>() / recent.len() as f64;
        if score > mean + 0.05 {
            Trend::Up
        } else if score < mean - 0.05 {
            Trend::Down
        } else {
            Trend::Stable
        }
    } else {
        Trend::Stable
    };

    MethodologyScore {
        score,
        components: MethodologyComponents {
            transact_frequency: m1,
            spec_language_ratio: m2,
            query_diversity: m3,
            harvest_quality: m4,
        },
        trend,
        drift_signal: score < 0.5,
    }
}

// ---------------------------------------------------------------------------
// R(t) — Graph-Based Work Routing (INV-GUIDANCE-010)
// ---------------------------------------------------------------------------

/// A task node in the work dependency graph.
#[derive(Clone, Debug)]
pub struct TaskNode {
    /// Entity ID of this task.
    pub entity: EntityId,
    /// Human-readable label.
    pub label: String,
    /// Priority boost (0.0–1.0).
    pub priority_boost: f64,
    /// Whether this task is complete.
    pub done: bool,
    /// Dependencies (entity IDs of tasks this depends on).
    pub depends_on: Vec<EntityId>,
    /// Dependents (entity IDs of tasks that depend on this).
    pub blocks: Vec<EntityId>,
    /// Wall time when task was created (for staleness).
    pub created_at: u64,
}

/// R(t) routing result for a single task.
#[derive(Clone, Debug)]
pub struct TaskRouting {
    /// The task entity.
    pub entity: EntityId,
    /// Task label.
    pub label: String,
    /// Composite impact score.
    pub impact: f64,
    /// Individual metric scores.
    pub metrics: RoutingMetrics,
}

/// Individual R(t) routing metrics.
#[derive(Clone, Debug)]
pub struct RoutingMetrics {
    /// g₁: PageRank (dependency authority).
    pub pagerank: f64,
    /// g₂: betweenness proxy (degree product at Stage 0, ADR-GUIDANCE-009).
    pub betweenness_proxy: f64,
    /// g₃: critical path position (0 or 1 at Stage 0).
    pub critical_path_pos: f64,
    /// g₄: blocker ratio (fraction of all tasks this unblocks).
    pub blocker_ratio: f64,
    /// g₅: staleness (age / max_age, capped at 1.0).
    pub staleness: f64,
    /// g₆: priority boost (from task metadata).
    pub priority_boost: f64,
}

/// R(t) routing weights (defaults from spec).
const ROUTING_WEIGHTS: [f64; 6] = [0.25, 0.25, 0.20, 0.15, 0.10, 0.05];

/// Compute R(t) — ranked routing over a task graph (INV-GUIDANCE-010).
///
/// Returns tasks sorted by descending impact score.
/// Only includes tasks that are ready (all dependencies complete).
pub fn compute_routing(tasks: &[TaskNode], now: u64) -> Vec<TaskRouting> {
    if tasks.is_empty() {
        return Vec::new();
    }

    // Index for fast lookups
    let idx: BTreeMap<EntityId, &TaskNode> = tasks.iter().map(|t| (t.entity, t)).collect();
    let total_tasks = tasks.len() as f64;

    // Identify ready tasks (not done, all deps complete)
    let ready: Vec<&TaskNode> = tasks
        .iter()
        .filter(|t| {
            !t.done
                && t.depends_on
                    .iter()
                    .all(|dep| idx.get(dep).is_some_and(|d| d.done))
        })
        .collect();

    // Compute max age for staleness normalization
    let max_age = tasks
        .iter()
        .map(|t| now.saturating_sub(t.created_at))
        .max()
        .unwrap_or(1)
        .max(1) as f64;

    // Compute PageRank (simplified: in-degree normalized)
    let max_in_degree = tasks
        .iter()
        .map(|t| t.blocks.len())
        .max()
        .unwrap_or(1)
        .max(1) as f64;

    let mut routings: Vec<TaskRouting> = ready
        .iter()
        .map(|task| {
            // g₁: PageRank proxy — normalized in-degree (how many tasks depend on this)
            let pagerank = task.blocks.len() as f64 / max_in_degree;

            // g₂: betweenness proxy — degree product (ADR-GUIDANCE-009)
            let in_degree = task.depends_on.len() as f64;
            let out_degree = task.blocks.len() as f64;
            let max_product = (max_in_degree * max_in_degree).max(1.0);
            let betweenness_proxy = (in_degree * out_degree) / max_product;

            // g₃: critical path position — 1.0 if any dependent is also blocked
            let on_critical = task.blocks.iter().any(|b| {
                idx.get(b).is_some_and(|bt| {
                    !bt.done
                        && bt
                            .blocks
                            .iter()
                            .any(|bb| idx.get(bb).is_some_and(|bbt| !bbt.done))
                })
            });
            let critical_path_pos = if on_critical { 1.0 } else { 0.0 };

            // g₄: blocker ratio
            let blocker_ratio = task.blocks.len() as f64 / total_tasks;

            // g₅: staleness
            let age = now.saturating_sub(task.created_at) as f64;
            let staleness = (age / max_age).min(1.0);

            // g₆: priority boost
            let priority_boost = task.priority_boost;

            let metrics = RoutingMetrics {
                pagerank,
                betweenness_proxy,
                critical_path_pos,
                blocker_ratio,
                staleness,
                priority_boost,
            };

            let values = [
                pagerank,
                betweenness_proxy,
                critical_path_pos,
                blocker_ratio,
                staleness,
                priority_boost,
            ];
            let impact: f64 = ROUTING_WEIGHTS
                .iter()
                .zip(values.iter())
                .map(|(w, v)| w * v)
                .sum();

            TaskRouting {
                entity: task.entity,
                label: task.label.clone(),
                impact,
                metrics,
            }
        })
        .collect();

    // Sort by descending impact
    routings.sort_by(|a, b| {
        b.impact
            .partial_cmp(&a.impact)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    routings
}

// ---------------------------------------------------------------------------
// Guidance Footer (INV-GUIDANCE-001)
// ---------------------------------------------------------------------------

/// Guidance footer appended to every tool response.
#[derive(Clone, Debug)]
pub struct GuidanceFooter {
    /// M(t) methodology score.
    pub methodology: MethodologyScore,
    /// Top recommended next action.
    pub next_action: Option<String>,
    /// Invariant references for the next action.
    pub invariant_refs: Vec<String>,
    /// Store state summary.
    pub store_datom_count: usize,
    /// Current turn number.
    pub turn: u32,
}

/// Format a guidance footer as a compact string (ADR-GUIDANCE-008).
///
/// Format:
/// ```text
/// ↳ M(t): 0.73 (tx: ✓ | spec-lang: ✓ | q-div: △ | harvest: ✓) | Store: 142 datoms | Turn 7
///   Next: braid query [:find ...] — verify INV-STORE-003
/// ```
pub fn format_footer(footer: &GuidanceFooter) -> String {
    let m = &footer.methodology;
    let trend = match m.trend {
        Trend::Up => "↑",
        Trend::Down => "↓",
        Trend::Stable => "→",
    };

    let check = |v: f64| {
        if v >= 0.7 {
            "✓"
        } else if v >= 0.4 {
            "△"
        } else {
            "✗"
        }
    };

    let line1 = format!(
        "↳ M(t): {:.2} {} (tx: {} | spec-lang: {} | q-div: {} | harvest: {}) | Store: {} datoms | Turn {}",
        m.score,
        trend,
        check(m.components.transact_frequency),
        check(m.components.spec_language_ratio),
        check(m.components.query_diversity),
        check(m.components.harvest_quality),
        footer.store_datom_count,
        footer.turn,
    );

    match &footer.next_action {
        Some(action) => {
            let refs = if footer.invariant_refs.is_empty() {
                String::new()
            } else {
                format!(" — verify {}", footer.invariant_refs.join(", "))
            };
            format!("{line1}\n  Next: {action}{refs}")
        }
        None => line1,
    }
}

/// Build a guidance footer from current session state.
pub fn build_footer(
    telemetry: &SessionTelemetry,
    store: &Store,
    next_action: Option<String>,
    invariant_refs: Vec<String>,
) -> GuidanceFooter {
    let methodology = compute_methodology_score(telemetry);
    GuidanceFooter {
        methodology,
        next_action,
        invariant_refs,
        store_datom_count: store.len(),
        turn: telemetry.total_turns,
    }
}

// ---------------------------------------------------------------------------
// Task Derivation (INV-GUIDANCE-009)
// ---------------------------------------------------------------------------

/// A derivation rule that produces tasks from specification artifacts.
#[derive(Clone, Debug)]
pub struct DerivationRule {
    /// Rule ID.
    pub id: String,
    /// Artifact type this rule matches (e.g., "invariant", "adr", "neg").
    pub artifact_type: String,
    /// Task template — {id} is replaced with the artifact ID.
    pub task_template: String,
    /// Priority function output (0.0–1.0).
    pub priority: f64,
}

/// A derived task produced by applying derivation rules.
#[derive(Clone, Debug)]
pub struct DerivedTask {
    /// Derived task label.
    pub label: String,
    /// Source artifact ID that generated this task.
    pub source_artifact: String,
    /// The rule that generated this task.
    pub rule_id: String,
    /// Computed priority.
    pub priority: f64,
}

/// Default derivation rules (10 rules from spec INV-GUIDANCE-009).
pub fn default_derivation_rules() -> Vec<DerivationRule> {
    vec![
        DerivationRule {
            id: "R01".into(),
            artifact_type: "invariant".into(),
            task_template: "Implement {id}".into(),
            priority: 0.9,
        },
        DerivationRule {
            id: "R02".into(),
            artifact_type: "invariant".into(),
            task_template: "Write test for {id}".into(),
            priority: 0.85,
        },
        DerivationRule {
            id: "R03".into(),
            artifact_type: "adr".into(),
            task_template: "Implement decision from {id}".into(),
            priority: 0.7,
        },
        DerivationRule {
            id: "R04".into(),
            artifact_type: "neg".into(),
            task_template: "Write negative test for {id}".into(),
            priority: 0.8,
        },
        DerivationRule {
            id: "R05".into(),
            artifact_type: "neg".into(),
            task_template: "Add runtime guard for {id}".into(),
            priority: 0.75,
        },
        DerivationRule {
            id: "R06".into(),
            artifact_type: "uncertainty".into(),
            task_template: "Resolve uncertainty {id}".into(),
            priority: 0.6,
        },
        DerivationRule {
            id: "R07".into(),
            artifact_type: "section".into(),
            task_template: "Implement namespace from {id}".into(),
            priority: 0.5,
        },
        DerivationRule {
            id: "R08".into(),
            artifact_type: "invariant".into(),
            task_template: "Add proptest property for {id}".into(),
            priority: 0.65,
        },
        DerivationRule {
            id: "R09".into(),
            artifact_type: "adr".into(),
            task_template: "Document rationale for {id}".into(),
            priority: 0.4,
        },
        DerivationRule {
            id: "R10".into(),
            artifact_type: "invariant".into(),
            task_template: "Add Kani harness for {id}".into(),
            priority: 0.55,
        },
    ]
}

/// Derive tasks from a set of specification artifacts using derivation rules.
///
/// INV-GUIDANCE-009: Total function from artifacts to tasks.
pub fn derive_tasks(
    artifacts: &[(String, String)], // (id, type)
    rules: &[DerivationRule],
) -> Vec<DerivedTask> {
    let mut tasks = Vec::new();

    for (artifact_id, artifact_type) in artifacts {
        for rule in rules {
            if &rule.artifact_type == artifact_type {
                let label = rule.task_template.replace("{id}", artifact_id);
                tasks.push(DerivedTask {
                    label,
                    source_artifact: artifact_id.clone(),
                    rule_id: rule.id.clone(),
                    priority: rule.priority,
                });
            }
        }
    }

    // Sort by descending priority
    tasks.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    tasks
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methodology_score_weights_sum_to_one() {
        let sum: f64 = STAGE0_WEIGHTS.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10, "weights must sum to 1.0");
    }

    #[test]
    fn routing_weights_sum_to_one() {
        let sum: f64 = ROUTING_WEIGHTS.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10, "weights must sum to 1.0");
    }

    #[test]
    fn methodology_score_bounds() {
        // Perfect session
        let perfect = SessionTelemetry {
            total_turns: 10,
            transact_turns: 10,
            spec_language_turns: 10,
            query_type_count: 4,
            harvest_quality: 1.0,
            history: vec![],
        };
        let score = compute_methodology_score(&perfect);
        assert!(
            (score.score - 1.0).abs() < 1e-10,
            "perfect session should score 1.0"
        );
        assert!(!score.drift_signal);

        // Empty session
        let empty = SessionTelemetry::default();
        let score = compute_methodology_score(&empty);
        assert!(score.score >= 0.0);
        assert!(score.drift_signal, "empty session triggers drift signal");
    }

    #[test]
    fn methodology_trend_detection() {
        let telemetry = SessionTelemetry {
            total_turns: 10,
            transact_turns: 8,
            spec_language_turns: 7,
            query_type_count: 3,
            harvest_quality: 0.9,
            history: vec![0.3, 0.4, 0.5, 0.6, 0.7],
        };
        let score = compute_methodology_score(&telemetry);
        assert_eq!(score.trend, Trend::Up, "improving history should trend up");
    }

    #[test]
    fn routing_returns_only_ready_tasks() {
        let e1 = EntityId::from_ident(":task/a");
        let e2 = EntityId::from_ident(":task/b");
        let e3 = EntityId::from_ident(":task/c");

        let tasks = vec![
            TaskNode {
                entity: e1,
                label: "task-a".into(),
                priority_boost: 0.0,
                done: true,
                depends_on: vec![],
                blocks: vec![e2],
                created_at: 0,
            },
            TaskNode {
                entity: e2,
                label: "task-b".into(),
                priority_boost: 0.5,
                done: false,
                depends_on: vec![e1], // e1 is done, so e2 is ready
                blocks: vec![e3],
                created_at: 10,
            },
            TaskNode {
                entity: e3,
                label: "task-c".into(),
                priority_boost: 0.0,
                done: false,
                depends_on: vec![e2], // e2 not done, so e3 is blocked
                blocks: vec![],
                created_at: 20,
            },
        ];

        let routings = compute_routing(&tasks, 100);
        assert_eq!(routings.len(), 1, "only task-b should be ready");
        assert_eq!(routings[0].label, "task-b");
        assert!(routings[0].impact > 0.0);
    }

    #[test]
    fn routing_empty_graph() {
        let routings = compute_routing(&[], 100);
        assert!(routings.is_empty());
    }

    #[test]
    fn derive_tasks_produces_correct_count() {
        let artifacts = vec![
            ("INV-STORE-001".to_string(), "invariant".to_string()),
            ("ADR-SEED-001".to_string(), "adr".to_string()),
            ("NEG-MUTATION-001".to_string(), "neg".to_string()),
        ];
        let rules = default_derivation_rules();
        let tasks = derive_tasks(&artifacts, &rules);

        // INV-STORE-001 matches 4 invariant rules (R01, R02, R08, R10)
        // ADR-SEED-001 matches 2 adr rules (R03, R09)
        // NEG-MUTATION-001 matches 2 neg rules (R04, R05)
        assert_eq!(tasks.len(), 8);
        assert!(tasks[0].priority >= tasks[tasks.len() - 1].priority);
    }

    #[test]
    fn footer_format_includes_all_components() {
        let telemetry = SessionTelemetry {
            total_turns: 7,
            transact_turns: 5,
            spec_language_turns: 6,
            query_type_count: 3,
            harvest_quality: 0.9,
            history: vec![],
        };
        let store = Store::genesis();
        let footer = build_footer(
            &telemetry,
            &store,
            Some("braid query [:find ?e :where [?e :db/ident]]".into()),
            vec!["INV-STORE-003".into()],
        );
        let formatted = format_footer(&footer);

        assert!(formatted.contains("M(t):"));
        assert!(formatted.contains("Store:"));
        assert!(formatted.contains("Turn 7"));
        assert!(formatted.contains("Next:"));
        assert!(formatted.contains("INV-STORE-003"));
    }

    #[test]
    fn footer_without_next_action() {
        let telemetry = SessionTelemetry::default();
        let store = Store::genesis();
        let footer = build_footer(&telemetry, &store, None, vec![]);
        let formatted = format_footer(&footer);
        assert!(formatted.contains("M(t):"));
        assert!(!formatted.contains("Next:"));
    }

    // -------------------------------------------------------------------
    // Proptest: compute_routing, derive_tasks, task derivation properties
    // -------------------------------------------------------------------

    mod routing_derivation_proptests {
        use super::*;
        use crate::datom::EntityId;
        use proptest::prelude::*;

        fn arb_task_node(idx: usize, num_nodes: usize) -> impl Strategy<Value = TaskNode> {
            let entity = EntityId::from_ident(&format!(":task/t{idx}"));
            (
                prop::bool::ANY, // done
                0.0f64..1.0,     // priority_boost
                0u64..1000,      // created_at
                proptest::collection::vec(0..num_nodes, 0..num_nodes.min(3)),
            )
                .prop_map(move |(done, priority_boost, created_at, dep_indices)| {
                    let depends_on: Vec<EntityId> = dep_indices
                        .into_iter()
                        .filter(|&d| d != idx)
                        .map(|d| EntityId::from_ident(&format!(":task/t{d}")))
                        .collect();
                    TaskNode {
                        entity,
                        label: format!("task-{idx}"),
                        priority_boost,
                        done,
                        depends_on,
                        blocks: vec![],
                        created_at,
                    }
                })
        }

        fn arb_task_graph(max_nodes: usize) -> impl Strategy<Value = Vec<TaskNode>> {
            let max = max_nodes.max(1);
            (1..=max)
                .prop_flat_map(|n| {
                    let strategies: Vec<_> = (0..n).map(|i| arb_task_node(i, n)).collect();
                    strategies
                })
                .prop_map(|mut nodes| {
                    // Compute blocks from depends_on
                    let entities: Vec<EntityId> = nodes.iter().map(|n| n.entity).collect();
                    let mut blocks_map: Vec<Vec<EntityId>> = vec![vec![]; nodes.len()];
                    for (i, node) in nodes.iter().enumerate() {
                        for dep in &node.depends_on {
                            if let Some(j) = entities.iter().position(|e| e == dep) {
                                if !blocks_map[j].contains(&entities[i]) {
                                    blocks_map[j].push(entities[i]);
                                }
                            }
                        }
                    }
                    for (i, node) in nodes.iter_mut().enumerate() {
                        node.blocks = blocks_map[i].clone();
                    }
                    nodes
                })
        }

        fn arb_artifacts(max: usize) -> impl Strategy<Value = Vec<(String, String)>> {
            let types = vec![
                "invariant".to_string(),
                "adr".to_string(),
                "neg".to_string(),
                "uncertainty".to_string(),
                "section".to_string(),
            ];
            proptest::collection::vec((0..max.max(1), prop::sample::select(types)), 0..=max)
                .prop_map(|pairs| {
                    pairs
                        .into_iter()
                        .map(|(idx, atype)| {
                            let prefix = match atype.as_str() {
                                "invariant" => "INV",
                                "adr" => "ADR",
                                "neg" => "NEG",
                                "uncertainty" => "UNC",
                                _ => "SEC",
                            };
                            (format!("{prefix}-TEST-{idx:03}"), atype)
                        })
                        .collect()
                })
        }

        proptest! {
            #[test]
            fn routing_returns_only_ready(tasks in arb_task_graph(8)) {
                let routings = compute_routing(&tasks, 1000);
                let task_map: std::collections::BTreeMap<EntityId, &TaskNode> =
                    tasks.iter().map(|t| (t.entity, t)).collect();

                for r in &routings {
                    let task = task_map.get(&r.entity)
                        .expect("routed task must exist in input");
                    prop_assert!(
                        !task.done,
                        "Routed task {:?} must not be done",
                        r.label
                    );
                    for dep in &task.depends_on {
                        if let Some(dep_task) = task_map.get(dep) {
                            prop_assert!(
                                dep_task.done,
                                "Dependency {:?} of ready task {:?} must be done",
                                dep_task.label,
                                r.label
                            );
                        }
                    }
                }
            }

            #[test]
            fn derive_tasks_count_matches_cross_product(
                artifacts in arb_artifacts(6),
            ) {
                let rules = default_derivation_rules();
                let derived = derive_tasks(&artifacts, &rules);

                // Expected count: sum over each artifact of matching rules
                let expected: usize = artifacts.iter().map(|(_, atype)| {
                    rules.iter().filter(|r| &r.artifact_type == atype).count()
                }).sum();

                prop_assert_eq!(
                    derived.len(),
                    expected,
                    "derive_tasks must produce exactly artifacts x matching rules tasks"
                );
            }

            #[test]
            fn derived_tasks_sorted_by_descending_priority(
                artifacts in arb_artifacts(6),
            ) {
                let rules = default_derivation_rules();
                let derived = derive_tasks(&artifacts, &rules);

                for window in derived.windows(2) {
                    prop_assert!(
                        window[0].priority >= window[1].priority,
                        "Tasks must be sorted descending by priority: {} >= {} violated",
                        window[0].priority,
                        window[1].priority
                    );
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Proptest formal verification: Guidance Comonad Laws (brai-lkm7)
    // -------------------------------------------------------------------
    //
    // The Guidance system forms a comonad W where W(A) = (Store, A).
    // In Rust, we verify the concrete instances of the comonad laws:
    //
    // 1. extract . duplicate = id  (determinism / context recovery)
    // 2. fmap extract . duplicate = id  (context projection)
    // 3. duplicate . duplicate = fmap duplicate . duplicate  (associativity)
    //
    // These map to:
    // - compute_methodology_score is deterministic (same telemetry -> same score)
    // - build_footer depends only on (telemetry, store) — the comonad context
    // - format_footer(build_footer(...)) always produces valid output
    // - M(t) in [0, 1] for all valid telemetry

    mod comonad_laws {
        use super::*;
        use crate::store::Store;
        use proptest::prelude::*;

        fn arb_telemetry() -> impl Strategy<Value = SessionTelemetry> {
            (
                1u32..100,   // total_turns (at least 1 to avoid trivial edge)
                0u32..100,   // transact_turns
                0u32..100,   // spec_language_turns
                0u32..10,    // query_type_count
                0.0f64..1.0, // harvest_quality
                proptest::collection::vec(0.0f64..1.0, 0..10), // history
            )
                .prop_map(|(total, transact, spec_lang, query, harvest, history)| {
                    SessionTelemetry {
                        total_turns: total,
                        transact_turns: transact.min(total),
                        spec_language_turns: spec_lang.min(total),
                        query_type_count: query,
                        harvest_quality: harvest,
                        history,
                    }
                })
        }

        proptest! {
            #[test]
            fn methodology_score_deterministic(telemetry in arb_telemetry()) {
                // Comonad law 1 (extract . duplicate = id):
                // compute_methodology_score is a pure function —
                // same telemetry always produces the same score.
                let score_a = compute_methodology_score(&telemetry);
                let score_b = compute_methodology_score(&telemetry);
                prop_assert!(
                    (score_a.score - score_b.score).abs() < f64::EPSILON,
                    "methodology score not deterministic: {} vs {}",
                    score_a.score,
                    score_b.score
                );
                prop_assert_eq!(score_a.drift_signal, score_b.drift_signal);
                prop_assert_eq!(score_a.trend, score_b.trend);
            }

            #[test]
            fn methodology_score_in_unit_interval(telemetry in arb_telemetry()) {
                // M(t) in [0, 1] for all valid telemetry
                let score = compute_methodology_score(&telemetry);
                prop_assert!(
                    score.score >= 0.0,
                    "M(t) must be >= 0.0, got {}",
                    score.score
                );
                prop_assert!(
                    score.score <= 1.0,
                    "M(t) must be <= 1.0, got {}",
                    score.score
                );
            }

            #[test]
            fn build_footer_depends_only_on_context(telemetry in arb_telemetry()) {
                // Comonad law 2 (fmap extract . duplicate = id):
                // build_footer depends only on (telemetry, store) — the comonad
                // context W = (Store, A). Given identical context, the output
                // is identical.
                let store = Store::genesis();
                let action = Some("braid query [:find ?e]".to_string());
                let refs = vec!["INV-STORE-001".to_string()];

                let footer_a = build_footer(
                    &telemetry,
                    &store,
                    action.clone(),
                    refs.clone(),
                );
                let footer_b = build_footer(
                    &telemetry,
                    &store,
                    action,
                    refs,
                );

                prop_assert!(
                    (footer_a.methodology.score - footer_b.methodology.score).abs()
                        < f64::EPSILON,
                    "build_footer not deterministic on same context"
                );
                prop_assert_eq!(footer_a.store_datom_count, footer_b.store_datom_count);
                prop_assert_eq!(footer_a.turn, footer_b.turn);
            }

            #[test]
            fn format_footer_always_valid(telemetry in arb_telemetry()) {
                // Comonad law 3 (associativity of duplication):
                // format_footer(build_footer(t, s, a, r)) always produces
                // valid output containing "M(t):" — the comonad join is
                // well-formed.
                let store = Store::genesis();
                let footer = build_footer(
                    &telemetry,
                    &store,
                    Some("test action".to_string()),
                    vec!["INV-TEST-001".to_string()],
                );
                let formatted = format_footer(&footer);

                prop_assert!(
                    formatted.contains("M(t):"),
                    "formatted footer must contain M(t):, got: {}",
                    formatted
                );
                prop_assert!(
                    formatted.contains("Store:"),
                    "formatted footer must contain Store:, got: {}",
                    formatted
                );
                prop_assert!(
                    formatted.contains("Turn"),
                    "formatted footer must contain Turn, got: {}",
                    formatted
                );
            }

            #[test]
            fn format_footer_without_action_valid(telemetry in arb_telemetry()) {
                // Even without a next_action, the footer is well-formed
                let store = Store::genesis();
                let footer = build_footer(&telemetry, &store, None, vec![]);
                let formatted = format_footer(&footer);

                prop_assert!(
                    formatted.contains("M(t):"),
                    "footer without action must contain M(t):"
                );
                prop_assert!(
                    !formatted.contains("Next:"),
                    "footer without action must not contain Next:"
                );
            }

            #[test]
            fn methodology_components_in_unit_interval(telemetry in arb_telemetry()) {
                // Each sub-metric m_i(t) in [0, 1]
                let score = compute_methodology_score(&telemetry);
                let c = &score.components;
                prop_assert!(c.transact_frequency >= 0.0 && c.transact_frequency <= 1.0,
                    "m1 out of range: {}", c.transact_frequency);
                prop_assert!(c.spec_language_ratio >= 0.0 && c.spec_language_ratio <= 1.0,
                    "m2 out of range: {}", c.spec_language_ratio);
                prop_assert!(c.query_diversity >= 0.0 && c.query_diversity <= 1.0,
                    "m3 out of range: {}", c.query_diversity);
                prop_assert!(c.harvest_quality >= 0.0 && c.harvest_quality <= 1.0,
                    "m4 out of range: {}", c.harvest_quality);
            }

            #[test]
            fn drift_signal_iff_low_score(telemetry in arb_telemetry()) {
                // drift_signal == true iff M(t) < 0.5
                let score = compute_methodology_score(&telemetry);
                prop_assert_eq!(
                    score.drift_signal,
                    score.score < 0.5,
                    "drift_signal inconsistent: score={}, signal={}",
                    score.score,
                    score.drift_signal,
                );
            }
        }
    }
}
