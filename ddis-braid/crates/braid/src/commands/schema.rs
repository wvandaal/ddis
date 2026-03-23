//! `braid schema` — Inspect the datom store schema.
//!
//! Lists all known attributes with their type, cardinality, and documentation.
//! Optimized for AI agent consumption: provides the information needed to
//! write correct queries and transactions (INV-INTERFACE-011).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use braid_kernel::datom::{Op, Value};
use braid_kernel::schema::{AttributeDef, Schema};
use braid_kernel::Attribute;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

// ---------------------------------------------------------------------------
// Diff entry — shared between run_diff, format_diff_human, build_diff_json
// ---------------------------------------------------------------------------

/// A schema attribute discovered after a given transaction wall-time.
struct DiffEntry {
    ident: String,
    value_type: String,
    doc: String,
    tx_wall: u64,
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Run the schema introspection command.
pub fn run(
    path: &Path,
    pattern: Option<&str>,
    verbose: bool,
    json: bool,
    diff_since: Option<u64>,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // --diff mode: show only attributes added since a given transaction wall-time.
    if let Some(since_tx) = diff_since {
        return run_diff(&store, since_tx, pattern, json);
    }

    let schema = store.schema();

    // Collect and filter attributes.
    let mut attrs: Vec<(&Attribute, &AttributeDef)> = schema
        .attributes()
        .filter(|(attr, _)| match pattern {
            Some(pat) => {
                // Support glob-like filtering: ":spec/*" matches all :spec/ attrs
                if let Some(prefix) = pat.strip_suffix('*') {
                    attr.as_str().starts_with(prefix)
                } else {
                    attr.as_str() == pat || attr.as_str().contains(pat)
                }
            }
            None => true,
        })
        .collect();

    // Sort alphabetically by attribute name.
    attrs.sort_by_key(|(a, _)| a.as_str().to_string());

    // Build the human-readable output.
    let human = if attrs.is_empty() {
        let known_ns = get_namespaces(schema);
        format!(
            "No attributes match '{}'\nKnown namespaces: {}\nTry: braid schema --pattern ':db/*' or braid schema --pattern ':spec/*'\n",
            pattern.unwrap_or(""),
            known_ns.join(", ")
        )
    } else if json {
        // Legacy --json flag: produce the JSON string as human output.
        let structured = build_structured_json(&attrs);
        serde_json::to_string_pretty(&structured).unwrap() + "\n"
    } else if verbose {
        format_verbose(&store, &attrs)?
    } else {
        format_terse(&attrs)?
    };

    // Build structured JSON (always, regardless of --json flag).
    let structured_json = build_structured_json(&attrs);

    // ACP projection for schema (INV-BUDGET-007)
    // Action = "braid query --attribute :db/valueType" (explore schema)
    // Context = attribute list as budget-scaled blocks
    // Evidence = "braid schema --pattern ':spec/*'"
    let action = braid_kernel::budget::ProjectedAction {
        command: "braid query '[:find ?e ?v :where [?e :db/valueType ?v]]'".to_string(),
        rationale: "explore schema value types".to_string(),
        impact: 0.3,
    };

    let mut context_blocks = Vec::new();

    // Summary (System)
    let ctx_label = if let Some(pat) = pattern {
        format!("schema: {} attributes (pattern: {pat})", attrs.len())
    } else {
        format!("schema: {} attributes", attrs.len())
    };
    context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
        braid_kernel::budget::OutputPrecedence::System,
        ctx_label,
        8,
    ));

    // Namespace breakdown (UserRequested)
    let ns_counts = {
        let mut ns: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for (attr, _) in &attrs {
            let s = attr.as_str();
            let prefix = match s.find('/') {
                Some(pos) => &s[..pos],
                None => s,
            };
            *ns.entry(prefix.to_string()).or_insert(0) += 1;
        }
        ns
    };
    let ns_summary: Vec<String> = ns_counts
        .iter()
        .map(|(ns, count)| format!("{ns}:{count}"))
        .collect();
    if !ns_summary.is_empty() {
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::UserRequested,
            format!("namespaces: {}", ns_summary.join(", ")),
            10 + ns_summary.len(),
        ));
    }

    // Individual attributes as Speculative blocks (capped at 30)
    let max_attr_blocks = 30;
    for (i, (attr, def)) in attrs.iter().take(max_attr_blocks).enumerate() {
        let precedence = if i < 10 {
            braid_kernel::budget::OutputPrecedence::Speculative
        } else {
            braid_kernel::budget::OutputPrecedence::Ambient
        };
        let doc_short = truncate_doc(&def.doc, 40);
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            precedence,
            format!(
                "{} {} {} \"{}\"",
                attr.as_str(),
                type_short_name(def),
                cardinality_short_name(def),
                doc_short
            ),
            8,
        ));
    }

    if attrs.len() > max_attr_blocks {
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::Ambient,
            format!("... and {} more", attrs.len() - max_attr_blocks),
            3,
        ));
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: "braid schema --pattern ':spec/*'".to_string(),
    };

    // Merge ACP into JSON
    let mut final_json = structured_json;
    if let serde_json::Value::Object(ref mut map) = final_json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    Ok(CommandOutput {
        json: final_json,
        agent,
        human,
    })
}

// ---------------------------------------------------------------------------
// --diff implementation
// ---------------------------------------------------------------------------

/// Diff mode: find schema attributes whose :db/valueType datom was asserted after `since_tx`.
///
/// Walks the :db/valueType datoms to find entities installed after the threshold,
/// then resolves :db/ident and :db/doc from those entities' datom sets.
fn run_diff(
    store: &braid_kernel::Store,
    since_tx: u64,
    pattern: Option<&str>,
    json: bool,
) -> Result<CommandOutput, BraidError> {
    let vt_attr = Attribute::from_keyword(":db/valueType");

    // Collect entity IDs where :db/valueType was asserted after since_tx.
    let new_entity_ids: BTreeSet<_> = store
        .attribute_datoms(&vt_attr)
        .iter()
        .filter(|d| d.op == Op::Assert && d.tx.wall_time() > since_tx)
        .map(|d| d.entity)
        .collect();

    if new_entity_ids.is_empty() {
        let human = format!("schema diff: 0 attributes added since tx {since_tx}\n");
        // ACP for empty diff result
        let action = braid_kernel::budget::ProjectedAction {
            command: "braid query '[:find ?e ?v :where [?e :db/valueType ?v]]'".to_string(),
            rationale: "explore schema value types".to_string(),
            impact: 0.2,
        };
        let projection = braid_kernel::ActionProjection {
            action,
            context: vec![braid_kernel::budget::ContextBlock::new_scored(
                braid_kernel::budget::OutputPrecedence::System,
                format!("schema diff: 0 new attributes since tx {since_tx}"),
                8,
            )],
            evidence_pointer: "braid schema --pattern ':spec/*'".to_string(),
        };
        let mut structured = serde_json::json!({
            "since_tx": since_tx,
            "count": 0,
            "attributes": [],
        });
        if let serde_json::Value::Object(ref mut map) = structured {
            let acp = projection.to_json();
            if let serde_json::Value::Object(acp_map) = acp {
                for (k, v) in acp_map {
                    map.insert(k, v);
                }
            }
        }
        let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
        let agent = AgentOutput {
            context: String::new(),
            content: agent_text,
            footer: String::new(),
        };
        return Ok(CommandOutput {
            json: structured,
            agent,
            human,
        });
    }

    // For each new entity, resolve :db/ident, :db/valueType, and :db/doc.
    let ident_attr = Attribute::from_keyword(":db/ident");
    let doc_attr = Attribute::from_keyword(":db/doc");

    let mut entries: Vec<DiffEntry> = Vec::new();

    // Index :db/ident datoms by entity for fast lookup.
    let ident_by_entity: BTreeMap<_, _> = store
        .attribute_datoms(&ident_attr)
        .iter()
        .filter(|d| d.op == Op::Assert)
        .map(|d| (d.entity, d))
        .collect();

    // Index :db/doc datoms by entity.
    let doc_by_entity: BTreeMap<_, _> = store
        .attribute_datoms(&doc_attr)
        .iter()
        .filter(|d| d.op == Op::Assert)
        .map(|d| (d.entity, d))
        .collect();

    // Index :db/valueType datoms by entity (pick the one after since_tx).
    let vt_by_entity: BTreeMap<_, _> = store
        .attribute_datoms(&vt_attr)
        .iter()
        .filter(|d| d.op == Op::Assert && d.tx.wall_time() > since_tx)
        .map(|d| (d.entity, d))
        .collect();

    for &eid in &new_entity_ids {
        let ident = ident_by_entity.get(&eid).and_then(|d| match &d.value {
            Value::Keyword(k) => Some(k.clone()),
            _ => None,
        });

        let ident_str = match ident {
            Some(ref s) => s.as_str(),
            None => continue, // Skip entities without :db/ident
        };

        // Apply pattern filter if provided.
        if let Some(pat) = pattern {
            let matches = if let Some(prefix) = pat.strip_suffix('*') {
                ident_str.starts_with(prefix)
            } else {
                ident_str == pat || ident_str.contains(pat)
            };
            if !matches {
                continue;
            }
        }

        let value_type = vt_by_entity
            .get(&eid)
            .and_then(|d| match &d.value {
                Value::Keyword(k) => Some(k.clone()),
                _ => None,
            })
            .unwrap_or_default();

        let doc = doc_by_entity
            .get(&eid)
            .and_then(|d| match &d.value {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        let tx_wall = vt_by_entity
            .get(&eid)
            .map(|d| d.tx.wall_time())
            .unwrap_or(0);

        entries.push(DiffEntry {
            ident: ident.unwrap(),
            value_type,
            doc,
            tx_wall,
        });
    }

    // Sort alphabetically by attribute name.
    entries.sort_by(|a, b| a.ident.cmp(&b.ident));

    // Build output.
    let count = entries.len();

    let human = if json {
        let structured = build_diff_json(since_tx, &entries);
        serde_json::to_string_pretty(&structured).unwrap() + "\n"
    } else {
        format_diff_human(since_tx, &entries)
    };

    let structured_json = build_diff_json(since_tx, &entries);

    // ACP for diff result
    let action = braid_kernel::budget::ProjectedAction {
        command: "braid query '[:find ?e ?v :where [?e :db/valueType ?v]]'".to_string(),
        rationale: "explore schema value types".to_string(),
        impact: 0.3,
    };
    let mut diff_context_blocks = vec![braid_kernel::budget::ContextBlock::new_scored(
        braid_kernel::budget::OutputPrecedence::System,
        format!("schema diff: {count} attributes added since tx {since_tx}"),
        8,
    )];
    for (i, entry) in entries.iter().take(20).enumerate() {
        let precedence = if i < 5 {
            braid_kernel::budget::OutputPrecedence::UserRequested
        } else {
            braid_kernel::budget::OutputPrecedence::Speculative
        };
        diff_context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            precedence,
            format!(
                "+ {} {} \"{}\"",
                entry.ident,
                entry.value_type,
                truncate_doc(&entry.doc, 40)
            ),
            8,
        ));
    }
    if count > 20 {
        diff_context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            braid_kernel::budget::OutputPrecedence::Ambient,
            format!("... and {} more", count - 20),
            3,
        ));
    }
    let projection = braid_kernel::ActionProjection {
        action,
        context: diff_context_blocks,
        evidence_pointer: "braid schema --pattern ':spec/*'".to_string(),
    };

    // Merge ACP into JSON
    let mut final_json = structured_json;
    if let serde_json::Value::Object(ref mut map) = final_json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    Ok(CommandOutput {
        json: final_json,
        agent,
        human,
    })
}

/// Format diff results as human-readable text.
fn format_diff_human(since_tx: u64, entries: &[DiffEntry]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "schema diff: {} attributes added since tx {}\n\n",
        entries.len(),
        since_tx
    ));

    if entries.is_empty() {
        return out;
    }

    let max_name_len = entries.iter().map(|e| e.ident.len()).max().unwrap_or(0);

    for entry in entries {
        let doc = truncate_doc(&entry.doc, 50);
        out.push_str(&format!(
            "  + {:<width$}  {:<12}  tx={:<14}  \"{}\"\n",
            entry.ident,
            entry.value_type,
            entry.tx_wall,
            doc,
            width = max_name_len,
        ));
    }

    out
}

/// Build structured JSON for diff results.
fn build_diff_json(since_tx: u64, entries: &[DiffEntry]) -> serde_json::Value {
    let attr_json: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "name": e.ident,
                "type": e.value_type,
                "doc": e.doc,
                "tx_wall_time": e.tx_wall,
            })
        })
        .collect();

    serde_json::json!({
        "since_tx": since_tx,
        "count": attr_json.len(),
        "attributes": attr_json,
    })
}

// ---------------------------------------------------------------------------
// Existing schema list formatting
// ---------------------------------------------------------------------------

/// Terse two-column table: attribute, type, cardinality, doc (truncated).
fn format_terse(attrs: &[(&Attribute, &AttributeDef)]) -> Result<String, BraidError> {
    // Compute column width for attribute names.
    let max_name_len = attrs
        .iter()
        .map(|(a, _)| a.as_str().len())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    out.push_str(&format!("schema: {} attributes\n\n", attrs.len()));

    for (attr, def) in attrs {
        let name = attr.as_str();
        let type_str = type_short_name(def);
        let card_str = cardinality_short_name(def);
        let doc = truncate_doc(&def.doc, 60);

        out.push_str(&format!(
            "  {:<width$}  {:<8}  {:<4}  \"{}\"\n",
            name,
            type_str,
            card_str,
            doc,
            width = max_name_len,
        ));
    }

    Ok(out)
}

/// Verbose output: full details per attribute including usage count.
fn format_verbose(
    store: &braid_kernel::Store,
    attrs: &[(&Attribute, &AttributeDef)],
) -> Result<String, BraidError> {
    let entity_count = store.entity_count();

    let mut out = String::new();
    out.push_str(&format!("schema: {} attributes\n\n", attrs.len()));

    for (attr, def) in attrs {
        let usage = store.attribute_datoms(attr).len();
        let res_str = resolution_short_name(def);

        out.push_str(&format!("{}\n", attr.as_str()));
        out.push_str(&format!(
            "  type: {}  cardinality: {}  resolution: {}\n",
            type_short_name(def),
            cardinality_short_name(def),
            res_str,
        ));
        if !def.doc.is_empty() {
            out.push_str(&format!("  doc: \"{}\"\n", def.doc));
        }
        out.push_str(&format!(
            "  datoms: {}  (used across {} entities)\n",
            usage, entity_count,
        ));
        if let Some(u) = &def.unique {
            out.push_str(&format!("  unique: {u:?}\n"));
        }
        if def.is_component {
            out.push_str("  component: true\n");
        }
        out.push('\n');
    }

    Ok(out)
}

/// Build structured JSON value for the attribute list.
fn build_structured_json(attrs: &[(&Attribute, &AttributeDef)]) -> serde_json::Value {
    let entries: Vec<serde_json::Value> = attrs
        .iter()
        .map(|(attr, def)| {
            let mut obj = serde_json::json!({
                "name": attr.as_str(),
                "type": type_short_name(def),
                "cardinality": cardinality_short_name(def),
                "resolution": resolution_short_name(def),
                "doc": def.doc,
            });
            if let Some(u) = &def.unique {
                obj["unique"] = serde_json::json!(format!("{u:?}"));
            }
            if def.is_component {
                obj["component"] = serde_json::json!(true);
            }
            obj
        })
        .collect();

    serde_json::json!({
        "count": entries.len(),
        "attributes": entries,
    })
}

/// Extract unique namespace prefixes for the empty-result suggestion.
fn get_namespaces(schema: &Schema) -> Vec<String> {
    let ns: BTreeSet<String> = schema
        .attributes()
        .map(|(attr, _)| {
            let s = attr.as_str();
            // Extract ":ns" from ":ns/name"
            match s.find('/') {
                Some(pos) => s[..pos].to_string(),
                None => s.to_string(),
            }
        })
        .collect();
    ns.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn type_short_name(def: &AttributeDef) -> &'static str {
    match def.value_type {
        braid_kernel::schema::ValueType::String => "String",
        braid_kernel::schema::ValueType::Keyword => "Keyword",
        braid_kernel::schema::ValueType::Boolean => "Boolean",
        braid_kernel::schema::ValueType::Long => "Long",
        braid_kernel::schema::ValueType::Double => "Double",
        braid_kernel::schema::ValueType::Instant => "Instant",
        braid_kernel::schema::ValueType::Uuid => "Uuid",
        braid_kernel::schema::ValueType::Ref => "Ref",
        braid_kernel::schema::ValueType::Bytes => "Bytes",
    }
}

fn cardinality_short_name(def: &AttributeDef) -> &'static str {
    match def.cardinality {
        braid_kernel::schema::Cardinality::One => "one",
        braid_kernel::schema::Cardinality::Many => "many",
    }
}

fn resolution_short_name(def: &AttributeDef) -> &'static str {
    match def.resolution_mode {
        braid_kernel::schema::ResolutionMode::Lww => "lww",
        braid_kernel::schema::ResolutionMode::Lattice { .. } => "lattice",
        braid_kernel::schema::ResolutionMode::Multi => "multi",
    }
}

fn truncate_doc(doc: &str, max_len: usize) -> String {
    if doc.len() <= max_len {
        doc.to_string()
    } else {
        format!("{}...", &doc[..max_len.saturating_sub(3)])
    }
}
