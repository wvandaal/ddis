//! Context rendering, ACP methodology blocks, footer formatting, and reconciliation.
//!
//! This module contains the context assembly and rendering pipeline:
//!
//! - **GuidanceContext**: Assembled telemetry snapshot for adaptive guidance decisions.
//! - **Footer rendering**: Multi-level footer formatting (Full → BasinToken) for
//!   INV-GUIDANCE-001 continuous injection.
//! - **Task derivation**: Specification artifact → task derivation rules (INV-GUIDANCE-009).
//! - **Action derivation**: Store state analysis → prioritized guidance actions.
//! - **Spec relevance**: Proactive spec retrieval and reconciliation checks (PSR-1, CRB-6).
//! - **Contextual hints**: Command output → observation suggestions (INV-GUIDANCE-014).
//! - **Methodology section**: DMP section generation for AGENTS.md injection.

use std::collections::{BTreeMap, BTreeSet};

use crate::budget::{quality_adjusted_budget, GuidanceLevel};
use crate::datom::{Attribute, EntityId, Op, Value};
use crate::methodology::*;
use crate::routing::*;
use crate::store::Store;
use crate::trilateral::{check_coherence_fast, CoherenceQuadrant};

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
// Block presentation count (UAQ-3)
// ---------------------------------------------------------------------------

/// Look up the presentation count for a named context block type.
///
/// Queries `:attention/block-label` → `:attention/presentation-count` from the store.
/// Returns 0 if no attention entity exists for this label (novel block).
fn block_presentation_count(store: &Store, label: &str) -> u64 {
    let label_attr = Attribute::from_keyword(":attention/block-label");
    let count_attr = Attribute::from_keyword(":attention/presentation-count");

    // Find the attention entity for this label
    store
        .attribute_datoms(&label_attr)
        .iter()
        .filter(|d| d.op == Op::Assert)
        .find(|d| matches!(&d.value, Value::String(s) if s == label))
        .and_then(|d| {
            // Found the entity — now get its presentation count
            store
                .entity_datoms(d.entity)
                .iter()
                .find(|ed| ed.attribute == count_attr && ed.op == Op::Assert)
                .and_then(|ed| match &ed.value {
                    Value::Long(n) => Some(*n as u64),
                    _ => None,
                })
        })
        .unwrap_or(0)
}

/// Generate datoms to record presentation of context blocks (UAQ-6).
///
/// For each presented block label, either creates a new attention entity with
/// count=1, or increments the existing entity's count. Omitted blocks are NOT
/// recorded (their novelty is preserved).
///
/// Returns datoms to be transacted. Caller decides when/how to transact.
pub fn record_block_presentations(
    store: &Store,
    presented_labels: &[&str],
    tx: crate::datom::TxId,
) -> Vec<crate::datom::Datom> {
    use crate::datom::{Datom, EntityId};

    let label_attr = Attribute::from_keyword(":attention/block-label");
    let count_attr = Attribute::from_keyword(":attention/presentation-count");
    let last_attr = Attribute::from_keyword(":attention/last-presented");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut datoms = Vec::new();

    for label in presented_labels {
        // Find existing attention entity for this label
        let existing = store
            .attribute_datoms(&label_attr)
            .iter()
            .filter(|d| d.op == Op::Assert)
            .find(|d| matches!(&d.value, Value::String(s) if s.as_str() == *label))
            .map(|d| {
                // Get LIVE count: the last Assert not superseded by a Retract.
                // After ACP-FIX-1 retractions, old Assert datoms remain in the store.
                // Using .filter().last() gets the most recent Assert (insertion order).
                let count = store
                    .entity_datoms(d.entity)
                    .iter()
                    .filter(|ed| ed.attribute == count_attr && ed.op == Op::Assert)
                    .last()
                    .and_then(|ed| match &ed.value {
                        Value::Long(n) => Some(*n),
                        _ => None,
                    })
                    .unwrap_or(0);
                (d.entity, count)
            });

        let (entity, old_count, old_last) = match existing {
            Some((e, count)) => {
                // Read existing last-presented for retraction
                // Get LIVE last-presented (same pattern: last Assert wins)
                let last = store
                    .entity_datoms(e)
                    .iter()
                    .filter(|ed| ed.attribute == last_attr && ed.op == Op::Assert)
                    .last()
                    .and_then(|ed| match &ed.value {
                        Value::Instant(ts) => Some(*ts),
                        _ => None,
                    });
                (e, Some(count), last)
            }
            None => {
                // Create new attention entity
                let e = EntityId::from_ident(&format!(":attention/{}", label));
                datoms.push(Datom::new(
                    e,
                    label_attr.clone(),
                    Value::String(label.to_string()),
                    tx,
                    Op::Assert,
                ));
                (e, None, None)
            }
        };

        let new_count = old_count.map_or(1, |c| c + 1);

        // Retract old count before asserting new (C1: retractions are new datoms)
        if let Some(old) = old_count {
            datoms.push(Datom::new(
                entity,
                count_attr.clone(),
                Value::Long(old),
                tx,
                Op::Retract,
            ));
        }
        datoms.push(Datom::new(
            entity,
            count_attr.clone(),
            Value::Long(new_count),
            tx,
            Op::Assert,
        ));

        // Retract old timestamp before asserting new
        if let Some(old_ts) = old_last {
            datoms.push(Datom::new(
                entity,
                last_attr.clone(),
                Value::Instant(old_ts),
                tx,
                Op::Retract,
            ));
        }
        datoms.push(Datom::new(
            entity,
            last_attr.clone(),
            Value::Instant(now),
            tx,
            Op::Assert,
        ));
    }

    datoms
}

/// Extract canonical labels from context blocks that were presented (not omitted).
///
/// Uses a simple heuristic: the first word of the content before ':' or space,
/// lowercased. This matches the block labels used in `methodology_context_blocks`.
pub fn extract_block_labels(blocks: &[crate::budget::ContextBlock], budget: usize) -> Vec<String> {
    let mut remaining = budget;
    let mut labels = Vec::new();

    for block in blocks {
        if block.tokens <= remaining {
            remaining = remaining.saturating_sub(block.tokens);
            // Extract label: content prefix before ':' or first word
            let label = block
                .content
                .split_once(':')
                .map(|(prefix, _)| prefix.trim().to_lowercase())
                .unwrap_or_else(|| {
                    block
                        .content
                        .split_whitespace()
                        .next()
                        .unwrap_or("unknown")
                        .to_lowercase()
                });
            labels.push(label);
        }
    }

    labels
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
/// Build methodology context blocks, reusing a pre-computed `CalibrationReport`.
///
/// When the caller has already computed calibration (e.g., via
/// [`compute_routing_with_calibration`]), pass it here to skip the redundant
/// O(H*K) hypothesis scan. Passing `None` computes calibration on demand.
pub fn methodology_context_blocks_with_calibration(
    store: &Store,
    calibration: Option<&CalibrationReport>,
) -> Vec<crate::budget::ContextBlock> {
    methodology_context_blocks_inner(store, calibration)
}

/// Build methodology Context blocks for ACP projections (ACP-9).
///
/// Convenience wrapper that computes calibration on demand. For callers
/// that also compute routing, prefer [`methodology_context_blocks_with_calibration`]
/// to avoid redundant hypothesis scans (ACP-DRY-2).
pub fn methodology_context_blocks(store: &Store) -> Vec<crate::budget::ContextBlock> {
    methodology_context_blocks_inner(store, None)
}

fn methodology_context_blocks_inner(
    store: &Store,
    precomputed_calibration: Option<&CalibrationReport>,
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
        check(
            "spec-lang",
            score.components.spec_language_ratio,
            0.4,
            "query --entity :spec/..."
        ),
        check("q-div", score.components.query_diversity, 0.4, "query"),
        check("harvest", score.components.harvest_quality, 0.4, "harvest"),
    );

    // UAQ-3/UAQ-5: Score methodology blocks with AcquisitionScore.
    // Each block gets a canonical label for presentation-count tracking.
    // UAQ-5: Use per-type calibration for confidence factor.
    // ACP-DRY-2: Reuse pre-computed calibration when available.
    let owned_calibration;
    let calibration = match precomputed_calibration {
        Some(c) => c,
        None => {
            owned_calibration = compute_calibration_metrics(store);
            &owned_calibration
        }
    };
    let block_confidence = calibration
        .per_type_accuracy
        .get("block")
        .map(|e| (1.0 - e).clamp(0.1, 1.0))
        .unwrap_or_else(|| {
            if calibration.completed_hypotheses >= 5 {
                (1.0 - calibration.mean_error).clamp(0.1, 1.0)
            } else {
                1.0
            }
        });
    let score_block = |label: &str, impact: f64, tokens: usize| -> Option<crate::budget::AcquisitionScore> {
        let count = block_presentation_count(store, label);
        let novelty = crate::budget::novelty_from_count(count);
        Some(crate::budget::AcquisitionScore::from_factors(
            crate::budget::ObservationKind::ContextBlock,
            impact,
            1.0, // methodology blocks always relevant
            novelty,
            block_confidence,
            crate::budget::ObservationCost::from_tokens(tokens),
        ))
    };

    let mut blocks = vec![crate::budget::ContextBlock {
        precedence: crate::budget::OutputPrecedence::Methodology,
        content: m_line,
        tokens: 20,
        attention: score_block("methodology", 0.8, 20),
    }];

    // Store state context
    blocks.push(crate::budget::ContextBlock {
        precedence: crate::budget::OutputPrecedence::Ambient,
        content: format!(
            "Store: {} datoms | Turn {}",
            store.len(),
            store.frontier().len()
        ),
        tokens: 8,
        attention: score_block("store-state", 0.3, 8),
    });

    blocks
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
pub(crate) fn worst_metric_command(
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
            command: Some("braid status --verbose".into()),
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

    // AR-3: Spec graph neighbors — bridge the lexical gap.
    // Parse spec refs from input text and find graph-connected entities.
    let spec_refs = crate::task::parse_spec_refs(text);
    if !spec_refs.is_empty() {
        let neighbors = spec_graph_neighbors(store, &spec_refs);
        let ident_attr = Attribute::from_keyword(":db/ident");
        let task_id_attr = Attribute::from_keyword(":task/id");

        // Build a set of entities already found by keyword search
        let mut existing_entities: BTreeSet<EntityId> = BTreeSet::new();
        for r in &results {
            // Resolve ident back to entity for dedup
            if r.source == "task" {
                // Task idents are ":task/{id}" — look up entity from :task/id
                let id_part = r.ident.strip_prefix(":task/").unwrap_or(&r.ident);
                let task_entity = EntityId::from_ident(&format!(":task/{}", id_part));
                existing_entities.insert(task_entity);
            } else {
                let entity = EntityId::from_ident(&r.ident);
                existing_entities.insert(entity);
            }
        }

        for (entity, graph_score) in neighbors {
            if existing_entities.contains(&entity) {
                // Merge: upgrade score if graph_score is higher
                for r in results.iter_mut() {
                    let r_entity = if r.source == "task" {
                        let id_part = r.ident.strip_prefix(":task/").unwrap_or(&r.ident);
                        EntityId::from_ident(&format!(":task/{}", id_part))
                    } else {
                        EntityId::from_ident(&r.ident)
                    };
                    if r_entity == entity && graph_score > r.score {
                        r.score = graph_score;
                    }
                }
                continue;
            }

            // New entity from graph — resolve its identity for display
            let entity_datoms_list = store.entity_datoms(entity);

            // Try :task/id first (task entity), then :db/ident (spec entity)
            let (ident, human_id, source, summary) =
                if let Some(tid) = entity_datoms_list.iter().find(|d| {
                    d.attribute == task_id_attr && d.op == Op::Assert
                }) {
                    let id = match &tid.value {
                        Value::String(s) => s.clone(),
                        _ => format!("{:?}", entity),
                    };
                    let title = store
                        .live_value(entity, &Attribute::from_keyword(":task/title"))
                        .and_then(|v| match v {
                            Value::String(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    let summary = crate::budget::safe_truncate_bytes(&title, 60).to_string();
                    (format!(":task/{}", id), id, "task".to_string(), summary)
                } else if let Some(id_datom) = entity_datoms_list.iter().find(|d| {
                    d.attribute == ident_attr && d.op == Op::Assert
                }) {
                    let ident_str = match &id_datom.value {
                        Value::Keyword(k) => k.clone(),
                        _ => format!("{:?}", entity),
                    };
                    let human = crate::spec_id::SpecId::from_store_ident(&ident_str)
                        .map(|s| s.human_form())
                        .unwrap_or_else(|| ident_str.clone());
                    let stmt = store
                        .live_value(entity, &Attribute::from_keyword(":spec/statement"))
                        .and_then(|v| match v {
                            Value::String(s) => Some(s.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    let summary = crate::budget::safe_truncate_bytes(&stmt, 60).to_string();
                    (ident_str.clone(), human, "spec".to_string(), summary)
                } else {
                    continue; // Skip entities without identifiable metadata
                };

            results.push(SpecRelevance {
                ident,
                human_id,
                summary,
                score: graph_score,
                source: format!("{}+graph", source),
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
// Spec Graph Neighbors (AR-3) — VAET reverse-ref BFS
// ---------------------------------------------------------------------------

/// Extract the spec namespace from a spec ref ID (e.g., "INV-TOPOLOGY-001" -> "TOPOLOGY").
///
/// Splits on '-' and returns the middle part. For malformed IDs, returns the
/// original string.
///
/// # Examples
///
/// ```
/// use braid_kernel::guidance::extract_spec_namespace;
/// assert_eq!(extract_spec_namespace("INV-TOPOLOGY-001"), "TOPOLOGY");
/// assert_eq!(extract_spec_namespace("ADR-STORE-003"), "STORE");
/// assert_eq!(extract_spec_namespace("NEG-MERGE-002"), "MERGE");
/// assert_eq!(extract_spec_namespace("malformed"), "malformed");
/// ```
pub fn extract_spec_namespace(spec_ref: &str) -> &str {
    let parts: Vec<&str> = spec_ref.split('-').collect();
    if parts.len() >= 3 {
        parts[1]
    } else {
        spec_ref
    }
}

/// Find entities in the store that are connected to the given spec refs
/// via the spec dependency graph (VAET reverse-ref BFS, depth 2).
///
/// This bridges the lexical gap: entities that share no keywords but
/// trace to the same spec elements are discovered as neighbors.
///
/// IDF weighting: `1.0 / ln(2 + ref_count)` penalizes ubiquitous specs.
///
/// # Algorithm
///
/// 1. For each `spec_ref_id`, resolve to `EntityId` via `EntityId::from_ident(":spec/{id}")`.
/// 2. For each resolved spec entity, VAET reverse lookup to find entities
///    with `:task/traces-to` or `:impl/implements` pointing to it.
/// 3. Score = `1.0 / ln(2 + ref_count)` (IDF -- rare specs score higher).
/// 4. For 1-hop: follow `:spec/traces-to` from the spec entity to neighbor
///    spec entities, then VAET on those neighbors. Score *= 0.5 for 1-hop.
/// 5. Deduplicate by max score, cap at top 10.
pub fn spec_graph_neighbors(
    store: &Store,
    spec_ref_ids: &[String],
) -> Vec<(EntityId, f64)> {
    if spec_ref_ids.is_empty() {
        return Vec::new();
    }

    let traces_to_attr = Attribute::from_keyword(":task/traces-to");
    let implements_attr = Attribute::from_keyword(":impl/implements");
    let spec_traces_attr = Attribute::from_keyword(":spec/traces-to");

    // Accumulate scores per entity (max of all paths)
    let mut scores: BTreeMap<EntityId, f64> = BTreeMap::new();

    for spec_ref in spec_ref_ids {
        let ident = format!(":spec/{}", spec_ref.to_lowercase());
        let spec_entity = EntityId::from_ident(&ident);

        // 0-hop: entities referencing this spec entity directly
        let referencing = store.vaet_referencing(spec_entity);
        let ref_count = referencing
            .iter()
            .filter(|d| {
                d.op == Op::Assert
                    && (d.attribute == traces_to_attr || d.attribute == implements_attr)
            })
            .count();

        // IDF score: rare specs weight higher
        let idf = 1.0 / (2.0 + ref_count as f64).ln();

        for datom in referencing {
            if datom.op != Op::Assert {
                continue;
            }
            if datom.attribute == traces_to_attr || datom.attribute == implements_attr {
                let entry = scores.entry(datom.entity).or_insert(0.0);
                if idf > *entry {
                    *entry = idf;
                }
            }
        }

        // 1-hop: follow :spec/traces-to from spec entity to neighbor specs,
        // then find entities referencing those neighbors
        for spec_datom in store.entity_datoms(spec_entity) {
            if spec_datom.op != Op::Assert || spec_datom.attribute != spec_traces_attr {
                continue;
            }
            let neighbor_spec = match &spec_datom.value {
                Value::Ref(target) => *target,
                _ => continue,
            };

            let neighbor_refs = store.vaet_referencing(neighbor_spec);
            let neighbor_count = neighbor_refs
                .iter()
                .filter(|d| {
                    d.op == Op::Assert
                        && (d.attribute == traces_to_attr || d.attribute == implements_attr)
                })
                .count();

            let neighbor_idf = 0.5 / (2.0 + neighbor_count as f64).ln();

            for datom in neighbor_refs {
                if datom.op != Op::Assert {
                    continue;
                }
                if datom.attribute == traces_to_attr || datom.attribute == implements_attr {
                    let entry = scores.entry(datom.entity).or_insert(0.0);
                    if neighbor_idf > *entry {
                        *entry = neighbor_idf;
                    }
                }
            }
        }
    }

    // Sort by score descending, cap at top 10
    let mut results: Vec<(EntityId, f64)> = scores.into_iter().collect();
    results.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(10);
    results
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
        for cs in &gaps.concentration {
            out.push_str(&format!(
                "- concentration: {} traces in {} \u{2014} {}\n",
                cs.trace_count, cs.neighborhood, cs.suggestion
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
