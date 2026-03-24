//! R(t) routing — graph-based work prioritization and impact scoring.
//!
//! Extracted from `guidance.rs` to reduce module size. Contains:
//! - **R(t)**: Graph-based work routing (INV-GUIDANCE-010).
//! - **TaskNode/TaskRouting**: Task graph nodes and routing results.
//! - **Routing weights**: Default and learned (ridge regression, RFL-4).
//! - **Hypothesis ledger**: Predictive action recording (HL-2, ADR-FOUNDATION-018).
//! - **Calibration**: Hypothesis outcome tracking (HL-4).
//! - **Action computation**: Single code path for ACP (INV-BUDGET-009).
//! - **Spec anchor**: Task-to-spec resolution scoring (SFE-3.1).
//! - **Action classification**: Follow-through tracking (RFL-3).

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{AcquisitionScore, ObservationCost, ObservationKind};
use crate::datom::{Attribute, EntityId, Op, Value};
use crate::methodology::{count_txns_since_last_harvest, last_harvest_wall_time};
use crate::store::Store;

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
    /// UAQ acquisition score (ADR-FOUNDATION-025).
    /// Populated by `compute_routing_from_store` with all four factors.
    /// `compute_routing` (pure graph algorithm) sets defaults that
    /// `compute_routing_from_store` enriches with store-derived signals.
    pub acquisition_score: AcquisitionScore,
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
    /// CE-6: Projected fitness delta magnitude from gradient routing.
    /// The exact projected change in F(S) if this task were completed.
    pub gradient_delta: f64,
}

/// R(t) routing weights (defaults from spec).
pub const DEFAULT_ROUTING_WEIGHTS: [f64; 6] = [0.25, 0.25, 0.20, 0.15, 0.10, 0.05];

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
        let outcome = entity_datoms
            .iter()
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
        let features = entity_datoms
            .iter()
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
fn solve_linear_system_6x6(
    a: &[[f64; N_FEATURES]; N_FEATURES],
    b: &[f64; N_FEATURES],
) -> Option<[f64; N_FEATURES]> {
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
                let weighted_blocks: f64 = task
                    .blocks
                    .iter()
                    .map(|blocked_entity| {
                        tasks
                            .iter()
                            .find(|n| n.entity == *blocked_entity)
                            .map(|n| n.task_type.type_multiplier())
                            .unwrap_or(1.0)
                    })
                    .sum();
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
                gradient_delta: 0.0, // Populated in compute_routing_from_store
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

            // UAQ-2: Initialize acquisition score with graph-derived impact.
            // Novelty, relevance, confidence default to 1.0 here; enriched
            // by compute_routing_from_store with store-derived signals.
            let acquisition_score = AcquisitionScore::from_factors(
                ObservationKind::Task,
                impact,
                1.0, // relevance: enriched in compute_routing_from_store
                1.0, // novelty: enriched in compute_routing_from_store
                1.0, // confidence: enriched in compute_routing_from_store
                ObservationCost::zero(), // cost: enriched in compute_routing_from_store
            );

            TaskRouting {
                entity: task.entity,
                label: task.label.clone(),
                impact,
                metrics,
                acquisition_score,
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
/// Compute R(t) routing AND calibration metrics in a single pass.
///
/// Returns the routings and the `CalibrationReport` that was used to compute
/// per-type confidence factors. Callers that also need calibration (e.g., for
/// methodology context blocks) should use this to avoid redundant O(H*K) scans.
pub fn compute_routing_with_calibration(
    store: &Store,
) -> (Vec<TaskRouting>, CalibrationReport) {
    let routings = compute_routing_from_store_inner(store);
    // The calibration was already computed inside; recompute here for the return.
    // This is still cheaper than having BOTH routing AND context compute it.
    let calibration = compute_calibration_metrics(store);
    (routings, calibration)
}

/// Compute R(t) task routings from the store.
///
/// For callers that also need calibration metrics, prefer
/// [`compute_routing_with_calibration`] to avoid redundant hypothesis scans.
pub fn compute_routing_from_store(store: &Store) -> Vec<TaskRouting> {
    compute_routing_from_store_inner(store)
}

fn compute_routing_from_store_inner(store: &Store) -> Vec<TaskRouting> {
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

    // CE-6: Gradient-based routing — project fitness delta for each ready task.
    // For each task with traces-to spec refs, simulate "what if completed" by
    // generating hypothetical :impl/implements datoms, then project the F(S) delta.
    let views = store.views();
    for r in &mut routings {
        // Find the task's traces-to spec refs
        let task_datoms = store.entity_datoms(r.entity);
        let spec_refs: Vec<EntityId> = task_datoms
            .iter()
            .filter(|d| d.attribute.as_str() == ":task/traces-to" && d.op == Op::Assert)
            .filter_map(|d| {
                if let crate::datom::Value::Ref(target) = &d.value {
                    Some(*target)
                } else {
                    None
                }
            })
            .collect();

        if !spec_refs.is_empty() {
            // Generate hypothetical impl datoms (task completion would create these)
            let hypothetical_impl = EntityId::from_ident(&format!(
                ":impl/projected-{}",
                r.label.chars().take(20).collect::<String>()
            ));
            let placeholder_tx =
                crate::datom::TxId::new(0, 0, crate::datom::AgentId::from_name("gradient"));
            let hypothetical: Vec<crate::datom::Datom> = spec_refs
                .iter()
                .map(|spec| {
                    crate::datom::Datom::new(
                        hypothetical_impl,
                        crate::datom::Attribute::from_keyword(":impl/implements"),
                        crate::datom::Value::Ref(*spec),
                        placeholder_tx,
                        Op::Assert,
                    )
                })
                .collect();

            let delta = views.project_delta(&hypothetical);
            let mag = delta.weighted_magnitude();
            r.metrics.gradient_delta = mag;

            // Blend gradient with existing impact: additive boost scaled by 2.0
            // so that tasks improving uncovered spec areas get significant lift
            if mag > f64::EPSILON {
                r.impact += mag * 2.0;
            }
        }
    }

    // UAQ-2/UAQ-5: Enrich acquisition scores with store-derived signals.
    // Factors: impact (already from R(t)), relevance (session × spec anchor),
    // novelty (1/sqrt(presentation_count)), confidence (per-type calibration error).
    let calibration = compute_calibration_metrics(store);
    // UAQ-5: Use per-type accuracy for tasks; fall back to overall mean.
    let task_confidence = calibration
        .per_type_accuracy
        .get("task")
        .map(|e| (1.0 - e).clamp(0.1, 1.0))
        .unwrap_or_else(|| {
            if calibration.completed_hypotheses >= 5 {
                (1.0 - calibration.mean_error).clamp(0.1, 1.0)
            } else {
                1.0
            }
        });

    let attention_attr = Attribute::from_keyword(":attention/presentation-count");
    for r in &mut routings {
        // Novelty from presentation count (how many times this task was recommended)
        let presentation_count = store
            .entity_datoms(r.entity)
            .iter()
            .find(|d| d.attribute == attention_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                Value::Long(n) => Some(*n as u64),
                _ => None,
            })
            .unwrap_or(0);
        let novelty = crate::budget::novelty_from_count(presentation_count);

        // Relevance = session_boost × spec_anchor (both already computed)
        let relevance = (r.metrics.session_boost * r.metrics.spec_anchor).min(1.0);

        // Recompute acquisition score with enriched factors (UAQ-5: per-type confidence)
        r.acquisition_score = AcquisitionScore::from_factors(
            ObservationKind::Task,
            r.impact,
            relevance,
            novelty,
            task_confidence,
            ObservationCost::zero(), // tasks don't have token cost
        );
    }

    // Re-sort by alpha (acquisition score), falling back to impact for ties
    routings.sort_by(|a, b| {
        b.acquisition_score
            .composite()
            .partial_cmp(&a.acquisition_score.composite())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    routings
}

// ---------------------------------------------------------------------------
// Hypothesis Ledger (HL-2, ADR-FOUNDATION-018)
// ---------------------------------------------------------------------------

/// Generate hypothesis datoms for the top-N R(t) recommendations.
///
/// Each hypothesis records: what action was recommended, the predicted ΔF(S),
/// which boundary it targets, the initial confidence (0.5 prior), and the
/// item type for per-type calibration (UAQ-4).
///
/// Returns datoms to be transacted. Does NOT transact them — caller decides.
///
/// **HL-2**: `predicted` = R(t) impact score normalized to [0, 1] as expected ΔF(S).
/// **UAQ-4**: `item_type` enables per-type calibration (task/block/boundary).
/// **HL-5** (future): confidence adjusts based on outcome history.
pub fn record_hypotheses(
    routings: &[TaskRouting],
    top_n: usize,
    tx: crate::datom::TxId,
) -> Vec<crate::datom::Datom> {
    record_hypotheses_with_type(routings, top_n, tx, "task")
}

/// Generate hypothesis datoms with explicit item type (UAQ-4).
///
/// `item_type` should be one of: "task", "block", "boundary".
pub fn record_hypotheses_with_type(
    routings: &[TaskRouting],
    top_n: usize,
    tx: crate::datom::TxId,
    item_type: &str,
) -> Vec<crate::datom::Datom> {
    use crate::datom::{Attribute, Datom, Value};

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut datoms = Vec::new();

    for routing in routings.iter().take(top_n) {
        // Skip zero-impact recommendations (noise)
        if routing.impact <= f64::EPSILON {
            continue;
        }

        // Entity for this hypothesis — content-addressed from action entity + timestamp
        let entity_hash = &format!("{:?}", routing.entity)[..8]; // first 8 chars of debug repr
        let hypothesis_id = EntityId::from_ident(&format!(
            ":hypothesis/r-{}-{}",
            entity_hash, now
        ));

        // :hypothesis/action — ref to the task entity
        datoms.push(Datom::new(
            hypothesis_id,
            Attribute::from_keyword(":hypothesis/action"),
            Value::Ref(routing.entity),
            tx,
            Op::Assert,
        ));

        // :hypothesis/predicted — R(t) impact normalized to expected ΔF(S)
        // Impact is already in [0, ~2] range; clamp to [0, 1] for ΔF(S) semantics
        let predicted = routing.impact.clamp(0.0, 1.0);
        datoms.push(Datom::new(
            hypothesis_id,
            Attribute::from_keyword(":hypothesis/predicted"),
            Value::Double(ordered_float::OrderedFloat(predicted)),
            tx,
            Op::Assert,
        ));

        // :hypothesis/boundary — infer from gradient metrics
        let boundary = if routing.metrics.gradient_delta > f64::EPSILON {
            "spec<->impl".to_string()
        } else {
            "general".to_string()
        };
        datoms.push(Datom::new(
            hypothesis_id,
            Attribute::from_keyword(":hypothesis/boundary"),
            Value::String(boundary),
            tx,
            Op::Assert,
        ));

        // :hypothesis/confidence — start at 0.5 (uninformative prior)
        datoms.push(Datom::new(
            hypothesis_id,
            Attribute::from_keyword(":hypothesis/confidence"),
            Value::Double(ordered_float::OrderedFloat(0.5)),
            tx,
            Op::Assert,
        ));

        // :hypothesis/timestamp — when the prediction was made
        datoms.push(Datom::new(
            hypothesis_id,
            Attribute::from_keyword(":hypothesis/timestamp"),
            Value::Instant(now),
            tx,
            Op::Assert,
        ));

        // :hypothesis/item-type — UAQ-4: per-type calibration
        datoms.push(Datom::new(
            hypothesis_id,
            Attribute::from_keyword(":hypothesis/item-type"),
            Value::String(item_type.to_string()),
            tx,
            Op::Assert,
        ));
    }

    datoms
}

/// Count recorded hypotheses in the store.
pub fn hypothesis_count(store: &Store) -> usize {
    let attr = crate::datom::Attribute::from_keyword(":hypothesis/action");
    store
        .attribute_datoms(&attr)
        .iter()
        .filter(|d| d.op == Op::Assert)
        .count()
}

/// Count hypotheses that have been completed (have :hypothesis/actual set).
pub fn hypothesis_completed_count(store: &Store) -> usize {
    let attr = crate::datom::Attribute::from_keyword(":hypothesis/actual");
    store
        .attribute_datoms(&attr)
        .iter()
        .filter(|d| d.op == Op::Assert)
        .count()
}

// ---------------------------------------------------------------------------
// Calibration Metrics (HL-4, ADR-FOUNDATION-018)
// ---------------------------------------------------------------------------

/// Calibration trend (improving, stable, or degrading).
#[derive(Clone, Debug, PartialEq)]
pub enum CalibrationTrend {
    /// Recent predictions are more accurate than all-time.
    Improving,
    /// Recent and all-time accuracy are similar.
    Stable,
    /// Recent predictions are less accurate than all-time.
    Degrading,
    /// Not enough data to determine trend.
    Insufficient,
}

/// Calibration report for the hypothesis ledger.
#[derive(Clone, Debug)]
pub struct CalibrationReport {
    /// Total hypotheses recorded.
    pub total_hypotheses: usize,
    /// Hypotheses with outcomes measured.
    pub completed_hypotheses: usize,
    /// Mean absolute error across completed hypotheses.
    pub mean_error: f64,
    /// Per-boundary accuracy: boundary_name → mean error.
    pub per_boundary_accuracy: std::collections::BTreeMap<String, f64>,
    /// Per-type accuracy: item_type → mean error (UAQ-4).
    /// Keys: "task", "block", "boundary". Missing key = no data for that type.
    pub per_type_accuracy: std::collections::BTreeMap<String, f64>,
    /// Trend: comparing last-20 vs all-time mean error.
    pub trend: CalibrationTrend,
}

/// Compute calibration metrics from the hypothesis ledger (HL-4).
///
/// Scans all hypothesis entities with `:hypothesis/actual` set,
/// computes error statistics, and determines the calibration trend.
pub fn compute_calibration_metrics(store: &Store) -> CalibrationReport {
    let action_attr = crate::datom::Attribute::from_keyword(":hypothesis/action");
    let actual_attr = crate::datom::Attribute::from_keyword(":hypothesis/actual");
    let error_attr = crate::datom::Attribute::from_keyword(":hypothesis/error");
    let boundary_attr = crate::datom::Attribute::from_keyword(":hypothesis/boundary");
    let completed_attr = crate::datom::Attribute::from_keyword(":hypothesis/completed");
    let item_type_attr = crate::datom::Attribute::from_keyword(":hypothesis/item-type");

    let total_hypotheses = hypothesis_count(store);

    // Collect completed hypotheses with their errors, boundaries, and item types
    struct HypRecord {
        error: f64,
        boundary: String,
        item_type: String,
        completed_at: u64,
    }

    let mut records: Vec<HypRecord> = Vec::new();

    // Find all hypothesis entities (those with :hypothesis/action)
    let hyp_entities: Vec<EntityId> = store
        .attribute_datoms(&action_attr)
        .iter()
        .filter(|d| d.op == Op::Assert)
        .map(|d| d.entity)
        .collect();

    for hyp in &hyp_entities {
        let datoms = store.entity_datoms(*hyp);

        // Check if completed (has :hypothesis/actual)
        let has_actual = datoms
            .iter()
            .any(|d| d.attribute == actual_attr && d.op == Op::Assert);
        if !has_actual {
            continue;
        }

        let error = datoms
            .iter()
            .rev()
            .find(|d| d.attribute == error_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                crate::datom::Value::Double(v) => Some(v.into_inner()),
                _ => None,
            })
            .unwrap_or(0.0);

        let boundary = datoms
            .iter()
            .find(|d| d.attribute == boundary_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                crate::datom::Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "unknown".into());

        let completed_at = datoms
            .iter()
            .rev()
            .find(|d| d.attribute == completed_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                crate::datom::Value::Instant(t) => Some(*t),
                _ => None,
            })
            .unwrap_or(0);

        // UAQ-4: item type for per-type calibration
        let item_type = datoms
            .iter()
            .find(|d| d.attribute == item_type_attr && d.op == Op::Assert)
            .and_then(|d| match &d.value {
                crate::datom::Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "task".into()); // default for pre-UAQ-4 hypotheses

        records.push(HypRecord {
            error,
            boundary,
            item_type,
            completed_at,
        });
    }

    let completed_hypotheses = records.len();

    if completed_hypotheses == 0 {
        return CalibrationReport {
            total_hypotheses,
            completed_hypotheses: 0,
            mean_error: 0.0,
            per_boundary_accuracy: std::collections::BTreeMap::new(),
            per_type_accuracy: std::collections::BTreeMap::new(),
            trend: CalibrationTrend::Insufficient,
        };
    }

    // Mean error
    let mean_error = records.iter().map(|r| r.error).sum::<f64>() / completed_hypotheses as f64;

    // Per-boundary accuracy
    let mut boundary_errors: std::collections::BTreeMap<String, Vec<f64>> =
        std::collections::BTreeMap::new();
    for r in &records {
        boundary_errors
            .entry(r.boundary.clone())
            .or_default()
            .push(r.error);
    }
    let per_boundary_accuracy: std::collections::BTreeMap<String, f64> = boundary_errors
        .iter()
        .map(|(k, v)| {
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            (k.clone(), mean)
        })
        .collect();

    // UAQ-4: Per-type accuracy
    let mut type_errors: std::collections::BTreeMap<String, Vec<f64>> =
        std::collections::BTreeMap::new();
    for r in &records {
        type_errors
            .entry(r.item_type.clone())
            .or_default()
            .push(r.error);
    }
    let per_type_accuracy: std::collections::BTreeMap<String, f64> = type_errors
        .iter()
        .map(|(k, v)| {
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            (k.clone(), mean)
        })
        .collect();

    // Trend: compare last-20 vs all-time
    let trend = if completed_hypotheses < 5 {
        CalibrationTrend::Insufficient
    } else {
        // Sort by completion time
        let mut sorted = records.iter().collect::<Vec<_>>();
        sorted.sort_by_key(|r| r.completed_at);

        let recent_n = 20.min(completed_hypotheses);
        let recent_errors: Vec<f64> = sorted
            .iter()
            .rev()
            .take(recent_n)
            .map(|r| r.error)
            .collect();
        let recent_mean = recent_errors.iter().sum::<f64>() / recent_n as f64;

        if recent_mean < mean_error * 0.8 {
            CalibrationTrend::Improving
        } else if recent_mean > mean_error * 1.2 {
            CalibrationTrend::Degrading
        } else {
            CalibrationTrend::Stable
        }
    };

    CalibrationReport {
        total_hypotheses,
        completed_hypotheses,
        mean_error,
        per_boundary_accuracy,
        per_type_accuracy,
        trend,
    }
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
    let routings = compute_routing_from_store(store);
    compute_action_from_routing(store, &routings)
}

/// Compute the projected action from pre-computed routing results.
///
/// PERF-2a: When the caller has already computed routing (e.g., via
/// [`compute_routing_with_calibration`]), pass the results here to avoid
/// redundant O(tasks × datoms) recomputation.
pub fn compute_action_from_routing(
    store: &Store,
    routings: &[TaskRouting],
) -> crate::budget::ProjectedAction {
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
    if let Some(top) = routings.first() {
        // Find the task ID from the label or entity
        let task_id = store
            .entity_datoms(top.entity)
            .iter()
            .find(|d| d.attribute.as_str() == ":task/id" && d.op == crate::datom::Op::Assert)
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
            .find(|d| d.attribute.as_str() == ":task/traces-to" && d.op == crate::datom::Op::Assert)
            .and_then(|d| match &d.value {
                crate::datom::Value::Ref(target) => {
                    // Resolve the spec element's human ID
                    store
                        .entity_datoms(*target)
                        .iter()
                        .find(|dd| {
                            dd.attribute.as_str() == ":spec/id" && dd.op == crate::datom::Op::Assert
                        })
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
                if latest_action
                    .as_ref()
                    .map(|(_, _, w)| wall > *w)
                    .unwrap_or(true)
                {
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
