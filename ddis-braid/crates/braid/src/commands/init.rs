//! `braid init` — Initialize a new braid store, optionally auto-bootstrapping spec elements.

use std::path::Path;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(path: &Path, spec_dir: &Path) -> Result<String, BraidError> {
    let layout = DiskLayout::init(path)?;
    let hashes = layout.list_tx_hashes()?;
    let store = layout.load_store()?;

    let mut out = format!(
        "initialized braid store at {}\n  genesis: {} transaction(s), {} datom(s)\n",
        path.display(),
        hashes.len(),
        store.len(),
    );

    // Auto-bootstrap: if spec_dir exists and contains .md files, bootstrap them.
    if spec_dir.is_dir() {
        let elements = crate::bootstrap::parse_spec_dir(spec_dir);
        if !elements.is_empty() {
            let agent = braid_kernel::datom::AgentId::from_name("braid:bootstrap");
            let tx = crate::bootstrap::elements_to_tx(&elements, agent);
            let datom_count = tx.datoms.len();
            let file_path = layout.write_tx(&tx)?;

            let invs = elements
                .iter()
                .filter(|e| e.kind == crate::bootstrap::SpecElementKind::Invariant)
                .count();
            let adrs = elements
                .iter()
                .filter(|e| e.kind == crate::bootstrap::SpecElementKind::Adr)
                .count();
            let negs = elements
                .iter()
                .filter(|e| e.kind == crate::bootstrap::SpecElementKind::NegativeCase)
                .count();

            out.push_str(&format!(
                "  bootstrap: {} elements ({} INV, {} ADR, {} NEG) \u{2192} {} datoms\n    \u{2192} {}\n",
                elements.len(),
                invs,
                adrs,
                negs,
                datom_count,
                file_path.relative_path(),
            ));
        }
    }

    Ok(out)
}
