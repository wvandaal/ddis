//! `braid observe` — One-liner knowledge capture as exploration datoms.
//!
//! Creates an exploration entity in the store from a single text observation.
//! Uses existing Layer 3 `:exploration/*` schema attributes (schema.rs lines 1158-1291).
//!
//! # Design Rationale
//!
//! This replaces manual EDN transaction construction with a single command:
//!   braid observe "merge is a bottleneck" --confidence 0.8 --tag bottleneck
//!
//! The observation becomes an exploration entity with:
//! - `:exploration/body` — the observation text
//! - `:exploration/confidence` — epistemic confidence (0.0-1.0)
//! - `:exploration/category` — auto-classified or explicit
//! - `:exploration/tags` — taxonomy tags for filtering
//! - `:exploration/source` — "braid:observe" (provenance)
//! - `:db/ident` — content-addressed identity from body text
//! - `:db/doc` — same as body (for discoverability via standard queries)
//!
//! Entity ID is `EntityId::from_ident(":observation/{slug}")` where slug is
//! derived from the body text, ensuring content-addressable identity (C2).

use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, Value};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::layout::DiskLayout;

/// Arguments for the observe command.
pub struct ObserveArgs<'a> {
    pub path: &'a Path,
    pub text: &'a str,
    pub confidence: f64,
    pub tags: &'a [String],
    pub category: Option<&'a str>,
    pub agent: &'a str,
    pub relates_to: Option<&'a str>,
}

/// Generate a slug from observation text for the entity ident.
///
/// Takes the first ~60 chars of the text, lowercased, with spaces → hyphens,
/// non-alphanumeric stripped. Produces a deterministic, content-derived identifier.
fn slug_from_text(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .take(60)
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c == ' ' || c == '_' || c == '-' {
                '-'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect();

    // Trim trailing hyphens and collapse consecutive hyphens
    let mut result = String::new();
    let mut prev_hyphen = false;
    for c in cleaned.chars() {
        if c == '-' {
            if !prev_hyphen && !result.is_empty() {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    result.trim_end_matches('-').to_string()
}

/// Resolve a category string to a valid `:exploration.cat/*` keyword.
///
/// Supported categories: observation (default), conjecture, definition,
/// algorithm, design-decision, open-question, theorem.
fn resolve_category(cat: Option<&str>) -> String {
    match cat {
        Some("theorem") => ":exploration.cat/theorem".to_string(),
        Some("conjecture") => ":exploration.cat/conjecture".to_string(),
        Some("definition") => ":exploration.cat/definition".to_string(),
        Some("algorithm") => ":exploration.cat/algorithm".to_string(),
        Some("design-decision") | Some("decision") => {
            ":exploration.cat/design-decision".to_string()
        }
        Some("open-question") | Some("question") => ":exploration.cat/open-question".to_string(),
        Some("observation") | None => ":exploration.cat/observation".to_string(),
        Some(other) => format!(":exploration.cat/{other}"),
    }
}

pub fn run(args: ObserveArgs<'_>) -> Result<String, BraidError> {
    // Validate inputs
    if args.text.trim().is_empty() {
        return Err(BraidError::Validation(
            "observation text cannot be empty".to_string(),
        ));
    }
    if !(0.0..=1.0).contains(&args.confidence) {
        return Err(BraidError::Validation(format!(
            "confidence must be 0.0-1.0, got {}",
            args.confidence
        )));
    }

    let layout = DiskLayout::open(args.path)?;
    let store = layout.load_store()?;

    let agent = AgentId::from_name(args.agent);
    let slug = slug_from_text(args.text);
    let ident = format!(":observation/{slug}");
    let entity = EntityId::from_ident(&ident);
    let category = resolve_category(args.category);

    // Generate TxId: advance past the store's current frontier (Unix epoch seconds)
    let tx_id = super::write::next_tx_id(&store, agent);

    // Compute BLAKE3 content hash for cross-session dedup (INV-HARVEST-006)
    let content_hash = blake3::hash(args.text.as_bytes());

    // Build datom vector — 8 core assertions + tags + optional cross-ref
    let mut datoms = vec![
        // Core identity
        Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(ident.clone()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(args.text.to_string()),
            tx_id,
            Op::Assert,
        ),
        // Exploration attributes
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/body"),
            Value::String(args.text.to_string()),
            tx_id,
            Op::Assert,
        ),
        // Content hash for crystallization guard (INV-HARVEST-006)
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/content-hash"),
            Value::Bytes(content_hash.as_bytes().to_vec()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/confidence"),
            Value::Double(ordered_float::OrderedFloat(args.confidence)),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/category"),
            Value::Keyword(category),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/source"),
            Value::String("braid:observe".to_string()),
            tx_id,
            Op::Assert,
        ),
        Datom::new(
            entity,
            Attribute::from_keyword(":exploration/maturity"),
            Value::Keyword(":exploration.maturity/sketch".to_string()),
            tx_id,
            Op::Assert,
        ),
    ];

    // Add tags
    for tag in args.tags {
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":exploration/tags"),
            Value::Keyword(format!(":tag/{tag}")),
            tx_id,
            Op::Assert,
        ));
    }

    // Add cross-reference if provided
    if let Some(relates_to) = args.relates_to {
        let target = EntityId::from_ident(relates_to);
        datoms.push(Datom::new(
            entity,
            Attribute::from_keyword(":exploration/related-spec"),
            Value::Ref(target),
            tx_id,
            Op::Assert,
        ));
    }

    let tx = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Observed,
        rationale: args.text.to_string(),
        causal_predecessors: vec![],
        datoms,
    };

    let datom_count = tx.datoms.len();
    let file_path = layout.write_tx(&tx)?;

    // Count new store size (current + new datoms)
    let new_total = store.datoms().count() + datom_count;

    let mut out = String::new();
    out.push_str(&format!("observed: {ident}\n"));
    out.push_str(&format!(
        "  confidence: {:.1} | category: {} | datoms: {}\n",
        args.confidence,
        resolve_category(args.category)
            .strip_prefix(":exploration.cat/")
            .unwrap_or("observation"),
        datom_count
    ));
    if !args.tags.is_empty() {
        out.push_str(&format!("  tags: {}\n", args.tags.join(", ")));
    }
    if let Some(relates_to) = args.relates_to {
        out.push_str(&format!("  relates-to: {relates_to}\n"));
    }
    out.push_str(&format!("  store: {new_total} datoms (+{datom_count})\n"));
    out.push_str(&format!("  tx: {}\n", file_path.relative_path()));

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_generation() {
        assert_eq!(
            slug_from_text("merge is a bottleneck"),
            "merge-is-a-bottleneck"
        );
        assert_eq!(slug_from_text("Hello, World!"), "hello-world");
        assert_eq!(
            slug_from_text("  spaces   and    tabs  "),
            "spaces-and-tabs"
        );
        assert_eq!(slug_from_text(""), "");
        assert_eq!(
            slug_from_text("Datalog joins return 0 results — CRITICAL"),
            "datalog-joins-return-0-results-critical"
        );
    }

    #[test]
    fn category_resolution() {
        assert_eq!(resolve_category(None), ":exploration.cat/observation");
        assert_eq!(
            resolve_category(Some("conjecture")),
            ":exploration.cat/conjecture"
        );
        assert_eq!(
            resolve_category(Some("decision")),
            ":exploration.cat/design-decision"
        );
        assert_eq!(
            resolve_category(Some("question")),
            ":exploration.cat/open-question"
        );
        assert_eq!(resolve_category(Some("custom")), ":exploration.cat/custom");
    }

    #[test]
    fn observe_creates_exploration_entity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");

        // Initialize store
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        // Observe
        let result = run(ObserveArgs {
            path: &path,
            text: "merge is a structural bottleneck",
            confidence: 0.8,
            tags: &["bottleneck".to_string(), "graph".to_string()],
            category: None,
            agent: "test",
            relates_to: None,
        })
        .unwrap();

        assert!(result.contains("observed: :observation/merge-is-a-structural-bottleneck"));
        assert!(result.contains("confidence: 0.8"));
        assert!(result.contains("tags: bottleneck, graph"));

        // Verify entity exists in store
        let layout = DiskLayout::open(&path).unwrap();
        let store = layout.load_store().unwrap();
        let entity = EntityId::from_ident(":observation/merge-is-a-structural-bottleneck");
        let datoms = store.entity_datoms(entity);
        assert!(
            datoms.len() >= 8,
            "expected at least 8 datoms for observation entity (incl. content-hash), got {}",
            datoms.len()
        );

        // Verify specific attributes
        let body = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":exploration/body")
            .expect("should have :exploration/body");
        assert!(matches!(&body.value, Value::String(s) if s == "merge is a structural bottleneck"));

        let confidence = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":exploration/confidence")
            .expect("should have :exploration/confidence");
        assert!(matches!(&confidence.value, Value::Double(f) if f.into_inner() == 0.8));
    }

    #[test]
    fn observe_validates_confidence_range() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        let result = run(ObserveArgs {
            path: &path,
            text: "test",
            confidence: 1.5,
            tags: &[],
            category: None,
            agent: "test",
            relates_to: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn observe_validates_empty_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        let result = run(ObserveArgs {
            path: &path,
            text: "  ",
            confidence: 0.7,
            tags: &[],
            category: None,
            agent: "test",
            relates_to: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn observe_with_relates_to() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        let result = run(ObserveArgs {
            path: &path,
            text: "CRDT merge is commutative",
            confidence: 0.95,
            tags: &[],
            category: Some("theorem"),
            agent: "test",
            relates_to: Some(":spec/inv-store-004"),
        })
        .unwrap();

        assert!(result.contains("relates-to: :spec/inv-store-004"));
        assert!(result.contains("category: theorem"));
    }

    #[test]
    fn observe_content_addressable_identity() {
        // Same text → same entity ID (C2 constraint)
        let slug1 = slug_from_text("merge is a bottleneck");
        let slug2 = slug_from_text("merge is a bottleneck");
        assert_eq!(slug1, slug2);

        let eid1 = EntityId::from_ident(&format!(":observation/{slug1}"));
        let eid2 = EntityId::from_ident(&format!(":observation/{slug2}"));
        assert_eq!(
            eid1, eid2,
            "same observation text must produce same entity ID"
        );
    }

    #[test]
    fn observe_queryable_via_datalog() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".braid");
        crate::commands::init::run(&path, Path::new("spec")).unwrap();

        // Create observation
        run(ObserveArgs {
            path: &path,
            text: "the store is append-only",
            confidence: 0.99,
            tags: &[],
            category: None,
            agent: "test",
            relates_to: None,
        })
        .unwrap();

        // Query it back via Datalog
        let layout = DiskLayout::open(&path).unwrap();
        let store = layout.load_store().unwrap();

        let query = braid_kernel::QueryExpr::new(
            braid_kernel::FindSpec::Rel(vec!["?e".into(), "?body".into()]),
            vec![braid_kernel::Clause::Pattern(braid_kernel::Pattern::new(
                braid_kernel::query::clause::Term::Variable("?e".into()),
                braid_kernel::query::clause::Term::Attr(Attribute::from_keyword(
                    ":exploration/body",
                )),
                braid_kernel::query::clause::Term::Variable("?body".into()),
            ))],
        );

        let result = braid_kernel::evaluate(&store, &query);
        match result {
            braid_kernel::query::evaluator::QueryResult::Rel(rows) => {
                assert!(
                    !rows.is_empty(),
                    "observation should be queryable via Datalog"
                );
                let found = rows.iter().any(
                    |row| matches!(&row[1], Value::String(s) if s == "the store is append-only"),
                );
                assert!(found, "observation body should appear in query results");
            }
            _ => panic!("expected Rel result"),
        }
    }
}
