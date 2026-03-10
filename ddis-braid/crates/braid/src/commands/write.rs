//! `braid write` — Unified store mutation command.
//!
//! All structured store modifications go through subcommands:
//! - `write assert`: add new datoms to the store (default)
//! - `write retract`: retract existing assertions (append-only: creates retraction datoms)
//! - `write promote`: promote exploration entity to formal spec element
//! - `write export`: render store entities to spec/*.md
//!
//! Traces to: INV-STORE-001 (append-only), C1 (immutability), C2 (content-addressable),
//!            INV-INTERFACE-010 (CLI/MCP equivalence)

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::layout::TxFile;
use braid_kernel::promote::{promote, verify_dual_identity, PromotionRequest, PromotionTargetType};
use braid_kernel::Store;
use ordered_float::OrderedFloat;

use crate::error::BraidError;
use crate::layout::DiskLayout;

// ── Shared Helpers ──────────────────────────────────────────────────────────

/// Parse a CLI value string into a datom Value.
///
/// Tries in order: integer, float, boolean, keyword (:prefix), string.
pub(crate) fn parse_value(s: &str) -> Value {
    if let Ok(n) = s.parse::<i64>() {
        return Value::Long(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::Double(OrderedFloat(f));
    }
    if s == "true" {
        return Value::Boolean(true);
    }
    if s == "false" {
        return Value::Boolean(false);
    }
    if s.starts_with(':') {
        return Value::Keyword(s.to_string());
    }
    Value::String(s.to_string())
}

/// Generate a TxId that advances past the store's current frontier.
///
/// Uses `max(current_wall + 1, unix_epoch_seconds)` to ensure:
/// - Monotonicity: never goes backward
/// - Seamless migration: old stores (wall_time=0..17) jump to real time
/// - Git integration: wall_time maps directly to `git log --after=`
pub fn next_tx_id(store: &Store, agent: AgentId) -> TxId {
    let current_wall = store
        .frontier()
        .values()
        .map(|tx| tx.wall_time())
        .max()
        .unwrap_or(0);
    let unix_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    TxId::new(current_wall.max(unix_now) + 1, 0, agent)
}

// ── ASSERT ──────────────────────────────────────────────────────────────────

/// Assert datoms into the store.
pub fn run_assert(
    path: &Path,
    agent_name: &str,
    rationale: &str,
    datoms_raw: &[String],
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let agent = AgentId::from_name(agent_name);

    if datoms_raw.len() % 3 != 0 {
        return Err(BraidError::Parse(
            "datoms must be triples: entity attribute value".into(),
        ));
    }

    let tx_id = next_tx_id(&store, agent);

    let mut datoms = Vec::new();
    for chunk in datoms_raw.chunks(3) {
        let entity = EntityId::from_ident(&chunk[0]);
        let attribute = Attribute::from_keyword(&chunk[1]);
        let value = parse_value(&chunk[2]);

        datoms.push(Datom::new(entity, attribute, value, tx_id, Op::Assert));
    }

    if datoms.is_empty() {
        return Err(BraidError::Parse("no datoms to assert".into()));
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
        "asserted {} datom(s) \u{2192} {}\n",
        datom_count,
        file_path.relative_path(),
    ))
}

// ── RETRACT ─────────────────────────────────────────────────────────────────

/// Find active (non-retracted) assertion datoms for an entity+attribute pair.
fn find_active_assertions<'a>(
    store: &'a Store,
    entity: EntityId,
    attribute: &Attribute,
) -> Vec<&'a Datom> {
    let entity_datoms = store.entity_datoms(entity);

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

    assertions
        .into_iter()
        .filter(|a| !retracted_values.contains(&&a.value))
        .collect()
}

/// Retract existing assertions from the store.
///
/// Creates retraction datoms (Op::Retract). The store remains append-only
/// (INV-STORE-001): retractions are new datoms that record the retraction.
pub fn run_retract(
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

    let active = find_active_assertions(&store, entity, &attribute);

    if active.is_empty() {
        return Err(BraidError::Parse(format!(
            "no active assertion found for entity {} attribute {}",
            entity_ident, attribute_keyword,
        )));
    }

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

    let tx_id = next_tx_id(&store, agent);

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
        "retracted {} datom(s) for {} {} \u{2192} {}\n",
        datom_count,
        entity_ident,
        attribute_keyword,
        file_path.relative_path(),
    ))
}

// ── PROMOTE ─────────────────────────────────────────────────────────────────

/// Arguments for the promote subcommand.
pub struct PromoteArgs<'a> {
    pub path: &'a Path,
    pub entity_ident: &'a str,
    pub target_id: &'a str,
    pub namespace: &'a str,
    pub target_type: &'a str,
    pub agent_name: &'a str,
    pub statement: Option<&'a str>,
    pub falsification: Option<&'a str>,
    pub verification: Option<&'a str>,
    pub problem: Option<&'a str>,
    pub decision: Option<&'a str>,
}

/// Promote an exploration entity to a formal spec element.
pub fn run_promote(args: PromoteArgs<'_>) -> Result<String, BraidError> {
    let layout = DiskLayout::open(args.path)?;
    let store = layout.load_store()?;
    let datoms: BTreeSet<_> = store.datoms().cloned().collect();

    let entity = EntityId::from_ident(args.entity_ident);

    let entity_exists = datoms
        .iter()
        .any(|d| d.entity == entity && d.op == Op::Assert);
    if !entity_exists {
        return Err(BraidError::Parse(format!(
            "Entity {} not found in store",
            args.entity_ident
        )));
    }

    let ptype = match args.target_type {
        "invariant" | "inv" => PromotionTargetType::Invariant,
        "adr" => PromotionTargetType::Adr,
        "negative-case" | "neg" => PromotionTargetType::NegativeCase,
        other => {
            return Err(BraidError::Parse(format!(
                "Unknown target type: {other}. Use: invariant, adr, negative-case"
            )));
        }
    };

    let agent = AgentId::from_name(args.agent_name);
    let tx_id = next_tx_id(&store, agent);

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
            "promoted: {} already promoted to {} (no-op)\n",
            args.entity_ident, args.target_id
        ));
    }

    let tx_file = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: format!(
            "Promote {} \u{2192} {} ({})",
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
        "promoted: {} \u{2192} {}\n  type: {}\n  namespace: {}\n  attrs added: {}\n  \u{2192} {}\n",
        args.entity_ident,
        args.target_id,
        args.target_type,
        args.namespace,
        result.attrs_added,
        file_path.relative_path(),
    );

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

// ── EXPORT ──────────────────────────────────────────────────────────────────

/// A spec element reconstructed from store datoms for export rendering.
#[derive(Clone, Debug)]
struct StoreElement {
    id: String,
    element_type: String,
    namespace: String,
    title: String,
    body: String,
    statement: String,
    falsification: String,
    verification: String,
    problem: String,
    decision_text: String,
    violation: String,
    status: String,
    confidence: Option<f64>,
    traces_to: Vec<String>,
}

/// Render store entities to spec/*.md (inverse of bootstrap).
pub fn run_export(
    path: &Path,
    output_dir: &Path,
    namespace_filter: Option<&str>,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let mut elements: BTreeMap<EntityId, StoreElement> = BTreeMap::new();

    // First pass: find all entities with :element/id
    for d in store.datoms().filter(|d| d.op == Op::Assert) {
        if d.attribute == Attribute::from_keyword(":element/id") {
            if let Value::String(ref id) = d.value {
                elements.entry(d.entity).or_insert_with(|| StoreElement {
                    id: id.clone(),
                    element_type: String::new(),
                    namespace: String::new(),
                    title: String::new(),
                    body: String::new(),
                    statement: String::new(),
                    falsification: String::new(),
                    verification: String::new(),
                    problem: String::new(),
                    decision_text: String::new(),
                    violation: String::new(),
                    status: String::new(),
                    confidence: None,
                    traces_to: Vec::new(),
                });
            }
        }
    }

    // Second pass: populate fields
    for d in store.datoms().filter(|d| d.op == Op::Assert) {
        if let Some(elem) = elements.get_mut(&d.entity) {
            let attr = d.attribute.as_str();
            match attr {
                ":element/type" => {
                    if let Value::Keyword(ref k) = d.value {
                        elem.element_type = k.clone();
                    }
                }
                ":element/namespace" => {
                    if let Value::Keyword(ref k) = d.value {
                        elem.namespace = k.clone();
                    }
                }
                ":element/title" | ":exploration/title" => {
                    if let Value::String(ref s) = d.value {
                        if elem.title.is_empty() || attr == ":element/title" {
                            elem.title = s.clone();
                        }
                    }
                }
                ":element/body" | ":exploration/body" => {
                    if let Value::String(ref s) = d.value {
                        if elem.body.is_empty() || attr == ":element/body" {
                            elem.body = s.clone();
                        }
                    }
                }
                ":element/status" => {
                    if let Value::Keyword(ref k) = d.value {
                        elem.status = k.clone();
                    }
                }
                ":element/confidence" => {
                    if let Value::Double(f) = d.value {
                        elem.confidence = Some(f.into_inner());
                    }
                }
                ":element/traces-to" => {
                    if let Value::String(ref s) = d.value {
                        elem.traces_to.push(s.clone());
                    }
                }
                ":inv/statement" => {
                    if let Value::String(ref s) = d.value {
                        elem.statement = s.clone();
                    }
                }
                ":inv/falsification" => {
                    if let Value::String(ref s) = d.value {
                        elem.falsification = s.clone();
                    }
                }
                ":inv/verification" => {
                    if let Value::String(ref s) = d.value {
                        elem.verification = s.clone();
                    }
                }
                ":adr/problem" => {
                    if let Value::String(ref s) = d.value {
                        elem.problem = s.clone();
                    }
                }
                ":adr/decision" => {
                    if let Value::String(ref s) = d.value {
                        elem.decision_text = s.clone();
                    }
                }
                ":neg/violation" => {
                    if let Value::String(ref s) = d.value {
                        elem.violation = s.clone();
                    }
                }
                ":db/doc" => {
                    if let Value::String(ref s) = d.value {
                        if elem.title.is_empty() {
                            elem.title = s.clone();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Filter by namespace if requested
    let namespace_kw = namespace_filter.map(|ns| format!(":element.ns/{}", ns.to_lowercase()));
    let filtered: Vec<&StoreElement> = elements
        .values()
        .filter(|e| {
            if let Some(ref ns) = namespace_kw {
                e.namespace == *ns
            } else {
                true
            }
        })
        .collect();

    if filtered.is_empty() {
        return Ok("export: no promoted elements found in store\n".to_string());
    }

    // Group by namespace
    let mut by_namespace: BTreeMap<String, Vec<&StoreElement>> = BTreeMap::new();
    for elem in &filtered {
        let ns = extract_namespace_name(&elem.namespace);
        by_namespace.entry(ns).or_default().push(elem);
    }

    // Ensure output directory exists
    std::fs::create_dir_all(output_dir)?;

    let mut files_written = 0;
    let mut total_elements = 0;

    for (ns_name, elems) in &by_namespace {
        let filename = format!("{}.md", ns_name.to_lowercase());
        let filepath = output_dir.join(&filename);

        let content = render_namespace_spec(ns_name, elems);
        total_elements += elems.len();

        let should_write = match std::fs::read_to_string(&filepath) {
            Ok(existing) => existing != content,
            Err(_) => true,
        };

        if should_write {
            std::fs::write(&filepath, &content)?;
            files_written += 1;
        }
    }

    Ok(format!(
        "exported: {total_elements} elements across {} namespaces\n  files written: {files_written}\n  output: {}\n",
        by_namespace.len(),
        output_dir.display(),
    ))
}

// ── Export rendering helpers ────────────────────────────────────────────────

fn extract_namespace_name(ns_keyword: &str) -> String {
    ns_keyword
        .strip_prefix(":element.ns/")
        .or_else(|| ns_keyword.strip_prefix(":spec.ns/"))
        .unwrap_or(ns_keyword)
        .to_uppercase()
}

fn render_namespace_spec(namespace: &str, elements: &[&StoreElement]) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "# {} \u{2014} Generated Specification\n\n",
        namespace
    ));
    out.push_str("> Auto-generated by `braid write export` from store entities.\n");
    out.push_str(
        "> Edit the store (via `braid write promote` / `braid write assert`), not this file.\n\n",
    );
    out.push_str("---\n\n");

    let mut invariants: Vec<&&StoreElement> = elements
        .iter()
        .filter(|e| e.element_type.contains("invariant"))
        .collect();
    let mut adrs: Vec<&&StoreElement> = elements
        .iter()
        .filter(|e| e.element_type.contains("adr"))
        .collect();
    let mut negs: Vec<&&StoreElement> = elements
        .iter()
        .filter(|e| e.element_type.contains("negative"))
        .collect();

    invariants.sort_by(|a, b| a.id.cmp(&b.id));
    adrs.sort_by(|a, b| a.id.cmp(&b.id));
    negs.sort_by(|a, b| a.id.cmp(&b.id));

    if !invariants.is_empty() {
        out.push_str("## Invariants\n\n");
        for elem in &invariants {
            render_invariant(&mut out, elem);
        }
    }

    if !adrs.is_empty() {
        out.push_str("## Architecture Decision Records\n\n");
        for elem in &adrs {
            render_adr(&mut out, elem);
        }
    }

    if !negs.is_empty() {
        out.push_str("## Negative Cases\n\n");
        for elem in &negs {
            render_neg(&mut out, elem);
        }
    }

    out
}

fn render_invariant(out: &mut String, elem: &StoreElement) {
    out.push_str(&format!("### {}: {}\n\n", elem.id, elem.title));
    if !elem.traces_to.is_empty() {
        out.push_str(&format!("- **Traces to**: {}\n", elem.traces_to.join(", ")));
    }
    out.push_str("- **Type**: Invariant\n");
    if !elem.statement.is_empty() {
        out.push_str(&format!("- **Statement**: {}\n", elem.statement));
    }
    if !elem.falsification.is_empty() {
        out.push_str(&format!("- **Falsification**: {}\n", elem.falsification));
    }
    if !elem.verification.is_empty() {
        out.push_str(&format!("- **Verification**: {}\n", elem.verification));
    }
    if let Some(conf) = elem.confidence {
        out.push_str(&format!("- **Confidence**: {conf:.2}\n"));
    }
    if !elem.body.is_empty() {
        out.push_str(&format!("\n{}\n", elem.body));
    }
    out.push('\n');
}

fn render_adr(out: &mut String, elem: &StoreElement) {
    out.push_str(&format!("### {}: {}\n\n", elem.id, elem.title));
    if !elem.traces_to.is_empty() {
        out.push_str(&format!("- **Traces to**: {}\n", elem.traces_to.join(", ")));
    }
    out.push_str("- **Type**: ADR\n");
    if !elem.problem.is_empty() {
        out.push_str(&format!("\n**Problem**: {}\n", elem.problem));
    }
    if !elem.decision_text.is_empty() {
        out.push_str(&format!("\n**Decision**: {}\n", elem.decision_text));
    }
    if !elem.body.is_empty() {
        out.push_str(&format!("\n{}\n", elem.body));
    }
    out.push('\n');
}

fn render_neg(out: &mut String, elem: &StoreElement) {
    out.push_str(&format!("### {}: {}\n\n", elem.id, elem.title));
    if !elem.traces_to.is_empty() {
        out.push_str(&format!("- **Traces to**: {}\n", elem.traces_to.join(", ")));
    }
    out.push_str("- **Type**: Negative Case\n");
    if !elem.violation.is_empty() {
        out.push_str(&format!("- **Violation**: {}\n", elem.violation));
    }
    if !elem.body.is_empty() {
        out.push_str(&format!("\n{}\n", elem.body));
    }
    out.push('\n');
}
