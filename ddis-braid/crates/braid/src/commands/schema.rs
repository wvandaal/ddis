//! `braid schema` — Inspect the datom store schema.
//!
//! Lists all known attributes with their type, cardinality, and documentation.
//! Optimized for AI agent consumption: provides the information needed to
//! write correct queries and transactions (INV-INTERFACE-011).

use std::collections::BTreeSet;
use std::path::Path;

use braid_kernel::schema::{AttributeDef, Schema};
use braid_kernel::Attribute;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

/// Run the schema introspection command.
pub fn run(
    path: &Path,
    pattern: Option<&str>,
    verbose: bool,
    json: bool,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
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

    // Build agent output.
    let context = if let Some(pat) = pattern {
        format!("schema: {} attributes (pattern: {})", attrs.len(), pat)
    } else {
        format!("schema: {} attributes", attrs.len())
    };

    let agent = AgentOutput {
        context,
        content: human.clone(),
        footer: "explore: braid query '[:find ?e :where [?e :db/doc ?v]]' | filter: braid schema --pattern ':spec/*'".to_string(),
    };

    Ok(CommandOutput {
        json: structured_json,
        agent,
        human,
    })
}

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
