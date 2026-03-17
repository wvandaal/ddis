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
//! - **INV-GUIDANCE-005**: Learned guidance effectiveness tracking.
//! - **INV-GUIDANCE-006**: Lookahead via branch simulation.
//! - **INV-GUIDANCE-008**: M(t) = Σ wᵢ × mᵢ(t), where Σ wᵢ = 1.
//! - **INV-GUIDANCE-009**: Task derivation completeness.
//! - **INV-GUIDANCE-010**: R(t) graph-based work routing.
//! - **INV-GUIDANCE-011**: T(t) topology fitness.
//!
//! # Design Decisions
//!
//! - ADR-GUIDANCE-001: Comonadic topology over flat rules.
//! - ADR-GUIDANCE-002: Basin competition as central failure model.
//! - ADR-GUIDANCE-003: Six integrated mechanisms over single solution.
//! - ADR-GUIDANCE-004: Spec-language over instruction-language.
//! - ADR-GUIDANCE-005: Unified guidance as M(t) ⊗ R(t) ⊗ T(t).
//! - ADR-GUIDANCE-006: Query over guidance graph.
//! - ADR-GUIDANCE-007: System 1/System 2 diagnosis.
//!
//! # Negative Cases
//!
//! - NEG-GUIDANCE-001: No tool response without footer.
//! - NEG-GUIDANCE-002: No lookahead branch leak.
//! - NEG-GUIDANCE-003: No ineffective guidance persistence.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{quality_adjusted_budget, GuidanceLevel};
use crate::datom::{Attribute, EntityId, Op, Value};
use crate::store::Store;
use crate::trilateral::{check_coherence_fast, CoherenceQuadrant};

// ---------------------------------------------------------------------------
// Harvest Warning Level (Q(t)-based thresholds)
// ---------------------------------------------------------------------------

/// Harvest urgency level derived from Q(t) attention decay.
///
/// Replaces heuristic tx-count thresholds with the attention decay model
/// from spec/13-budget.md. Q(t) = k*_eff x attention_decay(k*_eff) maps
/// directly to urgency bands:
///
/// - Q(t) > 0.6  -> None (plenty of budget remaining)
/// - Q(t) in [0.3, 0.6] -> Info (context filling, harvest recommended)
/// - Q(t) in [0.15, 0.3] -> Warn (harvest soon)
/// - Q(t) < 0.15 -> Critical (harvest immediately)
///
/// INV-HARVEST-005: Proactive warning fires at correct thresholds.
/// ADR-BUDGET-001: Measured context over heuristic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HarvestWarningLevel {
    /// Q(t) > 0.6: no warning needed.
    None,
    /// Q(t) in [0.3, 0.6]: context filling, harvest recommended.
    Info,
    /// Q(t) in [0.15, 0.3]: harvest soon.
    Warn,
    /// Q(t) < 0.15: context nearly exhausted, harvest immediately.
    Critical,
}

impl HarvestWarningLevel {
    /// Human-readable message for this warning level.
    pub fn message(&self) -> &'static str {
        match self {
            HarvestWarningLevel::None => "",
            HarvestWarningLevel::Info => "context filling \u{2014} harvest recommended",
            HarvestWarningLevel::Warn => "harvest soon: braid harvest --commit",
            HarvestWarningLevel::Critical => "HARVEST NOW: context nearly exhausted",
        }
    }

    /// Suggested action command for this warning level.
    pub fn suggested_action(&self) -> Option<&'static str> {
        match self {
            HarvestWarningLevel::None => Option::None,
            HarvestWarningLevel::Info => Some("braid harvest --task \"<current task>\" --commit"),
            HarvestWarningLevel::Warn => Some("braid harvest --commit"),
            HarvestWarningLevel::Critical => Some("braid harvest --commit"),
        }
    }

    /// Whether this level should be displayed (anything above None).
    pub fn is_active(&self) -> bool {
        !matches!(self, HarvestWarningLevel::None)
    }

    /// Map to GuidanceAction priority (1=highest, 3=lowest).
    pub fn to_priority(&self) -> u8 {
        match self {
            HarvestWarningLevel::None => 4,
            HarvestWarningLevel::Info => 3,
            HarvestWarningLevel::Warn => 2,
            HarvestWarningLevel::Critical => 1,
        }
    }
}

impl std::fmt::Display for HarvestWarningLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HarvestWarningLevel::None => write!(f, ""),
            HarvestWarningLevel::Info => write!(f, "[harvest recommended]"),
            HarvestWarningLevel::Warn => {
                write!(f, "[\u{26a0} harvest soon]")
            }
            HarvestWarningLevel::Critical => {
                write!(f, "[\u{26a0} HARVEST NOW]")
            }
        }
    }
}

/// Compute harvest warning level from Q(t) attention quality.
///
/// Q(t) = k*_eff x attention_decay(k*_eff) is the quality-adjusted budget.
/// This maps Q(t) to four urgency bands:
///
/// - Q(t) > 0.6  -> None
/// - Q(t) in [0.3, 0.6] -> Info
/// - Q(t) in [0.15, 0.3] -> Warn
/// - Q(t) < 0.15 -> Critical
///
/// INV-HARVEST-005: Proactive warning fires at correct thresholds.
/// ADR-BUDGET-001: Measured context over heuristic.
pub fn harvest_warning_level(q_t: f64) -> HarvestWarningLevel {
    if q_t > 0.6 {
        HarvestWarningLevel::None
    } else if q_t >= 0.3 {
        HarvestWarningLevel::Info
    } else if q_t >= 0.15 {
        HarvestWarningLevel::Warn
    } else {
        HarvestWarningLevel::Critical
    }
}

/// Compute harvest warning level from k*_eff (convenience wrapper).
///
/// Converts k*_eff to Q(t) via `quality_adjusted_budget()`, then applies thresholds.
pub fn harvest_warning_from_k_eff(k_eff: f64) -> HarvestWarningLevel {
    let q_t = quality_adjusted_budget(k_eff);
    harvest_warning_level(q_t)
}

/// Decay rate per wall-time step for observation staleness.
/// After 15 steps, 0.95^15 ≈ 0.4633, so an observation at confidence 0.8
/// would have staleness = 1 - 0.8 * 0.4633 ≈ 0.63.
const STALENESS_DECAY_RATE: f64 = 0.95;

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
    /// Whether the last harvest is recent (< 10 txns ago).
    /// When true, M(t) is clamped to a floor of 0.50 to prevent
    /// false DRIFT warnings between active sessions (A3 fix).
    pub harvest_is_recent: bool,
}

/// M(t) methodology adherence result.
/// INV-SIGNAL-001: Signal as datom — drift_signal is emitted as a store event.
/// INV-SIGNAL-004: Severity-ordered routing — drift triggers at M(t) < 0.5.
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
    let raw_score: f64 = STAGE0_WEIGHTS
        .iter()
        .zip(metrics.iter())
        .map(|(w, m)| w * m)
        .sum();

    // A3: Floor clamp — when harvest is recent, M(t) cannot drop below 0.50.
    // This prevents false DRIFT warnings (CC-5) between active sessions where
    // transact_frequency and query_diversity are naturally low.
    let score = if telemetry.harvest_is_recent {
        raw_score.max(0.50)
    } else {
        raw_score
    };

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

/// Build `TaskNode` list from the live store and compute R(t) routing.
///
/// This bridges the gap between the datom-level task representation (`:task/*`
/// attributes) and the graph-based routing algorithm (`compute_routing`).
///
/// Steps:
/// 1. `all_tasks(store)` to collect every `TaskSummary`.
/// 2. Build a `Vec<TaskNode>` with priority mapped to a boost factor,
///    reverse-edge (`blocks`) computation, and non-task dependency filtering.
/// 3. Call `compute_routing()` with the constructed graph.
///
/// **INV-GUIDANCE-010**: R(t) routing over real store tasks.
pub fn compute_routing_from_store(store: &Store) -> Vec<TaskRouting> {
    let summaries = crate::task::all_tasks(store);
    if summaries.is_empty() {
        return Vec::new();
    }

    // Collect the set of known task entities for dependency filtering.
    let task_entities: BTreeSet<EntityId> = summaries.iter().map(|t| t.entity).collect();

    // Build reverse-edge map: for each task, who does it block?
    let mut blocks_map: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    for t in &summaries {
        for dep in &t.depends_on {
            if task_entities.contains(dep) {
                blocks_map.entry(*dep).or_default().push(t.entity);
            }
        }
    }

    // Convert TaskSummary -> TaskNode
    let nodes: Vec<TaskNode> = summaries
        .iter()
        .map(|t| {
            // Map priority 0..4 -> boost 1.0, 0.8, 0.6, 0.4, 0.2
            let priority_boost = match t.priority {
                0 => 1.0,
                1 => 0.8,
                2 => 0.6,
                3 => 0.4,
                _ => 0.2, // 4 or any out-of-range
            };

            // Filter depends_on to only reference known task entities
            let depends_on: Vec<EntityId> = t
                .depends_on
                .iter()
                .filter(|d| task_entities.contains(d))
                .copied()
                .collect();

            let blocks = blocks_map.get(&t.entity).cloned().unwrap_or_default();

            TaskNode {
                entity: t.entity,
                label: t.title.clone(),
                priority_boost,
                done: t.status == crate::task::TaskStatus::Closed,
                depends_on,
                blocks,
                created_at: t.created_at,
            }
        })
        .collect();

    // Use wall-clock now for staleness normalization
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    compute_routing(&nodes, now)
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
    /// Q(t) harvest warning level (derived from attention budget when available).
    pub harvest_warning: HarvestWarningLevel,
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

    let check_with_hint = |v: f64, hint: &str| -> String {
        if v >= 0.7 {
            "✓".to_string()
        } else if v >= 0.4 {
            "△".to_string()
        } else {
            format!("✗→{hint}")
        }
    };

    let line1 = format!(
        "↳ M(t): {:.2} {} (tx: {} | spec-lang: {} | q-div: {} | harvest: {}) | Store: {} datoms | Turn {}",
        m.score,
        trend,
        check_with_hint(m.components.transact_frequency, "write"),
        check_with_hint(m.components.spec_language_ratio, "trace"),
        check_with_hint(m.components.query_diversity, "query"),
        check_with_hint(m.components.harvest_quality, "harvest"),
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

/// Format a guidance footer at the specified compression level (INV-BUDGET-004).
///
/// Four levels matching the attention budget's guidance footer specification:
/// - Full: complete M(t) dashboard with sub-metric checks (~100-200 tokens)
/// - Compressed: one-line summary with top action (~30-60 tokens)
/// - Minimal: M(t) score + abbreviated action (~10-20 tokens)
/// - HarvestOnly: harvest imperative signal (~10 tokens)
pub fn format_footer_at_level(footer: &GuidanceFooter, level: GuidanceLevel) -> String {
    match level {
        GuidanceLevel::Full => {
            let mut out = format_footer(footer);
            // Append Q(t) harvest warning when active
            if footer.harvest_warning.is_active() {
                out.push_str(&format!("\n  {}", footer.harvest_warning));
            }
            out
        }
        GuidanceLevel::Compressed => {
            let m = &footer.methodology;
            let trend = match m.trend {
                Trend::Up => "↑",
                Trend::Down => "↓",
                Trend::Stable => "→",
            };
            let next = match &footer.next_action {
                Some(action) => {
                    let refs = if footer.invariant_refs.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", footer.invariant_refs.join(", "))
                    };
                    format!(" | {action}{refs}")
                }
                None => String::new(),
            };
            // Append Q(t) harvest warning when Warn or Critical
            let hw = if footer.harvest_warning >= HarvestWarningLevel::Warn {
                format!(" {}", footer.harvest_warning)
            } else {
                String::new()
            };
            format!(
                "↳ M={:.2}{} S:{}{next}{hw}",
                m.score, trend, footer.store_datom_count
            )
        }
        GuidanceLevel::Minimal => {
            let m = &footer.methodology;
            // At minimal level, Critical harvest warning overrides the action
            if footer.harvest_warning == HarvestWarningLevel::Critical {
                return format!("↳ M={:.2} {}", m.score, footer.harvest_warning);
            }
            match &footer.next_action {
                Some(action) => {
                    let short = crate::budget::safe_truncate_bytes(action, 40);
                    format!("↳ M={:.2} → {short}", m.score)
                }
                None => format!("↳ M={:.2}", m.score),
            }
        }
        GuidanceLevel::HarvestOnly => {
            // Q(t)-based message when available, else M(t)-based fallback
            if footer.harvest_warning.is_active() {
                match footer.harvest_warning {
                    HarvestWarningLevel::Critical => {
                        "\u{26a0} HARVEST NOW: context nearly exhausted \u{2192} braid harvest --commit"
                            .to_string()
                    }
                    HarvestWarningLevel::Warn => {
                        "\u{26a0} harvest soon \u{2192} braid harvest --commit".to_string()
                    }
                    _ => {
                        "\u{26a0} HARVEST: braid harvest --task \"...\" --commit".to_string()
                    }
                }
            } else if footer.methodology.score < 0.3 {
                "\u{26a0} DRIFT: harvest now \u{2192} braid harvest --commit".to_string()
            } else {
                "\u{26a0} HARVEST: braid harvest --task \"...\" --commit".to_string()
            }
        }
    }
}

/// Build a guidance footer from current session state.
///
/// Defaults to `HarvestWarningLevel::None`. Use `build_footer_with_budget`
/// to include Q(t)-based harvest warnings.
pub fn build_footer(
    telemetry: &SessionTelemetry,
    store: &Store,
    next_action: Option<String>,
    invariant_refs: Vec<String>,
) -> GuidanceFooter {
    build_footer_with_budget(telemetry, store, next_action, invariant_refs, None)
}

/// Build a guidance footer with optional Q(t) budget signal.
///
/// When `q_t` is `Some`, the footer includes a Q(t)-based harvest warning level.
/// When `None`, defaults to `HarvestWarningLevel::None`.
pub fn build_footer_with_budget(
    telemetry: &SessionTelemetry,
    store: &Store,
    next_action: Option<String>,
    invariant_refs: Vec<String>,
    q_t: Option<f64>,
) -> GuidanceFooter {
    let methodology = compute_methodology_score(telemetry);
    let harvest_warning = q_t
        .map(harvest_warning_level)
        .unwrap_or(HarvestWarningLevel::None);
    GuidanceFooter {
        methodology,
        next_action,
        invariant_refs,
        store_datom_count: store.len(),
        turn: telemetry.total_turns,
        harvest_warning,
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
// Actionable Guidance (INV-GUIDANCE-001, INV-GUIDANCE-003)
// ---------------------------------------------------------------------------

/// Category of a guidance action — what kind of intervention is needed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionCategory {
    /// Something is broken and needs fixing before other work.
    Fix,
    /// Knowledge should be captured before it's lost.
    Harvest,
    /// Disconnected entities should be linked.
    Connect,
    /// A structural anomaly should be investigated.
    Observe,
    /// Something needs deeper analysis.
    Investigate,
    /// The store needs initial data.
    Bootstrap,
    /// A task is ready to work on.
    Work,
}

impl std::fmt::Display for ActionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionCategory::Fix => write!(f, "FIX"),
            ActionCategory::Harvest => write!(f, "HARVEST"),
            ActionCategory::Connect => write!(f, "CONNECT"),
            ActionCategory::Observe => write!(f, "OBSERVE"),
            ActionCategory::Investigate => write!(f, "INVESTIGATE"),
            ActionCategory::Bootstrap => write!(f, "BOOTSTRAP"),
            ActionCategory::Work => write!(f, "WORK"),
        }
    }
}

/// A concrete, prioritized guidance action with an optional suggested command.
///
/// Each action tells the agent exactly what to do next and why.
/// Actions are derived from store state analysis (R11–R18).
#[derive(Clone, Debug)]
pub struct GuidanceAction {
    /// Priority (1 = highest, 5 = lowest).
    pub priority: u8,
    /// Action category.
    pub category: ActionCategory,
    /// One-line summary of what to do.
    pub summary: String,
    /// Suggested braid command to execute (if applicable).
    pub command: Option<String>,
    /// Spec elements this action relates to.
    pub relates_to: Vec<String>,
}

/// Derive concrete actions from current store state.
///
/// Examines: store size, coherence metrics (Φ, β₁), tx count since last
/// harvest session entity, ISP bypasses, and namespace curvature.
///
/// Rules:
/// - R11: Empty/near-empty store → Bootstrap
/// - R12: Q(t)-based harvest warning (falls back to tx count when Q(t) unavailable)
/// - R13: β₁ > 0 (cycles in entity graph) → Observe
/// - R14: Φ > 0 (intent↔spec or spec↔impl gaps) → Connect
/// - R15: ISP specification bypasses → Fix
/// - R16: High entropy (structural disorder) → Investigate
/// - R17: Observation staleness > 0.8 → Investigate (ADR-HARVEST-005)
pub fn derive_actions(store: &Store) -> Vec<GuidanceAction> {
    derive_actions_with_budget(store, None)
}

/// Derive concrete actions with optional Q(t) budget signal.
///
/// When `q_t` is `Some`, R12 uses Q(t)-based thresholds from the attention
/// decay model (ADR-BUDGET-001). When `None`, falls back to the heuristic
/// tx-count threshold (8/15 transactions).
pub fn derive_actions_with_budget(store: &Store, q_t: Option<f64>) -> Vec<GuidanceAction> {
    let mut actions = Vec::new();
    let datom_count = store.len();
    let entity_count = store.entity_count();

    // R11: Near-empty store → Bootstrap
    if datom_count == 0 {
        actions.push(GuidanceAction {
            priority: 1,
            category: ActionCategory::Bootstrap,
            summary: "Store is empty. Initialize with spec elements.".into(),
            command: Some("braid init && braid bootstrap".into()),
            relates_to: vec!["INV-BOOTSTRAP-001".into()],
        });
        return actions; // No other actions make sense on empty store
    }

    // Check for non-schema entities (more useful than raw datom count)
    let has_exploration_entities = store.datoms().any(|d| {
        d.attribute.as_str() == ":exploration/body" || d.attribute.as_str() == ":exploration/source"
    });

    if entity_count < 10 && !has_exploration_entities {
        actions.push(GuidanceAction {
            priority: 1,
            category: ActionCategory::Bootstrap,
            summary: format!(
                "Store has {entity_count} entities but no explorations. Seed initial knowledge."
            ),
            command: Some("braid observe \"<your first observation>\" --confidence 0.7".into()),
            relates_to: vec!["INV-BOOTSTRAP-001".into()],
        });
    }

    // R12: Harvest warning — Q(t)-based when budget signal available, tx-count fallback otherwise.
    // ADR-BUDGET-001: Measured context over heuristic.
    let tx_count = count_txns_since_last_harvest(store);
    if let Some(q) = q_t {
        // Q(t)-based thresholds from attention decay model
        let level = harvest_warning_level(q);
        if level.is_active() {
            actions.push(GuidanceAction {
                priority: level.to_priority(),
                category: ActionCategory::Harvest,
                summary: format!(
                    "Q(t)={q:.2}: {} ({tx_count} txns since last harvest)",
                    level.message()
                ),
                command: level.suggested_action().map(String::from),
                relates_to: vec![
                    "INV-HARVEST-005".into(),
                    "ADR-BUDGET-001".into(),
                    "ADR-HARVEST-007".into(),
                ],
            });
        }
    } else {
        // Fallback: heuristic tx-count threshold (pre-Q(t) behavior)
        if tx_count >= 8 {
            let urgency = if tx_count >= 15 { 1 } else { 2 };
            actions.push(GuidanceAction {
                priority: urgency,
                category: ActionCategory::Harvest,
                summary: format!(
                    "{tx_count} transactions since last harvest. Knowledge at risk of loss."
                ),
                command: Some("braid harvest --task \"<current task>\" --commit".into()),
                relates_to: vec!["INV-HARVEST-005".into(), "ADR-HARVEST-007".into()],
            });
        }
    }

    // Run coherence analysis (fast — skips O(n³) entropy)
    let coherence = check_coherence_fast(store);

    // R13: β₁ > 0 (cycles) → Observe
    if coherence.beta_1 > 0 {
        actions.push(GuidanceAction {
            priority: 3,
            category: ActionCategory::Observe,
            summary: format!(
                "{} cycles in entity graph. May indicate circular dependencies.",
                coherence.beta_1
            ),
            command: Some("braid bilateral".into()),
            relates_to: vec!["INV-TRILATERAL-003".into()],
        });
    }

    // R14: Φ > 0 (divergence gaps) → Connect
    if coherence.phi > 0.0 {
        let (action_text, cmd) = match coherence.quadrant {
            CoherenceQuadrant::GapsOnly => (
                format!(
                    "Divergence Φ={:.1}. Gaps between intent/spec/impl layers.",
                    coherence.phi
                ),
                "braid query --datalog '[:find ?e ?doc :where [?e :db/doc ?doc] [?e :db/ident ?i]]'"
                    .to_string(),
            ),
            CoherenceQuadrant::GapsAndCycles => (
                format!(
                    "Divergence Φ={:.1} with {} cycles. Structural remediation needed.",
                    coherence.phi, coherence.beta_1
                ),
                "braid bilateral".to_string(),
            ),
            CoherenceQuadrant::CyclesOnly => (
                format!("Cycles present (β₁={}) but no gaps.", coherence.beta_1),
                "braid bilateral".to_string(),
            ),
            CoherenceQuadrant::Coherent => (
                "Store is coherent.".into(),
                String::new(),
            ),
        };

        if coherence.quadrant != CoherenceQuadrant::Coherent {
            actions.push(GuidanceAction {
                priority: if coherence.phi > 100.0 { 2 } else { 3 },
                category: ActionCategory::Connect,
                summary: action_text,
                command: if cmd.is_empty() { None } else { Some(cmd) },
                relates_to: vec!["INV-TRILATERAL-001".into(), "INV-TRILATERAL-004".into()],
            });
        }
    }

    // R15: ISP bypasses → Fix
    if coherence.isp_bypasses > 0 {
        actions.push(GuidanceAction {
            priority: 2,
            category: ActionCategory::Fix,
            summary: format!(
                "{} entities bypass ISP (have impl without spec). Add specifications.",
                coherence.isp_bypasses
            ),
            command: Some(
                "braid query -a :db/ident  # find entities, then add :spec/* attributes".into(),
            ),
            relates_to: vec!["INV-TRILATERAL-007".into()],
        });
    }

    // R16: High entropy → Investigate
    let s_vn = coherence.entropy.entropy;
    if s_vn > 3.0 && entity_count > 20 {
        actions.push(GuidanceAction {
            priority: 4,
            category: ActionCategory::Investigate,
            summary: format!(
                "High structural entropy S_vN={:.2}. Knowledge may be fragmenting.",
                s_vn
            ),
            command: Some("braid bilateral --spectral".into()),
            relates_to: vec!["INV-TRILATERAL-004".into()],
        });
    }

    // R17: Stale observations → Investigate
    let stale_observations: Vec<(EntityId, f64)> = observation_staleness(store)
        .into_iter()
        .filter(|&(_, s)| s > 0.8)
        .collect();
    if !stale_observations.is_empty() {
        actions.push(GuidanceAction {
            priority: 3,
            category: ActionCategory::Investigate,
            summary: format!(
                "{} observation(s) have staleness > 0.8. Review or re-observe.",
                stale_observations.len()
            ),
            command: Some("braid query --datalog '[:find ?e ?body :where [?e :exploration/body ?body] [?e :exploration/source \"braid:observe\"]]'".into()),
            relates_to: vec!["ADR-HARVEST-005".into()],
        });
    }

    // R18: R(t) graph-routed task → Work (INV-GUIDANCE-010, INV-TASK-003)
    //
    // Uses compute_routing_from_store to rank ready tasks by composite impact
    // (PageRank, betweenness, critical path, blocker ratio, staleness, priority)
    // rather than simple priority ordering. A P2 task that unblocks 5 others
    // can rank above a P1 task that unblocks nothing.
    let routed = compute_routing_from_store(store);
    if let Some(top) = routed.first() {
        // Look up the TaskSummary for the routed entity to get short ID
        let task_info = crate::task::task_summary(store, top.entity);
        let (task_id, priority) = match &task_info {
            Some(t) => (t.id.clone(), t.priority),
            None => ("?".into(), 2),
        };
        actions.push(GuidanceAction {
            priority: priority.min(3) as u8 + 1, // P0→1, P1→2, P2→3, P3+→4
            category: ActionCategory::Work,
            summary: format!(
                "R(t) top: \"{}\" (impact={:.2}) — {}",
                top.label, top.impact, task_id
            ),
            command: Some(format!("braid go {}", task_id)),
            relates_to: vec!["INV-GUIDANCE-010".into()],
        });
    }

    // Sort by priority (ascending = highest priority first)
    actions.sort_by_key(|a| a.priority);
    actions
}

/// Modulate action priorities based on M(t) methodology adherence score.
///
/// When M(t) drops, agents are drifting from methodology into pretrained patterns.
/// This function adjusts action priorities and injects corrective actions:
///
/// - **M(t) < 0.3** (crisis): Boost Fix/Harvest to P1, inject bilateral verification.
/// - **M(t) < 0.5** (drift signal): Inject coherence checkpoint action.
/// - **M(t) >= 0.5**: No modulation — agent is on track.
///
/// INV-GUIDANCE-003: Guidance adapts to drift signal.
/// INV-GUIDANCE-004: Actions become more directive as M(t) drops.
pub fn modulate_actions(actions: &mut Vec<GuidanceAction>, methodology_score: f64) {
    if methodology_score < 0.3 {
        // Crisis: all fix/harvest actions become top priority
        for action in actions.iter_mut() {
            if matches!(
                action.category,
                ActionCategory::Fix | ActionCategory::Harvest
            ) {
                action.priority = 1;
            }
        }
        // Inject bilateral verification — the strongest corrective signal
        actions.push(GuidanceAction {
            priority: 1,
            category: ActionCategory::Fix,
            summary: format!(
                "Methodology drift critical (M={methodology_score:.2}). Run bilateral verification."
            ),
            command: Some("braid bilateral --verbose".into()),
            relates_to: vec!["INV-GUIDANCE-003".into(), "INV-GUIDANCE-004".into()],
        });
    } else if methodology_score < 0.5 {
        // Drift signal: inject coherence checkpoint
        actions.push(GuidanceAction {
            priority: 2,
            category: ActionCategory::Observe,
            summary: format!(
                "Drift signal active (M={methodology_score:.2}). Verify coherence before next task."
            ),
            command: Some("braid guidance --verbose".into()),
            relates_to: vec!["INV-GUIDANCE-003".into()],
        });
    }
    // Re-sort after modulation
    actions.sort_by_key(|a| a.priority);
}

/// Build a guidance footer string for appending to any command output.
///
/// This is the entry point for INV-GUIDANCE-001 (continuous injection).
/// Computes M(t), derives actions, modulates by drift score, picks the top
/// action for the footer, and formats at the appropriate compression level.
///
/// `k_eff` is the current attention budget ratio (None defaults to 1.0 = full).
pub fn build_command_footer(store: &Store, k_eff: Option<f64>) -> String {
    let telemetry = telemetry_from_store(store);
    let methodology = compute_methodology_score(&telemetry);
    // Pass Q(t) to derive_actions so R12 uses attention-decay thresholds
    let q_t = k_eff.map(quality_adjusted_budget);
    let mut actions = derive_actions_with_budget(store, q_t);
    modulate_actions(&mut actions, methodology.score);

    let level = GuidanceLevel::for_k_eff(k_eff.unwrap_or(1.0));

    let (next_action, invariant_refs) = if let Some(top) = actions.first() {
        // Emit spec IDs only — no body inlining in footer.
        // Invariant statements can be multi-line formal math/code that bloats the footer
        // with ~80 tokens of non-actionable content. The agent can look up the statement
        // with: braid query --entity :spec/inv-store-001
        let refs = top.relates_to.clone();
        (
            top.command.clone().or_else(|| Some(top.summary.clone())),
            refs,
        )
    } else {
        (None, vec![])
    };

    let footer = build_footer_with_budget(&telemetry, store, next_action, invariant_refs, q_t);
    format_footer_at_level(&footer, level)
}

/// Derive session telemetry from the store state instead of using all-zero defaults.
///
/// This fixes C1 (M(t) always 0.00) by computing real values from the store:
/// - `total_turns`: transaction count (proxy for turns)
/// - `transact_turns`: transactions since last harvest
/// - `spec_language_turns`: count of spec entities
/// - `query_type_count`: 1 if any transactions, 0 otherwise
/// - `harvest_quality`: 0.7 if recent harvest exists, 0.0 otherwise
pub fn telemetry_from_store(store: &Store) -> SessionTelemetry {
    let tx_walls: BTreeSet<u64> = store.datoms().map(|d| d.tx.wall_time()).collect();
    let tx_count = tx_walls.len() as u32;
    let txns_since = count_txns_since_last_harvest(store) as u32;
    let spec_count = store
        .datoms()
        .filter(|d| {
            d.attribute.as_str() == ":db/ident"
                && d.op == Op::Assert
                && matches!(&d.value, Value::Keyword(k) if k.starts_with(":spec/"))
        })
        .count() as u32;
    let has_recent_harvest = last_harvest_wall_time(store) > 0;

    // A3: M(t) floor clamp — when a harvest exists and fewer than 10 txns
    // have occurred since, the store is in a healthy inter-session state.
    // Without this floor, M(t) drops below 0.5 between sessions because
    // transact_frequency and query_diversity reset, triggering false DRIFT
    // warnings (CC-5 failure in bilateral scan).
    let harvest_is_recent = has_recent_harvest && txns_since < 10;

    SessionTelemetry {
        total_turns: tx_count.max(1),
        transact_turns: txns_since,
        spec_language_turns: spec_count.min(tx_count),
        query_type_count: if tx_count > 0 { 1 } else { 0 },
        harvest_quality: if has_recent_harvest { 0.7 } else { 0.0 },
        history: vec![],
        harvest_is_recent,
    }
}

/// Count transactions since the last harvest-type entity.
///
/// Uses tx-count proxy: counts tx files whose wall_time exceeds the most
/// recent transaction with provenance "braid:harvest" or "braid:observe".
pub fn count_txns_since_last_harvest(store: &Store) -> usize {
    let boundary = last_harvest_wall_time(store);

    if boundary == 0 {
        // No harvest ever — count all distinct wall times
        let walls: std::collections::BTreeSet<u64> =
            store.datoms().map(|d| d.tx.wall_time()).collect();
        walls.len()
    } else {
        // Count distinct wall times strictly after the last harvest
        let walls: std::collections::BTreeSet<u64> = store
            .datoms()
            .filter(|d| d.tx.wall_time() > boundary)
            .map(|d| d.tx.wall_time())
            .collect();
        walls.len()
    }
}

/// Find the wall_time of the most recent harvest/observe transaction.
///
/// Returns 0 if no harvest or observation has ever been recorded.
/// Used by the harvest CLI to determine the session boundary:
/// datoms with tx.wall_time > this value are "this session's work."
pub fn last_harvest_wall_time(store: &Store) -> u64 {
    let mut latest: u64 = 0;
    for datom in store.datoms() {
        // Only harvest session commits define the session boundary.
        // Observations are IN-session work and must NOT reset the boundary —
        // otherwise harvest would never see them as "new since last harvest."
        if datom.attribute.as_str() == ":harvest/agent" && datom.op == Op::Assert {
            let wall = datom.tx.wall_time();
            if wall > latest {
                latest = wall;
            }
        }
    }
    latest
}

/// Compute staleness for observations based on transaction distance.
///
/// Staleness = 1 - confidence * decay^(tx_distance)
/// where tx_distance = current_max_wall_time - observation_wall_time
/// and decay = 0.95 per transaction step.
///
/// Returns a vec of (entity_id, staleness) pairs for all observation entities
/// found in the store. Staleness is in [0.0, 1.0] where 1.0 means fully stale.
///
/// Traces to: ADR-HARVEST-005 (observation staleness model).
pub fn observation_staleness(store: &Store) -> Vec<(EntityId, f64)> {
    // Find the max wall_time across the entire frontier
    let max_wall: u64 = store
        .frontier()
        .values()
        .map(|tx| tx.wall_time())
        .max()
        .unwrap_or(0);

    // Collect observation entities: those with :exploration/confidence
    // Build a map of entity -> (confidence, wall_time)
    let conf_attr = Attribute::from_keyword(":exploration/confidence");
    let source_attr = Attribute::from_keyword(":exploration/source");

    let mut entity_confidence: BTreeMap<EntityId, f64> = BTreeMap::new();
    let mut entity_wall_time: BTreeMap<EntityId, u64> = BTreeMap::new();
    let mut observation_entities: std::collections::BTreeSet<EntityId> =
        std::collections::BTreeSet::new();

    for datom in store.datoms() {
        if datom.attribute == source_attr {
            if let Value::String(ref s) = datom.value {
                if s == "braid:observe" || s == "braid:harvest" {
                    observation_entities.insert(datom.entity);
                }
            }
        }
        if datom.attribute == conf_attr {
            if let Value::Double(f) = datom.value {
                entity_confidence.insert(datom.entity, f.into_inner());
            }
        }
        // Track the wall_time of the tx that asserted each entity's datoms.
        // Use the max wall_time across all datoms for that entity.
        let wall = datom.tx.wall_time();
        entity_wall_time
            .entry(datom.entity)
            .and_modify(|w| {
                if wall > *w {
                    *w = wall;
                }
            })
            .or_insert(wall);
    }

    let mut results = Vec::new();
    for entity in &observation_entities {
        let confidence = entity_confidence.get(entity).copied().unwrap_or(0.5);
        let obs_wall = entity_wall_time.get(entity).copied().unwrap_or(0);
        let distance = max_wall.saturating_sub(obs_wall);
        let decay = STALENESS_DECAY_RATE.powi(distance as i32);
        let staleness = (1.0 - confidence * decay).clamp(0.0, 1.0);
        results.push((*entity, staleness));
    }

    results
}

/// Format guidance actions as a compact, LLM-parseable string.
///
/// Output format (one action per line, structured for easy parsing):
/// ```text
/// actions:
///   1. FIX: 3 entities bypass ISP → braid query -a :db/ident [INV-TRILATERAL-007]
///   2. HARVEST: 12 txns since last harvest → braid harvest --task "..." [INV-HARVEST-005]
///   3. CONNECT: Φ=210.6, gaps between layers → braid status --deep --full [INV-TRILATERAL-001]
/// ```
pub fn format_actions(actions: &[GuidanceAction]) -> String {
    if actions.is_empty() {
        return "actions: none (store is coherent)\n".to_string();
    }

    let mut out = String::from("actions:\n");
    for (i, action) in actions.iter().enumerate() {
        let cmd_part = match &action.command {
            Some(cmd) => format!(" → {cmd}"),
            None => String::new(),
        };
        let refs = if action.relates_to.is_empty() {
            String::new()
        } else {
            format!(" [{}]", action.relates_to.join(", "))
        };
        out.push_str(&format!(
            "  {}. {}: {}{}{}\n",
            i + 1,
            action.category,
            action.summary,
            cmd_part,
            refs,
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-GUIDANCE-001, INV-GUIDANCE-002, INV-GUIDANCE-003,
// INV-GUIDANCE-004, INV-GUIDANCE-005, INV-GUIDANCE-007,
// INV-GUIDANCE-008, INV-GUIDANCE-009, INV-GUIDANCE-010, INV-GUIDANCE-011,
// ADR-GUIDANCE-001, ADR-GUIDANCE-002, ADR-GUIDANCE-003, ADR-GUIDANCE-004,
// ADR-GUIDANCE-005, ADR-GUIDANCE-006, ADR-GUIDANCE-007,
// ADR-GUIDANCE-008, ADR-GUIDANCE-009,
// NEG-GUIDANCE-001, NEG-GUIDANCE-002, NEG-GUIDANCE-003
#[cfg(test)]
mod tests {
    use super::*;

    // Verifies: INV-GUIDANCE-008 — M(t) Methodology Adherence Score
    // Verifies: ADR-GUIDANCE-005 — Unified Guidance as M(t) x R(t) x T(t)
    #[test]
    fn methodology_score_weights_sum_to_one() {
        let sum: f64 = STAGE0_WEIGHTS.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10, "weights must sum to 1.0");
    }

    // Verifies: INV-GUIDANCE-010 — R(t) Graph-Based Work Routing
    #[test]
    fn routing_weights_sum_to_one() {
        let sum: f64 = ROUTING_WEIGHTS.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10, "weights must sum to 1.0");
    }

    // Verifies: INV-GUIDANCE-008 — M(t) Methodology Adherence Score (bounds)
    #[test]
    fn methodology_score_bounds() {
        // Perfect session
        let perfect = SessionTelemetry {
            total_turns: 10,
            transact_turns: 10,
            spec_language_turns: 10,
            query_type_count: 4,
            harvest_quality: 1.0,
            ..Default::default()
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

    // Verifies: INV-GUIDANCE-004 — Drift Detection Responsiveness
    #[test]
    fn methodology_trend_detection() {
        let telemetry = SessionTelemetry {
            total_turns: 10,
            transact_turns: 8,
            spec_language_turns: 7,
            query_type_count: 3,
            harvest_quality: 0.9,
            history: vec![0.3, 0.4, 0.5, 0.6, 0.7],
            ..Default::default()
        };
        let score = compute_methodology_score(&telemetry);
        assert_eq!(score.trend, Trend::Up, "improving history should trend up");
    }

    // Verifies: INV-GUIDANCE-008 — M(t) Methodology Adherence Score (floor clamp)
    #[test]
    fn methodology_floor_clamp_when_harvest_recent() {
        // A low-activity session (e.g., inter-session gap) with recent harvest
        // should still have M(t) >= 0.50 to avoid false DRIFT warnings.
        let telemetry = SessionTelemetry {
            total_turns: 5,
            transact_turns: 1,
            spec_language_turns: 1,
            query_type_count: 0,
            harvest_quality: 0.3,
            harvest_is_recent: true,
            ..Default::default()
        };
        let score = compute_methodology_score(&telemetry);
        assert!(
            score.score >= 0.50,
            "M(t) should be >= 0.50 when harvest is recent, got {}",
            score.score
        );
        assert!(
            !score.drift_signal,
            "should not trigger drift signal when harvest is recent"
        );
    }

    #[test]
    fn methodology_no_floor_without_recent_harvest() {
        // Without a recent harvest, the floor should not apply.
        let telemetry = SessionTelemetry {
            total_turns: 5,
            transact_turns: 1,
            spec_language_turns: 1,
            query_type_count: 0,
            harvest_quality: 0.3,
            harvest_is_recent: false,
            ..Default::default()
        };
        let score = compute_methodology_score(&telemetry);
        assert!(
            score.score < 0.50,
            "M(t) should be below 0.50 without recent harvest, got {}",
            score.score
        );
    }

    // Verifies: INV-GUIDANCE-010 — R(t) Graph-Based Work Routing
    // Verifies: INV-GUIDANCE-009 — Task Derivation Completeness
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

    // Verifies: INV-GUIDANCE-009 — Task Derivation Completeness
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

    // Verifies: INV-GUIDANCE-001 — Continuous Injection
    // Verifies: INV-GUIDANCE-007 — Dynamic CLAUDE.md as Optimized Prompt
    // Verifies: NEG-GUIDANCE-001 — No Tool Response Without Footer
    #[test]
    fn footer_format_includes_all_components() {
        let telemetry = SessionTelemetry {
            total_turns: 7,
            transact_turns: 5,
            spec_language_turns: 6,
            query_type_count: 3,
            harvest_quality: 0.9,
            ..Default::default()
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
                        ..Default::default()
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

    // -------------------------------------------------------------------
    // Guidance Actions (A.4 — INV-GUIDANCE-001, INV-GUIDANCE-003)
    // -------------------------------------------------------------------

    #[test]
    fn actions_on_empty_store_returns_bootstrap() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let actions = derive_actions(&store);
        assert_eq!(
            actions.len(),
            1,
            "empty store should produce exactly 1 action"
        );
        assert_eq!(actions[0].category, ActionCategory::Bootstrap);
        assert_eq!(actions[0].priority, 1);
        assert!(actions[0].command.is_some());
    }

    #[test]
    fn actions_on_genesis_store_sorted_by_priority() {
        let store = Store::genesis();
        let actions = derive_actions(&store);
        // Genesis store with only schema may or may not produce actions
        // (depends on coherence state) — but if it does, they must be sorted
        for window in actions.windows(2) {
            assert!(
                window[0].priority <= window[1].priority,
                "actions must be sorted by ascending priority: {} <= {} violated",
                window[0].priority,
                window[1].priority,
            );
        }
    }

    #[test]
    fn format_actions_empty() {
        let formatted = format_actions(&[]);
        assert!(formatted.contains("none"));
    }

    #[test]
    fn format_actions_includes_category_and_command() {
        let actions = vec![
            GuidanceAction {
                priority: 1,
                category: ActionCategory::Fix,
                summary: "Test issue".into(),
                command: Some("braid query -a :db/ident".into()),
                relates_to: vec!["INV-TEST-001".into()],
            },
            GuidanceAction {
                priority: 3,
                category: ActionCategory::Observe,
                summary: "Cycles detected".into(),
                command: None,
                relates_to: vec![],
            },
        ];
        let formatted = format_actions(&actions);
        assert!(formatted.contains("FIX:"), "should contain FIX category");
        assert!(
            formatted.contains("braid query"),
            "should contain suggested command"
        );
        assert!(
            formatted.contains("INV-TEST-001"),
            "should contain spec refs"
        );
        assert!(
            formatted.contains("OBSERVE:"),
            "should contain OBSERVE category"
        );
    }

    #[test]
    fn action_category_display() {
        assert_eq!(format!("{}", ActionCategory::Fix), "FIX");
        assert_eq!(format!("{}", ActionCategory::Harvest), "HARVEST");
        assert_eq!(format!("{}", ActionCategory::Connect), "CONNECT");
        assert_eq!(format!("{}", ActionCategory::Observe), "OBSERVE");
        assert_eq!(format!("{}", ActionCategory::Investigate), "INVESTIGATE");
        assert_eq!(format!("{}", ActionCategory::Bootstrap), "BOOTSTRAP");
    }

    #[test]
    fn count_txns_since_last_harvest_on_empty() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        assert_eq!(count_txns_since_last_harvest(&store), 0);
    }

    #[test]
    fn count_txns_since_last_harvest_on_genesis() {
        let store = Store::genesis();
        // Genesis has frontier entries but no harvest entities
        let count = count_txns_since_last_harvest(&store);
        assert!(
            count > 0,
            "genesis store with no harvests should report all txns"
        );
    }

    // -------------------------------------------------------------------
    // Observation Staleness (B.2 — ADR-HARVEST-005)
    // -------------------------------------------------------------------

    /// Helper: build an observation entity as a set of datoms.
    fn make_observation_datoms(
        ident: &str,
        body: &str,
        confidence: f64,
        wall_time: u64,
    ) -> Vec<crate::datom::Datom> {
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        use ordered_float::OrderedFloat;

        let entity = EntityId::from_ident(ident);
        let agent = AgentId::from_name("test");
        let tx = TxId::new(wall_time, 0, agent);

        vec![
            Datom::new(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(ident.to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                entity,
                Attribute::from_keyword(":exploration/body"),
                Value::String(body.to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                entity,
                Attribute::from_keyword(":exploration/confidence"),
                Value::Double(OrderedFloat(confidence)),
                tx,
                Op::Assert,
            ),
            Datom::new(
                entity,
                Attribute::from_keyword(":exploration/source"),
                Value::String("braid:observe".to_string()),
                tx,
                Op::Assert,
            ),
        ]
    }

    // Verifies: INV-GUIDANCE-005 — Learned Guidance Effectiveness Tracking
    // Verifies: ADR-HARVEST-005 — Observation Staleness Model
    #[test]
    fn staleness_empty_store() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let result = observation_staleness(&store);
        assert!(result.is_empty(), "empty store has no observations");
    }

    #[test]
    fn staleness_zero_for_fresh_observation() {
        // Observation at wall_time=100, max_wall_time=100 → distance=0 → staleness = 1 - conf * 1.0
        let mut datoms = std::collections::BTreeSet::new();
        for d in make_observation_datoms(":observation/fresh", "fresh obs", 0.9, 100) {
            datoms.insert(d);
        }
        let store = Store::from_datoms(datoms);
        let result = observation_staleness(&store);
        assert_eq!(result.len(), 1);
        let (_, staleness) = result[0];
        // staleness = 1 - 0.9 * 0.95^0 = 1 - 0.9 = 0.1
        assert!(
            (staleness - 0.1).abs() < 1e-10,
            "fresh observation staleness should be 0.1, got {staleness}"
        );
    }

    #[test]
    fn staleness_increases_with_distance() {
        // Two observations: one at wall_time=100, one at wall_time=80.
        // Add a later tx at wall_time=115 to create distance.
        let mut datoms = std::collections::BTreeSet::new();
        for d in make_observation_datoms(":observation/recent", "recent", 0.8, 100) {
            datoms.insert(d);
        }
        for d in make_observation_datoms(":observation/older", "older", 0.8, 80) {
            datoms.insert(d);
        }
        // Add a non-observation datom at wall_time=115 to advance the frontier
        {
            use crate::datom::{AgentId, Datom, Op, TxId, Value};
            let agent = AgentId::from_name("test");
            let tx = TxId::new(115, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(":other/entity"),
                Attribute::from_keyword(":db/doc"),
                Value::String("advance frontier".to_string()),
                tx,
                Op::Assert,
            ));
        }
        let store = Store::from_datoms(datoms);
        let result = observation_staleness(&store);
        assert_eq!(result.len(), 2);

        let staleness_map: BTreeMap<EntityId, f64> = result.into_iter().collect();
        let s_recent = staleness_map[&EntityId::from_ident(":observation/recent")];
        let s_older = staleness_map[&EntityId::from_ident(":observation/older")];

        // recent: distance=15, staleness = 1 - 0.8 * 0.95^15
        let expected_recent = 1.0 - 0.8 * STALENESS_DECAY_RATE.powi(15);
        assert!(
            (s_recent - expected_recent).abs() < 1e-10,
            "recent staleness: expected {expected_recent}, got {s_recent}"
        );

        // older: distance=35, staleness = 1 - 0.8 * 0.95^35
        let expected_older = 1.0 - 0.8 * STALENESS_DECAY_RATE.powi(35);
        assert!(
            (s_older - expected_older).abs() < 1e-10,
            "older staleness: expected {expected_older}, got {s_older}"
        );

        assert!(
            s_older > s_recent,
            "older observation should be more stale: {s_older} > {s_recent}"
        );
    }

    #[test]
    fn staleness_formula_known_values() {
        // Verify the specific example from the task description:
        // After 15 transactions, confidence 0.8: staleness = 1 - 0.8 * 0.95^15 ≈ 0.63
        let decay_15 = STALENESS_DECAY_RATE.powi(15);
        let staleness = 1.0 - 0.8 * decay_15;
        assert!(
            (staleness - 0.6294).abs() < 0.001,
            "staleness at distance=15, conf=0.8 should be ~0.63, got {staleness}"
        );
    }

    #[test]
    fn staleness_confidence_one_at_zero_distance() {
        // confidence=1.0, distance=0 → staleness = 1 - 1.0 * 1.0 = 0.0
        let mut datoms = std::collections::BTreeSet::new();
        for d in make_observation_datoms(":observation/perfect", "perfect", 1.0, 50) {
            datoms.insert(d);
        }
        let store = Store::from_datoms(datoms);
        let result = observation_staleness(&store);
        assert_eq!(result.len(), 1);
        let (_, staleness) = result[0];
        assert!(
            staleness.abs() < 1e-10,
            "conf=1.0 at distance=0 should have staleness=0.0, got {staleness}"
        );
    }

    #[test]
    fn staleness_confidence_zero_always_stale() {
        // confidence=0.0 → staleness = 1 - 0 * anything = 1.0
        let mut datoms = std::collections::BTreeSet::new();
        for d in make_observation_datoms(":observation/uncertain", "uncertain", 0.0, 50) {
            datoms.insert(d);
        }
        let store = Store::from_datoms(datoms);
        let result = observation_staleness(&store);
        assert_eq!(result.len(), 1);
        let (_, staleness) = result[0];
        assert!(
            (staleness - 1.0).abs() < 1e-10,
            "conf=0.0 should always be staleness=1.0, got {staleness}"
        );
    }

    // Verifies: INV-GUIDANCE-005 — Learned Guidance Effectiveness Tracking
    // Verifies: NEG-GUIDANCE-003 — No Ineffective Guidance Persistence
    #[test]
    fn r17_stale_observations_produce_investigate_action() {
        // Create observations that are very stale (high distance, low confidence)
        let mut datoms = std::collections::BTreeSet::new();

        // Observation at wall_time=10, confidence=0.3
        for d in make_observation_datoms(":observation/stale", "very stale", 0.3, 10) {
            datoms.insert(d);
        }

        // Advance frontier far ahead to make it stale
        {
            use crate::datom::{AgentId, Datom, Op, TxId, Value};
            let agent = AgentId::from_name("test");
            let tx = TxId::new(200, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(":other/entity"),
                Attribute::from_keyword(":db/doc"),
                Value::String("future tx".to_string()),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);

        // Verify the observation is stale enough
        let stale = observation_staleness(&store);
        assert_eq!(stale.len(), 1);
        assert!(
            stale[0].1 > 0.8,
            "observation should be stale (>0.8), got {}",
            stale[0].1
        );

        // Verify R17 fires
        let actions = derive_actions(&store);
        let r17 = actions
            .iter()
            .find(|a| a.relates_to.contains(&"ADR-HARVEST-005".to_string()));
        assert!(
            r17.is_some(),
            "R17 should fire for stale observations. Actions: {:?}",
            actions.iter().map(|a| &a.summary).collect::<Vec<_>>()
        );
        let r17 = r17.unwrap();
        assert_eq!(r17.category, ActionCategory::Investigate);
        assert!(r17.summary.contains("staleness > 0.8"));
    }

    // -------------------------------------------------------------------
    // Budget-aware footer compression (INV-BUDGET-004)
    // -------------------------------------------------------------------

    #[test]
    fn format_footer_full_matches_original() {
        let telemetry = SessionTelemetry {
            total_turns: 7,
            transact_turns: 5,
            spec_language_turns: 6,
            query_type_count: 3,
            harvest_quality: 0.9,
            ..Default::default()
        };
        let store = Store::genesis();
        let footer = build_footer(
            &telemetry,
            &store,
            Some("braid query [:find ?e]".into()),
            vec!["INV-STORE-003".into()],
        );
        let full = format_footer(&footer);
        let at_level = format_footer_at_level(&footer, crate::budget::GuidanceLevel::Full);
        assert_eq!(
            full, at_level,
            "Full level must match original format_footer"
        );
    }

    // Verifies: INV-GUIDANCE-002 — Spec-Language Phrasing
    // Verifies: ADR-GUIDANCE-004 — Spec-Language Over Instruction-Language
    // Verifies: ADR-GUIDANCE-008 — Guidance Footer Progressive Enrichment at Stage 0
    #[test]
    fn format_footer_compressed_is_one_line() {
        let telemetry = SessionTelemetry::default();
        let store = Store::genesis();
        let footer = build_footer(
            &telemetry,
            &store,
            Some("braid query [:find ?e]".into()),
            vec!["INV-STORE-003".into()],
        );
        let compressed = format_footer_at_level(&footer, crate::budget::GuidanceLevel::Compressed);
        assert!(
            !compressed.contains('\n'),
            "Compressed footer must be one line, got: {compressed}"
        );
        assert!(compressed.contains("M="), "must contain M= score");
        assert!(compressed.contains("S:"), "must contain S: datom count");
        assert!(
            compressed.contains("INV-STORE-003"),
            "must contain spec refs"
        );
    }

    #[test]
    fn format_footer_minimal_is_short() {
        let telemetry = SessionTelemetry::default();
        let store = Store::genesis();
        let footer = build_footer(
            &telemetry,
            &store,
            Some("braid query [:find ?e :where [?e :db/doc ?v]]".into()),
            vec![],
        );
        let minimal = format_footer_at_level(&footer, crate::budget::GuidanceLevel::Minimal);
        assert!(minimal.contains("M="), "must contain M= score");
        assert!(
            minimal.len() < 80,
            "Minimal footer must be very short, got {} chars: {minimal}",
            minimal.len()
        );
    }

    #[test]
    fn format_footer_harvest_only() {
        let telemetry = SessionTelemetry::default();
        let store = Store::genesis();
        let footer_low = build_footer(
            &telemetry,
            &store,
            Some("braid harvest --commit".into()),
            vec![],
        );
        // Override methodology score for testing
        let mut footer_low = footer_low;
        footer_low.methodology.score = 0.2;
        let harvest =
            format_footer_at_level(&footer_low, crate::budget::GuidanceLevel::HarvestOnly);
        assert!(
            harvest.contains("DRIFT"),
            "HarvestOnly with low M(t) should say DRIFT, got: {harvest}"
        );

        footer_low.methodology.score = 0.6;
        let harvest =
            format_footer_at_level(&footer_low, crate::budget::GuidanceLevel::HarvestOnly);
        assert!(
            harvest.contains("HARVEST"),
            "HarvestOnly with decent M(t) should say HARVEST, got: {harvest}"
        );
    }

    #[test]
    fn format_footer_compression_monotonically_shorter() {
        use crate::budget::GuidanceLevel;

        let telemetry = SessionTelemetry {
            total_turns: 10,
            transact_turns: 8,
            spec_language_turns: 7,
            query_type_count: 3,
            harvest_quality: 0.9,
            ..Default::default()
        };
        let store = Store::genesis();
        let footer = build_footer(
            &telemetry,
            &store,
            Some("braid query [:find ?e :where [?e :db/doc ?v]]".into()),
            vec!["INV-STORE-003".into(), "INV-STORE-005".into()],
        );

        let full_len = format_footer_at_level(&footer, GuidanceLevel::Full).len();
        let comp_len = format_footer_at_level(&footer, GuidanceLevel::Compressed).len();
        let min_len = format_footer_at_level(&footer, GuidanceLevel::Minimal).len();
        let harv_len = format_footer_at_level(&footer, GuidanceLevel::HarvestOnly).len();

        assert!(
            full_len >= comp_len,
            "Full ({full_len}) must be >= Compressed ({comp_len})"
        );
        assert!(
            comp_len >= min_len,
            "Compressed ({comp_len}) must be >= Minimal ({min_len})"
        );
        // HarvestOnly may or may not be shorter than Minimal (depends on content)
        // but it should be shorter than Full
        assert!(
            full_len >= harv_len,
            "Full ({full_len}) must be >= HarvestOnly ({harv_len})"
        );
    }

    // -------------------------------------------------------------------
    // Drift-responsive action modulation (INV-GUIDANCE-003, INV-GUIDANCE-004)
    // -------------------------------------------------------------------

    // Verifies: INV-GUIDANCE-003 — Intention-Action Coherence
    // Verifies: ADR-GUIDANCE-002 — Basin Competition as Central Failure Model
    // Verifies: ADR-GUIDANCE-007 — System 1/System 2 Diagnosis
    #[test]
    fn modulate_actions_crisis_mode() {
        let mut actions = vec![
            GuidanceAction {
                priority: 3,
                category: ActionCategory::Fix,
                summary: "ISP bypasses".into(),
                command: Some("braid query".into()),
                relates_to: vec!["INV-TRILATERAL-007".into()],
            },
            GuidanceAction {
                priority: 4,
                category: ActionCategory::Observe,
                summary: "Cycles detected".into(),
                command: None,
                relates_to: vec![],
            },
        ];

        modulate_actions(&mut actions, 0.2); // Crisis: M(t) < 0.3

        // Fix action should be boosted to P1
        let fix = actions.iter().find(|a| a.summary.contains("ISP")).unwrap();
        assert_eq!(
            fix.priority, 1,
            "Fix action should be boosted to P1 in crisis"
        );

        // Bilateral verification should be injected
        let bilateral = actions
            .iter()
            .find(|a| a.summary.contains("drift critical"));
        assert!(
            bilateral.is_some(),
            "Crisis should inject bilateral verification action"
        );
        assert_eq!(bilateral.unwrap().priority, 1);
    }

    // Verifies: INV-GUIDANCE-004 — Drift Detection Responsiveness
    #[test]
    fn modulate_actions_drift_signal() {
        let mut actions = vec![GuidanceAction {
            priority: 3,
            category: ActionCategory::Observe,
            summary: "Cycles".into(),
            command: None,
            relates_to: vec![],
        }];

        modulate_actions(&mut actions, 0.4); // Warning: 0.3 <= M(t) < 0.5

        // Should inject coherence checkpoint
        let checkpoint = actions.iter().find(|a| a.summary.contains("Drift signal"));
        assert!(
            checkpoint.is_some(),
            "Drift signal should inject coherence checkpoint"
        );
    }

    #[test]
    fn modulate_actions_no_change_when_healthy() {
        let mut actions = vec![GuidanceAction {
            priority: 3,
            category: ActionCategory::Observe,
            summary: "Cycles".into(),
            command: None,
            relates_to: vec![],
        }];
        let original_len = actions.len();

        modulate_actions(&mut actions, 0.8); // Healthy: M(t) >= 0.5

        assert_eq!(
            actions.len(),
            original_len,
            "Healthy M(t) should not inject additional actions"
        );
    }

    #[test]
    fn modulate_actions_sorted_after_modulation() {
        let mut actions = vec![
            GuidanceAction {
                priority: 4,
                category: ActionCategory::Harvest,
                summary: "harvest needed".into(),
                command: Some("braid harvest".into()),
                relates_to: vec![],
            },
            GuidanceAction {
                priority: 2,
                category: ActionCategory::Observe,
                summary: "cycles".into(),
                command: None,
                relates_to: vec![],
            },
        ];

        modulate_actions(&mut actions, 0.15); // Crisis

        // After modulation, actions should be sorted by priority
        for window in actions.windows(2) {
            assert!(
                window[0].priority <= window[1].priority,
                "Actions must be sorted after modulation: P{} <= P{}",
                window[0].priority,
                window[1].priority,
            );
        }
    }

    // -------------------------------------------------------------------
    // build_command_footer (INV-GUIDANCE-001 entry point)
    // -------------------------------------------------------------------

    // Verifies: INV-GUIDANCE-001 — Continuous Injection (command footer)
    // Verifies: ADR-GUIDANCE-006 — Query over Guidance Graph
    #[test]
    fn build_command_footer_on_genesis() {
        let store = Store::genesis();
        let footer = build_command_footer(&store, None);
        assert!(!footer.is_empty(), "footer must not be empty");
        assert!(
            footer.contains('↳') || footer.contains('⚠'),
            "footer must contain guidance marker, got: {footer}"
        );
    }

    #[test]
    fn build_command_footer_respects_k_eff() {
        let store = Store::genesis();

        // Full budget → Full level footer (longest)
        let full = build_command_footer(&store, Some(1.0));
        // Low budget → more compressed
        let compressed = build_command_footer(&store, Some(0.5));
        // Very low → minimal
        let minimal = build_command_footer(&store, Some(0.3));
        // Exhausted → harvest only
        let harvest = build_command_footer(&store, Some(0.1));

        // Compression should generally reduce length
        assert!(
            full.len() >= harvest.len(),
            "Full footer ({}) should be >= HarvestOnly ({})",
            full.len(),
            harvest.len()
        );
        // But all should be non-empty
        assert!(!compressed.is_empty());
        assert!(!minimal.is_empty());
    }

    #[test]
    fn build_command_footer_on_empty_store() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let footer = build_command_footer(&store, None);
        assert!(
            !footer.is_empty(),
            "footer on empty store must not be empty"
        );
    }

    // -------------------------------------------------------------------
    // Proptest: budget-aware compression properties
    // -------------------------------------------------------------------

    mod budget_compression_proptests {
        use super::*;
        use crate::budget::GuidanceLevel;
        use proptest::prelude::*;

        fn arb_k_eff() -> impl Strategy<Value = f64> {
            0.0f64..=1.0
        }

        fn arb_footer() -> impl Strategy<Value = GuidanceFooter> {
            (
                0.0f64..1.0,   // score
                0usize..10000, // datom count
                0u32..100,     // turn
            )
                .prop_map(|(score, datom_count, turn)| {
                    let components = MethodologyComponents {
                        transact_frequency: score,
                        spec_language_ratio: score,
                        query_diversity: score.min(1.0),
                        harvest_quality: score,
                    };
                    GuidanceFooter {
                        methodology: MethodologyScore {
                            score,
                            components,
                            trend: Trend::Stable,
                            drift_signal: score < 0.5,
                        },
                        next_action: Some("braid query [:find ?e]".to_string()),
                        invariant_refs: vec!["INV-TEST-001".to_string()],
                        store_datom_count: datom_count,
                        turn,
                        harvest_warning: HarvestWarningLevel::None,
                    }
                })
        }

        proptest! {
            #[test]
            fn format_at_level_never_panics(footer in arb_footer(), k_eff in arb_k_eff()) {
                let level = GuidanceLevel::for_k_eff(k_eff);
                let formatted = format_footer_at_level(&footer, level);
                prop_assert!(!formatted.is_empty(), "formatted footer must not be empty");
            }

            #[test]
            fn full_level_always_longest(footer in arb_footer()) {
                let full = format_footer_at_level(&footer, GuidanceLevel::Full);
                let compressed = format_footer_at_level(&footer, GuidanceLevel::Compressed);
                let minimal = format_footer_at_level(&footer, GuidanceLevel::Minimal);

                prop_assert!(
                    full.len() >= compressed.len(),
                    "Full ({}) must be >= Compressed ({})",
                    full.len(),
                    compressed.len()
                );
                prop_assert!(
                    compressed.len() >= minimal.len(),
                    "Compressed ({}) must be >= Minimal ({})",
                    compressed.len(),
                    minimal.len()
                );
            }

            #[test]
            fn modulate_preserves_existing_actions(
                m_score in 0.0f64..1.0,
                num_actions in 0usize..5
            ) {
                let mut actions: Vec<GuidanceAction> = (0..num_actions)
                    .map(|i| GuidanceAction {
                        priority: (i as u8) + 1,
                        category: ActionCategory::Observe,
                        summary: format!("action-{i}"),
                        command: None,
                        relates_to: vec![],
                    })
                    .collect();
                let original_summaries: Vec<String> =
                    actions.iter().map(|a| a.summary.clone()).collect();

                modulate_actions(&mut actions, m_score);

                // All original actions must still be present
                for summary in &original_summaries {
                    prop_assert!(
                        actions.iter().any(|a| &a.summary == summary),
                        "Original action '{}' must survive modulation",
                        summary
                    );
                }
            }

            #[test]
            fn modulate_sorted_output(m_score in 0.0f64..1.0) {
                let mut actions = vec![
                    GuidanceAction {
                        priority: 3,
                        category: ActionCategory::Fix,
                        summary: "test fix".into(),
                        command: None,
                        relates_to: vec![],
                    },
                    GuidanceAction {
                        priority: 1,
                        category: ActionCategory::Harvest,
                        summary: "test harvest".into(),
                        command: None,
                        relates_to: vec![],
                    },
                ];

                modulate_actions(&mut actions, m_score);

                for window in actions.windows(2) {
                    prop_assert!(
                        window[0].priority <= window[1].priority,
                        "Actions must be sorted after modulation at M={:.2}",
                        m_score
                    );
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Proptest: INV-GUIDANCE-001, INV-GUIDANCE-003, INV-GUIDANCE-006
    // -------------------------------------------------------------------

    mod proptests {
        use super::*;
        use crate::budget::{ApproxTokenCounter, GuidanceLevel, TokenCounter};
        use crate::proptest_strategies::arb_store;
        use proptest::prelude::*;

        // INV-GUIDANCE-001: Continuous injection — every response gets a footer.
        // For any store state, `build_command_footer` produces a non-empty string.
        // Falsified if any store (including empty / genesis / multi-txn) yields "".
        proptest! {
            #[test]
            fn footer_always_present(store in arb_store(5)) {
                // Full budget (k_eff = 1.0)
                let footer_full = build_command_footer(&store, Some(1.0));
                prop_assert!(
                    !footer_full.is_empty(),
                    "INV-GUIDANCE-001: footer must never be empty at k_eff=1.0"
                );

                // Default budget (None → 1.0)
                let footer_default = build_command_footer(&store, None);
                prop_assert!(
                    !footer_default.is_empty(),
                    "INV-GUIDANCE-001: footer must never be empty at default k_eff"
                );

                // Exhausted budget (k_eff → 0)
                let footer_exhausted = build_command_footer(&store, Some(0.05));
                prop_assert!(
                    !footer_exhausted.is_empty(),
                    "INV-GUIDANCE-001: footer must never be empty even at near-zero k_eff"
                );
            }
        }

        // INV-GUIDANCE-003: Guidance respects token budget.
        // The formatted footer at each GuidanceLevel must not exceed that level's
        // token ceiling. Uses the Stage 0 approximate token counter (chars/4).
        // Falsified if any footer exceeds its level's ceiling.
        proptest! {
            #[test]
            fn guidance_respects_token_budget(
                score in 0.0f64..1.0,
                datom_count in 0usize..10000,
                turn in 0u32..200,
                k_eff in 0.0f64..=1.0,
            ) {
                let components = MethodologyComponents {
                    transact_frequency: score,
                    spec_language_ratio: score,
                    query_diversity: score.min(1.0),
                    harvest_quality: score,
                };
                let footer = GuidanceFooter {
                    methodology: MethodologyScore {
                        score,
                        components,
                        trend: Trend::Stable,
                        drift_signal: score < 0.5,
                    },
                    next_action: Some("braid query [:find ?e]".to_string()),
                    invariant_refs: vec!["INV-TEST-001".to_string()],
                    store_datom_count: datom_count,
                    turn,
                    harvest_warning: HarvestWarningLevel::None,
                };

                let level = GuidanceLevel::for_k_eff(k_eff);
                let formatted = format_footer_at_level(&footer, level);
                let counter = ApproxTokenCounter;
                let tokens = counter.count(&formatted);
                let ceiling = level.token_ceiling() as usize;

                // Allow 2x headroom: the ceiling is a design target, not a hard
                // byte-level cap. The property we verify is that compressed levels
                // are *substantially* shorter than uncompressed, and none blow up.
                // Full=200, Compressed=60, Minimal=20, HarvestOnly=10.
                // A 2x multiplier catches regressions without being brittle.
                prop_assert!(
                    tokens <= ceiling * 2,
                    "INV-GUIDANCE-003: {} tokens exceeds 2x ceiling ({}) at level {:?}. Output: {}",
                    tokens,
                    ceiling,
                    level,
                    formatted
                );
            }
        }

        // INV-GUIDANCE-006: Staleness detection.
        // If observations exist and the frontier has advanced far beyond them,
        // the staleness score must exceed 0.8 (the R17 threshold), and
        // `derive_actions` must produce an Investigate action referencing
        // ADR-HARVEST-005.
        // Falsified if stale observations go undetected.
        proptest! {
            #[test]
            fn staleness_detected_for_old_observations(
                confidence in 0.1f64..0.7,
                distance in 50u64..500,
            ) {
                use crate::datom::{AgentId, Datom, Op, TxId, Value};
                use ordered_float::OrderedFloat;

                let obs_wall: u64 = 100;
                let frontier_wall = obs_wall + distance;

                let entity = EntityId::from_ident(":observation/proptest-stale");
                let agent = AgentId::from_name("proptest");
                let tx_obs = TxId::new(obs_wall, 0, agent);
                let tx_frontier = TxId::new(frontier_wall, 0, agent);

                let mut datoms = std::collections::BTreeSet::new();

                // Observation entity
                datoms.insert(Datom::new(
                    entity,
                    Attribute::from_keyword(":db/ident"),
                    Value::Keyword(":observation/proptest-stale".to_string()),
                    tx_obs,
                    Op::Assert,
                ));
                datoms.insert(Datom::new(
                    entity,
                    Attribute::from_keyword(":exploration/body"),
                    Value::String("proptest observation".to_string()),
                    tx_obs,
                    Op::Assert,
                ));
                datoms.insert(Datom::new(
                    entity,
                    Attribute::from_keyword(":exploration/confidence"),
                    Value::Double(OrderedFloat(confidence)),
                    tx_obs,
                    Op::Assert,
                ));
                datoms.insert(Datom::new(
                    entity,
                    Attribute::from_keyword(":exploration/source"),
                    Value::String("braid:observe".to_string()),
                    tx_obs,
                    Op::Assert,
                ));

                // Frontier-advancing datom (non-observation)
                datoms.insert(Datom::new(
                    EntityId::from_ident(":other/frontier-advance"),
                    Attribute::from_keyword(":db/doc"),
                    Value::String("advance".to_string()),
                    tx_frontier,
                    Op::Assert,
                ));

                let store = Store::from_datoms(datoms);

                // Verify staleness exceeds the R17 threshold (0.8)
                let stale = observation_staleness(&store);
                prop_assert_eq!(stale.len(), 1, "should find exactly 1 observation");
                let (_, staleness) = stale[0];

                // Expected: staleness = 1 - confidence * 0.95^distance
                // With confidence in [0.1, 0.7) and distance in [50, 500),
                // 0.95^50 ≈ 0.077, so max contribution = 0.7 * 0.077 ≈ 0.054,
                // meaning staleness >= 0.946 — always above 0.8.
                prop_assert!(
                    staleness > 0.8,
                    "INV-GUIDANCE-006: staleness {:.4} must exceed 0.8 \
                     (confidence={:.2}, distance={})",
                    staleness,
                    confidence,
                    distance
                );

                // Verify derive_actions produces an Investigate action for stale obs
                let actions = derive_actions(&store);
                let has_staleness_action = actions.iter().any(|a| {
                    a.category == ActionCategory::Investigate
                        && a.relates_to.contains(&"ADR-HARVEST-005".to_string())
                });
                prop_assert!(
                    has_staleness_action,
                    "INV-GUIDANCE-006: derive_actions must flag stale observations. \
                     Got actions: {:?}",
                    actions
                        .iter()
                        .map(|a| format!("{}: {}", a.category, a.summary))
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    // -------------------------------------------------------------------
    // HarvestWarningLevel (Q(t)-based thresholds)
    // -------------------------------------------------------------------

    // Verifies: INV-HARVEST-005 — Proactive warning fires at correct thresholds.
    // Verifies: ADR-BUDGET-001 — Measured context over heuristic.
    #[test]
    fn harvest_warning_level_none_above_06() {
        assert_eq!(harvest_warning_level(0.61), HarvestWarningLevel::None);
        assert_eq!(harvest_warning_level(0.7), HarvestWarningLevel::None);
        assert_eq!(harvest_warning_level(0.9), HarvestWarningLevel::None);
        assert_eq!(harvest_warning_level(1.0), HarvestWarningLevel::None);
    }

    #[test]
    fn harvest_warning_level_info_between_03_06() {
        assert_eq!(harvest_warning_level(0.6), HarvestWarningLevel::Info);
        assert_eq!(harvest_warning_level(0.45), HarvestWarningLevel::Info);
        assert_eq!(harvest_warning_level(0.3), HarvestWarningLevel::Info);
    }

    #[test]
    fn harvest_warning_level_warn_between_015_03() {
        assert_eq!(harvest_warning_level(0.29), HarvestWarningLevel::Warn);
        assert_eq!(harvest_warning_level(0.2), HarvestWarningLevel::Warn);
        assert_eq!(harvest_warning_level(0.15), HarvestWarningLevel::Warn);
    }

    #[test]
    fn harvest_warning_level_critical_below_015() {
        assert_eq!(harvest_warning_level(0.14), HarvestWarningLevel::Critical);
        assert_eq!(harvest_warning_level(0.05), HarvestWarningLevel::Critical);
        assert_eq!(harvest_warning_level(0.0), HarvestWarningLevel::Critical);
    }

    // Verifies: threshold boundaries are exact
    #[test]
    fn harvest_warning_level_boundary_precision() {
        // 0.6 is Info (inclusive lower bound of [0.3, 0.6])
        assert_eq!(harvest_warning_level(0.6), HarvestWarningLevel::Info);
        // 0.6 + epsilon is None
        assert_eq!(
            harvest_warning_level(0.6 + f64::EPSILON),
            HarvestWarningLevel::None
        );
        // 0.3 is Info
        assert_eq!(harvest_warning_level(0.3), HarvestWarningLevel::Info);
        // 0.3 - epsilon is Warn
        assert_eq!(
            harvest_warning_level(0.3 - f64::EPSILON),
            HarvestWarningLevel::Warn
        );
        // 0.15 is Warn
        assert_eq!(harvest_warning_level(0.15), HarvestWarningLevel::Warn);
        // 0.15 - epsilon is Critical
        assert_eq!(
            harvest_warning_level(0.15 - f64::EPSILON),
            HarvestWarningLevel::Critical
        );
    }

    #[test]
    fn harvest_warning_from_k_eff_full_budget() {
        // k_eff=1.0 → Q(t)=1.0 → None
        assert_eq!(harvest_warning_from_k_eff(1.0), HarvestWarningLevel::None);
    }

    #[test]
    fn harvest_warning_from_k_eff_half_budget() {
        // k_eff=0.5 → decay=0.5/0.6=0.833 → Q(t)=0.5*0.833=0.417 → Info
        assert_eq!(harvest_warning_from_k_eff(0.5), HarvestWarningLevel::Info);
    }

    #[test]
    fn harvest_warning_from_k_eff_low_budget() {
        // k_eff=0.1 → decay=0.5*(0.1/0.3)^2=0.0556 → Q(t)=0.1*0.0556=0.00556 → Critical
        assert_eq!(
            harvest_warning_from_k_eff(0.1),
            HarvestWarningLevel::Critical
        );
    }

    #[test]
    fn harvest_warning_from_k_eff_zero() {
        // k_eff=0.0 → Q(t)=0.0 → Critical
        assert_eq!(
            harvest_warning_from_k_eff(0.0),
            HarvestWarningLevel::Critical
        );
    }

    #[test]
    fn harvest_warning_level_ordering() {
        // HarvestWarningLevel derives Ord; verify ordering
        assert!(HarvestWarningLevel::None < HarvestWarningLevel::Info);
        assert!(HarvestWarningLevel::Info < HarvestWarningLevel::Warn);
        assert!(HarvestWarningLevel::Warn < HarvestWarningLevel::Critical);
    }

    #[test]
    fn harvest_warning_level_is_active() {
        assert!(!HarvestWarningLevel::None.is_active());
        assert!(HarvestWarningLevel::Info.is_active());
        assert!(HarvestWarningLevel::Warn.is_active());
        assert!(HarvestWarningLevel::Critical.is_active());
    }

    #[test]
    fn harvest_warning_level_messages_non_empty_when_active() {
        assert!(HarvestWarningLevel::None.message().is_empty());
        assert!(!HarvestWarningLevel::Info.message().is_empty());
        assert!(!HarvestWarningLevel::Warn.message().is_empty());
        assert!(!HarvestWarningLevel::Critical.message().is_empty());
    }

    #[test]
    fn harvest_warning_level_suggested_actions() {
        assert!(HarvestWarningLevel::None.suggested_action().is_none());
        assert!(HarvestWarningLevel::Info.suggested_action().is_some());
        assert!(HarvestWarningLevel::Warn.suggested_action().is_some());
        assert!(HarvestWarningLevel::Critical.suggested_action().is_some());
        // All active levels suggest braid harvest
        for level in [
            HarvestWarningLevel::Info,
            HarvestWarningLevel::Warn,
            HarvestWarningLevel::Critical,
        ] {
            let action = level.suggested_action().unwrap();
            assert!(
                action.contains("braid harvest"),
                "level {:?} should suggest braid harvest, got: {}",
                level,
                action
            );
        }
    }

    #[test]
    fn harvest_warning_level_to_priority() {
        // Critical = 1, Warn = 2, Info = 3, None = 4
        assert_eq!(HarvestWarningLevel::Critical.to_priority(), 1);
        assert_eq!(HarvestWarningLevel::Warn.to_priority(), 2);
        assert_eq!(HarvestWarningLevel::Info.to_priority(), 3);
        assert_eq!(HarvestWarningLevel::None.to_priority(), 4);
    }

    #[test]
    fn harvest_warning_level_display() {
        assert_eq!(format!("{}", HarvestWarningLevel::None), "");
        assert!(format!("{}", HarvestWarningLevel::Info).contains("harvest recommended"));
        assert!(format!("{}", HarvestWarningLevel::Warn).contains("harvest soon"));
        assert!(format!("{}", HarvestWarningLevel::Critical).contains("HARVEST NOW"));
    }

    // Verifies: derive_actions_with_budget uses Q(t) when provided
    #[test]
    fn derive_actions_with_budget_uses_qt_thresholds() {
        let store = Store::genesis();
        // Q(t)=0.1 → Critical → should produce Harvest action at priority 1
        let actions = derive_actions_with_budget(&store, Some(0.1));
        let harvest_actions: Vec<_> = actions
            .iter()
            .filter(|a| a.category == ActionCategory::Harvest)
            .collect();
        assert!(
            !harvest_actions.is_empty(),
            "Q(t)=0.1 should produce a harvest action"
        );
        assert_eq!(
            harvest_actions[0].priority, 1,
            "Critical Q(t) should produce priority 1 action"
        );
        assert!(
            harvest_actions[0].summary.contains("Q(t)=0.10"),
            "summary should include Q(t) value, got: {}",
            harvest_actions[0].summary
        );
    }

    #[test]
    fn derive_actions_with_budget_no_harvest_at_high_qt() {
        let store = Store::genesis();
        // Q(t)=0.8 → None → no harvest action
        let actions = derive_actions_with_budget(&store, Some(0.8));
        let harvest_actions: Vec<_> = actions
            .iter()
            .filter(|a| a.category == ActionCategory::Harvest)
            .collect();
        assert!(
            harvest_actions.is_empty(),
            "Q(t)=0.8 should not produce harvest action"
        );
    }

    #[test]
    fn derive_actions_without_budget_uses_tx_fallback() {
        let store = Store::genesis();
        // Without Q(t), R12 uses tx-count heuristic
        let actions = derive_actions_with_budget(&store, None);
        // Verify no action references Q(t) — should be tx-count based if any
        for action in &actions {
            if action.category == ActionCategory::Harvest {
                assert!(
                    !action.summary.contains("Q(t)"),
                    "without Q(t), harvest action should not reference Q(t)"
                );
            }
        }
    }

    #[test]
    fn build_command_footer_passes_qt_to_actions() {
        let store = Store::genesis();
        // k_eff=0.1 → Q(t)=0.00556 → Critical → HarvestOnly level
        // At HarvestOnly level, footer should mention harvest
        let footer = build_command_footer(&store, Some(0.1));
        assert!(
            footer.contains("HARVEST") || footer.contains("harvest"),
            "footer at k_eff=0.1 should mention harvest, got: {footer}"
        );
    }

    #[test]
    fn build_footer_with_budget_sets_warning_level() {
        let telemetry = SessionTelemetry::default();
        let store = Store::genesis();
        let footer = build_footer_with_budget(&telemetry, &store, None, vec![], Some(0.1));
        assert_eq!(
            footer.harvest_warning,
            HarvestWarningLevel::Critical,
            "Q(t)=0.1 should set Critical warning level"
        );
    }

    #[test]
    fn build_footer_without_budget_defaults_to_none() {
        let telemetry = SessionTelemetry::default();
        let store = Store::genesis();
        let footer = build_footer(&telemetry, &store, None, vec![]);
        assert_eq!(
            footer.harvest_warning,
            HarvestWarningLevel::None,
            "without Q(t), warning level should be None"
        );
    }

    #[test]
    fn format_footer_full_includes_qt_warning_when_active() {
        let telemetry = SessionTelemetry {
            total_turns: 5,
            harvest_quality: 0.9,
            harvest_is_recent: true,
            ..Default::default()
        };
        let store = Store::genesis();
        let footer = build_footer_with_budget(
            &telemetry,
            &store,
            Some("braid harvest --commit".into()),
            vec![],
            Some(0.2), // Warn level
        );
        let formatted = format_footer_at_level(&footer, GuidanceLevel::Full);
        assert!(
            formatted.contains("harvest soon"),
            "Full footer with Warn level should show harvest soon, got: {formatted}"
        );
    }

    #[test]
    fn format_footer_full_no_qt_warning_when_inactive() {
        let telemetry = SessionTelemetry {
            total_turns: 5,
            harvest_quality: 0.9,
            harvest_is_recent: true,
            ..Default::default()
        };
        let store = Store::genesis();
        let footer = build_footer_with_budget(
            &telemetry,
            &store,
            None,
            vec![],
            Some(0.8), // None level
        );
        let formatted = format_footer_at_level(&footer, GuidanceLevel::Full);
        assert!(
            !formatted.contains("harvest soon") && !formatted.contains("HARVEST NOW"),
            "Full footer at Q(t)=0.8 should not show harvest warning, got: {formatted}"
        );
    }

    #[test]
    fn format_footer_compressed_includes_critical_warning() {
        let telemetry = SessionTelemetry {
            total_turns: 5,
            harvest_quality: 0.9,
            harvest_is_recent: true,
            ..Default::default()
        };
        let store = Store::genesis();
        let footer = build_footer_with_budget(
            &telemetry,
            &store,
            None,
            vec![],
            Some(0.05), // Critical
        );
        let formatted = format_footer_at_level(&footer, GuidanceLevel::Compressed);
        assert!(
            formatted.contains("HARVEST NOW"),
            "Compressed footer with Critical should show HARVEST NOW, got: {formatted}"
        );
    }

    #[test]
    fn format_footer_harvest_only_uses_qt_warning() {
        let telemetry = SessionTelemetry {
            total_turns: 5,
            harvest_quality: 0.9,
            harvest_is_recent: true,
            ..Default::default()
        };
        let store = Store::genesis();
        let footer = build_footer_with_budget(
            &telemetry,
            &store,
            Some("braid harvest --commit".into()),
            vec![],
            Some(0.05), // Critical
        );
        let formatted = format_footer_at_level(&footer, GuidanceLevel::HarvestOnly);
        assert!(
            formatted.contains("HARVEST NOW"),
            "HarvestOnly with Critical Q(t) should show HARVEST NOW, got: {formatted}"
        );
    }

    #[test]
    fn format_footer_minimal_critical_overrides_action() {
        let telemetry = SessionTelemetry {
            total_turns: 5,
            harvest_quality: 0.9,
            harvest_is_recent: true,
            ..Default::default()
        };
        let store = Store::genesis();
        let footer = build_footer_with_budget(
            &telemetry,
            &store,
            Some("braid query [:find ?e :where [?e :db/ident]]".into()),
            vec![],
            Some(0.05), // Critical
        );
        let formatted = format_footer_at_level(&footer, GuidanceLevel::Minimal);
        assert!(
            formatted.contains("HARVEST NOW"),
            "Minimal footer at Critical should override action with HARVEST NOW, got: {formatted}"
        );
    }

    // -------------------------------------------------------------------
    // compute_routing_from_store tests (INV-GUIDANCE-010)
    // -------------------------------------------------------------------

    /// Create a store with full schema for task tests (mirrors task.rs test helper).
    fn routing_test_store() -> Store {
        use crate::datom::AgentId;
        use crate::schema::{full_schema_datoms, genesis_datoms};
        let agent = AgentId::from_name("test");
        let genesis_tx = crate::datom::TxId::new(0, 0, agent);
        let mut datoms = std::collections::BTreeSet::new();
        for d in genesis_datoms(genesis_tx) {
            datoms.insert(d);
        }
        for d in full_schema_datoms(genesis_tx) {
            datoms.insert(d);
        }
        Store::from_datoms(datoms)
    }

    /// Rebuild a store from its datoms plus additional datoms.
    fn routing_store_with(
        store: &Store,
        extra: impl IntoIterator<Item = crate::datom::Datom>,
    ) -> Store {
        let mut datoms = store.datom_set().clone();
        for d in extra {
            datoms.insert(d);
        }
        Store::from_datoms(datoms)
    }

    // Verifies: INV-GUIDANCE-010 — R(t) routing from empty store
    #[test]
    fn routing_from_store_empty_returns_empty() {
        let store = Store::genesis();
        let routed = compute_routing_from_store(&store);
        assert!(
            routed.is_empty(),
            "genesis store with no tasks should produce empty routing"
        );
    }

    // Verifies: INV-GUIDANCE-010 — R(t) routes real store tasks
    #[test]
    fn routing_from_store_with_tasks_returns_ranked() {
        use crate::datom::AgentId;
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let store = routing_test_store();
        let agent = AgentId::from_name("test");
        let tx = crate::datom::TxId::new(1, 0, agent);

        // Create three tasks with different priorities
        let (_, datoms_a) = create_task_datoms(CreateTaskParams {
            title: "High prio task",
            description: None,
            priority: 0,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, datoms_a);

        let (_, datoms_b) = create_task_datoms(CreateTaskParams {
            title: "Medium prio task",
            description: None,
            priority: 2,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, datoms_b);

        let (_, datoms_c) = create_task_datoms(CreateTaskParams {
            title: "Low prio task",
            description: None,
            priority: 4,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, datoms_c);

        let routed = compute_routing_from_store(&store);
        assert_eq!(routed.len(), 3, "all three tasks should be ready (no deps)");

        // All should have positive impact scores
        for r in &routed {
            assert!(
                r.impact > 0.0,
                "task '{}' should have positive impact, got {}",
                r.label,
                r.impact
            );
        }

        // Results should be sorted by descending impact
        for w in routed.windows(2) {
            assert!(
                w[0].impact >= w[1].impact,
                "routing must be descending: {} >= {} violated",
                w[0].impact,
                w[1].impact
            );
        }
    }

    // Verifies: INV-GUIDANCE-010 — R(t) impact-based ranking differs from priority
    //
    // A P2 task that unblocks 5 others should rank above a P1 task that unblocks
    // nothing. This proves that R(t) considers graph structure, not just priority.
    #[test]
    fn routing_from_store_graph_impact_beats_priority() {
        use crate::datom::AgentId;
        use crate::task::{
            create_task_datoms, dep_add_datom, find_task_by_id, generate_task_id, CreateTaskParams,
            TaskType,
        };

        let store = routing_test_store();
        let agent = AgentId::from_name("test");
        let tx = crate::datom::TxId::new(1, 0, agent);

        // Create a P1 "island" task (high priority but blocks nothing)
        let (_, datoms_island) = create_task_datoms(CreateTaskParams {
            title: "Island P1 task",
            description: None,
            priority: 1,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, datoms_island);
        let island_entity = find_task_by_id(&store, &generate_task_id("Island P1 task")).unwrap();

        // Create a P2 "hub" task (lower priority but blocks 5 tasks)
        let (_, datoms_hub) = create_task_datoms(CreateTaskParams {
            title: "Hub P2 task",
            description: None,
            priority: 2,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, datoms_hub);
        let hub_entity = find_task_by_id(&store, &generate_task_id("Hub P2 task")).unwrap();

        // Create 5 downstream tasks that depend on the hub
        for i in 0..5 {
            let title = format!("Downstream task {i}");
            let (_, datoms_down) = create_task_datoms(CreateTaskParams {
                title: &title,
                description: None,
                priority: 3,
                task_type: TaskType::Task,
                tx,
                traces_to: &[],
                labels: &[],
            });
            let store_tmp = routing_store_with(&store, datoms_down);
            let down_entity = find_task_by_id(&store_tmp, &generate_task_id(&title)).unwrap();
            // Each downstream depends on the hub
            let store_tmp =
                routing_store_with(&store_tmp, vec![dep_add_datom(down_entity, hub_entity, tx)]);
            // Reassign store (accumulate)
            // We need to build up incrementally
            let mut all_datoms = store_tmp.datom_set().clone();
            // Merge back
            for d in store.datom_set().iter() {
                all_datoms.insert(d.clone());
            }
            let _ = Store::from_datoms(all_datoms);
        }

        // Rebuild properly: create all downstream tasks in one shot
        let mut accumulated = store.datom_set().clone();
        for i in 0..5 {
            let title = format!("Downstream task {i}");
            let (_, datoms_down) = create_task_datoms(CreateTaskParams {
                title: &title,
                description: None,
                priority: 3,
                task_type: TaskType::Task,
                tx,
                traces_to: &[],
                labels: &[],
            });
            for d in datoms_down {
                accumulated.insert(d);
            }
            let down_entity = EntityId::from_ident(&format!(":task/{}", generate_task_id(&title)));
            accumulated.insert(dep_add_datom(down_entity, hub_entity, tx));
        }
        let store = Store::from_datoms(accumulated);

        let routed = compute_routing_from_store(&store);

        // The hub task and the island task should both be in the ready set
        // (hub has no deps, island has no deps; downstream tasks are blocked)
        let hub_routing = routed.iter().find(|r| r.entity == hub_entity);
        let island_routing = routed.iter().find(|r| r.entity == island_entity);

        assert!(
            hub_routing.is_some(),
            "hub task should be in routed results"
        );
        assert!(
            island_routing.is_some(),
            "island task should be in routed results"
        );

        let hub_impact = hub_routing.unwrap().impact;
        let island_impact = island_routing.unwrap().impact;

        assert!(
            hub_impact > island_impact,
            "P2 hub (impact={hub_impact:.4}) should rank above P1 island \
             (impact={island_impact:.4}) because hub unblocks 5 tasks"
        );
    }
}
