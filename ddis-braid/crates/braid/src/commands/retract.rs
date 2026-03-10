//! `braid retract` — Retract existing assertions from the store.
//!
//! Creates a new transaction containing retraction datoms (Op::Retract).
//! The store remains append-only (INV-STORE-001): retractions are new datoms
//! that record the retraction of a previously asserted fact.

use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::layout::TxFile;
use braid_kernel::Store;

use crate::error::BraidError;
use crate::layout::DiskLayout;

/// Find the current (most-recent, non-retracted) assertion datoms for an entity+attribute pair.
///
/// Scans all datoms for the entity with the given attribute, then filters to only
/// those assertions that have not been retracted by a later datom in the store.
fn find_active_assertions<'a>(
    store: &'a Store,
    entity: EntityId,
    attribute: &Attribute,
) -> Vec<&'a Datom> {
    let entity_datoms = store.entity_datoms(entity);

    // Collect all assertions and retractions for this attribute
    let mut assertions: Vec<&Datom> = Vec::new();
    let mut retracted_values: Vec<&Value> = Vec::new();

    for datom in &entity_datoms {
        if datom.attribute != *attribute {
            continue;
        }
        match datom.op {
            Op::Assert => assertions.push(datom),
            Op::Retract => retracted_values.push(&datom.value),
        }
    }

    // Filter out assertions whose value has already been retracted
    assertions
        .into_iter()
        .filter(|a| !retracted_values.contains(&&a.value))
        .collect()
}

pub fn run(
    path: &Path,
    agent_name: &str,
    entity_ident: &str,
    attribute_keyword: &str,
    value_filter: Option<&str>,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let agent = AgentId::from_name(agent_name);
    let entity = EntityId::from_ident(entity_ident);
    let attribute = Attribute::from_keyword(attribute_keyword);

    // Find active assertions for this entity+attribute
    let active = find_active_assertions(&store, entity, &attribute);

    if active.is_empty() {
        return Err(BraidError::Parse(format!(
            "no active assertion found for entity {} attribute {}",
            entity_ident, attribute_keyword,
        )));
    }

    // If a value filter is provided, narrow to matching assertions
    let to_retract: Vec<&Datom> = if let Some(val_str) = value_filter {
        let target_value = parse_value(val_str);
        let filtered: Vec<&Datom> = active
            .into_iter()
            .filter(|d| d.value == target_value)
            .collect();
        if filtered.is_empty() {
            return Err(BraidError::Parse(format!(
                "no active assertion with value {} found for entity {} attribute {}",
                val_str, entity_ident, attribute_keyword,
            )));
        }
        filtered
    } else {
        active
    };

    // Generate TxId: advance past the store's current frontier
    let current_wall = store
        .frontier()
        .values()
        .map(|tx| tx.wall_time())
        .max()
        .unwrap_or(0);
    let tx_id = TxId::new(current_wall + 1, 0, agent);

    // Create retraction datoms
    let datoms: Vec<Datom> = to_retract
        .iter()
        .map(|original| {
            Datom::new(
                original.entity,
                original.attribute.clone(),
                original.value.clone(),
                tx_id,
                Op::Retract,
            )
        })
        .collect();

    let datom_count = datoms.len();

    let tx = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Observed,
        rationale: format!(
            "retract {} {} ({})",
            entity_ident, attribute_keyword, datom_count
        ),
        causal_predecessors: vec![],
        datoms,
    };

    let file_path = layout.write_tx(&tx)?;

    Ok(format!(
        "retracted {} datom(s) for {} {} -> {}\n",
        datom_count,
        entity_ident,
        attribute_keyword,
        file_path.relative_path(),
    ))
}

/// Parse a CLI value string into a Value.
fn parse_value(s: &str) -> Value {
    // Try integer
    if let Ok(n) = s.parse::<i64>() {
        return Value::Long(n);
    }
    // Try float
    if let Ok(f) = s.parse::<f64>() {
        return Value::Double(ordered_float::OrderedFloat(f));
    }
    // Try boolean
    if s == "true" {
        return Value::Boolean(true);
    }
    if s == "false" {
        return Value::Boolean(false);
    }
    // Keyword (starts with :)
    if s.starts_with(':') {
        return Value::Keyword(s.to_string());
    }
    // Default: string
    Value::String(s.to_string())
}
