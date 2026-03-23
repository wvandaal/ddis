//! `braid challenge` — Comonadic duplicate made executable (DC-4).
//!
//! Takes a hypothesis/claim entity, examines its assertions, generates
//! falsification criteria (contrapositives), tracks survival/falsification
//! outcomes, and increments comonadic depth on survival.
//!
//! The challenge protocol maps to the comonadic `duplicate` operation:
//!   H → ¬H → ¬¬H → ... converging to a fixed point.
//!
//! Depth levels:
//!   0 = OPINION (assertion only, no falsification context)
//!   1 = HYPOTHESIS (assertion + falsification criteria registered)
//!   2 = TESTED (challenge attempted)
//!   3 = SURVIVED (challenge survived, dialectically deepened)
//!   4 = KNOWLEDGE (formal proof / comonadic fixed point)
//!
//! Traces to: ADR-FOUNDATION-020, INV-FOUNDATION-008

use std::path::Path;

use braid_kernel::bilateral::{comonadic_depth, depth_weight, set_depth_datom};
use braid_kernel::datom::{
    AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value,
};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::CommandOutput;

/// Run `braid challenge <entity> [--survive|--falsify|--register]`.
///
/// Three modes:
/// - `--register`: Register falsification criteria (depth 0→1)
/// - Default (no flag): Attempt a challenge (depth 1→2)
/// - `--survive`: Record survival (depth 2→3)
/// - `--falsify`: Record falsification (depth resets to 0, retract claim)
pub fn run(
    path: &Path,
    entity_id: &str,
    survive: bool,
    falsify: bool,
    register: bool,
    criteria: &[String],
    agent_name: &str,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    let agent = AgentId::from_name(agent_name);

    // Resolve the target entity
    let entity = EntityId::from_ident(entity_id);

    // Check entity exists in store
    let entity_datoms = store.entity_datoms(entity);
    if entity_datoms.is_empty() {
        return Err(BraidError::Validation(format!(
            "entity {entity_id} not found in store"
        )));
    }

    let current_depth = comonadic_depth(&store, &entity);
    let current_weight = depth_weight(current_depth);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let tx = TxId::new(now, 0, agent);

    let mut datoms = Vec::new();
    let mut out = String::new();
    let new_depth;

    if register {
        // Register falsification criteria: depth 0→1 (OPINION→HYPOTHESIS)
        if current_depth > 0 && criteria.is_empty() {
            out.push_str(&format!(
                "entity already at depth {} ({}), use --survive or default challenge\n",
                current_depth,
                depth_label(current_depth)
            ));
            new_depth = current_depth;
        } else {
            new_depth = 1.max(current_depth); // don't decrease depth
            datoms.push(set_depth_datom(&entity, new_depth, tx));

            // Record falsification criteria as refs
            for criterion in criteria {
                let crit_entity = EntityId::from_ident(&format!(
                    ":falsification/{}-{}",
                    &entity_id.replace(':', "").replace('/', "-"),
                    criterion
                        .chars()
                        .take(30)
                        .collect::<String>()
                        .replace(' ', "-")
                ));
                // Create the criterion entity with description
                datoms.push(Datom::new(
                    crit_entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String(criterion.clone()),
                    tx,
                    Op::Assert,
                ));
                // Link from target to criterion
                datoms.push(Datom::new(
                    entity,
                    Attribute::from_keyword(":comonad/falsification"),
                    Value::Ref(crit_entity),
                    tx,
                    Op::Assert,
                ));
            }

            out.push_str(&format!(
                "registered: {} → depth {} ({}) with {} falsification criteria\n",
                entity_id,
                new_depth,
                depth_label(new_depth),
                criteria.len()
            ));
        }
    } else if falsify {
        // Falsification: reset depth to 0, record the event
        new_depth = 0;
        datoms.push(set_depth_datom(&entity, new_depth, tx));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":comonad/last-challenged"),
            Value::Instant(now),
            tx,
            Op::Assert,
        ));
        // Record survival rate = 0.0 (falsified)
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":comonad/survival-rate"),
            Value::Double(ordered_float::OrderedFloat(0.0)),
            tx,
            Op::Assert,
        ));
        out.push_str(&format!(
            "falsified: {} → depth {} ({}) — claim failed challenge\n",
            entity_id,
            new_depth,
            depth_label(new_depth)
        ));
    } else if survive {
        // Survival: increment depth (2→3 or 3→4), record the event
        new_depth = (current_depth + 1).min(4);
        datoms.push(set_depth_datom(&entity, new_depth, tx));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":comonad/last-challenged"),
            Value::Instant(now),
            tx,
            Op::Assert,
        ));
        out.push_str(&format!(
            "survived: {} → depth {} ({}) — challenge passed\n",
            entity_id,
            new_depth,
            depth_label(new_depth)
        ));
    } else {
        // Default: attempt a challenge (depth → max(2, current+1))
        new_depth = current_depth.max(2).min(4);
        datoms.push(set_depth_datom(&entity, new_depth, tx));
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":comonad/last-challenged"),
            Value::Instant(now),
            tx,
            Op::Assert,
        ));

        // Show falsification criteria if any exist
        let falsification_attr = Attribute::from_keyword(":comonad/falsification");
        let crit_refs: Vec<EntityId> = entity_datoms
            .iter()
            .filter(|d| d.attribute == falsification_attr && d.op == Op::Assert)
            .filter_map(|d| {
                if let Value::Ref(r) = &d.value {
                    Some(*r)
                } else {
                    None
                }
            })
            .collect();

        out.push_str(&format!(
            "challenged: {} → depth {} ({})\n",
            entity_id,
            new_depth,
            depth_label(new_depth)
        ));

        if !crit_refs.is_empty() {
            out.push_str("falsification criteria:\n");
            for crit in &crit_refs {
                let doc = store
                    .entity_datoms(*crit)
                    .iter()
                    .find(|d| d.attribute.as_str() == ":db/doc" && d.op == Op::Assert)
                    .map(|d| match &d.value {
                        Value::String(s) => s.clone(),
                        _ => format!("{:?}", d.value),
                    })
                    .unwrap_or_else(|| "(no description)".into());
                out.push_str(&format!("  - {}\n", doc));
            }
            out.push_str("outcome: braid challenge --survive or --falsify\n");
        } else {
            out.push_str(
                "no falsification criteria registered — use --register first\n",
            );
        }
    }

    let new_weight = depth_weight(new_depth);
    out.push_str(&format!(
        "F(S) weight: {:.2} → {:.2}\n",
        current_weight, new_weight
    ));

    // Provenance
    datoms.push(Datom::new(
        EntityId::from_ident(&format!(":challenge/c-{}", now)),
        Attribute::from_keyword(":tx/provenance-type"),
        Value::Keyword(":provenance.type/challenge".into()),
        tx,
        Op::Assert,
    ));

    // Write transaction
    if !datoms.is_empty() {
        let tx_file = TxFile {
            tx_id: tx,
            agent,
            provenance: ProvenanceType::Observed,
            rationale: format!("challenge {entity_id}"),
            causal_predecessors: vec![],
            datoms,
        };
        layout.write_tx(&tx_file)?;
    }

    Ok(CommandOutput {
        human: out.clone(),
        json: serde_json::json!({
            "entity": entity_id,
            "depth": new_depth,
            "label": depth_label(new_depth),
            "weight": new_weight,
        }),
        agent: crate::output::AgentOutput {
            context: format!("challenge {entity_id}"),
            content: out,
            footer: String::new(),
        },
    })
}

/// Human-readable label for comonadic depth.
fn depth_label(depth: i64) -> &'static str {
    match depth {
        0 => "OPINION",
        1 => "HYPOTHESIS",
        2 => "TESTED",
        3 => "SURVIVED",
        4 => "KNOWLEDGE",
        _ => "UNKNOWN",
    }
}
