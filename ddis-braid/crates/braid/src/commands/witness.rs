//! Witness command — FBW coverage, staleness, and completeness (INV-WITNESS-011).
//!
//! `braid witness status`       Show witness coverage summary.
//! `braid witness check`        Run staleness detection on all witnesses.
//! `braid witness completeness` Show unwitnessed invariants.
//!
//! Traces to: spec/21-witness.md INV-WITNESS-001..011.

use std::path::Path;

use braid_kernel::witness::{self, WitnessVerdict, FBW};

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

/// Run `braid witness status` -- show witness coverage summary.
pub fn run_status(path: &Path, json: bool) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let witnesses = witness::all_witnesses(&store);
    let (score, valid_count, stale_count, untested_count) =
        witness::witness_validation_score(&store);

    // Count total invariants (valid L2+ witnessed, stale, untested covers all)
    let total_invariants = valid_count + stale_count + untested_count;

    // Alignment score distribution
    let mut align_buckets = [0u32; 5]; // [0-0.2), [0.2-0.4), [0.4-0.6), [0.6-0.8), [0.8-1.0]
    for w in &witnesses {
        let idx = match w.alignment_score {
            s if s < 0.2 => 0,
            s if s < 0.4 => 1,
            s if s < 0.6 => 2,
            s if s < 0.8 => 3,
            _ => 4,
        };
        align_buckets[idx] += 1;
    }

    // Verdict distribution
    let confirmed = witnesses
        .iter()
        .filter(|w| w.verdict == WitnessVerdict::Confirmed)
        .count();
    let provisional = witnesses
        .iter()
        .filter(|w| w.verdict == WitnessVerdict::Provisional)
        .count();
    let inconclusive = witnesses
        .iter()
        .filter(|w| w.verdict == WitnessVerdict::Inconclusive)
        .count();
    let refuted = witnesses
        .iter()
        .filter(|w| w.verdict == WitnessVerdict::Refuted)
        .count();

    if json {
        let json_val = serde_json::json!({
            "total_invariants": total_invariants,
            "witnesses": witnesses.len(),
            "valid": valid_count,
            "stale": stale_count,
            "untested": untested_count,
            "validation_score": score,
            "verdicts": {
                "confirmed": confirmed,
                "provisional": provisional,
                "inconclusive": inconclusive,
                "refuted": refuted,
            },
            "alignment_distribution": {
                "0.0-0.2": align_buckets[0],
                "0.2-0.4": align_buckets[1],
                "0.4-0.6": align_buckets[2],
                "0.6-0.8": align_buckets[3],
                "0.8-1.0": align_buckets[4],
            },
        });
        let json_str = serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| "{}".to_string());
        return Ok(CommandOutput::from_human(json_str));
    }

    // Human output: compact dashboard
    let mut out = String::new();
    out.push_str(&format!(
        "witness: {total_invariants} invariants, {valid_count} witnessed, \
         {stale_count} stale, {untested_count} untested\n"
    ));
    out.push_str(&format!("  validation score: {score:.2}\n"));
    out.push_str(&format!("  witnesses total:  {}\n", witnesses.len()));
    out.push_str(&format!(
        "  verdicts: {confirmed} confirmed, {provisional} provisional, \
         {inconclusive} inconclusive, {refuted} refuted\n"
    ));
    out.push_str("  alignment: ");
    let labels = ["<0.2", "0.2-0.4", "0.4-0.6", "0.6-0.8", "0.8+"];
    let parts: Vec<String> = labels
        .iter()
        .zip(align_buckets.iter())
        .filter(|(_, &count)| count > 0)
        .map(|(label, count)| format!("{label}:{count}"))
        .collect();
    if parts.is_empty() {
        out.push_str("(none)\n");
    } else {
        out.push_str(&parts.join(" "));
        out.push('\n');
    }

    if untested_count > 0 {
        out.push_str(&format!(
            "\nhint: {untested_count} invariants lack L2+ witnesses. \
             Run: braid witness completeness\n"
        ));
    }
    if stale_count > 0 {
        out.push_str(&format!(
            "hint: {stale_count} stale witnesses detected. \
             Run: braid witness check --commit\n"
        ));
    }

    let agent_content = format!(
        "{valid_count}/{total_invariants} witnessed, {stale_count} stale, score={score:.2}"
    );

    // ACP projection for witness status (INV-BUDGET-007)
    // Action = "braid witness check" (next diagnostic step)
    // Context = coverage summary as budget-scaled blocks
    let action = braid_kernel::budget::ProjectedAction {
        command: "braid witness check".to_string(),
        rationale: "check witness staleness".to_string(),
        impact: if stale_count > 0 { 0.7 } else { 0.3 },
    };

    let mut context_blocks = vec![braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::System,
        content: format!(
            "witness: {total_invariants} invariants, {valid_count} valid, \
             {stale_count} stale, {untested_count} untested, score={score:.2}"
        ),
        tokens: 15,
    }];

    if confirmed > 0 || refuted > 0 {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
            content: format!(
                "verdicts: {confirmed} confirmed, {provisional} provisional, \
                 {inconclusive} inconclusive, {refuted} refuted"
            ),
            tokens: 10,
        });
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: "details: braid witness check | gaps: braid witness completeness"
            .to_string(),
    };

    // Merge ACP into JSON
    let mut json = serde_json::json!({
        "total_invariants": total_invariants,
        "valid": valid_count,
        "stale": stale_count,
        "untested": untested_count,
        "validation_score": score,
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // Human output uses ACP full projection (but keep existing human if richer)
    let _ = agent_content; // consumed by ACP
    Ok(CommandOutput {
        json,
        agent,
        human: out,
    })
}

/// Run `braid witness check` -- staleness detection on all witnesses.
pub fn run_check(path: &Path, commit: bool, json: bool) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let witnesses = witness::all_witnesses(&store);
    let current = witness::current_spec_hashes(&store);
    let stale_list = witness::detect_stale_witnesses(&witnesses, &current);

    // Build a lookup for witness entity -> FBW for display
    let fbw_map: std::collections::BTreeMap<braid_kernel::EntityId, &FBW> =
        witnesses.iter().map(|w| (w.entity, w)).collect();

    if json {
        let stale_entries: Vec<serde_json::Value> = stale_list
            .iter()
            .map(|(entity, reason)| {
                let reason_str = format_stale_reason(reason);
                let inv_ref = fbw_map
                    .get(entity)
                    .map(|w| format!("{:?}", w.inv_ref))
                    .unwrap_or_default();
                serde_json::json!({
                    "witness": format!("{entity:?}"),
                    "inv_ref": inv_ref,
                    "reason": reason_str,
                })
            })
            .collect();

        let json_val = serde_json::json!({
            "total_witnesses": witnesses.len(),
            "stale_found": stale_list.len(),
            "committed": commit,
            "stale": stale_entries,
        });
        let json_str = serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| "{}".to_string());
        return Ok(CommandOutput::from_human(json_str));
    }

    let mut out = String::new();
    out.push_str(&format!(
        "witness check: {} witnesses, {} stale\n",
        witnesses.len(),
        stale_list.len()
    ));

    if stale_list.is_empty() {
        out.push_str("  all witnesses current\n");
    } else {
        for (entity, reason) in &stale_list {
            let inv_display = fbw_map
                .get(entity)
                .map(|w| format!("{:?}", w.inv_ref))
                .unwrap_or_else(|| format!("{entity:?}"));
            out.push_str(&format!(
                "  STALE {entity:?} -> {inv_display}: {}\n",
                format_stale_reason(reason)
            ));
        }
    }

    // Optionally transact stale markers
    if commit && !stale_list.is_empty() {
        use braid_kernel::datom::*;
        let agent = AgentId::from_name("braid:witness");
        let tx_id = crate::commands::write::next_tx_id(&store, agent);
        let mut datoms = Vec::new();
        for (entity, _) in &stale_list {
            datoms.extend(witness::mark_stale_datoms(*entity, tx_id));
        }
        let datom_count = datoms.len();
        let tx = braid_kernel::layout::TxFile {
            tx_id,
            agent,
            provenance: ProvenanceType::Derived,
            rationale: format!(
                "witness check: marked {} witnesses stale (INV-WITNESS-001)",
                stale_list.len()
            ),
            causal_predecessors: vec![],
            datoms,
        };
        layout.write_tx(&tx)?;
        out.push_str(&format!(
            "\ntransacted: {datom_count} stale marker datoms\n"
        ));
    } else if !stale_list.is_empty() && !commit {
        out.push_str("\nhint: use --commit to transact stale markers\n");
    }

    let agent_content = if stale_list.is_empty() {
        "all witnesses current".to_string()
    } else {
        format!("{} stale witnesses detected", stale_list.len())
    };

    // ACP projection for witness check (INV-BUDGET-007)
    // Action = "braid harvest --commit" (refresh stale witnesses)
    // Context = stale witness list as budget-scaled blocks
    let action = braid_kernel::budget::ProjectedAction {
        command: "braid harvest --commit".to_string(),
        rationale: "refresh stale witness data".to_string(),
        impact: if stale_list.is_empty() { 0.1 } else { 0.8 },
    };

    let mut context_blocks = vec![braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::System,
        content: format!(
            "witness check: {} witnesses, {} stale{}",
            witnesses.len(),
            stale_list.len(),
            if commit { " (committed)" } else { "" }
        ),
        tokens: 10,
    }];

    // Add stale entries as individual context blocks
    for (entity, reason) in &stale_list {
        let inv_display = fbw_map
            .get(entity)
            .map(|w| format!("{:?}", w.inv_ref))
            .unwrap_or_else(|| format!("{entity:?}"));
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
            content: format!("STALE {inv_display}: {}", format_stale_reason(reason)),
            tokens: 8,
        });
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: "status: braid witness status | completeness: braid witness completeness"
            .to_string(),
    };

    // Merge ACP into JSON
    let mut json = serde_json::json!({
        "total_witnesses": witnesses.len(),
        "stale_found": stale_list.len(),
        "committed": commit,
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    let _ = agent_content; // consumed by ACP
    Ok(CommandOutput {
        json,
        agent,
        human: out,
    })
}

/// Run `braid witness completeness` -- show unwitnessed invariants.
pub fn run_completeness(path: &Path, json: bool) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let witnesses = witness::all_witnesses(&store);
    let unwitnessed = witness::completeness_guard(&store, &witnesses);

    // Resolve idents for display
    let ident_attr = braid_kernel::datom::Attribute::from_keyword(":db/ident");
    let title_attr = braid_kernel::datom::Attribute::from_keyword(":element/title");

    let mut entries: Vec<(String, String)> = Vec::new();
    for entity in &unwitnessed {
        let entity_datoms = store.entity_datoms(*entity);
        let ident = entity_datoms
            .iter()
            .find(|d| d.attribute == ident_attr && d.op == braid_kernel::datom::Op::Assert)
            .and_then(|d| match &d.value {
                braid_kernel::datom::Value::Keyword(k) => Some(k.clone()),
                braid_kernel::datom::Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| format!("{entity:?}"));
        let title = entity_datoms
            .iter()
            .find(|d| d.attribute == title_attr && d.op == braid_kernel::datom::Op::Assert)
            .and_then(|d| match &d.value {
                braid_kernel::datom::Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();
        entries.push((ident, title));
    }

    if json {
        let inv_list: Vec<serde_json::Value> = entries
            .iter()
            .map(|(ident, title)| {
                serde_json::json!({
                    "ident": ident,
                    "title": title,
                })
            })
            .collect();
        let json_val = serde_json::json!({
            "total_unwitnessed": unwitnessed.len(),
            "invariants": inv_list,
        });
        let json_str = serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| "{}".to_string());
        return Ok(CommandOutput::from_human(json_str));
    }

    let mut out = String::new();
    out.push_str(&format!(
        "completeness: {} invariants lack L2+ witnesses\n",
        unwitnessed.len()
    ));

    if entries.is_empty() {
        out.push_str("  all invariants have L2+ witnesses\n");
    } else {
        for (ident, title) in &entries {
            if title.is_empty() {
                out.push_str(&format!("  {ident}: needs L2+ witness\n"));
            } else {
                out.push_str(&format!("  {ident}: {title} -- needs L2+ witness\n"));
            }
        }
    }

    let agent_content = if unwitnessed.is_empty() {
        "all invariants witnessed at L2+".to_string()
    } else {
        format!("{} invariants need L2+ witnesses", unwitnessed.len())
    };

    // ACP projection for witness completeness (INV-BUDGET-007)
    // Action = "braid trace --commit" (add coverage for unwitnessed invariants)
    // Context = unwitnessed invariant list as budget-scaled blocks
    let action = braid_kernel::budget::ProjectedAction {
        command: "braid trace --commit".to_string(),
        rationale: "add witness coverage".to_string(),
        impact: if unwitnessed.is_empty() { 0.1 } else { 0.6 },
    };

    let mut context_blocks = vec![braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::System,
        content: format!(
            "completeness: {} invariants lack L2+ witnesses",
            unwitnessed.len()
        ),
        tokens: 8,
    }];

    // Add unwitnessed invariants as individual context blocks
    let max_entries = 20;
    for (i, (ident, title)) in entries.iter().take(max_entries).enumerate() {
        let display = if title.is_empty() {
            format!("{ident}: needs L2+ witness")
        } else {
            format!("{ident}: {title}")
        };
        let precedence = if i < 5 {
            braid_kernel::budget::OutputPrecedence::UserRequested
        } else {
            braid_kernel::budget::OutputPrecedence::Speculative
        };
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence,
            content: display,
            tokens: 10,
        });
    }

    if entries.len() > max_entries {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::Ambient,
            content: format!("... and {} more", entries.len() - max_entries),
            tokens: 3,
        });
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: "status: braid witness status | check: braid witness check".to_string(),
    };

    // Merge ACP into JSON
    let mut json = serde_json::json!({
        "total_unwitnessed": unwitnessed.len(),
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    let _ = agent_content; // consumed by ACP
    Ok(CommandOutput {
        json,
        agent,
        human: out,
    })
}

/// Batch-generate L1 witness skeletons + promote existing tests to L2.
///
/// Dry-run mode (default): shows how many would be created.
/// Commit mode: transacts the witness datoms.
pub fn run_generate(path: &Path, commit: bool) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let agent = braid_kernel::datom::AgentId::from_name("braid:witness-batch");
    let tx_id = crate::commands::write::next_tx_id(&store, agent);

    // Step 1: L1 witnesses for all unwitnessed invariants
    let (mut datoms, l1_count) = witness::batch_generate_l1_witnesses(&store, tx_id);

    // Step 2: L2 promotion for Kani proofs + Stateright models
    let kani_bindings = witness::kani_proof_bindings();
    let sr_bindings = witness::stateright_model_bindings();
    let (kani_datoms, kani_count) =
        witness::promote_tests_to_l2(&store, &kani_bindings, tx_id);
    let (sr_datoms, sr_count) =
        witness::promote_tests_to_l2(&store, &sr_bindings, tx_id);
    datoms.extend(kani_datoms);
    datoms.extend(sr_datoms);

    let total_count = l1_count + kani_count + sr_count;
    let datom_count = datoms.len();
    let mut out = format!(
        "witness generate: {l1_count} L1 + {kani_count} L2(Kani) + {sr_count} L2(Stateright) = {total_count} total\n"
    );

    if commit && !datoms.is_empty() {
        let tx = braid_kernel::layout::TxFile {
            tx_id,
            agent,
            provenance: braid_kernel::datom::ProvenanceType::Derived,
            rationale: format!(
                "witness generation: {l1_count} L1 + {kani_count} L2(Kani) + {sr_count} L2(Stateright) for INV-WITNESS-011"
            ),
            causal_predecessors: vec![],
            datoms,
        };
        layout.write_tx(&tx)?;
        out.push_str(&format!("committed: {datom_count} datoms ({total_count} witnesses)\n"));
    } else if !datoms.is_empty() && !commit {
        out.push_str("dry-run: use --commit to transact\n");
    } else {
        out.push_str("all invariants already have witnesses at required depth\n");
    }

    let json = serde_json::json!({
        "l1_generated": l1_count,
        "l2_kani": kani_count,
        "l2_stateright": sr_count,
        "total": total_count,
        "committed": commit && datom_count > 0,
        "datom_count": datom_count,
    });

    let agent_out = AgentOutput {
        context: String::new(),
        content: format!("witness generate: {total_count} witnesses ({kani_count} L2 promoted)"),
        footer: String::new(),
    };

    Ok(CommandOutput {
        json,
        agent: agent_out,
        human: out,
    })
}

/// Format a `StaleReason` into a human-readable string.
fn format_stale_reason(reason: &witness::StaleReason) -> String {
    match reason {
        witness::StaleReason::SpecDrift => "spec statement changed".to_string(),
        witness::StaleReason::FalsificationDrift => "falsification condition changed".to_string(),
        witness::StaleReason::TestBodyDrift => "test body changed".to_string(),
        witness::StaleReason::MultiDrift(reasons) => {
            let parts: Vec<String> = reasons.iter().map(format_stale_reason).collect();
            parts.join(" + ")
        }
    }
}
