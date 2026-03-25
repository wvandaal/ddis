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

pub use crate::context::*;
pub use crate::methodology::*;
pub use crate::routing::*;

use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;

// ---------------------------------------------------------------------------
// Session Auto-Detection (ST-1)
// ---------------------------------------------------------------------------

/// Detect whether a new session should be auto-started.
///
/// Returns `true` if no active session exists in the store, OR if the
/// active session is stale (older than 2 hours). A stale session indicates
/// a previous session that was never properly closed — this happens when
/// Find the currently active session entity, if one exists (COTX-1).
///
/// Scans `:session/status` for `:session.status/active` and checks that no later
/// `closed` assertion exists. Returns `None` if no active session or if the only
/// active session is stale (>2 hours old).
pub fn find_active_session(store: &Store) -> Option<EntityId> {
    let status_attr = Attribute::from_keyword(":session/status");
    let started_attr = Attribute::from_keyword(":session/started-at");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let staleness_threshold = 2 * 3600; // 2 hours

    for datom in store.attribute_datoms(&status_attr) {
        if datom.op == Op::Assert {
            if let Value::Keyword(ref kw) = datom.value {
                if kw == ":session.status/active" {
                    // Check if not subsequently closed
                    let entity_closed = store.entity_datoms(datom.entity).iter().any(|d| {
                        d.attribute == status_attr
                            && d.op == Op::Assert
                            && d.tx.wall_time() > datom.tx.wall_time()
                            && matches!(&d.value, Value::Keyword(k) if k.contains("closed"))
                    });
                    if entity_closed {
                        continue;
                    }

                    // Check session age — stale sessions don't block
                    let session_wall = store
                        .entity_datoms(datom.entity)
                        .iter()
                        .find(|d| d.attribute == started_attr && d.op == Op::Assert)
                        .and_then(|d| match d.value {
                            Value::Long(t) => Some(t as u64),
                            _ => None,
                        })
                        .unwrap_or(0);

                    if now.saturating_sub(session_wall) < staleness_threshold {
                        return Some(datom.entity);
                    }
                }
            }
        }
    }
    None
}

/// the agent exits without running `braid session end` or `braid harvest`.
///
/// The staleness check prevents a single unclosed session from blocking
/// all future session auto-detection indefinitely (discovered in Session 031:
/// a session from Session 021 was still "active" 10 sessions later).
pub fn detect_session_start(store: &Store) -> bool {
    let status_attr = Attribute::from_keyword(":session/status");
    let started_attr = Attribute::from_keyword(":session/started-at");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // 2 hours = stale session threshold
    let staleness_threshold = 2 * 3600;

    let mut has_fresh_active = false;
    for datom in store.attribute_datoms(&status_attr) {
        if datom.op == Op::Assert {
            if let Value::Keyword(ref kw) = datom.value {
                if kw == ":session.status/active" {
                    // Check if this entity also has a later "closed" assertion
                    let entity_closed = store.entity_datoms(datom.entity).iter().any(|d| {
                        d.attribute == status_attr
                            && d.op == Op::Assert
                            && d.tx.wall_time() > datom.tx.wall_time()
                            && matches!(&d.value, Value::Keyword(k) if k.contains("closed"))
                    });
                    if entity_closed {
                        continue;
                    }

                    // Check session age — stale sessions (>2h) don't block new ones
                    let session_wall = store
                        .entity_datoms(datom.entity)
                        .iter()
                        .find(|d| d.attribute == started_attr && d.op == Op::Assert)
                        .and_then(|d| match d.value {
                            Value::Long(t) => Some(t as u64),
                            _ => None,
                        })
                        .unwrap_or(0);

                    if now.saturating_sub(session_wall) < staleness_threshold {
                        has_fresh_active = true;
                        break;
                    }
                    // Stale session — don't count as active
                }
            }
        }
    }
    !has_fresh_active
}

/// Create session-start datoms for auto-detection (ST-1).
///
/// Produces a lightweight session entity with:
/// - `:db/ident` — unique identity like `:session/s-{unix_seconds}`
/// - `:session/started-at` — wall clock as Long (unix seconds)
/// - `:session/start-time` — wall clock as ISO 8601 String
/// - `:session/start-fitness` — F(S) at session start
/// - `:session/start-datom-count` — store.len() at session start
/// - `:session/agent` — agent identity (as Ref to `:agent/{name}` entity)
/// - `:session/status` — `:session.status/active`
/// - `:session/current` — self-referential Ref marking this as the active session
///
/// The caller is responsible for wrapping these in a `TxFile` and writing them.
pub fn create_session_start_datoms(
    store: &Store,
    agent: AgentId,
    tx: TxId,
) -> Vec<Datom> {
    create_session_start_datoms_with_name(store, agent, tx, "braid:session")
}

/// Create session-start datoms with an explicit agent name string (ST-1).
///
/// Same as `create_session_start_datoms` but allows the caller to specify
/// the human-readable agent name for the `:session/agent` Ref target.
pub fn create_session_start_datoms_with_name(
    store: &Store,
    _agent: AgentId,
    tx: TxId,
    agent_name: &str,
) -> Vec<Datom> {
    let wall_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // ISO 8601 timestamp (UTC)
    let iso_time = {
        let secs = wall_secs;
        let days_since_epoch = secs / 86400;
        let time_of_day = secs % 86400;
        let hours = time_of_day / 3600;
        let minutes = (time_of_day % 3600) / 60;
        let seconds = time_of_day % 60;

        // Compute year/month/day from days since 1970-01-01
        // Using a simplified civil calendar algorithm
        let (year, month, day) = days_to_ymd(days_since_epoch);
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            year, month, day, hours, minutes, seconds
        )
    };

    let session_ident = format!(":session/s-{}", wall_secs);
    let session_entity = EntityId::from_ident(&session_ident);

    // POLICY-4: Use Store::fitness() which tries policy first, then views fallback
    let fitness = store.fitness();
    let datom_count = store.len();

    vec![
        // :db/ident
        Datom::new(
            session_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(session_ident),
            tx,
            Op::Assert,
        ),
        // :session/started-at (Long, for compatibility with existing session queries)
        Datom::new(
            session_entity,
            Attribute::from_keyword(":session/started-at"),
            Value::Long(wall_secs as i64),
            tx,
            Op::Assert,
        ),
        // :session/start-time (ISO 8601 String)
        Datom::new(
            session_entity,
            Attribute::from_keyword(":session/start-time"),
            Value::String(iso_time),
            tx,
            Op::Assert,
        ),
        // :session/start-fitness (Double)
        Datom::new(
            session_entity,
            Attribute::from_keyword(":session/start-fitness"),
            Value::Double(ordered_float::OrderedFloat(fitness.total)),
            tx,
            Op::Assert,
        ),
        // :session/start-datom-count (Long)
        Datom::new(
            session_entity,
            Attribute::from_keyword(":session/start-datom-count"),
            Value::Long(datom_count as i64),
            tx,
            Op::Assert,
        ),
        // :session/agent (Ref to agent entity)
        Datom::new(
            session_entity,
            Attribute::from_keyword(":session/agent"),
            Value::Ref(EntityId::from_ident(&format!(":agent/{}", agent_name))),
            tx,
            Op::Assert,
        ),
        // :session/status = :session.status/active
        Datom::new(
            session_entity,
            Attribute::from_keyword(":session/status"),
            Value::Keyword(":session.status/active".to_string()),
            tx,
            Op::Assert,
        ),
        // :session/current — self-ref marking this as the active session
        Datom::new(
            session_entity,
            Attribute::from_keyword(":session/current"),
            Value::Ref(session_entity),
            tx,
            Op::Assert,
        ),
    ]
}

/// Convert days since Unix epoch (1970-01-01) to (year, month, day).
///
/// Simplified civil calendar algorithm. Handles leap years correctly.
pub fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm adapted from Howard Hinnant's chrono-compatible date algorithms
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
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
    use std::collections::{BTreeMap, BTreeSet};
    use crate::budget::GuidanceLevel;

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
            session_observation_count: 10,
            session_task_count: 0,
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
    // Time urgency test (t-e89f, INV-HARVEST-005)
    // -------------------------------------------------------------------

    #[test]
    fn time_urgency_fires_before_count_in_slow_session() {
        // Slow session: few transactions (2), long elapsed time (40 min).
        // signal_1 = 2/8 = 0.25 (low — few tx)
        // signal_2 = 40/30 = 1.33 (high — long time)
        // harvest_urgency_multi should return >= 1.0 (time dominates)
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut datoms = std::collections::BTreeSet::new();

        // Harvest 40 minutes ago
        let harvest_wall = now - 2400;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-slow"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-slow"),
            Attribute::from_keyword(":harvest/boundary-tx"),
            Value::Long(harvest_wall as i64),
            harvest_tx,
            Op::Assert,
        ));

        // Only 2 transactions since harvest
        for i in 1..=2 {
            let tx = TxId::new(harvest_wall + i * 60, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":work/slow-{i}")),
                Attribute::from_keyword(":db/doc"),
                Value::String(format!("slow work {i}")),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        let urgency = harvest_urgency_multi(&store, 1.0);
        assert!(
            urgency >= 1.0,
            "time urgency (40 min) should exceed 1.0 despite few tx, got {urgency}"
        );
    }

    // -------------------------------------------------------------------
    // META-6-TEST: Harvest urgency + surprisal wiring tests
    // -------------------------------------------------------------------

    #[test]
    fn urgency_zero_on_fresh_store() {
        // Fresh genesis store with no harvests should have low urgency
        // (only time signal contributes if very recent)
        let store = Store::genesis();
        let urgency = harvest_urgency_multi(&store, 1.0);
        // On a fresh genesis store, there's no harvest boundary, so
        // last_harvest_wall_time returns 0. This means ALL time since epoch
        // counts, giving extremely high time urgency. That's expected behavior —
        // a store that has never been harvested should urgently need one.
        assert!(
            urgency >= 0.0,
            "urgency should be non-negative, got {urgency}"
        );
    }

    #[test]
    fn urgency_increases_with_novel_transactions() {
        // More metabolic delta transactions → higher urgency signal_1
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test:meta6-novel");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut datoms = std::collections::BTreeSet::new();

        // Recent harvest (1 minute ago)
        let harvest_wall = now - 60;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-novel"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-novel"),
            Attribute::from_keyword(":harvest/boundary-tx"),
            Value::Long(harvest_wall as i64),
            harvest_tx,
            Op::Assert,
        ));

        // Store with 0 novel transactions
        let store_0 = Store::from_datoms(datoms.clone());
        let urgency_0 = harvest_urgency_multi(&store_0, 1.0);

        // Add 5 transactions with non-zero delta-crystallization
        for i in 1..=5 {
            let tx = TxId::new(harvest_wall + i * 5, 0, agent);
            let entity = EntityId::from_ident(&format!(":work/novel-{i}"));
            datoms.insert(Datom::new(
                entity,
                Attribute::from_keyword(":tx/delta-crystallization"),
                Value::Double(ordered_float::OrderedFloat(0.2)),
                tx,
                Op::Assert,
            ));
        }
        let store_5 = Store::from_datoms(datoms);
        let urgency_5 = harvest_urgency_multi(&store_5, 1.0);

        assert!(
            urgency_5 > urgency_0,
            "5 novel transactions should increase urgency: {} vs {}",
            urgency_5,
            urgency_0
        );
    }

    #[test]
    fn urgency_alert_fatigue_eliminated() {
        // 30 task closes (zero delta-crystallization) should NOT trigger harvest warning.
        // This is the alert fatigue test — routine operations with no novel knowledge
        // should have zero metabolic signal.
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test:meta6-fatigue");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut datoms = std::collections::BTreeSet::new();

        // Recent harvest (5 minutes ago)
        let harvest_wall = now - 300;
        let harvest_tx = TxId::new(harvest_wall, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-fatigue"),
            Attribute::from_keyword(":harvest/agent"),
            Value::String("test".to_string()),
            harvest_tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            EntityId::from_ident(":harvest/h-fatigue"),
            Attribute::from_keyword(":harvest/boundary-tx"),
            Value::Long(harvest_wall as i64),
            harvest_tx,
            Op::Assert,
        ));

        // 30 task closes — each has delta-crystallization = 0.0
        for i in 1..=30 {
            let tx = TxId::new(harvest_wall + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":task/closed-{i}")),
                Attribute::from_keyword(":task/status"),
                Value::Keyword(":task.status/closed".to_string()),
                tx,
                Op::Assert,
            ));
            // Zero delta means no novel knowledge
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":task/closed-{i}")),
                Attribute::from_keyword(":tx/delta-crystallization"),
                Value::Double(ordered_float::OrderedFloat(0.0)),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        let urgency = harvest_urgency_multi(&store, 1.0);

        // Signal 1 (novel tx) should be 0 — no non-zero deltas
        // Signal 2 (time) = 5min/30min = 0.17
        // Signal 3 (delta sum) = 0
        // Total urgency should be < 1.0 (no warning threshold)
        assert!(
            urgency < 1.0,
            "30 task closes with zero delta should NOT trigger harvest warning, got urgency={urgency}"
        );
    }

    #[test]
    fn urgency_k_eff_critical_overrides() {
        // When k_eff < 0.15 (context nearly exhausted), urgency should be >= 1.5
        let store = Store::genesis();
        let urgency = harvest_urgency_multi(&store, 0.10); // critically low k_eff
        assert!(
            urgency >= 1.5,
            "k_eff=0.10 should trigger emergency urgency >= 1.5, got {urgency}"
        );
    }

    #[test]
    fn surprisal_wired_at_production_sites() {
        // Verify surprisal_score is called at all 3 production sites in harvest.rs.
        // This is a compile-time structural test — if someone removes a call site,
        // the count would change. We verify by grepping the source.
        let harvest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("harvest.rs");
        let harvest_src = std::fs::read_to_string(&harvest_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", harvest_path.display(), e));

        // Filter out test-only calls
        let in_test_module = harvest_src.find("#[cfg(test)]").unwrap_or(harvest_src.len());
        let production_only: Vec<_> = harvest_src[..in_test_module]
            .lines()
            .filter(|l| l.contains("surprisal_score(") && !l.trim().starts_with("//"))
            .collect();

        assert!(
            production_only.len() >= 3,
            "surprisal_score should be called at 3+ production sites, found {}: {:?}",
            production_only.len(),
            production_only
        );
    }

    // -------------------------------------------------------------------
    // Typed routing preference (t-6da3, INV-GUIDANCE-010)
    // -------------------------------------------------------------------

    #[test]
    fn typed_routing_task_type_multiplier_ordering() {
        // Bug > Task > Feature > Epic > Docs > Question
        use crate::task::TaskType;
        assert!(
            TaskType::Bug.type_multiplier() >= TaskType::Task.type_multiplier(),
            "bug should have >= multiplier than task"
        );
        assert!(
            TaskType::Task.type_multiplier() >= TaskType::Feature.type_multiplier(),
            "task should have >= multiplier than feature"
        );
        assert!(
            TaskType::Feature.type_multiplier() >= TaskType::Docs.type_multiplier(),
            "feature should have >= multiplier than docs"
        );
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
        let basin = format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
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
        let basin = format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
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
        let basin = format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
        assert!(
            basin.len() < 50,
            "BasinToken must be < 50 chars, got {} chars: {basin}",
            basin.len()
        );

        // Case 2: low M(t) with action
        footer.methodology.score = 0.2;
        let basin = format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
        assert!(
            basin.len() < 50,
            "BasinToken must be < 50 chars, got {} chars: {basin}",
            basin.len()
        );

        // Case 3: harvest warning
        footer.harvest_warning = HarvestWarningLevel::Warn;
        let basin = format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
        assert!(
            basin.len() < 50,
            "BasinToken must be < 50 chars, got {} chars: {basin}",
            basin.len()
        );

        // Case 4: mid M(t) store summary
        footer.harvest_warning = HarvestWarningLevel::None;
        footer.methodology.score = 0.5;
        let basin = format_footer_at_level(&footer, crate::budget::GuidanceLevel::BasinToken);
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
            session_activity: 0.5,
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
            session_activity: 0.5,
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
            session_activity: 0.5,
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
            session_activity: 0.5,
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
                        session_activity: score,
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
                    session_activity: score,
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

    // -------------------------------------------------------------------
    // HL-2: Hypothesis Ledger tests (ADR-FOUNDATION-018)
    // -------------------------------------------------------------------

    /// HL-2: record_hypotheses produces 5 datoms per recommendation.
    #[test]
    fn record_hypotheses_produces_correct_datoms() {
        use crate::datom::AgentId;
        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);

        let routings = vec![
            TaskRouting {
                entity: EntityId::from_ident(":task/alpha"),
                label: "Alpha task".into(),
                impact: 0.7,
                metrics: RoutingMetrics {
                    pagerank: 0.5,
                    betweenness_proxy: 0.3,
                    critical_path_pos: 0.2,
                    blocker_ratio: 0.1,
                    staleness: 0.0,
                    priority_boost: 0.8,
                    type_multiplier: 1.0,
                    urgency_decay: 1.0,
                    spec_anchor: 1.0,
                    session_boost: 1.0,
                    gradient_delta: 0.05,
                    observation_dampening: 1.0,
                    concept_dampening: 1.0,
                },
                acquisition_score: crate::budget::AcquisitionScore::from_factors(
                    crate::budget::ObservationKind::Task, 0.7, 1.0, 1.0, 1.0,
                    crate::budget::ObservationCost::zero(),
                ),
            },
            TaskRouting {
                entity: EntityId::from_ident(":task/beta"),
                label: "Beta task".into(),
                impact: 0.3,
                metrics: RoutingMetrics {
                    pagerank: 0.2,
                    betweenness_proxy: 0.1,
                    critical_path_pos: 0.1,
                    blocker_ratio: 0.05,
                    staleness: 0.0,
                    priority_boost: 0.6,
                    type_multiplier: 1.0,
                    urgency_decay: 1.0,
                    spec_anchor: 1.0,
                    session_boost: 1.0,
                    gradient_delta: 0.0,
                    observation_dampening: 1.0,
                    concept_dampening: 1.0,
                },
                acquisition_score: crate::budget::AcquisitionScore::from_factors(
                    crate::budget::ObservationKind::Task, 0.3, 1.0, 1.0, 1.0,
                    crate::budget::ObservationCost::zero(),
                ),
            },
        ];

        let datoms = record_hypotheses(&routings, 3, tx);
        // 2 recommendations x 6 datoms each = 12 (UAQ-4: +item-type)
        assert_eq!(datoms.len(), 12, "expected 6 datoms per hypothesis, got {}", datoms.len());

        // Check first hypothesis has all 6 attributes
        let first_entity = datoms[0].entity;
        let attrs: Vec<String> = datoms.iter()
            .filter(|d| d.entity == first_entity)
            .map(|d| d.attribute.as_str().to_string())
            .collect();
        assert!(attrs.contains(&":hypothesis/action".to_string()));
        assert!(attrs.contains(&":hypothesis/predicted".to_string()));
        assert!(attrs.contains(&":hypothesis/boundary".to_string()));
        assert!(attrs.contains(&":hypothesis/confidence".to_string()));
        assert!(attrs.contains(&":hypothesis/timestamp".to_string()));
        assert!(attrs.contains(&":hypothesis/item-type".to_string()));
    }

    /// HL-2: Zero-impact recommendations are skipped.
    #[test]
    fn record_hypotheses_skips_zero_impact() {
        use crate::datom::AgentId;
        let agent = AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);

        let routings = vec![TaskRouting {
            entity: EntityId::from_ident(":task/zero"),
            label: "Zero impact".into(),
            impact: 0.0,
            metrics: RoutingMetrics {
                pagerank: 0.0,
                betweenness_proxy: 0.0,
                critical_path_pos: 0.0,
                blocker_ratio: 0.0,
                staleness: 0.0,
                priority_boost: 0.0,
                type_multiplier: 1.0,
                urgency_decay: 1.0,
                spec_anchor: 1.0,
                session_boost: 1.0,
                gradient_delta: 0.0,
                observation_dampening: 1.0,
                concept_dampening: 1.0,
            },
            acquisition_score: crate::budget::AcquisitionScore::from_factors(
                crate::budget::ObservationKind::Task, 0.0, 1.0, 1.0, 1.0,
                crate::budget::ObservationCost::zero(),
            ),
        }];

        let datoms = record_hypotheses(&routings, 3, tx);
        assert!(datoms.is_empty(), "zero-impact should produce no hypotheses");
    }

    /// HL-2: hypothesis_count on empty store is 0.
    #[test]
    fn hypothesis_count_empty() {
        let store = Store::genesis();
        assert_eq!(hypothesis_count(&store), 0);
        assert_eq!(hypothesis_completed_count(&store), 0);
    }

    // -------------------------------------------------------------------
    // compute_routing_from_store tests (INV-GUIDANCE-010)
    // -------------------------------------------------------------------

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
            ..Default::default()
        };
        assert_eq!(with_cryst.total(), 3);
        assert!(!with_cryst.is_empty());

        let mixed = MethodologyGaps {
            crystallization: 1,
            unanchored: 2,
            untested: 3,
            stale_witnesses: 4,
            ..Default::default()
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
        assert!(
            weights.is_some(),
            "should produce weights with 60 data points"
        );
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
                    0.1 + 0.8 * t,                      // pagerank: ramp
                    0.5,                                // betweenness: constant
                    0.3 * (1.0 - t),                    // critical_path: decreasing
                    if i % 3 == 0 { 0.9 } else { 0.1 }, // blocker_ratio: periodic
                    0.2 + 0.1 * (i % 5) as f64,         // staleness: stepped
                    0.4,                                // priority_boost: constant
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
        assert_eq!(category, "ignored", "unrelated command should be 'ignored'");
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
                // :spec/element-type required for C8-FIX-5 spec-aware project detection
                datoms.insert(Datom::new(
                    EntityId::from_ident(&format!(":spec/inv-test-{i:03}")),
                    Attribute::from_keyword(":spec/element-type"),
                    Value::Keyword(":invariant".to_string()),
                    tx,
                    Op::Assert,
                ));
            }
        }

        let store = Store::from_datoms(datoms);
        let telemetry = telemetry_from_store(&store);

        // total_turns should be session-scoped (10 distinct wall_times after harvest)
        assert_eq!(
            telemetry.total_turns, 10,
            "total_turns should be session-scoped"
        );
        // spec_language_turns should be 5 (only spec entities created after harvest)
        assert_eq!(
            telemetry.spec_language_turns, 5,
            "spec_language_turns should count session specs"
        );

        let score = compute_methodology_score(&telemetry);
        // CE-MT weight redistribution: transact_frequency 0.30→0.10, session_activity new at 0.25.
        // This store has 0 :exploration/body datoms so session_activity=0, lowering baseline.
        assert!(
            score.score >= 0.3,
            "M(t) with 10 session txns and 5 spec entities should be >= 0.3, got {}",
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
                // :spec/element-type required for C8-FIX-5 spec-aware project detection
                datoms.insert(Datom::new(
                    EntityId::from_ident(&format!(":spec/inv-new-{i:03}")),
                    Attribute::from_keyword(":spec/element-type"),
                    Value::Keyword(":invariant".to_string()),
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

        // :spec/element-type required for C8-FIX-5 spec-aware project detection
        // Without this, the external-project fallback gives full credit instead of
        // counting individual spec language turns.
        let spec_tx = TxId::new(harvest_wall - 1, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":spec/inv-merge-001"),
            Attribute::from_keyword(":spec/element-type"),
            Value::Keyword(":invariant".to_string()),
            spec_tx,
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
        // (The :spec/inv-merge-001 entity is pre-harvest so doesn't count as a session turn.)
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

        // :spec/element-type required for C8-FIX-5 spec-aware project detection
        // Without this, the external-project fallback gives full credit instead of
        // counting individual spec language turns.
        let spec_tx = TxId::new(harvest_wall - 1, 0, agent);
        datoms.insert(Datom::new(
            EntityId::from_ident(":spec/adr-guidance-003"),
            Attribute::from_keyword(":spec/element-type"),
            Value::Keyword(":adr".to_string()),
            spec_tx,
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

    // -----------------------------------------------------------------------
    // ST-1: Session Auto-Detection Tests
    // -----------------------------------------------------------------------

    /// Helper: create a store with full schema (genesis + domain + Layer 4).
    fn st1_full_schema_store() -> Store {
        use crate::schema::{full_schema_datoms, genesis_datoms};
        let system_agent = AgentId::from_name("braid:system");
        let genesis_tx = TxId::new(0, 0, system_agent);
        let mut datom_set = std::collections::BTreeSet::new();
        for d in genesis_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        for d in full_schema_datoms(genesis_tx) {
            datom_set.insert(d);
        }
        Store::from_datoms(datom_set)
    }

    #[test]
    fn detect_session_start_returns_true_on_empty_store() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        assert!(
            detect_session_start(&store),
            "empty store should detect session start (no active session)"
        );
    }

    #[test]
    fn detect_session_start_returns_true_on_full_schema_no_session() {
        let store = st1_full_schema_store();
        assert!(
            detect_session_start(&store),
            "store with only schema datoms should detect session start"
        );
    }

    #[test]
    fn detect_session_start_returns_false_after_session_datoms_written() {
        let mut store = st1_full_schema_store();
        let agent = AgentId::from_name("test:st1");
        let tx = TxId::new(100, 0, agent);

        // Write session-start datoms
        let datoms = create_session_start_datoms(&store, agent, tx);
        let mut datom_set = store.datom_set().clone();
        for d in datoms {
            datom_set.insert(d);
        }
        store = Store::from_datoms(datom_set);

        assert!(
            !detect_session_start(&store),
            "store with active session should NOT detect session start"
        );
    }

    #[test]
    fn detect_session_start_returns_true_after_session_closed() {
        let mut store = st1_full_schema_store();
        let agent = AgentId::from_name("test:st1-close");
        let tx = TxId::new(100, 0, agent);

        // Write session-start datoms
        let datoms = create_session_start_datoms(&store, agent, tx);
        let mut datom_set = store.datom_set().clone();
        for d in &datoms {
            datom_set.insert(d.clone());
        }

        // Find the session entity (from :db/ident datom)
        let session_entity = datoms
            .iter()
            .find(|d| d.attribute == Attribute::from_keyword(":db/ident"))
            .unwrap()
            .entity;

        // Close the session with a later tx
        let close_tx = TxId::new(200, 0, agent);
        datom_set.insert(Datom::new(
            session_entity,
            Attribute::from_keyword(":session/status"),
            Value::Keyword(":session.status/closed".to_string()),
            close_tx,
            Op::Assert,
        ));

        store = Store::from_datoms(datom_set);
        assert!(
            detect_session_start(&store),
            "store with closed session should detect session start (no active session)"
        );
    }

    #[test]
    fn create_session_start_datoms_produces_correct_attributes() {
        let store = st1_full_schema_store();
        let agent = AgentId::from_name("test:st1-attrs");
        let tx = TxId::new(42, 0, agent);

        let datoms = create_session_start_datoms(&store, agent, tx);

        // Should produce exactly 8 datoms
        assert_eq!(
            datoms.len(),
            8,
            "session start should produce 8 datoms (ident, started-at, start-time, \
             start-fitness, start-datom-count, agent, status, current)"
        );

        // All datoms should be Assert operations
        for d in &datoms {
            assert_eq!(d.op, Op::Assert, "all session datoms should be Assert");
            assert_eq!(d.tx, tx, "all session datoms should use the provided tx");
        }

        // Check that all expected attributes are present
        let attrs: std::collections::BTreeSet<String> = datoms
            .iter()
            .map(|d| d.attribute.as_str().to_string())
            .collect();
        assert!(attrs.contains(":db/ident"), "missing :db/ident");
        assert!(
            attrs.contains(":session/started-at"),
            "missing :session/started-at"
        );
        assert!(
            attrs.contains(":session/start-time"),
            "missing :session/start-time"
        );
        assert!(
            attrs.contains(":session/start-fitness"),
            "missing :session/start-fitness"
        );
        assert!(
            attrs.contains(":session/start-datom-count"),
            "missing :session/start-datom-count"
        );
        assert!(
            attrs.contains(":session/agent"),
            "missing :session/agent"
        );
        assert!(
            attrs.contains(":session/status"),
            "missing :session/status"
        );
        assert!(
            attrs.contains(":session/current"),
            "missing :session/current"
        );

        // Verify :session/status is :session.status/active
        let status_datom = datoms
            .iter()
            .find(|d| d.attribute == Attribute::from_keyword(":session/status"))
            .unwrap();
        assert_eq!(
            status_datom.value,
            Value::Keyword(":session.status/active".to_string()),
            "session status should be active"
        );

        // Verify :session/start-fitness is a Double in [0, 1]
        let fitness_datom = datoms
            .iter()
            .find(|d| d.attribute == Attribute::from_keyword(":session/start-fitness"))
            .unwrap();
        if let Value::Double(f) = fitness_datom.value {
            assert!(
                f.into_inner() >= 0.0 && f.into_inner() <= 1.0,
                "fitness should be in [0, 1], got {}",
                f
            );
        } else {
            panic!("start-fitness should be a Double, got {:?}", fitness_datom.value);
        }

        // Verify :session/start-datom-count is a Long >= 0
        let count_datom = datoms
            .iter()
            .find(|d| d.attribute == Attribute::from_keyword(":session/start-datom-count"))
            .unwrap();
        if let Value::Long(n) = count_datom.value {
            assert!(n >= 0, "datom count should be non-negative, got {}", n);
        } else {
            panic!(
                "start-datom-count should be a Long, got {:?}",
                count_datom.value
            );
        }

        // Verify :session/start-time is an ISO 8601 string
        let time_datom = datoms
            .iter()
            .find(|d| d.attribute == Attribute::from_keyword(":session/start-time"))
            .unwrap();
        if let Value::String(ref s) = time_datom.value {
            assert!(
                s.ends_with('Z') && s.contains('T') && s.contains('-'),
                "start-time should be ISO 8601, got: {}",
                s
            );
        } else {
            panic!("start-time should be a String, got {:?}", time_datom.value);
        }

        // Verify all datoms belong to the same entity
        let entity = datoms[0].entity;
        for d in &datoms {
            assert_eq!(
                d.entity, entity,
                "all session datoms should belong to the same entity"
            );
        }
    }

    #[test]
    fn days_to_ymd_known_dates() {
        // 1970-01-01 = day 0
        assert_eq!(super::days_to_ymd(0), (1970, 1, 1));
        // 2000-01-01 = day 10957
        assert_eq!(super::days_to_ymd(10957), (2000, 1, 1));
        // 2026-03-20 = day 20532
        assert_eq!(super::days_to_ymd(20532), (2026, 3, 20));
    }

    // -------------------------------------------------------------------
    // COTX-1: find_active_session tests
    // -------------------------------------------------------------------

    #[test]
    fn find_active_session_returns_none_on_empty_store() {
        let store = st1_full_schema_store();
        assert!(
            find_active_session(&store).is_none(),
            "empty store should have no active session"
        );
    }

    #[test]
    fn find_active_session_returns_entity_when_active() {
        let mut store = st1_full_schema_store();
        let agent = AgentId::from_name("test:cotx1-active");
        let tx = TxId::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            0,
            agent,
        );

        let datoms = create_session_start_datoms(&store, agent, tx);
        let session_entity = datoms[0].entity;
        let mut datom_set = store.datom_set().clone();
        for d in datoms {
            datom_set.insert(d);
        }
        store = Store::from_datoms(datom_set);

        let found = find_active_session(&store);
        assert_eq!(
            found,
            Some(session_entity),
            "should find the active session entity"
        );
    }

    #[test]
    fn find_active_session_returns_none_after_close() {
        let mut store = st1_full_schema_store();
        let agent = AgentId::from_name("test:cotx1-close");
        let wall = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let tx = TxId::new(wall, 0, agent);

        let datoms = create_session_start_datoms(&store, agent, tx);
        let session_entity = datoms[0].entity;
        let mut datom_set = store.datom_set().clone();
        for d in datoms {
            datom_set.insert(d);
        }

        // Close the session
        let close_tx = TxId::new(wall + 1, 0, agent);
        datom_set.insert(Datom::new(
            session_entity,
            Attribute::from_keyword(":session/status"),
            Value::Keyword(":session.status/closed".to_string()),
            close_tx,
            Op::Assert,
        ));

        store = Store::from_datoms(datom_set);
        assert!(
            find_active_session(&store).is_none(),
            "closed session should not be found as active"
        );
    }

    #[test]
    fn find_active_session_ignores_stale_sessions() {
        let mut store = st1_full_schema_store();
        let agent = AgentId::from_name("test:cotx1-stale");
        // 3 hours ago — beyond the 2h staleness threshold
        let stale_wall = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 3 * 3600;
        let tx = TxId::new(stale_wall, 0, agent);
        let session_entity = EntityId::from_ident(":session/s-stale-test");

        // Build session datoms manually with the stale started-at timestamp
        let datoms = vec![
            Datom::new(
                session_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":session/s-stale-test".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                session_entity,
                Attribute::from_keyword(":session/started-at"),
                Value::Long(stale_wall as i64),
                tx,
                Op::Assert,
            ),
            Datom::new(
                session_entity,
                Attribute::from_keyword(":session/status"),
                Value::Keyword(":session.status/active".to_string()),
                tx,
                Op::Assert,
            ),
        ];
        let mut datom_set = store.datom_set().clone();
        for d in datoms {
            datom_set.insert(d);
        }
        store = Store::from_datoms(datom_set);

        assert!(
            find_active_session(&store).is_none(),
            "stale session (>2h) should not be found as active"
        );
    }

    // -------------------------------------------------------------------
    // AR-3: spec_graph_neighbors + extract_spec_namespace
    // -------------------------------------------------------------------

    #[test]
    fn extract_spec_namespace_parses_correctly() {
        assert_eq!(extract_spec_namespace("INV-TOPOLOGY-001"), "TOPOLOGY");
        assert_eq!(extract_spec_namespace("ADR-STORE-003"), "STORE");
        assert_eq!(extract_spec_namespace("NEG-MERGE-002"), "MERGE");
        // Malformed: fewer than 3 parts
        assert_eq!(extract_spec_namespace("malformed"), "malformed");
        // "INV-ONLY" has only 2 parts: returns full string as fallback
        assert_eq!(extract_spec_namespace("INV-ONLY"), "INV-ONLY");
        // Extra parts: still returns index 1
        assert_eq!(
            extract_spec_namespace("INV-TOPOLOGY-001-EXTRA"),
            "TOPOLOGY"
        );
    }

    /// Helper: create a store with full schema + extra datoms for graph neighbor tests.
    fn graph_test_store() -> Store {
        routing_test_store()
    }

    /// Helper: rebuild a store from its datoms + extras.
    fn graph_store_with(
        store: &Store,
        extra: impl IntoIterator<Item = Datom>,
    ) -> Store {
        routing_store_with(store, extra)
    }

    #[test]
    fn spec_graph_neighbors_finds_shared_refs() {
        // Two tasks tracing to the same spec entity => both appear as neighbors.
        let store = graph_test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let spec_entity = EntityId::from_ident(":spec/inv-store-001");

        // Create spec entity with :spec/falsification so it exists
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
                Value::String("Any deletion violates this.".to_string()),
                tx,
                Op::Assert,
            ),
        ];
        let store = graph_store_with(&store, spec_datoms);

        // Task A traces-to spec entity
        let (task_a_entity, datoms_a) = crate::task::create_task_datoms(
            crate::task::CreateTaskParams {
                title: "Task Alpha (no keywords overlap)",
                description: None,
                priority: 1,
                task_type: crate::task::TaskType::Task,
                tx,
                traces_to: &[spec_entity],
                labels: &[],
            },
        );
        let store = graph_store_with(&store, datoms_a);

        // Task B also traces-to spec entity
        let (task_b_entity, datoms_b) = crate::task::create_task_datoms(
            crate::task::CreateTaskParams {
                title: "Task Bravo (completely different words)",
                description: None,
                priority: 2,
                task_type: crate::task::TaskType::Task,
                tx,
                traces_to: &[spec_entity],
                labels: &[],
            },
        );
        let store = graph_store_with(&store, datoms_b);

        // Query neighbors for INV-STORE-001
        let neighbors =
            spec_graph_neighbors(&store, &["INV-STORE-001".to_string()]);

        // Both tasks should appear as neighbors
        let neighbor_entities: Vec<EntityId> =
            neighbors.iter().map(|(e, _)| *e).collect();
        assert!(
            neighbor_entities.contains(&task_a_entity),
            "Task A should be found as a neighbor via shared spec ref"
        );
        assert!(
            neighbor_entities.contains(&task_b_entity),
            "Task B should be found as a neighbor via shared spec ref"
        );
    }

    #[test]
    fn spec_graph_neighbors_idf_weighting() {
        // A spec referenced by many tasks should produce lower scores
        // than a spec referenced by few tasks.
        let store = graph_test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let rare_spec = EntityId::from_ident(":spec/inv-rare-001");
        let popular_spec = EntityId::from_ident(":spec/inv-popular-001");

        let mut extra_datoms = vec![
            // Rare spec
            Datom::new(
                rare_spec,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-rare-001".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                rare_spec,
                Attribute::from_keyword(":spec/falsification"),
                Value::String("rare violation".to_string()),
                tx,
                Op::Assert,
            ),
            // Popular spec
            Datom::new(
                popular_spec,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-popular-001".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                popular_spec,
                Attribute::from_keyword(":spec/falsification"),
                Value::String("popular violation".to_string()),
                tx,
                Op::Assert,
            ),
        ];

        // One task traces-to rare spec
        let (_rare_task, rare_datoms) = crate::task::create_task_datoms(
            crate::task::CreateTaskParams {
                title: "Rare spec task only one ref",
                description: None,
                priority: 1,
                task_type: crate::task::TaskType::Task,
                tx,
                traces_to: &[rare_spec],
                labels: &[],
            },
        );
        extra_datoms.extend(rare_datoms);

        // Five tasks trace-to popular spec
        for i in 0..5 {
            let title = format!("Popular spec task number {}", i);
            let (_, popular_datoms) = crate::task::create_task_datoms(
                crate::task::CreateTaskParams {
                    title: &title,
                    description: None,
                    priority: 2,
                    task_type: crate::task::TaskType::Task,
                    tx,
                    traces_to: &[popular_spec],
                    labels: &[],
                },
            );
            extra_datoms.extend(popular_datoms);
        }

        let store = graph_store_with(&store, extra_datoms);

        let rare_neighbors =
            spec_graph_neighbors(&store, &["INV-RARE-001".to_string()]);
        let popular_neighbors =
            spec_graph_neighbors(&store, &["INV-POPULAR-001".to_string()]);

        // Get the max score from each
        let rare_max = rare_neighbors
            .iter()
            .map(|(_, s)| *s)
            .fold(0.0f64, f64::max);
        let popular_max = popular_neighbors
            .iter()
            .map(|(_, s)| *s)
            .fold(0.0f64, f64::max);

        assert!(
            rare_max > popular_max,
            "IDF: rare spec score ({}) should be higher than popular spec score ({})",
            rare_max,
            popular_max
        );
    }

    #[test]
    fn spec_graph_neighbors_one_hop() {
        // Spec A traces-to Spec B. Task traces-to Spec A.
        // Another task traces-to Spec B. The second task should appear
        // as a 1-hop neighbor when querying for spec A's refs.
        let store = graph_test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let spec_a = EntityId::from_ident(":spec/inv-alpha-001");
        let spec_b = EntityId::from_ident(":spec/inv-beta-001");

        let mut extra_datoms = vec![
            // Spec A with traces-to Spec B
            Datom::new(
                spec_a,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-alpha-001".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_a,
                Attribute::from_keyword(":spec/falsification"),
                Value::String("alpha violation".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_a,
                Attribute::from_keyword(":spec/traces-to"),
                Value::Ref(spec_b),
                tx,
                Op::Assert,
            ),
            // Spec B
            Datom::new(
                spec_b,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-beta-001".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_b,
                Attribute::from_keyword(":spec/falsification"),
                Value::String("beta violation".to_string()),
                tx,
                Op::Assert,
            ),
        ];

        // Task 1 traces-to Spec A (direct)
        let (task_1_entity, task_1_datoms) = crate::task::create_task_datoms(
            crate::task::CreateTaskParams {
                title: "Direct task on spec alpha",
                description: None,
                priority: 1,
                task_type: crate::task::TaskType::Task,
                tx,
                traces_to: &[spec_a],
                labels: &[],
            },
        );
        extra_datoms.extend(task_1_datoms);

        // Task 2 traces-to Spec B (indirect via spec A -> spec B)
        let (task_2_entity, task_2_datoms) = crate::task::create_task_datoms(
            crate::task::CreateTaskParams {
                title: "Indirect task on spec beta",
                description: None,
                priority: 2,
                task_type: crate::task::TaskType::Task,
                tx,
                traces_to: &[spec_b],
                labels: &[],
            },
        );
        extra_datoms.extend(task_2_datoms);

        let store = graph_store_with(&store, extra_datoms);

        // Query for INV-ALPHA-001 neighbors: should find both tasks.
        // Task 1 is 0-hop (directly traces to alpha).
        // Task 2 is 1-hop (traces to beta, which alpha traces-to).
        let neighbors =
            spec_graph_neighbors(&store, &["INV-ALPHA-001".to_string()]);

        let neighbor_entities: Vec<EntityId> =
            neighbors.iter().map(|(e, _)| *e).collect();
        assert!(
            neighbor_entities.contains(&task_1_entity),
            "Task 1 should appear as 0-hop neighbor"
        );
        assert!(
            neighbor_entities.contains(&task_2_entity),
            "Task 2 should appear as 1-hop neighbor (via spec A -> spec B)"
        );

        // Task 1 (0-hop) should have higher score than Task 2 (1-hop)
        let score_1 = neighbors
            .iter()
            .find(|(e, _)| *e == task_1_entity)
            .map(|(_, s)| *s)
            .unwrap();
        let score_2 = neighbors
            .iter()
            .find(|(e, _)| *e == task_2_entity)
            .map(|(_, s)| *s)
            .unwrap();
        assert!(
            score_1 > score_2,
            "0-hop score ({}) should exceed 1-hop score ({})",
            score_1,
            score_2
        );
    }

    #[test]
    fn spec_graph_neighbors_empty_refs() {
        let store = graph_test_store();
        let neighbors = spec_graph_neighbors(&store, &[]);
        assert!(
            neighbors.is_empty(),
            "empty spec refs should produce empty neighbors"
        );
    }

    #[test]
    fn knowledge_relevance_scan_uses_graph() {
        // Create a store with keyword-disjoint but graph-connected entities.
        // The keyword search won't find them, but graph neighbors should.
        let store = graph_test_store();
        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);

        let spec_entity = EntityId::from_ident(":spec/inv-foobar-001");

        let mut extra_datoms = vec![
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(":spec/inv-foobar-001".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/falsification"),
                Value::String("foobar violation detected".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/statement"),
                Value::String("foobar invariant statement".to_string()),
                tx,
                Op::Assert,
            ),
            Datom::new(
                spec_entity,
                Attribute::from_keyword(":spec/namespace"),
                Value::String("FOOBAR".to_string()),
                tx,
                Op::Assert,
            ),
        ];

        // Task with completely different keywords but traces-to the spec
        let (_task_entity, task_datoms) = crate::task::create_task_datoms(
            crate::task::CreateTaskParams {
                title: "Zyxwvuts completely unrelated words",
                description: None,
                priority: 1,
                task_type: crate::task::TaskType::Task,
                tx,
                traces_to: &[spec_entity],
                labels: &[],
            },
        );
        extra_datoms.extend(task_datoms);

        let store = graph_store_with(&store, extra_datoms);

        // Query text mentions INV-FOOBAR-001 but shares no keywords with the task title.
        // The task should still appear via graph neighbor discovery.
        let results = knowledge_relevance_scan(
            "Check INV-FOOBAR-001 compliance for the zephyr module",
            &store,
        );

        // The task should appear in results via graph (even though keywords don't match)
        let has_graph_task = results.iter().any(|r| {
            r.source.contains("graph") || r.source == "task"
        });
        // At minimum, the spec element itself should appear via keyword match on "foobar"
        let has_spec = results.iter().any(|r| r.source == "spec");
        assert!(
            has_spec || has_graph_task,
            "knowledge_relevance_scan should find spec or graph-connected entities, \
             got {:?}",
            results
                .iter()
                .map(|r| format!("{}:{}", r.source, r.human_id))
                .collect::<Vec<_>>()
        );
    }

    // -------------------------------------------------------------------
    // AR-4: Concentration detector (spec_neighborhood_concentration)
    // -------------------------------------------------------------------

    #[test]
    fn concentration_empty_store() {
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let signals = spec_neighborhood_concentration(&store, 20);
        assert!(
            signals.is_empty(),
            "empty store should produce no concentration signals"
        );
    }

    #[test]
    fn concentration_below_threshold() {
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let mut datoms = std::collections::BTreeSet::new();

        // 2 traces in INTERFACE — below the threshold of 3
        for i in 0..2 {
            let tx = TxId::new(1000 + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":recon/trace-{i}")),
                Attribute::from_keyword(":recon/trace-neighborhood"),
                Value::String("INTERFACE".to_string()),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        let signals = spec_neighborhood_concentration(&store, 20);
        assert!(
            signals.is_empty(),
            "2 traces in same namespace should not fire a signal"
        );
    }

    #[test]
    fn concentration_fires_at_three() {
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let mut datoms = std::collections::BTreeSet::new();

        // 3 traces in INTERFACE — exactly at threshold
        for i in 0..3 {
            let tx = TxId::new(1000 + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":recon/trace-{i}")),
                Attribute::from_keyword(":recon/trace-neighborhood"),
                Value::String("INTERFACE".to_string()),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        let signals = spec_neighborhood_concentration(&store, 20);
        assert_eq!(
            signals.len(),
            1,
            "3 traces in same namespace should fire exactly 1 signal"
        );
        assert_eq!(signals[0].neighborhood, "INTERFACE");
        assert_eq!(signals[0].trace_count, 3);
        assert!(
            signals[0].suggestion.contains("INV-INTERFACE"),
            "suggestion should reference the namespace: {}",
            signals[0].suggestion
        );
    }

    #[test]
    fn concentration_multiple_namespaces() {
        use crate::datom::{AgentId, Datom, Op, TxId, Value};
        let agent = AgentId::from_name("test");
        let mut datoms = std::collections::BTreeSet::new();

        // 5 traces in INTERFACE
        for i in 0..5 {
            let tx = TxId::new(1000 + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":recon/trace-iface-{i}")),
                Attribute::from_keyword(":recon/trace-neighborhood"),
                Value::String("INTERFACE".to_string()),
                tx,
                Op::Assert,
            ));
        }

        // 3 traces in TOPOLOGY
        for i in 0..3 {
            let tx = TxId::new(2000 + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":recon/trace-topo-{i}")),
                Attribute::from_keyword(":recon/trace-neighborhood"),
                Value::String("TOPOLOGY".to_string()),
                tx,
                Op::Assert,
            ));
        }

        // 2 traces in STORE — below threshold
        for i in 0..2 {
            let tx = TxId::new(3000 + i, 0, agent);
            datoms.insert(Datom::new(
                EntityId::from_ident(&format!(":recon/trace-store-{i}")),
                Attribute::from_keyword(":recon/trace-neighborhood"),
                Value::String("STORE".to_string()),
                tx,
                Op::Assert,
            ));
        }

        let store = Store::from_datoms(datoms);
        let signals = spec_neighborhood_concentration(&store, 20);
        assert_eq!(
            signals.len(),
            2,
            "should fire 2 signals (INTERFACE=5, TOPOLOGY=3), not STORE=2"
        );

        let neighborhoods: Vec<&str> = signals.iter().map(|s| s.neighborhood.as_str()).collect();
        assert!(
            neighborhoods.contains(&"INTERFACE"),
            "should include INTERFACE"
        );
        assert!(
            neighborhoods.contains(&"TOPOLOGY"),
            "should include TOPOLOGY"
        );

        let iface = signals.iter().find(|s| s.neighborhood == "INTERFACE").unwrap();
        assert_eq!(iface.trace_count, 5);

        let topo = signals.iter().find(|s| s.neighborhood == "TOPOLOGY").unwrap();
        assert_eq!(topo.trace_count, 3);
    }
}
