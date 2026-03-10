//! `braid log` — Browse the transaction log with filtering.
//!
//! Default: one-line-per-tx summary. `--verbose` or `--datoms` for details.

use std::collections::HashMap;
use std::path::Path;

use braid_kernel::datom::{Attribute, Op, TxId};

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(
    path: &Path,
    limit: usize,
    agent_filter: Option<&str>,
    show_datoms: bool,
    json: bool,
    verbose: bool,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Single pass: group all datoms by TxId — O(N).
    let mut by_tx: HashMap<TxId, Vec<_>> = HashMap::new();
    for d in store.datoms() {
        by_tx.entry(d.tx).or_default().push(d);
    }

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

    if json {
        let entries: Vec<serde_json::Value> = tx_ids
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

                let mut entry = serde_json::json!({
                    "wall_time": tx_id.wall_time(),
                    "agent": format!("{:?}", tx_id.agent()),
                    "provenance": provenance,
                    "rationale": rationale,
                    "assert_count": assert_count,
                    "retract_count": retract_count,
                });

                if show_datoms {
                    let datom_list: Vec<serde_json::Value> = tx_datoms
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
        return Ok(serde_json::to_string_pretty(&result).unwrap() + "\n");
    }

    let mut out = String::new();

    // Verbose or show_datoms: full multi-line per-tx output
    if verbose || show_datoms {
        out.push_str(&format!("transaction log ({} entries):\n\n", tx_ids.len()));

        for tx_id in &tx_ids {
            let tx_datoms = &by_tx[tx_id];

            let rationale = tx_datoms
                .iter()
                .find(|d| d.attribute == rationale_attr)
                .map(|d| format!("{:?}", d.value))
                .unwrap_or_else(|| "-".to_string());

            let provenance = tx_datoms
                .iter()
                .find(|d| d.attribute == provenance_attr)
                .map(|d| format!("{:?}", d.value))
                .unwrap_or_else(|| "-".to_string());

            out.push_str(&format!(
                "tx wall={} agent={:?}\n",
                tx_id.wall_time(),
                tx_id.agent(),
            ));
            out.push_str(&format!("  provenance: {provenance}\n"));
            out.push_str(&format!("  rationale: {rationale}\n"));

            let assert_count = tx_datoms.iter().filter(|d| d.op == Op::Assert).count();
            let retract_count = tx_datoms.iter().filter(|d| d.op == Op::Retract).count();
            out.push_str(&format!(
                "  datoms: {} assert, {} retract\n",
                assert_count, retract_count
            ));

            if show_datoms {
                for d in tx_datoms {
                    out.push_str(&format!(
                        "    {:?} {:?} {} {:?}\n",
                        d.op, d.entity, d.attribute, d.value
                    ));
                }
            }

            out.push('\n');
        }

        return Ok(out);
    }

    // Terse default: one line per tx
    out.push_str(&format!("log: {} transactions\n", tx_ids.len()));
    for tx_id in &tx_ids {
        let tx_datoms = &by_tx[tx_id];
        let assert_count = tx_datoms.iter().filter(|d| d.op == Op::Assert).count();
        let retract_count = tx_datoms.iter().filter(|d| d.op == Op::Retract).count();

        let rationale = tx_datoms
            .iter()
            .find(|d| d.attribute == rationale_attr)
            .and_then(|d| match &d.value {
                braid_kernel::datom::Value::String(s) => {
                    // Truncate long rationales
                    if s.len() > 50 {
                        Some(format!("{}...", &s[..47]))
                    } else {
                        Some(s.clone())
                    }
                }
                _ => None,
            })
            .unwrap_or_default();

        let retract_str = if retract_count > 0 {
            format!(" -{retract_count}")
        } else {
            String::new()
        };

        out.push_str(&format!(
            "  wall={} +{}{} {}\n",
            tx_id.wall_time(),
            assert_count,
            retract_str,
            rationale,
        ));
    }

    Ok(out)
}
