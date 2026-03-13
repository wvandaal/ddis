//! `braid config` command (WP2: Configuration as datoms).
//!
//! Reads and writes config values in the store. No config files.
//! The store IS the configuration (ADR-INTERFACE-005).

use std::path::Path;

use braid_kernel::datom::{AgentId, ProvenanceType};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

/// Run `braid config [key] [value]`.
///
/// - No args: list all config (store values + defaults for unset keys)
/// - One arg: get a specific key
/// - Two args: set a key to a value
/// - --reset key: remove override (revert to default)
pub fn run(
    path: &Path,
    key: Option<&str>,
    value: Option<&str>,
    reset: bool,
    agent: &str,
) -> Result<CommandOutput, BraidError> {
    match (key, value, reset) {
        (Some(k), Some(v), false) => run_set(path, k, v, agent),
        (Some(k), None, true) => run_reset(path, k, agent),
        (Some(k), None, false) => run_get(path, k),
        (None, None, _) => run_list(path),
        _ => Err(BraidError::Validation(
            "Usage: braid config [key] [value] | braid config --reset <key>".into(),
        )),
    }
}

fn run_get(path: &Path, key: &str) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let (human, value_str, source) =
        if let Some(val) = braid_kernel::config::get_config(&store, key) {
            (format!("{key} = {val}\n"), val, "store")
        } else {
            let defaults = braid_kernel::config::defaults();
            if let Some((default_val, desc)) = defaults.get(key) {
                (
                    format!("{key} = {default_val} (default: {desc})\n"),
                    default_val.to_string(),
                    "default",
                )
            } else {
                (
                    format!("{key}: not set (no default)\n"),
                    String::new(),
                    "unset",
                )
            }
        };

    let json = serde_json::json!({
        "operation": "get",
        "key": key,
        "value": value_str,
        "source": source,
    });

    let agent = AgentOutput {
        context: format!("config: get {key}"),
        content: human.clone(),
        footer: "verify: braid config | set: braid config <key> <value>".into(),
    };

    Ok(CommandOutput { json, agent, human })
}

fn run_set(path: &Path, key: &str, value: &str, agent_name: &str) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let agent_id = AgentId::from_name(agent_name);
    let tx_id = super::write::next_tx_id(&store, agent_id);

    let datoms = braid_kernel::config::set_config_datoms(key, value, "project", tx_id);

    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("config: set {key} = {value}"),
        causal_predecessors: vec![],
        datoms,
    };

    layout.write_tx(&tx)?;

    let human = format!("set: {key} = {value}\n");

    let json = serde_json::json!({
        "operation": "set",
        "key": key,
        "value": value,
    });

    let agent = AgentOutput {
        context: format!("config: set {key} = {value}"),
        content: human.clone(),
        footer: "verify: braid config | set: braid config <key> <value>".into(),
    };

    Ok(CommandOutput { json, agent, human })
}

fn run_reset(path: &Path, key: &str, agent_name: &str) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let key_attr = braid_kernel::Attribute::from_keyword(":config/key");
    let val_attr = braid_kernel::Attribute::from_keyword(":config/value");

    let entity = store
        .attribute_datoms(&key_attr)
        .iter()
        .filter(|d| d.op == braid_kernel::Op::Assert)
        .find(|d| matches!(&d.value, braid_kernel::Value::String(k) if k == key))
        .map(|d| d.entity);

    let Some(entity) = entity else {
        let human = format!("{key}: already at default (not set)\n");
        let json = serde_json::json!({
            "operation": "reset",
            "key": key,
            "status": "already_default",
        });
        let agent = AgentOutput {
            context: format!("config: reset {key}"),
            content: human.clone(),
            footer: "verify: braid config | set: braid config <key> <value>".into(),
        };
        return Ok(CommandOutput { json, agent, human });
    };

    let current_val = store
        .entity_datoms(entity)
        .into_iter()
        .rfind(|d| d.attribute == val_attr && d.op == braid_kernel::Op::Assert)
        .cloned();

    let Some(current) = current_val else {
        let human = format!("{key}: no value to reset\n");
        let json = serde_json::json!({
            "operation": "reset",
            "key": key,
            "status": "no_value",
        });
        let agent = AgentOutput {
            context: format!("config: reset {key}"),
            content: human.clone(),
            footer: "verify: braid config | set: braid config <key> <value>".into(),
        };
        return Ok(CommandOutput { json, agent, human });
    };

    let agent_id = AgentId::from_name(agent_name);
    let tx_id = super::write::next_tx_id(&store, agent_id);

    let datoms = vec![braid_kernel::Datom::new(
        entity,
        val_attr,
        current.value.clone(),
        tx_id,
        braid_kernel::Op::Retract,
    )];

    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("config: reset {key}"),
        causal_predecessors: vec![],
        datoms,
    };

    layout.write_tx(&tx)?;

    let human = format!("reset: {key} (reverted to default)\n");

    let json = serde_json::json!({
        "operation": "reset",
        "key": key,
        "status": "reverted",
    });

    let agent = AgentOutput {
        context: format!("config: reset {key}"),
        content: human.clone(),
        footer: "verify: braid config | set: braid config <key> <value>".into(),
    };

    Ok(CommandOutput { json, agent, human })
}

fn run_list(path: &Path) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let store_config = braid_kernel::config::all_config(&store);
    let defaults = braid_kernel::config::defaults();

    let mut out = String::from("Configuration:\n");
    let mut entries = Vec::new();

    // Show store-set values first
    let mut shown_keys = std::collections::HashSet::new();
    for (key, value, scope) in &store_config {
        out.push_str(&format!("  {key} = {value}  ({scope})\n"));
        entries.push(serde_json::json!({
            "key": key,
            "value": value,
            "source": scope,
        }));
        shown_keys.insert(key.clone());
    }

    // Show defaults for unset keys
    let mut default_entries: Vec<_> = defaults.iter().collect();
    default_entries.sort_by_key(|e| e.0.clone());
    for (key, (default_val, desc)) in default_entries {
        if !shown_keys.contains(key.as_str()) {
            out.push_str(&format!("  {key} = {default_val}  (default: {desc})\n"));
            entries.push(serde_json::json!({
                "key": key,
                "value": default_val,
                "source": format!("default: {desc}"),
            }));
        }
    }

    let entry_count = entries.len();

    let json = serde_json::json!({
        "operation": "list",
        "entries": entries,
    });

    let agent = AgentOutput {
        context: format!("config: {entry_count} entries"),
        content: out.clone(),
        footer: "verify: braid config | set: braid config <key> <value>".into(),
    };

    Ok(CommandOutput { json, agent, human: out })
}
