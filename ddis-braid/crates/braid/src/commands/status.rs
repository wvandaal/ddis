//! `braid status` — Show store status.
//!
//! Default: terse 5-line dashboard (LLM-optimized).
//! `--verbose`: full frontier, schema details, per-agent breakdown.

use std::path::Path;

use braid_kernel::guidance::{count_txns_since_last_harvest, derive_actions};
use braid_kernel::trilateral::check_coherence_fast;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(path: &Path, json: bool, verbose: bool) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    let hashes = layout.list_tx_hashes()?;

    let tx_since_harvest = count_txns_since_last_harvest(&store);

    if json {
        let frontier: Vec<serde_json::Value> = store
            .frontier()
            .iter()
            .map(|(agent, tx_id)| {
                serde_json::json!({
                    "agent": format!("{:?}", agent),
                    "wall_time": tx_id.wall_time(),
                })
            })
            .collect();

        let schema_attrs: Vec<String> = store
            .schema()
            .attributes()
            .map(|(a, _def)| a.as_str().to_string())
            .collect();

        let result = serde_json::json!({
            "store": path.display().to_string(),
            "datom_count": store.len(),
            "transaction_count": hashes.len(),
            "entity_count": store.entities().len(),
            "schema_attribute_count": store.schema().len(),
            "schema_attributes": schema_attrs,
            "frontier": frontier,
            "tx_since_last_harvest": tx_since_harvest,
        });
        return Ok(serde_json::to_string_pretty(&result).unwrap() + "\n");
    }

    if verbose {
        return run_verbose(path, &store, &hashes, tx_since_harvest);
    }

    // Terse default: 5-line dashboard optimized for LLM consumption
    let coherence = check_coherence_fast(&store);
    let actions = derive_actions(&store);

    let harvest_tag = if tx_since_harvest >= 15 {
        " OVERDUE"
    } else if tx_since_harvest >= 8 {
        " (harvest?)"
    } else {
        ""
    };

    let top_action = actions
        .first()
        .map(|a| format!("{}: {}", a.category, a.summary))
        .unwrap_or_else(|| "none".into());

    let mut out = String::new();
    out.push_str(&format!(
        "store: {} ({} datoms, {} entities, {} txns)\n",
        path.display(),
        store.len(),
        store.entity_count(),
        hashes.len(),
    ));
    out.push_str(&format!(
        "coherence: phi={:.1} beta1={} {:?}\n",
        coherence.phi, coherence.beta_1, coherence.quadrant,
    ));
    out.push_str(&format!(
        "live: intent={} spec={} impl={} | agents={}\n",
        coherence.live_intent,
        coherence.live_spec,
        coherence.live_impl,
        store.frontier().len(),
    ));
    out.push_str(&format!(
        "harvest: {} tx since last{}\n",
        tx_since_harvest, harvest_tag,
    ));
    out.push_str(&format!("next: {}\n", top_action));

    Ok(out)
}

/// Full verbose output with frontier details and per-agent breakdown.
fn run_verbose(
    path: &Path,
    store: &braid_kernel::Store,
    hashes: &[String],
    tx_since_harvest: usize,
) -> Result<String, BraidError> {
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

    // Harvest health
    let harvest_warning = if tx_since_harvest >= 15 {
        " [OVERDUE — harvest immediately]"
    } else if tx_since_harvest >= 8 {
        " [consider harvesting]"
    } else {
        ""
    };
    out.push_str(&format!(
        "  tx since last harvest: {}{}\n",
        tx_since_harvest, harvest_warning
    ));

    Ok(out)
}
