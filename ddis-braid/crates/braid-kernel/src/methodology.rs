//! Methodology assessment — M(t) scoring, telemetry, gaps, and ceremony.
//!
//! Extracted from `guidance.rs` to reduce module size. Contains:
//! - **M(t)**: Methodology adherence score computation (INV-GUIDANCE-008).
//! - **Telemetry**: Session telemetry extraction from store state.
//! - **Harvest urgency**: Multi-signal harvest warning (INV-GUIDANCE-019).
//! - **Gaps**: Crystallization, unanchored tasks, witness coverage (INV-GUIDANCE-021).
//! - **Ceremony**: Adaptive ceremony level (INV-GUIDANCE-023).
//! - **Capabilities**: Subsystem capability scan (INV-GUIDANCE-022).
//! - **Decision-Task Integrity**: Orphaned decision detection (DTIC-1/2).

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::quality_adjusted_budget;
use crate::datom::{Attribute, EntityId, Op, Value};
use crate::store::Store;

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
pub(crate) const STALENESS_DECAY_RATE: f64 = 0.95;

// ---------------------------------------------------------------------------
// M(t) — Methodology Adherence Score (INV-GUIDANCE-008)
// ---------------------------------------------------------------------------

/// Stage 0 methodology adherence weights (renormalized without m₅).
///
/// m₁ = transact_frequency (0.30)
/// m₂ = spec_language_ratio (0.23)
/// m₃ = query_diversity (0.17)
/// m₄ = harvest_quality (0.30)
pub(crate) const STAGE0_WEIGHTS: [f64; 4] = [0.30, 0.23, 0.17, 0.30];

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
// Telemetry Extraction from Store
// ---------------------------------------------------------------------------

/// Extract session telemetry from store state (T1-1, T1-2).
///
/// Computes all sub-metrics for M(t) by scanning the store for session-scoped
/// transactions and spec engagement signals.
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
    let raw_total_spec = spec_entity_count
        .saturating_add(tasks_with_spec_refs)
        .saturating_add(observations_with_spec_refs)
        .saturating_add(impl_links);

    // C8-FIX-5: Don't penalize external projects that don't use :spec/* datoms.
    // If the store has no :spec/element-type datoms at all (store-wide, not just
    // session-scoped), the project doesn't use DDIS spec format. In that case:
    // - If observations or tasks exist → spec_language_ratio = 1.0 (process is active)
    // - If store is empty → spec_language_ratio = 0.0 (no methodology at all)
    let store_has_spec_datoms = store.datoms().any(|d| {
        d.op == Op::Assert
            && (d.attribute.as_str() == ":spec/element-type"
                || (d.attribute.as_str() == ":db/ident"
                    && matches!(&d.value, Value::Keyword(k) if k.starts_with(":spec/"))))
    });
    let total_spec = if store_has_spec_datoms {
        // Braid-like project: compute normally
        raw_total_spec
    } else {
        // External project: check if any methodology is happening
        let has_observations = store
            .datoms()
            .any(|d| d.attribute.as_str() == ":exploration/body" && d.op == Op::Assert);
        let has_tasks = store
            .datoms()
            .any(|d| d.attribute.as_str() == ":task/title" && d.op == Op::Assert);
        if has_observations || has_tasks {
            // Process is active, just not in DDIS spec format — full credit
            session_turn_count.max(1)
        } else {
            // Empty store — no methodology at all
            0
        }
    };

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
        // FIX-NAG: On fresh stores (<10 txns), nothing to harvest yet — don't nag.
        harvest_quality: if has_recent_harvest || session_turn_count < 10 {
            0.7
        } else {
            0.0
        },
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

/// Multi-dimensional harvest urgency (ZCM-2, META-6, INV-GUIDANCE-019).
///
/// Four signals, urgency = max of all:
/// 1. novel_tx_since / dynamic_threshold (metabolic — only transactions that moved Intent↔Spec boundary)
/// 2. minutes_since_harvest / 30 (time ceiling — ensures harvest even during slow work)
/// 3. sum(|delta-crystallization|) / 3.0 (cumulative coherence movement — replaces raw exploration count)
/// 4. k_eff_critical (Q(t) < 0.15 — context exhaustion emergency)
///
/// META-6: Signal 1 now uses metabolic delta-crystallization data. Transactions with
/// delta = 0.0 (task management, session metadata) do NOT count toward urgency.
/// Only transactions that touched the Intent↔Spec boundary contribute.
/// This eliminates alert fatigue during batch task operations.
///
/// Returns urgency in [0, 1+]. Values > 1.0 mean OVERDUE.
pub fn harvest_urgency_multi(store: &Store, k_eff: f64) -> f64 {
    // FIX-NAG: Fresh stores (<10 txns) have nothing worth harvesting yet.
    // Genesis + init detection + session auto-start generate ~5-8 system txns.
    // But always honor the k_eff emergency signal (signal_4).
    let is_fresh = count_txns_since_last_harvest(store) < 10
        && last_harvest_wall_time(store) == 0;

    let velocity = tx_velocity(store);
    let threshold = dynamic_threshold(velocity);
    let last_harvest_wall = last_harvest_wall_time(store);

    // Signal 1: novel transaction count / threshold (metabolic)
    // Count transactions since last harvest that have non-zero :tx/delta-crystallization.
    // This replaces raw tx count — routine operations (task close, status) score zero.
    let delta_attr = Attribute::from_keyword(":tx/delta-crystallization");
    let mut novel_tx_count = 0usize;
    let mut total_delta_count = 0usize; // total delta datoms (including zero)
    let mut delta_sum = 0.0f64;
    for d in store.attribute_datoms(&delta_attr) {
        if d.op == crate::datom::Op::Assert && d.tx.wall_time() > last_harvest_wall {
            if let crate::datom::Value::Double(ref v) = d.value {
                total_delta_count += 1;
                let val = v.into_inner();
                if val.abs() > f64::EPSILON {
                    novel_tx_count += 1;
                    delta_sum += val.abs();
                }
            }
        }
    }

    // Metabolic signal: if delta data exists (store has metabolic system), use it.
    // Only fall back to legacy when NO delta data exists (pre-metabolic store).
    // This eliminates alert fatigue: 30 task closes with zero delta = no warning.
    let signal_1 = if total_delta_count > 0 {
        // Metabolic system active — use novel (non-zero) delta count
        novel_tx_count as f64 / threshold.max(1) as f64
    } else {
        // Legacy fallback: no metabolic data exists (store predates metabolic system)
        let tx_since = count_txns_since_last_harvest(store);
        let exploration_type_attr = Attribute::from_keyword(":exploration/type");
        let exploration_count = store
            .attribute_datoms(&exploration_type_attr)
            .iter()
            .filter(|d2| d2.op == crate::datom::Op::Assert && d2.tx.wall_time() > last_harvest_wall)
            .count();
        if exploration_count > 0 {
            exploration_count as f64 / threshold.max(1) as f64
        } else {
            tx_since as f64 / threshold.max(1) as f64
        }
    };

    // Signal 2: time since harvest / 30 minutes (unchanged — hard backstop)
    let now_wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let minutes_since = (now_wall.saturating_sub(last_harvest_wall)) as f64 / 60.0;
    let signal_2 = minutes_since / 30.0;

    // Signal 3: cumulative coherence movement / 3.0 (metabolic — replaces raw exploration count)
    let signal_3 = delta_sum / 3.0;

    // Signal 4: k_eff critical (Q(t) < 0.15) (unchanged — emergency)
    let signal_4 = if k_eff < 0.15 { 1.5 } else { 0.0 };

    // Urgency = max of all signals.
    // FIX-NAG: On fresh stores, only the k_eff emergency (signal_4) fires.
    if is_fresh {
        signal_4
    } else {
        signal_1.max(signal_2).max(signal_3).max(signal_4)
    }
}

/// Check if a harvest warning should be shown at session exit.
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
pub(crate) fn extract_spec_ids(text: &str) -> Vec<String> {
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

/// Concentration signal: a spec neighborhood with repeated recent activity.
///
/// Fired by `spec_neighborhood_concentration` when a single namespace accumulates
/// 3+ `:recon/trace-neighborhood` datoms within the recent window, suggesting the
/// agent is circling an area without making forward progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConcentrationSignal {
    /// Spec namespace (e.g., "INTERFACE", "TOPOLOGY").
    pub neighborhood: String,
    /// Number of recent traces in this neighborhood.
    pub trace_count: usize,
    /// Suggestion for the agent.
    pub suggestion: String,
}

/// Detect spec-neighborhood concentration from recent `:recon/trace` datoms.
///
/// Scans `:recon/trace-neighborhood` datoms, groups by neighborhood, fires a
/// signal for any neighborhood with 3+ traces in the recent `window`.
pub fn spec_neighborhood_concentration(store: &Store, window: usize) -> Vec<ConcentrationSignal> {
    let attr = Attribute::from_keyword(":recon/trace-neighborhood");
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    // Scan recent trace neighborhoods
    let all_datoms: Vec<_> = store
        .attribute_datoms(&attr)
        .iter()
        .filter(|d| d.op == Op::Assert)
        .cloned()
        .collect();

    // Take only the most recent `window` traces
    let recent = if all_datoms.len() > window {
        &all_datoms[all_datoms.len() - window..]
    } else {
        &all_datoms
    };

    for d in recent {
        if let Value::String(ns) = &d.value {
            *counts.entry(ns.clone()).or_insert(0) += 1;
        }
    }

    counts
        .into_iter()
        .filter(|(_, count)| *count >= 3)
        .map(|(ns, count)| ConcentrationSignal {
            suggestion: format!("Review: braid task search INV-{ns}"),
            neighborhood: ns,
            trace_count: count,
        })
        .collect()
}

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
    /// Spec neighborhoods with concentrated recent activity (AR-4).
    pub concentration: Vec<ConcentrationSignal>,
}

impl MethodologyGaps {
    /// Total gap count across all categories.
    pub fn total(&self) -> u32 {
        self.crystallization
            + self.unanchored
            + self.untested
            + self.stale_witnesses
            + self.concentration.len() as u32
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
    let concentration = spec_neighborhood_concentration(store, 20);

    MethodologyGaps {
        crystallization,
        unanchored,
        untested: untested_w,
        stale_witnesses: stale_w,
        concentration,
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
    // Concentration signals pass through unsuppressed in all modes.
    let concentration = raw.concentration.clone();
    let adjusted = match mode {
        ActivityMode::Implementation => MethodologyGaps {
            crystallization: scale_up(raw.crystallization, 0.1),
            unanchored: scale_up(raw.unanchored, 0.2),
            untested: raw.untested,
            stale_witnesses: raw.stale_witnesses,
            concentration: concentration.clone(),
        },
        ActivityMode::Specification => MethodologyGaps {
            crystallization: raw.crystallization,
            unanchored: raw.unanchored,
            untested: scale_up(raw.untested, 0.3),
            stale_witnesses: raw.stale_witnesses,
            concentration: concentration.clone(),
        },
        ActivityMode::Mixed => MethodologyGaps {
            crystallization: raw.crystallization,
            unanchored: raw.unanchored,
            untested: raw.untested,
            stale_witnesses: raw.stale_witnesses,
            concentration,
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
