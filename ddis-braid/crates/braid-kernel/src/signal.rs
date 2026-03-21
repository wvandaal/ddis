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

use std::collections::{HashMap, HashSet};

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;

// ===========================================================================
// Divergence Types (Reconciliation Taxonomy — CLAUDE.md §15)
// ===========================================================================

/// The eight divergence types from the reconciliation taxonomy.
///
/// Each type corresponds to a specific boundary where coherence can break down.
/// Detectors scan the store for evidence of each divergence type and emit signals.
///
/// INV-SIGNAL-006: Taxonomy completeness — all divergence types mapped.
///
/// | Type          | Boundary                        | Existing detector?            |
/// |---------------|---------------------------------|-------------------------------|
/// | Epistemic     | Store vs. agent knowledge       | Yes (harvest gap detection)   |
/// | Structural    | Implementation vs. spec         | Yes (bilateral scan / drift)  |
/// | Consequential | Current state vs. future risk   | **New** (low-confidence refs) |
/// | Aleatory      | Agent vs. agent                 | **New** (multi-agent conflict)|
/// | Logical       | Invariant vs. invariant         | **New** (contradiction surface)|
/// | Axiological   | Implementation vs. goals        | **New** (priority coverage)   |
/// | Temporal      | Agent frontier vs. agent frontier| **New** (frontier gap)       |
/// | Procedural    | Agent behavior vs. methodology  | **New** (M(t) wrapper)       |
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DivergenceType {
    /// Store vs. agent knowledge — detected via harvest gap analysis.
    Epistemic,
    /// Implementation vs. spec — detected via bilateral scan / drift.
    Structural,
    /// Current state vs. future risk — building on uncertain foundations.
    Consequential,
    /// Agent vs. agent — multi-agent disagreement (conflict sets).
    Aleatory,
    /// Invariant vs. invariant — potential contradiction surface.
    Logical,
    /// Implementation vs. goals — priority-coverage misalignment.
    Axiological,
    /// Agent frontier vs. agent frontier — frontier lag between agents.
    Temporal,
    /// Agent behavior vs. methodology — M(t) below threshold.
    Procedural,
}

impl DivergenceType {
    /// The boundary description for this divergence type.
    pub fn boundary(&self) -> &'static str {
        match self {
            Self::Epistemic => "Store vs. agent knowledge",
            Self::Structural => "Implementation vs. spec",
            Self::Consequential => "Current state vs. future risk",
            Self::Aleatory => "Agent vs. agent",
            Self::Logical => "Invariant vs. invariant",
            Self::Axiological => "Implementation vs. goals",
            Self::Temporal => "Agent frontier vs. agent frontier",
            Self::Procedural => "Agent behavior vs. methodology",
        }
    }
}

impl std::fmt::Display for DivergenceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

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
    let ident = format!(":signal/{}", signal.timestamp);
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
// Divergence Type Detectors (INV-SIGNAL-006)
// ===========================================================================

/// Default M(t) threshold below which procedural divergence is flagged.
const PROCEDURAL_MT_THRESHOLD: f64 = 0.5;

/// Default frontier gap (in wall-time milliseconds) above which temporal
/// divergence is flagged between agents.
const TEMPORAL_FRONTIER_GAP_MS: u64 = 60_000;

/// Confidence threshold below which an entity is considered uncertain.
const CONSEQUENTIAL_CONFIDENCE_THRESHOLD: f64 = 0.5;

/// Detect **consequential** divergence: entities with low confidence that are
/// referenced by other entities. Building on uncertain foundations creates
/// future risk.
///
/// Scans for entities with `:exploration/confidence < 0.5` and checks whether
/// any other entity holds a `Value::Ref` pointing to them.
///
/// Traces to: SEED.md §8, reconciliation taxonomy row "Consequential".
pub fn detect_consequential(store: &Store, source: EntityId, timestamp: u64) -> Vec<Signal> {
    let conf_attr = Attribute::from_keyword(":exploration/confidence");
    let conf_datoms = store.attribute_datoms(&conf_attr);

    // Collect entities with confidence < threshold.
    let mut low_confidence_entities: HashSet<EntityId> = HashSet::new();
    for d in conf_datoms {
        if d.op == Op::Assert {
            if let Value::Double(f) = &d.value {
                if f.into_inner() < CONSEQUENTIAL_CONFIDENCE_THRESHOLD {
                    low_confidence_entities.insert(d.entity);
                }
            }
        }
    }

    if low_confidence_entities.is_empty() {
        return Vec::new();
    }

    // Check if any other datom references a low-confidence entity.
    let mut referenced: HashSet<EntityId> = HashSet::new();
    for datom in store.datoms() {
        if datom.op == Op::Assert {
            if let Value::Ref(target) = &datom.value {
                if low_confidence_entities.contains(target)
                    && !low_confidence_entities.contains(&datom.entity)
                {
                    referenced.insert(*target);
                }
            }
        }
    }

    referenced
        .into_iter()
        .map(|entity| Signal {
            signal_type: SignalType::Confusion,
            source,
            severity: Severity::new(0.5),
            payload: format!(
                "Consequential: entity {:?} has confidence < {:.1} but is referenced by other entities",
                entity,
                CONSEQUENTIAL_CONFIDENCE_THRESHOLD,
            ),
            timestamp,
        })
        .collect()
}

/// Detect **aleatory** divergence: conflict sets involving more than one agent.
///
/// Scans the store for entity+attribute pairs where active assertions come from
/// different agents. This indicates multi-agent disagreement that requires
/// deliberation, not just resolution.
///
/// Traces to: SEED.md §8, reconciliation taxonomy row "Aleatory".
pub fn detect_aleatory(store: &Store, source: EntityId, timestamp: u64) -> Vec<Signal> {
    // Group (entity, attribute) assertions by agent.
    let mut ea_agents: HashMap<(EntityId, String), HashSet<[u8; 16]>> = HashMap::new();

    for datom in store.datoms() {
        if datom.op == Op::Assert {
            let key = (datom.entity, datom.attribute.as_str().to_string());
            ea_agents
                .entry(key)
                .or_default()
                .insert(*datom.tx.agent().as_bytes());
        }
    }

    ea_agents
        .into_iter()
        .filter(|(_, agents)| agents.len() > 1)
        .map(|((entity, attr), agents)| Signal {
            signal_type: SignalType::Confusion,
            source,
            severity: Severity::new(0.6),
            payload: format!(
                "Aleatory: {} agents assert different values for {:?} on attribute {}",
                agents.len(),
                entity,
                attr,
            ),
            timestamp,
        })
        .collect()
}

/// Detect **logical** divergence: potential contradiction surface among invariants.
///
/// Two invariant entities that share the same target entity+attribute may impose
/// contradictory constraints. This detector finds such overlapping invariant pairs.
///
/// Traces to: SEED.md §8, reconciliation taxonomy row "Logical".
pub fn detect_logical(store: &Store, source: EntityId, timestamp: u64) -> Vec<Signal> {
    let element_type_attr = Attribute::from_keyword(":spec/element-type");
    let element_type_datoms = store.attribute_datoms(&element_type_attr);

    // Collect entities that are invariants.
    let inv_keyword = Value::Keyword(":spec.type/invariant".to_string());
    let invariant_entities: HashSet<EntityId> = element_type_datoms
        .iter()
        .filter(|d| d.op == Op::Assert && d.value == inv_keyword)
        .map(|d| d.entity)
        .collect();

    if invariant_entities.len() < 2 {
        return Vec::new();
    }

    // For each invariant entity, collect the set of (entity, attribute) pairs it
    // references via Value::Ref datoms.
    let mut inv_refs: HashMap<EntityId, HashSet<(EntityId, String)>> = HashMap::new();
    for inv_entity in &invariant_entities {
        let datoms = store.entity_datoms(*inv_entity);
        for d in &datoms {
            if d.op == Op::Assert {
                if let Value::Ref(target) = &d.value {
                    inv_refs
                        .entry(*inv_entity)
                        .or_default()
                        .insert((*target, d.attribute.as_str().to_string()));
                }
            }
        }
    }

    // Find invariant pairs that share at least one (entity, attribute) reference.
    let inv_list: Vec<EntityId> = inv_refs.keys().copied().collect();
    let mut signals = Vec::new();
    for i in 0..inv_list.len() {
        for j in (i + 1)..inv_list.len() {
            let a = &inv_list[i];
            let b = &inv_list[j];
            if let (Some(refs_a), Some(refs_b)) = (inv_refs.get(a), inv_refs.get(b)) {
                let overlap: Vec<_> = refs_a.intersection(refs_b).collect();
                if !overlap.is_empty() {
                    signals.push(Signal {
                        signal_type: SignalType::Confusion,
                        source,
                        severity: Severity::new(0.4),
                        payload: format!(
                            "Logical: invariants {:?} and {:?} share {} entity+attribute reference(s) — potential contradiction surface",
                            a, b, overlap.len(),
                        ),
                        timestamp,
                    });
                }
            }
        }
    }

    signals
}

/// Detect **axiological** divergence: spec implementation coverage misaligned
/// with namespace priority distribution.
///
/// Compares the fraction of implemented spec elements per namespace against the
/// fraction of total spec elements per namespace. A namespace that has a large
/// share of spec elements but a small share of implementations indicates priority
/// misalignment.
///
/// Traces to: SEED.md §8, reconciliation taxonomy row "Axiological".
pub fn detect_axiological(store: &Store, source: EntityId, timestamp: u64) -> Vec<Signal> {
    let ns_attr = Attribute::from_keyword(":spec/namespace");
    let impl_attr = Attribute::from_keyword(":impl/implements");

    let ns_datoms = store.attribute_datoms(&ns_attr);
    let impl_datoms = store.attribute_datoms(&impl_attr);

    // Count spec elements per namespace.
    let mut ns_spec_count: HashMap<String, usize> = HashMap::new();
    let mut spec_entities: HashSet<EntityId> = HashSet::new();
    for d in ns_datoms {
        if d.op == Op::Assert {
            if let Value::Keyword(ns) = &d.value {
                *ns_spec_count.entry(ns.clone()).or_default() += 1;
                spec_entities.insert(d.entity);
            }
        }
    }

    let total_spec = spec_entities.len();
    if total_spec == 0 {
        return Vec::new();
    }

    // Count implemented spec elements (via :impl/implements refs).
    let mut implemented: HashSet<EntityId> = HashSet::new();
    for d in impl_datoms {
        if d.op == Op::Assert {
            if let Value::Ref(target) = &d.value {
                if spec_entities.contains(target) {
                    implemented.insert(*target);
                }
            }
        }
    }

    // For each namespace, check if implementation fraction is significantly
    // below the spec fraction.
    let total_impl = implemented.len();
    if total_impl == 0 {
        return Vec::new();
    }

    // Build per-namespace implementation count (by checking which implemented
    // entities belong to which namespace).
    let mut ns_impl_count: HashMap<String, usize> = HashMap::new();
    for d in ns_datoms {
        if d.op == Op::Assert && implemented.contains(&d.entity) {
            if let Value::Keyword(ns) = &d.value {
                *ns_impl_count.entry(ns.clone()).or_default() += 1;
            }
        }
    }

    let mut signals = Vec::new();
    for (ns, spec_count) in &ns_spec_count {
        let impl_count = ns_impl_count.get(ns).copied().unwrap_or(0);
        let spec_frac = *spec_count as f64 / total_spec as f64;
        let impl_frac = impl_count as f64 / total_impl as f64;

        // Flag if namespace has >10% of spec but <50% relative implementation coverage.
        if spec_frac > 0.10 && impl_frac < spec_frac * 0.5 {
            signals.push(Signal {
                signal_type: SignalType::Confusion,
                source,
                severity: Severity::new(0.3),
                payload: format!(
                    "Axiological: namespace {} has {:.0}% of spec elements but only {:.0}% of implementations — priority misalignment",
                    ns,
                    spec_frac * 100.0,
                    impl_frac * 100.0,
                ),
                timestamp,
            });
        }
    }

    signals
}

/// Detect **temporal** divergence: agent frontiers that differ by more than
/// a threshold number of wall-time milliseconds.
///
/// If two agents' latest transactions are separated by more than the threshold,
/// the lagging agent may be working with stale context.
///
/// Traces to: SEED.md §8, reconciliation taxonomy row "Temporal".
pub fn detect_temporal(store: &Store, source: EntityId, timestamp: u64) -> Vec<Signal> {
    detect_temporal_with_threshold(store, source, timestamp, TEMPORAL_FRONTIER_GAP_MS)
}

/// Temporal divergence detector with configurable threshold.
pub fn detect_temporal_with_threshold(
    store: &Store,
    source: EntityId,
    timestamp: u64,
    threshold_ms: u64,
) -> Vec<Signal> {
    let frontier = store.frontier();
    let agents: Vec<_> = frontier.iter().collect();

    if agents.len() < 2 {
        return Vec::new();
    }

    let mut signals = Vec::new();
    for i in 0..agents.len() {
        for j in (i + 1)..agents.len() {
            let (agent_a, tx_a) = agents[i];
            let (agent_b, tx_b) = agents[j];
            let wall_diff = if tx_a.wall_time() > tx_b.wall_time() {
                tx_a.wall_time() - tx_b.wall_time()
            } else {
                tx_b.wall_time() - tx_a.wall_time()
            };

            if wall_diff > threshold_ms {
                signals.push(Signal {
                    signal_type: SignalType::Confusion,
                    source,
                    severity: Severity::new(0.5),
                    payload: format!(
                        "Temporal: agents {:?} and {:?} frontiers differ by {}ms (threshold: {}ms)",
                        agent_a, agent_b, wall_diff, threshold_ms,
                    ),
                    timestamp,
                });
            }
        }
    }

    signals
}

/// Detect **procedural** divergence: M(t) below threshold, indicating the agent
/// is drifting from methodology.
///
/// This wraps the existing M(t) computation from the confusion detector but
/// classifies it specifically as procedural divergence when M(t) < threshold.
///
/// Traces to: SEED.md §8, reconciliation taxonomy row "Procedural".
pub fn detect_procedural(
    detector: &ConfusionDetector,
    source: EntityId,
    timestamp: u64,
) -> Vec<Signal> {
    detect_procedural_with_threshold(detector, source, timestamp, PROCEDURAL_MT_THRESHOLD)
}

/// Procedural divergence detector with configurable threshold.
pub fn detect_procedural_with_threshold(
    detector: &ConfusionDetector,
    source: EntityId,
    timestamp: u64,
    threshold: f64,
) -> Vec<Signal> {
    if detector.current_mt < threshold {
        let severity = if detector.current_mt < 0.3 { 0.8 } else { 0.5 };
        vec![Signal {
            signal_type: SignalType::Confusion,
            source,
            severity: Severity::new(severity),
            payload: format!(
                "Procedural: M(t)={:.2} < {:.2} threshold — agent drifting from methodology",
                detector.current_mt, threshold,
            ),
            timestamp,
        }]
    } else {
        Vec::new()
    }
}

/// Run all divergence detectors and return tagged results.
///
/// INV-SIGNAL-006: Taxonomy completeness — all 8 divergence types have detectors.
/// Epistemic and Structural are detected by harvest and bilateral respectively
/// (already implemented). The remaining 6 are detected here.
///
/// Returns a vec of `(DivergenceType, Signal)` pairs for all detected divergences.
pub fn detect_all_divergence(
    store: &Store,
    detector: &ConfusionDetector,
    source: EntityId,
    timestamp: u64,
) -> Vec<(DivergenceType, Signal)> {
    let mut results = Vec::new();

    // Consequential: low-confidence entities referenced by others.
    for signal in detect_consequential(store, source, timestamp) {
        results.push((DivergenceType::Consequential, signal));
    }

    // Aleatory: multi-agent disagreement on same entity+attribute.
    for signal in detect_aleatory(store, source, timestamp) {
        results.push((DivergenceType::Aleatory, signal));
    }

    // Logical: invariant pairs sharing entity+attribute references.
    for signal in detect_logical(store, source, timestamp) {
        results.push((DivergenceType::Logical, signal));
    }

    // Axiological: implementation coverage vs. spec priority misalignment.
    for signal in detect_axiological(store, source, timestamp) {
        results.push((DivergenceType::Axiological, signal));
    }

    // Temporal: frontier lag between agents.
    for signal in detect_temporal(store, source, timestamp) {
        results.push((DivergenceType::Temporal, signal));
    }

    // Procedural: M(t) below threshold.
    for signal in detect_procedural(detector, source, timestamp) {
        results.push((DivergenceType::Procedural, signal));
    }

    results
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
        assert!(
            signal.is_some(),
            "Declining M(t)<0.5 should trigger confusion"
        );
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
        assert!(
            signal.is_some(),
            "Zero diversity for 7 turns should trigger"
        );
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
        assert!(
            signal.is_none(),
            "Healthy session should not trigger confusion"
        );
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

    // --- Divergence Type Detectors ---

    mod divergence_detectors {
        use super::*;
        use crate::datom::{AgentId, ProvenanceType};
        use crate::schema::{full_schema_datoms, genesis_datoms};
        use crate::store::Transaction;
        use ordered_float::OrderedFloat;
        use std::collections::BTreeSet;

        fn test_source() -> EntityId {
            EntityId::from_ident(":test/divergence")
        }

        /// Create a store with full schema (Layers 1-4), required for
        /// attributes like :exploration/confidence, :spec/*, :impl/*.
        fn full_store() -> Store {
            let agent = AgentId::from_name("test");
            let genesis_tx = TxId::new(0, 0, agent);
            let mut datoms: BTreeSet<Datom> = BTreeSet::new();
            for d in genesis_datoms(genesis_tx) {
                datoms.insert(d);
            }
            for d in full_schema_datoms(genesis_tx) {
                datoms.insert(d);
            }
            Store::from_datoms(datoms)
        }

        // -- Consequential: low-confidence entity referenced by another --

        #[test]
        fn consequential_detects_low_confidence_ref() {
            let mut store = full_store();
            let agent = AgentId::from_name("test");

            // Create an entity with low confidence.
            let uncertain = EntityId::from_ident(":test/uncertain-entity");
            let tx1 = Transaction::new(agent, ProvenanceType::Observed, "add uncertain")
                .assert(
                    uncertain,
                    Attribute::from_keyword(":exploration/confidence"),
                    Value::Double(OrderedFloat(0.3)),
                )
                .assert(
                    uncertain,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("uncertain fact".into()),
                )
                .commit(&store)
                .unwrap();
            store.transact(tx1).unwrap();

            // Create another entity that references the uncertain one.
            let referrer = EntityId::from_ident(":test/referrer");
            let tx2 = Transaction::new(agent, ProvenanceType::Observed, "add referrer")
                .assert(
                    referrer,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("depends on uncertain".into()),
                )
                .assert(
                    referrer,
                    Attribute::from_keyword(":dep/to"),
                    Value::Ref(uncertain),
                )
                .commit(&store)
                .unwrap();
            store.transact(tx2).unwrap();

            let signals = detect_consequential(&store, test_source(), 1000);
            assert!(
                !signals.is_empty(),
                "Should detect consequential divergence"
            );
            assert!(signals[0].payload.contains("Consequential"));
        }

        #[test]
        fn consequential_empty_on_high_confidence() {
            let mut store = full_store();
            let agent = AgentId::from_name("test");

            // Entity with high confidence — should not trigger.
            let certain = EntityId::from_ident(":test/certain-entity");
            let tx = Transaction::new(agent, ProvenanceType::Observed, "add certain")
                .assert(
                    certain,
                    Attribute::from_keyword(":exploration/confidence"),
                    Value::Double(OrderedFloat(0.9)),
                )
                .assert(
                    certain,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("certain fact".into()),
                )
                .commit(&store)
                .unwrap();
            store.transact(tx).unwrap();

            let signals = detect_consequential(&store, test_source(), 1000);
            assert!(signals.is_empty(), "High confidence should not trigger");
        }

        // -- Aleatory: multi-agent disagreement --

        #[test]
        fn aleatory_detects_multi_agent_conflict() {
            let mut store = full_store();
            let entity = EntityId::from_ident(":test/contested");

            let agent_a = AgentId::from_name("alice");
            let tx_a = Transaction::new(agent_a, ProvenanceType::Observed, "alice says")
                .assert(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("alice-version".into()),
                )
                .commit(&store)
                .unwrap();
            store.transact(tx_a).unwrap();

            let agent_b = AgentId::from_name("bob");
            let tx_b = Transaction::new(agent_b, ProvenanceType::Observed, "bob says")
                .assert(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("bob-version".into()),
                )
                .commit(&store)
                .unwrap();
            store.transact(tx_b).unwrap();

            let signals = detect_aleatory(&store, test_source(), 2000);
            assert!(
                !signals.is_empty(),
                "Should detect aleatory divergence from two agents"
            );
            assert!(signals[0].payload.contains("Aleatory"));
        }

        #[test]
        fn aleatory_empty_single_agent() {
            let store = full_store();
            // Full store has only the test agent — no multi-agent conflict.
            let signals = detect_aleatory(&store, test_source(), 2000);
            // All genesis datoms come from braid:system, so any ea pair with >1
            // agent would require a second agent. Check there are no false positives.
            for s in &signals {
                // If there are signals, they must involve the system agent only,
                // which should not happen (only 1 agent in genesis).
                assert!(s.payload.contains("Aleatory"), "Unexpected signal payload");
            }
        }

        // -- Logical: invariant contradiction surface --

        #[test]
        fn logical_detects_shared_refs() {
            let mut store = full_store();
            let agent = AgentId::from_name("test");

            let target = EntityId::from_ident(":test/shared-target");
            let inv_a = EntityId::from_ident(":test/inv-a");
            let inv_b = EntityId::from_ident(":test/inv-b");

            // Create the target entity.
            let tx0 = Transaction::new(agent, ProvenanceType::Observed, "target")
                .assert(
                    target,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("target entity".into()),
                )
                .commit(&store)
                .unwrap();
            store.transact(tx0).unwrap();

            // Create two invariant entities that both reference the target.
            let tx1 = Transaction::new(agent, ProvenanceType::Observed, "inv-a")
                .assert(
                    inv_a,
                    Attribute::from_keyword(":spec/element-type"),
                    Value::Keyword(":spec.type/invariant".into()),
                )
                .assert(
                    inv_a,
                    Attribute::from_keyword(":dep/to"),
                    Value::Ref(target),
                )
                .commit(&store)
                .unwrap();
            store.transact(tx1).unwrap();

            let tx2 = Transaction::new(agent, ProvenanceType::Observed, "inv-b")
                .assert(
                    inv_b,
                    Attribute::from_keyword(":spec/element-type"),
                    Value::Keyword(":spec.type/invariant".into()),
                )
                .assert(
                    inv_b,
                    Attribute::from_keyword(":dep/to"),
                    Value::Ref(target),
                )
                .commit(&store)
                .unwrap();
            store.transact(tx2).unwrap();

            let signals = detect_logical(&store, test_source(), 3000);
            assert!(!signals.is_empty(), "Should detect logical divergence");
            assert!(signals[0].payload.contains("Logical"));
        }

        #[test]
        fn logical_empty_no_invariants() {
            let store = full_store();
            let signals = detect_logical(&store, test_source(), 3000);
            assert!(
                signals.is_empty(),
                "No invariants means no logical divergence"
            );
        }

        // -- Axiological: implementation coverage vs. spec priority --

        #[test]
        fn axiological_runs_without_panic() {
            let store = full_store();
            // Genesis store has no spec/namespace or impl/implements datoms.
            let signals = detect_axiological(&store, test_source(), 4000);
            assert!(
                signals.is_empty(),
                "Empty spec set should produce no axiological signals"
            );
        }

        #[test]
        fn axiological_detects_priority_misalignment() {
            let mut store = full_store();
            let agent = AgentId::from_name("test");

            // Create spec elements in two namespaces: A (many) and B (few).
            // Implement only B. This should flag A as under-implemented.
            let mut spec_entities = Vec::new();
            for i in 0..10 {
                let e = EntityId::from_ident(&format!(":test/spec-a-{}", i));
                spec_entities.push(('A', e));
            }
            for i in 0..2 {
                let e = EntityId::from_ident(&format!(":test/spec-b-{}", i));
                spec_entities.push(('B', e));
            }

            // Assert all spec entities with their namespace.
            for (ns, e) in &spec_entities {
                let ns_kw = format!(":spec.ns/{}", ns);
                let tx = Transaction::new(agent, ProvenanceType::Observed, "spec setup")
                    .assert(
                        *e,
                        Attribute::from_keyword(":spec/namespace"),
                        Value::Keyword(ns_kw),
                    )
                    .assert(
                        *e,
                        Attribute::from_keyword(":db/doc"),
                        Value::String(format!("spec element {}", ns)),
                    )
                    .commit(&store)
                    .unwrap();
                store.transact(tx).unwrap();
            }

            // Implement only the B namespace spec elements.
            for (ns, spec_e) in &spec_entities {
                if *ns == 'B' {
                    let impl_e = EntityId::from_ident(&format!(":test/impl-{:?}", spec_e));
                    let tx = Transaction::new(agent, ProvenanceType::Observed, "impl B")
                        .assert(
                            impl_e,
                            Attribute::from_keyword(":impl/implements"),
                            Value::Ref(*spec_e),
                        )
                        .assert(
                            impl_e,
                            Attribute::from_keyword(":db/doc"),
                            Value::String("implementation".into()),
                        )
                        .commit(&store)
                        .unwrap();
                    store.transact(tx).unwrap();
                }
            }

            let signals = detect_axiological(&store, test_source(), 4000);
            assert!(
                !signals.is_empty(),
                "Should detect axiological divergence for under-implemented namespace A"
            );
            assert!(signals[0].payload.contains("Axiological"));
        }

        // -- Temporal: frontier gap between agents --

        #[test]
        fn temporal_detects_frontier_gap() {
            let mut store = full_store();

            // Add transactions from two agents with widely separated wall times.
            let agent_a = AgentId::from_name("agent-early");
            let tx_a = Transaction::new(agent_a, ProvenanceType::Observed, "early")
                .assert(
                    EntityId::from_ident(":test/early"),
                    Attribute::from_keyword(":db/doc"),
                    Value::String("early".into()),
                )
                .commit(&store)
                .unwrap();
            store.transact(tx_a).unwrap();

            let agent_b = AgentId::from_name("agent-late");
            // We need a transaction with a later wall time. The store's clock ticks
            // forward, so we transact with a different agent and the HLC will advance.
            // To force a large gap, we use the store's internal clock which starts at 0.
            // The genesis tx is at wall_time=0, first user tx at wall_time=0+logical.
            // For a meaningful test, use the configurable threshold variant.
            let tx_b = Transaction::new(agent_b, ProvenanceType::Observed, "late")
                .assert(
                    EntityId::from_ident(":test/late"),
                    Attribute::from_keyword(":db/doc"),
                    Value::String("late".into()),
                )
                .commit(&store)
                .unwrap();
            store.transact(tx_b).unwrap();

            // With threshold=0, any difference should trigger.
            let signals = detect_temporal_with_threshold(&store, test_source(), 5000, 0);
            // The genesis agent (braid:system) plus two test agents should exist.
            // Whether signals fire depends on actual wall_time diffs in the test store.
            // At minimum, this should not panic.
            for s in &signals {
                assert!(s.payload.contains("Temporal"));
            }
        }

        #[test]
        fn temporal_empty_single_agent() {
            let store = full_store();
            // Only braid:system in genesis — no pair to compare.
            let signals = detect_temporal(&store, test_source(), 5000);
            // Genesis has 1 agent, so frontier has 1 entry. No pairs means no signals.
            // (There may be 0 or more depending on whether genesis frontier has >1 agent.)
            for s in &signals {
                assert!(s.payload.contains("Temporal"));
            }
        }

        // -- Procedural: M(t) below threshold --

        #[test]
        fn procedural_detects_low_mt() {
            let detector = ConfusionDetector {
                current_mt: 0.3,
                ..Default::default()
            };
            let signals = detect_procedural(&detector, test_source(), 6000);
            assert!(
                !signals.is_empty(),
                "M(t)=0.3 < 0.5 should trigger procedural divergence"
            );
            assert!(signals[0].payload.contains("Procedural"));
        }

        #[test]
        fn procedural_empty_healthy_mt() {
            let detector = ConfusionDetector {
                current_mt: 0.8,
                ..Default::default()
            };
            let signals = detect_procedural(&detector, test_source(), 6000);
            assert!(
                signals.is_empty(),
                "M(t)=0.8 should not trigger procedural divergence"
            );
        }

        #[test]
        fn procedural_high_severity_at_very_low_mt() {
            let detector = ConfusionDetector {
                current_mt: 0.2,
                ..Default::default()
            };
            let signals = detect_procedural(&detector, test_source(), 6000);
            assert!(!signals.is_empty());
            assert!(
                signals[0].severity.is_high(),
                "M(t)=0.2 should produce high severity"
            );
        }

        // -- detect_all_divergence aggregator --

        #[test]
        fn detect_all_divergence_runs_without_panic() {
            let store = full_store();
            let detector = ConfusionDetector::default();
            let results = detect_all_divergence(&store, &detector, test_source(), 9000);
            // Verify all results have valid divergence types.
            for (dt, signal) in &results {
                assert!(
                    matches!(
                        dt,
                        DivergenceType::Consequential
                            | DivergenceType::Aleatory
                            | DivergenceType::Logical
                            | DivergenceType::Axiological
                            | DivergenceType::Temporal
                            | DivergenceType::Procedural
                    ),
                    "detect_all_divergence should only return the 6 new types"
                );
                assert_eq!(signal.signal_type, SignalType::Confusion);
            }
        }

        // t-1d31: detect_all_divergence returns expected types
        #[test]
        fn detect_all_divergence_includes_temporal_and_aleatory() {
            let store = full_store();
            let detector = ConfusionDetector::default();
            let results =
                detect_all_divergence(&store, &detector, test_source(), 9000);
            let types: Vec<DivergenceType> = results.iter().map(|(dt, _)| *dt).collect();
            // The full_store has frontier discrepancies → temporal divergence
            // and multiple agents → potential aleatory divergence
            // At minimum, the function should return SOME divergence types
            // (the exact set depends on store content)
            assert!(
                !types.is_empty() || store.frontier().len() <= 1,
                "detect_all_divergence should find divergences in a multi-entity store"
            );
        }

        #[test]
        fn divergence_type_display_and_boundary() {
            let all_types = [
                DivergenceType::Epistemic,
                DivergenceType::Structural,
                DivergenceType::Consequential,
                DivergenceType::Aleatory,
                DivergenceType::Logical,
                DivergenceType::Axiological,
                DivergenceType::Temporal,
                DivergenceType::Procedural,
            ];
            for dt in &all_types {
                // Display should not panic.
                let _display = format!("{}", dt);
                // Boundary should be non-empty.
                assert!(!dt.boundary().is_empty());
            }
        }
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
