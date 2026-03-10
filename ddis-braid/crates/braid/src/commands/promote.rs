//! CLI `braid promote` command: promote exploration entities to spec elements.
//!
//! This implements the store-first specification pipeline. Instead of writing
//! markdown specs and parsing them into the store, exploration entities in the
//! store gain `:spec/*` and `:element/*` attributes via promotion.

use std::collections::BTreeSet;
use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, EntityId, Op, Value};
use braid_kernel::promote::{promote, verify_dual_identity, PromotionRequest, PromotionTargetType};

/// Arguments for the promote command.
pub struct PromoteArgs<'a> {
    /// Path to the .braid directory.
    pub path: &'a Path,
    /// Entity ident to promote.
    pub entity_ident: &'a str,
    /// Target spec element ID.
    pub target_id: &'a str,
    /// Target namespace.
    pub namespace: &'a str,
    /// Target type string (invariant, adr, negative-case).
    pub target_type: &'a str,
    /// Agent name.
    pub agent_name: &'a str,
    /// Formal statement text.
    pub statement: Option<&'a str>,
    /// Falsification condition.
    pub falsification: Option<&'a str>,
    /// Verification method.
    pub verification: Option<&'a str>,
    /// Problem statement.
    pub problem: Option<&'a str>,
    /// Decision text.
    pub decision: Option<&'a str>,
}

/// Execute the promote command.
pub fn run(args: PromoteArgs<'_>) -> Result<String, crate::error::BraidError> {
    let layout = crate::layout::DiskLayout::open(args.path)?;
    let store = layout.load_store()?;
    let datoms: BTreeSet<_> = store.datoms().cloned().collect();

    let entity = EntityId::from_ident(args.entity_ident);

    // Verify the entity exists in the store
    let entity_exists = datoms
        .iter()
        .any(|d| d.entity == entity && d.op == Op::Assert);
    if !entity_exists {
        return Err(crate::error::BraidError::Parse(format!(
            "Entity {} not found in store",
            args.entity_ident
        )));
    }

    // Resolve target type
    let ptype = match args.target_type {
        "invariant" | "inv" => PromotionTargetType::Invariant,
        "adr" => PromotionTargetType::Adr,
        "negative-case" | "neg" => PromotionTargetType::NegativeCase,
        other => {
            return Err(crate::error::BraidError::Parse(format!(
                "Unknown target type: {other}. Use: invariant, adr, negative-case"
            )));
        }
    };

    let agent = AgentId::from_name(args.agent_name);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let tx_id = braid_kernel::datom::TxId::new(now_ms, 0, agent);

    let request = PromotionRequest {
        entity,
        target_element_id: args.target_id.to_string(),
        target_namespace: args.namespace.to_string(),
        target_type: ptype,
        statement: args.statement.map(|s| s.to_string()),
        falsification: args.falsification.map(|s| s.to_string()),
        verification: args.verification.map(|s| s.to_string()),
        problem: args.problem.map(|s| s.to_string()),
        decision: args.decision.map(|s| s.to_string()),
    };

    let result = promote(&request, &datoms, tx_id);

    if result.was_noop {
        return Ok(format!(
            "promote: {} already promoted to {} (no-op)\n",
            args.entity_ident, args.target_id
        ));
    }

    // Write the promotion datoms as a new transaction
    let tx_file = braid_kernel::layout::TxFile {
        tx_id,
        agent,
        provenance: braid_kernel::datom::ProvenanceType::Derived,
        rationale: format!(
            "Promote {} → {} ({})",
            args.entity_ident, args.target_id, args.target_type
        ),
        causal_predecessors: vec![],
        datoms: result.datoms,
    };

    let file_path = layout.write_tx(&tx_file)?;

    // Verify dual identity after promotion
    let mut updated_datoms = datoms;
    for d in &tx_file.datoms {
        updated_datoms.insert(d.clone());
    }
    let check = verify_dual_identity(entity, &updated_datoms);

    let mut output = format!(
        "promote: {} → {}\n  type: {}\n  namespace: {}\n  attrs added: {}\n  → {}\n",
        args.entity_ident,
        args.target_id,
        args.target_type,
        args.namespace,
        result.attrs_added,
        file_path.relative_path(),
    );

    // Report title from exploration if available
    let title = updated_datoms
        .iter()
        .find(|d| {
            d.entity == entity
                && d.op == Op::Assert
                && d.attribute == Attribute::from_keyword(":exploration/title")
        })
        .and_then(|d| match &d.value {
            Value::String(s) => Some(s.clone()),
            _ => None,
        });
    if let Some(t) = title {
        output.push_str(&format!("  title: {t}\n"));
    }

    // INV-PROMOTE-002 verification
    if check.is_valid {
        output.push_str("  dual-identity: PASS (has :exploration/*, :element/*, :promotion/*)\n");
    } else {
        output.push_str(&format!(
            "  dual-identity: FAIL (exploration={}, element={}, promotion={})\n",
            check.has_exploration, check.has_element, check.has_promotion
        ));
    }

    Ok(output)
}
