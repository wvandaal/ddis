//! `braid transact` — Assert datoms into the store.

use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::layout::TxFile;
use ordered_float::OrderedFloat;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(
    path: &Path,
    agent_name: &str,
    rationale: &str,
    datoms_raw: &[String],
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let agent = AgentId::from_name(agent_name);

    // Parse datom triples: each group of 3 strings = (entity, attribute, value)
    if datoms_raw.len() % 3 != 0 {
        return Err(BraidError::Parse(
            "datoms must be triples: entity attribute value".into(),
        ));
    }

    // Generate TxId: advance past the store's current frontier
    let current_wall = store
        .frontier()
        .values()
        .map(|tx| tx.wall_time())
        .max()
        .unwrap_or(0);
    let tx_id = TxId::new(current_wall + 1, 0, agent);

    let mut datoms = Vec::new();

    for chunk in datoms_raw.chunks(3) {
        let entity = EntityId::from_ident(&chunk[0]);
        let attribute = Attribute::from_keyword(&chunk[1]);
        let value = parse_value(&chunk[2]);

        datoms.push(braid_kernel::datom::Datom::new(
            entity,
            attribute,
            value,
            tx_id,
            Op::Assert,
        ));
    }

    if datoms.is_empty() {
        return Err(BraidError::Parse("no datoms to transact".into()));
    }

    let tx = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Observed,
        rationale: rationale.to_string(),
        causal_predecessors: vec![],
        datoms,
    };

    let datom_count = tx.datoms.len();
    let file_path = layout.write_tx(&tx)?;

    Ok(format!(
        "transacted {} datom(s) → {}\n",
        datom_count,
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
        return Value::Double(OrderedFloat(f));
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
