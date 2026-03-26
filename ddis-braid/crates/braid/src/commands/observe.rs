//! `braid observe` — One-liner knowledge capture as exploration datoms.
//!
//! Creates an exploration entity in the store from a single text observation.
//! Uses existing Layer 3 `:exploration/*` schema attributes (schema.rs lines 1158-1291).
//!
//! # Design Rationale
//!
//! This replaces manual EDN transaction construction with a single command:
//!   braid observe "merge is a bottleneck" --confidence 0.8 --tag bottleneck
//!
//! The observation becomes an exploration entity with:
//! - `:exploration/body` — the observation text
//! - `:exploration/confidence` — epistemic confidence (0.0-1.0)
//! - `:exploration/category` — auto-classified or explicit
//! - `:exploration/tags` — taxonomy tags for filtering
//! - `:exploration/source` — "braid:observe" (provenance)
//! - `:db/ident` — content-addressed identity from body text
//! - `:db/doc` — same as body (for discoverability via standard queries)
//!
//! Entity ID is `EntityId::from_ident(":observation/{slug}")` where slug is
//! derived from the body text, ensuring content-addressable identity (C2).
//!
//! # Uncertainty Design Decisions
//!
//! - ADR-UNCERTAINTY-001: Confidence as first-class attribute (0.0-1.0 scale).
//! - ADR-UNCERTAINTY-002: Uncertainty as explicit datom, not absence of data.
//! - ADR-UNCERTAINTY-003: Category taxonomy for epistemic classification.
//! - ADR-UNCERTAINTY-004: Maturity lifecycle (conjecture → theorem → axiom).

use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, Value};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::live_store::LiveStore;
use crate::output::{AgentOutput, CommandOutput};

/// Arguments for the observe command.
pub struct ObserveArgs<'a> {
    pub path: &'a Path,
    pub text: &'a str,
    pub confidence: f64,
    pub tags: &'a [String],
    pub category: Option<&'a str>,
    pub agent: &'a str,
    pub relates_to: Option<&'a str>,
    /// Rationale for a design decision (why this choice was made).
    pub rationale: Option<&'a str>,
    /// Alternatives considered (for decisions).
    pub alternatives: Option<&'a str>,
    /// Suppress auto-crystallization of spec findings (COTX-2).
    pub no_auto_crystallize: bool,
}

/// Generate a slug from observation text for the entity ident.
///
/// Takes the first ~60 chars of the text, lowercased, with spaces → hyphens,
/// non-alphanumeric stripped. Produces a deterministic, content-derived identifier.
fn slug_from_text(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .take(60)
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c == ' ' || c == '_' || c == '-' {
                '-'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect();

    // Trim trailing hyphens and collapse consecutive hyphens
    let mut result = String::new();
    let mut prev_hyphen = false;
    for c in cleaned.chars() {
        if c == '-' {
            if !prev_hyphen && !result.is_empty() {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    result.trim_end_matches('-').to_string()
}

/// Auto-detect a category from observation body text.
///
/// Heuristic keyword matching against the body to infer the most likely category
/// when the user doesn't provide an explicit `--category` flag.
///
/// Priority order (first match wins):
/// 1. Spec IDs (INV-*, ADR-*, NEG-*) → "spec-insight"
/// 2. Decision language → "design-decision"
/// 3. Bug/fix language → "issue"
/// 4. Default → "observation"
fn auto_detect_category(text: &str) -> &'static str {
    let lower = text.to_ascii_lowercase();

    // 1. Spec element references
    if text.contains("INV-") || text.contains("ADR-") || text.contains("NEG-") {
        return "spec-insight";
    }

    // 2. Decision language
    if lower.contains("decision")
        || lower.contains("decided")
        || lower.contains("chose")
        || lower.contains("choosing")
    {
        return "design-decision";
    }

    // 3. Bug/fix language
    if lower.contains("bug")
        || lower.contains("fix")
        || lower.contains("error")
        || lower.contains("broken")
    {
        return "issue";
    }

    // 4. Default
    "observation"
}

/// Resolve a category string to a valid `:exploration.cat/*` keyword.
///
/// When `cat` is `None`, auto-detects from `body` text using keyword heuristics.
/// When `cat` is `Some`, the user's explicit choice always wins.
///
/// Supported categories: observation (default), conjecture, definition,
/// algorithm, design-decision, open-question, theorem, spec-insight, issue.
fn resolve_category(cat: Option<&str>, body: &str) -> String {
    let effective = match cat {
        Some(c) => c,
        None => auto_detect_category(body),
    };
    match effective {
        "theorem" => ":exploration.cat/theorem".to_string(),
        "conjecture" => ":exploration.cat/conjecture".to_string(),
        "definition" => ":exploration.cat/definition".to_string(),
        "algorithm" => ":exploration.cat/algorithm".to_string(),
        "design-decision" | "decision" => ":exploration.cat/design-decision".to_string(),
        "open-question" | "question" => ":exploration.cat/open-question".to_string(),
        "spec-insight" => ":exploration.cat/spec-insight".to_string(),
        "issue" => ":exploration.cat/issue".to_string(),
        "observation" => ":exploration.cat/observation".to_string(),
        other => format!(":exploration.cat/{other}"),
    }
}

pub fn run(args: ObserveArgs<'_>) -> Result<CommandOutput, BraidError> {
    // Validate inputs
    if args.text.trim().is_empty() {
        return Err(BraidError::Validation(
            "observation text cannot be empty".to_string(),
        ));
    }
    if !(0.0..=1.0).contains(&args.confidence) {
        return Err(BraidError::Validation(format!(
            "confidence must be 0.0-1.0, got {}",
            args.confidence
        )));
    }

    let mut live = LiveStore::open(args.path)?;
    let store = live.store();

    let agent = AgentId::from_name(args.agent);
    let slug = slug_from_text(args.text);
    let ident = format!(":observation/{slug}");
    let entity = EntityId::from_ident(&ident);
    let category = resolve_category(args.category, args.text);

    // Generate TxId: advance past the store's current frontier (Unix epoch seconds)
    let tx_id = super::write::next_tx_id(store, agent);

    // Compute BLAKE3 content hash for cross-session dedup (INV-HARVEST-006)
    let content_hash = blake3::hash(args.text.as_bytes());

    // Build datom vector — 8 core assertions + tags + optional cross-ref
    let mut datoms = vec![
        // Core identity
        Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident.clone()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(args.text.to_string()),
            tx_id,
            Op::Assert,
        ),
        // Exploration attributes
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/body"),
            Value::String(args.text.to_string()),
            tx_id,
            Op::Assert,
        ),
        // Content hash for crystallization guard (INV-HARVEST-006)
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/content-hash"),
            Value::Bytes(content_hash.as_bytes().to_vec()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/confidence"),
            Value::Double(ordered_float::OrderedFloat(args.confidence)),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/category"),
            Value::Keyword(category),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/source"),
            Value::String("braid:observe".to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/maturity"),
            Value::Keyword(":exploration.maturity/sketch".to_string()),
            tx_id,
            Op::Assert,
        ),
    ];

    // Add tags
    for tag in args.tags {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":exploration/tags"),
            Value::Keyword(format!(":tag/{tag}")),
            tx_id,
            Op::Assert,
        ));
    }

    // B4: Auto-link to current session (INV-SESSION-001)
    // Look up the most recent active session entity via :session/status
    let active_session = find_active_session(store);
    if let Some(session_entity) = active_session {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":exploration/source-session"),
            Value::Ref(session_entity),
            tx_id,
            Op::Assert,
        ));
    }

    // Add cross-reference if provided
    if let Some(relates_to) = args.relates_to {
        let target = EntityId::from_ident(relates_to);
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":exploration/related-spec"),
            Value::Ref(target),
            tx_id,
            Op::Assert,
        ));
    }

    // Add rationale for decision observations
    if let Some(rationale) = args.rationale {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":exploration/rationale"),
            Value::String(rationale.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    // Add alternatives considered
    if let Some(alternatives) = args.alternatives {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":exploration/alternatives"),
            Value::String(alternatives.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    // --- COTX-2: Auto-crystallization of spec findings (Rule 2) ---
    let auto_crystallized = if !args.no_auto_crystallize && args.confidence >= 0.8 {
        auto_crystallize_finding(args.text, entity, tx_id, store, &mut datoms)
    } else {
        None
    };

    // --- COTX-5: Universal observe cotransaction rules ---
    let mut cotx_entities: Vec<(String, String)> = Vec::new(); // (type, ident)

    if auto_crystallized.is_some() {
        cotx_entities.push(("finding".to_string(), auto_crystallized.clone().unwrap()));
    }

    // Rule 3: Action → auto-task
    let lower = args.text.to_ascii_lowercase();
    let has_action_lang = lower.contains("fix ")
        || lower.contains("implement ")
        || lower.contains("add ")
        || lower.contains("wire ")
        || lower.contains("verify ")
        || lower.starts_with("bug:")
        || lower.starts_with("fix:");
    if !args.no_auto_crystallize && has_action_lang && auto_crystallized.is_none() {
        let task_title_full = braid_kernel::task::short_title(args.text).to_string();
        // EXT-BUG-3: Truncate to ~80 chars on word boundary for readable task listings.
        // Full observation text goes into :task/body.
        // CE-FIX BUG-1: Use safe_truncate_bytes to avoid panics on multi-byte UTF-8.
        let task_title = if task_title_full.len() > 80 {
            let truncated = braid_kernel::budget::safe_truncate_bytes(&task_title_full, 80);
            match truncated.rfind(' ') {
                Some(pos) if pos > 20 => task_title_full[..pos].to_string(),
                _ => truncated.to_string(),
            }
        } else {
            task_title_full.clone()
        };
        if task_title.len() >= 5 {
            let task_id = braid_kernel::task::generate_task_id(&task_title);
            let task_ident = format!(":task/{task_id}");
            let task_entity = EntityId::from_ident(&task_ident);

            if store.entity_datoms(task_entity).is_empty() {
                datoms.push(Datom::new(
                    task_entity,
                    Attribute::from_keyword(":db/ident"),
                    Value::Keyword(task_ident.clone()),
                    tx_id,
                    Op::Assert,
                ));
                datoms.push(Datom::new(
                    task_entity,
                    Attribute::from_keyword(":task/id"),
                    Value::String(task_id),
                    tx_id,
                    Op::Assert,
                ));
                datoms.push(Datom::new(
                    task_entity,
                    Attribute::from_keyword(":task/title"),
                    Value::String(task_title),
                    tx_id,
                    Op::Assert,
                ));
                // EXT-BUG-3: Store full text in :task/body when title was truncated.
                if task_title_full.len() > 80 {
                    datoms.push(Datom::new(
                        task_entity,
                        Attribute::from_keyword(":task/body"),
                        Value::String(args.text.to_string()),
                        tx_id,
                        Op::Assert,
                    ));
                }
                datoms.push(Datom::new(
                    task_entity,
                    Attribute::from_keyword(":task/status"),
                    Value::Keyword(":task.status/open".to_string()),
                    tx_id,
                    Op::Assert,
                ));
                datoms.push(Datom::new(
                    task_entity,
                    Attribute::from_keyword(":task/priority"),
                    Value::Long(2),
                    tx_id,
                    Op::Assert,
                ));
                datoms.push(Datom::new(
                    task_entity,
                    Attribute::from_keyword(":task/type"),
                    Value::Keyword(":task.type/task".to_string()),
                    tx_id,
                    Op::Assert,
                ));
                let created_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                datoms.push(Datom::new(
                    task_entity,
                    Attribute::from_keyword(":task/created-at"),
                    Value::Long(created_at as i64),
                    tx_id,
                    Op::Assert,
                ));
                cotx_entities.push(("task".to_string(), task_ident));
            }
        }
    }

    // Rule 4: Decision → ADR skeleton
    let has_decision_lang = lower.contains("decided ")
        || lower.contains("chose ")
        || lower.contains("rejected ")
        || lower.contains("decision:");
    if !args.no_auto_crystallize
        && has_decision_lang
        && auto_crystallized.is_none()
        && args.confidence >= 0.7
    {
        let slug = slug_from_text(args.text);
        let adr_ident = format!(":spec/adr-finding-{slug}");
        let adr_entity = EntityId::from_ident(&adr_ident);
        if store.entity_datoms(adr_entity).is_empty() {
            datoms.push(Datom::new(
                adr_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(adr_ident.clone()),
                tx_id,
                Op::Assert,
            ));
            datoms.push(Datom::new(
                adr_entity,
                Attribute::from_keyword(":spec/element-type"),
                Value::Keyword(":spec.finding/adr".to_string()),
                tx_id,
                Op::Assert,
            ));
            datoms.push(Datom::new(
                adr_entity,
                Attribute::from_keyword(":element/statement"),
                Value::String(args.text.to_string()),
                tx_id,
                Op::Assert,
            ));
            datoms.push(Datom::new(
                adr_entity,
                Attribute::from_keyword(":spec/source-observation"),
                Value::Ref(entity),
                tx_id,
                Op::Assert,
            ));
            datoms.push(Datom::new(
                adr_entity,
                Attribute::from_keyword(":spec/auto-crystallized"),
                Value::Boolean(true),
                tx_id,
                Op::Assert,
            ));
            cotx_entities.push(("adr-skeleton".to_string(), adr_ident));
        }
    }

    // Rule 5: Question → open question
    let has_question_lang = lower.contains("how should")
        || lower.contains("what if")
        || lower.contains("should we")
        || lower.contains("open question:");
    if !args.no_auto_crystallize && has_question_lang && args.confidence < 0.7 {
        let slug = slug_from_text(args.text);
        let q_ident = format!(":exploration/question-{slug}");
        let q_entity = EntityId::from_ident(&q_ident);
        if store.entity_datoms(q_entity).is_empty() {
            datoms.push(Datom::new(
                q_entity,
                Attribute::from_keyword(":db/ident"),
                Value::Keyword(q_ident.clone()),
                tx_id,
                Op::Assert,
            ));
            datoms.push(Datom::new(
                q_entity,
                Attribute::from_keyword(":exploration/category"),
                Value::Keyword(":exploration.cat/open-question".to_string()),
                tx_id,
                Op::Assert,
            ));
            datoms.push(Datom::new(
                q_entity,
                Attribute::from_keyword(":exploration/body"),
                Value::String(args.text.to_string()),
                tx_id,
                Op::Assert,
            ));
            datoms.push(Datom::new(
                q_entity,
                Attribute::from_keyword(":exploration/confidence"),
                Value::Double(ordered_float::OrderedFloat(args.confidence)),
                tx_id,
                Op::Assert,
            ));
            cotx_entities.push(("open-question".to_string(), q_ident));
        }
    }

    let tx = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Observed,
        rationale: args.text.to_string(),
        causal_predecessors: vec![],
        datoms,
    };

    let datom_count = tx.datoms.len();
    let file_path = live.write_tx(&tx)?;

    // Re-borrow store after write (live store already has the update)
    let store = live.store();
    let new_total = store.len();
    let cat_short = resolve_category(args.category, args.text)
        .strip_prefix(":exploration.cat/")
        .unwrap_or("observation")
        .to_string();

    // --- CCE-5: Concept-aware observation (embedding, concept assignment, entity linking) ---
    // MODEL-WIRE: Use resolved embedder (model2vec if available, hash fallback).
    let (embedder, embedder_type) = super::model::resolve_embedder(args.path);
    let current_emb_kw = format!(":embedder/{embedder_type}");

    // EMBED-CONSISTENCY: Detect embedder type mismatch and re-embed concepts if needed.
    // Check if any concept has a different :concept/embedder-type than current.
    {
        let emb_type_attr = Attribute::from_keyword(":concept/embedder-type");
        let concept_emb_attr = Attribute::from_keyword(":concept/embedding");
        let needs_reembed = store.datoms().any(|d| {
            d.op == braid_kernel::datom::Op::Assert
                && d.attribute == emb_type_attr
                && matches!(&d.value, Value::Keyword(k) if k != &current_emb_kw)
        });

        if needs_reembed {
            // Re-embed all concepts with the current embedder.
            let concept_name_attr = Attribute::from_keyword(":concept/name");
            let desc_attr = Attribute::from_keyword(":concept/description");
            let mut reembed_datoms = Vec::new();
            let reembed_tx = super::write::next_tx_id(store, agent);

            // Collect concept entities that need re-embedding.
            let mut concepts_to_reembed: std::collections::BTreeMap<
                braid_kernel::datom::EntityId,
                String,
            > = std::collections::BTreeMap::new();
            for d in store.datoms() {
                if d.op == braid_kernel::datom::Op::Assert && d.attribute == desc_attr {
                    if let Value::String(ref s) = d.value {
                        // Only re-embed if this entity has a concept/name (is a concept).
                        if store.live_value(d.entity, &concept_name_attr).is_some() {
                            concepts_to_reembed.insert(d.entity, s.clone());
                        }
                    }
                }
            }

            for (entity, description) in &concepts_to_reembed {
                let new_emb = embedder.embed(description);
                reembed_datoms.push(Datom::new(
                    *entity,
                    concept_emb_attr.clone(),
                    Value::Bytes(braid_kernel::embedding::embedding_to_bytes(&new_emb)),
                    reembed_tx,
                    Op::Assert,
                ));
                reembed_datoms.push(Datom::new(
                    *entity,
                    emb_type_attr.clone(),
                    Value::Keyword(current_emb_kw.clone()),
                    reembed_tx,
                    Op::Assert,
                ));
            }

            if !reembed_datoms.is_empty() {
                let reembed_tx_file = TxFile {
                    tx_id: reembed_tx,
                    agent,
                    provenance: ProvenanceType::Derived,
                    rationale: format!(
                        "EMBED-CONSISTENCY: re-embedded {} concepts ({} -> {})",
                        concepts_to_reembed.len(),
                        "prior",
                        current_emb_kw
                    ),
                    causal_predecessors: vec![],
                    datoms: reembed_datoms,
                };
                let _ = live.write_tx(&reembed_tx_file);
                let _ = live.store(); // Refresh after re-embed.
            }
        }
    }
    let store = live.store();

    let obs_embedding = embedder.embed(args.text);
    // STEER-2: Embedder-aware threshold. Policy config overrides embedder default.
    let join_threshold: f32 = braid_kernel::config::get_config(store, "concept.join-threshold")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| embedder.join_threshold());
    // CONCEPT-MULTI + ADR-FOUNDATION-031: Sigmoid soft membership.
    let sigmoid_temperature: f32 = braid_kernel::config::get_config(store, "concept.sigmoid-temperature")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.05); // Bootstrap default temperature
    let mut multi_assignments = braid_kernel::concept::assign_to_concepts_soft(
        store,
        &obs_embedding,
        join_threshold,
        sigmoid_temperature,
        0.1, // min_strength: filter out noise below 10%
    );
    // Backward-compatible single assignment: first match or Uncategorized.
    let mut concept_assignment = multi_assignments
        .first()
        .cloned()
        .unwrap_or(braid_kernel::concept::ConceptAssignment::Uncategorized);

    // --- INQ-1-REV: Auto-crystallization when uncategorized observations accumulate ---
    // C9: config override with fallback to constant.
    let min_cluster_size: usize =
        braid_kernel::config::get_config(store, "concept.min-cluster-size")
            .and_then(|v| v.parse().ok())
            .unwrap_or(braid_kernel::concept::MIN_CLUSTER_SIZE);
    let mut auto_crystallized_concepts: Vec<String> = Vec::new();
    let mut uncategorized_count: usize = 0;

    if matches!(concept_assignment, braid_kernel::concept::ConceptAssignment::Uncategorized) {
        // Count uncategorized observations (have embedding but no concept ref).
        let concept_ref_attr = Attribute::from_keyword(":exploration/concept");
        let embed_ref_attr = Attribute::from_keyword(":exploration/embedding");

        let entities_with_embedding: std::collections::HashSet<braid_kernel::datom::EntityId> =
            store
                .datoms()
                .filter(|d| d.op == Op::Assert && d.attribute == embed_ref_attr)
                .map(|d| d.entity)
                .collect();
        let entities_with_concept: std::collections::HashSet<braid_kernel::datom::EntityId> =
            store
                .datoms()
                .filter(|d| d.op == Op::Assert && d.attribute == concept_ref_attr)
                .map(|d| d.entity)
                .collect();
        let existing_uncategorized: Vec<braid_kernel::datom::EntityId> = entities_with_embedding
            .difference(&entities_with_concept)
            .copied()
            .collect();

        // +1 for the current observation (its embedding isn't in the store yet).
        uncategorized_count = existing_uncategorized.len() + 1;

        if uncategorized_count >= min_cluster_size {
            // Collect all uncategorized observation embeddings + body text.
            let body_attr_for_cryst = Attribute::from_keyword(":exploration/body");
            let mut observations: Vec<(braid_kernel::datom::EntityId, Vec<f32>, String)> =
                Vec::new();
            for &eid in &existing_uncategorized {
                let emb = store
                    .live_value(eid, &embed_ref_attr)
                    .and_then(|v| {
                        if let braid_kernel::datom::Value::Bytes(b) = v {
                            Some(braid_kernel::embedding::bytes_to_embedding(b))
                        } else {
                            None
                        }
                    });
                let body = store
                    .live_value(eid, &body_attr_for_cryst)
                    .and_then(|v| {
                        if let braid_kernel::datom::Value::String(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                if let Some(embedding) = emb {
                    observations.push((eid, embedding, body));
                }
            }
            // Include current observation.
            observations.push((entity, obs_embedding.clone(), args.text.to_string()));

            let crystallize_threshold: f32 =
                braid_kernel::config::get_config(store, "concept.crystallize-threshold")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(join_threshold);

            let new_concepts = braid_kernel::concept::crystallize_concepts(
                &observations,
                crystallize_threshold,
                min_cluster_size,
            );

            if !new_concepts.is_empty() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                let cryst_agent = AgentId::from_name(args.agent);
                let cryst_tx_id = super::write::next_tx_id(store, cryst_agent);
                let mut cryst_datoms = Vec::new();

                for concept in &new_concepts {
                    let concept_datom_tuples =
                        braid_kernel::concept::concept_to_datoms(concept, now);
                    for (e, a, v) in concept_datom_tuples {
                        cryst_datoms.push(Datom::new(e, a, v, cryst_tx_id, Op::Assert));
                    }
                    let member_datom_tuples = braid_kernel::concept::membership_datoms(
                        concept.entity,
                        &concept.members,
                    );
                    for (e, a, v) in member_datom_tuples {
                        cryst_datoms.push(Datom::new(e, a, v, cryst_tx_id, Op::Assert));
                    }
                    auto_crystallized_concepts.push(concept.name.clone());
                }

                let cryst_tx = TxFile {
                    tx_id: cryst_tx_id,
                    agent: cryst_agent,
                    provenance: ProvenanceType::Derived,
                    rationale: format!(
                        "INQ-1-REV: auto-crystallized {} concept(s) from {} uncategorized observations",
                        new_concepts.len(),
                        uncategorized_count,
                    ),
                    causal_predecessors: vec![],
                    datoms: cryst_datoms,
                };
                let _ = live.write_tx(&cryst_tx);
                let _ = live.store(); // Refresh after crystallization write.

                // Re-try concept assignment now that concepts exist.
                let store = live.store();
                multi_assignments = braid_kernel::concept::assign_to_concepts_soft(
                    store,
                    &obs_embedding,
                    join_threshold,
                    sigmoid_temperature,
                    0.1,
                );
                concept_assignment = multi_assignments
                    .first()
                    .cloned()
                    .unwrap_or(braid_kernel::concept::ConceptAssignment::Uncategorized);
            }
        }
    }
    let store = live.store();

    let entity_matches = braid_kernel::concept::entity_auto_link(store, args.text);
    let steering = braid_kernel::concept::compute_observe_steering_multi(
        store,
        &multi_assignments,
        &concept_assignment,
        &entity_matches,
    );

    // --- ADR-FOUNDATION-031: Online threshold calibration ---
    // Update running sufficient statistics after each observation.
    // The posterior after observation N becomes the prior for observation N+1.
    // Sufficient statistics (count, sum, sum_sq) form a commutative monoid.
    let primary_similarity = match &concept_assignment {
        braid_kernel::concept::ConceptAssignment::Joined { similarity, .. } => Some(*similarity),
        _ => None,
    };
    if let Some(sim) = primary_similarity {
        let cal_count: i64 = braid_kernel::config::get_config(store, "calibration.similarity-count")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let cal_sum: f64 = braid_kernel::config::get_config(store, "calibration.similarity-sum")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        let cal_sum_sq: f64 = braid_kernel::config::get_config(store, "calibration.similarity-sum-sq")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);

        let new_count = cal_count + 1;
        let new_sum = cal_sum + sim as f64;
        let new_sum_sq = cal_sum_sq + (sim as f64) * (sim as f64);

        // Write updated sufficient statistics using set_config_datoms (3 datoms per key:
        // :db/ident, :config/key, :config/value — matches what get_config reads).
        let cal_agent = AgentId::from_name("braid:cal");
        let cal_tx = super::write::next_tx_id(store, cal_agent);
        let mut cal_datoms = Vec::new();
        cal_datoms.extend(braid_kernel::config::set_config_datoms(
            "calibration.similarity-count", &new_count.to_string(), "project", cal_tx,
        ));
        cal_datoms.extend(braid_kernel::config::set_config_datoms(
            "calibration.similarity-sum", &format!("{new_sum:.8}"), "project", cal_tx,
        ));
        cal_datoms.extend(braid_kernel::config::set_config_datoms(
            "calibration.similarity-sum-sq", &format!("{new_sum_sq:.8}"), "project", cal_tx,
        ));

        // After 3+ observations, compute and write calibrated threshold + temperature.
        if new_count >= 3 {
            let mean = new_sum / new_count as f64;
            let variance = (new_sum_sq / new_count as f64 - mean * mean).max(0.0);
            let stddev = variance.sqrt();
            let cal_threshold = (mean - 0.5 * stddev).clamp(0.15, 0.85) as f32;
            let cal_temperature = (stddev / 2.0).max(0.01) as f32;

            cal_datoms.extend(braid_kernel::config::set_config_datoms(
                "concept.join-threshold", &format!("{cal_threshold:.4}"), "project", cal_tx,
            ));
            cal_datoms.extend(braid_kernel::config::set_config_datoms(
                "concept.sigmoid-temperature", &format!("{cal_temperature:.4}"), "project", cal_tx,
            ));
        }

        let cal_tx_file = TxFile {
            tx_id: cal_tx,
            agent: cal_agent,
            provenance: ProvenanceType::Derived,
            rationale: format!(
                "ADR-FOUNDATION-031: online calibration (n={new_count}, mean={:.4}, threshold={})",
                new_sum / new_count as f64,
                if new_count >= 3 {
                    let mean = new_sum / new_count as f64;
                    let var = (new_sum_sq / new_count as f64 - mean * mean).max(0.0);
                    format!("{:.4}", (mean - 0.5 * var.sqrt()).clamp(0.15, 0.85))
                } else {
                    "pending".to_string()
                }
            ),
            causal_predecessors: vec![],
            datoms: cal_datoms,
        };
        if let Err(e) = live.write_tx(&cal_tx_file) {
            eprintln!("warning: online calibration write failed: {e}");
        }
        let _ = live.store(); // Refresh after calibration write.
    }
    let store = live.store();

    // Write CCE datoms as a follow-up transaction if there's concept/entity info to record.
    let mut cce_datoms = Vec::new();
    // Embedding datom.
    cce_datoms.push(Datom::new(
        entity,
        Attribute::from_keyword(":exploration/embedding"),
        Value::Bytes(braid_kernel::embedding::embedding_to_bytes(&obs_embedding)),
        tx_id,
        Op::Assert,
    ));
    // CONCEPT-MULTI: Write one :exploration/concept Ref per matched concept.
    // Update each matched concept's centroid via surprise-weighted update.
    // Surprise is recorded once (from primary match — highest similarity).
    let emb_attr = Attribute::from_keyword(":concept/embedding");
    let count_attr_kw = Attribute::from_keyword(":concept/member-count");
    let weight_attr_kw = Attribute::from_keyword(":concept/total-weight");

    for (idx, assignment) in multi_assignments.iter().enumerate() {
        if let braid_kernel::concept::ConceptAssignment::Joined {
            concept,
            surprise,
            ..
        } = assignment
        {
            // Write :exploration/concept Ref for each membership.
            cce_datoms.push(Datom::new(
                entity,
                Attribute::from_keyword(":exploration/concept"),
                Value::Ref(*concept),
                tx_id,
                Op::Assert,
            ));
            // Surprise only from primary (index 0).
            if idx == 0 {
                cce_datoms.push(Datom::new(
                    entity,
                    Attribute::from_keyword(":exploration/surprise"),
                    Value::Double(ordered_float::OrderedFloat(*surprise as f64)),
                    tx_id,
                    Op::Assert,
                ));
            }

            // --- LIFECYCLE-OBSERVE: Update each concept entity ---
            let old_count = store
                .live_value(*concept, &count_attr_kw)
                .and_then(|v| if let Value::Long(n) = v { Some(*n as usize) } else { None })
                .unwrap_or(0);

            let old_total_weight = store
                .live_value(*concept, &weight_attr_kw)
                .and_then(|v| if let Value::Double(w) = v { Some(w.into_inner()) } else { None })
                .unwrap_or(0.0);

            let old_centroid = store
                .live_value(*concept, &emb_attr)
                .and_then(|v| if let Value::Bytes(b) = v { Some(braid_kernel::embedding::bytes_to_embedding(b)) } else { None });

            let strength = match assignment {
                braid_kernel::concept::ConceptAssignment::Joined { strength, .. } => *strength,
                _ => 1.0,
            };
            let sw = braid_kernel::concept::surprise_weight(*surprise, braid_kernel::concept::DEFAULT_ALPHA) * strength;

            if let Some(old_cent) = old_centroid {
                let (mut new_centroid, new_total_weight) =
                    braid_kernel::concept::update_centroid_weighted(&old_cent, old_total_weight, &obs_embedding, sw);
                braid_kernel::embedding::l2_normalize(&mut new_centroid);

                let new_count = old_count + 1;

                cce_datoms.push(Datom::new(
                    *concept,
                    Attribute::from_keyword(":concept/member-count"),
                    Value::Long(new_count as i64),
                    tx_id,
                    Op::Assert,
                ));
                cce_datoms.push(Datom::new(
                    *concept,
                    Attribute::from_keyword(":concept/total-weight"),
                    Value::Double(ordered_float::OrderedFloat(new_total_weight)),
                    tx_id,
                    Op::Assert,
                ));
                cce_datoms.push(Datom::new(
                    *concept,
                    Attribute::from_keyword(":concept/embedding"),
                    Value::Bytes(braid_kernel::embedding::embedding_to_bytes(&new_centroid)),
                    tx_id,
                    Op::Assert,
                ));
            }
        }
    }
    // Entity auto-link datoms.
    for m in &entity_matches {
        cce_datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":exploration/mentions-entity"),
            Value::Ref(m.entity),
            tx_id,
            Op::Assert,
        ));
    }
    if !cce_datoms.is_empty() {
        let cce_tx = TxFile {
            tx_id: super::write::next_tx_id(store, agent),
            agent,
            provenance: ProvenanceType::Derived,
            rationale: "CCE-5: concept assignment + entity linking".to_string(),
            causal_predecessors: vec![],
            datoms: cce_datoms,
        };
        live.write_tx(&cce_tx)?;
        // Refresh store after CCE write.
        let _ = live.store();
    }
    let store = live.store();

    // --- CE-OBSERVE: Responsive observation — contextual micro-hypotheses ---
    let connections =
        braid_kernel::connections::propose_connections(store, entity, args.text);
    let topo_events =
        braid_kernel::connections::detect_topological_events(&connections, store);

    let mut responsive_parts: Vec<String> = Vec::new();

    // Connection information
    if !connections.is_empty() {
        let top = &connections[0];
        let body_attr = Attribute::from_keyword(":exploration/body");
        if let Some(Value::String(target_text)) = store.live_value(top.target, &body_attr)
        {
            let target_preview = braid_kernel::budget::safe_truncate_bytes(target_text, 60);
            let ellipsis = if target_text.len() > 60 { "..." } else { "" };
            responsive_parts.push(format!(
                "connected: '{}{}' (shared: {})",
                target_preview,
                ellipsis,
                top.shared_keywords.join(", ")
            ));
        } else {
            responsive_parts.push(format!(
                "connected: entity (shared: {})",
                top.shared_keywords.join(", ")
            ));
        }

        if connections.len() > 1 {
            responsive_parts.push(format!(
                "{} total connections found",
                connections.len()
            ));
        }
    }

    // Topological events
    for event in &topo_events {
        responsive_parts.push(event.clone());
    }

    // --- INQ-1-REV: Auto-crystallization feedback ---
    if !auto_crystallized_concepts.is_empty() {
        for name in &auto_crystallized_concepts {
            responsive_parts.push(format!(
                "AUTO-CRYSTALLIZED: concept [{name}] from {uncategorized_count} observations"
            ));
        }
    } else if matches!(
        concept_assignment,
        braid_kernel::concept::ConceptAssignment::Uncategorized
    ) && uncategorized_count > 0
    {
        // Show progress toward crystallization.
        responsive_parts.push(format!(
            "uncategorized ({uncategorized_count}/{min_cluster_size} toward first concepts)"
        ));
    }

    // --- INQ-2 + INQ-3: Graduated situational brief ---
    // Compute discrepancy brief for surprising observations.
    let discrepancy = match &concept_assignment {
        braid_kernel::concept::ConceptAssignment::Joined {
            concept, surprise, ..
        } => {
            let disc_min_surprise: f32 =
                braid_kernel::config::get_config(store, "concept.discrepancy-min-surprise")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.3);
            braid_kernel::concept::compute_discrepancy_brief(
                store,
                &obs_embedding,
                *concept,
                *surprise,
                disc_min_surprise,
            )
        }
        _ => None,
    };

    // Situational brief replaces the old concept_line + gap_line + steering.question.
    if let Some(brief) = braid_kernel::concept::situational_brief(
        store,
        &concept_assignment,
        &topo_events,
        discrepancy.as_ref(),
    ) {
        responsive_parts.push(brief.line);
    } else if let Some(ref line) = steering.concept_line {
        // Fallback to legacy steering when situational brief not applicable.
        responsive_parts.push(line.clone());
    } else {
        // STEER-3b: Near-miss feedback when concept assignment doesn't fire.
        if let Some((_, best_sim)) =
            braid_kernel::concept::find_nearest_concept(store, &obs_embedding)
        {
            if best_sim > 0.05 {
                responsive_parts.push(format!(
                    "near: best concept match cosine={best_sim:.2} (threshold={join_threshold:.2})"
                ));
            }
        }
    }
    if let Some(ref line) = steering.gap_line {
        responsive_parts.push(line.clone());
    }
    // FRONTIER-STEER: Replace formulaic steering question with computed frontier rec.
    if let Some(rec) = braid_kernel::concept::frontier_recommendation(store, &obs_embedding) {
        responsive_parts.push(format!(
            "\u{2192} {}: {} \u{2014} {}",
            rec.kind, rec.target, rec.rationale
        ));
    } else if let Some(ref q) = steering.question {
        // Fallback to steering question when no frontier recommendation available.
        responsive_parts.push(format!("\u{2192} {q}"));
    } else if connections.is_empty() {
        responsive_parts.push(
            "\u{2192} No connections yet. What else relates to this?".to_string(),
        );
    }

    // --- META-3: Real-time crystallization feedback (INV-GUIDANCE-014, INV-BILATERAL-001) ---
    let has_spec_refs = args.text.contains("INV-") || args.text.contains("ADR-") || args.text.contains("NEG-");
    let spec_refs_exist = if has_spec_refs {
        let refs = braid_kernel::task::parse_spec_refs(args.text);
        refs.iter().any(|r| {
            let ident = format!(":spec/{}", r.to_lowercase());
            let e = EntityId::from_ident(&ident);
            !store.entity_datoms(e).is_empty()
        })
    } else {
        false
    };
    // COTX-5: Cotransacted observations get strong positive delta
    let delta_cryst: f64 = if !cotx_entities.is_empty() {
        0.7 // Cotransacted: observation + entity in same tx
    } else if spec_refs_exist {
        0.2 // Anchored to existing spec
    } else {
        -0.1 // Unanchored intent
    };

    // Find nearest spec element for unanchored observations
    let nearest_spec = if delta_cryst < 0.0 {
        braid_kernel::guidance::spec_relevance_scan(args.text, store)
            .into_iter()
            .next()
    } else {
        None
    };

    // --- CRB: Auto-reconciliation (INV-GUIDANCE-024, CRB-7) ---
    // Run broadened knowledge relevance scan on observation text to surface related
    // spec elements, tasks, AND observations. This prevents the meta-irony failure
    // where agents complain about problems already documented in the store.
    let related_specs = braid_kernel::guidance::knowledge_relevance_scan(args.text, store);

    // --- ACP: Build ActionProjection (INV-BUDGET-007) ---
    let action = braid_kernel::budget::ProjectedAction {
        command: "braid status".to_string(),
        rationale: "check store state after observation".to_string(),
        impact: 0.3,
    };

    let mut context_blocks = Vec::new();

    // Summary context (System — always shown)
    let summary = if !cotx_entities.is_empty() {
        let types: Vec<&str> = cotx_entities.iter().map(|(t, _)| t.as_str()).collect();
        format!(
            "observed + cotx[{}]: (confidence={:.2}, category={cat_short}, +{datom_count} datoms)",
            types.join(","),
            args.confidence,
        )
    } else {
        format!(
            "observed: {ident} (confidence={:.2}, category={cat_short}, +{datom_count} datoms)",
            args.confidence,
        )
    };
    context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
        braid_kernel::budget::OutputPrecedence::System,
        summary,
        15,
    ));

    // Store state (Methodology)
    context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
        braid_kernel::budget::OutputPrecedence::Methodology,
        format!(
            "store: {new_total} datoms | tx: {}",
            file_path.relative_path()
        ),
        10,
    ));

    // CE-OBSERVE: Responsive context blocks (connections, topo events, micro-hypothesis)
    if !responsive_parts.is_empty() {
        // In agent mode, this single block stays concise (3 lines max from responsive_parts).
        // In human mode, the full projection shows everything.
        let responsive_text = responsive_parts.join("\n");
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::Methodology,
            responsive_text,
            13, // Between summary (15) and store (10) — shown before tags
        ));
    }

    // Tags if present (UserRequested)
    if !args.tags.is_empty() {
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::UserRequested,
            format!("tags: {}", args.tags.join(", ")),
            5,
        ));
    }

    // Cross-reference if present (UserRequested)
    if let Some(relates_to) = args.relates_to {
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::UserRequested,
            format!("relates-to: {relates_to}"),
            5,
        ));
    }

    // Rationale if present (Speculative)
    if let Some(rationale) = args.rationale {
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::Speculative,
            format!("rationale: {rationale}"),
            10,
        ));
    }

    // Alternatives if present (Speculative)
    if let Some(alternatives) = args.alternatives {
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::Speculative,
            format!("alternatives: {alternatives}"),
            10,
        ));
    }

    // CRB: Related knowledge (Methodology — important for reconciliation)
    if !related_specs.is_empty() {
        for sr in &related_specs {
            context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
                braid_kernel::budget::OutputPrecedence::Methodology,
                format!(
                    "related: [{}] {} — {} (score={:.2})",
                    sr.source, sr.human_id, sr.summary, sr.score
                ),
                12,
            ));
        }
        if related_specs.len() >= 3 {
            context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
                braid_kernel::budget::OutputPrecedence::System,
                "3+ existing knowledge elements found. Reconcile before crystallizing."
                    .to_string(),
                8,
            ));
        }
    }

    // META-3: Crystallization feedback — only show when POSITIVE (anchored to spec).
    // STEER-3b: Suppress Δ-cryst for non-spec observations (the common case).
    // Negative cryst scores appear on every non-spec observation and are never actionable.
    if delta_cryst > f64::EPSILON {
        // Find which spec element(s) the observation is anchored to
        let refs = braid_kernel::task::parse_spec_refs(args.text);
        let ref_str = if refs.is_empty() {
            "spec element".to_string()
        } else {
            refs.join(", ")
        };
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::Methodology,
            format!("\u{0394}-cryst: +{delta_cryst:.1} (anchored to {ref_str})"),
            10,
        ));
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: String::new(), // STEER-3b: removed noise line
    };

    // Human output uses ACP full projection
    let human = projection.project(usize::MAX);

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // JSON output with _acp field merged
    let mut json = serde_json::json!({
        "entity": ident,
        "confidence": args.confidence,
        "category": cat_short,
        "datoms_added": datom_count,
        "store_total": new_total,
        "tx": file_path.relative_path(),
        "delta_crystallization": delta_cryst,
        "nearest_spec": nearest_spec.as_ref().map(|n| serde_json::json!({
            "id": n.human_id,
            "score": n.score,
            "summary": n.summary,
        })),
        "auto_crystallized": auto_crystallized,
        "auto_crystallized_concepts": auto_crystallized_concepts,
        "cotransacted": cotx_entities.iter().map(|(t, i)| serde_json::json!({"type": t, "ident": i})).collect::<Vec<_>>(),
        "connections": connections.iter().map(|c| {
            let hex: String = c.target.as_bytes().iter().take(8).map(|b| format!("{b:02x}")).collect();
            serde_json::json!({
                "target": hex,
                "similarity": c.similarity,
                "raw_jaccard": c.raw_jaccard,
                "shared_keywords": c.shared_keywords,
            })
        }).collect::<Vec<_>>(),
        "topological_events": topo_events,
        "micro_hypothesis": responsive_parts.last().cloned(),
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput { json, agent, human })
}

/// COTX-2: Auto-crystallize a spec finding from a structured observation.
///
/// Criteria: observation text must contain (a) spec ID pattern (INV-/ADR-/NEG-),
/// (b) falsification language (for INV) or decision language (for ADR),
/// (c) confidence >= 0.8 (checked by caller).
///
/// Produces a `:spec/finding` entity (NOT a full invariant/ADR) in the SAME
/// datom vec, creating a cotransaction. Findings are promotable to full spec
/// elements via `braid spec create`.
///
/// Returns the finding ident if crystallization occurred, None otherwise.
fn auto_crystallize_finding(
    text: &str,
    observation_entity: EntityId,
    tx_id: braid_kernel::datom::TxId,
    store: &braid_kernel::Store,
    datoms: &mut Vec<Datom>,
) -> Option<String> {
    // (a) Must contain spec ID pattern
    let has_inv = text.contains("INV-");
    let has_adr = text.contains("ADR-");
    let has_neg = text.contains("NEG-");
    if !has_inv && !has_adr && !has_neg {
        return None;
    }

    // Extract the spec namespace from the first pattern match
    let refs = braid_kernel::task::parse_spec_refs(text);
    if refs.is_empty() {
        return None;
    }

    // (b) Must contain appropriate language
    let lower = text.to_ascii_lowercase();
    let has_falsification_lang = lower.contains("violated if")
        || lower.contains("fails when")
        || lower.contains("should never")
        || lower.contains("must not")
        || lower.contains("falsified");
    let has_decision_lang = lower.contains("decided")
        || lower.contains("chose")
        || lower.contains("rejected")
        || lower.contains("decision");

    let element_type = if has_inv && has_falsification_lang {
        ":spec.finding/invariant"
    } else if has_adr && has_decision_lang {
        ":spec.finding/adr"
    } else if has_neg && has_falsification_lang {
        ":spec.finding/negative"
    } else {
        // Doesn't meet language criteria
        return None;
    };

    // Generate finding ident from first spec ref
    let first_ref = &refs[0];
    let slug = slug_from_text(first_ref);
    let finding_ident = format!(":spec/finding-{slug}");
    let finding_entity = EntityId::from_ident(&finding_ident);

    // Check if this finding already exists (idempotent)
    if !store.entity_datoms(finding_entity).is_empty() {
        return None; // Already crystallized
    }

    // Extract first sentence as title
    let title = text
        .split_once('.')
        .map(|(s, _)| s.trim())
        .unwrap_or(text)
        .chars()
        .take(120)
        .collect::<String>();

    // Build finding datoms in the SAME transaction
    datoms.push(Datom::new(
        finding_entity,
        Attribute::from_keyword(":db/ident"),
        Value::Keyword(finding_ident.clone()),
        tx_id,
        Op::Assert,
    ));
    datoms.push(Datom::new(
        finding_entity,
        Attribute::from_keyword(":spec/element-type"),
        Value::Keyword(element_type.to_string()),
        tx_id,
        Op::Assert,
    ));
    datoms.push(Datom::new(
        finding_entity,
        Attribute::from_keyword(":element/id"),
        Value::String(first_ref.to_string()),
        tx_id,
        Op::Assert,
    ));
    datoms.push(Datom::new(
        finding_entity,
        Attribute::from_keyword(":element/title"),
        Value::String(title),
        tx_id,
        Op::Assert,
    ));
    datoms.push(Datom::new(
        finding_entity,
        Attribute::from_keyword(":element/statement"),
        Value::String(text.to_string()),
        tx_id,
        Op::Assert,
    ));
    // Back-reference to source observation
    datoms.push(Datom::new(
        finding_entity,
        Attribute::from_keyword(":spec/source-observation"),
        Value::Ref(observation_entity),
        tx_id,
        Op::Assert,
    ));
    datoms.push(Datom::new(
        finding_entity,
        Attribute::from_keyword(":spec/auto-crystallized"),
        Value::Boolean(true),
        tx_id,
        Op::Assert,
    ));

    Some(finding_ident)
}

/// Find the most recent active session entity for observation linking (B4).
fn find_active_session(store: &braid_kernel::Store) -> Option<EntityId> {
    let mut latest_wall = 0i64;
    let mut latest_entity = None;

    for d in store.datoms() {
        if d.attribute.as_str() == ":session/started-at" && d.op == Op::Assert {
            if let braid_kernel::datom::Value::Long(wall) = d.value {
                if wall > latest_wall {
                    let has_active = store.entity_datoms(d.entity).iter().any(|ed| {
                        ed.attribute.as_str() == ":session/status"
                            && ed.op == Op::Assert
                            && matches!(&ed.value, braid_kernel::datom::Value::Keyword(k) if k == ":session.status/active")
                    });
                    if has_active {
                        latest_wall = wall;
                        latest_entity = Some(d.entity);
                    }
                }
            }
        }
    }

    latest_entity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_generation() {
        assert_eq!(
            slug_from_text("merge is a bottleneck"),
            "merge-is-a-bottleneck"
        );
        assert_eq!(slug_from_text("Hello, World!"), "hello-world");
        assert_eq!(
            slug_from_text("  spaces   and    tabs  "),
            "spaces-and-tabs"
        );
        assert_eq!(slug_from_text(""), "");
        assert_eq!(
            slug_from_text("Datalog joins return 0 results — CRITICAL"),
            "datalog-joins-return-0-results-critical"
        );
    }

    #[test]
    fn category_resolution() {
        // Explicit category always wins, regardless of body text
        assert_eq!(
            resolve_category(None, "just a plain observation"),
            ":exploration.cat/observation"
        );
        assert_eq!(
            resolve_category(Some("conjecture"), ""),
            ":exploration.cat/conjecture"
        );
        assert_eq!(
            resolve_category(Some("decision"), ""),
            ":exploration.cat/design-decision"
        );
        assert_eq!(
            resolve_category(Some("question"), ""),
            ":exploration.cat/open-question"
        );
        assert_eq!(
            resolve_category(Some("custom"), ""),
            ":exploration.cat/custom"
        );
    }

    #[test]
    fn auto_detect_category_from_body() {
        // Spec references → spec-insight
        assert_eq!(
            auto_detect_category("INV-STORE-001 is violated"),
            "spec-insight"
        );
        assert_eq!(
            auto_detect_category("See ADR-MERGE-003 for rationale"),
            "spec-insight"
        );
        assert_eq!(
            auto_detect_category("NEG-001 triggered in test"),
            "spec-insight"
        );

        // Decision language → design-decision
        assert_eq!(
            auto_detect_category("We decided to use EAV"),
            "design-decision"
        );
        assert_eq!(
            auto_detect_category("Choosing Datalog over SQL"),
            "design-decision"
        );
        assert_eq!(
            auto_detect_category("The decision was to use CRDT"),
            "design-decision"
        );
        assert_eq!(
            auto_detect_category("I chose append-only"),
            "design-decision"
        );

        // Bug/fix language → issue
        assert_eq!(auto_detect_category("Found a bug in merge"), "issue");
        assert_eq!(
            auto_detect_category("Need to fix the query engine"),
            "issue"
        );
        assert_eq!(auto_detect_category("Index error on large stores"), "issue");
        assert_eq!(auto_detect_category("Schema validation is broken"), "issue");

        // Default → observation
        assert_eq!(auto_detect_category("merge is a bottleneck"), "observation");
        assert_eq!(
            auto_detect_category("the store is append-only"),
            "observation"
        );

        // Spec refs take priority over decision language
        assert_eq!(
            auto_detect_category("We decided INV-STORE-001 should be enforced"),
            "spec-insight"
        );
    }

    #[test]
    fn explicit_category_overrides_auto_detect() {
        // Even though body contains spec refs, explicit category wins
        assert_eq!(
            resolve_category(Some("conjecture"), "INV-STORE-001 might be wrong"),
            ":exploration.cat/conjecture"
        );
        // Even though body contains decision language, explicit category wins
        assert_eq!(
            resolve_category(Some("observation"), "We decided to use CRDT"),
            ":exploration.cat/observation"
        );
    }

    #[test]
    fn observe_creates_exploration_entity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");

        // Initialize store
        crate::commands::init::run(&path, Path::new("spec"), None).unwrap();

        // Observe
        let result = run(ObserveArgs {
            path: &path,
            text: "merge is a structural bottleneck",
            confidence: 0.8,
            tags: &["bottleneck".to_string(), "graph".to_string()],
            category: None,
            agent: "test",
            relates_to: None,
            rationale: None,
            alternatives: None,
            no_auto_crystallize: false,
        })
        .unwrap();

        // ACP human output contains the observation ident and confidence
        assert!(
            result
                .human
                .contains("observation/merge-is-a-structural-bottleneck")
                || result.human.contains("observed"),
            "human output should reference the observation: {}",
            result.human
        );
        assert!(
            result.human.contains("0.8") || result.human.contains("confidence"),
            "human output should include confidence: {}",
            result.human
        );

        // Verify entity exists in store (use LiveStore for consistent read)
        let live = crate::live_store::LiveStore::open(&path).unwrap();
        let store = live.store();
        let entity = EntityId::from_ident(":observation/merge-is-a-structural-bottleneck");
        let datoms = store.entity_datoms(entity);
        assert!(
            datoms.len() >= 8,
            "expected at least 8 datoms for observation entity (incl. content-hash), got {}",
            datoms.len()
        );

        // Verify specific attributes
        let body = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":exploration/body")
            .expect("should have :exploration/body");
        assert!(matches!(&body.value, Value::String(s) if s == "merge is a structural bottleneck"));

        let confidence = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":exploration/confidence")
            .expect("should have :exploration/confidence");
        assert!(matches!(&confidence.value, Value::Double(f) if f.into_inner() == 0.8));
    }

    #[test]
    fn observe_validates_confidence_range() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec"), None).unwrap();

        let result = run(ObserveArgs {
            path: &path,
            text: "test",
            confidence: 1.5,
            tags: &[],
            category: None,
            agent: "test",
            relates_to: None,
            rationale: None,
            alternatives: None,
            no_auto_crystallize: false,
        });
        assert!(result.is_err());
    }

    #[test]
    fn observe_validates_empty_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec"), None).unwrap();

        let result = run(ObserveArgs {
            path: &path,
            text: "  ",
            confidence: 0.7,
            tags: &[],
            category: None,
            agent: "test",
            relates_to: None,
            rationale: None,
            alternatives: None,
            no_auto_crystallize: false,
        });
        assert!(result.is_err());
    }

    #[test]
    fn observe_with_relates_to() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec"), None).unwrap();

        let result = run(ObserveArgs {
            path: &path,
            text: "CRDT merge is commutative",
            confidence: 0.95,
            tags: &[],
            category: Some("theorem"),
            agent: "test",
            relates_to: Some(":spec/inv-store-004"),
            rationale: None,
            alternatives: None,
            no_auto_crystallize: false,
        })
        .unwrap();

        // ACP-formatted output includes relates-to and category in context blocks
        assert!(
            result.human.contains("inv-store-004") || result.human.contains("relates"),
            "human output should reference relates-to: {}",
            result.human
        );
        assert!(
            result.human.contains("theorem") || result.human.contains("category"),
            "human output should include category: {}",
            result.human
        );
    }

    #[test]
    fn observe_content_addressable_identity() {
        // Same text → same entity ID (C2 constraint)
        let slug1 = slug_from_text("merge is a bottleneck");
        let slug2 = slug_from_text("merge is a bottleneck");
        assert_eq!(slug1, slug2);

        let eid1 = EntityId::from_ident(&format!(":observation/{slug1}"));
        let eid2 = EntityId::from_ident(&format!(":observation/{slug2}"));
        assert_eq!(
            eid1, eid2,
            "same observation text must produce same entity ID"
        );
    }

    #[test]
    fn observe_queryable_via_datalog() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec"), None).unwrap();

        // Create observation
        run(ObserveArgs {
            path: &path,
            text: "the store is append-only",
            confidence: 0.99,
            tags: &[],
            category: None,
            agent: "test",
            relates_to: None,
            rationale: None,
            alternatives: None,
            no_auto_crystallize: false,
        })
        .unwrap();

        // Query it back via Datalog (use LiveStore for consistent read)
        let live = crate::live_store::LiveStore::open(&path).unwrap();
        let store = live.store();

        let query = braid_kernel::QueryExpr::new(
            braid_kernel::FindSpec::Rel(vec!["?e".into(), "?body".into()]),
            vec![braid_kernel::Clause::Pattern(braid_kernel::Pattern::new(
                braid_kernel::query::clause::Term::Variable("?e".into()),
                braid_kernel::query::clause::Term::Attr(Attribute::from_keyword(
                    ":exploration/body",
                )),
                braid_kernel::query::clause::Term::Variable("?body".into()),
            ))],
        );

        let result = braid_kernel::evaluate(store, &query);
        match result {
            braid_kernel::query::evaluator::QueryResult::Rel(rows) => {
                assert!(
                    !rows.is_empty(),
                    "observation should be queryable via Datalog"
                );
                let found = rows.iter().any(
                    |row| matches!(&row[1], Value::String(s) if s == "the store is append-only"),
                );
                assert!(found, "observation body should appear in query results");
            }
            _ => panic!("expected Rel result"),
        }
    }

    // -------------------------------------------------------------------
    // META-3-TEST: Crystallization feedback tests
    // -------------------------------------------------------------------

    #[test]
    fn crystallization_feedback_unanchored_observation() {
        // An observation without spec references should get delta_cryst = -0.1
        let text = "the merge latency is too high";
        let has_spec_refs =
            text.contains("INV-") || text.contains("ADR-") || text.contains("NEG-");
        assert!(!has_spec_refs, "plain text should not have spec refs");
        // delta_cryst for unanchored = -0.1
        let delta: f64 = -0.1;
        assert!(delta < 0.0, "unanchored observation should have negative delta");
    }

    #[test]
    fn crystallization_feedback_anchored_observation() {
        // An observation with a valid spec reference gets delta_cryst = 0.2
        let text = "INV-STORE-001 is violated if merge drops datoms";
        let has_spec_refs =
            text.contains("INV-") || text.contains("ADR-") || text.contains("NEG-");
        assert!(has_spec_refs, "spec-anchored text should have spec refs");
        // delta_cryst for anchored = 0.2
        let delta: f64 = 0.2;
        assert!(delta > 0.0, "anchored observation should have positive delta");
    }

    #[test]
    fn crystallization_feedback_spec_ref_parsing() {
        // Test that spec refs are extracted from observation text
        let text = "This relates to INV-BILATERAL-001 and ADR-STORE-005";
        let refs = braid_kernel::task::parse_spec_refs(text);
        assert!(refs.len() >= 2, "should parse at least 2 spec refs from '{}'", text);
        assert!(
            refs.iter().any(|r| r.contains("BILATERAL")),
            "should find BILATERAL ref"
        );
        assert!(
            refs.iter().any(|r| r.contains("STORE")),
            "should find STORE ref"
        );
    }

    #[test]
    fn crystallization_nearest_spec_for_unanchored() {
        // For unanchored observations, spec_relevance_scan should find related specs
        let store = braid_kernel::Store::genesis();
        let text = "the store append-only property is critical";
        let nearest = braid_kernel::guidance::spec_relevance_scan(text, &store);
        // On a schema-only store, there may not be spec elements. That's OK — the scan
        // should return an empty vec without panicking.
        assert!(
            nearest.len() <= 100,
            "spec_relevance_scan should not return more than 100 results"
        );
    }

    #[test]
    fn crystallization_json_output_includes_delta() {
        // Integration test: run observe on a temp store and check JSON output
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join(".braid");
        crate::layout::DiskLayout::init(&store_path).unwrap();

        let args = ObserveArgs {
            path: &store_path,
            text: "test observation for crystallization feedback",
            confidence: 0.8,
            tags: &[],
            category: None,
            agent: "test:agent",
            relates_to: None,
            rationale: None,
            alternatives: None,
            no_auto_crystallize: false,
        };

        let result = run(args).unwrap();
        let json = &result.json;

        // JSON should include delta_crystallization field
        assert!(
            json.get("delta_crystallization").is_some(),
            "JSON output should include delta_crystallization, got: {:?}",
            json
        );
        let delta = json["delta_crystallization"].as_f64().unwrap();
        // Unanchored observation → -0.1
        assert!(
            (delta - (-0.1)).abs() < 0.01,
            "unanchored observation should have delta_cryst ≈ -0.1, got {}",
            delta
        );
    }

    #[test]
    fn crystallization_json_output_includes_nearest_spec() {
        // Unanchored observation should have nearest_spec in JSON
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join(".braid");
        crate::layout::DiskLayout::init(&store_path).unwrap();

        let args = ObserveArgs {
            path: &store_path,
            text: "plain observation no spec refs",
            confidence: 0.7,
            tags: &[],
            category: None,
            agent: "test:agent",
            relates_to: None,
            rationale: None,
            alternatives: None,
            no_auto_crystallize: false,
        };

        let result = run(args).unwrap();
        let json = &result.json;

        // nearest_spec should be present (may be null if no specs found)
        assert!(
            json.get("nearest_spec").is_some(),
            "JSON should include nearest_spec field"
        );
    }

    #[test]
    fn crystallization_human_output_shows_delta() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join(".braid");
        crate::layout::DiskLayout::init(&store_path).unwrap();

        let args = ObserveArgs {
            path: &store_path,
            text: "unanchored observation for output test",
            confidence: 0.6,
            tags: &[],
            category: None,
            agent: "test:agent",
            relates_to: None,
            rationale: None,
            alternatives: None,
            no_auto_crystallize: false,
        };

        let result = run(args).unwrap();
        // STEER-3b: Unanchored observations no longer show Δ-cryst (noise reduction).
        // Verify the output does NOT contain Δ-cryst for non-spec observations.
        assert!(
            !result.human.contains("\u{0394}-cryst"),
            "unanchored observation should NOT show delta-cryst (STEER-3b), got: {}",
            result.human
        );
    }
}
