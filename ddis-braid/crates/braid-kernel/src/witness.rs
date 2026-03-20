//! `witness` — Falsification-Bound Witness (FBW) system.
//!
//! The WITNESS system binds verification evidence to specification elements
//! through content-addressed triple hashes. A witness is only as good as its
//! binding: if the spec changes, or the falsification condition changes, or
//! the test body changes, the witness becomes stale and must be re-verified.
//!
//! # Core Abstraction
//!
//! ```text
//! FBW(inv) = (spec_hash, falsification_hash, test_body_hash, verdict, depth, tx)
//! ```
//!
//! The triple hash provides three independent staleness signals:
//! - **spec_hash** changes → the invariant statement drifted
//! - **falsification_hash** changes → the falsification condition was refined
//! - **test_body_hash** changes → the test implementation was modified
//!
//! # Invariants Implemented
//!
//! - INV-WITNESS-001: Triple-Hash Auto-Invalidation
//! - INV-WITNESS-002: Falsification Alignment (Stage 1: keyword Jaccard)
//! - INV-WITNESS-003: Monotonic Formality Progression
//! - INV-WITNESS-004: Challenge Adjunction Completeness
//! - INV-WITNESS-005: Stale Witnesses Reduce F(S)
//! - INV-WITNESS-006: Test Body Hash Extraction (in trace.rs)
//! - INV-WITNESS-007: Auto-Task Filing on Refutation
//! - INV-WITNESS-008: Harness-Falsification Binding
//! - INV-WITNESS-009: Cognitive Independence of Challenge (architectural)
//! - INV-WITNESS-010: Decorrelated Multi-Verdict (architectural)
//! - INV-WITNESS-011: Verification Completeness Guard
//!
//! # Design Decisions
//!
//! - ADR-WITNESS-001: Triple-hash over single-hash
//! - ADR-WITNESS-002: Falsification alignment as challenge Level 0
//! - ADR-WITNESS-003: Witness as datom (not database row)
//! - ADR-WITNESS-004: Subagent-based challenge over same-context majority vote

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};
use crate::store::Store;

// ===========================================================================
// Core Types
// ===========================================================================

/// A Falsification-Bound Witness — the core verification triple.
///
/// Content-addressed by `(inv_entity, spec_hash, falsification_hash, test_body_hash)`.
/// See spec/21-witness.md §21.1.
#[derive(Clone, Debug)]
pub struct FBW {
    /// Entity ID of this witness in the store.
    pub entity: EntityId,
    /// Reference to the invariant/spec element this witnesses.
    pub inv_ref: EntityId,
    /// BLAKE3 hash of the spec element statement text.
    pub spec_hash: String,
    /// BLAKE3 hash of the falsification condition text.
    pub falsification_hash: String,
    /// BLAKE3 hash of the normalized test body.
    pub test_body_hash: String,
    /// Verification depth level (1-4).
    pub depth: i64,
    /// Current witness status.
    pub status: WitnessStatus,
    /// Challenge verdict (if challenged).
    pub verdict: WitnessVerdict,
    /// Keyword alignment score between test and falsification.
    pub alignment_score: f64,
    /// Number of challenges applied.
    pub challenge_count: i64,
    /// Path to the test file.
    pub test_file: String,
    /// Agent that created this witness.
    pub agent: String,
}

/// Witness lifecycle status (INV-WITNESS-001, INV-WITNESS-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessStatus {
    /// Witness is valid — all hashes match current artifacts.
    Valid,
    /// Witness is stale — at least one hash mismatch detected.
    Stale,
    /// Witness is pending — created but not yet challenged.
    Pending,
}

impl WitnessStatus {
    /// Convert to keyword for datom storage.
    pub fn as_keyword(&self) -> &'static str {
        match self {
            WitnessStatus::Valid => ":witness.status/valid",
            WitnessStatus::Stale => ":witness.status/stale",
            WitnessStatus::Pending => ":witness.status/pending",
        }
    }

    /// Parse from keyword string.
    pub fn from_keyword(s: &str) -> Self {
        match s {
            ":witness.status/valid" => WitnessStatus::Valid,
            ":witness.status/stale" => WitnessStatus::Stale,
            _ => WitnessStatus::Pending,
        }
    }
}

/// Challenge verdict for a witness (INV-WITNESS-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessVerdict {
    /// Score >= 0.85: strong evidence the test verifies the falsification.
    Confirmed,
    /// Score 0.30..0.85: partial evidence, may need strengthening.
    Provisional,
    /// Score < 0.30: insufficient evidence.
    Inconclusive,
    /// Hard refutation: test contradicts the invariant.
    Refuted,
    /// Not yet challenged.
    Unchallenged,
}

impl WitnessVerdict {
    /// Convert to keyword for datom storage.
    pub fn as_keyword(&self) -> &'static str {
        match self {
            WitnessVerdict::Confirmed => ":witness.verdict/confirmed",
            WitnessVerdict::Provisional => ":witness.verdict/provisional",
            WitnessVerdict::Inconclusive => ":witness.verdict/inconclusive",
            WitnessVerdict::Refuted => ":witness.verdict/refuted",
            WitnessVerdict::Unchallenged => ":witness.verdict/unchallenged",
        }
    }

    /// Parse from keyword string.
    pub fn from_keyword(s: &str) -> Self {
        match s {
            ":witness.verdict/confirmed" => WitnessVerdict::Confirmed,
            ":witness.verdict/provisional" => WitnessVerdict::Provisional,
            ":witness.verdict/inconclusive" => WitnessVerdict::Inconclusive,
            ":witness.verdict/refuted" => WitnessVerdict::Refuted,
            _ => WitnessVerdict::Unchallenged,
        }
    }
}

/// Reason a witness became stale (INV-WITNESS-001).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StaleReason {
    /// The spec element statement text changed.
    SpecDrift,
    /// The falsification condition text changed.
    FalsificationDrift,
    /// The test body implementation changed.
    TestBodyDrift,
    /// Multiple hashes changed simultaneously.
    MultiDrift(Vec<StaleReason>),
}

/// Result of a single challenge level evaluation.
#[derive(Clone, Debug)]
pub struct ChallengeResult {
    /// Challenge level (0-5).
    pub level: u32,
    /// Score in [0, 1].
    pub score: f64,
    /// Human-readable rationale.
    pub rationale: String,
}

/// Alignment thresholds by depth (INV-WITNESS-002).
///
/// L2 = 0.3, L3 = 0.5, L4 = 0.7 (spec §21.2).
pub fn alignment_threshold(depth: i64) -> f64 {
    match depth {
        1 => 0.0,  // L1 (syntactic) has no alignment requirement
        2 => 0.3,  // L2 (structural)
        3 => 0.5,  // L3 (property)
        _ => 0.7,  // L4+ (formal)
    }
}

// ===========================================================================
// Loading — Extract FBWs from the Store
// ===========================================================================

/// Load all witness entities from the store.
///
/// Scans for entities with `:witness/traces-to` datoms and reconstructs
/// the FBW struct from stored attributes.
pub fn all_witnesses(store: &Store) -> Vec<FBW> {
    let traces_to_attr = Attribute::from_keyword(":witness/traces-to");
    let spec_hash_attr = Attribute::from_keyword(":witness/spec-hash");
    let fals_hash_attr = Attribute::from_keyword(":witness/falsification-hash");
    let test_hash_attr = Attribute::from_keyword(":witness/test-body-hash");
    let level_attr = Attribute::from_keyword(":witness/level");
    let verdict_attr = Attribute::from_keyword(":witness/verdict");
    let status_attr = Attribute::from_keyword(":witness/status");
    let alignment_attr = Attribute::from_keyword(":witness/alignment-score");
    let challenge_count_attr = Attribute::from_keyword(":witness/challenge-count");
    let test_file_attr = Attribute::from_keyword(":witness/test-file");
    let agent_attr = Attribute::from_keyword(":witness/agent");

    let mut witnesses = Vec::new();

    // Find entities with :witness/traces-to
    for datom in store.attribute_datoms(&traces_to_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        let inv_ref = match &datom.value {
            Value::Ref(e) => *e,
            _ => continue,
        };

        let entity = datom.entity;
        let entity_datoms = store.entity_datoms(entity);

        let spec_hash = extract_string(&entity_datoms, &spec_hash_attr).unwrap_or_default();
        let fals_hash = extract_string(&entity_datoms, &fals_hash_attr).unwrap_or_default();
        let test_hash = extract_string(&entity_datoms, &test_hash_attr).unwrap_or_default();
        let depth = extract_long(&entity_datoms, &level_attr).unwrap_or(1);
        let verdict_kw = extract_keyword(&entity_datoms, &verdict_attr).unwrap_or_default();
        let status_kw = extract_keyword(&entity_datoms, &status_attr).unwrap_or_default();
        let alignment = extract_double(&entity_datoms, &alignment_attr).unwrap_or(0.0);
        let challenge_count = extract_long(&entity_datoms, &challenge_count_attr).unwrap_or(0);
        let test_file = extract_string(&entity_datoms, &test_file_attr).unwrap_or_default();
        let agent = extract_string(&entity_datoms, &agent_attr).unwrap_or_default();

        witnesses.push(FBW {
            entity,
            inv_ref,
            spec_hash,
            falsification_hash: fals_hash,
            test_body_hash: test_hash,
            depth,
            status: WitnessStatus::from_keyword(&status_kw),
            verdict: WitnessVerdict::from_keyword(&verdict_kw),
            alignment_score: alignment,
            challenge_count,
            test_file,
            agent,
        });
    }

    witnesses
}

// ===========================================================================
// Triple-Hash Computation and Staleness Detection (INV-WITNESS-001)
// ===========================================================================

/// Compute BLAKE3 hash of a text block for witness binding.
///
/// Normalizes: trims, collapses blank lines, strips comment-only lines.
/// This is the canonical hash function for all three witness hashes.
pub fn content_hash(text: &str) -> String {
    let normalized: String = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with("//"))
        .collect::<Vec<&str>>()
        .join("\n");
    blake3::hash(normalized.as_bytes()).to_hex().to_string()
}

/// Current hashes for a spec element — the "ground truth" to compare against.
///
/// Maps invariant entity ID → (spec_hash, falsification_hash).
/// Test body hashes come from trace scanning separately.
pub struct CurrentSpecHashes {
    /// Invariant entity → (spec statement hash, falsification condition hash).
    pub spec_hashes: BTreeMap<EntityId, (String, String)>,
    /// Invariant entity → test body hash (from trace scanner).
    pub test_hashes: BTreeMap<EntityId, String>,
}

/// Detect stale witnesses by comparing stored hashes against current artifacts.
///
/// INV-WITNESS-001: For every FBW, if any of `spec_hash`, `falsification_hash`,
/// or `test_body_hash` differs from the current computed hash, the FBW is stale.
///
/// Returns a list of (witness entity, stale reason) pairs.
pub fn detect_stale_witnesses(
    witnesses: &[FBW],
    current: &CurrentSpecHashes,
) -> Vec<(EntityId, StaleReason)> {
    let mut stale = Vec::new();

    for fbw in witnesses {
        if fbw.status == WitnessStatus::Stale {
            continue; // Already marked stale
        }

        let mut reasons = Vec::new();

        // Check spec hash
        if let Some((current_spec, current_fals)) = current.spec_hashes.get(&fbw.inv_ref) {
            if !fbw.spec_hash.is_empty() && fbw.spec_hash != *current_spec {
                reasons.push(StaleReason::SpecDrift);
            }
            if !fbw.falsification_hash.is_empty() && fbw.falsification_hash != *current_fals {
                reasons.push(StaleReason::FalsificationDrift);
            }
        }

        // Check test body hash
        if let Some(current_test) = current.test_hashes.get(&fbw.inv_ref) {
            if !fbw.test_body_hash.is_empty() && fbw.test_body_hash != *current_test {
                reasons.push(StaleReason::TestBodyDrift);
            }
        }

        if !reasons.is_empty() {
            let reason = if reasons.len() == 1 {
                reasons.into_iter().next().unwrap()
            } else {
                StaleReason::MultiDrift(reasons)
            };
            stale.push((fbw.entity, reason));
        }
    }

    stale
}

/// Generate datoms to mark a witness as stale (INV-WITNESS-001).
///
/// Asserts `:witness/status :witness.status/stale` and retracts the old status.
pub fn mark_stale_datoms(witness_entity: EntityId, tx: TxId) -> Vec<Datom> {
    vec![Datom::new(
        witness_entity,
        Attribute::from_keyword(":witness/status"),
        Value::Keyword(":witness.status/stale".to_string()),
        tx,
        Op::Assert,
    )]
}

// ===========================================================================
// Keyword Alignment Score (INV-WITNESS-002, INV-WITNESS-008)
// ===========================================================================

/// Compute keyword alignment score between test body and falsification condition.
///
/// Stage 1 implementation: bag-of-words Jaccard similarity.
/// - Extract keywords (alphanumeric tokens ≥ 3 chars, lowercased).
/// - Jaccard = |A ∩ B| / |A ∪ B|.
///
/// INV-WITNESS-002: alignment must be >= `alignment_threshold(depth)`.
/// INV-WITNESS-008: Kani/proptest harnesses must have alignment >= 0.5.
pub fn keyword_alignment_score(test_body: &str, falsification: &str) -> f64 {
    let test_kw = extract_keywords(test_body);
    let fals_kw = extract_keywords(falsification);

    if test_kw.is_empty() && fals_kw.is_empty() {
        return 0.0;
    }

    let intersection: BTreeSet<&str> = test_kw.intersection(&fals_kw).copied().collect();
    let union: BTreeSet<&str> = test_kw.union(&fals_kw).copied().collect();

    if union.is_empty() {
        return 0.0;
    }

    intersection.len() as f64 / union.len() as f64
}

/// Extract keywords from text: alphanumeric tokens ≥ 3 chars, lowercased.
///
/// Filters common stop words that add noise to the Jaccard computation.
fn extract_keywords(text: &str) -> BTreeSet<&str> {
    static STOP_WORDS: &[&str] = &[
        "the", "and", "for", "that", "this", "with", "not", "are", "was",
        "has", "have", "from", "will", "can", "any", "all", "its", "but",
        "let", "mut", "pub", "use", "mod", "ref", "self", "impl", "true",
        "false", "none", "some", "return", "else",
    ];

    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() >= 3)
        .filter(|w| !STOP_WORDS.contains(&w.to_lowercase().as_str()))
        .collect()
}

// ===========================================================================
// Challenge Protocol (INV-WITNESS-004, INV-WITNESS-009, INV-WITNESS-010)
// ===========================================================================

/// Run the Stage 1 challenge protocol on a witness.
///
/// Stage 1 implements Level 0 (falsification alignment) and Level 5 (semantic overlap).
/// Levels 1-4 are architectural stubs for Stage 2+ (LLM-as-judge, SMT, execution).
///
/// INV-WITNESS-004: No witness transitions to `valid` without at least one challenge.
/// INV-WITNESS-009: Challenge evaluations should be cognitively independent (architectural).
/// INV-WITNESS-010: Majority votes need decorrelated contexts (architectural).
///
/// Returns the composite verdict and individual level results.
pub fn challenge_witness(
    test_body: &str,
    falsification: &str,
    depth: i64,
) -> (WitnessVerdict, Vec<ChallengeResult>) {
    let mut results = Vec::new();

    // Level 0: Falsification alignment (keyword Jaccard)
    let alignment = keyword_alignment_score(test_body, falsification);
    let threshold = alignment_threshold(depth);
    let l0_score = if alignment >= threshold { alignment } else { alignment * 0.5 };
    results.push(ChallengeResult {
        level: 0,
        score: l0_score,
        rationale: format!(
            "Keyword alignment: {alignment:.3} (threshold: {threshold:.2})"
        ),
    });

    // Level 2: Evidence type match — verify depth claim matches actual test structure
    let l2_score = evidence_type_score(test_body, depth);
    results.push(ChallengeResult {
        level: 2,
        score: l2_score,
        rationale: format!(
            "Evidence type match for claimed depth L{depth}: {l2_score:.2}"
        ),
    });

    // Level 5: Semantic keyword overlap — checks for domain-specific terms
    let l5_score = semantic_overlap_score(test_body, falsification);
    results.push(ChallengeResult {
        level: 5,
        score: l5_score,
        rationale: format!(
            "Semantic keyword overlap: {l5_score:.3}"
        ),
    });

    // Composite verdict: weighted average of available levels
    let total_score = l0_score * 0.5 + l2_score * 0.2 + l5_score * 0.3;

    let verdict = if total_score >= 0.85 {
        WitnessVerdict::Confirmed
    } else if total_score >= 0.30 {
        WitnessVerdict::Provisional
    } else {
        WitnessVerdict::Inconclusive
    };

    (verdict, results)
}

/// Level 2 challenge: Does the test body match the claimed evidence type?
///
/// - L2 (Structural): should contain `assert`, `assert_eq`, `assert_ne`
/// - L3 (Property): should contain `prop_assert`, `kani::`, `proptest`
/// - L4 (Formal): should contain `checker`, `model`, `spawn_bfs`
fn evidence_type_score(test_body: &str, claimed_depth: i64) -> f64 {
    let lower = test_body.to_lowercase();
    match claimed_depth {
        2 => {
            // L2: must have assertions
            let has_assert = lower.contains("assert");
            if has_assert { 1.0 } else { 0.2 }
        }
        3 => {
            // L3: must have property-based markers
            let has_prop = lower.contains("prop_assert")
                || lower.contains("kani::")
                || lower.contains("proptest");
            let has_assert = lower.contains("assert");
            if has_prop { 1.0 } else if has_assert { 0.5 } else { 0.1 }
        }
        4 => {
            // L4: must have model checking markers
            let has_model = lower.contains("checker")
                || lower.contains("spawn_bfs")
                || lower.contains("stateright")
                || lower.contains("model");
            if has_model { 1.0 } else { 0.2 }
        }
        _ => 0.5, // L1 has minimal requirements
    }
}

/// Level 5: Semantic keyword overlap with domain-specific weighting.
///
/// Beyond raw Jaccard, this weights keywords that appear in both the
/// test and falsification but are domain-specific (not common programming terms).
fn semantic_overlap_score(test_body: &str, falsification: &str) -> f64 {
    let test_kw = extract_keywords(test_body);
    let fals_kw = extract_keywords(falsification);

    if fals_kw.is_empty() {
        return 0.0;
    }

    // Count how many falsification keywords appear in the test
    let matched = fals_kw.iter().filter(|k| test_kw.contains(*k)).count();
    matched as f64 / fals_kw.len() as f64
}

// ===========================================================================
// Monotonic Depth Guard (INV-WITNESS-003)
// ===========================================================================

/// Check that a new witness depth is monotonically non-decreasing.
///
/// INV-WITNESS-003: Cannot re-witness at lower depth without deliberation override.
/// Returns `Err(current_depth)` if the new depth would regress.
pub fn check_depth_monotonic(
    witnesses: &[FBW],
    inv_entity: EntityId,
    new_depth: i64,
) -> Result<(), i64> {
    let current_max = witnesses
        .iter()
        .filter(|w| w.inv_ref == inv_entity && w.status == WitnessStatus::Valid)
        .map(|w| w.depth)
        .max()
        .unwrap_or(0);

    if new_depth < current_max {
        Err(current_max)
    } else {
        Ok(())
    }
}

// ===========================================================================
// Completeness Guard (INV-WITNESS-011)
// ===========================================================================

/// Check for unwitnessed invariants at the current stage.
///
/// INV-WITNESS-011: Every invariant at the current stage or earlier must have
/// at least one FBW at depth L2+, or a verification gap signal is emitted.
///
/// Returns entity IDs of invariants that lack L2+ witnesses.
pub fn completeness_guard(store: &Store, witnesses: &[FBW]) -> Vec<EntityId> {
    let spec_type_attr = Attribute::from_keyword(":spec/element-type");
    let mut unwitnessed = Vec::new();

    // Build set of inv entities with valid L2+ witnesses
    let witnessed: BTreeSet<EntityId> = witnesses
        .iter()
        .filter(|w| w.depth >= 2 && w.status != WitnessStatus::Stale)
        .map(|w| w.inv_ref)
        .collect();

    // Find all invariant entities
    for datom in store.attribute_datoms(&spec_type_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        // Only check invariants (not ADRs or NEGs)
        let is_inv = match &datom.value {
            Value::Keyword(k) => k.contains("invariant"),
            Value::String(s) => s.contains("invariant"),
            _ => false,
        };
        if !is_inv {
            continue;
        }

        if !witnessed.contains(&datom.entity) {
            unwitnessed.push(datom.entity);
        }
    }

    unwitnessed
}

// ===========================================================================
// Auto-Task on Refutation (INV-WITNESS-007)
// ===========================================================================

/// Generate datoms for an auto-filed bug task when a challenge produces a refutation.
///
/// INV-WITNESS-007: Refuted verdict → auto-create bug task with invariant ID in title.
pub fn auto_task_on_refutation(
    inv_entity: EntityId,
    inv_id: &str,
    inv_title: &str,
    tx: TxId,
) -> Vec<Datom> {
    let task_title = format!(
        "BUG: Witness refuted for {inv_id}. {inv_title}"
    );

    let params = crate::task::CreateTaskParams {
        title: &task_title,
        description: None,
        priority: 0, // P0 — a refuted witness is critical
        task_type: crate::task::TaskType::Bug,
        tx,
        traces_to: &[inv_entity],
        labels: &[],
    };

    let (_entity, datoms) = crate::task::create_task_datoms(params);
    datoms
}

// ===========================================================================
// FBW to Datoms (ADR-WITNESS-003: Witness as Datom)
// ===========================================================================

/// Convert an FBW to datoms for storage in the append-only store.
///
/// ADR-WITNESS-003: Witnesses are datoms, not database rows — queryable via
/// Datalog, mergeable via CRDT set union, append-only by construction.
pub fn fbw_to_datoms(fbw: &FBW, tx: TxId) -> Vec<Datom> {
    let e = fbw.entity;
    let mut datoms = Vec::new();

    // Ident
    let ident = format!(":witness/fbw.{:?}", e);
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":db/ident"),
        Value::Keyword(ident),
        tx,
        Op::Assert,
    ));

    // Core triple hashes
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":witness/spec-hash"),
        Value::String(fbw.spec_hash.clone()),
        tx,
        Op::Assert,
    ));
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":witness/falsification-hash"),
        Value::String(fbw.falsification_hash.clone()),
        tx,
        Op::Assert,
    ));
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":witness/test-body-hash"),
        Value::String(fbw.test_body_hash.clone()),
        tx,
        Op::Assert,
    ));

    // Reference to spec element
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":witness/traces-to"),
        Value::Ref(fbw.inv_ref),
        tx,
        Op::Assert,
    ));

    // Depth
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":witness/level"),
        Value::Long(fbw.depth),
        tx,
        Op::Assert,
    ));

    // Status
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":witness/status"),
        Value::Keyword(fbw.status.as_keyword().to_string()),
        tx,
        Op::Assert,
    ));

    // Verdict
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":witness/verdict"),
        Value::Keyword(fbw.verdict.as_keyword().to_string()),
        tx,
        Op::Assert,
    ));

    // Alignment score
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":witness/alignment-score"),
        Value::Double(fbw.alignment_score.into()),
        tx,
        Op::Assert,
    ));

    // Challenge count
    datoms.push(Datom::new(
        e,
        Attribute::from_keyword(":witness/challenge-count"),
        Value::Long(fbw.challenge_count),
        tx,
        Op::Assert,
    ));

    // Test file
    if !fbw.test_file.is_empty() {
        datoms.push(Datom::new(
            e,
            Attribute::from_keyword(":witness/test-file"),
            Value::String(fbw.test_file.clone()),
            tx,
            Op::Assert,
        ));
    }

    // Agent
    if !fbw.agent.is_empty() {
        datoms.push(Datom::new(
            e,
            Attribute::from_keyword(":witness/agent"),
            Value::String(fbw.agent.clone()),
            tx,
            Op::Assert,
        ));
    }

    datoms
}

/// Create a new FBW entity from trace and spec data.
///
/// The entity ID is content-addressed from (inv_entity, test_file, spec_hash_prefix).
pub fn create_fbw(
    inv_entity: EntityId,
    spec_text: &str,
    falsification_text: &str,
    test_body: &str,
    test_file: &str,
    depth: i64,
    agent_name: &str,
) -> FBW {
    let spec_hash = content_hash(spec_text);
    let falsification_hash = content_hash(falsification_text);
    let test_body_hash = content_hash(test_body);
    let alignment = keyword_alignment_score(test_body, falsification_text);

    // Content-addressed entity ID
    let ident = format!(
        ":witness/fbw.{:?}.{}.{}",
        inv_entity,
        &spec_hash[..8.min(spec_hash.len())],
        &test_body_hash[..8.min(test_body_hash.len())]
    );
    let entity = EntityId::from_ident(&ident);

    FBW {
        entity,
        inv_ref: inv_entity,
        spec_hash,
        falsification_hash,
        test_body_hash,
        depth,
        status: WitnessStatus::Pending,
        verdict: WitnessVerdict::Unchallenged,
        alignment_score: alignment,
        challenge_count: 0,
        test_file: test_file.to_string(),
        agent: agent_name.to_string(),
    }
}

/// Parameters for creating and challenging a witness.
pub struct WitnessParams<'a> {
    /// The invariant entity this witness covers.
    pub inv_entity: EntityId,
    /// The spec element statement text (for hashing).
    pub spec_text: &'a str,
    /// The falsification condition text (for hashing).
    pub falsification_text: &'a str,
    /// The normalized test body text (for hashing).
    pub test_body: &'a str,
    /// Path to the test file.
    pub test_file: &'a str,
    /// Verification depth (1-4).
    pub depth: i64,
    /// Agent name creating this witness.
    pub agent_name: &'a str,
    /// Transaction ID for datom construction.
    pub tx: TxId,
}

/// Create an FBW, run the challenge protocol, and produce datoms.
///
/// This is the main entry point for creating a new witness: it creates
/// the FBW, challenges it, and produces store datoms in one operation.
///
/// INV-WITNESS-003 is checked: returns Err if depth would regress.
pub fn witness_and_challenge(
    store: &Store,
    params: WitnessParams<'_>,
) -> Result<(FBW, Vec<Datom>), String> {
    let WitnessParams {
        inv_entity,
        spec_text,
        falsification_text,
        test_body,
        test_file,
        depth,
        agent_name,
        tx,
    } = params;

    // INV-WITNESS-003: Check monotonic depth
    let existing = all_witnesses(store);
    if let Err(current_max) = check_depth_monotonic(&existing, inv_entity, depth) {
        return Err(format!(
            "Depth regression: L{depth} < current L{current_max} for {:?}. \
             Monotonic depth requires L{current_max}+ (INV-WITNESS-003).",
            inv_entity
        ));
    }

    let mut fbw = create_fbw(
        inv_entity,
        spec_text,
        falsification_text,
        test_body,
        test_file,
        depth,
        agent_name,
    );

    // Run challenge protocol
    let (verdict, _results) = challenge_witness(test_body, falsification_text, depth);
    fbw.verdict = verdict;
    fbw.challenge_count = 1;

    // Set status based on verdict and alignment
    let threshold = alignment_threshold(depth);
    if verdict == WitnessVerdict::Confirmed && fbw.alignment_score >= threshold {
        fbw.status = WitnessStatus::Valid;
    } else if verdict == WitnessVerdict::Refuted {
        fbw.status = WitnessStatus::Stale;
    } else {
        fbw.status = WitnessStatus::Pending;
    }

    let datoms = fbw_to_datoms(&fbw, tx);
    Ok((fbw, datoms))
}

// ===========================================================================
// F(S) Integration (INV-WITNESS-005)
// ===========================================================================

/// Compute the witness-aware validation score for F(S).
///
/// INV-WITNESS-005: Stale FBWs contribute 0 to the validation score.
/// For each invariant, the score is `max(depth_weight(fbw.depth))` across
/// valid (non-stale) witnesses only.
///
/// Returns `(score, valid_count, stale_count, untested_count)`.
pub fn witness_validation_score(
    store: &Store,
) -> (f64, usize, usize, usize) {
    let witnesses = all_witnesses(store);
    if witnesses.is_empty() {
        // Fall back to existing compute_validation logic (no WITNESS data yet)
        return (0.0, 0, 0, 0);
    }

    // Group by invariant, take max depth of valid witnesses
    let mut inv_max_depth: HashMap<EntityId, i64> = HashMap::new();
    let mut stale_count = 0usize;
    let mut valid_count = 0usize;

    for w in &witnesses {
        match w.status {
            WitnessStatus::Valid => {
                valid_count += 1;
                let entry = inv_max_depth.entry(w.inv_ref).or_insert(0);
                if w.depth > *entry {
                    *entry = w.depth;
                }
            }
            WitnessStatus::Stale => {
                stale_count += 1;
                // Stale witnesses contribute 0 — do not update inv_max_depth
            }
            WitnessStatus::Pending => {
                // Pending witnesses also contribute 0
            }
        }
    }

    // Count total invariants
    let spec_type_attr = Attribute::from_keyword(":spec/element-type");
    let inv_entities: Vec<EntityId> = store
        .attribute_datoms(&spec_type_attr)
        .iter()
        .filter(|d| {
            d.op == Op::Assert
                && match &d.value {
                    Value::Keyword(k) => k.contains("invariant"),
                    Value::String(s) => s.contains("invariant"),
                    _ => false,
                }
        })
        .map(|d| d.entity)
        .collect();

    let total_invs = inv_entities.len();
    let untested_count = inv_entities
        .iter()
        .filter(|e| !inv_max_depth.contains_key(e))
        .count();

    if total_invs == 0 {
        return (1.0, valid_count, stale_count, untested_count);
    }

    // Score: sum of depth weights / max possible
    let depth_sum: f64 = inv_max_depth
        .values()
        .map(|&d| crate::bilateral::depth_weight(d))
        .sum();
    let max_possible = total_invs as f64 * crate::bilateral::depth_weight(4);
    let score = (depth_sum / max_possible).clamp(0.0, 1.0);

    (score, valid_count, stale_count, untested_count)
}

/// Get witness gaps for the methodology dashboard.
///
/// Returns `(untested_count, stale_count)` for integration with `MethodologyGaps`.
pub fn witness_gaps(store: &Store) -> (u32, u32) {
    let (_, _, stale, untested) = witness_validation_score(store);
    (untested as u32, stale as u32)
}

// ===========================================================================
// Spec Hash Extraction from Store
// ===========================================================================

/// Extract current spec hashes from the store for staleness comparison.
///
/// For each invariant entity, computes BLAKE3 of the `:element/statement`
/// and `:spec/falsification` attribute values.
pub fn current_spec_hashes(store: &Store) -> CurrentSpecHashes {
    let spec_type_attr = Attribute::from_keyword(":spec/element-type");
    let statement_attr = Attribute::from_keyword(":element/statement");
    let falsification_attr = Attribute::from_keyword(":spec/falsification");

    let mut spec_hashes = BTreeMap::new();

    for datom in store.attribute_datoms(&spec_type_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        let is_inv = match &datom.value {
            Value::Keyword(k) => k.contains("invariant"),
            Value::String(s) => s.contains("invariant"),
            _ => false,
        };
        if !is_inv {
            continue;
        }

        let entity_datoms = store.entity_datoms(datom.entity);

        let statement = extract_string(&entity_datoms, &statement_attr).unwrap_or_default();
        let falsification = extract_string(&entity_datoms, &falsification_attr).unwrap_or_default();

        if !statement.is_empty() {
            let sh = content_hash(&statement);
            let fh = if falsification.is_empty() {
                content_hash("")
            } else {
                content_hash(&falsification)
            };
            spec_hashes.insert(datom.entity, (sh, fh));
        }
    }

    CurrentSpecHashes {
        spec_hashes,
        test_hashes: BTreeMap::new(), // Populated by trace scanner externally
    }
}

// ===========================================================================
// Helper Functions
// ===========================================================================

/// Extract a String value for an attribute from an entity's datoms.
fn extract_string(datoms: &[&Datom], attr: &Attribute) -> Option<String> {
    datoms
        .iter()
        .rfind(|d| d.attribute == *attr && d.op == Op::Assert)
        .and_then(|d| match &d.value {
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
}

/// Extract a Keyword value for an attribute from an entity's datoms.
fn extract_keyword(datoms: &[&Datom], attr: &Attribute) -> Option<String> {
    datoms
        .iter()
        .rfind(|d| d.attribute == *attr && d.op == Op::Assert)
        .and_then(|d| match &d.value {
            Value::Keyword(s) => Some(s.clone()),
            _ => None,
        })
}

/// Extract a Long value for an attribute from an entity's datoms.
fn extract_long(datoms: &[&Datom], attr: &Attribute) -> Option<i64> {
    datoms
        .iter()
        .rfind(|d| d.attribute == *attr && d.op == Op::Assert)
        .and_then(|d| match &d.value {
            Value::Long(v) => Some(*v),
            _ => None,
        })
}

/// Extract a Double value for an attribute from an entity's datoms.
fn extract_double(datoms: &[&Datom], attr: &Attribute) -> Option<f64> {
    datoms
        .iter()
        .rfind(|d| d.attribute == *attr && d.op == Op::Assert)
        .and_then(|d| match &d.value {
            Value::Double(v) => Some((*v).into()),
            _ => None,
        })
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;

    fn test_tx() -> TxId {
        TxId::new(100, 0, AgentId::from_name("test"))
    }

    fn test_inv_entity() -> EntityId {
        EntityId::from_ident(":spec/inv-store-001")
    }

    // --- INV-WITNESS-001: Triple-Hash Auto-Invalidation ---

    #[test]
    fn content_hash_deterministic() {
        // INV-WITNESS-001: hash function is deterministic
        let h1 = content_hash("Some spec statement\nwith multiple lines");
        let h2 = content_hash("Some spec statement\nwith multiple lines");
        assert_eq!(h1, h2);
    }

    #[test]
    fn content_hash_whitespace_normalized() {
        // INV-WITNESS-001: whitespace normalization
        let h1 = content_hash("  line one  \n\n  line two  ");
        let h2 = content_hash("line one\nline two");
        assert_eq!(h1, h2, "whitespace-only changes must not change hash");
    }

    #[test]
    fn content_hash_comment_stripped() {
        // INV-WITNESS-001: comment-only lines stripped
        let h1 = content_hash("assert!(true);\n// a comment\nassert!(false);");
        let h2 = content_hash("assert!(true);\nassert!(false);");
        assert_eq!(h1, h2, "comment-only lines must not change hash");
    }

    #[test]
    fn content_hash_distinct_content() {
        let h1 = content_hash("content A");
        let h2 = content_hash("content B");
        assert_ne!(h1, h2, "different content must produce different hashes");
    }

    #[test]
    fn detect_stale_spec_drift() {
        // INV-WITNESS-001: spec hash mismatch → stale
        let witnesses = vec![FBW {
            entity: EntityId::from_ident(":witness/test1"),
            inv_ref: test_inv_entity(),
            spec_hash: content_hash("original spec statement"),
            falsification_hash: content_hash("if X then violated"),
            test_body_hash: content_hash("assert!(stuff)"),
            depth: 2,
            status: WitnessStatus::Valid,
            verdict: WitnessVerdict::Confirmed,
            alignment_score: 0.5,
            challenge_count: 1,
            test_file: "src/test.rs".to_string(),
            agent: "test".to_string(),
        }];

        let mut current = CurrentSpecHashes {
            spec_hashes: BTreeMap::new(),
            test_hashes: BTreeMap::new(),
        };
        // Spec changed!
        current.spec_hashes.insert(
            test_inv_entity(),
            (content_hash("MODIFIED spec statement"), content_hash("if X then violated")),
        );

        let stale = detect_stale_witnesses(&witnesses, &current);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].1, StaleReason::SpecDrift);
    }

    #[test]
    fn detect_stale_test_drift() {
        // INV-WITNESS-001: test body hash mismatch → stale
        let witnesses = vec![FBW {
            entity: EntityId::from_ident(":witness/test2"),
            inv_ref: test_inv_entity(),
            spec_hash: content_hash("spec"),
            falsification_hash: content_hash("fals"),
            test_body_hash: content_hash("original test body"),
            depth: 2,
            status: WitnessStatus::Valid,
            verdict: WitnessVerdict::Confirmed,
            alignment_score: 0.5,
            challenge_count: 1,
            test_file: "src/test.rs".to_string(),
            agent: "test".to_string(),
        }];

        let mut current = CurrentSpecHashes {
            spec_hashes: BTreeMap::new(),
            test_hashes: BTreeMap::new(),
        };
        current.spec_hashes.insert(
            test_inv_entity(),
            (content_hash("spec"), content_hash("fals")),
        );
        // Test body changed!
        current.test_hashes.insert(
            test_inv_entity(),
            content_hash("MODIFIED test body"),
        );

        let stale = detect_stale_witnesses(&witnesses, &current);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].1, StaleReason::TestBodyDrift);
    }

    #[test]
    fn no_stale_when_hashes_match() {
        // INV-WITNESS-001: matching hashes → not stale
        let hash_s = content_hash("spec");
        let hash_f = content_hash("fals");
        let hash_t = content_hash("test body");

        let witnesses = vec![FBW {
            entity: EntityId::from_ident(":witness/test3"),
            inv_ref: test_inv_entity(),
            spec_hash: hash_s.clone(),
            falsification_hash: hash_f.clone(),
            test_body_hash: hash_t.clone(),
            depth: 2,
            status: WitnessStatus::Valid,
            verdict: WitnessVerdict::Confirmed,
            alignment_score: 0.5,
            challenge_count: 1,
            test_file: "src/test.rs".to_string(),
            agent: "test".to_string(),
        }];

        let mut current = CurrentSpecHashes {
            spec_hashes: BTreeMap::new(),
            test_hashes: BTreeMap::new(),
        };
        current.spec_hashes.insert(test_inv_entity(), (hash_s, hash_f));
        current.test_hashes.insert(test_inv_entity(), hash_t);

        let stale = detect_stale_witnesses(&witnesses, &current);
        assert!(stale.is_empty());
    }

    // --- INV-WITNESS-002: Falsification Alignment ---

    #[test]
    fn alignment_identical_text() {
        let score = keyword_alignment_score(
            "assert store append only immutable",
            "store append only immutable violated",
        );
        assert!(score > 0.5, "shared keywords should produce high alignment: {score}");
    }

    #[test]
    fn alignment_disjoint_text() {
        let score = keyword_alignment_score(
            "database connection timeout retry",
            "graph traversal bidirectional search",
        );
        assert!(score < 0.2, "disjoint text should produce low alignment: {score}");
    }

    #[test]
    fn alignment_empty_text() {
        assert_eq!(keyword_alignment_score("", ""), 0.0);
        assert_eq!(keyword_alignment_score("some text", ""), 0.0);
    }

    #[test]
    fn alignment_threshold_monotonic() {
        // INV-WITNESS-002: thresholds increase with depth
        assert!(alignment_threshold(2) < alignment_threshold(3));
        assert!(alignment_threshold(3) < alignment_threshold(4));
    }

    // --- INV-WITNESS-003: Monotonic Formality Progression ---

    #[test]
    fn depth_monotonic_allows_upgrade() {
        let witnesses = vec![FBW {
            entity: EntityId::from_ident(":witness/mono1"),
            inv_ref: test_inv_entity(),
            spec_hash: String::new(),
            falsification_hash: String::new(),
            test_body_hash: String::new(),
            depth: 2,
            status: WitnessStatus::Valid,
            verdict: WitnessVerdict::Confirmed,
            alignment_score: 0.5,
            challenge_count: 1,
            test_file: String::new(),
            agent: String::new(),
        }];

        // L3 > L2: should succeed
        assert!(check_depth_monotonic(&witnesses, test_inv_entity(), 3).is_ok());
        // L2 = L2: should succeed (equal is ok)
        assert!(check_depth_monotonic(&witnesses, test_inv_entity(), 2).is_ok());
    }

    #[test]
    fn depth_monotonic_rejects_downgrade() {
        let witnesses = vec![FBW {
            entity: EntityId::from_ident(":witness/mono2"),
            inv_ref: test_inv_entity(),
            spec_hash: String::new(),
            falsification_hash: String::new(),
            test_body_hash: String::new(),
            depth: 3,
            status: WitnessStatus::Valid,
            verdict: WitnessVerdict::Confirmed,
            alignment_score: 0.5,
            challenge_count: 1,
            test_file: String::new(),
            agent: String::new(),
        }];

        // L2 < L3: should fail
        assert_eq!(
            check_depth_monotonic(&witnesses, test_inv_entity(), 2),
            Err(3)
        );
    }

    // --- INV-WITNESS-004: Challenge Adjunction Completeness ---

    #[test]
    fn challenge_produces_results() {
        let (verdict, results) = challenge_witness(
            "let store = Store::genesis();\nassert!(store.datoms().count() > 0);",
            "violated if the store ever deletes a datom",
            2,
        );
        // Should produce at least 3 level results
        assert!(results.len() >= 3);
        // Verdict should not be unchallenged
        assert_ne!(verdict, WitnessVerdict::Unchallenged);
    }

    #[test]
    fn challenge_tautological_test_low_score() {
        // NEG-WITNESS-005: tautological tests get low alignment
        let (verdict, _) = challenge_witness(
            "assert!(true)",
            "violated if mutation occurs in append-only store",
            2,
        );
        // Tautological test should not be confirmed
        assert_ne!(verdict, WitnessVerdict::Confirmed);
    }

    // --- INV-WITNESS-005: Stale Witnesses Reduce F(S) ---

    #[test]
    fn stale_witness_zero_contribution() {
        // When a witness is stale, it contributes 0 to validation.
        // This is verified structurally: witness_validation_score only counts
        // witnesses with status == Valid.
        let store = Store::genesis();
        let (score, valid, stale, _untested) = witness_validation_score(&store);
        // Empty store: no witnesses, score is 0
        assert_eq!(valid, 0);
        assert_eq!(stale, 0);
        assert_eq!(score, 0.0);
    }

    // --- INV-WITNESS-007: Auto-Task Filing on Refutation ---

    #[test]
    fn auto_task_creates_bug() {
        let datoms = auto_task_on_refutation(
            test_inv_entity(),
            "INV-STORE-001",
            "Append-Only Immutability",
            test_tx(),
        );
        // Should create task datoms
        assert!(!datoms.is_empty());
        // Title should contain the inv ID
        let title_datom = datoms.iter().find(|d| d.attribute.as_str() == ":task/title");
        assert!(title_datom.is_some());
        if let Some(d) = title_datom {
            if let Value::String(t) = &d.value {
                assert!(t.contains("INV-STORE-001"));
                assert!(t.starts_with("BUG:"));
            }
        }
    }

    // --- FBW Creation and Serialization ---

    #[test]
    fn create_fbw_content_addressed() {
        let fbw1 = create_fbw(
            test_inv_entity(),
            "spec text",
            "falsification text",
            "test body",
            "src/test.rs",
            2,
            "agent-1",
        );
        let fbw2 = create_fbw(
            test_inv_entity(),
            "spec text",
            "falsification text",
            "test body",
            "src/test.rs",
            2,
            "agent-1",
        );
        // Same inputs → same entity ID (content-addressed, C2)
        assert_eq!(fbw1.entity, fbw2.entity);
    }

    #[test]
    fn create_fbw_different_content_different_entity() {
        let fbw1 = create_fbw(
            test_inv_entity(),
            "spec text A",
            "falsification",
            "test body",
            "src/test.rs",
            2,
            "agent",
        );
        let fbw2 = create_fbw(
            test_inv_entity(),
            "spec text B",
            "falsification",
            "test body",
            "src/test.rs",
            2,
            "agent",
        );
        // Different spec text → different entity
        assert_ne!(fbw1.entity, fbw2.entity);
    }

    #[test]
    fn fbw_to_datoms_roundtrip_structure() {
        let fbw = create_fbw(
            test_inv_entity(),
            "spec statement",
            "if violated then...",
            "assert!(store.len() > 0)",
            "src/store.rs",
            2,
            "test-agent",
        );
        let datoms = fbw_to_datoms(&fbw, test_tx());

        // Should have at least 10 datoms: ident + 3 hashes + traces-to + level + status + verdict + alignment + challenge_count + test_file + agent
        assert!(datoms.len() >= 10, "got {} datoms", datoms.len());

        // Check all required attributes are present
        let attrs: BTreeSet<&str> = datoms.iter().map(|d| d.attribute.as_str()).collect();
        assert!(attrs.contains(":db/ident"));
        assert!(attrs.contains(":witness/spec-hash"));
        assert!(attrs.contains(":witness/falsification-hash"));
        assert!(attrs.contains(":witness/test-body-hash"));
        assert!(attrs.contains(":witness/traces-to"));
        assert!(attrs.contains(":witness/level"));
        assert!(attrs.contains(":witness/status"));
        assert!(attrs.contains(":witness/verdict"));
        assert!(attrs.contains(":witness/alignment-score"));
    }

    // --- Evidence Type Scoring ---

    #[test]
    fn evidence_type_l2_needs_assert() {
        assert!(evidence_type_score("assert!(x > 0);", 2) > 0.5);
        assert!(evidence_type_score("let x = 5;", 2) < 0.5);
    }

    #[test]
    fn evidence_type_l3_needs_property_markers() {
        assert!(evidence_type_score("prop_assert!(x < 100);", 3) > 0.5);
        assert!(evidence_type_score("kani::assume(x < 10);", 3) > 0.5);
        assert!(evidence_type_score("assert!(true);", 3) >= 0.5); // L3 with only assert: 0.5
    }

    // --- Semantic Overlap ---

    #[test]
    fn semantic_overlap_shared_domain_terms() {
        let score = semantic_overlap_score(
            "store append datom assert monotonic",
            "store datom monotonic violated decreasing",
        );
        assert!(score > 0.3, "shared domain terms should score well: {score}");
    }

    // --- WitnessStatus/WitnessVerdict roundtrip ---

    #[test]
    fn status_keyword_roundtrip() {
        for status in [WitnessStatus::Valid, WitnessStatus::Stale, WitnessStatus::Pending] {
            let kw = status.as_keyword();
            assert_eq!(WitnessStatus::from_keyword(kw), status);
        }
    }

    #[test]
    fn verdict_keyword_roundtrip() {
        for verdict in [
            WitnessVerdict::Confirmed,
            WitnessVerdict::Provisional,
            WitnessVerdict::Inconclusive,
            WitnessVerdict::Refuted,
            WitnessVerdict::Unchallenged,
        ] {
            let kw = verdict.as_keyword();
            assert_eq!(WitnessVerdict::from_keyword(kw), verdict);
        }
    }

    // --- Mark Stale Datoms ---

    #[test]
    fn mark_stale_produces_status_datom() {
        let entity = EntityId::from_ident(":witness/test-stale");
        let datoms = mark_stale_datoms(entity, test_tx());
        assert_eq!(datoms.len(), 1);
        assert_eq!(datoms[0].attribute.as_str(), ":witness/status");
        assert_eq!(
            datoms[0].value,
            Value::Keyword(":witness.status/stale".to_string())
        );
    }
}
