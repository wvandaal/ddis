//! `braid log` — Browse the transaction log with filtering.

use std::path::Path;

use braid_kernel::datom::{Attribute, Op};

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

    // Collect all unique TxIds from the store, sorted by wall_time
    let mut tx_ids: Vec<_> = store
        .datoms()
        .map(|d| d.tx)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    // Sort by wall_time descending (newest first)
    tx_ids.sort_by_key(|t| std::cmp::Reverse(t.wall_time()));

    // Apply agent filter
    if let Some(agent_name) = agent_filter {
        tx_ids.retain(|tx| format!("{:?}", tx.agent()).contains(agent_name));
    }

    // Apply limit
    let tx_ids: Vec<_> = tx_ids.into_iter().take(limit).collect();

    let mut out = String::new();
    out.push_str(&format!("transaction log ({} entries):\n\n", tx_ids.len()));

    for tx_id in &tx_ids {
        // Find datoms belonging to this transaction
        let tx_datoms: Vec<_> = store.datoms().filter(|d| d.tx == *tx_id).collect();

        let rationale = tx_datoms
            .iter()
            .find(|d| d.attribute == Attribute::from_keyword(":tx/rationale"))
            .map(|d| format!("{:?}", d.value))
            .unwrap_or_else(|| "-".to_string());

        let provenance = tx_datoms
            .iter()
            .find(|d| d.attribute == Attribute::from_keyword(":tx/provenance"))
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
            for d in &tx_datoms {
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
