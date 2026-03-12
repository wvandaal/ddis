//! `braid spec create` — Zero-friction spec element creation (WP5).
//!
//! Creates a spec entity with all required attributes in one command.
//! Auto-detects type from ID prefix: INV- → invariant, ADR- → adr, NEG- → negative-case.
//!
//! Traces to: C5 (traceability), C6 (falsifiability), INV-INTERFACE-011 (CLI as prompt).

use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, Value};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::layout::DiskLayout;

/// Arguments for `braid spec create`.
pub struct CreateArgs<'a> {
    pub path: &'a Path,
    pub id: &'a str,
    pub title: &'a str,
    pub statement: Option<&'a str>,
    pub falsification: Option<&'a str>,
    pub problem: Option<&'a str>,
    pub decision: Option<&'a str>,
    pub traces_to: Option<&'a str>,
    pub confidence: Option<f64>,
    pub agent: &'a str,
}

/// Run `braid spec create`.
pub fn run_create(args: CreateArgs<'_>) -> Result<String, BraidError> {
    // Auto-detect type from ID prefix
    let element_type = if args.id.starts_with("INV-") {
        "invariant"
    } else if args.id.starts_with("ADR-") {
        "adr"
    } else if args.id.starts_with("NEG-") {
        "negative-case"
    } else {
        return Err(BraidError::Validation(format!(
            "Spec ID must start with INV-, ADR-, or NEG-. Got: {}",
            args.id
        )));
    };

    // Extract namespace from ID: INV-STORE-001 → STORE
    let namespace = extract_namespace(args.id).ok_or_else(|| {
        BraidError::Validation(format!(
            "Cannot extract namespace from ID: {}. Expected format: INV-NAMESPACE-NNN",
            args.id
        ))
    })?;

    let layout = DiskLayout::open(args.path)?;
    let store = layout.load_store()?;

    let agent_id = AgentId::from_name(args.agent);
    let tx_id = super::write::next_tx_id(&store, agent_id);

    // Build entity ident: :spec/inv-store-001 (lowercase)
    let ident = format!(":spec/{}", args.id.to_lowercase());
    let entity = EntityId::from_ident(&ident);

    let mut datoms = vec![
        Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident.clone()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":spec/id"),
            Value::String(args.id.to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":spec/element-type"),
            Value::Keyword(format!(":spec.type/{element_type}")),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":spec/namespace"),
            Value::Keyword(format!(":spec.ns/{namespace}")),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(args.title.to_string()),
            tx_id,
            Op::Assert,
        ),
    ];

    // Type-specific attributes
    if let Some(stmt) = args.statement {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":spec/statement"),
            Value::String(stmt.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    if let Some(fals) = args.falsification {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":spec/falsification"),
            Value::String(fals.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    if let Some(prob) = args.problem {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":adr/problem"),
            Value::String(prob.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    if let Some(dec) = args.decision {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":adr/decision"),
            Value::String(dec.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    if let Some(traces) = args.traces_to {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":element/traces-to"),
            Value::String(traces.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    if let Some(conf) = args.confidence {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":spec/confidence"),
            Value::Double(ordered_float::OrderedFloat(conf)),
            tx_id,
            Op::Assert,
        ));
    }

    let datom_count = datoms.len();

    let tx = TxFile {
        tx_id,
        agent: agent_id,
        provenance: ProvenanceType::Observed,
        rationale: format!("spec create: {} \"{}\"", args.id, args.title),
        causal_predecessors: vec![],
        datoms,
    };

    layout.write_tx(&tx)?;

    // Reload to get updated counts
    let store = layout.load_store()?;

    Ok(format!(
        "created: {} ({}, namespace={})\nstore: +{} datoms ({} total)\n\nnext: braid trace --commit (to link implementations) | ref: C5 traceability\n",
        ident,
        element_type,
        namespace,
        datom_count,
        store.datom_set().len(),
    ))
}

/// Extract namespace from spec ID: INV-STORE-001 → STORE
fn extract_namespace(id: &str) -> Option<String> {
    // Skip prefix (INV-, ADR-, NEG-)
    let rest = if id.starts_with("INV-") || id.starts_with("ADR-") || id.starts_with("NEG-") {
        &id[4..]
    } else {
        return None;
    };

    // Find the namespace (uppercase letters before the next hyphen+digits)
    let ns_end = rest.find(|c: char| !c.is_ascii_uppercase() && c != '_');
    if let Some(end) = ns_end {
        if end > 0 && rest[end..].starts_with('-') {
            return Some(rest[..end].to_string());
        }
    }

    // If no digits follow, the whole rest is the namespace (e.g., "INV-STORE")
    if rest.chars().all(|c| c.is_ascii_uppercase() || c == '_') && !rest.is_empty() {
        Some(rest.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_namespace_from_inv() {
        assert_eq!(
            extract_namespace("INV-STORE-001"),
            Some("STORE".to_string())
        );
    }

    #[test]
    fn extract_namespace_from_adr() {
        assert_eq!(
            extract_namespace("ADR-INTERFACE-010"),
            Some("INTERFACE".to_string())
        );
    }

    #[test]
    fn extract_namespace_from_neg() {
        assert_eq!(
            extract_namespace("NEG-MUTATION-001"),
            Some("MUTATION".to_string())
        );
    }

    #[test]
    fn extract_namespace_multi_word() {
        assert_eq!(
            extract_namespace("INV-BILATERAL-005"),
            Some("BILATERAL".to_string())
        );
    }

    #[test]
    fn extract_namespace_invalid() {
        assert_eq!(extract_namespace("FOO-STORE-001"), None);
        assert_eq!(extract_namespace("INV-"), None);
    }
}
