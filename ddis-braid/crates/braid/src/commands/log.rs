//! `braid log` — Browse the transaction log with filtering.
//!
//! Default: one-line-per-tx summary. `--verbose` or `--datoms` for details.

use std::collections::HashMap;
use std::path::Path;

use braid_kernel::datom::{Attribute, Op, TxId};

use crate::error::BraidError;
use crate::live_store::LiveStore;
use crate::output::{AgentOutput, CommandOutput};

pub fn run(
    path: &Path,
    limit: usize,
    agent_filter: Option<&str>,
    show_datoms: bool,
    json: bool,
    verbose: bool,
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

    // Single pass: group all datoms by TxId — O(N).
    let mut by_tx: HashMap<TxId, Vec<_>> = HashMap::new();
    for d in store.datoms() {
        by_tx.entry(d.tx).or_default().push(d);
    }

    let total_txns = by_tx.len();

    // Collect unique TxIds sorted by wall_time descending (newest first)
    let mut tx_ids: Vec<_> = by_tx.keys().copied().collect();
    tx_ids.sort_by_key(|t| std::cmp::Reverse(t.wall_time()));

    // Apply agent filter
    if let Some(agent_name) = agent_filter {
        tx_ids.retain(|tx| format!("{:?}", tx.agent()).contains(agent_name));
    }

    // Apply limit
    let tx_ids: Vec<_> = tx_ids.into_iter().take(limit).collect();

    // Pre-construct attributes once (avoid per-iteration allocation)
    let rationale_attr = Attribute::from_keyword(":tx/rationale");
    let provenance_attr = Attribute::from_keyword(":tx/provenance");

    // --- Build structured transaction data (used for both JSON and AgentOutput) ---
    struct TxInfo {
        wall_time: u64,
        agent: String,
        provenance: String,
        rationale: String,
        assert_count: usize,
        retract_count: usize,
    }

    let tx_infos: Vec<TxInfo> = tx_ids
        .iter()
        .map(|tx_id| {
            let tx_datoms = &by_tx[tx_id];

            let rationale = tx_datoms
                .iter()
                .find(|d| d.attribute == rationale_attr)
                .map(|d| format!("{:?}", d.value))
                .unwrap_or_default();

            let provenance = tx_datoms
                .iter()
                .find(|d| d.attribute == provenance_attr)
                .map(|d| format!("{:?}", d.value))
                .unwrap_or_default();

            let assert_count = tx_datoms.iter().filter(|d| d.op == Op::Assert).count();
            let retract_count = tx_datoms.iter().filter(|d| d.op == Op::Retract).count();

            TxInfo {
                wall_time: tx_id.wall_time(),
                agent: format!("{:?}", tx_id.agent()),
                provenance,
                rationale,
                assert_count,
                retract_count,
            }
        })
        .collect();

    // --- Build human-mode output string ---
    let human = if json {
        // --json flag: human-mode output is the pretty-printed JSON
        let entries: Vec<serde_json::Value> = tx_ids
            .iter()
            .zip(tx_infos.iter())
            .map(|(tx_id, info)| {
                let mut entry = serde_json::json!({
                    "wall_time": info.wall_time,
                    "agent": &info.agent,
                    "provenance": &info.provenance,
                    "rationale": &info.rationale,
                    "assert_count": info.assert_count,
                    "retract_count": info.retract_count,
                });

                if show_datoms {
                    let datom_list: Vec<serde_json::Value> = by_tx[tx_id]
                        .iter()
                        .map(|d| {
                            serde_json::json!({
                                "op": format!("{:?}", d.op),
                                "entity": format!("{:?}", d.entity),
                                "attribute": d.attribute.as_str(),
                                "value": format!("{:?}", d.value),
                            })
                        })
                        .collect();
                    entry["datoms"] = serde_json::json!(datom_list);
                }

                entry
            })
            .collect();

        let result = serde_json::json!({
            "count": entries.len(),
            "transactions": entries,
        });
        serde_json::to_string_pretty(&result).unwrap() + "\n"
    } else if verbose || show_datoms {
        let mut out = String::new();
        out.push_str(&format!("transaction log ({} entries):\n\n", tx_ids.len()));

        for (tx_id, info) in tx_ids.iter().zip(tx_infos.iter()) {
            let rationale_display = if info.rationale.is_empty() {
                "-".to_string()
            } else {
                info.rationale.clone()
            };
            let provenance_display = if info.provenance.is_empty() {
                "-".to_string()
            } else {
                info.provenance.clone()
            };

            out.push_str(&format!(
                "tx wall={} agent={}\n",
                info.wall_time, info.agent,
            ));
            out.push_str(&format!("  provenance: {provenance_display}\n"));
            out.push_str(&format!("  rationale: {rationale_display}\n"));
            out.push_str(&format!(
                "  datoms: {} assert, {} retract\n",
                info.assert_count, info.retract_count
            ));

            if show_datoms {
                let tx_datoms = &by_tx[tx_id];
                for d in tx_datoms {
                    out.push_str(&format!(
                        "    {:?} {:?} {} {:?}\n",
                        d.op, d.entity, d.attribute, d.value
                    ));
                }
            }

            out.push('\n');
        }
        out
    } else {
        // Terse default: one line per tx
        let mut out = String::new();
        out.push_str(&format!("log: {} transactions\n", tx_ids.len()));
        for info in &tx_infos {
            // Truncate long rationales for terse mode
            let rationale_short = if info.rationale.len() > 50 {
                format!("{}...", &info.rationale[..47])
            } else {
                info.rationale.clone()
            };

            let retract_str = if info.retract_count > 0 {
                format!(" -{}", info.retract_count)
            } else {
                String::new()
            };

            out.push_str(&format!(
                "  wall={} +{}{} {}\n",
                info.wall_time, info.assert_count, retract_str, rationale_short,
            ));
        }
        out
    };

    // --- Build structured JSON for CommandOutput (always, independent of --json flag) ---
    let structured_txns: Vec<serde_json::Value> = tx_infos
        .iter()
        .map(|info| {
            serde_json::json!({
                "tx_id": info.wall_time,
                "agent": &info.agent,
                "provenance": &info.provenance,
                "rationale": &info.rationale,
                "datom_count": info.assert_count + info.retract_count,
                "wall_time": info.wall_time,
            })
        })
        .collect();

    let mut structured_json = serde_json::json!({
        "count": tx_infos.len(),
        "total": total_txns,
        "transactions": structured_txns,
    });

    // --- ACP: Build ActionProjection (INV-BUDGET-007) ---
    let action = braid_kernel::budget::ProjectedAction {
        command: "braid status".to_string(),
        rationale: "review store state".to_string(),
        impact: 0.2,
    };

    let mut context_blocks = Vec::new();

    // Summary context (System)
    context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
        braid_kernel::budget::OutputPrecedence::System,
        format!(
            "log: {} transactions shown ({} total)",
            tx_infos.len(),
            total_txns,
        ),
        10,
    ));

    // Recent transaction entries as context (Methodology)
    for info in tx_infos.iter().take(5) {
        let rationale_short = if info.rationale.len() > 60 {
            format!("{}...", &info.rationale[..57.min(info.rationale.len())])
        } else {
            info.rationale.clone()
        };
        let retract_str = if info.retract_count > 0 {
            format!(" -{}", info.retract_count)
        } else {
            String::new()
        };
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::Methodology,
            format!(
                "tx wall={} +{}{} {}",
                info.wall_time, info.assert_count, retract_str, rationale_short,
            ),
            12,
        ));
    }

    // Remaining count if truncated (Speculative)
    if tx_infos.len() > 5 {
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::Speculative,
            format!("... and {} more transactions", tx_infos.len() - 5),
            5,
        ));
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: "detail: braid log --datoms --limit 1 | full: braid log --limit 100"
            .to_string(),
    };

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // Merge _acp into structured JSON
    if let serde_json::Value::Object(ref mut map) = structured_json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput {
        json: structured_json,
        agent,
        human,
    })
}
