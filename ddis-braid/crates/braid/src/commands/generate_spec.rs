//! CLI `braid generate-spec` command: render store entities to spec markdown.
//!
//! This is the inverse of `braid bootstrap`. Where bootstrap parses markdown
//! into datoms, generate-spec renders datoms back to markdown. Together with
//! `braid promote`, this completes the store-first specification pipeline:
//!
//!   exploration doc → store → promote → generate-spec → spec/*.md

use std::collections::BTreeMap;
use std::path::Path;

use braid_kernel::datom::{Attribute, EntityId, Op, Value};

/// A spec element reconstructed from store datoms.
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

/// Execute the generate-spec command.
pub fn run(
    path: &Path,
    output_dir: &Path,
    namespace_filter: Option<&str>,
) -> Result<String, crate::error::BraidError> {
    let layout = crate::layout::DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Collect all entities that have :element/id (promoted spec elements)
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
        return Ok("generate-spec: no promoted elements found in store\n".to_string());
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

        // Only write if content differs or file doesn't exist
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
        "generate-spec: {total_elements} elements across {} namespaces\n  files written: {files_written}\n  output: {}\n",
        by_namespace.len(),
        output_dir.display(),
    ))
}

/// Extract the human-readable namespace name from a keyword like ":element.ns/topology".
fn extract_namespace_name(ns_keyword: &str) -> String {
    ns_keyword
        .strip_prefix(":element.ns/")
        .or_else(|| ns_keyword.strip_prefix(":spec.ns/"))
        .unwrap_or(ns_keyword)
        .to_uppercase()
}

/// Render a namespace's spec elements to markdown.
fn render_namespace_spec(namespace: &str, elements: &[&StoreElement]) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {} — Generated Specification\n\n", namespace));
    out.push_str("> Auto-generated by `braid generate-spec` from store entities.\n");
    out.push_str("> Edit the store (via `braid promote` / `braid transact`), not this file.\n\n");
    out.push_str("---\n\n");

    // Separate by type
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
