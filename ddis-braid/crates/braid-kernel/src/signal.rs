//! Signal system — event-driven communication within the store.
//!
//! Signals are typed events that trigger specific responses. At Stage 1,
//! only the `Confusion` signal type is implemented. Other types (Conflict,
//! UncertaintySpike, etc.) are Stage 2+.
//!
//! # Formal Model (spec/09-signal.md)
//!
//! ```text
//! SignalType = Confusion | Conflict | UncertaintySpike | ResolutionProposal
//!            | DelegationRequest | GoalDrift | BranchReady | DeliberationTurn
//!
//! Signal = (type: SignalType, source: EntityId, severity: f64,
//!           payload: Value, timestamp: u64)
//!
//! dispatch: Signal → Action where:
//!   Confusion → re-ASSOCIATE (rebuild context from store, recompute seed)
//!   Conflict → route to deliberation pipeline (Stage 2)
//!   UncertaintySpike → escalate to guidance (Stage 2)
//!   GoalDrift → emit ADR revision proposal (Stage 2)
//! ```
//!
//! # Traces To
//!
//! - INV-SIGNAL-002: Confusion triggers re-ASSOCIATE
//! - ADR-SIGNAL-001: Signal as first-class datom
//! - ADR-SIGNAL-003: Subscription debounce over immediate fire
//! - SEED.md §8 (Signal system)

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;

// ===========================================================================
// Signal Types (Stage 1: Confusion only)
// ===========================================================================

/// Signal type classification.
///
/// Stage 1 implements only `Confusion`. Other variants are defined for
/// forward compatibility but cannot be constructed at Stage 1.
///
/// INV-SIGNAL-006: Taxonomy completeness — all divergence types mapped.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SignalType {
    /// Agent is confused — M(t) declining or stuck.
    /// Triggers: re-ASSOCIATE (rebuild context from store).
    Confusion,
    // --- Stage 2+ (defined but not constructible at Stage 1) ---
    // Conflict,
    // UncertaintySpike,
    // ResolutionProposal,
    // DelegationRequest,
    // GoalDrift,
    // BranchReady,
    // DeliberationTurn,
}

/// Severity level for signals.
///
/// Higher severity → more urgent response.
/// - Low (0.0–0.3): informational, may be batched
/// - Medium (0.3–0.7): should be acted on soon
/// - High (0.7–1.0): immediate action required
#[derive(Clone, Debug, PartialEq)]
pub struct Severity(f64);

impl Severity {
    /// Create a new severity, clamped to [0.0, 1.0].
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    /// Get the numeric severity value.
    pub fn value(&self) -> f64 {
        self.0
    }

    /// Is this a high-severity signal (>= 0.7)?
    pub fn is_high(&self) -> bool {
        self.0 >= 0.7
    }

    /// Is this a medium-severity signal (0.3–0.7)?
    pub fn is_medium(&self) -> bool {
        self.0 >= 0.3 && self.0 < 0.7
    }

    /// Is this a low-severity signal (< 0.3)?
    pub fn is_low(&self) -> bool {
        self.0 < 0.3
    }
}

/// A signal event in the store.
///
/// Signals are first-class datoms (ADR-SIGNAL-001): they are persisted
/// in the store like any other fact, enabling audit and replay.
#[derive(Clone, Debug)]
pub struct Signal {
    /// What kind of signal.
    pub signal_type: SignalType,
    /// Entity that triggered the signal (e.g., the session entity).
    pub source: EntityId,
    /// How urgent the signal is.
    pub severity: Severity,
    /// Additional context (signal-type-specific).
    pub payload: String,
    /// Wall-clock timestamp (HLC component).
    pub timestamp: u64,
}

/// Action to take in response to a signal.
///
/// INV-SIGNAL-002: Confusion → ReAssociate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignalAction {
    /// Rebuild context from store. Recompute seed.
    /// Triggered by: Confusion.
    ReAssociate,
    // --- Stage 2+ ---
    // RouteToDeliberation,
    // EscalateToGuidance,
    // EmitADRRevision,
}

// ===========================================================================
// Signal Emission and Dispatch
// ===========================================================================

/// Dispatch a signal to its appropriate action.
///
/// INV-SIGNAL-002: For all states where confusion = true, the system
/// returns a ReAssociate action.
///
/// INV-SIGNAL-005: Signal dispatch is deterministic —
/// same signal type always maps to same action.
pub fn dispatch(signal: &Signal) -> SignalAction {
    match signal.signal_type {
        SignalType::Confusion => SignalAction::ReAssociate,
    }
}

/// Convert a signal to datoms for persistence in the store.
///
/// ADR-SIGNAL-001: Signal as first-class datom.
/// Each signal becomes an entity with typed attributes.
pub fn signal_to_datoms(signal: &Signal, tx_id: TxId) -> Vec<Datom> {
    let ident = format!(
        ":signal/{}",
        signal.timestamp
    );
    let entity = EntityId::from_ident(&ident);

    vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":signal/type"),
            Value::Keyword(format!(":signal.type/{:?}", signal.signal_type).to_lowercase()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":signal/source"),
            Value::Ref(signal.source),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":signal/severity"),
            Value::Double(ordered_float::OrderedFloat(signal.severity.value())),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":signal/payload"),
            Value::String(signal.payload.clone()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":signal/timestamp"),
            Value::Instant(signal.timestamp),
            tx_id,
            Op::Assert,
        ),
    ]
}

// ===========================================================================
// Confusion Detection (INV-SIGNAL-002)
// ===========================================================================

/// Session telemetry for confusion detection.
///
/// Tracks the signals that indicate agent confusion:
/// - M(t) < 0.3 → high-severity confusion (context is very stale)
/// - M(t) < 0.5 and declining → medium-severity (degrading)
/// - Query diversity = 0 for 5+ turns → low-severity (stuck in loop)
#[derive(Clone, Debug)]
pub struct ConfusionDetector {
    /// Current methodology alignment score M(t) ∈ [0, 1].
    pub current_mt: f64,
    /// Recent M(t) history (oldest first).
    pub mt_history: Vec<f64>,
    /// Number of distinct query patterns in recent turns.
    pub query_diversity: usize,
    /// Number of turns since last unique query.
    pub turns_since_unique_query: usize,
    /// Minimum declining turns to trigger medium confusion.
    pub declining_threshold: usize,
    /// Minimum stuck turns to trigger low confusion.
    pub stuck_threshold: usize,
}

impl Default for ConfusionDetector {
    fn default() -> Self {
        Self {
            current_mt: 0.5,
            mt_history: Vec::new(),
            declining_threshold: 3,
            stuck_threshold: 5,
            query_diversity: 1,
            turns_since_unique_query: 0,
        }
    }
}

/// Detect confusion from session telemetry.
///
/// INV-SIGNAL-002: For all states where confusion(s) = true, the system
/// emits a Confusion signal within the same turn.
///
/// Returns `Some(Signal)` if confusion is detected, `None` otherwise.
/// The signal severity reflects the urgency:
/// - High (>= 0.7): M(t) < 0.3 — context is very stale
/// - Medium (0.3–0.7): M(t) < 0.5 and declining over 3+ turns
/// - Low (< 0.3): query diversity = 0 for 5+ turns (stuck in loop)
pub fn detect_confusion(
    detector: &ConfusionDetector,
    source: EntityId,
    timestamp: u64,
) -> Option<Signal> {
    // Rule 1: M(t) < 0.3 → high-severity confusion
    if detector.current_mt < 0.3 {
        return Some(Signal {
            signal_type: SignalType::Confusion,
            source,
            severity: Severity::new(0.9),
            payload: format!(
                "M(t)={:.2} < 0.3 — context is very stale. Recommended: braid seed --task 'your task'",
                detector.current_mt
            ),
            timestamp,
        });
    }

    // Rule 2: M(t) < 0.5 and declining over N turns → medium-severity
    if detector.current_mt < 0.5 && is_declining(&detector.mt_history, detector.declining_threshold)
    {
        return Some(Signal {
            signal_type: SignalType::Confusion,
            source,
            severity: Severity::new(0.5),
            payload: format!(
                "M(t)={:.2} declining over {} turns — context degrading. Consider: braid harvest --commit",
                detector.current_mt,
                detector.declining_threshold
            ),
            timestamp,
        });
    }

    // Rule 3: Query diversity = 0 for N+ turns → low-severity (stuck)
    if detector.query_diversity == 0
        && detector.turns_since_unique_query >= detector.stuck_threshold
    {
        return Some(Signal {
            signal_type: SignalType::Confusion,
            source,
            severity: Severity::new(0.2),
            payload: format!(
                "No unique queries for {} turns — agent may be stuck. Try: braid seed --task 'different approach'",
                detector.turns_since_unique_query
            ),
            timestamp,
        });
    }

    None
}

/// Check if M(t) has been declining over the last `window` entries.
fn is_declining(history: &[f64], window: usize) -> bool {
    if history.len() < window {
        return false;
    }
    let recent = &history[history.len() - window..];
    recent.windows(2).all(|w| w[1] <= w[0] + 1e-10) // Allow tiny float noise
}

/// Build a corrective guidance footer when confusion is detected.
///
/// INV-SIGNAL-002: The guidance footer includes a re-association action.
/// This replaces the normal navigative footer with a corrective one.
pub fn corrective_footer(signal: &Signal) -> String {
    let severity_label = if signal.severity.is_high() {
        "CRITICAL"
    } else if signal.severity.is_medium() {
        "WARNING"
    } else {
        "INFO"
    };

    format!(
        "⚠ {severity_label}: confusion detected — {}",
        signal.payload
    )
}

/// Count signals in a store by type.
pub fn count_signals(store: &Store, signal_type: &str) -> usize {
    let type_attr = Attribute::from_keyword(":signal/type");
    let target = format!(":signal.type/{signal_type}");
    store
        .attribute_datoms(&type_attr)
        .iter()
        .filter(|d| d.op == Op::Assert && d.value == Value::Keyword(target.clone()))
        .count()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, EntityId};

    fn test_source() -> EntityId {
        EntityId::from_ident(":test/session")
    }

    // --- INV-SIGNAL-002: Confusion triggers re-ASSOCIATE ---

    #[test]
    fn high_severity_confusion_at_low_mt() {
        let detector = ConfusionDetector {
            current_mt: 0.2,
            ..Default::default()
        };
        let signal = detect_confusion(&detector, test_source(), 1000);
        assert!(signal.is_some(), "M(t)=0.2 should trigger confusion");
        let s = signal.unwrap();
        assert_eq!(s.signal_type, SignalType::Confusion);
        assert!(s.severity.is_high(), "M(t)<0.3 should be high severity");
        assert_eq!(dispatch(&s), SignalAction::ReAssociate);
    }

    #[test]
    fn medium_severity_confusion_on_decline() {
        let detector = ConfusionDetector {
            current_mt: 0.45,
            mt_history: vec![0.6, 0.55, 0.5, 0.48, 0.45],
            declining_threshold: 3,
            ..Default::default()
        };
        let signal = detect_confusion(&detector, test_source(), 2000);
        assert!(signal.is_some(), "Declining M(t)<0.5 should trigger confusion");
        let s = signal.unwrap();
        assert!(s.severity.is_medium());
    }

    #[test]
    fn low_severity_confusion_when_stuck() {
        let detector = ConfusionDetector {
            current_mt: 0.7, // M(t) is fine
            query_diversity: 0,
            turns_since_unique_query: 7,
            stuck_threshold: 5,
            ..Default::default()
        };
        let signal = detect_confusion(&detector, test_source(), 3000);
        assert!(signal.is_some(), "Zero diversity for 7 turns should trigger");
        let s = signal.unwrap();
        assert!(s.severity.is_low());
    }

    #[test]
    fn no_confusion_when_healthy() {
        let detector = ConfusionDetector {
            current_mt: 0.7,
            mt_history: vec![0.5, 0.6, 0.65, 0.7],
            query_diversity: 3,
            turns_since_unique_query: 0,
            ..Default::default()
        };
        let signal = detect_confusion(&detector, test_source(), 4000);
        assert!(signal.is_none(), "Healthy session should not trigger confusion");
    }

    // --- Signal dispatch ---

    #[test]
    fn dispatch_confusion_to_reassociate() {
        let signal = Signal {
            signal_type: SignalType::Confusion,
            source: test_source(),
            severity: Severity::new(0.9),
            payload: "test".into(),
            timestamp: 5000,
        };
        assert_eq!(dispatch(&signal), SignalAction::ReAssociate);
    }

    // --- Signal to datoms ---

    #[test]
    fn signal_to_datoms_produces_6_datoms() {
        let agent = AgentId::from_name("test");
        let tx_id = TxId::new(100, 0, agent);
        let signal = Signal {
            signal_type: SignalType::Confusion,
            source: test_source(),
            severity: Severity::new(0.8),
            payload: "test payload".into(),
            timestamp: 6000,
        };
        let datoms = signal_to_datoms(&signal, tx_id);
        assert_eq!(datoms.len(), 6, "Signal should produce 6 datoms");
        // All datoms share the same entity
        let entity = datoms[0].entity;
        for d in &datoms {
            assert_eq!(d.entity, entity, "All datoms should share entity");
        }
    }

    // --- Corrective footer ---

    #[test]
    fn corrective_footer_contains_severity() {
        let signal = Signal {
            signal_type: SignalType::Confusion,
            source: test_source(),
            severity: Severity::new(0.9),
            payload: "M(t)=0.2 — stale".into(),
            timestamp: 7000,
        };
        let footer = corrective_footer(&signal);
        assert!(footer.contains("CRITICAL"));
        assert!(footer.contains("M(t)=0.2"));
    }

    // --- Severity bounds ---

    #[test]
    fn severity_clamped_to_unit_interval() {
        assert_eq!(Severity::new(-1.0).value(), 0.0);
        assert_eq!(Severity::new(2.0).value(), 1.0);
        assert_eq!(Severity::new(0.5).value(), 0.5);
    }

    // --- Property tests ---

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// INV-SIGNAL-002: detect_confusion is total — never panics.
            #[test]
            fn detect_confusion_total(
                mt in 0.0f64..=1.0,
                history_len in 0usize..=10,
                diversity in 0usize..=5,
                turns_stuck in 0usize..=20,
            ) {
                let history: Vec<f64> = (0..history_len).map(|i| mt - (i as f64) * 0.01).collect();
                let detector = ConfusionDetector {
                    current_mt: mt,
                    mt_history: history,
                    query_diversity: diversity,
                    turns_since_unique_query: turns_stuck,
                    ..Default::default()
                };
                let source = EntityId::from_ident(":test/proptest");
                // Must not panic for any inputs
                let _ = detect_confusion(&detector, source, 999);
            }

            /// INV-SIGNAL-002: When confusion IS detected, severity is in [0, 1].
            #[test]
            fn confusion_severity_bounded(
                mt in 0.0f64..=0.5,
            ) {
                let detector = ConfusionDetector {
                    current_mt: mt,
                    mt_history: vec![0.6, 0.5, 0.4, mt],
                    declining_threshold: 3,
                    ..Default::default()
                };
                let source = EntityId::from_ident(":test/bound");
                if let Some(signal) = detect_confusion(&detector, source, 888) {
                    let sev = signal.severity.value();
                    prop_assert!((0.0..=1.0).contains(&sev),
                        "Severity {} out of [0,1]", sev);
                }
            }
        }
    }
}
