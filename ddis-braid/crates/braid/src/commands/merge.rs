//! `braid merge` — Merge another store into the current store (CRDT set union).

use std::path::Path;

use braid_kernel::merge::{verify_frontier_advancement, verify_monotonicity};

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(path: &Path, source_path: &Path) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let mut store = layout.load_store()?;

    let source_layout = DiskLayout::open(source_path)?;
    let source = source_layout.load_store()?;

    let pre_datoms = store.datom_set().clone();
    let pre_frontier = store.frontier().clone();
    let pre_len = store.len();

    // CRDT merge: set union (INV-STORE-004..007)
    let receipt = store.merge(&source);

    // Verify monotonicity (INV-STORE-002)
    let monotonic = verify_monotonicity(&pre_datoms, store.datom_set());
    let frontier_advanced = verify_frontier_advancement(&pre_frontier, store.frontier());

    // Persist merged transactions: write any source txns we don't already have
    let source_hashes = source_layout.list_tx_hashes()?;
    let our_hashes: std::collections::HashSet<String> =
        layout.list_tx_hashes()?.into_iter().collect();
    let mut new_files = 0;
    for hash in &source_hashes {
        if !our_hashes.contains(hash) {
            let tx = source_layout.read_tx(hash)?;
            layout.write_tx(&tx)?;
            new_files += 1;
        }
    }

    let mut out = String::new();
    out.push_str(&format!(
        "merge: {} → {}\n",
        source_path.display(),
        path.display()
    ));
    out.push_str(&format!(
        "  datoms: {} → {} (+{})\n",
        pre_len,
        store.len(),
        receipt.new_datoms
    ));
    out.push_str(&format!("  new tx files: {new_files}\n"));
    out.push_str(&format!(
        "  frontier agents: {} → {}\n",
        pre_frontier.len(),
        store.frontier().len()
    ));
    out.push_str(&format!(
        "  monotonicity: {}\n",
        if monotonic { "OK" } else { "VIOLATED" }
    ));
    out.push_str(&format!(
        "  frontier advancement: {}\n",
        if frontier_advanced { "OK" } else { "NO CHANGE" }
    ));

    Ok(out)
}
