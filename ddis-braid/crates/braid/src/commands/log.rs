//! `braid log` — Browse the transaction log with filtering.

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

    let mut out = String::new();
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

    Ok(out)
}
