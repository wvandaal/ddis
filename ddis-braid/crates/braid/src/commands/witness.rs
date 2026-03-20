//! Witness command — FBW coverage, staleness, and completeness (INV-WITNESS-011).
//!
//! `braid witness status`       Show witness coverage summary.
//! `braid witness check`        Run staleness detection on all witnesses.
//! `braid witness completeness` Show unwitnessed invariants.
//!
//! Traces to: spec/21-witness.md INV-WITNESS-001..011.

use std::path::Path;

use braid_kernel::witness::{self, FBW, WitnessVerdict};

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
    let confirmed = witnesses.iter().filter(|w| w.verdict == WitnessVerdict::Confirmed).count();
    let provisional = witnesses.iter().filter(|w| w.verdict == WitnessVerdict::Provisional).count();
    let inconclusive = witnesses
        .iter()
        .filter(|w| w.verdict == WitnessVerdict::Inconclusive)
        .count();
    let refuted = witnesses.iter().filter(|w| w.verdict == WitnessVerdict::Refuted).count();

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
        let json_str =
            serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| "{}".to_string());
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
    Ok(CommandOutput {
        json: serde_json::json!({
            "total_invariants": total_invariants,
            "valid": valid_count,
            "stale": stale_count,
            "untested": untested_count,
            "validation_score": score,
        }),
        agent: AgentOutput {
            context: format!(
                "witness: {valid_count}/{total_invariants} L2+, score={score:.2}"
            ),
            content: agent_content,
            footer: String::new(),
        },
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
        let json_str =
            serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| "{}".to_string());
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

    Ok(CommandOutput {
        json: serde_json::json!({
            "total_witnesses": witnesses.len(),
            "stale_found": stale_list.len(),
            "committed": commit,
        }),
        agent: AgentOutput {
            context: format!("witness check: {}/{} stale", stale_list.len(), witnesses.len()),
            content: agent_content,
            footer: String::new(),
        },
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
            .find(|d| {
                d.attribute == ident_attr && d.op == braid_kernel::datom::Op::Assert
            })
            .and_then(|d| match &d.value {
                braid_kernel::datom::Value::Keyword(k) => Some(k.clone()),
                braid_kernel::datom::Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| format!("{entity:?}"));
        let title = entity_datoms
            .iter()
            .find(|d| {
                d.attribute == title_attr && d.op == braid_kernel::datom::Op::Assert
            })
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
        let json_str =
            serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| "{}".to_string());
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

    Ok(CommandOutput {
        json: serde_json::json!({
            "total_unwitnessed": unwitnessed.len(),
        }),
        agent: AgentOutput {
            context: format!("completeness: {} gaps", unwitnessed.len()),
            content: agent_content,
            footer: String::new(),
        },
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
