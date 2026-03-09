//! `braid init` — Initialize a new braid store.

use std::path::Path;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(path: &Path) -> Result<String, BraidError> {
    let layout = DiskLayout::init(path)?;
    let hashes = layout.list_tx_hashes()?;
    let store = layout.load_store()?;

    Ok(format!(
        "initialized braid store at {}\n  genesis: {} transaction(s), {} datom(s)\n",
        path.display(),
        hashes.len(),
        store.len(),
    ))
}
