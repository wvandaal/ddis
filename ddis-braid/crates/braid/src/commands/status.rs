//! `braid status` — Show store status.

use std::path::Path;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(path: &Path) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    let hashes = layout.list_tx_hashes()?;

    let mut out = String::new();
    out.push_str(&format!("store: {}\n", path.display()));
    out.push_str(&format!("  datoms: {}\n", store.len()));
    out.push_str(&format!("  transactions: {}\n", hashes.len()));
    out.push_str(&format!("  entities: {}\n", store.entities().len()));
    out.push_str(&format!("  schema attributes: {}\n", store.schema().len()));

    // Frontier
    out.push_str("  frontier:\n");
    for (agent, tx_id) in store.frontier() {
        out.push_str(&format!("    {:?}: wall={}\n", agent, tx_id.wall_time()));
    }

    Ok(out)
}
