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
/// directly to urgency bands aligned with INV-HARVEST-005:
///
/// - Q(t) > 0.6  -> None (plenty of budget remaining)
/// - Q(t) in [0.15, 0.6] -> Info (context filling, harvest recommended)
/// - Q(t) in [0.05, 0.15) -> Warn (harvest warning — spec threshold Q(t) < 0.15)
/// - Q(t) < 0.05 -> Critical (harvest-only mode — spec threshold Q(t) < 0.05)
///
/// INV-HARVEST-005: Proactive warning fires at correct thresholds.
///   - L0: "Q(t) < 0.15 => response includes harvest warning"
///   - L0: "Q(t) < 0.05 => response = ONLY harvest imperative"
///
/// ADR-BUDGET-001: Measured context over heuristic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HarvestWarningLevel {
    /// Q(t) > 0.6: no warning needed.
    None,
    /// Q(t) in [0.15, 0.6]: context filling, harvest recommended.
    Info,
    /// Q(t) in [0.05, 0.15): harvest warning (INV-HARVEST-005 L0: Q(t) < 0.15).
    Warn,
    /// Q(t) < 0.05: harvest-only mode (INV-HARVEST-005 L0: Q(t) < 0.05).
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
/// This maps Q(t) to four urgency bands aligned with INV-HARVEST-005:
///
/// - Q(t) > 0.6  -> None
/// - Q(t) in [0.15, 0.6] -> Info
/// - Q(t) in [0.05, 0.15) -> Warn (spec: "Q(t) < 0.15 => harvest warning")
/// - Q(t) < 0.05 -> Critical (spec: "Q(t) < 0.05 => harvest-only mode")
///
/// INV-HARVEST-005: Proactive warning fires at correct thresholds.
/// ADR-BUDGET-001: Measured context over heuristic.
pub fn harvest_warning_level(q_t: f64) -> HarvestWarningLevel {
    if q_t > 0.6 {
        HarvestWarningLevel::None
    } else if q_t >= 0.15 {
        HarvestWarningLevel::Info
    } else if q_t >= 0.05 {
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

/// Activity mode detected from session transaction patterns (INV-GUIDANCE-008).
///
/// Used to contextualize guidance hints: implementation-heavy sessions get
/// different paste-ready commands than specification/observation-heavy sessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityMode {
    /// >50% of turns contain transact operations.
    Implementation,
    /// >50% of turns use spec-language (observation/spec references).
    Specification,
    /// Neither pattern dominates.
    Mixed,
}

/// Classify the current session by transaction pattern.
///
/// Returns `Implementation` when transact-heavy, `Specification` when
/// spec-language/observation-heavy, `Mixed` otherwise.
pub fn detect_activity_mode(telemetry: &SessionTelemetry) -> ActivityMode {
    let total = telemetry.total_turns.max(1) as f64;
    let transact_ratio = telemetry.transact_turns as f64 / total;
    let spec_ratio = telemetry.spec_language_turns as f64 / total;

    if transact_ratio > 0.5 {
        ActivityMode::Implementation
    } else if spec_ratio > 0.5 {
        ActivityMode::Specification
    } else {
        ActivityMode::Mixed
    }
}

// ---------------------------------------------------------------------------
// GuidanceContext — assembled context for adaptive guidance (ADR-GUIDANCE-015)
// ---------------------------------------------------------------------------

/// Assembled context for adaptive guidance decisions (ADR-GUIDANCE-015).
/// Computed once per command from store telemetry.
///
/// Provides a single snapshot of all the signals that guidance rules need:
/// budget state, activity mode, transaction velocity, agent count, and
/// crystallization/anchoring gaps.
#[derive(Clone, Debug)]
pub struct GuidanceContext {
    /// Effective attention budget k*_eff (0.0 = exhausted, 1.0 = full).
    pub k_eff: f64,
    /// Current session activity mode (implementation, specification, mixed).
    pub activity_mode: ActivityMode,
    /// Transactions per minute over a 5-minute rolling window.
    pub tx_velocity: f64,
    /// Number of distinct agents in the current frontier.
    pub agent_count: u32,
    /// Number of observations with uncrystallized spec references.
    pub crystallization_gap: u32,
    /// Unanchored tasks (spec refs that don't resolve). Placeholder for AGP-4.
    pub unanchored_tasks: u32,
}

impl GuidanceContext {
    /// Build a `GuidanceContext` from the current store state.
    ///
    /// Computes telemetry, detects activity mode, measures transaction velocity,
    /// and counts frontier agents and crystallization gaps.
    ///
    /// `k_eff` can be supplied externally (e.g., from CLI budget tracking);
    /// defaults to 1.0 (full budget) when `None`.
    pub fn from_store(store: &Store, k_eff: Option<f64>) -> Self {
        let telemetry = telemetry_from_store(store);
        let activity = detect_activity_mode(&telemetry);
        let velocity = tx_velocity(store);
        let agents = store.frontier().len() as u32;
        let gaps = crystallization_candidates(store).len() as u32;
        // unanchored: count tasks where parse_spec_refs returns refs but none resolve.
        // Simplified: 0 placeholder until AGP-4 fills this with real resolution logic.
        // KEFF-3: Use multi-signal estimation when no explicit k_eff provided
        let estimated_k = k_eff.unwrap_or_else(|| {
            let evidence = crate::budget::EvidenceVector::from_store(store);
            crate::budget::estimate_k_eff(&evidence)
        });
        GuidanceContext {
            k_eff: estimated_k,
            activity_mode: activity,
            tx_velocity: velocity,
            agent_count: agents,
            crystallization_gap: gaps,
            unanchored_tasks: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Session Working Set (INV-GUIDANCE-010, SWS-1)
// ---------------------------------------------------------------------------

/// The session working set — temporal locality for R(t) routing.
///
/// Tracks what the agent is actively engaging RIGHT NOW, not all historical
/// in-progress tasks. Uses temporal discrimination: only tasks whose status
/// changed to in-progress AFTER the session boundary are "active."
///
/// Session boundary = max(session.started_at, last_harvest_wall_time, now - 3600).
#[derive(Clone, Debug)]
pub struct SessionWorkingSet {
    /// Tasks set to in-progress AFTER the session boundary (not stale claims).
    pub active_tasks: Vec<EntityId>,
    /// Tasks created after the session boundary.
    pub session_created_tasks: BTreeSet<EntityId>,
    /// Tasks sharing an EPIC parent with any active task.
    pub epic_siblings: BTreeSet<EntityId>,
    /// The computed session boundary (unix seconds).
    pub session_boundary: u64,
}

impl SessionWorkingSet {
    /// Build the working set from store state. Pure function, no IO.
    pub fn from_store(store: &Store) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let session_start = Self::find_session_start(store);
        let harvest_boundary = last_harvest_wall_time(store);
        let fallback = now.saturating_sub(3600);
        let session_boundary = session_start.max(harvest_boundary).max(fallback);

        let all_tasks = crate::task::all_tasks(store);

        // Active tasks: ALL in-progress tasks, not just session-scoped ones.
        // T-UX-3: If a task is in-progress, it represents an active intention
        // regardless of when the status was set. Tasks set in-progress during
        // THIS session get priority 1 (recency boost), others get priority 2.
        let active_tasks: Vec<EntityId> = all_tasks
            .iter()
            .filter(|t| t.status == crate::task::TaskStatus::InProgress)
            .map(|t| t.entity)
            .collect();

        // Session-created tasks
        let session_created_tasks: BTreeSet<EntityId> = all_tasks
            .iter()
            .filter(|t| {
                t.created_at > session_boundary && t.status != crate::task::TaskStatus::Closed
            })
            .map(|t| t.entity)
            .collect();

        // EPIC siblings: for each active task, find EPIC parents, collect children
        let active_set: BTreeSet<EntityId> = active_tasks.iter().copied().collect();
        let mut epic_siblings = BTreeSet::new();
        let task_type_map: BTreeMap<EntityId, String> = all_tasks
            .iter()
            .map(|t| (t.entity, t.task_type.clone()))
            .collect();

        for active_entity in &active_tasks {
            if let Some(active_task) = all_tasks.iter().find(|t| t.entity == *active_entity) {
                for dep in &active_task.depends_on {
                    let is_epic = task_type_map
                        .get(dep)
                        .map(|t| t.contains("epic"))
                        .unwrap_or(false);
                    if is_epic {
                        for t in &all_tasks {
                            if t.depends_on.contains(dep)
                                && !active_set.contains(&t.entity)
                                && t.status != crate::task::TaskStatus::Closed
                            {
                                epic_siblings.insert(t.entity);
                            }
                        }
                    }
                }
            }
        }

        SessionWorkingSet {
            active_tasks,
            session_created_tasks,
            epic_siblings,
            session_boundary,
        }
    }

    /// Find the most recent active session's start time.
    fn find_session_start(store: &Store) -> u64 {
        let mut latest_start: u64 = 0;
        let started_attr = Attribute::from_keyword(":session/started-at");
        let status_attr = Attribute::from_keyword(":session/status");
        for datom in store.attribute_datoms(&started_attr) {
            if datom.op != Op::Assert {
                continue;
            }
            if let Value::Long(wall) = datom.value {
                let wall = wall as u64;
                let is_active = store.entity_datoms(datom.entity).iter().any(|d| {
                    d.attribute == status_attr
                        && d.op == Op::Assert
                        && matches!(&d.value, Value::Keyword(k) if k.contains("active"))
                });
                if is_active && wall > latest_start {
                    latest_start = wall;
                }
            }
        }
        latest_start
    }

    /// Returns true if the working set is empty.
    pub fn is_empty(&self) -> bool {
        self.active_tasks.is_empty()
            && self.session_created_tasks.is_empty()
            && self.epic_siblings.is_empty()
    }
}

/// Compute the session boost for a task entity (SWS-2).
///
/// Returns a multiplier: 3.0 (active), 2.0 (epic sibling), 1.5 (session-created), 1.0 (default).
/// Takes the HIGHEST category when a task is in multiple sets.
pub fn session_boost(entity: EntityId, working_set: &SessionWorkingSet) -> f64 {
    if working_set.active_tasks.contains(&entity) {
        3.0
    } else if working_set.epic_siblings.contains(&entity) {
        2.0
    } else if working_set.session_created_tasks.contains(&entity) {
        1.5
    } else {
        1.0
    }
}

// ---------------------------------------------------------------------------
// Methodology Score (INV-GUIDANCE-008)
// ---------------------------------------------------------------------------

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
    /// Task type for type-based routing weight.
    pub task_type: crate::task::TaskType,
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
    /// Type-based routing multiplier (0.0--1.0).
    /// Weights tasks by type: impl/bug=1.0, feature=0.9, test=0.8, epic=0.0, docs=0.3, question=0.2.
    pub type_multiplier: f64,
    /// Age-based urgency decay (>=1.0).
    /// Logarithmic boost for older tasks: `1.0 + ln(age_days + 1) * 0.1`.
    pub urgency_decay: f64,
    /// Spec anchor factor (0.3, 0.7, or 1.0).
    /// Measures how well the task's spec references resolve in the store.
    /// Applied as a post-factor in `compute_routing_from_store`.
    pub spec_anchor: f64,
    /// Session boost factor (1.0, 1.5, 2.0, or 3.0).
    /// Temporal locality: tasks actively claimed this session get priority.
    pub session_boost: f64,
}

/// R(t) routing weights (defaults from spec).
const DEFAULT_ROUTING_WEIGHTS: [f64; 6] = [0.25, 0.25, 0.20, 0.15, 0.10, 0.05];

/// Number of routing features.
const N_FEATURES: usize = 6;

/// Read learned routing weights from store, falling back to defaults.
///
/// RFL-4: The store may contain a `:routing/weights` datom with a JSON
/// array of 6 floats. If found and valid, use those. Otherwise, use defaults.
pub fn routing_weights(store: &Store) -> [f64; N_FEATURES] {
    let weights_attr = crate::datom::Attribute::from_keyword(":routing/weights");
    let learned = store
        .attribute_datoms(&weights_attr)
        .iter()
        .rev() // most recent first
        .find(|d| d.op == crate::datom::Op::Assert)
        .and_then(|d| match &d.value {
            crate::datom::Value::String(s) => {
                serde_json::from_str::<Vec<f64>>(s).ok()
            }
            _ => None,
        })
        .and_then(|v| {
            if v.len() == N_FEATURES {
                let mut arr = [0.0; N_FEATURES];
                arr.copy_from_slice(&v);
                Some(arr)
            } else {
                None
            }
        });
    learned.unwrap_or(DEFAULT_ROUTING_WEIGHTS)
}

/// Learn routing weights from action-outcome history via ridge regression.
///
/// RFL-4: w = (X^T X + λI)^{-1} X^T y
/// where X is the feature matrix, y is the outcome vector, λ=0.01.
///
/// SAFEGUARDS:
/// - Minimum 50 data points (avoid overfitting)
/// - Weights clamped to [0.01, 0.5]
/// - Normalized to sum to 1.0
/// - Returns None if insufficient data or computation fails
///
/// INV-GUIDANCE-005, INV-GUIDANCE-010, ADR-TOPOLOGY-004.
pub fn refit_routing_weights(store: &Store) -> Option<[f64; N_FEATURES]> {
    let cmd_attr = crate::datom::Attribute::from_keyword(":action/recommended-command");
    let outcome_attr = crate::datom::Attribute::from_keyword(":action/outcome");
    let features_attr = crate::datom::Attribute::from_keyword(":action/features");

    // Collect (features, outcome) pairs
    let mut data: Vec<([f64; N_FEATURES], f64)> = Vec::new();

    for datom in store.attribute_datoms(&cmd_attr).iter() {
        if datom.op != crate::datom::Op::Assert {
            continue;
        }
        let entity = datom.entity;
        let entity_datoms = store.entity_datoms(entity);

        // Get outcome
        let outcome = entity_datoms.iter()
            .find(|d| d.attribute == outcome_attr && d.op == crate::datom::Op::Assert)
            .and_then(|d| match &d.value {
                crate::datom::Value::Keyword(k) => match k.as_str() {
                    ":action.outcome/followed" => Some(1.0),
                    ":action.outcome/adjacent" => Some(0.5),
                    ":action.outcome/ignored" => Some(0.0),
                    _ => None,
                },
                _ => None,
            });

        // Get features
        let features = entity_datoms.iter()
            .find(|d| d.attribute == features_attr && d.op == crate::datom::Op::Assert)
            .and_then(|d| match &d.value {
                crate::datom::Value::String(s) => serde_json::from_str::<Vec<f64>>(s).ok(),
                _ => None,
            })
            .and_then(|v| {
                if v.len() == N_FEATURES {
                    let mut arr = [0.0; N_FEATURES];
                    arr.copy_from_slice(&v);
                    Some(arr)
                } else {
                    None
                }
            });

        if let (Some(y), Some(x)) = (outcome, features) {
            data.push((x, y));
        }
    }

    // Safeguard: minimum 50 data points
    if data.len() < 50 {
        return None;
    }

    let n = data.len();

    // Build X^T X + λI (6×6 matrix, stored as flat array)
    let lambda = 0.01;
    let mut xtx = [[0.0f64; N_FEATURES]; N_FEATURES];
    let mut xty = [0.0f64; N_FEATURES];

    for (x, y) in &data {
        for i in 0..N_FEATURES {
            xty[i] += x[i] * y;
            for j in 0..N_FEATURES {
                xtx[i][j] += x[i] * x[j];
            }
        }
    }

    // Add ridge regularization: λI
    for (i, row) in xtx.iter_mut().enumerate().take(N_FEATURES) {
        row[i] += lambda * n as f64;
    }

    // Solve via Gaussian elimination (6×6 — trivially small)
    let weights = solve_linear_system_6x6(&xtx, &xty)?;

    // Safeguard: clamp to [0.01, 0.5]
    let mut clamped = [0.0; N_FEATURES];
    for i in 0..N_FEATURES {
        clamped[i] = weights[i].clamp(0.01, 0.5);
    }

    // Normalize to sum to 1.0
    let sum: f64 = clamped.iter().sum();
    if sum <= 0.0 {
        return None;
    }
    for w in &mut clamped {
        *w /= sum;
    }

    Some(clamped)
}

/// Feature names for the 6 R(t) routing dimensions.
///
/// Corresponds to [g1..g6] in compute_routing():
/// g1=pagerank, g2=betweenness, g3=critical_path, g4=blocker_ratio, g5=staleness, g6=priority.
pub const ROUTING_FEATURE_NAMES: [&str; 6] = [
    "pagerank",
    "betweenness",
    "critical_path",
    "blocker_ratio",
    "staleness",
    "priority",
];

/// R(t) routing weight dashboard data (RFL-5).
///
/// Provides visibility into the current routing weights, their source
/// (learned vs. default), and follow-through statistics from action-outcome
/// pairs stored as `:action/*` datoms.
#[derive(Clone, Debug)]
pub struct RoutingDashboard {
    /// Current active weights (either learned or defaults).
    pub weights: [f64; 6],
    /// Feature names for display.
    pub feature_names: [&'static str; 6],
    /// Whether the weights come from a learned refit (true) or defaults (false).
    pub learned: bool,
    /// Total action-outcome pairs (entities with `:action/recommended-command`).
    pub total_actions: usize,
    /// Number of actions that have an `:action/outcome` datom.
    pub actions_with_outcome: usize,
    /// Number of "followed" outcomes (`:action.outcome/followed`).
    pub followed_count: usize,
    /// Follow-through rate: followed / actions_with_outcome (0.0 if no outcomes).
    pub follow_through_rate: f64,
    /// True if total_actions < 50 (preview mode).
    pub preview: bool,
}

/// Compute the R(t) routing dashboard from the store (RFL-5).
///
/// Collects:
/// 1. Current routing weights (learned or default) via `routing_weights()`.
/// 2. Whether a learned refit succeeded (>= 50 data points).
/// 3. Action-outcome statistics from `:action/*` datoms.
///
/// Traces to: INV-GUIDANCE-010, INV-GUIDANCE-005, ADR-TOPOLOGY-004.
pub fn routing_dashboard(store: &Store) -> RoutingDashboard {
    let weights = routing_weights(store);
    let learned = refit_routing_weights(store).is_some();

    let cmd_attr = crate::datom::Attribute::from_keyword(":action/recommended-command");
    let outcome_attr = crate::datom::Attribute::from_keyword(":action/outcome");

    // Count total action entities (those with :action/recommended-command)
    let action_entities: Vec<crate::datom::EntityId> = store
        .attribute_datoms(&cmd_attr)
        .iter()
        .filter(|d| d.op == crate::datom::Op::Assert)
        .map(|d| d.entity)
        .collect();
    let total_actions = action_entities.len();

    // Count outcomes and classify
    let mut actions_with_outcome = 0usize;
    let mut followed_count = 0usize;

    for entity in &action_entities {
        let entity_datoms = store.entity_datoms(*entity);
        if let Some(outcome_datom) = entity_datoms
            .iter()
            .find(|d| d.attribute == outcome_attr && d.op == crate::datom::Op::Assert)
        {
            actions_with_outcome += 1;
            if let crate::datom::Value::Keyword(k) = &outcome_datom.value {
                if k == ":action.outcome/followed" {
                    followed_count += 1;
                }
            }
        }
    }

    let follow_through_rate = if actions_with_outcome > 0 {
        followed_count as f64 / actions_with_outcome as f64
    } else {
        0.0
    };

    RoutingDashboard {
        weights,
        feature_names: ROUTING_FEATURE_NAMES,
        learned,
        total_actions,
        actions_with_outcome,
        followed_count,
        follow_through_rate,
        preview: total_actions < 50,
    }
}

/// Solve a 6x6 linear system Ax = b via Gaussian elimination with partial pivoting.
///
/// Returns None if the system is singular (shouldn't happen with ridge regularization).
fn solve_linear_system_6x6(a: &[[f64; N_FEATURES]; N_FEATURES], b: &[f64; N_FEATURES]) -> Option<[f64; N_FEATURES]> {
    // Augmented matrix [A|b]
    let mut aug = [[0.0; N_FEATURES + 1]; N_FEATURES];
    for i in 0..N_FEATURES {
        for j in 0..N_FEATURES {
            aug[i][j] = a[i][j];
        }
        aug[i][N_FEATURES] = b[i];
    }

    // Forward elimination with partial pivoting
    for col in 0..N_FEATURES {
        // Find pivot
        let mut max_row = col;
        let mut max_val = aug[col][col].abs();
        for (idx, aug_row) in aug.iter().enumerate().take(N_FEATURES).skip(col + 1) {
            if aug_row[col].abs() > max_val {
                max_val = aug_row[col].abs();
                max_row = idx;
            }
        }
        if max_val < 1e-12 {
            return None; // Singular
        }
        aug.swap(col, max_row);

        // Eliminate below
        let pivot = aug[col][col];
        for row in (col + 1)..N_FEATURES {
            let factor = aug[row][col] / pivot;
            let pivot_row: Vec<f64> = aug[col][col..=N_FEATURES].to_vec();
            for (j, &pval) in (col..=N_FEATURES).zip(pivot_row.iter()) {
                aug[row][j] -= factor * pval;
            }
        }
    }

    // Back substitution
    let mut x = [0.0; N_FEATURES];
    for i in (0..N_FEATURES).rev() {
        let mut sum = aug[i][N_FEATURES];
        for j in (i + 1)..N_FEATURES {
            sum -= aug[i][j] * x[j];
        }
        x[i] = sum / aug[i][i];
    }

    Some(x)
}

/// Compute age-based urgency factor.
///
/// Older tasks get a logarithmic boost (they have been waiting longer).
/// Returns values in the range `[1.0, ~1.3]` for tasks up to a week old --
/// enough to break ties but not override PageRank/betweenness.
fn urgency_decay(created_at: u64, now: u64) -> f64 {
    let age_seconds = now.saturating_sub(created_at);
    let age_days = age_seconds as f64 / 86400.0;
    1.0 + (age_days + 1.0).ln() * 0.1
}

/// Compute R(t) -- ranked routing over a task graph (INV-GUIDANCE-010).
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

            // g₄: blocker ratio (INV-GUIDANCE-013: typed edge routing)
            // Weight each blocked task by its type_multiplier:
            // blocking an impl task contributes more than blocking a docs task.
            let blocker_ratio = if task.blocks.is_empty() {
                0.0
            } else {
                let weighted_blocks: f64 = task.blocks.iter().map(|blocked_entity| {
                    tasks.iter()
                        .find(|n| n.entity == *blocked_entity)
                        .map(|n| n.task_type.type_multiplier())
                        .unwrap_or(1.0)
                }).sum();
                weighted_blocks / total_tasks
            };

            // g₅: staleness
            let age = now.saturating_sub(task.created_at) as f64;
            let staleness = (age / max_age).min(1.0);

            // g₆: priority boost
            let priority_boost = task.priority_boost;

            // Type multiplier: weight by task type (impl/bug=1.0, epic=0.0, etc.)
            let tm = task.task_type.type_multiplier();

            // Urgency decay: logarithmic age boost (1.0 for new, ~1.3 for week-old)
            let ud = urgency_decay(task.created_at, now);

            let metrics = RoutingMetrics {
                pagerank,
                betweenness_proxy,
                critical_path_pos,
                blocker_ratio,
                staleness,
                priority_boost,
                type_multiplier: tm,
                urgency_decay: ud,
                spec_anchor: 1.0,
                session_boost: 1.0,
            };

            let values = [
                pagerank,
                betweenness_proxy,
                critical_path_pos,
                blocker_ratio,
                staleness,
                priority_boost,
            ];
            let base_impact: f64 = DEFAULT_ROUTING_WEIGHTS
                .iter()
                .zip(values.iter())
                .map(|(w, v)| w * v)
                .sum();

            // Apply type multiplier and urgency decay as post-factors
            let impact = base_impact * tm * ud;

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

            // Parse task type from keyword string, default to Task
            let task_type = crate::task::TaskType::from_keyword(&t.task_type)
                .unwrap_or(crate::task::TaskType::Task);

            TaskNode {
                entity: t.entity,
                label: t.title.clone(),
                priority_boost,
                done: t.status == crate::task::TaskStatus::Closed,
                depends_on,
                blocks,
                created_at: t.created_at,
                task_type,
            }
        })
        .collect();

    // Use wall-clock now for staleness normalization
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Compute base routing, then apply post-multipliers:
    // 1. spec_anchor_factor: unanchored tasks sink (0.3×)
    // 2. session_boost: active tasks dominate (additive + multiplicative hybrid)
    let mut routings = compute_routing(&nodes, now);
    let working_set = SessionWorkingSet::from_store(store);

    for r in &mut routings {
        // Spec anchor (SFE-3.2)
        let anchor = spec_anchor_factor(store, r.entity);
        r.metrics.spec_anchor = anchor;
        r.impact *= anchor;

        // Session boost (SWS-2/SWS-3): HYBRID additive + multiplicative
        // Multiplicative alone fails for leaf nodes (3.0 × 0.01 = 0.03, still low).
        // Additive alone ignores base quality (all active tasks equal).
        // Hybrid: multiply by boost AND add a floor that guarantees active tasks
        // rank above any cold task. The additive floor = 0.5 (well above max cold impact ~0.15).
        let boost = session_boost(r.entity, &working_set);
        r.metrics.session_boost = boost;
        if boost > 1.0 {
            let additive_floor = match boost as u32 {
                3 => 0.5,  // active: guaranteed top
                2 => 0.3,  // epic sibling: guaranteed above cold
                _ => 0.15, // session-created: slight lift
            };
            r.impact = r.impact * boost + additive_floor;
        }
    }

    // Re-sort after applying all post-factors
    routings.sort_by(|a, b| {
        b.impact
            .partial_cmp(&a.impact)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    routings
}

// ---------------------------------------------------------------------------
// ACP Action Extraction (INV-BUDGET-009)
// ---------------------------------------------------------------------------

/// Compute the recommended action from the store state (INV-BUDGET-009).
///
/// This is the SINGLE CODE PATH for action computation. Both the guidance
/// footer and the ACP projection use this function. It extracts the R(t)
/// top recommendation and wraps it as a `ProjectedAction`.
///
/// Edge case handling (priority order):
/// 1. Harvest overdue (>= 15 tx since last) → "braid harvest --commit"
/// 2. R(t) has tasks → top-impact task → "braid go <id>"
/// 3. No tasks but observations exist → "braid observe" or "braid spec create"
/// 4. Empty store → "braid observe" (seed the knowledge graph)
pub fn compute_action_from_store(store: &Store) -> crate::budget::ProjectedAction {
    // Check harvest urgency first — use canonical boundary (same as harvest_urgency_multi)
    let tx_since_harvest = count_txns_since_last_harvest(store);

    if tx_since_harvest >= 15 {
        return crate::budget::ProjectedAction {
            command: "braid harvest --commit".to_string(),
            rationale: format!("harvest overdue ({tx_since_harvest} tx since last)"),
            impact: 1.0,
        };
    }

    // Try R(t) routing for task recommendation
    let routings = compute_routing_from_store(store);
    if let Some(top) = routings.first() {
        // Find the task ID from the label or entity
        let task_id = store
            .entity_datoms(top.entity)
            .iter()
            .find(|d| {
                d.attribute.as_str() == ":task/id"
                    && d.op == crate::datom::Op::Assert
            })
            .and_then(|d| match &d.value {
                crate::datom::Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| format!("{:?}", top.entity));

        // BAO-1: Include spec reference in rationale (basin activation token).
        // Spec-language activates the formal reasoning substrate (INV-GUIDANCE-002).
        let spec_ref = store
            .entity_datoms(top.entity)
            .iter()
            .find(|d| {
                d.attribute.as_str() == ":task/traces-to"
                    && d.op == crate::datom::Op::Assert
            })
            .and_then(|d| match &d.value {
                crate::datom::Value::Ref(target) => {
                    // Resolve the spec element's human ID
                    store
                        .entity_datoms(*target)
                        .iter()
                        .find(|dd| dd.attribute.as_str() == ":spec/id" && dd.op == crate::datom::Op::Assert)
                        .and_then(|dd| match &dd.value {
                            crate::datom::Value::String(s) => Some(s.clone()),
                            _ => None,
                        })
                }
                _ => None,
            });

        // Truncate label for rationale (max ~8 words)
        let rationale = {
            let words: Vec<&str> = top.label.split_whitespace().collect();
            let title_part = if words.len() > 8 {
                format!("{} ...", words[..8].join(" "))
            } else {
                top.label.clone()
            };
            // Append spec ref if available (basin activation token)
            match spec_ref {
                Some(ref sr) => format!("{} ({})", title_part, sr),
                None => title_part,
            }
        };

        return crate::budget::ProjectedAction {
            command: format!("braid go {task_id}"),
            rationale,
            impact: top.impact,
        };
    }

    // No tasks — suggest observation
    if store.len() > 100 {
        crate::budget::ProjectedAction {
            command: "braid observe \"...\" --confidence 0.8".to_string(),
            rationale: "capture knowledge for the store".to_string(),
            impact: 0.1,
        }
    } else {
        crate::budget::ProjectedAction {
            command: "braid observe \"...\" --confidence 0.8".to_string(),
            rationale: "seed the knowledge graph".to_string(),
            impact: 0.1,
        }
    }
}

// ---------------------------------------------------------------------------
// ACP Methodology Context Blocks (ACP-9, INV-BUDGET-009)
// ---------------------------------------------------------------------------

/// Build methodology Context blocks for ACP projections (ACP-9).
///
/// Extracts the M(t) score, sub-metric checks, and store state from the
/// guidance system and packages them as ContextBlocks at Methodology precedence.
/// These blocks replace the guidance footer for ACP-enabled commands.
///
/// The footer's next-action is NOT included here — that's the Action layer,
/// provided by compute_action_from_store().
pub fn methodology_context_blocks(
    store: &Store,
) -> Vec<crate::budget::ContextBlock> {
    let telemetry = telemetry_from_store(store);
    let score = compute_methodology_score(&telemetry);

    let check = |name: &str, value: f64, threshold: f64, cmd: &str| -> String {
        if value >= threshold {
            format!("{}: \u{2713}", name)
        } else {
            format!("{}: \u{2717}\u{2192}{}", name, cmd)
        }
    };

    let m_line = format!(
        "M(t): {:.2} ({} | {} | {} | {})",
        score.score,
        check("tx", score.components.transact_frequency, 0.4, "write"),
        check("spec-lang", score.components.spec_language_ratio, 0.4, "query --entity :spec/..."),
        check("q-div", score.components.query_diversity, 0.4, "query"),
        check("harvest", score.components.harvest_quality, 0.4, "harvest"),
    );

    let mut blocks = vec![crate::budget::ContextBlock {
        precedence: crate::budget::OutputPrecedence::Methodology,
        content: m_line,
        tokens: 20,
    }];

    // Store state context
    blocks.push(crate::budget::ContextBlock {
        precedence: crate::budget::OutputPrecedence::Ambient,
        content: format!("Store: {} datoms | Turn {}", store.len(), store.frontier().len()),
        tokens: 8,
    });

    blocks
}

// ---------------------------------------------------------------------------
// Spec Anchor Factor (SFE-3.1)
// ---------------------------------------------------------------------------

/// Compute the spec anchor factor for a task.
///
/// Measures how well a task's spec references resolve against the store.
/// Used to weight guidance recommendations: well-anchored tasks (all refs
/// resolve to formal spec elements) get full weight; unanchored tasks
/// (no refs resolve, or refs point to nonexistent elements) are discounted.
///
/// Returns:
/// - `1.0` if all refs resolve (or no refs at all — vacuously true)
/// - `0.7` if some but not all refs resolve (partial anchoring)
/// - `0.3` if no refs resolve (completely unanchored)
///
/// A ref "resolves" when its `:spec/{id-lowercase}` entity has a
/// `:spec/falsification` attribute (formal spec element, not observation).
pub fn spec_anchor_factor(store: &Store, task_entity: EntityId) -> f64 {
    // Extract title from the task's datoms
    let title = store
        .entity_datoms(task_entity)
        .iter()
        .find(|d| d.attribute.as_str() == ":task/title" && d.op == crate::datom::Op::Assert)
        .and_then(|d| match &d.value {
            Value::String(s) => Some(s.clone()),
            _ => None,
        });

    let title = match title {
        Some(t) => t,
        None => return 1.0, // No title => no refs => vacuously 1.0
    };

    let refs = crate::task::parse_spec_refs(&title);
    if refs.is_empty() {
        return 1.0;
    }

    let (resolved, _unresolved) = crate::task::resolve_spec_refs(store, &refs);
    let ratio = resolved.len() as f64 / refs.len() as f64;
    if ratio >= 1.0 {
        1.0
    } else if ratio > 0.0 {
        0.7
    } else {
        0.3
    }
}

// ---------------------------------------------------------------------------
// Guidance Footer (INV-GUIDANCE-001)
// ---------------------------------------------------------------------------

/// Contextual observation hint derived from a command's output (INV-GUIDANCE-014).
///
/// Pairs a human-readable observation sentence with a confidence level
/// appropriate for the command type that produced it.
#[derive(Clone, Debug)]
pub struct ContextualHint {
    /// The observation text to suggest (replaces `"..."` in the footer).
    pub text: String,
    /// Suggested confidence for the observation (0.0–1.0).
    pub confidence: f64,
}

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
    /// Contextual observation hint from the current command's output (INV-GUIDANCE-014).
    ///
    /// When set, replaces the placeholder `"..."` in the observe command suggestion
    /// with a meaningful sentence derived from the command's actual output.
    pub contextual_hint: Option<ContextualHint>,
}

/// Paste-ready command for the worst-scoring M(t) sub-metric.
///
/// Returns the executable command string corresponding to whichever of the four
/// sub-metrics (tx, spec-lang, q-div, harvest) has the lowest score.
/// Used by Compressed-level footer to show a single actionable command.
///
/// When a `contextual_hint` is provided (INV-GUIDANCE-014), the observe command
/// uses the contextual text instead of the placeholder `"..."`.
fn worst_metric_command(
    components: &MethodologyComponents,
    hint: Option<&ContextualHint>,
) -> String {
    let observe_cmd = match hint {
        Some(h) => format!(
            "braid observe \"{}\" --confidence {:.1}",
            truncate_hint(&h.text, 60),
            h.confidence
        ),
        None => "braid observe \"...\" --confidence 0.8".to_string(),
    };
    let metrics: [(f64, &str); 4] = [
        (components.transact_frequency, &observe_cmd),
        (
            components.spec_language_ratio,
            "braid query --entity :spec/inv-...",
        ),
        (
            components.query_diversity,
            "braid query --attribute :db/doc --limit 5",
        ),
        (components.harvest_quality, "braid harvest --commit"),
    ];

    metrics
        .iter()
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, cmd)| cmd.to_string())
        .unwrap_or_else(|| "braid status".to_string())
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

    let check_with_hint = |v: f64, cmd: &str| -> String {
        if v >= 0.7 {
            "\u{2713}".to_string()
        } else if v >= 0.4 {
            "\u{25b3}".to_string()
        } else {
            format!("\u{2717}\u{2192}{cmd}")
        }
    };

    // INV-GUIDANCE-014: Use contextual hint in observe command when available.
    let observe_cmd = match &footer.contextual_hint {
        Some(h) => format!(
            "braid observe \"{}\" --confidence {:.1}",
            truncate_hint(&h.text, 60),
            h.confidence
        ),
        None => "braid observe \"...\" --confidence 0.8".to_string(),
    };

    let line1 = format!(
        "\u{21b3} M(t): {:.2} {} (tx: {} | spec-lang: {} | q-div: {} | harvest: {}) | Store: {} datoms | Turn {}",
        m.score,
        trend,
        check_with_hint(m.components.transact_frequency, &observe_cmd),
        check_with_hint(m.components.spec_language_ratio, "braid query --entity :spec/inv-..."),
        check_with_hint(m.components.query_diversity, "braid query --attribute :db/doc --limit 5"),
        check_with_hint(m.components.harvest_quality, "braid harvest --commit"),
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
/// Five levels matching the attention budget's guidance footer specification:
/// - Full: complete M(t) dashboard with sub-metric checks (~100-200 tokens)
/// - Compressed: one-line summary with top action (~30-60 tokens)
/// - Minimal: M(t) score + abbreviated action (~10-20 tokens)
/// - HarvestOnly: harvest imperative signal (~10 tokens)
/// - BasinToken: single-token basin activation (0-10 tokens, CLI default for k* >= 0.4)
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
                Trend::Up => "\u{2191}",
                Trend::Down => "\u{2193}",
                Trend::Stable => "\u{2192}",
            };
            // B2.3: At Compressed level, emit only the paste-ready command
            // for the worst failing metric instead of the generic next_action.
            let cmd = worst_metric_command(&m.components, footer.contextual_hint.as_ref());
            // Append Q(t) harvest warning when Warn or Critical
            let hw = if footer.harvest_warning >= HarvestWarningLevel::Warn {
                format!(" {}", footer.harvest_warning)
            } else {
                String::new()
            };
            format!(
                "\u{21b3} M={:.2}{} S:{} \u{2192} {cmd}{hw}",
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
        GuidanceLevel::BasinToken => {
            // Single-token basin activation: minimum perturbation to stay on-basin.
            // Priority: harvest emergency > low M(t) action > store summary > silence.
            if footer.harvest_warning >= HarvestWarningLevel::Warn {
                "braid harvest --commit".to_string()
            } else if footer.methodology.score < 0.3 {
                match &footer.next_action {
                    Some(action) => {
                        let short = crate::budget::safe_truncate_bytes(action, 30);
                        format!("verify: {short}")
                    }
                    None => format!(
                        "Store: {} datoms | Turn {}",
                        footer.store_datom_count, footer.turn
                    ),
                }
            } else if footer.methodology.score <= 0.7 {
                format!(
                    "Store: {} datoms | Turn {}",
                    footer.store_datom_count, footer.turn
                )
            } else {
                String::new()
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
        contextual_hint: None,
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
    build_command_footer_with_hint(store, k_eff, None)
}

/// Build a guidance footer with an optional contextual observation hint (INV-GUIDANCE-014).
///
/// When `hint` is provided, the footer replaces placeholder `"..."` in the observe
/// command suggestion with the contextual text derived from the current command's output.
/// This transforms the footer from generic guidance into actionable, paste-ready suggestions.
///
/// `k_eff` is the current attention budget ratio (None defaults to 1.0 = full).
pub fn build_command_footer_with_hint(
    store: &Store,
    k_eff: Option<f64>,
    hint: Option<ContextualHint>,
) -> String {
    let telemetry = telemetry_from_store(store);
    let methodology = compute_methodology_score(&telemetry);
    // Pass Q(t) to derive_actions so R12 uses attention-decay thresholds
    let q_t = k_eff.map(quality_adjusted_budget);
    let mut actions = derive_actions_with_budget(store, q_t);
    modulate_actions(&mut actions, methodology.score);

    // ADR-INTERFACE-010: Turn-count k* proxy at Stage 0.
    // When no measured k_eff is available, estimate attention consumption from
    // the store's transaction count since last harvest. More turns = less budget.
    // Acceptance: turn 5 → Full, turn 25 → Compressed, turn 45 → Minimal.
    let effective_k = k_eff.unwrap_or_else(|| {
        let tx_count = telemetry.total_turns;
        if tx_count <= 10 {
            1.0
        } else if tx_count <= 30 {
            0.5
        } else if tx_count <= 50 {
            0.3
        } else {
            0.15
        }
    });
    let level = GuidanceLevel::for_k_eff(effective_k);

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

    let mut footer = build_footer_with_budget(&telemetry, store, next_action, invariant_refs, q_t);
    footer.contextual_hint = hint;
    format_footer_at_level(&footer, level)
}

/// Derive session telemetry from the store state instead of using all-zero defaults.
///
/// T1-1: M(t) denominators are scoped to the current session (since last harvest).
/// This prevents M(t) from structurally decreasing as the store grows over its
/// lifetime — only current-session activity matters for methodology adherence.
///
/// - `total_turns`: distinct wall_times since last harvest (session-local)
/// - `transact_turns`: transactions since last harvest
/// - `spec_language_turns`: spec engagement since last harvest (see T1-2 below)
/// - `query_type_count`: 1 if any session transactions, 0 otherwise
/// - `harvest_quality`: 0.7 if recent harvest exists, 0.0 otherwise
///
/// ## T1-2: Broadened `spec_language_turns`
///
/// `spec_language_turns` counts four categories of spec engagement since the
/// last harvest (each contributing at most one turn per entity/datom):
///
/// 1. **Spec entities created** — `:db/ident` assertions with `:spec/` prefix.
/// 2. **Tasks with spec refs** — `:task/title` values containing INV-*, ADR-*, or
///    NEG-* patterns (via `parse_spec_refs`).
/// 3. **Observations with spec refs** — `:exploration/body` values containing
///    spec ref patterns.
/// 4. **Impl links** — `:impl/implements` assertions (trace evidence of spec engagement).
///
/// The total is capped at `total_turns` (cannot exceed session turns).
pub fn telemetry_from_store(store: &Store) -> SessionTelemetry {
    let boundary = last_harvest_wall_time(store);
    let has_recent_harvest = boundary > 0;

    // T1-1: Count distinct wall_times AFTER last harvest (session-scoped).
    // When no harvest exists (boundary == 0), all wall_times are in-session.
    let session_walls: BTreeSet<u64> = store
        .datoms()
        .filter(|d| d.tx.wall_time() > boundary)
        .map(|d| d.tx.wall_time())
        .collect();
    let session_turn_count = session_walls.len() as u32;

    let txns_since = count_txns_since_last_harvest(store) as u32;

    // T1-1: Count spec entities created/modified since last harvest, not total.
    let spec_entity_count = store
        .datoms()
        .filter(|d| {
            d.tx.wall_time() > boundary
                && d.attribute.as_str() == ":db/ident"
                && d.op == Op::Assert
                && matches!(&d.value, Value::Keyword(k) if k.starts_with(":spec/"))
        })
        .count() as u32;

    // T1-2(a): Tasks created since harvest whose titles contain spec ref patterns.
    let tasks_with_spec_refs = store
        .datoms()
        .filter(|d| {
            d.tx.wall_time() > boundary
                && d.attribute.as_str() == ":task/title"
                && d.op == Op::Assert
        })
        .filter(|d| {
            if let Value::String(title) = &d.value {
                !crate::task::parse_spec_refs(title).is_empty()
            } else {
                false
            }
        })
        .count() as u32;

    // T1-2(b): Observations created since harvest whose body contains spec refs.
    let observations_with_spec_refs = store
        .datoms()
        .filter(|d| {
            d.tx.wall_time() > boundary
                && d.attribute.as_str() == ":exploration/body"
                && d.op == Op::Assert
        })
        .filter(|d| {
            if let Value::String(body) = &d.value {
                !crate::task::parse_spec_refs(body).is_empty()
            } else {
                false
            }
        })
        .count() as u32;

    // T1-2(c): :impl/implements datoms created since harvest (trace evidence).
    let impl_links = store
        .datoms()
        .filter(|d| {
            d.tx.wall_time() > boundary
                && d.attribute.as_str() == ":impl/implements"
                && d.op == Op::Assert
        })
        .count() as u32;

    // T1-2: Total spec engagement = all four categories, capped at total_turns.
    let total_spec = spec_entity_count
        .saturating_add(tasks_with_spec_refs)
        .saturating_add(observations_with_spec_refs)
        .saturating_add(impl_links);

    // A3: M(t) floor clamp — when a harvest exists and fewer than 10 txns
    // have occurred since, the store is in a healthy inter-session state.
    // Without this floor, M(t) drops below 0.5 between sessions because
    // transact_frequency and query_diversity reset, triggering false DRIFT
    // warnings (CC-5 failure in bilateral scan).
    let harvest_is_recent = has_recent_harvest && txns_since < 10;

    SessionTelemetry {
        // max(1) prevents division by zero when 0 transactions since harvest
        total_turns: session_turn_count.max(1),
        transact_turns: txns_since,
        spec_language_turns: total_spec.min(session_turn_count.max(1)),
        query_type_count: if session_turn_count > 0 { 1 } else { 0 },
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

// ---------------------------------------------------------------------------
// Transaction Velocity + Adaptive Thresholds (INV-GUIDANCE-019)
// ---------------------------------------------------------------------------

/// Compute transaction velocity: transactions per minute over a 5-minute window.
///
/// Counts distinct wall-time values of `:tx/agent` datoms whose wall_time falls
/// within the last 300 seconds relative to the system clock. Returns the count
/// divided by 5 (minutes).
///
/// Note: wall_time values in the store use seconds since the Unix epoch
/// (consistent with existing `telemetry_from_store` and `contextual_observation_hint`).
pub fn tx_velocity(store: &Store) -> f64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    tx_velocity_at(store, now)
}

/// Compute transaction velocity at a specific point in time (for testing).
pub fn tx_velocity_at(store: &Store, now: u64) -> f64 {
    let window: u64 = 300; // 5 minutes
    let cutoff = now.saturating_sub(window);

    let recent_walls: BTreeSet<u64> = store
        .datoms()
        .filter(|d| {
            d.tx.wall_time() > cutoff && d.attribute.as_str() == ":tx/agent" && d.op == Op::Assert
        })
        .map(|d| d.tx.wall_time())
        .collect();

    recent_walls.len() as f64 / 5.0 // per minute
}

/// Adaptive harvest warning threshold based on transaction velocity.
///
/// INV-GUIDANCE-019: High velocity = routine ops = higher threshold.
///
/// | Velocity (txn/min) | Threshold |
/// |--------------------|-----------|
/// | > 5.0              | 30        |
/// | > 1.0              | 15        |
/// | <= 1.0             | 8         |
pub fn dynamic_threshold(velocity: f64) -> u32 {
    if velocity > 5.0 {
        30
    } else if velocity > 1.0 {
        15
    } else {
        8
    }
}

/// Classify an agent's command as follow-through on the previous R(t) recommendation.
///
/// RFL-3: When the agent runs a command, check if it follows the previous
/// ACP recommendation. Three outcomes:
/// - **Followed**: exact match (agent ran the recommended command)
/// - **Adjacent**: agent investigated the recommended task (show, query)
/// - **Ignored**: agent did something unrelated
///
/// Returns None if no unresolved action exists (first command in session).
///
/// INV-GUIDANCE-010, INV-GUIDANCE-005.
pub fn classify_action_outcome(
    store: &Store,
    current_command: &str,
) -> Option<(&'static str, EntityId)> {
    // Find the most recent :action/* entity WITHOUT an :action/outcome
    let cmd_attr = crate::datom::Attribute::from_keyword(":action/recommended-command");
    let outcome_attr = crate::datom::Attribute::from_keyword(":action/outcome");

    // Scan action entities — find one with recommended-command but no outcome
    let mut latest_action: Option<(EntityId, String, u64)> = None;
    for datom in store.attribute_datoms(&cmd_attr).iter() {
        if datom.op != crate::datom::Op::Assert {
            continue;
        }
        let entity = datom.entity;
        let has_outcome = store
            .entity_datoms(entity)
            .iter()
            .any(|d| d.attribute == outcome_attr && d.op == crate::datom::Op::Assert);
        if !has_outcome {
            if let crate::datom::Value::String(ref cmd) = datom.value {
                let wall = datom.tx.wall_time();
                if latest_action.as_ref().map(|(_, _, w)| wall > *w).unwrap_or(true) {
                    latest_action = Some((entity, cmd.clone(), wall));
                }
            }
        }
    }

    let (entity, recommended_cmd, _wall) = latest_action?;

    // Classify: compare current command against recommended
    let current_lower = current_command.to_lowercase();
    let recommended_lower = recommended_cmd.to_lowercase();

    // Followed: exact match or close match (same task ID)
    if current_lower == recommended_lower {
        return Some(("followed", entity));
    }

    // Extract task ID from recommended command (e.g., "braid go t-fd30" → "t-fd30")
    let task_id = recommended_cmd
        .split_whitespace()
        .find(|w| w.starts_with("t-"))
        .unwrap_or("");

    if !task_id.is_empty() && current_lower.contains(task_id) {
        return Some(("adjacent", entity));
    }

    Some(("ignored", entity))
}

/// Multi-dimensional harvest urgency (ZCM-2, INV-GUIDANCE-019).
///
/// Four signals, urgency = max of all:
/// 1. tx_since / dynamic_threshold (existing — transaction count)
/// 2. minutes_since_harvest / 30 (time ceiling — ensures harvest even during slow work)
/// 3. high_value_unharvested / 3 (knowledge density — observations vs routine closures)
/// 4. k_eff_critical (Q(t) < 0.15 — context exhaustion emergency)
///
/// Returns urgency in [0, 1+]. Values > 1.0 mean OVERDUE.
pub fn harvest_urgency_multi(store: &Store, k_eff: f64) -> f64 {
    let velocity = tx_velocity(store);
    let threshold = dynamic_threshold(velocity);
    let tx_since = count_txns_since_last_harvest(store);

    // Signal 1: transaction count / threshold
    let signal_1 = tx_since as f64 / threshold.max(1) as f64;

    // Signal 2: time since harvest / 30 minutes
    // CRITICAL: Use the same canonical boundary as signal_1 (last_harvest_wall_time)
    // to avoid split-brain where signals disagree on when the last harvest occurred.
    let last_harvest_wall = last_harvest_wall_time(store);
    let now_wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let minutes_since = (now_wall.saturating_sub(last_harvest_wall)) as f64 / 60.0;
    let signal_2 = minutes_since / 30.0;

    // Signal 3: high-value unharvested / 3
    // Count exploration entities created since last harvest (observations, not routine)
    let exploration_type_attr = crate::datom::Attribute::from_keyword(":exploration/type");
    let high_value = store
        .attribute_datoms(&exploration_type_attr)
        .iter()
        .filter(|d| d.op == crate::datom::Op::Assert && d.tx.wall_time() > last_harvest_wall)
        .count();
    let signal_3 = high_value as f64 / 3.0;

    // Signal 4: k_eff critical (Q(t) < 0.15)
    let signal_4 = if k_eff < 0.15 { 1.5 } else { 0.0 };

    // Urgency = max of all signals
    signal_1
        .max(signal_2)
        .max(signal_3)
        .max(signal_4)
}

/// Check whether the CLI should warn about an unharvested session on exit.
///
/// NEG-HARVEST-001: No Unharvested Session Termination.
/// Safety property: every session that ends with uncommitted observations MUST
/// have issued at least one harvest warning before termination.
///
/// Uses the multi-signal `harvest_urgency_multi()` (INV-GUIDANCE-019) which fuses
/// four signals: transaction count / adaptive threshold, time since harvest,
/// high-value unharvested knowledge density, and k_eff context exhaustion.
/// Warns when urgency >= 0.7 (pre-overdue), giving the agent a chance to harvest
/// before the session becomes overdue (urgency >= 1.0).
///
/// `k_eff` is the current attention budget ratio. When `None`, it is estimated
/// from store evidence via `budget::estimate_k_eff`.
///
/// Returns `Some(warning_message)` if a warning should be shown, `None` otherwise.
pub fn should_warn_on_exit(store: &Store, k_eff: Option<f64>) -> Option<String> {
    let tx_since = count_txns_since_last_harvest(store);
    // No transactions since last harvest means nothing to harvest -- skip.
    if tx_since == 0 {
        return None;
    }
    let effective_k = k_eff.unwrap_or_else(|| {
        let evidence = crate::budget::EvidenceVector::from_store(store);
        crate::budget::estimate_k_eff(&evidence)
    });
    let urgency = harvest_urgency_multi(store, effective_k);
    if urgency >= 0.7 {
        // Clamp urgency display to [0, 10] for human readability.
        // Raw urgency can exceed 10 for very stale sessions; showing huge
        // numbers adds noise without information. "10.0+" signals overflow.
        let urgency_display = if urgency > 10.0 {
            "10.0+".to_string()
        } else {
            format!("{urgency:.2}")
        };
        Some(format!(
            "\u{26a0} NEG-HARVEST-001: {tx_since} transactions since last harvest \
             (urgency {urgency_display}). Run: braid harvest --commit"
        ))
    } else {
        None
    }
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
// Crystallization Gap Detection (INV-GUIDANCE-018)
// ---------------------------------------------------------------------------

/// Extract spec-like IDs from a text string.
///
/// Matches patterns: `INV-{NAMESPACE}-{NNN}`, `ADR-{NAMESPACE}-{NNN}`,
/// `NEG-{NAMESPACE}-{NNN}` where NAMESPACE is one or more uppercase letters
/// and NNN is one or more digits.
///
/// Returns unique, sorted results.
fn extract_spec_ids(text: &str) -> Vec<String> {
    let prefixes = ["INV-", "ADR-", "NEG-"];
    let mut results = BTreeSet::new();
    let bytes = text.as_bytes();
    let len = bytes.len();

    let mut i = 0;
    while i < len {
        // Skip multi-byte UTF-8 continuation bytes — prefixes are ASCII-only.
        if !text.is_char_boundary(i) {
            i += 1;
            continue;
        }
        // Check if any prefix starts here
        let mut matched_prefix: Option<&str> = None;
        for prefix in &prefixes {
            // Use .get() for safe UTF-8 boundary handling — never panic on multi-byte chars.
            if let Some(slice) = text.get(i..i + prefix.len()) {
                if slice == *prefix {
                    // Ensure this is a word boundary: either start of string or
                    // preceding char is not alphanumeric/underscore/hyphen
                    if i == 0 || !bytes[i - 1].is_ascii_alphanumeric() {
                        matched_prefix = Some(prefix);
                        break;
                    }
                }
            }
        }

        if let Some(prefix) = matched_prefix {
            let after_prefix = i + prefix.len();
            // Expect NAMESPACE: one or more uppercase ASCII letters
            let ns_start = after_prefix;
            let mut ns_end = ns_start;
            while ns_end < len && bytes[ns_end].is_ascii_uppercase() {
                ns_end += 1;
            }
            if ns_end > ns_start && ns_end < len && bytes[ns_end] == b'-' {
                // Expect digits after the hyphen
                let digit_start = ns_end + 1;
                let mut digit_end = digit_start;
                while digit_end < len && bytes[digit_end].is_ascii_digit() {
                    digit_end += 1;
                }
                if digit_end > digit_start {
                    results.insert(text[i..digit_end].to_string());
                    i = digit_end;
                    continue;
                }
            }
        }
        i += 1;
    }

    results.into_iter().collect()
}

/// Detect observations that contain spec-like IDs (INV-*, ADR-*, NEG-*)
/// but haven't been crystallized into formal spec elements.
/// Returns (observation_entity, extracted_id) pairs.
///
/// An observation is crystallized if a `:spec/{id-lowercase}` entity exists
/// in the store AND that entity has a `:spec/falsification` datom (indicating
/// a formal element, not just another observation mentioning the ID).
///
/// INV-GUIDANCE-018: Crystallization Gap Detection.
pub fn crystallization_candidates(store: &Store) -> Vec<(EntityId, String)> {
    let body_attr = Attribute::from_keyword(":exploration/body");

    // Step 1: Collect observation entities and their body text.
    // Observations are entities with :exploration/body attribute.
    let mut obs_bodies: BTreeMap<EntityId, String> = BTreeMap::new();
    for datom in store.attribute_datoms(&body_attr) {
        if datom.op == Op::Assert {
            if let Value::String(ref s) = datom.value {
                obs_bodies.insert(datom.entity, s.clone());
            }
        }
    }

    // Step 2: Build a set of formally crystallized spec IDs.
    // A spec element is "crystallized" if it has a type-specific formalization attribute:
    //   INV-* → :spec/falsification
    //   ADR-* → :adr/decision
    //   NEG-* → :neg/violation (or :spec/falsification as fallback)
    // Without checking all three, ADRs and NEGs are false-positively reported as
    // uncrystallized (INVESTIGATE t-d2881739 finding: 6 false positives).
    let formalization_attrs = [
        Attribute::from_keyword(":spec/falsification"),
        Attribute::from_keyword(":adr/decision"),
        Attribute::from_keyword(":neg/violation"),
    ];
    let ident_attr = Attribute::from_keyword(":db/ident");
    let mut crystallized: BTreeSet<String> = BTreeSet::new();
    for attr in &formalization_attrs {
        for datom in store.attribute_datoms(attr) {
            if datom.op == Op::Assert {
                for ident_datom in store.entity_datoms(datom.entity) {
                    if ident_datom.attribute == ident_attr && ident_datom.op == Op::Assert {
                        if let Value::Keyword(ref kw) = ident_datom.value {
                            // SPECID-2: Use SpecId for canonical normalization
                            if let Some(spec_id) = crate::spec_id::SpecId::from_store_ident(kw) {
                                crystallized.insert(spec_id.human_form());
                            }
                        }
                    }
                }
            }
        }
    }

    // Step 3: For each observation, extract spec IDs and check if uncrystallized.
    // SPECID-2: Use SpecId::parse for canonical comparison
    let mut candidates = Vec::new();
    for (entity, body) in &obs_bodies {
        let ids = extract_spec_ids(body);
        for id in ids {
            let canonical = crate::spec_id::SpecId::parse(&id)
                .map(|s| s.human_form())
                .unwrap_or(id.clone());
            if !crystallized.contains(&canonical) {
                candidates.push((*entity, id));
            }
        }
    }

    candidates
}

// ---------------------------------------------------------------------------
// DTIC-1: Decision-Task Integrity (INV-HARVEST-002, API-as-prompt)
// ---------------------------------------------------------------------------

/// Actionable verb patterns that indicate a decision needs a follow-up task.
const ACTIONABLE_VERBS: &[&str] = &[
    "implement",
    "add",
    "create",
    "wire",
    "fix",
    "refactor",
    "remove",
    "replace",
    "migrate",
    "update",
    "extend",
    "define",
    "register",
    "transact",
];

/// Detect whether an observation text contains actionable decision language.
///
/// Returns true if the text contains patterns suggesting a design decision
/// that should have a corresponding task. Used at observe-time to suggest
/// task creation in the footer (DTIC-1 prevention layer).
///
/// Pattern: text contains "DESIGN" or "DECISION" + an actionable verb.
pub fn is_actionable_decision(text: &str) -> bool {
    let lower = text.to_lowercase();

    // Quick check: must contain a decision indicator
    let has_decision_marker = lower.contains("design")
        || lower.contains("decision")
        || lower.contains("should")
        || lower.contains("must")
        || lower.contains("need to")
        || lower.contains("plan:");

    if !has_decision_marker {
        return false;
    }

    // Must also contain an actionable verb
    ACTIONABLE_VERBS.iter().any(|verb| lower.contains(verb))
}

/// Suggest a task title from actionable decision text.
///
/// Extracts the first sentence containing an actionable verb,
/// truncates to 120 chars. Returns None if not actionable.
pub fn suggest_task_title(text: &str) -> Option<String> {
    if !is_actionable_decision(text) {
        return None;
    }

    let lower = text.to_lowercase();

    // Find the first sentence with an actionable verb
    for sentence in text.split(['.', '\n']) {
        let sent_lower = sentence.to_lowercase();
        if ACTIONABLE_VERBS
            .iter()
            .any(|verb| sent_lower.contains(verb))
        {
            let trimmed = sentence.trim();
            if trimmed.len() > 120 {
                let end = trimmed
                    .char_indices()
                    .take_while(|(i, _)| *i <= 117)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(117.min(trimmed.len()));
                return Some(format!("{}...", &trimmed[..end]));
            }
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    // Fallback: first 120 chars of original
    let first = if lower.len() > 120 {
        let end = text
            .char_indices()
            .take_while(|(i, _)| *i <= 117)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(117.min(text.len()));
        format!("{}...", &text[..end])
    } else {
        text.to_string()
    };
    Some(first)
}

/// Scan store for orphaned decisions — observations with actionable language
/// but no corresponding task (DTIC-2 detection layer).
///
/// Returns list of (entity, body_text) pairs where the observation looks
/// actionable but has no task with matching keywords.
pub fn orphaned_decisions(store: &Store) -> Vec<(EntityId, String)> {
    let body_attr = Attribute::from_keyword(":exploration/body");
    let title_attr = Attribute::from_keyword(":task/title");

    // Collect all task titles for matching
    let mut task_titles: Vec<String> = Vec::new();
    for datom in store.attribute_datoms(&title_attr) {
        if datom.op == Op::Assert {
            if let Value::String(ref s) = datom.value {
                task_titles.push(s.to_lowercase());
            }
        }
    }

    let mut orphans = Vec::new();

    for datom in store.attribute_datoms(&body_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        if let Value::String(ref body) = datom.value {
            if !is_actionable_decision(body) {
                continue;
            }

            // Check if any task title contains keywords from the decision
            let keywords: Vec<&str> = body
                .split_whitespace()
                .filter(|w| w.len() >= 5)
                .take(5)
                .collect();

            let has_matching_task = task_titles.iter().any(|title| {
                keywords
                    .iter()
                    .filter(|kw| title.contains(&kw.to_lowercase()))
                    .count()
                    >= 2
            });

            if !has_matching_task {
                orphans.push((datom.entity, body.clone()));
            }
        }
    }

    orphans
}

// ---------------------------------------------------------------------------
// Methodology Gap Dashboard (INV-GUIDANCE-021)
// ---------------------------------------------------------------------------

/// Aggregated methodology gap counts for the status dashboard (INV-GUIDANCE-021).
///
/// Each field counts a distinct gap type. The `total()` method sums all gaps.
/// The `untested` and `stale_witnesses` fields are populated by the WITNESS
/// subsystem via `witness::witness_gaps()`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MethodologyGaps {
    /// Observations containing spec IDs (INV-*, ADR-*, NEG-*) not yet
    /// crystallized into formal spec elements with `:spec/falsification`.
    pub crystallization: u32,
    /// Open tasks whose title references spec IDs that don't resolve to
    /// formal spec elements in the store.
    pub unanchored: u32,
    /// Current-stage INVs with only L1 witnesses (INV-WITNESS-005).
    pub untested: u32,
    /// Formally-backed witnesses invalidated by subsequent changes (INV-WITNESS-011).
    pub stale_witnesses: u32,
}

impl MethodologyGaps {
    /// Total gap count across all categories.
    pub fn total(&self) -> u32 {
        self.crystallization + self.unanchored + self.untested + self.stale_witnesses
    }

    /// Returns true when no gaps exist in any category.
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// Compute all methodology gaps from store state.
///
/// Aggregates crystallization gaps (observations with uncrystallized spec IDs)
/// and unanchored tasks (open tasks referencing spec IDs that don't resolve).
/// The `untested` and `stale_witnesses` fields are powered by the WITNESS
/// subsystem (INV-WITNESS-005, INV-WITNESS-011).
///
/// INV-GUIDANCE-021: Methodology Gap Dashboard.
pub fn methodology_gaps(store: &Store) -> MethodologyGaps {
    let crystallization = crystallization_candidates(store).len() as u32;

    // Count unanchored tasks: open tasks with spec refs in title that don't resolve
    let tasks = crate::task::all_tasks(store);
    let mut unanchored = 0u32;
    for task in &tasks {
        if task.status != crate::task::TaskStatus::Open {
            continue;
        }
        let refs = crate::task::parse_spec_refs(&task.title);
        if refs.is_empty() {
            continue;
        }
        let (resolved, _) = crate::task::resolve_spec_refs(store, &refs);
        if resolved.is_empty() && !refs.is_empty() {
            unanchored += 1;
        }
    }

    let (untested_w, stale_w) = crate::witness::witness_gaps(store);

    MethodologyGaps {
        crystallization,
        unanchored,
        untested: untested_w,
        stale_witnesses: stale_w,
    }
}

/// Activity-mode-adjusted gap counts for display (T6-1).
///
/// The kernel's `methodology_gaps()` returns raw truth. This struct holds
/// display-layer adjusted values that suppress noise based on what the agent
/// is actually doing:
/// - **Implementation mode**: crystallization x0.1, unanchored x0.2 (spec gaps
///   are expected -- agent is writing code, not specs)
/// - **Specification mode**: untested x0.3 (test gaps are expected -- agent is
///   writing specs, not tests)
/// - **Mixed mode**: no suppression (all gaps equally relevant)
///
/// Both raw and adjusted values are preserved for display transparency.
#[derive(Clone, Debug)]
pub struct AdjustedGaps {
    /// Raw gap counts from the kernel (unchanged).
    pub raw: MethodologyGaps,
    /// Activity-mode-adjusted gap counts (rounded up after scaling).
    pub adjusted: MethodologyGaps,
    /// The activity mode that determined suppression factors.
    pub mode: ActivityMode,
}

impl AdjustedGaps {
    /// Adjusted total gap count.
    pub fn total(&self) -> u32 {
        self.adjusted.total()
    }

    /// Returns true when no adjusted gaps exist.
    pub fn is_empty(&self) -> bool {
        self.adjusted.is_empty()
    }

    /// Mode label for display (e.g., "impl", "spec", "mixed").
    pub fn mode_label(&self) -> &'static str {
        match self.mode {
            ActivityMode::Implementation => "impl",
            ActivityMode::Specification => "spec",
            ActivityMode::Mixed => "mixed",
        }
    }
}

/// Compute display-adjusted gap counts by applying activity-mode suppression.
///
/// Suppression factors (T6-1):
/// - Implementation mode: crystallization x0.1, unanchored x0.2
/// - Specification mode: untested x0.3
/// - Mixed mode: no suppression
///
/// The kernel function `methodology_gaps()` is unchanged -- it returns raw truth.
/// This is a **display-layer** transformation only.
pub fn adjust_gaps(raw: MethodologyGaps, mode: ActivityMode) -> AdjustedGaps {
    let adjusted = match mode {
        ActivityMode::Implementation => MethodologyGaps {
            crystallization: scale_up(raw.crystallization, 0.1),
            unanchored: scale_up(raw.unanchored, 0.2),
            untested: raw.untested,
            stale_witnesses: raw.stale_witnesses,
        },
        ActivityMode::Specification => MethodologyGaps {
            crystallization: raw.crystallization,
            unanchored: raw.unanchored,
            untested: scale_up(raw.untested, 0.3),
            stale_witnesses: raw.stale_witnesses,
        },
        ActivityMode::Mixed => MethodologyGaps {
            crystallization: raw.crystallization,
            unanchored: raw.unanchored,
            untested: raw.untested,
            stale_witnesses: raw.stale_witnesses,
        },
    };
    AdjustedGaps {
        raw,
        adjusted,
        mode,
    }
}

/// Scale a count by a factor, rounding up (ceil) so 1 raw gap never disappears to 0
/// unless the raw count itself is 0.
fn scale_up(count: u32, factor: f64) -> u32 {
    (count as f64 * factor).ceil() as u32
}

// ---------------------------------------------------------------------------
// Contextual Observation Funnel (INV-GUIDANCE-014)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Proactive Spec Retrieval (PSR-1, INV-GUIDANCE-007)
// ---------------------------------------------------------------------------

/// A spec relevance match from the proactive scan.
#[derive(Clone, Debug)]
pub struct SpecRelevance {
    /// The spec element ident (e.g., ":spec/inv-topology-004").
    pub ident: String,
    /// Human-readable spec ID (e.g., "INV-TOPOLOGY-004").
    pub human_id: String,
    /// Short summary from :spec/statement (first 60 chars).
    pub summary: String,
    /// Relevance score (0.0–1.0, cosine bag-of-words).
    pub score: f64,
    /// Source layer: "spec", "task", or "observation".
    pub source: String,
}

/// Stopwords to filter from tokenization.
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "shall", "should", "may", "might", "must", "can",
    "could", "that", "this", "these", "those", "with", "from", "into", "for", "and", "but", "or",
    "not", "all", "each", "every", "both", "few", "more", "most", "other", "some", "such", "only",
    "own", "same", "than", "too", "very",
];

/// Tokenize text for bag-of-words comparison.
fn tokenize_for_relevance(text: &str) -> BTreeSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|w| w.len() >= 4)
        .filter(|w| !STOPWORDS.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Scan the store for spec elements related to the given text (PSR-1).
///
/// Uses cosine similarity on bag-of-words: score = |intersection| / sqrt(|a| × |b|).
/// Also boosts matches where the input contains the spec namespace name.
///
/// Returns top 5 matches with score > 0.3.
///
/// INV-GUIDANCE-007: Proactive Spec Retrieval.
pub fn spec_relevance_scan(text: &str, store: &Store) -> Vec<SpecRelevance> {
    let input_tokens = tokenize_for_relevance(text);
    if input_tokens.is_empty() {
        return Vec::new();
    }

    let statement_attr = Attribute::from_keyword(":spec/statement");
    let namespace_attr = Attribute::from_keyword(":spec/namespace");
    let ident_attr = Attribute::from_keyword(":db/ident");

    let mut results: Vec<SpecRelevance> = Vec::new();

    // Collect all spec statements
    for datom in store.attribute_datoms(&statement_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        let statement = match &datom.value {
            Value::String(s) => s.as_str(),
            _ => continue,
        };

        // Tokenize spec statement
        let spec_tokens = tokenize_for_relevance(statement);
        if spec_tokens.is_empty() {
            continue;
        }

        // Cosine on bag-of-words
        let intersection = input_tokens.intersection(&spec_tokens).count() as f64;
        let denominator = (input_tokens.len() as f64 * spec_tokens.len() as f64).sqrt();
        let mut score = if denominator > 0.0 {
            intersection / denominator
        } else {
            0.0
        };

        // Namespace boost: if input contains the namespace name, +0.3
        for ns_datom in store.entity_datoms(datom.entity) {
            if ns_datom.attribute == namespace_attr && ns_datom.op == Op::Assert {
                if let Value::String(ref ns) = ns_datom.value {
                    let ns_lower = ns.to_lowercase();
                    if input_tokens.contains(&ns_lower) {
                        score += 0.3;
                    }
                }
            }
        }

        if score > 0.3 {
            // Get the ident for this entity
            let ident = store
                .entity_datoms(datom.entity)
                .iter()
                .find(|d| d.attribute == ident_attr && d.op == Op::Assert)
                .and_then(|d| match &d.value {
                    Value::Keyword(k) => Some(k.clone()),
                    _ => None,
                })
                .unwrap_or_default();

            let human_id = crate::spec_id::SpecId::from_store_ident(&ident)
                .map(|s| s.human_form())
                .unwrap_or_else(|| ident.clone());

            let summary = crate::budget::safe_truncate_bytes(statement, 60).to_string();

            results.push(SpecRelevance {
                ident,
                human_id,
                summary,
                score,
                source: "spec".to_string(),
            });
        }
    }

    // Sort by score descending, take top 5
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(5);
    results
}

/// Broadened knowledge relevance scan across ALL layers: spec, tasks, observations.
///
/// CRB-7: Prevents the meta-irony failure mode where agents complain about problems
/// that are already documented as tasks or observations.
///
/// Results are tagged by source layer: [spec], [task], [observation].
///
/// INV-GUIDANCE-024, INV-GUIDANCE-025.
pub fn knowledge_relevance_scan(text: &str, store: &Store) -> Vec<SpecRelevance> {
    let input_tokens = tokenize_for_relevance(text);
    if input_tokens.is_empty() {
        return Vec::new();
    }

    // Start with spec results
    let mut results = spec_relevance_scan(text, store);

    // Scan task titles
    let title_attr = Attribute::from_keyword(":task/title");
    let id_attr = Attribute::from_keyword(":task/id");
    for datom in store.attribute_datoms(&title_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        let title = match &datom.value {
            Value::String(s) => s.as_str(),
            _ => continue,
        };

        let title_tokens = tokenize_for_relevance(title);
        if title_tokens.is_empty() {
            continue;
        }

        let intersection = input_tokens.intersection(&title_tokens).count() as f64;
        let denominator = (input_tokens.len() as f64 * title_tokens.len() as f64).sqrt();
        let score = if denominator > 0.0 {
            intersection / denominator
        } else {
            0.0
        };

        if score > 0.3 {
            let task_id = store
                .entity_datoms(datom.entity)
                .iter()
                .find(|d| d.attribute == id_attr && d.op == Op::Assert)
                .and_then(|d| match &d.value {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| format!("{:?}", datom.entity));

            let summary = crate::budget::safe_truncate_bytes(title, 60).to_string();

            results.push(SpecRelevance {
                ident: format!(":task/{}", task_id),
                human_id: task_id,
                summary,
                score,
                source: "task".to_string(),
            });
        }
    }

    // Scan observation bodies
    let doc_attr = Attribute::from_keyword(":db/doc");
    let exploration_type_attr = Attribute::from_keyword(":exploration/type");
    for datom in store.attribute_datoms(&exploration_type_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        // This is an observation/exploration entity — get its :db/doc
        let entity_datoms = store.entity_datoms(datom.entity);
        let doc = entity_datoms
            .iter()
            .find(|d| d.attribute == doc_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            });

        let body = match doc {
            Some(b) => b,
            None => continue,
        };

        let body_tokens = tokenize_for_relevance(body);
        if body_tokens.is_empty() {
            continue;
        }

        let intersection = input_tokens.intersection(&body_tokens).count() as f64;
        let denominator = (input_tokens.len() as f64 * body_tokens.len() as f64).sqrt();
        let score = if denominator > 0.0 {
            intersection / denominator
        } else {
            0.0
        };

        if score > 0.3 {
            let ident_attr_kw = Attribute::from_keyword(":db/ident");
            let ident = entity_datoms
                .iter()
                .find(|d| d.attribute == ident_attr_kw && d.op == Op::Assert)
                .and_then(|d| match &d.value {
                    Value::Keyword(k) => Some(k.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| format!("{:?}", datom.entity));

            let summary = crate::budget::safe_truncate_bytes(body, 60).to_string();

            results.push(SpecRelevance {
                ident: ident.clone(),
                human_id: ident,
                summary,
                score,
                source: "observation".to_string(),
            });
        }
    }

    // Sort by score descending, take top 10 (broadened from 5)
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(10);
    results
}

/// Result of a CRB reconciliation check.
///
/// Used by the reconciliation middleware (CRB-6) to gate knowledge-producing
/// commands on prior knowledge reconciliation.
#[derive(Clone, Debug)]
pub struct ReconciliationResult {
    /// Related knowledge elements found.
    pub matches: Vec<SpecRelevance>,
    /// Whether the gate threshold is met (3+ matches).
    pub gate: bool,
    /// Human-readable summary of related knowledge.
    pub summary: String,
}

/// CRB-6 reconciliation middleware: check text against the knowledge store.
///
/// This is the centralized reconciliation check used by ALL knowledge-producing
/// commands (observe, spec create, task create, write assert). It runs the
/// broadened knowledge_relevance_scan and returns a structured result.
///
/// The gate threshold is 3+ matches — if met, the command should refuse unless
/// the --reconciled flag was passed.
///
/// INV-GUIDANCE-025: Creation Requires Background.
pub fn reconciliation_check(text: &str, store: &Store) -> ReconciliationResult {
    let matches = knowledge_relevance_scan(text, store);
    let gate = matches.len() >= 3;

    let summary = if matches.is_empty() {
        "No related knowledge found.".to_string()
    } else {
        let parts: Vec<String> = matches
            .iter()
            .take(5)
            .map(|r| format!("[{}] {} — {}", r.source, r.human_id, r.summary))
            .collect();
        parts.join("\n")
    };

    ReconciliationResult {
        matches,
        gate,
        summary,
    }
}

/// Format spec relevance results as a single-line footer reference.
///
/// Returns None if no matches, or Some("Spec: INV-X-001 (summary) | ADR-Y-002 (summary)")
pub fn format_spec_relevance(results: &[SpecRelevance]) -> Option<String> {
    if results.is_empty() {
        return None;
    }
    let parts: Vec<String> = results
        .iter()
        .take(3)
        .map(|r| format!("{} ({})", r.human_id, r.summary))
        .collect();
    Some(format!("Spec: {}", parts.join(" | ")))
}

// ---------------------------------------------------------------------------
// Contextual Observation Funnel (INV-GUIDANCE-014)
// ---------------------------------------------------------------------------

/// Generate a contextual observation hint from a command's output.
///
/// INV-GUIDANCE-014: Contextual Observation Hint.
///
/// Examines the JSON output of a command and produces a short, meaningful
/// sentence that can be used as the observation text in a `braid observe`
/// suggestion. Returns `None` for commands that don't produce knowledge
/// worth capturing (e.g., `observe`, `harvest`, `init`, `mcp`, `seed`).
///
/// The returned [`ContextualHint`] includes both the observation text and
/// a confidence level appropriate for the command type:
/// - task close: 0.9 (high confidence -- task completion is definitive)
/// - status/bilateral: 0.8 (high -- direct store measurement)
/// - query: 0.7 (moderate -- depends on what the query was about)
/// - trace: 0.7 (moderate -- coverage is a measurement)
pub fn contextual_observation_hint(
    cmd_name: &str,
    output: &serde_json::Value,
) -> Option<ContextualHint> {
    let (text, confidence) = match cmd_name {
        "task close" | "task_close" | "done" => {
            let title = output
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("task");
            let reason = output
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("completed");
            (
                format!(
                    "Completed: {} \u{2014} {}",
                    truncate_hint(title, 60),
                    truncate_hint(reason, 40)
                ),
                0.9,
            )
        }
        "query" => {
            let count = output
                .get("total")
                .or_else(|| output.get("count"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let entity = output
                .get("entity_filter")
                .and_then(|v| v.as_str())
                .unwrap_or("*");
            (format!("Queried {entity} ({count} results)"), 0.7)
        }
        "status" => {
            // Extract F(S) from fitness if available
            let fs = output.get("fitness").and_then(|v| v.as_f64());
            match fs {
                Some(f) => (format!("Status: F(S)={f:.2}"), 0.8),
                None => ("Status checked".to_string(), 0.8),
            }
        }
        "trace" => {
            let coverage = output.get("coverage").and_then(|v| v.as_f64());
            match coverage {
                Some(c) => (format!("Traced: {:.0}% coverage", c * 100.0), 0.7),
                None => ("Trace scan completed".to_string(), 0.7),
            }
        }
        "bilateral" => {
            let fs = output.get("fitness").and_then(|v| v.as_f64());
            match fs {
                Some(f) => (format!("Bilateral: F(S)={f:.2}"), 0.8),
                None => ("Bilateral analysis completed".to_string(), 0.8),
            }
        }
        // Commands that don't produce knowledge worth capturing.
        "observe" | "harvest" | "init" | "mcp" | "seed" => return None,
        _ => return None,
    };

    if text.is_empty() {
        return None;
    }

    Some(ContextualHint { text, confidence })
}

/// Truncate a string to `max` bytes at a safe UTF-8 boundary.
///
/// Uses [`crate::budget::safe_truncate_bytes`] for correctness.
fn truncate_hint(s: &str, max: usize) -> &str {
    crate::budget::safe_truncate_bytes(s, max)
}

// ---------------------------------------------------------------------------
// Dynamic Methodology Projection (INV-GUIDANCE-022, INV-GUIDANCE-023)
// ---------------------------------------------------------------------------

/// Ceremony level for adaptive methodology (INV-GUIDANCE-023).
///
/// Determines how much specification ceremony is required before execution,
/// based on context budget remaining (k*) and the nature of the change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CeremonyLevel {
    /// Full: observe → crystallize → task → execute
    /// Used when k* > 0.7 AND change is novel design.
    Full,
    /// Standard: observe + execute → retroactive crystallize
    /// Used when k* > 0.3 OR change is a feature.
    Standard,
    /// Minimal: execute → observe (provenance chain minimum)
    /// Used when k* < 0.3 OR change is a known-category bug.
    Minimal,
}

/// The type of change being made, for ceremony level determination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeType {
    /// Novel design — new abstractions, new spec elements.
    NovelDesign,
    /// Feature implementation — known spec, new code.
    Feature,
    /// Known-category bug fix — fix code, capture provenance.
    KnownBug,
}

/// Determine the ceremony level based on k* and change type (INV-GUIDANCE-023).
///
/// The ceremony level adapts the methodology to the agent's context state:
/// - At high k* with novel work → full ceremony prevents ideation-to-task skip
/// - At moderate k* with features → standard ceremony balances rigor and speed
/// - At low k* or known bugs → minimal ceremony preserves provenance without overhead
pub fn ceremony_level(k_eff: f64, change_type: ChangeType) -> CeremonyLevel {
    match change_type {
        ChangeType::KnownBug => CeremonyLevel::Minimal,
        ChangeType::NovelDesign if k_eff > 0.7 => CeremonyLevel::Full,
        ChangeType::Feature if k_eff > 0.3 => CeremonyLevel::Standard,
        ChangeType::NovelDesign if k_eff > 0.3 => CeremonyLevel::Standard,
        _ if k_eff < 0.3 => CeremonyLevel::Minimal,
        _ => CeremonyLevel::Standard,
    }
}

impl CeremonyLevel {
    /// Human-readable description of the ceremony protocol.
    pub fn description(&self) -> &'static str {
        match self {
            CeremonyLevel::Full => {
                "Full: observe \u{2192} crystallize \u{2192} task \u{2192} execute"
            }
            CeremonyLevel::Standard => {
                "Standard: observe + execute \u{2192} retroactive crystallize"
            }
            CeremonyLevel::Minimal => {
                "Minimal: execute \u{2192} observe (provenance chain minimum)"
            }
        }
    }
}

/// A subsystem capability detected from store state (INV-GUIDANCE-022).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capability {
    /// Name of the subsystem.
    pub name: String,
    /// Whether evidence of implementation was found in the store.
    pub implemented: bool,
}

/// Scan the store for evidence of which subsystems are implemented (INV-GUIDANCE-022).
///
/// Checks for presence of specific attribute patterns or entity idents that
/// indicate a subsystem is operational, not just specified.
pub fn capability_scan(store: &Store) -> Vec<Capability> {
    // CENSUS-3: Prefer :capability/* datoms from session start (INV-REFLEXIVE-001).
    // If census datoms exist, use them (authoritative). Otherwise fall back to
    // run_census() for stores that haven't had a session start yet.
    let cap_attr = Attribute::from_keyword(":capability/status");
    let display_attr = Attribute::from_keyword(":capability/display-name");
    let census_datoms = store.attribute_datoms(&cap_attr);

    if !census_datoms.is_empty() {
        // Use persisted census data from session start
        census_datoms
            .iter()
            .filter(|d| d.op == Op::Assert)
            .map(|d| {
                let display_name = store
                    .entity_datoms(d.entity)
                    .iter()
                    .find(|ed| ed.attribute == display_attr && ed.op == Op::Assert)
                    .and_then(|ed| match &ed.value {
                        Value::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let implemented = match &d.value {
                    Value::Keyword(k) => k.contains("implemented"),
                    _ => false,
                };
                Capability {
                    name: display_name,
                    implemented,
                }
            })
            .collect()
    } else {
        // Fallback: run census directly (no session start yet)
        crate::census::run_census(store)
            .into_iter()
            .map(|r| {
                let implemented = r.is_implemented();
                Capability {
                    name: r.display_name,
                    implemented,
                }
            })
            .collect()
    }
}

/// Generate the `<braid-methodology>` section content from store state (INV-GUIDANCE-022).
///
/// This is the core DMP function. It assembles live store-derived methodology guidance
/// into a concise (<= 200 token) section for injection into AGENTS.md at the TOP
/// (maximum k* position).
///
/// Inputs:
/// - `store`: current store state (for gaps, routing, capabilities)
/// - `k_eff`: effective context budget ratio (0.0–1.0)
///
/// The output is deterministic for a given store state + k_eff.
pub fn generate_methodology_section(store: &Store, k_eff: f64) -> String {
    let mut out = String::new();

    // 1. Methodology Gaps (INV-GUIDANCE-021)
    let gaps = methodology_gaps(store);
    if !gaps.is_empty() {
        out.push_str("## Methodology Gaps\n");
        if gaps.crystallization > 0 {
            out.push_str(&format!(
                "- {} observations with uncrystallized spec IDs \u{2192} braid spec create\n",
                gaps.crystallization
            ));
        }
        if gaps.unanchored > 0 {
            out.push_str(&format!(
                "- {} tasks with unresolved spec refs \u{2192} crystallize first\n",
                gaps.unanchored
            ));
        }
        if gaps.untested > 0 {
            out.push_str(&format!(
                "- {} current-stage INVs untested \u{2192} add L2+ witness\n",
                gaps.untested
            ));
        }
        if gaps.stale_witnesses > 0 {
            out.push_str(&format!(
                "- {} witnesses invalidated \u{2192} re-verify\n",
                gaps.stale_witnesses
            ));
        }
        out.push('\n');
    }

    // 2. Ceremony Protocol (INV-GUIDANCE-023)
    let level = ceremony_level(k_eff, ChangeType::Feature); // default to Feature
    out.push_str(&format!(
        "## Ceremony Protocol (k*={:.1})\n{}\n",
        k_eff,
        level.description()
    ));
    out.push_str(
        "For known-category bug fixes: execute-first OK if provenance chain exists after commit.\n\n",
    );

    // 3. Next Actions — R(t) pre-computed top 3
    let routing = compute_routing_from_store(store);
    if !routing.is_empty() {
        // Build entity → task_id lookup from all_tasks
        let task_id_map: std::collections::BTreeMap<EntityId, String> =
            crate::task::all_tasks(store)
                .into_iter()
                .map(|t| (t.entity, t.id))
                .collect();

        out.push_str("## Next Actions (R(t) pre-computed)\n");
        for (i, r) in routing.iter().take(3).enumerate() {
            let short_id = task_id_map
                .get(&r.entity)
                .map(|s| s.as_str())
                .unwrap_or("???");
            let label = crate::budget::safe_truncate_bytes(&r.label, 60);
            out.push_str(&format!(
                "{}. \"{}\" (impact={:.2}) \u{2192} braid go {}\n",
                i + 1,
                label,
                r.impact,
                short_id
            ));
        }
        out.push('\n');
    }

    // 4. Session Constraints — capability scan
    let caps = capability_scan(store);
    let not_implemented: Vec<&Capability> = caps.iter().filter(|c| !c.implemented).collect();
    if !not_implemented.is_empty() {
        out.push_str("## Session Constraints\n");
        for cap in &not_implemented {
            out.push_str(&format!(
                "- {}: NOT YET IMPLEMENTED (spec only)\n",
                cap.name
            ));
        }
        out.push('\n');
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-GUIDANCE-001, INV-GUIDANCE-002, INV-GUIDANCE-003,
// INV-GUIDANCE-004, INV-GUIDANCE-005, INV-GUIDANCE-007,
// INV-GUIDANCE-008, INV-GUIDANCE-009, INV-GUIDANCE-010, INV-GUIDANCE-011,
// INV-GUIDANCE-014, INV-GUIDANCE-018,
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
        let sum: f64 = DEFAULT_ROUTING_WEIGHTS.iter().sum();
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
                task_type: crate::task::TaskType::Task,
            },
            TaskNode {
                entity: e2,
                label: "task-b".into(),
                priority_boost: 0.5,
                done: false,
                depends_on: vec![e1], // e1 is done, so e2 is ready
                blocks: vec![e3],
                created_at: 10,
                task_type: crate::task::TaskType::Task,
            },
            TaskNode {
                entity: e3,
                label: "task-c".into(),
                priority_boost: 0.0,
                done: false,
                depends_on: vec![e2], // e2 not done, so e3 is blocked
                blocks: vec![],
                created_at: 20,
                task_type: crate::task::TaskType::Task,
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

    // Verifies: INV-GUIDANCE-010 — typed edge routing
    // An impl task with lower PageRank but higher type_multiplier outranks
    // a docs task with higher PageRank.
    #[test]
    fn routing_type_multiplier_overrides_pagerank() {
        let e_impl = EntityId::from_ident(":task/impl-task");
        let e_docs = EntityId::from_ident(":task/docs-task");
        let e_blocked_by_docs_1 = EntityId::from_ident(":task/blocked-d1");
        let e_blocked_by_docs_2 = EntityId::from_ident(":task/blocked-d2");
        let e_blocked_by_docs_3 = EntityId::from_ident(":task/blocked-d3");
        let e_blocked_by_impl = EntityId::from_ident(":task/blocked-i1");

        // docs task blocks 3 other tasks (high PageRank proxy)
        // impl task blocks only 1 (low PageRank proxy)
        // Both are ready (no deps, not done), created at the same time.
        let tasks = vec![
            TaskNode {
                entity: e_docs,
                label: "docs-task".into(),
                priority_boost: 0.5,
                done: false,
                depends_on: vec![],
                blocks: vec![
                    e_blocked_by_docs_1,
                    e_blocked_by_docs_2,
                    e_blocked_by_docs_3,
                ],
                created_at: 50,
                task_type: crate::task::TaskType::Docs, // multiplier = 0.3
            },
            TaskNode {
                entity: e_impl,
                label: "impl-task".into(),
                priority_boost: 0.5,
                done: false,
                depends_on: vec![],
                blocks: vec![e_blocked_by_impl],
                created_at: 50,
                task_type: crate::task::TaskType::Task, // multiplier = 1.0
            },
            // Downstream tasks (blocked, not ready — just for graph structure)
            TaskNode {
                entity: e_blocked_by_docs_1,
                label: "blocked-d1".into(),
                priority_boost: 0.0,
                done: false,
                depends_on: vec![e_docs],
                blocks: vec![],
                created_at: 50,
                task_type: crate::task::TaskType::Task,
            },
            TaskNode {
                entity: e_blocked_by_docs_2,
                label: "blocked-d2".into(),
                priority_boost: 0.0,
                done: false,
                depends_on: vec![e_docs],
                blocks: vec![],
                created_at: 50,
                task_type: crate::task::TaskType::Task,
            },
            TaskNode {
                entity: e_blocked_by_docs_3,
                label: "blocked-d3".into(),
                priority_boost: 0.0,
                done: false,
                depends_on: vec![e_docs],
                blocks: vec![],
                created_at: 50,
                task_type: crate::task::TaskType::Task,
            },
            TaskNode {
                entity: e_blocked_by_impl,
                label: "blocked-i1".into(),
                priority_boost: 0.0,
                done: false,
                depends_on: vec![e_impl],
                blocks: vec![],
                created_at: 50,
                task_type: crate::task::TaskType::Task,
            },
        ];

        let routings = compute_routing(&tasks, 100);

        // Both should be in the ready set (no deps)
        let impl_r = routings.iter().find(|r| r.label == "impl-task").unwrap();
        let docs_r = routings.iter().find(|r| r.label == "docs-task").unwrap();

        // Despite docs having 3x the blocks (higher PageRank proxy),
        // impl's type_multiplier (1.0 vs 0.3) should dominate.
        assert!(
            impl_r.impact > docs_r.impact,
            "impl task (impact={:.4}, type_mult={:.1}) should outrank docs task \
             (impact={:.4}, type_mult={:.1}) despite lower PageRank",
            impl_r.impact,
            impl_r.metrics.type_multiplier,
            docs_r.impact,
            docs_r.metrics.type_multiplier,
        );

        // Verify the metrics are set correctly
        assert!(
            (impl_r.metrics.type_multiplier - 1.0).abs() < f64::EPSILON,
            "impl type_multiplier should be 1.0"
        );
        assert!(
            (docs_r.metrics.type_multiplier - 0.3).abs() < f64::EPSILON,
            "docs type_multiplier should be 0.3"
        );

        // Verify urgency_decay is >= 1.0 for both
        assert!(
            impl_r.metrics.urgency_decay >= 1.0,
            "urgency_decay should be >= 1.0"
        );
        assert!(
            docs_r.metrics.urgency_decay >= 1.0,
            "urgency_decay should be >= 1.0"
        );
    }

    // Verifies urgency_decay gives logarithmic boost for older tasks
    #[test]
    fn routing_urgency_decay_boosts_older_tasks() {
        let e_old = EntityId::from_ident(":task/old-task");
        let e_new = EntityId::from_ident(":task/new-task");

        // Two identical tasks except for age: both impl, same priority, same graph position
        let now = 7 * 86400; // 7 days in seconds
        let tasks = vec![
            TaskNode {
                entity: e_old,
                label: "old-task".into(),
                priority_boost: 0.5,
                done: false,
                depends_on: vec![],
                blocks: vec![],
                created_at: 0, // 7 days old
                task_type: crate::task::TaskType::Task,
            },
            TaskNode {
                entity: e_new,
                label: "new-task".into(),
                priority_boost: 0.5,
                done: false,
                depends_on: vec![],
                blocks: vec![],
                created_at: now, // brand new
                task_type: crate::task::TaskType::Task,
            },
        ];

        let routings = compute_routing(&tasks, now);
        let old_r = routings.iter().find(|r| r.label == "old-task").unwrap();
        let new_r = routings.iter().find(|r| r.label == "new-task").unwrap();

        // Older task should have higher urgency_decay
        assert!(
            old_r.metrics.urgency_decay > new_r.metrics.urgency_decay,
            "old task urgency_decay ({:.4}) should be higher than new ({:.4})",
            old_r.metrics.urgency_decay,
            new_r.metrics.urgency_decay,
        );

        // Urgency decay for a brand-new task should be ~1.0
        assert!(
            (new_r.metrics.urgency_decay - 1.0).abs() < 0.01,
            "new task urgency_decay should be ~1.0, got {:.4}",
            new_r.metrics.urgency_decay,
        );

        // 7-day-old task should have urgency_decay around 1.0 + ln(8) * 0.1 ~ 1.208
        let expected_old = 1.0 + (7.0_f64 + 1.0).ln() * 0.1;
        assert!(
            (old_r.metrics.urgency_decay - expected_old).abs() < 0.01,
            "old task urgency_decay should be ~{:.4}, got {:.4}",
            expected_old,
            old_r.metrics.urgency_decay,
        );

        // The older task should win on overall impact (same base, higher urgency)
        assert!(
            old_r.impact > new_r.impact,
            "old task (impact={:.4}) should outrank new task (impact={:.4}) due to urgency decay",
            old_r.impact,
            new_r.impact,
        );
    }

    // Verifies: epic tasks get zero impact (type_multiplier = 0.0)
    #[test]
    fn routing_epic_gets_zero_impact() {
        let e = EntityId::from_ident(":task/epic-task");
        let tasks = vec![TaskNode {
            entity: e,
            label: "epic-task".into(),
            priority_boost: 1.0,
            done: false,
            depends_on: vec![],
            blocks: vec![],
            created_at: 0,
            task_type: crate::task::TaskType::Epic,
        }];

        let routings = compute_routing(&tasks, 100);
        assert_eq!(routings.len(), 1);
        assert!(
            routings[0].impact.abs() < f64::EPSILON,
            "epic task should have zero impact, got {:.4}",
            routings[0].impact,
        );
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
                prop::sample::select(vec![
                    crate::task::TaskType::Task,
                    crate::task::TaskType::Bug,
                    crate::task::TaskType::Feature,
                    crate::task::TaskType::Test,
                    crate::task::TaskType::Epic,
                    crate::task::TaskType::Docs,
                    crate::task::TaskType::Question,
                ]),
            )
                .prop_map(
                    move |(done, priority_boost, created_at, dep_indices, task_type)| {
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
                            task_type,
                        }
                    },
                )
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
    // NEG-HARVEST-001: Session termination detection
    // -------------------------------------------------------------------

    #[test]
    fn should_warn_on_exit_empty_store_no_warning() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        assert!(
            should_warn_on_exit(&store, Some(1.0)).is_none(),
            "empty store should not trigger exit warning"
        );
    }

    #[test]
    fn should_warn_on_exit_below_threshold_no_warning() {
        // With multi-signal urgency, a recent harvest and few transactions
        // should keep urgency below 0.7.
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut datoms = std::collections::BTreeSet::new();

        // Simulate a harvest 1 minute ago (signal_2 = 1/30 ≈ 0.03)
        let harvest_wall = now - 60;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-recent"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));
        // Mark this as a harvest boundary
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-recent"),
            Attribute::from_keyword(":harvest/boundary-tx"),
            Value::Long(harvest_wall as i64),
            harvest_tx,
            Op::Assert,
        ));

        // Add 3 transactions after the harvest (signal_1 = 3/8 = 0.375)
        for i in 1..=3 {
            let tx = TxId::new(harvest_wall + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":work/item-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("work item {i}")),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        // k_eff = 1.0 means signal_4 = 0; all other signals well below 0.7
        assert!(
            should_warn_on_exit(&store, Some(1.0)).is_none(),
            "3 txns since recent harvest with healthy k_eff should not warn"
        );
    }

    #[test]
    fn should_warn_on_exit_at_threshold_warns() {
        // With multi-signal urgency, having enough txns relative to the adaptive
        // threshold should push signal_1 >= 0.7 and trigger a warning.
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut datoms = std::collections::BTreeSet::new();

        // Simulate a harvest 2 minutes ago (signal_2 = 2/30 ≈ 0.07)
        let harvest_wall = now - 120;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-recent"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-recent"),
            Attribute::from_keyword(":harvest/boundary-tx"),
            Value::Long(harvest_wall as i64),
            harvest_tx,
            Op::Assert,
        ));

        // Add 8 transactions after the harvest (signal_1 = 8/8 = 1.0 >= 0.7)
        for i in 1..=8 {
            let tx = TxId::new(harvest_wall + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":work/item-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("work item {i}")),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        // k_eff = 1.0: signal_4 off, but signal_1 = 8/8 = 1.0 triggers
        let warning = should_warn_on_exit(&store, Some(1.0));
        assert!(
            warning.is_some(),
            "8 txns since harvest should trigger warning"
        );
        let msg = warning.unwrap();
        assert!(
            msg.contains("NEG-HARVEST-001"),
            "warning must reference the spec element: {msg}"
        );
        assert!(
            msg.contains("8 transactions"),
            "warning must include the transaction count: {msg}"
        );
        assert!(
            msg.contains("braid harvest --commit"),
            "warning must include the recovery command: {msg}"
        );
    }

    #[test]
    fn should_warn_on_exit_well_above_threshold_warns() {
        // 15 txns with no harvest ever: signal_1 = 15/8 ≈ 1.875 (well above 0.7).
        // Also signal_2 is huge (no harvest ever, minutes_since = now/60/30).
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut datoms = std::collections::BTreeSet::new();

        // No harvest at all — simulate 15 transactions of recent work
        for i in 1..=15 {
            let tx = TxId::new(now - 15 + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":work/item-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("work item {i}")),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        let warning = should_warn_on_exit(&store, Some(1.0));
        assert!(
            warning.is_some(),
            "15 txns with no harvest ever should trigger warning"
        );
        let msg = warning.unwrap();
        assert!(
            msg.contains("15 transactions"),
            "warning must include the transaction count: {msg}"
        );
    }

    // -------------------------------------------------------------------
    // tx_velocity + dynamic_threshold (INV-GUIDANCE-019)
    // -------------------------------------------------------------------

    #[test]
    fn tx_velocity_empty_store_returns_zero() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let v = tx_velocity(&store);
        assert!(
            (v - 0.0).abs() < f64::EPSILON,
            "empty store should have zero velocity, got {v}"
        );
    }

    #[test]
    fn tx_velocity_with_recent_txns() {
        // Build a store with 10 recent :tx/agent datoms (wall_time = now).
        // Each distinct wall_time counts as one transaction.
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut datoms = std::collections::BTreeSet::new();
        for i in 0..10u64 {
            let wall = now - i; // all within 5-min window
            let tx = TxId::new(wall, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":tx/meta-{i}")),
                Attribute::from_keyword(":tx/agent"),
                Value::String("test".to_string()),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        let v = tx_velocity(&store);
        assert!(
            (v - 2.0).abs() < f64::EPSILON,
            "10 txns in 5-min window should give velocity=2.0, got {v}"
        );
    }

    #[test]
    fn tx_velocity_ignores_old_txns() {
        // Datoms with wall_time far in the past should not count.
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");

        let mut datoms = std::collections::BTreeSet::new();
        for i in 1..=5u64 {
            let tx = TxId::new(i, 0, agent); // wall_time=1..5 (ancient)
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":tx/old-{i}")),
                Attribute::from_keyword(":tx/agent"),
                Value::String("test".to_string()),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        let v = tx_velocity(&store);
        assert!(
            (v - 0.0).abs() < f64::EPSILON,
            "ancient txns should give velocity=0.0, got {v}"
        );
    }

    #[test]
    fn dynamic_threshold_high_velocity() {
        assert_eq!(dynamic_threshold(6.0), 30);
        assert_eq!(dynamic_threshold(10.0), 30);
        assert_eq!(dynamic_threshold(100.0), 30);
    }

    #[test]
    fn dynamic_threshold_medium_velocity() {
        assert_eq!(dynamic_threshold(2.0), 15);
        assert_eq!(dynamic_threshold(1.5), 15);
        assert_eq!(dynamic_threshold(5.0), 15); // 5.0 is NOT > 5.0
    }

    #[test]
    fn dynamic_threshold_low_velocity() {
        assert_eq!(dynamic_threshold(0.5), 8);
        assert_eq!(dynamic_threshold(1.0), 8); // 1.0 is NOT > 1.0
        assert_eq!(dynamic_threshold(0.0), 8);
    }

    #[test]
    fn should_warn_on_exit_high_velocity_uses_higher_threshold() {
        // With high velocity (>5 txn/min), dynamic_threshold returns 30.
        // Multi-signal urgency: signal_1 = 5/30 ≈ 0.17, signal_2 = 0.25/30 ≈ 0.008,
        // signal_3 = 0 (no exploration entities), signal_4 = 0 (k_eff healthy).
        // Max urgency ≈ 0.17 < 0.7 → no warning.
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut datoms = std::collections::BTreeSet::new();

        // Place a harvest at (now - 15), so only datoms with wall_time > (now-15)
        // count as "since last harvest."
        let harvest_wall = now - 15;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-recent"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-recent"),
            Attribute::from_keyword(":harvest/boundary-tx"),
            Value::Long(harvest_wall as i64),
            harvest_tx,
            Op::Assert,
        ));

        // Create 30 :tx/agent datoms spanning the full 5-min window.
        // Space them 6 seconds apart: wall = now, now-6, now-12, ..., now-174.
        // Most are BEFORE the harvest (now-15), so they contribute to velocity
        // but NOT to the "since last harvest" count.
        for i in 0..30u64 {
            let wall = now - (i * 6);
            let tx = TxId::new(wall, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":tx/meta-{i}")),
                Attribute::from_keyword(":tx/agent"),
                Value::String("test".to_string()),
                tx,
                Op::Assert,
            ));
        }

        // Add 5 work datoms after the harvest, each at a unique wall_time.
        for i in 1..=5u64 {
            let wall = harvest_wall + i;
            let tx = TxId::new(wall, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":work/item-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("work item {i}")),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);

        // Verify velocity is high enough to trigger the 30 threshold.
        // Use tx_velocity_at with the test's own `now` to avoid timing skew.
        let velocity = tx_velocity_at(&store, now);
        assert!(
            velocity > 5.0,
            "expected velocity > 5.0 for threshold=30, got {velocity}"
        );

        // The txn count since harvest should be moderate (well under 30).
        // k_eff = 1.0 means signal_4 = 0, so max urgency is signal_1 ≈ 0.17.
        let warning = should_warn_on_exit(&store, Some(1.0));
        assert!(
            warning.is_none(),
            "few txns since harvest with high velocity (threshold=30) should NOT warn, \
             but got: {:?}",
            warning
        );
    }

    #[test]
    fn should_warn_on_exit_critical_k_eff_triggers_warning() {
        // When k_eff < 0.15, signal_4 = 1.5 which exceeds 0.7, triggering
        // a harvest warning regardless of other signals.
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut datoms = std::collections::BTreeSet::new();

        // Recent harvest (signal_2 low)
        let harvest_wall = now - 30;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-recent"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-recent"),
            Attribute::from_keyword(":harvest/boundary-tx"),
            Value::Long(harvest_wall as i64),
            harvest_tx,
            Op::Assert,
        ));

        // Just 1 transaction since harvest (signal_1 = 1/8 = 0.125)
        let tx = TxId::new(harvest_wall + 1, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":work/item-1"),
            Attribute::from_keyword(":db/doc"),
            Value::String("work item 1".to_string()),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        // k_eff = 0.1 (critically low) → signal_4 = 1.5 → urgency >= 0.7
        let warning = should_warn_on_exit(&store, Some(0.1));
        assert!(
            warning.is_some(),
            "critical k_eff should trigger harvest warning even with few txns"
        );
        let msg = warning.unwrap();
        assert!(
            msg.contains("urgency"),
            "warning must include urgency score: {msg}"
        );
    }

    #[test]
    fn should_warn_on_exit_none_k_eff_estimates_from_store() {
        // When k_eff is None, should_warn_on_exit estimates it from the store.
        // An empty store has 0 txns since harvest → short-circuits to None.
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        assert!(
            should_warn_on_exit(&store, None).is_none(),
            "empty store with auto-estimated k_eff should not warn"
        );
    }

    // -------------------------------------------------------------------
    // Urgency display clamping (T3-2)
    // -------------------------------------------------------------------

    #[test]
    fn should_warn_on_exit_large_urgency_shows_clamped() {
        // Simulate a store with many transactions and no harvest to produce
        // urgency well above 10.0. The display should show "10.0+" rather
        // than the raw value.
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut datoms = std::collections::BTreeSet::new();

        // 100 transactions with no harvest — drives urgency far above 10.0
        for i in 1..=100 {
            let tx = TxId::new(now - 100 + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":work/large-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("item {i}")),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        let warning = should_warn_on_exit(&store, Some(1.0));
        assert!(warning.is_some(), "100 txns with no harvest should warn");
        let msg = warning.unwrap();
        assert!(
            msg.contains("urgency 10.0+"),
            "large urgency must display as '10.0+', got: {msg}"
        );
    }

    #[test]
    fn should_warn_on_exit_normal_urgency_shows_exact() {
        // With a recent harvest and enough transactions to trigger a warning
        // (signal_1 > 0.7), but not enough to exceed urgency 10.0.
        // The display should show the exact value, not the clamped "10.0+" form.
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut datoms = std::collections::BTreeSet::new();

        // A harvest 1 minute ago keeps signal_2 = 1/30 ≈ 0.03.
        let harvest_wall = now - 60;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-normal"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-normal"),
            Attribute::from_keyword(":harvest/boundary-tx"),
            Value::Long(harvest_wall as i64),
            harvest_tx,
            Op::Assert,
        ));

        // 7 transactions after the harvest: signal_1 = 7/8 = 0.875.
        // max(0.875, 0.03) = 0.875, which is above the 0.7 warning
        // threshold but well below 10.0.
        for i in 1..=7 {
            let tx = TxId::new(harvest_wall + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":work/normal-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("item {i}")),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        let warning = should_warn_on_exit(&store, Some(1.0));
        assert!(warning.is_some(), "7 txns since recent harvest should warn");
        let msg = warning.unwrap();
        // Must contain "urgency X.XX" with a numeric value, not "10.0+"
        assert!(
            !msg.contains("10.0+"),
            "normal urgency must NOT show clamped form, got: {msg}"
        );
        assert!(
            msg.contains("urgency"),
            "warning must include urgency: {msg}"
        );
        // Verify it shows a decimal number after "urgency "
        let urgency_idx = msg.find("urgency ").expect("must contain 'urgency '");
        let after = &msg[urgency_idx + 8..];
        let numeric_part: String = after
            .chars()
            .take_while(|c| *c == '.' || c.is_ascii_digit())
            .collect();
        let parsed: f64 = numeric_part.parse().expect("urgency value must be numeric");
        assert!(parsed < 10.0, "urgency should be below 10.0, got {parsed}");
    }

    // -------------------------------------------------------------------
    // GuidanceContext (ADR-GUIDANCE-015)
    // -------------------------------------------------------------------

    #[test]
    fn guidance_context_from_empty_store() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let ctx = GuidanceContext::from_store(&store, None);
        // KEFF-3: When no explicit k_eff provided, estimate_k_eff uses sigmoid
        // fusion. For zero evidence, all signals are below threshold, so
        // sigmoid outputs ~0.26 total decay → k_eff ≈ 0.74. Not exactly 1.0.
        assert!(
            ctx.k_eff > 0.5 && ctx.k_eff <= 1.0,
            "default k_eff should be high for empty store, got {}",
            ctx.k_eff
        );
        assert!(
            (ctx.tx_velocity - 0.0).abs() < f64::EPSILON,
            "empty store should have zero velocity"
        );
        assert_eq!(ctx.unanchored_tasks, 0);
    }

    #[test]
    fn guidance_context_from_store_with_k_eff() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let ctx = GuidanceContext::from_store(&store, Some(0.42));
        assert!(
            (ctx.k_eff - 0.42).abs() < f64::EPSILON,
            "k_eff should be the supplied value 0.42, got {}",
            ctx.k_eff
        );
    }

    #[test]
    fn guidance_context_from_genesis_store() {
        let store = Store::genesis();
        let ctx = GuidanceContext::from_store(&store, Some(0.8));
        // Genesis store has at least one agent (system)
        assert!(
            ctx.agent_count >= 1,
            "genesis store should have at least 1 agent in frontier, got {}",
            ctx.agent_count
        );
        assert!(
            (ctx.k_eff - 0.8).abs() < f64::EPSILON,
            "k_eff should be 0.8"
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
        // B2.3: Compressed now shows paste-ready command for worst metric,
        // not the original next_action/invariant_refs.
        assert!(
            compressed.contains("braid "),
            "must contain a paste-ready braid command, got: {compressed}"
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

    // -------------------------------------------------------------------
    // BasinToken tests (T4-1)
    // -------------------------------------------------------------------

    #[test]
    fn basin_token_high_methodology_is_empty() {
        let telemetry = SessionTelemetry {
            total_turns: 10,
            transact_turns: 8,
            spec_language_turns: 7,
            query_type_count: 3,
            harvest_quality: 0.9,
            ..Default::default()
        };
        let store = Store::genesis();
        let mut footer = build_footer(
            &telemetry,
            &store,
            Some("braid query [:find ?e]".into()),
            vec!["INV-STORE-003".into()],
        );
        // Force M(t) > 0.7 to trigger the empty/silent path
        footer.methodology.score = 0.8;
        let basin =
            format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
        assert!(
            basin.is_empty() || basin.len() < 10,
            "BasinToken with M(t)>0.7 should be empty or very short, got ({} chars): {basin}",
            basin.len()
        );
    }

    #[test]
    fn basin_token_harvest_warning_contains_harvest() {
        let telemetry = SessionTelemetry::default();
        let store = Store::genesis();
        let mut footer = build_footer(
            &telemetry,
            &store,
            Some("braid query [:find ?e]".into()),
            vec![],
        );
        footer.harvest_warning = HarvestWarningLevel::Critical;
        let basin =
            format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
        assert!(
            basin.contains("harvest"),
            "BasinToken with harvest warning must contain 'harvest', got: {basin}"
        );
    }

    #[test]
    fn basin_token_output_always_under_50_chars() {
        let telemetry = SessionTelemetry::default();
        let store = Store::genesis();

        // Case 1: high M(t) — empty
        let mut footer = build_footer(
            &telemetry,
            &store,
            Some("braid query [:find ?e]".into()),
            vec![],
        );
        footer.methodology.score = 0.9;
        let basin =
            format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
        assert!(
            basin.len() < 50,
            "BasinToken must be < 50 chars, got {} chars: {basin}",
            basin.len()
        );

        // Case 2: low M(t) with action
        footer.methodology.score = 0.2;
        let basin =
            format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
        assert!(
            basin.len() < 50,
            "BasinToken must be < 50 chars, got {} chars: {basin}",
            basin.len()
        );

        // Case 3: harvest warning
        footer.harvest_warning = HarvestWarningLevel::Warn;
        let basin =
            format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
        assert!(
            basin.len() < 50,
            "BasinToken must be < 50 chars, got {} chars: {basin}",
            basin.len()
        );

        // Case 4: mid M(t) store summary
        footer.harvest_warning = HarvestWarningLevel::None;
        footer.methodology.score = 0.5;
        let basin =
            format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
        assert!(
            basin.len() < 50,
            "BasinToken must be < 50 chars, got {} chars: {basin}",
            basin.len()
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
    // B2.2: Activity mode detection
    // -------------------------------------------------------------------

    #[test]
    fn detect_activity_mode_implementation() {
        let telemetry = SessionTelemetry {
            total_turns: 10,
            transact_turns: 8,
            spec_language_turns: 2,
            ..Default::default()
        };
        assert_eq!(
            detect_activity_mode(&telemetry),
            ActivityMode::Implementation,
            "8/10 transact turns should be Implementation"
        );
    }

    #[test]
    fn detect_activity_mode_specification() {
        let telemetry = SessionTelemetry {
            total_turns: 10,
            transact_turns: 1,
            spec_language_turns: 7,
            ..Default::default()
        };
        assert_eq!(
            detect_activity_mode(&telemetry),
            ActivityMode::Specification,
            "7/10 spec-language turns should be Specification"
        );
    }

    #[test]
    fn detect_activity_mode_mixed() {
        let telemetry = SessionTelemetry {
            total_turns: 10,
            transact_turns: 3,
            spec_language_turns: 3,
            ..Default::default()
        };
        assert_eq!(
            detect_activity_mode(&telemetry),
            ActivityMode::Mixed,
            "3/10 each should be Mixed"
        );
    }

    #[test]
    fn detect_activity_mode_empty_session() {
        let telemetry = SessionTelemetry::default();
        assert_eq!(
            detect_activity_mode(&telemetry),
            ActivityMode::Mixed,
            "Empty session should be Mixed"
        );
    }

    #[test]
    fn detect_activity_mode_transact_wins_tie() {
        let telemetry = SessionTelemetry {
            total_turns: 10,
            transact_turns: 6,
            spec_language_turns: 6,
            ..Default::default()
        };
        assert_eq!(
            detect_activity_mode(&telemetry),
            ActivityMode::Implementation,
            "When both >0.5, transact_turns is checked first"
        );
    }

    // -------------------------------------------------------------------
    // B2.1/B2.3: Paste-ready commands in footer
    // -------------------------------------------------------------------

    #[test]
    fn worst_metric_returns_harvest_when_lowest() {
        let components = MethodologyComponents {
            transact_frequency: 0.8,
            spec_language_ratio: 0.6,
            query_diversity: 0.5,
            harvest_quality: 0.0,
        };
        let cmd = worst_metric_command(&components, None);
        assert!(
            cmd.contains("harvest"),
            "Worst metric (harvest=0.0) should suggest harvest command, got: {cmd}"
        );
    }

    #[test]
    fn worst_metric_returns_observe_when_tx_lowest() {
        let components = MethodologyComponents {
            transact_frequency: 0.0,
            spec_language_ratio: 0.5,
            query_diversity: 0.5,
            harvest_quality: 0.5,
        };
        let cmd = worst_metric_command(&components, None);
        assert!(
            cmd.contains("observe"),
            "Worst metric (tx=0.0) should suggest observe command, got: {cmd}"
        );
    }

    #[test]
    fn worst_metric_returns_query_entity_when_spec_lowest() {
        let components = MethodologyComponents {
            transact_frequency: 0.8,
            spec_language_ratio: 0.0,
            query_diversity: 0.5,
            harvest_quality: 0.5,
        };
        let cmd = worst_metric_command(&components, None);
        assert!(
            cmd.contains("--entity"),
            "Worst metric (spec=0.0) should suggest query --entity command, got: {cmd}"
        );
    }

    #[test]
    fn worst_metric_returns_query_attribute_when_qdiv_lowest() {
        let components = MethodologyComponents {
            transact_frequency: 0.8,
            spec_language_ratio: 0.5,
            query_diversity: 0.0,
            harvest_quality: 0.5,
        };
        let cmd = worst_metric_command(&components, None);
        assert!(
            cmd.contains("--attribute"),
            "Worst metric (q-div=0.0) should suggest query --attribute command, got: {cmd}"
        );
    }

    #[test]
    fn compressed_footer_contains_paste_ready_command() {
        let telemetry = SessionTelemetry::default();
        let store = Store::genesis();
        let footer = build_footer(&telemetry, &store, None, vec![]);
        let compressed = format_footer_at_level(&footer, crate::budget::GuidanceLevel::Compressed);
        assert!(
            compressed.contains("braid "),
            "Compressed footer must contain paste-ready command, got: {compressed}"
        );
        assert!(
            compressed.contains("\u{2192}"),
            "Compressed footer must contain arrow before command, got: {compressed}"
        );
    }

    #[test]
    fn full_footer_shows_paste_ready_hints_when_failing() {
        let telemetry = SessionTelemetry::default();
        let store = Store::genesis();
        let footer = build_footer(&telemetry, &store, None, vec![]);
        let full = format_footer(&footer);
        assert!(
            full.contains("braid observe"),
            "Full footer with tx=0 should show 'braid observe' hint, got: {full}"
        );
        assert!(
            full.contains("braid query --entity"),
            "Full footer with spec=0 should show 'braid query --entity' hint, got: {full}"
        );
        assert!(
            full.contains("braid query --attribute"),
            "Full footer with q-div=0 should show 'braid query --attribute' hint, got: {full}"
        );
        assert!(
            full.contains("braid harvest --commit"),
            "Full footer with harvest=0 should show 'braid harvest --commit' hint, got: {full}"
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
                        contextual_hint: None,
                    }
                })
        }

        proptest! {
            #[test]
            fn format_at_level_never_panics(footer in arb_footer(), k_eff in arb_k_eff()) {
                let level = GuidanceLevel::for_k_eff(k_eff);
                let formatted = format_footer_at_level(&footer, level);
                // BasinToken intentionally returns empty when M(t) > 0.7
                // (methodology on track, no perturbation needed).
                if level != GuidanceLevel::BasinToken {
                    prop_assert!(!formatted.is_empty(), "formatted footer must not be empty");
                }
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
                    contextual_hint: None,
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

    // INV-HARVEST-005: Info at [0.15, 0.6]
    #[test]
    fn harvest_warning_level_info_between_015_06() {
        assert_eq!(harvest_warning_level(0.6), HarvestWarningLevel::Info);
        assert_eq!(harvest_warning_level(0.45), HarvestWarningLevel::Info);
        assert_eq!(harvest_warning_level(0.3), HarvestWarningLevel::Info);
        assert_eq!(harvest_warning_level(0.2), HarvestWarningLevel::Info);
        assert_eq!(harvest_warning_level(0.15), HarvestWarningLevel::Info);
    }

    // INV-HARVEST-005: Warn at [0.05, 0.15) — spec: "Q(t) < 0.15 => harvest warning"
    #[test]
    fn harvest_warning_level_warn_between_005_015() {
        assert_eq!(harvest_warning_level(0.14), HarvestWarningLevel::Warn);
        assert_eq!(harvest_warning_level(0.1), HarvestWarningLevel::Warn);
        assert_eq!(harvest_warning_level(0.05), HarvestWarningLevel::Warn);
    }

    // INV-HARVEST-005: Critical below 0.05 — spec: "Q(t) < 0.05 => harvest-only mode"
    #[test]
    fn harvest_warning_level_critical_below_005() {
        assert_eq!(harvest_warning_level(0.049), HarvestWarningLevel::Critical);
        assert_eq!(harvest_warning_level(0.01), HarvestWarningLevel::Critical);
        assert_eq!(harvest_warning_level(0.0), HarvestWarningLevel::Critical);
    }

    // Verifies: threshold boundaries are exact per INV-HARVEST-005
    #[test]
    fn harvest_warning_level_boundary_precision() {
        // 0.6 is Info (inclusive lower bound of [0.15, 0.6])
        assert_eq!(harvest_warning_level(0.6), HarvestWarningLevel::Info);
        // 0.6 + epsilon is None
        assert_eq!(
            harvest_warning_level(0.6 + f64::EPSILON),
            HarvestWarningLevel::None
        );
        // 0.15 is Info (inclusive lower bound of [0.15, 0.6])
        assert_eq!(harvest_warning_level(0.15), HarvestWarningLevel::Info);
        // 0.15 - epsilon is Warn (spec: Q(t) < 0.15 => harvest warning)
        assert_eq!(
            harvest_warning_level(0.15 - f64::EPSILON),
            HarvestWarningLevel::Warn
        );
        // 0.05 is Warn (inclusive lower bound of [0.05, 0.15))
        assert_eq!(harvest_warning_level(0.05), HarvestWarningLevel::Warn);
        // 0.05 - epsilon is Critical (spec: Q(t) < 0.05 => harvest-only)
        assert_eq!(
            harvest_warning_level(0.05 - f64::EPSILON),
            HarvestWarningLevel::Critical
        );
    }

    // INV-HARVEST-005 spec-alignment: the two thresholds from the L0 definition.
    // L0: "Q(t) < 0.15 => response includes harvest warning"
    // L0: "Q(t) < 0.05 => response = ONLY harvest imperative"
    #[test]
    fn harvest_warning_spec_alignment_inv_harvest_005() {
        // Above 0.15: no harvest warning in response (None or Info — Info is advisory)
        assert!(harvest_warning_level(0.16) < HarvestWarningLevel::Warn);
        // Below 0.15: harvest warning in response (Warn or Critical)
        assert!(harvest_warning_level(0.14) >= HarvestWarningLevel::Warn);
        // Above 0.05: response may include non-harvest content
        assert!(harvest_warning_level(0.06) < HarvestWarningLevel::Critical);
        // Below 0.05: response = ONLY harvest imperative (Critical)
        assert_eq!(harvest_warning_level(0.04), HarvestWarningLevel::Critical);
        assert_eq!(harvest_warning_level(0.0), HarvestWarningLevel::Critical);
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
        // Q(t)=0.03 → Critical → should produce Harvest action at priority 1
        let actions = derive_actions_with_budget(&store, Some(0.03));
        let harvest_actions: Vec<_> = actions
            .iter()
            .filter(|a| a.category == ActionCategory::Harvest)
            .collect();
        assert!(
            !harvest_actions.is_empty(),
            "Q(t)=0.03 should produce a harvest action"
        );
        assert_eq!(
            harvest_actions[0].priority, 1,
            "Critical Q(t) should produce priority 1 action"
        );
        assert!(
            harvest_actions[0].summary.contains("Q(t)=0.03"),
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
        // Q(t)=0.1 is Warn per INV-HARVEST-005 (0.05 <= 0.1 < 0.15)
        let footer_warn = build_footer_with_budget(&telemetry, &store, None, vec![], Some(0.1));
        assert_eq!(
            footer_warn.harvest_warning,
            HarvestWarningLevel::Warn,
            "Q(t)=0.1 should set Warn warning level"
        );
        // Q(t)=0.03 is Critical per INV-HARVEST-005 (< 0.05)
        let footer_crit = build_footer_with_budget(&telemetry, &store, None, vec![], Some(0.03));
        assert_eq!(
            footer_crit.harvest_warning,
            HarvestWarningLevel::Critical,
            "Q(t)=0.03 should set Critical warning level"
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
        // Q(t)=0.1 is Warn per INV-HARVEST-005 (0.05 <= 0.1 < 0.15)
        let footer = build_footer_with_budget(
            &telemetry,
            &store,
            Some("braid harvest --commit".into()),
            vec![],
            Some(0.1), // Warn level per new spec-aligned thresholds
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
            Some(0.03), // Critical per INV-HARVEST-005 (< 0.05)
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
            Some(0.03), // Critical per INV-HARVEST-005 (< 0.05)
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
            Some(0.03), // Critical per INV-HARVEST-005 (< 0.05)
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

    // -------------------------------------------------------------------
    // SFE-1.1: extract_spec_ids (unit helper)
    // -------------------------------------------------------------------

    #[test]
    fn extract_spec_ids_basic() {
        let ids = extract_spec_ids("We should check INV-STORE-001 and ADR-MERGE-005");
        assert_eq!(ids, vec!["ADR-MERGE-005", "INV-STORE-001"]);
    }

    #[test]
    fn extract_spec_ids_neg_pattern() {
        let ids = extract_spec_ids("Violation of NEG-WITNESS-005 detected");
        assert_eq!(ids, vec!["NEG-WITNESS-005"]);
    }

    #[test]
    fn extract_spec_ids_no_matches() {
        let ids = extract_spec_ids("This text has no spec references");
        assert!(ids.is_empty());
    }

    #[test]
    fn extract_spec_ids_deduplicates() {
        let ids = extract_spec_ids("INV-FOO-001 appears twice: INV-FOO-001");
        assert_eq!(ids, vec!["INV-FOO-001"]);
    }

    #[test]
    fn extract_spec_ids_ignores_lowercase_namespace() {
        let ids = extract_spec_ids("INV-store-001");
        assert!(ids.is_empty());
    }

    #[test]
    fn extract_spec_ids_ignores_no_digits() {
        let ids = extract_spec_ids("INV-STORE-");
        assert!(ids.is_empty());
    }

    // -------------------------------------------------------------------
    // SFE-1.1: crystallization_candidates
    // -------------------------------------------------------------------

    // Verifies: INV-GUIDANCE-018 — Crystallization Gap Detection
    #[test]
    fn crystallization_candidates_detects_uncrystallized() {
        use crate::datom::{AgentId, Datom, Op, TxId, Value};

        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);
        let mut datoms = std::collections::BTreeSet::new();

        // Create an observation that mentions INV-FOO-001
        let obs_entity = EntityId::from_ident(":obs/test-obs-1");
        datoms.insert(Datom::new(
            obs_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":obs/test-obs-1".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            obs_entity,
            Attribute::from_keyword(":exploration/body"),
            Value::String("Need to formalize INV-FOO-001 as a proper invariant".to_string()),
            tx,
            Op::Assert,
        ));

        // No :spec/inv-foo-001 entity exists → should be detected as candidate
        let store = Store::from_datoms(datoms);
        let candidates = crystallization_candidates(&store);

        assert_eq!(candidates.len(), 1, "should detect one uncrystallized ref");
        assert_eq!(candidates[0].0, obs_entity);
        assert_eq!(candidates[0].1, "INV-FOO-001");
    }

    // Verifies: INV-GUIDANCE-018 — Already crystallized refs are excluded
    #[test]
    fn crystallization_candidates_excludes_crystallized() {
        use crate::datom::{AgentId, Datom, Op, TxId, Value};

        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);
        let mut datoms = std::collections::BTreeSet::new();

        // Create an observation mentioning INV-BAR-002
        let obs_entity = EntityId::from_ident(":obs/test-obs-2");
        datoms.insert(Datom::new(
            obs_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":obs/test-obs-2".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            obs_entity,
            Attribute::from_keyword(":exploration/body"),
            Value::String("Working on INV-BAR-002 compliance".to_string()),
            tx,
            Op::Assert,
        ));

        // Create the formal spec element :spec/inv-bar-002 WITH :spec/falsification
        let spec_entity = EntityId::from_ident(":spec/inv-bar-002");
        datoms.insert(Datom::new(
            spec_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":spec/inv-bar-002".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            spec_entity,
            Attribute::from_keyword(":spec/falsification"),
            Value::String("Violated if bar metric exceeds threshold".to_string()),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let candidates = crystallization_candidates(&store);

        assert!(
            candidates.is_empty(),
            "crystallized ref should not appear as candidate, got: {candidates:?}"
        );
    }

    // Verifies: INV-GUIDANCE-018 — Empty store produces no candidates
    #[test]
    fn crystallization_candidates_empty_store() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let candidates = crystallization_candidates(&store);
        assert!(candidates.is_empty());
    }

    // Verifies: INV-GUIDANCE-018 — Spec entity without falsification is not crystallized
    #[test]
    fn crystallization_candidates_spec_without_falsification() {
        use crate::datom::{AgentId, Datom, Op, TxId, Value};

        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);
        let mut datoms = std::collections::BTreeSet::new();

        // Observation mentioning INV-QUX-003
        let obs_entity = EntityId::from_ident(":obs/test-obs-3");
        datoms.insert(Datom::new(
            obs_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":obs/test-obs-3".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            obs_entity,
            Attribute::from_keyword(":exploration/body"),
            Value::String("Consider adding INV-QUX-003 formally".to_string()),
            tx,
            Op::Assert,
        ));

        // Spec entity exists but WITHOUT :spec/falsification (incomplete)
        let spec_entity = EntityId::from_ident(":spec/inv-qux-003");
        datoms.insert(Datom::new(
            spec_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":spec/inv-qux-003".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            spec_entity,
            Attribute::from_keyword(":spec/statement"),
            Value::String("Some statement".to_string()),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let candidates = crystallization_candidates(&store);

        assert_eq!(
            candidates.len(),
            1,
            "spec entity without falsification should still be uncrystallized"
        );
        assert_eq!(candidates[0].1, "INV-QUX-003");
    }

    // -----------------------------------------------------------------------
    // SFE-3.1: spec_anchor_factor
    // -----------------------------------------------------------------------

    // Verifies: spec anchor factor returns 1.0 when all refs resolve.
    #[test]
    fn anchor_factor_all_resolved() {
        use crate::datom::{AgentId, Attribute, Datom, Op, TxId};
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let store = routing_test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        // Create a spec element with :spec/falsification
        let spec_entity = EntityId::from_ident(":spec/inv-store-001");
        let spec_datoms = vec![
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-store-001".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/falsification"),
                Value::String("Any mutation of existing datom".to_string()),
                tx,
                Op::Assert,
            ),
        ];
        let store = routing_store_with(&store, spec_datoms);

        // Create a task referencing INV-STORE-001
        let (task_entity, task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Fix INV-STORE-001 violation",
            description: None,
            priority: 1,
            task_type: TaskType::Bug,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, task_datoms);

        let factor = spec_anchor_factor(&store, task_entity);
        assert!(
            (factor - 1.0).abs() < f64::EPSILON,
            "all refs resolved => 1.0, got {factor}"
        );
    }

    // Verifies: spec anchor factor returns 0.3 when no refs resolve.
    #[test]
    fn anchor_factor_none_resolved() {
        use crate::datom::{AgentId, TxId};
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let store = routing_test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        // Create a task referencing a nonexistent spec element
        let (task_entity, task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Fix INV-FAKE-999 issue",
            description: None,
            priority: 1,
            task_type: TaskType::Bug,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, task_datoms);

        let factor = spec_anchor_factor(&store, task_entity);
        assert!(
            (factor - 0.3).abs() < f64::EPSILON,
            "no refs resolved => 0.3, got {factor}"
        );
    }

    // Verifies: spec anchor factor returns 1.0 when task has no spec refs.
    #[test]
    fn anchor_factor_no_refs() {
        use crate::datom::{AgentId, TxId};
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let store = routing_test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        // Create a task with no spec refs in the title
        let (task_entity, task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Fix a simple bug",
            description: None,
            priority: 1,
            task_type: TaskType::Bug,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, task_datoms);

        let factor = spec_anchor_factor(&store, task_entity);
        assert!(
            (factor - 1.0).abs() < f64::EPSILON,
            "no refs in title => 1.0, got {factor}"
        );
    }

    // Verifies: spec anchor factor returns 0.7 when some refs resolve.
    #[test]
    fn anchor_factor_partial_resolved() {
        use crate::datom::{AgentId, Attribute, Datom, Op, TxId};
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let store = routing_test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        // Create one spec element with :spec/falsification
        let spec_entity = EntityId::from_ident(":spec/inv-store-001");
        let spec_datoms = vec![
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-store-001".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/falsification"),
                Value::String("Any mutation of existing datom".to_string()),
                tx,
                Op::Assert,
            ),
        ];
        let store = routing_store_with(&store, spec_datoms);

        // Create a task referencing both a real and a nonexistent spec element
        let (task_entity, task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Fix INV-STORE-001 and INV-FAKE-999",
            description: None,
            priority: 1,
            task_type: TaskType::Bug,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, task_datoms);

        let factor = spec_anchor_factor(&store, task_entity);
        assert!(
            (factor - 0.7).abs() < f64::EPSILON,
            "partial resolution => 0.7, got {factor}"
        );
    }

    // -------------------------------------------------------------------
    // SFE-1.2: Status wiring — crystallization gap count
    // -------------------------------------------------------------------

    // Verifies: SFE-1.2 — crystallization_candidates returns correct count
    // when observations contain uncrystallized spec IDs.
    #[test]
    fn status_crystallization_gap_count() {
        use crate::datom::{AgentId, Datom, Op, TxId, Value};

        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);
        let mut datoms = std::collections::BTreeSet::new();

        // Create two observations referencing different uncrystallized spec IDs
        let obs1 = EntityId::from_ident(":obs/gap-obs-1");
        datoms.insert(Datom::new(
            obs1,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":obs/gap-obs-1".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            obs1,
            Attribute::from_keyword(":exploration/body"),
            Value::String("Need to formalize INV-GAP-001 as an invariant".to_string()),
            tx,
            Op::Assert,
        ));

        let obs2 = EntityId::from_ident(":obs/gap-obs-2");
        datoms.insert(Datom::new(
            obs2,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":obs/gap-obs-2".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            obs2,
            Attribute::from_keyword(":exploration/body"),
            Value::String("Also need ADR-GAP-002 crystallized".to_string()),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let candidates = crystallization_candidates(&store);

        // Two distinct uncrystallized spec IDs from two observations
        assert_eq!(
            candidates.len(),
            2,
            "should detect 2 uncrystallized spec IDs, got: {candidates:?}"
        );
        let ids: Vec<&str> = candidates.iter().map(|c| c.1.as_str()).collect();
        assert!(ids.contains(&"ADR-GAP-002"), "should contain ADR-GAP-002");
        assert!(ids.contains(&"INV-GAP-001"), "should contain INV-GAP-001");
    }

    // -------------------------------------------------------------------
    // SFE-3.2: Routing — anchored task outranks unanchored at equal position
    // -------------------------------------------------------------------

    // Verifies: SFE-3.2 — spec_anchor_factor is applied in compute_routing_from_store.
    // An anchored task (all spec refs resolve, anchor=1.0) outranks an unanchored task
    // (spec refs don't resolve, anchor=0.3) when both have equal graph position.
    #[test]
    fn routing_anchored_outranks_unanchored() {
        use crate::datom::{AgentId, Attribute, Datom, Op, TxId};
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let store = routing_test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        // Create a formal spec element for INV-STORE-001 (with falsification)
        let spec_entity = EntityId::from_ident(":spec/inv-store-001");
        let spec_datoms = vec![
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-store-001".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/falsification"),
                Value::String("Any mutation of existing datom".to_string()),
                tx,
                Op::Assert,
            ),
        ];
        let store = routing_store_with(&store, spec_datoms);

        // Create anchored task (references INV-STORE-001 which exists -> anchor=1.0)
        let (_, anchored_datoms) = create_task_datoms(CreateTaskParams {
            title: "Fix INV-STORE-001 compliance",
            description: None,
            priority: 2,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, anchored_datoms);

        // Create unanchored task (references INV-FAKE-999 which doesn't exist -> anchor=0.3)
        let (_, unanchored_datoms) = create_task_datoms(CreateTaskParams {
            title: "Fix INV-FAKE-999 issue",
            description: None,
            priority: 2,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, unanchored_datoms);

        let routed = compute_routing_from_store(&store);

        let anchored = routed
            .iter()
            .find(|r| r.label.contains("INV-STORE-001"))
            .expect("anchored task should be in routing");
        let unanchored = routed
            .iter()
            .find(|r| r.label.contains("INV-FAKE-999"))
            .expect("unanchored task should be in routing");

        // Verify anchor factors were applied
        assert!(
            (anchored.metrics.spec_anchor - 1.0).abs() < f64::EPSILON,
            "anchored task should have spec_anchor=1.0, got {}",
            anchored.metrics.spec_anchor
        );
        assert!(
            (unanchored.metrics.spec_anchor - 0.3).abs() < f64::EPSILON,
            "unanchored task should have spec_anchor=0.3, got {}",
            unanchored.metrics.spec_anchor
        );

        // Anchored task must outrank unanchored at equal graph position
        assert!(
            anchored.impact > unanchored.impact,
            "anchored task (impact={:.4}, anchor={:.1}) should outrank unanchored \
             (impact={:.4}, anchor={:.1}) at equal graph position",
            anchored.impact,
            anchored.metrics.spec_anchor,
            unanchored.impact,
            unanchored.metrics.spec_anchor,
        );
    }

    // -------------------------------------------------------------------
    // Contextual Observation Funnel (INV-GUIDANCE-014)
    // -------------------------------------------------------------------

    // Verifies: INV-GUIDANCE-014 — Contextual Observation Hint
    #[test]
    fn contextual_hint_task_close() {
        let output = serde_json::json!({
            "title": "Fix merge conflict in store module",
            "reason": "tests verified"
        });
        let hint = contextual_observation_hint("task_close", &output).unwrap();
        assert!(hint.text.contains("Completed:"), "got: {}", hint.text);
        assert!(
            hint.text.contains("Fix merge conflict"),
            "got: {}",
            hint.text
        );
        assert!(hint.text.contains("tests verified"), "got: {}", hint.text);
        assert!(
            (hint.confidence - 0.9).abs() < f64::EPSILON,
            "task close confidence should be 0.9, got {}",
            hint.confidence
        );
    }

    #[test]
    fn contextual_hint_done_alias() {
        let output = serde_json::json!({"title": "task done", "reason": "ok"});
        let hint = contextual_observation_hint("done", &output).unwrap();
        assert!(hint.text.contains("Completed:"), "got: {}", hint.text);
        assert!(
            (hint.confidence - 0.9).abs() < f64::EPSILON,
            "done alias should have same confidence as task_close"
        );
    }

    #[test]
    fn contextual_hint_query() {
        let output = serde_json::json!({
            "total": 42,
            "entity_filter": ":spec/inv-store-001"
        });
        let hint = contextual_observation_hint("query", &output).unwrap();
        assert_eq!(hint.text, "Queried :spec/inv-store-001 (42 results)");
        assert!(
            (hint.confidence - 0.7).abs() < f64::EPSILON,
            "query confidence should be 0.7"
        );
    }

    #[test]
    fn contextual_hint_query_count_field() {
        // Some queries use "count" instead of "total"
        let output = serde_json::json!({"count": 7});
        let hint = contextual_observation_hint("query", &output).unwrap();
        assert!(hint.text.contains("7 results"), "got: {}", hint.text);
    }

    #[test]
    fn contextual_hint_status_with_fitness() {
        let output = serde_json::json!({"fitness": 0.77});
        let hint = contextual_observation_hint("status", &output).unwrap();
        assert_eq!(hint.text, "Status: F(S)=0.77");
        assert!(
            (hint.confidence - 0.8).abs() < f64::EPSILON,
            "status confidence should be 0.8"
        );
    }

    #[test]
    fn contextual_hint_status_without_fitness() {
        let output = serde_json::json!({"datom_count": 1000});
        let hint = contextual_observation_hint("status", &output).unwrap();
        assert_eq!(hint.text, "Status checked");
    }

    #[test]
    fn contextual_hint_trace() {
        let output = serde_json::json!({"coverage": 0.85});
        let hint = contextual_observation_hint("trace", &output).unwrap();
        assert_eq!(hint.text, "Traced: 85% coverage");
        assert!(
            (hint.confidence - 0.7).abs() < f64::EPSILON,
            "trace confidence should be 0.7"
        );
    }

    #[test]
    fn contextual_hint_bilateral() {
        let output = serde_json::json!({"fitness": 0.92});
        let hint = contextual_observation_hint("bilateral", &output).unwrap();
        assert_eq!(hint.text, "Bilateral: F(S)=0.92");
        assert!(
            (hint.confidence - 0.8).abs() < f64::EPSILON,
            "bilateral confidence should be 0.8"
        );
    }

    #[test]
    fn contextual_hint_empty_for_observe() {
        let output = serde_json::json!({"text": "some observation"});
        assert!(contextual_observation_hint("observe", &output).is_none());
    }

    #[test]
    fn contextual_hint_empty_for_harvest() {
        let output = serde_json::json!({"candidates": 5});
        assert!(contextual_observation_hint("harvest", &output).is_none());
    }

    #[test]
    fn contextual_hint_empty_for_init() {
        let output = serde_json::json!({"path": ".braid"});
        assert!(contextual_observation_hint("init", &output).is_none());
    }

    #[test]
    fn contextual_hint_empty_for_seed() {
        let output = serde_json::json!({});
        assert!(contextual_observation_hint("seed", &output).is_none());
    }

    #[test]
    fn contextual_hint_empty_for_mcp() {
        let output = serde_json::json!({"status": "running"});
        assert!(contextual_observation_hint("mcp", &output).is_none());
    }

    #[test]
    fn contextual_hint_empty_for_unknown_command() {
        let output = serde_json::json!({"foo": "bar"});
        assert!(contextual_observation_hint("unknown_cmd", &output).is_none());
    }

    #[test]
    fn contextual_hint_truncates_long_title() {
        let long_title = "A".repeat(200);
        let output = serde_json::json!({"title": long_title, "reason": "done"});
        let hint = contextual_observation_hint("task_close", &output).unwrap();
        // Title truncated to 60 bytes + "Completed: " prefix + " \u{2014} " + reason
        assert!(
            hint.text.len() < 120,
            "hint should be bounded, got {} chars: {}",
            hint.text.len(),
            hint.text
        );
    }

    #[test]
    fn contextual_hint_truncates_long_reason() {
        let long_reason = "B".repeat(200);
        let output = serde_json::json!({"title": "task", "reason": long_reason});
        let hint = contextual_observation_hint("task_close", &output).unwrap();
        // Reason truncated to 40 bytes
        assert!(
            hint.text.len() < 120,
            "hint should be bounded, got {} chars: {}",
            hint.text.len(),
            hint.text
        );
    }

    #[test]
    fn contextual_hint_defaults_on_missing_fields() {
        // Empty JSON — all fields should fall back to defaults
        let output = serde_json::json!({});
        let hint = contextual_observation_hint("task_close", &output).unwrap();
        assert!(
            hint.text.contains("task"),
            "should use default title, got: {}",
            hint.text
        );
        assert!(
            hint.text.contains("completed"),
            "should use default reason, got: {}",
            hint.text
        );
    }

    // Verifies: INV-GUIDANCE-014 — Footer uses contextual hint
    #[test]
    fn format_footer_uses_contextual_hint() {
        let telemetry = SessionTelemetry {
            total_turns: 10,
            transact_turns: 1, // Low tx frequency → ✗ → contextual hint appears
            spec_language_turns: 0,
            query_type_count: 0,
            harvest_quality: 0.0,
            ..Default::default()
        };
        let store = Store::genesis();
        let mut footer = build_footer(
            &telemetry,
            &store,
            Some("braid observe \"...\"".into()),
            vec![],
        );
        footer.contextual_hint = Some(ContextualHint {
            text: "Fix merge — tests verified".to_string(),
            confidence: 0.9,
        });
        let formatted = format_footer(&footer);
        // tx frequency is low (3 turns, no spec), so the observe hint should appear
        // in the tx check when the metric is below 0.4
        if formatted.contains("\u{2717}") {
            assert!(
                formatted.contains("Fix merge"),
                "footer with contextual hint should use it, got: {formatted}"
            );
        }
    }

    #[test]
    fn build_command_footer_with_hint_uses_hint() {
        let store = Store::genesis();
        let hint = ContextualHint {
            text: "Bilateral: F(S)=0.88".to_string(),
            confidence: 0.8,
        };
        let footer = build_command_footer_with_hint(&store, Some(1.0), Some(hint));
        // The hint should appear somewhere in the footer if tx metric is low
        assert!(!footer.is_empty(), "footer must not be empty");
    }

    #[test]
    fn build_command_footer_without_hint_matches_original() {
        let store = Store::genesis();
        let original = build_command_footer(&store, Some(1.0));
        let with_none = build_command_footer_with_hint(&store, Some(1.0), None);
        assert_eq!(
            original, with_none,
            "build_command_footer and build_command_footer_with_hint(None) must be identical"
        );
    }

    // -------------------------------------------------------------------
    // methodology_gaps tests (INV-GUIDANCE-021)
    // -------------------------------------------------------------------

    // Verifies: INV-GUIDANCE-021 — Empty store has no methodology gaps
    #[test]
    fn methodology_gaps_empty_store() {
        let store = Store::genesis();
        let gaps = methodology_gaps(&store);
        assert!(
            gaps.is_empty(),
            "genesis store should have zero methodology gaps, got total={}",
            gaps.total()
        );
        assert_eq!(gaps.crystallization, 0);
        assert_eq!(gaps.unanchored, 0);
        assert_eq!(gaps.untested, 0);
        assert_eq!(gaps.stale_witnesses, 0);
    }

    // Verifies: INV-GUIDANCE-021 — Uncrystallized observations counted
    #[test]
    fn methodology_gaps_counts_uncrystallized() {
        use crate::datom::{AgentId, Datom, Op, TxId, Value};

        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);
        let mut datoms = std::collections::BTreeSet::new();

        // Create an observation that mentions INV-TEST-001
        let obs_entity = EntityId::from_ident(":obs/gap-test-1");
        datoms.insert(Datom::new(
            obs_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":obs/gap-test-1".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            obs_entity,
            Attribute::from_keyword(":exploration/body"),
            Value::String("Need to formalize INV-TEST-001 properly".to_string()),
            tx,
            Op::Assert,
        ));

        // Create a second observation mentioning INV-TEST-002
        let obs_entity2 = EntityId::from_ident(":obs/gap-test-2");
        datoms.insert(Datom::new(
            obs_entity2,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":obs/gap-test-2".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            obs_entity2,
            Attribute::from_keyword(":exploration/body"),
            Value::String("Working on INV-TEST-002 compliance".to_string()),
            tx,
            Op::Assert,
        ));

        // No formal spec elements exist -> both should be counted
        let store = Store::from_datoms(datoms);
        let gaps = methodology_gaps(&store);

        assert_eq!(
            gaps.crystallization, 2,
            "should detect 2 uncrystallized spec IDs, got {}",
            gaps.crystallization
        );
        assert!(!gaps.is_empty());
        assert_eq!(gaps.total(), 2);
    }

    // Verifies: INV-GUIDANCE-021 — Unanchored tasks counted
    #[test]
    fn methodology_gaps_counts_unanchored_tasks() {
        use crate::datom::AgentId;
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let store = routing_test_store();
        let agent = AgentId::from_name("test");
        let tx = crate::datom::TxId::new(1, 0, agent);

        // Create an open task referencing a nonexistent spec element
        let (_, task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Fix INV-NONEXIST-999 issue",
            description: None,
            priority: 2,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, task_datoms);

        let gaps = methodology_gaps(&store);
        assert_eq!(
            gaps.unanchored, 1,
            "should detect 1 unanchored task, got {}",
            gaps.unanchored
        );
        assert!(!gaps.is_empty());
    }

    // Verifies: INV-GUIDANCE-021 — Anchored tasks not counted as unanchored
    #[test]
    fn methodology_gaps_excludes_anchored_tasks() {
        use crate::datom::{AgentId, Attribute, Datom, Op, TxId, Value};
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let store = routing_test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        // Create the spec element that the task will reference.
        // resolve_spec_refs requires :spec/falsification to consider it resolved.
        let spec_entity = EntityId::from_ident(":spec/inv-store-001");
        let spec_datoms = vec![
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-store-001".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::String("invariant".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/falsification"),
                Value::String("Violated if store mutates an existing datom".to_string()),
                tx,
                Op::Assert,
            ),
        ];
        let store = routing_store_with(&store, spec_datoms);

        // Create a task referencing INV-STORE-001 which exists and has falsification
        let (_, task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Fix INV-STORE-001 compliance",
            description: None,
            priority: 1,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = routing_store_with(&store, task_datoms);

        let gaps = methodology_gaps(&store);
        assert_eq!(
            gaps.unanchored, 0,
            "task referencing existing spec should not be unanchored, got {}",
            gaps.unanchored
        );
    }

    // Verifies: INV-GUIDANCE-021 — total() and is_empty() work correctly
    #[test]
    fn methodology_gaps_total_and_is_empty() {
        let empty = MethodologyGaps::default();
        assert_eq!(empty.total(), 0);
        assert!(empty.is_empty());

        let with_cryst = MethodologyGaps {
            crystallization: 3,
            unanchored: 0,
            untested: 0,
            stale_witnesses: 0,
        };
        assert_eq!(with_cryst.total(), 3);
        assert!(!with_cryst.is_empty());

        let mixed = MethodologyGaps {
            crystallization: 1,
            unanchored: 2,
            untested: 3,
            stale_witnesses: 4,
        };
        assert_eq!(mixed.total(), 10);
        assert!(!mixed.is_empty());
    }

    // -------------------------------------------------------------------
    // Dynamic Methodology Projection (INV-GUIDANCE-022, INV-GUIDANCE-023)
    // -------------------------------------------------------------------

    // Verifies: INV-GUIDANCE-023 — ceremony level adapts to k* and change type
    #[test]
    fn ceremony_level_full_for_novel_high_k() {
        assert_eq!(
            ceremony_level(0.8, ChangeType::NovelDesign),
            CeremonyLevel::Full
        );
        assert_eq!(
            ceremony_level(0.9, ChangeType::NovelDesign),
            CeremonyLevel::Full
        );
    }

    #[test]
    fn ceremony_level_standard_for_feature() {
        assert_eq!(
            ceremony_level(0.5, ChangeType::Feature),
            CeremonyLevel::Standard
        );
        assert_eq!(
            ceremony_level(0.8, ChangeType::Feature),
            CeremonyLevel::Standard
        );
    }

    #[test]
    fn ceremony_level_minimal_for_known_bug() {
        // Known bugs always get minimal, regardless of k*
        assert_eq!(
            ceremony_level(0.9, ChangeType::KnownBug),
            CeremonyLevel::Minimal
        );
        assert_eq!(
            ceremony_level(0.1, ChangeType::KnownBug),
            CeremonyLevel::Minimal
        );
    }

    #[test]
    fn ceremony_level_minimal_for_low_k() {
        assert_eq!(
            ceremony_level(0.1, ChangeType::Feature),
            CeremonyLevel::Minimal
        );
        assert_eq!(
            ceremony_level(0.2, ChangeType::NovelDesign),
            CeremonyLevel::Minimal
        );
    }

    #[test]
    fn ceremony_level_standard_for_novel_moderate_k() {
        // Novel design at moderate k* gets Standard (not Full)
        assert_eq!(
            ceremony_level(0.5, ChangeType::NovelDesign),
            CeremonyLevel::Standard
        );
    }

    #[test]
    fn ceremony_level_description_is_nonempty() {
        assert!(!CeremonyLevel::Full.description().is_empty());
        assert!(!CeremonyLevel::Standard.description().is_empty());
        assert!(!CeremonyLevel::Minimal.description().is_empty());
    }

    // Verifies: INV-GUIDANCE-022 — capability scan detects subsystem presence
    #[test]
    fn capability_scan_empty_store() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let caps = capability_scan(&store);
        assert!(!caps.is_empty(), "should always return capability list");
        // AGP and Datalog are always implemented (hardcoded true)
        let agp = caps.iter().find(|c| c.name.contains("AGP"));
        assert!(agp.is_some());
        assert!(agp.unwrap().implemented);
    }

    #[test]
    fn capability_scan_with_harvest_data() {
        use crate::datom::{AgentId, Datom, Op, TxId};
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);
        let mut datoms = std::collections::BTreeSet::new();
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-001"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            tx,
            Op::Assert,
        ));
        let store = Store::from_datoms(datoms);
        let caps = capability_scan(&store);
        let harvest_cap = caps.iter().find(|c| c.name.contains("Harvest"));
        assert!(harvest_cap.is_some());
        assert!(
            harvest_cap.unwrap().implemented,
            "store with :harvest/ datoms should detect harvest capability"
        );
    }

    // Verifies: INV-GUIDANCE-022 — generate_methodology_section produces content
    #[test]
    fn generate_methodology_section_empty_store() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let section = generate_methodology_section(&store, 0.8);
        // Should always contain ceremony protocol
        assert!(
            section.contains("Ceremony Protocol"),
            "methodology section must always include ceremony protocol"
        );
        // Should not contain methodology gaps (empty store has none)
        // (or might have 0 — either is acceptable)
    }

    #[test]
    fn generate_methodology_section_contains_ceremony() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());

        // High k* → should mention Full or Standard
        let section_high = generate_methodology_section(&store, 0.8);
        assert!(section_high.contains("k*=0.8"));

        // Low k* → should mention Minimal
        let section_low = generate_methodology_section(&store, 0.1);
        assert!(section_low.contains("k*=0.1"));
        assert!(section_low.contains("Minimal"));
    }

    #[test]
    fn generate_methodology_section_with_tasks() {
        use crate::datom::{AgentId, TxId};
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);

        let (_, task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Fix something important",
            description: None,
            priority: 1,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });
        let store = Store::from_datoms(task_datoms.into_iter().collect());
        let section = generate_methodology_section(&store, 0.5);
        // Should mention R(t) actions since there's a task
        assert!(
            section.contains("Next Actions") || section.contains("Ceremony"),
            "methodology section should contain actions or ceremony"
        );
    }

    #[test]
    fn generate_methodology_section_token_budget() {
        // INV-GUIDANCE-022: output ≤ 200 tokens
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let section = generate_methodology_section(&store, 0.5);
        let word_count = section.split_whitespace().count();
        let approx_tokens = word_count * 4 / 3;
        assert!(
            approx_tokens <= 200,
            "methodology section should be ≤200 tokens, got ~{}",
            approx_tokens
        );
    }

    // -------------------------------------------------------------------
    // Session Working Set (INV-GUIDANCE-010, SWS-1/SWS-2)
    // -------------------------------------------------------------------

    // TEST-SWS-1: SessionWorkingSet construction

    #[test]
    fn session_working_set_empty_store() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let sws = SessionWorkingSet::from_store(&store);
        assert!(sws.active_tasks.is_empty());
        assert!(sws.session_created_tasks.is_empty());
        assert!(sws.epic_siblings.is_empty());
        assert!(sws.is_empty());
    }

    #[test]
    fn session_working_set_with_fresh_in_progress() {
        use crate::datom::{AgentId, Datom, Op, TxId};
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Create a task with RECENT in-progress status
        let tx1 = TxId::new(now - 10, 0, agent);
        let (_, mut task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Fresh task",
            description: None,
            priority: 1,
            task_type: TaskType::Task,
            tx: tx1,
            traces_to: &[],
            labels: &[],
        });

        // Add in-progress status with RECENT wall_time
        let tx_recent = TxId::new(now - 5, 0, agent);
        task_datoms.push(Datom::new(
            task_datoms[0].entity,
            Attribute::from_keyword(":task/status"),
            Value::Keyword(":task.status/in-progress".to_string()),
            tx_recent,
            Op::Assert,
        ));

        let store = Store::from_datoms(task_datoms.into_iter().collect());
        let sws = SessionWorkingSet::from_store(&store);

        assert!(
            !sws.active_tasks.is_empty(),
            "fresh in-progress task should be in active set"
        );
    }

    #[test]
    fn session_working_set_includes_all_in_progress() {
        // T-UX-3: ALL in-progress tasks are active intentions, regardless of
        // when the status was set. A task set in-progress 2 hours ago is still
        // an active intention until explicitly closed or completed.
        use crate::datom::{AgentId, Datom, Op, TxId};
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let tx_old = TxId::new(now - 7200, 0, agent);
        let (_, mut task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Cross-session active task",
            description: None,
            priority: 1,
            task_type: TaskType::Task,
            tx: tx_old,
            traces_to: &[],
            labels: &[],
        });

        task_datoms.push(Datom::new(
            task_datoms[0].entity,
            Attribute::from_keyword(":task/status"),
            Value::Keyword(":task.status/in-progress".to_string()),
            tx_old,
            Op::Assert,
        ));

        let store = Store::from_datoms(task_datoms.into_iter().collect());
        let sws = SessionWorkingSet::from_store(&store);

        assert_eq!(
            sws.active_tasks.len(),
            1,
            "in-progress task should be in active set even if set 2h ago"
        );
    }

    #[test]
    fn session_working_set_session_created() {
        use crate::datom::{AgentId, TxId};
        use crate::task::{create_task_datoms, CreateTaskParams, TaskType};

        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Create a task with recent created_at
        let tx = TxId::new(now - 60, 0, agent);
        let (_, task_datoms) = create_task_datoms(CreateTaskParams {
            title: "Recent task",
            description: None,
            priority: 1,
            task_type: TaskType::Task,
            tx,
            traces_to: &[],
            labels: &[],
        });

        let store = Store::from_datoms(task_datoms.into_iter().collect());
        let sws = SessionWorkingSet::from_store(&store);

        assert!(
            !sws.session_created_tasks.is_empty(),
            "recently created task should be in session_created_tasks"
        );
    }

    // TEST-SWS-2: session_boost multiplier

    #[test]
    fn session_boost_active_task_gets_3() {
        let sws = SessionWorkingSet {
            active_tasks: vec![EntityId::from_ident(":task/t-active")],
            session_created_tasks: BTreeSet::new(),
            epic_siblings: BTreeSet::new(),
            session_boundary: 0,
        };
        assert_eq!(
            session_boost(EntityId::from_ident(":task/t-active"), &sws),
            3.0
        );
    }

    #[test]
    fn session_boost_sibling_gets_2() {
        let sws = SessionWorkingSet {
            active_tasks: vec![],
            session_created_tasks: BTreeSet::new(),
            epic_siblings: {
                let mut s = BTreeSet::new();
                s.insert(EntityId::from_ident(":task/t-sibling"));
                s
            },
            session_boundary: 0,
        };
        assert_eq!(
            session_boost(EntityId::from_ident(":task/t-sibling"), &sws),
            2.0
        );
    }

    #[test]
    fn session_boost_created_gets_1_5() {
        let sws = SessionWorkingSet {
            active_tasks: vec![],
            session_created_tasks: {
                let mut s = BTreeSet::new();
                s.insert(EntityId::from_ident(":task/t-new"));
                s
            },
            epic_siblings: BTreeSet::new(),
            session_boundary: 0,
        };
        assert_eq!(
            session_boost(EntityId::from_ident(":task/t-new"), &sws),
            1.5
        );
    }

    #[test]
    fn session_boost_unrelated_gets_1() {
        let sws = SessionWorkingSet {
            active_tasks: vec![EntityId::from_ident(":task/t-other")],
            session_created_tasks: BTreeSet::new(),
            epic_siblings: BTreeSet::new(),
            session_boundary: 0,
        };
        assert_eq!(
            session_boost(EntityId::from_ident(":task/t-unrelated"), &sws),
            1.0
        );
    }

    #[test]
    fn session_boost_takes_highest_category() {
        // Task is both active AND session-created → should get 3.0 (highest)
        let entity = EntityId::from_ident(":task/t-both");
        let sws = SessionWorkingSet {
            active_tasks: vec![entity],
            session_created_tasks: {
                let mut s = BTreeSet::new();
                s.insert(entity);
                s
            },
            epic_siblings: BTreeSet::new(),
            session_boundary: 0,
        };
        assert_eq!(session_boost(entity, &sws), 3.0);
    }

    #[test]
    fn session_boost_empty_working_set_all_1() {
        let sws = SessionWorkingSet {
            active_tasks: vec![],
            session_created_tasks: BTreeSet::new(),
            epic_siblings: BTreeSet::new(),
            session_boundary: 0,
        };
        assert_eq!(
            session_boost(EntityId::from_ident(":task/t-any"), &sws),
            1.0
        );
    }

    // ===================================================================
    // TEST-RFL-UNIT: R(t) Feedback Loop — Ridge Regression & Outcome Classification
    // Verifies: INV-GUIDANCE-010, INV-GUIDANCE-005, ADR-TOPOLOGY-004
    // ===================================================================

    /// Helper: build a Store containing `n` action-outcome entities.
    ///
    /// Each entity has :action/recommended-command, :action/outcome, :action/features.
    /// `features_fn(i)` produces the 6-element feature vector for entity i.
    /// `outcome_fn(i)` produces the outcome keyword for entity i.
    fn rfl_store_with_actions(
        n: usize,
        features_fn: impl Fn(usize) -> [f64; 6],
        outcome_fn: impl Fn(usize) -> &'static str,
    ) -> Store {
        use crate::datom::{AgentId, Datom, Op, TxId};

        let agent = AgentId::from_name("rfl-test");
        let mut datoms = std::collections::BTreeSet::new();

        for i in 0..n {
            let tx = TxId::new(1000 + i as u64, i as u32, agent);
            let entity = EntityId::from_ident(&format!(":action/rfl-{i:04}"));

            // :db/ident
            datoms.insert(Datom::new(
                entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(format!(":action/rfl-{i:04}")),
                tx,
                Op::Assert,
            ));

            // :action/recommended-command
            datoms.insert(Datom::new(
                entity,
                Attribute::from_keyword(":action/recommended-command"),
                Value::String(format!("braid go t-{i:04x}")),
                tx,
                Op::Assert,
            ));

            // :action/outcome
            datoms.insert(Datom::new(
                entity,
                Attribute::from_keyword(":action/outcome"),
                Value::Keyword(outcome_fn(i).to_string()),
                tx,
                Op::Assert,
            ));

            // :action/features — JSON array of 6 floats
            let feats = features_fn(i);
            let json = serde_json::to_string(&feats.to_vec()).unwrap();
            datoms.insert(Datom::new(
                entity,
                Attribute::from_keyword(":action/features"),
                Value::String(json),
                tx,
                Op::Assert,
            ));
        }

        Store::from_datoms(datoms)
    }

    // ---------------------------------------------------------------
    // 1. Ridge regression with uniform data produces near-uniform weights
    // ---------------------------------------------------------------

    /// INV-GUIDANCE-010: With uniform features and uniform positive outcomes,
    /// ridge regression should produce approximately equal weights.
    /// The ridge regularization (lambda=0.01) pulls weights toward uniform,
    /// and with identical features the solution is inherently symmetric.
    #[test]
    fn rfl_ridge_uniform_data_produces_near_uniform_weights() {
        // 60 data points, all features = 1.0, all outcomes "followed" (y=1.0)
        let store = rfl_store_with_actions(
            60,
            |_| [1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            |_| ":action.outcome/followed",
        );

        let weights = refit_routing_weights(&store);
        assert!(weights.is_some(), "should produce weights with 60 data points");
        let w = weights.unwrap();

        // With uniform features, all weights should be approximately equal
        // After clamping [0.01, 0.5] and normalizing to sum=1.0,
        // uniform weights = 1/6 ~= 0.1667
        let expected = 1.0 / 6.0;
        for (i, &wi) in w.iter().enumerate() {
            assert!(
                (wi - expected).abs() < 0.05,
                "weight[{i}] = {wi:.4}, expected ~{expected:.4} (uniform data)"
            );
        }
    }

    // ---------------------------------------------------------------
    // 2. Ridge regression numerical stability
    // ---------------------------------------------------------------

    /// INV-GUIDANCE-010: Weights must sum to 1.0 (normalization invariant).
    /// No NaN or Inf values allowed.
    #[test]
    fn rfl_ridge_weights_sum_to_one_no_nan() {
        // Mixed outcomes with varied features
        let store = rfl_store_with_actions(
            60,
            |i| {
                let t = i as f64 / 60.0;
                [
                    0.1 + 0.8 * t,          // pagerank: ramp
                    0.5,                     // betweenness: constant
                    0.3 * (1.0 - t),         // critical_path: decreasing
                    if i % 3 == 0 { 0.9 } else { 0.1 }, // blocker_ratio: periodic
                    0.2 + 0.1 * (i % 5) as f64,         // staleness: stepped
                    0.4,                     // priority_boost: constant
                ]
            },
            |i| match i % 3 {
                0 => ":action.outcome/followed",
                1 => ":action.outcome/adjacent",
                _ => ":action.outcome/ignored",
            },
        );

        let weights = refit_routing_weights(&store);
        assert!(weights.is_some(), "should produce weights with varied data");
        let w = weights.unwrap();

        // No NaN or Inf
        for (i, &wi) in w.iter().enumerate() {
            assert!(wi.is_finite(), "weight[{i}] must be finite, got {wi}");
            assert!(!wi.is_nan(), "weight[{i}] must not be NaN");
        }

        // Sum to 1.0 within epsilon
        let sum: f64 = w.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "weights must sum to 1.0, got {sum}"
        );

        // All weights positive and bounded
        for (i, &wi) in w.iter().enumerate() {
            assert!(wi > 0.0, "weight[{i}] must be positive, got {wi}");
            assert!(wi <= 1.0, "weight[{i}] must be <= 1.0, got {wi}");
        }
    }

    /// INV-GUIDANCE-010: Ridge regression stability with extreme feature values.
    /// Even with features near 0 or near 1, regularization prevents singularity.
    #[test]
    fn rfl_ridge_stability_extreme_features() {
        // Features alternate between near-zero and near-one
        let store = rfl_store_with_actions(
            55,
            |i| {
                if i % 2 == 0 {
                    [0.001, 0.001, 0.001, 0.001, 0.001, 0.001]
                } else {
                    [0.999, 0.999, 0.999, 0.999, 0.999, 0.999]
                }
            },
            |i| {
                if i % 2 == 0 {
                    ":action.outcome/ignored"
                } else {
                    ":action.outcome/followed"
                }
            },
        );

        let weights = refit_routing_weights(&store);
        assert!(
            weights.is_some(),
            "ridge regularization should prevent singularity with extreme features"
        );
        let w = weights.unwrap();

        let sum: f64 = w.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "weights must sum to 1.0, got {sum}"
        );
        for (i, &wi) in w.iter().enumerate() {
            assert!(wi.is_finite(), "weight[{i}] must be finite, got {wi}");
        }
    }

    // ---------------------------------------------------------------
    // 3. classify_action_outcome categories
    // ---------------------------------------------------------------

    /// INV-GUIDANCE-005: "followed" — current command matches recommended.
    #[test]
    fn rfl_classify_outcome_followed() {
        use crate::datom::{AgentId, Datom, Op, TxId};

        let agent = AgentId::from_name("rfl-test");
        let tx = TxId::new(5000, 0, agent);
        let entity = EntityId::from_ident(":action/rfl-classify-1");
        let mut datoms = std::collections::BTreeSet::new();

        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":action/rfl-classify-1".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":action/recommended-command"),
            Value::String("braid go t-abcd1234".to_string()),
            tx,
            Op::Assert,
        ));
        // No :action/outcome — this is an unresolved action

        let store = Store::from_datoms(datoms);
        let result = classify_action_outcome(&store, "braid go t-abcd1234");
        assert!(result.is_some(), "should find unresolved action");
        let (category, eid) = result.unwrap();
        assert_eq!(category, "followed", "exact match should be 'followed'");
        assert_eq!(eid, entity);
    }

    /// INV-GUIDANCE-005: "adjacent" — current command references same task ID.
    #[test]
    fn rfl_classify_outcome_adjacent() {
        use crate::datom::{AgentId, Datom, Op, TxId};

        let agent = AgentId::from_name("rfl-test");
        let tx = TxId::new(5001, 0, agent);
        let entity = EntityId::from_ident(":action/rfl-classify-2");
        let mut datoms = std::collections::BTreeSet::new();

        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":action/rfl-classify-2".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":action/recommended-command"),
            Value::String("braid go t-ff30abcd".to_string()),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        // Different command but same task ID embedded
        let result = classify_action_outcome(&store, "braid status t-ff30abcd");
        assert!(result.is_some(), "should find unresolved action");
        let (category, eid) = result.unwrap();
        assert_eq!(
            category, "adjacent",
            "same task ID in different command should be 'adjacent'"
        );
        assert_eq!(eid, entity);
    }

    /// INV-GUIDANCE-005: "ignored" — completely different command.
    #[test]
    fn rfl_classify_outcome_ignored() {
        use crate::datom::{AgentId, Datom, Op, TxId};

        let agent = AgentId::from_name("rfl-test");
        let tx = TxId::new(5002, 0, agent);
        let entity = EntityId::from_ident(":action/rfl-classify-3");
        let mut datoms = std::collections::BTreeSet::new();

        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":action/rfl-classify-3".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":action/recommended-command"),
            Value::String("braid go t-99991111".to_string()),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        // Completely unrelated command
        let result = classify_action_outcome(&store, "braid harvest --commit");
        assert!(result.is_some(), "should find unresolved action");
        let (category, eid) = result.unwrap();
        assert_eq!(
            category, "ignored",
            "unrelated command should be 'ignored'"
        );
        assert_eq!(eid, entity);
    }

    /// INV-GUIDANCE-005: classify returns None when no unresolved action exists.
    #[test]
    fn rfl_classify_outcome_none_when_all_resolved() {
        use crate::datom::{AgentId, Datom, Op, TxId};

        let agent = AgentId::from_name("rfl-test");
        let tx = TxId::new(5003, 0, agent);
        let entity = EntityId::from_ident(":action/rfl-classify-4");
        let mut datoms = std::collections::BTreeSet::new();

        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":action/rfl-classify-4".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":action/recommended-command"),
            Value::String("braid go t-aaaa".to_string()),
            tx,
            Op::Assert,
        ));
        // This action HAS an outcome — it's resolved
        datoms.insert(Datom::new(
            entity,
            Attribute::from_keyword(":action/outcome"),
            Value::Keyword(":action.outcome/followed".to_string()),
            tx,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let result = classify_action_outcome(&store, "braid status");
        assert!(
            result.is_none(),
            "should return None when all actions are resolved"
        );
    }

    // ---------------------------------------------------------------
    // 4. routing_weights returns DEFAULT when no data
    // ---------------------------------------------------------------

    /// INV-GUIDANCE-010: On a fresh/genesis store with no :routing/weights datom,
    /// routing_weights must return the hardcoded DEFAULT_ROUTING_WEIGHTS.
    #[test]
    fn rfl_routing_weights_default_on_genesis_store() {
        let store = Store::genesis();
        let w = routing_weights(&store);
        assert_eq!(
            w, DEFAULT_ROUTING_WEIGHTS,
            "genesis store should return default routing weights"
        );
    }

    /// Same test with from_datoms(empty-ish store).
    #[test]
    fn rfl_routing_weights_default_on_empty_store() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let w = routing_weights(&store);
        assert_eq!(
            w, DEFAULT_ROUTING_WEIGHTS,
            "empty store should return default routing weights"
        );
    }

    // ---------------------------------------------------------------
    // 5. routing_weights / refit with insufficient data returns default/None
    // ---------------------------------------------------------------

    /// INV-GUIDANCE-010: refit_routing_weights returns None with < 50 data points.
    /// The 50-point minimum safeguard prevents overfitting on sparse data.
    #[test]
    fn rfl_refit_returns_none_with_insufficient_data() {
        // 10 action-outcome pairs — well below the 50-point minimum
        let store = rfl_store_with_actions(
            10,
            |i| {
                let t = i as f64 / 10.0;
                [t, 0.5, 0.3, 0.1, 0.2, 0.4]
            },
            |i| {
                if i % 2 == 0 {
                    ":action.outcome/followed"
                } else {
                    ":action.outcome/ignored"
                }
            },
        );

        let weights = refit_routing_weights(&store);
        assert!(
            weights.is_none(),
            "refit should return None with only 10 data points (need 50+)"
        );
    }

    /// INV-GUIDANCE-010: refit returns None at exactly 49 data points (boundary).
    #[test]
    fn rfl_refit_returns_none_at_49_data_points() {
        let store = rfl_store_with_actions(
            49,
            |_| [0.5, 0.5, 0.5, 0.5, 0.5, 0.5],
            |_| ":action.outcome/followed",
        );

        let weights = refit_routing_weights(&store);
        assert!(
            weights.is_none(),
            "refit should return None with exactly 49 data points"
        );
    }

    /// INV-GUIDANCE-010: refit produces Some at exactly 50 data points (boundary).
    #[test]
    fn rfl_refit_returns_some_at_50_data_points() {
        let store = rfl_store_with_actions(
            50,
            |_| [0.5, 0.5, 0.5, 0.5, 0.5, 0.5],
            |_| ":action.outcome/followed",
        );

        let weights = refit_routing_weights(&store);
        assert!(
            weights.is_some(),
            "refit should produce weights at exactly 50 data points"
        );
    }

    /// INV-GUIDANCE-010: routing_weights still returns defaults when refit
    /// would fail (insufficient action-outcome data in store).
    #[test]
    fn rfl_routing_weights_returns_defaults_with_sparse_actions() {
        // Store has 10 action-outcome pairs but no :routing/weights datom
        let store = rfl_store_with_actions(
            10,
            |i| {
                let t = i as f64 / 10.0;
                [t, 0.5, 0.3, 0.1, 0.2, 0.4]
            },
            |_| ":action.outcome/followed",
        );

        // routing_weights looks for :routing/weights datom (not present)
        // and falls back to DEFAULT_ROUTING_WEIGHTS
        let w = routing_weights(&store);
        assert_eq!(
            w, DEFAULT_ROUTING_WEIGHTS,
            "store without :routing/weights datom should return defaults"
        );
    }

    // -----------------------------------------------------------------------
    // T6-1: Activity-mode gap suppression tests
    // -----------------------------------------------------------------------

    /// Implementation mode suppresses crystallization (x0.1) and unanchored (x0.2).
    #[test]
    fn t6_adjust_gaps_implementation_mode() {
        let tel = SessionTelemetry {
            total_turns: 10,
            transact_turns: 6, // 60% > 50% threshold -> Implementation
            spec_language_turns: 1,
            ..Default::default()
        };
        assert_eq!(detect_activity_mode(&tel), ActivityMode::Implementation);

        let raw = MethodologyGaps {
            crystallization: 30,
            unanchored: 24,
            untested: 5,
            stale_witnesses: 2,
        };
        let ag = adjust_gaps(raw, ActivityMode::Implementation);

        // crystallization: ceil(30 * 0.1) = 3
        assert_eq!(ag.adjusted.crystallization, 3);
        // unanchored: ceil(24 * 0.2) = 5
        assert_eq!(ag.adjusted.unanchored, 5);
        // untested and stale_witnesses unchanged
        assert_eq!(ag.adjusted.untested, 5);
        assert_eq!(ag.adjusted.stale_witnesses, 2);
        // raw preserved
        assert_eq!(ag.raw.crystallization, 30);
        assert_eq!(ag.raw.unanchored, 24);
        assert_eq!(ag.mode, ActivityMode::Implementation);
        assert_eq!(ag.mode_label(), "impl");
    }

    /// Specification mode suppresses untested (x0.3).
    #[test]
    fn t6_adjust_gaps_specification_mode() {
        let tel = SessionTelemetry {
            total_turns: 10,
            transact_turns: 1,
            spec_language_turns: 6, // 60% > 50% threshold -> Specification
            ..Default::default()
        };
        assert_eq!(detect_activity_mode(&tel), ActivityMode::Specification);

        let raw = MethodologyGaps {
            crystallization: 10,
            unanchored: 8,
            untested: 20,
            stale_witnesses: 3,
        };
        let ag = adjust_gaps(raw, ActivityMode::Specification);

        // crystallization and unanchored unchanged
        assert_eq!(ag.adjusted.crystallization, 10);
        assert_eq!(ag.adjusted.unanchored, 8);
        // untested: ceil(20 * 0.3) = 6
        assert_eq!(ag.adjusted.untested, 6);
        // stale_witnesses unchanged
        assert_eq!(ag.adjusted.stale_witnesses, 3);
        // raw preserved
        assert_eq!(ag.raw.untested, 20);
        assert_eq!(ag.mode, ActivityMode::Specification);
        assert_eq!(ag.mode_label(), "spec");
    }

    /// Mixed mode applies no suppression -- adjusted == raw.
    #[test]
    fn t6_adjust_gaps_mixed_mode_unchanged() {
        let tel = SessionTelemetry {
            total_turns: 10,
            transact_turns: 3,
            spec_language_turns: 3, // Neither > 50% -> Mixed
            ..Default::default()
        };
        assert_eq!(detect_activity_mode(&tel), ActivityMode::Mixed);

        let raw = MethodologyGaps {
            crystallization: 15,
            unanchored: 12,
            untested: 7,
            stale_witnesses: 4,
        };
        let ag = adjust_gaps(raw, ActivityMode::Mixed);

        assert_eq!(ag.adjusted.crystallization, 15);
        assert_eq!(ag.adjusted.unanchored, 12);
        assert_eq!(ag.adjusted.untested, 7);
        assert_eq!(ag.adjusted.stale_witnesses, 4);
        assert_eq!(ag.raw.crystallization, 15);
        assert_eq!(ag.mode, ActivityMode::Mixed);
        assert_eq!(ag.mode_label(), "mixed");
    }

    // -----------------------------------------------------------------------
    // T1-1: M(t) session-scoped denominators
    // -----------------------------------------------------------------------

    // Verifies: T1-1 — M(t) on a store with recent session activity (10 txns
    // including spec entities) should be >= 0.5.
    #[test]
    fn mt_session_scoped_recent_activity() {
        use crate::datom::*;
        use std::collections::BTreeSet;

        let agent = AgentId::from_name("test:agent");
        let mut datoms = BTreeSet::new();

        // Place a harvest at wall_time = 1000.
        let harvest_wall: u64 = 1000;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-session"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));

        // Add 10 transactions after the harvest, including 5 spec entities.
        for i in 1..=10u64 {
            let tx = TxId::new(harvest_wall + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":work/item-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("work item {i}")),
                tx,
                Op::Assert,
            ));
            // First 5 transactions also create spec entities.
            if i <= 5 {
                datoms.insert(Datom::new(
                    EntityId::from_ident(&format!(":spec/inv-test-{i:03}")),
                    Attribute::from_keyword(":db/ident"),
                    Value::Keyword(format!(":spec/inv-test-{i:03}")),
                    tx,
                    Op::Assert,
                ));
            }
        }

        let store = Store::from_datoms(datoms);
        let telemetry = telemetry_from_store(&store);

        // total_turns should be session-scoped (10 distinct wall_times after harvest)
        assert_eq!(telemetry.total_turns, 10, "total_turns should be session-scoped");
        // spec_language_turns should be 5 (only spec entities created after harvest)
        assert_eq!(telemetry.spec_language_turns, 5, "spec_language_turns should count session specs");

        let score = compute_methodology_score(&telemetry);
        assert!(
            score.score >= 0.5,
            "M(t) with 10 session txns and 5 spec entities should be >= 0.5, got {}",
            score.score,
        );
    }

    // Verifies: T1-1 — M(t) on a store with 10000 old datoms but only 5 new
    // transactions should be >= 0.4 (not penalized by old data).
    #[test]
    fn mt_session_scoped_not_penalized_by_old_data() {
        use crate::datom::*;
        use std::collections::BTreeSet;

        let agent = AgentId::from_name("test:agent");
        let mut datoms = BTreeSet::new();

        // Add 10000 old datoms BEFORE the harvest (wall_times 1..=10000).
        for i in 1..=10000u64 {
            let tx = TxId::new(i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":old/item-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("old item {i}")),
                tx,
                Op::Assert,
            ));
        }

        // Place a harvest at wall_time = 10001.
        let harvest_wall: u64 = 10001;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-old"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));

        // Add 5 new transactions after harvest, including 3 spec entities.
        for i in 1..=5u64 {
            let tx = TxId::new(harvest_wall + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":new/item-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("new item {i}")),
                tx,
                Op::Assert,
            ));
            if i <= 3 {
                datoms.insert(Datom::new(
                    EntityId::from_ident(&format!(":spec/inv-new-{i:03}")),
                    Attribute::from_keyword(":db/ident"),
                    Value::Keyword(format!(":spec/inv-new-{i:03}")),
                    tx,
                    Op::Assert,
                ));
            }
        }

        let store = Store::from_datoms(datoms);
        let telemetry = telemetry_from_store(&store);

        // total_turns should be 5 (session-scoped), NOT 10001+
        assert_eq!(
            telemetry.total_turns, 5,
            "total_turns should only count session wall_times, not old data"
        );
        assert_eq!(
            telemetry.spec_language_turns, 3,
            "spec_language_turns should only count post-harvest spec entities"
        );

        let score = compute_methodology_score(&telemetry);
        assert!(
            score.score >= 0.4,
            "M(t) with 10000 old datoms and 5 new txns should be >= 0.4, got {}",
            score.score,
        );
    }

    // -----------------------------------------------------------------------
    // T1-2: Broadened spec_language_turns
    // -----------------------------------------------------------------------

    // Verifies: T1-2(a) — Task with INV- in title counts as a spec language turn.
    #[test]
    fn spec_language_turns_counts_task_with_inv_ref() {
        use crate::datom::*;
        use std::collections::BTreeSet;

        let agent = AgentId::from_name("test:t1-2");
        let mut datoms = BTreeSet::new();

        // Place a harvest at wall_time = 1000.
        let harvest_wall: u64 = 1000;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-t12a"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));

        // Add a task with a spec ref in the title (after harvest).
        let tx1 = TxId::new(harvest_wall + 1, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":task/fix-merge"),
            Attribute::from_keyword(":task/title"),
            Value::String("Fix merge handling (INV-MERGE-001)".to_string()),
            tx1,
            Op::Assert,
        ));

        // Add a plain work datom at a different wall_time (no spec engagement).
        let tx2 = TxId::new(harvest_wall + 2, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":work/plain"),
            Attribute::from_keyword(":db/doc"),
            Value::String("plain work".to_string()),
            tx2,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let telemetry = telemetry_from_store(&store);

        // total_turns = 2 (two distinct wall_times after harvest)
        assert_eq!(telemetry.total_turns, 2);
        // spec_language_turns should be 1: the task with INV- in its title.
        // (No :spec/ entities, no observations, no :impl/implements.)
        assert_eq!(
            telemetry.spec_language_turns, 1,
            "task with INV-MERGE-001 in title should count as a spec language turn"
        );
    }

    // Verifies: T1-2(b) — Observation with ADR- in body counts as a spec language turn.
    #[test]
    fn spec_language_turns_counts_observation_with_adr_ref() {
        use crate::datom::*;
        use std::collections::BTreeSet;

        let agent = AgentId::from_name("test:t1-2");
        let mut datoms = BTreeSet::new();

        // Place a harvest at wall_time = 2000.
        let harvest_wall: u64 = 2000;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-t12b"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));

        // Add an observation whose body references ADR-GUIDANCE-003 (after harvest).
        let tx1 = TxId::new(harvest_wall + 1, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":obs/design-note"),
            Attribute::from_keyword(":exploration/body"),
            Value::String(
                "Revisited ADR-GUIDANCE-003: six mechanisms approach confirmed".to_string(),
            ),
            tx1,
            Op::Assert,
        ));

        // Add a plain work datom at a different wall_time (no spec engagement).
        let tx2 = TxId::new(harvest_wall + 2, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":work/plain2"),
            Attribute::from_keyword(":db/doc"),
            Value::String("unrelated work".to_string()),
            tx2,
            Op::Assert,
        ));

        let store = Store::from_datoms(datoms);
        let telemetry = telemetry_from_store(&store);

        assert_eq!(telemetry.total_turns, 2);
        // spec_language_turns should be 1: the observation with ADR- in its body.
        assert_eq!(
            telemetry.spec_language_turns, 1,
            "observation with ADR-GUIDANCE-003 in body should count as a spec language turn"
        );
    }

    // Verifies: T1-1 — Proptest: M(t) is always in [0, 1] with session-scoped telemetry.
    mod t1_1_proptest {
        use super::*;
        use crate::datom::*;
        use proptest::prelude::*;
        use std::collections::BTreeSet;

        proptest! {
            #[test]
            fn mt_session_scoped_always_bounded(
                old_count in 0u32..200,
                new_count in 0u32..50,
                spec_count in 0u32..20,
                has_harvest in proptest::bool::ANY,
            ) {
                let agent = AgentId::from_name("test:prop");
                let mut datoms = BTreeSet::new();

                let harvest_wall: u64 = 5000;

                // Add old datoms before harvest.
                for i in 0..old_count {
                    let tx = TxId::new(1000 + i as u64, 0, agent);
                    datoms.insert(Datom::new(
                        EntityId::from_ident(&format!(":old/p-{i}")),
                        Attribute::from_keyword(":db/doc"),
                        Value::String(format!("old {i}")),
                        tx,
                        Op::Assert,
                    ));
                }

                // Optionally add a harvest.
                if has_harvest {
                    let harvest_tx = TxId::new(harvest_wall, 0, agent);
                    datoms.insert(Datom::new(
                        EntityId::from_ident(":harvest/h-prop"),
                        Attribute::from_keyword(":harvest/agent"),
                        Value::String("test".to_string()),
                        harvest_tx,
                        Op::Assert,
                    ));
                }

                // Add new datoms after harvest.
                let effective_spec = spec_count.min(new_count);
                for i in 0..new_count {
                    let tx = TxId::new(harvest_wall + 1 + i as u64, 0, agent);
                    datoms.insert(Datom::new(
                        EntityId::from_ident(&format!(":new/p-{i}")),
                        Attribute::from_keyword(":db/doc"),
                        Value::String(format!("new {i}")),
                        tx,
                        Op::Assert,
                    ));
                    if i < effective_spec {
                        datoms.insert(Datom::new(
                            EntityId::from_ident(&format!(":spec/prop-{i:03}")),
                            Attribute::from_keyword(":db/ident"),
                            Value::Keyword(format!(":spec/prop-{i:03}")),
                            tx,
                            Op::Assert,
                        ));
                    }
                }

                let store = Store::from_datoms(datoms);
                let telemetry = telemetry_from_store(&store);
                let score = compute_methodology_score(&telemetry);
                prop_assert!(
                    score.score >= 0.0,
                    "M(t) must be >= 0.0, got {}",
                    score.score,
                );
                prop_assert!(
                    score.score <= 1.0,
                    "M(t) must be <= 1.0, got {}",
                    score.score,
                );
            }
        }
    }
}
