//! `braid spec` — Spec element creation and proposal review (WP5, W4B.3).
//!
//! Subcommands:
//! - `create`: Zero-friction spec element creation. Auto-detects type from ID prefix.
//! - `review`: List pending proposals (confidence < 0.9) awaiting human review.
//! - `accept <id>`: Accept a proposal, promoting it to a first-class spec element.
//! - `reject <id> --reason "..."`: Reject a proposal with rationale.
//! - `history`: Show all proposals with their lifecycle status.
//!
//! Traces to: C5 (traceability), C6 (falsifiability), INV-INTERFACE-011 (CLI as prompt).

use std::path::Path;

use braid_kernel::bilateral::set_depth_datom;
use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, Value};
use braid_kernel::layout::TxFile;
use braid_kernel::proposal;
use braid_kernel::Store;

use crate::error::BraidError;
use crate::live_store::LiveStore;
use crate::output::{AgentOutput, CommandOutput};

/// Arguments for `braid spec create`.
pub struct CreateArgs<'a> {
    pub path: &'a Path,
    pub id: &'a str,
    pub title: &'a str,
    pub statement: Option<&'a str>,
    pub falsification: Option<&'a str>,
    pub problem: Option<&'a str>,
    pub decision: Option<&'a str>,
    pub traces_to: Option<&'a str>,
    pub confidence: Option<f64>,
    pub agent: &'a str,
    /// Suppress auto-task generation (COTX-3).
    pub no_auto_task: bool,
    /// WRITER-3: Pre-opened LiveStore from main.rs (zero deserialization).
    pub pre_opened: Option<&'a mut LiveStore>,
}

/// Run `braid spec create`.
pub fn run_create(args: CreateArgs<'_>) -> Result<CommandOutput, BraidError> {
    // Auto-detect type from ID prefix
    let element_type = if args.id.starts_with("INV-") {
        "invariant"
    } else if args.id.starts_with("ADR-") {
        "adr"
    } else if args.id.starts_with("NEG-") {
        "negative-case"
    } else {
        return Err(BraidError::Validation(format!(
            "Spec ID must start with INV-, ADR-, or NEG-. Got: {}",
            args.id
        )));
    };

    // Extract namespace from ID: INV-STORE-001 → STORE
    let namespace = extract_namespace(args.id).ok_or_else(|| {
        BraidError::Validation(format!(
            "Cannot extract namespace from ID: {}. Expected format: INV-NAMESPACE-NNN",
            args.id
        ))
    })?;

    // WRITER-3: Use pre-opened LiveStore if available, else open fresh.
    let mut fallback;
    let live = match args.pre_opened {
        Some(l) => l,
        None => {
            fallback = LiveStore::open(args.path)?;
            &mut fallback
        }
    };

    let agent_id = AgentId::from_name(args.agent);
    let tx_id = super::write::next_tx_id(live.store(), agent_id);

    // Build entity ident: :spec/inv-store-001 (lowercase)
    let ident = format!(":spec/{}", args.id.to_lowercase());
    let entity = EntityId::from_ident(&ident);

    let mut datoms = vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident.clone()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":spec/id"),
            Value::String(args.id.to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":spec/element-type"),
            Value::Keyword(format!(":spec.type/{element_type}")),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":spec/namespace"),
            Value::Keyword(format!(":spec.ns/{namespace}")),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(args.title.to_string()),
            tx_id,
            Op::Assert,
        ),
    ];

    // Type-specific attributes
    if let Some(stmt) = args.statement {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":spec/statement"),
            Value::String(stmt.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    // DC-3: Auto-depth on spec create.
    // With --falsification: depth=1 (HYPOTHESIS) + falsification criterion entity.
    // Without: depth=0 (OPINION) + C6 warning.
    let c6_warning = if let Some(fals) = args.falsification {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":spec/falsification"),
            Value::String(fals.to_string()),
            tx_id,
            Op::Assert,
        ));

        // Create falsification criterion entity (same pattern as challenge.rs)
        let crit_entity = EntityId::from_ident(&format!(
            ":falsification/{}-{}",
            args.id.to_lowercase().replace('/', "-"),
            fals.chars().take(30).collect::<String>().replace(' ', "-")
        ));
        datoms.push(Datom::new(
            crit_entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(fals.to_string()),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":comonad/falsification"),
            Value::Ref(crit_entity),
            tx_id,
            Op::Assert,
        ));

        // Depth 1 = HYPOTHESIS
        datoms.push(set_depth_datom(&entity, 1, tx_id));
        false
    } else {
        // Depth 0 = OPINION (no falsification condition)
        datoms.push(set_depth_datom(&entity, 0, tx_id));
        true
    };

    if let Some(prob) = args.problem {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":adr/problem"),
            Value::String(prob.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    if let Some(dec) = args.decision {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":adr/decision"),
            Value::String(dec.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    if let Some(traces) = args.traces_to {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":element/traces-to"),
            Value::String(traces.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    if let Some(conf) = args.confidence {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":spec/confidence"),
            Value::Double(ordered_float::OrderedFloat(conf)),
            tx_id,
            Op::Assert,
        ));
    }

    // COTX-3: Auto-generate implementation task in the same transaction
    let auto_task_id = if !args.no_auto_task {
        let task_title = format!("Implement {}: {}", args.id, args.title);
        let task_id = braid_kernel::task::generate_task_id(&task_title);
        let task_ident = format!(":task/{task_id}");
        let task_entity = EntityId::from_ident(&task_ident);

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
            Value::String(task_id.clone()),
            tx_id,
            Op::Assert,
        ));
        datoms.push(Datom::new(
            task_entity,
            Attribute::from_keyword(":task/title"),
            Value::String(braid_kernel::task::short_title(&task_title).to_string()),
            tx_id,
            Op::Assert,
        ));
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
        // Link task to the spec element
        datoms.push(Datom::new(
            task_entity,
            Attribute::from_keyword(":task/traces-to"),
            Value::Ref(entity),
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

        Some(task_id)
    } else {
        None
    };

    let datom_count = datoms.len();

    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("spec create: {} \"{}\"", args.id, args.title),
        causal_predecessors: vec![],
        datoms,
    };

    live.write_tx(&tx)?;

    // LiveStore already has the update — no reload needed
    let store = live.store();
    let total = store.datom_set().len();

    let task_suffix = if let Some(ref tid) = auto_task_id {
        format!(" + auto-task {tid}")
    } else {
        String::new()
    };
    let depth = if c6_warning { 0 } else { 1 };
    let depth_label = if c6_warning { "OPINION" } else { "HYPOTHESIS" };
    let c6_line = if c6_warning {
        "\n\u{26a0} C6: no falsification condition \u{2014} entity starts at depth 0 (OPINION)\n"
    } else {
        ""
    };
    let human = format!(
        "created: {} ({}, namespace={}, depth={} {}){task_suffix}\nstore: +{} datoms ({} total)\n{c6_line}\nnext: braid trace --commit (to link implementations) | ref: C5 traceability\n",
        ident, element_type, namespace, depth, depth_label, datom_count, total,
    );

    let mut json = serde_json::json!({
        "ident": ident,
        "id": args.id,
        "element_type": element_type,
        "namespace": namespace,
        "datom_count": datom_count,
        "store_total": total,
        "auto_task_id": auto_task_id,
        "comonad_depth": depth,
        "comonad_depth_label": depth_label,
        "c6_warning": c6_warning,
    });

    // ACP: after creating a spec element, check store state
    let projection = braid_kernel::ActionProjection {
        action: braid_kernel::budget::ProjectedAction {
            command: "braid status".to_string(),
            rationale: "check spec state after creation".to_string(),
            impact: 0.4,
        },
        context: vec![braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::System,
            format!("created: {} ({}, ns={})", ident, element_type, namespace),
            8,
        )],
        evidence_pointer: format!("details: braid query --entity {ident}"),
    };

    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // Merge ACP into JSON
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

// ---------------------------------------------------------------------------
// review — list pending proposals (W4B.3)
// ---------------------------------------------------------------------------

/// Run `braid spec review`: list pending proposals awaiting human review.
///
/// Queries the store for all proposals with status `:proposal.status/proposed`
/// and confidence below the auto-accept threshold (0.9). Sorted by confidence
/// descending (highest first).
pub fn run_review(
    path: &Path,
    pre_opened: Option<&mut LiveStore>,
) -> Result<CommandOutput, BraidError> {
    // WRITER-3: Use pre-opened LiveStore if available, else open fresh.
    let mut fallback;
    let live = match pre_opened {
        Some(l) => l,
        None => {
            fallback = LiveStore::open(path)?;
            &mut fallback
        }
    };
    let store = live.store();

    let pending = proposal::pending_proposals(store);
    let threshold = proposal::auto_accept_threshold();

    if pending.is_empty() {
        let human = "No pending proposals.\n\nnext: braid harvest --commit (to generate proposals from observations)\n".to_string();
        return Ok(CommandOutput {
            json: serde_json::json!({ "proposals": [], "count": 0 }),
            agent: AgentOutput {
                context: "spec review: 0 pending proposals".to_string(),
                content: "No proposals awaiting review.".to_string(),
                footer: "generate: braid harvest --commit | ref: W4B.3 proposal review".to_string(),
            },
            human,
        });
    }

    // Build structured data for all three output modes.
    let mut json_proposals = Vec::new();
    let mut human_lines = Vec::new();
    let mut agent_lines = Vec::new();

    human_lines.push(format!(
        "Pending proposals: {} (auto-accept threshold: {:.1})\n",
        pending.len(),
        threshold
    ));

    for (i, (entity, suggested_id, confidence)) in pending.iter().enumerate() {
        let entity_hex = format_entity_short(store, *entity);
        let statement = extract_proposal_field(store, *entity, ":proposal/statement");
        let ptype = extract_proposal_field(store, *entity, ":proposal/type");
        let traces_to = extract_proposal_field(store, *entity, ":proposal/traces-to");
        let auto_eligible = *confidence >= threshold;

        json_proposals.push(serde_json::json!({
            "index": i + 1,
            "entity": entity_hex,
            "suggested_id": suggested_id,
            "confidence": confidence,
            "type": ptype,
            "statement": statement,
            "traces_to": traces_to,
            "auto_eligible": auto_eligible,
        }));

        let type_label = ptype
            .as_deref()
            .unwrap_or("unknown")
            .strip_prefix(":proposal.type/")
            .unwrap_or("unknown");
        let auto_tag = if auto_eligible {
            " [auto-eligible]"
        } else {
            ""
        };

        human_lines.push(format!(
            "  {}. {} ({}, confidence={:.2}){}\n     {}\n     entity: {}\n",
            i + 1,
            suggested_id,
            type_label,
            confidence,
            auto_tag,
            statement.as_deref().unwrap_or("(no statement)"),
            entity_hex,
        ));

        agent_lines.push(format!(
            "{}. {} ({}, c={:.2}){}: {}",
            i + 1,
            suggested_id,
            type_label,
            confidence,
            auto_tag,
            truncate(statement.as_deref().unwrap_or(""), 80),
        ));
    }

    let human = human_lines.join("")
        + "\nnext: braid spec accept <entity> | braid spec reject <entity> --reason \"...\"\n";

    let agent_content = agent_lines.join("\n");

    // ACP: action = accept/reject the first pending proposal
    let first_entity = pending
        .first()
        .map(|(e, _, _)| format_entity_short(store, *e))
        .unwrap_or_default();
    let projection = braid_kernel::ActionProjection {
        action: braid_kernel::budget::ProjectedAction {
            command: format!("braid spec accept {first_entity}"),
            rationale: format!("{} proposals awaiting review", pending.len()),
            impact: 0.5,
        },
        context: vec![braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::System,
            format!(
                "spec review: {} pending (threshold={:.1})",
                pending.len(),
                threshold
            ),
            8,
        )],
        evidence_pointer: "list: braid spec review | history: braid spec history".to_string(),
    };

    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent_out = AgentOutput {
        context: String::new(),
        content: format!("{agent_content}\n{agent_text}"),
        footer: String::new(),
    };

    let mut json = serde_json::json!({
        "proposals": json_proposals,
        "count": pending.len(),
        "auto_accept_threshold": threshold,
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput {
        json,
        agent: agent_out,
        human,
    })
}

// ---------------------------------------------------------------------------
// accept — promote a proposal to a spec element (W4B.3)
// ---------------------------------------------------------------------------

/// Run `braid spec accept <id>`: accept a pending proposal.
///
/// Finds the proposal entity by matching `<id>` against either the entity hex
/// prefix or the `:proposal/suggested-id`. Transitions status to accepted and
/// generates `:spec/*` datoms via promotion.
pub fn run_accept(
    path: &Path,
    id: &str,
    agent: &str,
    pre_opened: Option<&mut LiveStore>,
) -> Result<CommandOutput, BraidError> {
    // WRITER-3: Use pre-opened LiveStore if available, else open fresh.
    let mut fallback;
    let live = match pre_opened {
        Some(l) => l,
        None => {
            fallback = LiveStore::open(path)?;
            &mut fallback
        }
    };
    let store = live.store();

    let proposal_entity = resolve_proposal_entity(store, id)?;

    let agent_id = AgentId::from_name(agent);
    let tx_id = super::write::next_tx_id(store, agent_id);

    let accept_datoms = proposal::accept_proposal(store, proposal_entity, tx_id);
    if accept_datoms.is_empty() {
        return Err(BraidError::Validation(format!(
            "Cannot accept proposal '{}': entity not found, already accepted, or already rejected.",
            id
        )));
    }

    let suggested_id = extract_proposal_field(store, proposal_entity, ":proposal/suggested-id")
        .unwrap_or_else(|| id.to_string());
    let entity_hex = format_entity_short(store, proposal_entity);

    let datom_count = accept_datoms.len();
    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("spec accept: {} ({})", suggested_id, entity_hex),
        causal_predecessors: vec![],
        datoms: accept_datoms,
    };

    live.write_tx(&tx)?;

    // LiveStore already has the update — no reload needed
    let store = live.store();
    let total = store.datom_set().len();

    let human = format!(
        "accepted: {} (entity: {})\npromoted to spec element with {} datoms\nstore: {} total datoms\n\nnext: braid trace --commit | braid status\n",
        suggested_id, entity_hex, datom_count, total,
    );

    // ACP: after accepting, check store state
    let projection = braid_kernel::ActionProjection {
        action: braid_kernel::budget::ProjectedAction {
            command: "braid status".to_string(),
            rationale: "check spec state after acceptance".to_string(),
            impact: 0.4,
        },
        context: vec![braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::System,
            format!(
                "accepted: {} promoted (+{} datoms)",
                suggested_id, datom_count
            ),
            8,
        )],
        evidence_pointer: "review: braid spec review | trace: braid trace --commit".to_string(),
    };

    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent_out = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    let mut json = serde_json::json!({
        "action": "accepted",
        "suggested_id": suggested_id,
        "entity": entity_hex,
        "datoms_added": datom_count,
        "store_total": total,
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput {
        json,
        agent: agent_out,
        human,
    })
}

// ---------------------------------------------------------------------------
// reject — reject a proposal with rationale (W4B.3)
// ---------------------------------------------------------------------------

/// Run `braid spec reject <id> --reason "..."`: reject a pending proposal.
///
/// Transitions the proposal status to rejected and records the rationale.
pub fn run_reject(
    path: &Path,
    id: &str,
    reason: &str,
    agent: &str,
    pre_opened: Option<&mut LiveStore>,
) -> Result<CommandOutput, BraidError> {
    // WRITER-3: Use pre-opened LiveStore if available, else open fresh.
    let mut fallback;
    let live = match pre_opened {
        Some(l) => l,
        None => {
            fallback = LiveStore::open(path)?;
            &mut fallback
        }
    };
    let store = live.store();

    let proposal_entity = resolve_proposal_entity(store, id)?;

    let agent_id = AgentId::from_name(agent);
    let tx_id = super::write::next_tx_id(store, agent_id);
    let reviewer = EntityId::from_ident(&format!(":agent/{}", agent));

    let reject_datoms = proposal::reject_proposal(proposal_entity, reason, reviewer, tx_id);

    let suggested_id = extract_proposal_field(store, proposal_entity, ":proposal/suggested-id")
        .unwrap_or_else(|| id.to_string());
    let entity_hex = format_entity_short(store, proposal_entity);

    let datom_count = reject_datoms.len();
    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("spec reject: {} — {}", suggested_id, reason),
        causal_predecessors: vec![],
        datoms: reject_datoms,
    };

    live.write_tx(&tx)?;

    let human = format!(
        "rejected: {} (entity: {})\nreason: {}\n\nnext: braid spec review | braid spec history\n",
        suggested_id, entity_hex, reason,
    );

    // ACP: after rejection, review remaining proposals
    let projection = braid_kernel::ActionProjection {
        action: braid_kernel::budget::ProjectedAction {
            command: "braid spec review".to_string(),
            rationale: "review remaining proposals after rejection".to_string(),
            impact: 0.4,
        },
        context: vec![braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::System,
            format!("rejected: {}", suggested_id),
            5,
        )],
        evidence_pointer: "history: braid spec history | status: braid status".to_string(),
    };

    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent_out = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    let mut json = serde_json::json!({
        "action": "rejected",
        "suggested_id": suggested_id,
        "entity": entity_hex,
        "reason": reason,
        "datoms_added": datom_count,
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput {
        json,
        agent: agent_out,
        human,
    })
}

// ---------------------------------------------------------------------------
// history — show all proposals with lifecycle status (W4B.3)
// ---------------------------------------------------------------------------

/// Run `braid spec history`: show all proposals (accepted, rejected, pending).
///
/// Queries every entity with a `:proposal/status` attribute and displays its
/// full lifecycle status. Sorted by transaction time (newest first).
pub fn run_history(
    path: &Path,
    pre_opened: Option<&mut LiveStore>,
) -> Result<CommandOutput, BraidError> {
    // WRITER-3: Use pre-opened LiveStore if available, else open fresh.
    let mut fallback;
    let live = match pre_opened {
        Some(l) => l,
        None => {
            fallback = LiveStore::open(path)?;
            &mut fallback
        }
    };
    let store = live.store();

    let status_attr = Attribute::from_keyword(":proposal/status");
    let status_datoms = store.attribute_datoms(&status_attr);

    // Collect unique proposal entities from status datoms.
    let mut seen = std::collections::BTreeSet::new();
    let mut entities: Vec<EntityId> = Vec::new();
    for d in status_datoms.iter() {
        if d.op == Op::Assert && seen.insert(d.entity) {
            entities.push(d.entity);
        }
    }

    if entities.is_empty() {
        let human = "No proposals found.\n\nnext: braid harvest --commit (to generate proposals)\n"
            .to_string();
        return Ok(CommandOutput {
            json: serde_json::json!({ "proposals": [], "count": 0 }),
            agent: AgentOutput {
                context: "spec history: 0 proposals".to_string(),
                content: "No proposals in the store.".to_string(),
                footer: "generate: braid harvest --commit".to_string(),
            },
            human,
        });
    }

    // Build proposal records with latest status for each entity.
    struct ProposalRecord {
        entity_hex: String,
        suggested_id: String,
        confidence: f64,
        ptype: String,
        status: String,
        statement: Option<String>,
        review_note: Option<String>,
        latest_tx_wall: u64,
    }

    let mut records: Vec<ProposalRecord> = Vec::new();

    for entity in &entities {
        let edatoms = store.entity_datoms(*entity);

        let latest_status = edatoms
            .iter()
            .filter(|d| d.attribute.as_str() == ":proposal/status" && d.op == Op::Assert)
            .max_by_key(|d| d.tx.wall_time)
            .and_then(|d| {
                if let Value::Keyword(ref k) = d.value {
                    Some((k.clone(), d.tx.wall_time))
                } else {
                    None
                }
            });

        let (status, latest_tx_wall) = match latest_status {
            Some((s, t)) => (s, t),
            None => continue,
        };

        let suggested_id =
            extract_proposal_field(store, *entity, ":proposal/suggested-id").unwrap_or_default();
        let confidence = edatoms
            .iter()
            .find_map(|d| {
                if d.attribute.as_str() == ":proposal/confidence" && d.op == Op::Assert {
                    if let Value::Double(ordered_float::OrderedFloat(c)) = d.value {
                        return Some(c);
                    }
                }
                None
            })
            .unwrap_or(0.0);
        let ptype = extract_proposal_field(store, *entity, ":proposal/type").unwrap_or_default();
        let statement = extract_proposal_field(store, *entity, ":proposal/statement");
        let review_note = extract_proposal_field(store, *entity, ":proposal/review-note");
        let entity_hex = format_entity_short(store, *entity);

        records.push(ProposalRecord {
            entity_hex,
            suggested_id,
            confidence,
            ptype,
            status,
            statement,
            review_note,
            latest_tx_wall,
        });
    }

    // Sort by latest tx wall time descending (newest first).
    records.sort_by_key(|r| std::cmp::Reverse(r.latest_tx_wall));

    // Count by status.
    let mut n_proposed = 0usize;
    let mut n_accepted = 0usize;
    let mut n_rejected = 0usize;
    for r in &records {
        match r.status.as_str() {
            ":proposal.status/proposed" => n_proposed += 1,
            ":proposal.status/accepted" => n_accepted += 1,
            ":proposal.status/rejected" => n_rejected += 1,
            _ => {}
        }
    }

    let mut json_proposals = Vec::new();
    let mut human_lines = Vec::new();
    let mut agent_lines = Vec::new();

    human_lines.push(format!(
        "Proposal history: {} total ({} pending, {} accepted, {} rejected)\n\n",
        records.len(),
        n_proposed,
        n_accepted,
        n_rejected,
    ));

    for (i, r) in records.iter().enumerate() {
        let status_label = r
            .status
            .strip_prefix(":proposal.status/")
            .unwrap_or(&r.status);
        let type_label = r.ptype.strip_prefix(":proposal.type/").unwrap_or(&r.ptype);
        let status_icon = match status_label {
            "proposed" => "[?]",
            "accepted" => "[+]",
            "rejected" => "[-]",
            _ => "[.]",
        };

        json_proposals.push(serde_json::json!({
            "index": i + 1,
            "entity": r.entity_hex,
            "suggested_id": r.suggested_id,
            "confidence": r.confidence,
            "type": r.ptype,
            "status": r.status,
            "statement": r.statement,
            "review_note": r.review_note,
        }));

        let note_suffix = match &r.review_note {
            Some(note) => format!("\n     note: {}", note),
            None => String::new(),
        };

        human_lines.push(format!(
            "  {} {}. {} ({}, c={:.2}) — {}\n     {}\n     entity: {}{}\n",
            status_icon,
            i + 1,
            r.suggested_id,
            type_label,
            r.confidence,
            status_label,
            r.statement.as_deref().unwrap_or("(no statement)"),
            r.entity_hex,
            note_suffix,
        ));

        agent_lines.push(format!(
            "{} {} ({}, c={:.2}) — {}",
            status_icon, r.suggested_id, type_label, r.confidence, status_label,
        ));
    }

    let human = human_lines.join("")
        + "\nnext: braid spec review (pending only) | braid spec accept/reject <entity>\n";
    let agent_content = agent_lines.join("\n");

    // ACP: action depends on whether there are pending proposals
    let (acp_command, acp_rationale) = if n_proposed > 0 {
        (
            "braid spec review".to_string(),
            format!("{n_proposed} proposals still pending review"),
        )
    } else {
        (
            "braid status".to_string(),
            "all proposals resolved".to_string(),
        )
    };
    let projection = braid_kernel::ActionProjection {
        action: braid_kernel::budget::ProjectedAction {
            command: acp_command,
            rationale: acp_rationale,
            impact: 0.3,
        },
        context: vec![braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::System,
            format!(
                "history: {} total ({} pending, {} accepted, {} rejected)",
                records.len(),
                n_proposed,
                n_accepted,
                n_rejected,
            ),
            10,
        )],
        evidence_pointer: "review: braid spec review | status: braid status".to_string(),
    };

    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent_out = AgentOutput {
        context: String::new(),
        content: format!("{agent_content}\n{agent_text}"),
        footer: String::new(),
    };

    let mut json = serde_json::json!({
        "proposals": json_proposals,
        "count": records.len(),
        "proposed": n_proposed,
        "accepted": n_accepted,
        "rejected": n_rejected,
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput {
        json,
        agent: agent_out,
        human,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a user-provided proposal identifier to an EntityId.
///
/// Matches against:
/// 1. Entity hex prefix (e.g., "a1b2c3d4" matches an entity starting with those bytes)
/// 2. Suggested ID (e.g., "INV-STORE-017")
/// 3. Exact entity hex (full 64-char hex string)
fn resolve_proposal_entity(store: &Store, id: &str) -> Result<EntityId, BraidError> {
    let status_attr = Attribute::from_keyword(":proposal/status");
    let status_datoms = store.attribute_datoms(&status_attr);

    // Collect unique proposal entities.
    let mut seen = std::collections::BTreeSet::new();
    let mut proposal_entities: Vec<EntityId> = Vec::new();
    for d in status_datoms.iter() {
        if d.op == Op::Assert && seen.insert(d.entity) {
            proposal_entities.push(d.entity);
        }
    }

    // Try matching by suggested-id first (most natural).
    for entity in &proposal_entities {
        if let Some(sid) = extract_proposal_field(store, *entity, ":proposal/suggested-id") {
            if sid == id || sid.eq_ignore_ascii_case(id) {
                return Ok(*entity);
            }
        }
    }

    // Try matching by entity hex prefix.
    let id_lower = id.to_lowercase();
    let mut hex_matches: Vec<EntityId> = Vec::new();
    for entity in &proposal_entities {
        let hex = encode_hex(entity.as_bytes());
        if hex.starts_with(&id_lower) || hex == id_lower {
            hex_matches.push(*entity);
        }
    }

    match hex_matches.len() {
        0 => Err(BraidError::Validation(format!(
            "No proposal found matching '{}'. Run 'braid spec review' to list pending proposals.",
            id
        ))),
        1 => Ok(hex_matches[0]),
        n => Err(BraidError::Validation(format!(
            "Ambiguous proposal identifier '{}': matches {} entities. Use a longer hex prefix or the suggested ID.",
            id, n
        ))),
    }
}

/// Extract a string-valued proposal field from an entity's datoms.
fn extract_proposal_field(store: &Store, entity: EntityId, attr: &str) -> Option<String> {
    store.entity_datoms(entity).into_iter().find_map(|d| {
        if d.attribute.as_str() == attr && d.op == Op::Assert {
            match &d.value {
                Value::String(s) => Some(s.clone()),
                Value::Keyword(k) => Some(k.clone()),
                _ => None,
            }
        } else {
            None
        }
    })
}

/// Format an entity as a short hex string for display.
///
/// Tries `:db/ident` first (e.g., `:spec/inv-store-001`), falls back to
/// truncated hex (8 chars).
fn format_entity_short(store: &Store, entity: EntityId) -> String {
    for datom in store.entity_datoms(entity) {
        if datom.attribute.as_str() == ":db/ident" {
            if let Value::Keyword(kw) = &datom.value {
                return kw.clone();
            }
        }
    }
    let bytes = entity.as_bytes();
    format!(
        "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]
    )
}

/// Encode bytes as lowercase hexadecimal string.
fn encode_hex(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX_CHARS[(b >> 4) as usize] as char);
        s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
    }
    s
}

/// Truncate a string to at most `max_len` bytes, appending "..." if truncated.
///
/// Uses [`braid_kernel::budget::safe_truncate_bytes`] to avoid panics on
/// multi-byte UTF-8 characters (em-dash, smart quotes, emoji, etc.).
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let truncated = braid_kernel::budget::safe_truncate_bytes(s, max_len.saturating_sub(3));
        format!("{truncated}...")
    }
}

/// Extract namespace from spec ID: INV-STORE-001 → STORE
fn extract_namespace(id: &str) -> Option<String> {
    // Skip prefix (INV-, ADR-, NEG-)
    let rest = if id.starts_with("INV-") || id.starts_with("ADR-") || id.starts_with("NEG-") {
        &id[4..]
    } else {
        return None;
    };

    // Find the namespace (uppercase letters before the next hyphen+digits)
    let ns_end = rest.find(|c: char| !c.is_ascii_uppercase() && c != '_');
    if let Some(end) = ns_end {
        if end > 0 && rest[end..].starts_with('-') {
            return Some(rest[..end].to_string());
        }
    }

    // If no digits follow, the whole rest is the namespace (e.g., "INV-STORE")
    if rest.chars().all(|c| c.is_ascii_uppercase() || c == '_') && !rest.is_empty() {
        Some(rest.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_namespace_from_inv() {
        assert_eq!(
            extract_namespace("INV-STORE-001"),
            Some("STORE".to_string())
        );
    }

    #[test]
    fn extract_namespace_from_adr() {
        assert_eq!(
            extract_namespace("ADR-INTERFACE-010"),
            Some("INTERFACE".to_string())
        );
    }

    #[test]
    fn extract_namespace_from_neg() {
        assert_eq!(
            extract_namespace("NEG-MUTATION-001"),
            Some("MUTATION".to_string())
        );
    }

    #[test]
    fn extract_namespace_multi_word() {
        assert_eq!(
            extract_namespace("INV-BILATERAL-005"),
            Some("BILATERAL".to_string())
        );
    }

    #[test]
    fn extract_namespace_invalid() {
        assert_eq!(extract_namespace("FOO-STORE-001"), None);
        assert_eq!(extract_namespace("INV-"), None);
    }
}
