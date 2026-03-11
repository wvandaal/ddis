//! `braid trace` — Automated spec-to-impl traceability scanner.
//!
//! Scans Rust source files for spec references (INV-STORE-001, ADR-SEED-002, etc.)
//! in comments and creates `:impl/implements` datoms linking code entities to spec
//! entities in the store. Also marks `:spec/witnessed = true` on spec entities
//! referenced from test files.
//!
//! Traces to: INV-TRACE-001 (completeness), INV-TRACE-002 (idempotency),
//!            ADR-TRACE-001 (comment-based traceability over annotation macros)

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::layout::TxFile;
use braid_kernel::store::Store;

use crate::error::BraidError;
use crate::layout::DiskLayout;

// ---------------------------------------------------------------------------
// Spec reference pattern
// ---------------------------------------------------------------------------

/// Regex pattern for spec references: (INV|ADR|NEG)-NAMESPACE-NNN
/// Matches in comments like `// INV-STORE-001`, `/// Traces to: ADR-SEED-002`
fn extract_spec_refs(line: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut rest = line;

    // Manual matching: find (INV|ADR|NEG)-UPPERCASE-DIGITS patterns
    while let Some(pos) = find_spec_ref_start(rest) {
        let candidate = &rest[pos..];
        if let Some(ref_str) = parse_spec_ref(candidate) {
            refs.push(ref_str);
            rest = &rest[pos + 3..]; // skip past prefix to find next
        } else {
            rest = &rest[pos + 3..];
        }
    }

    refs
}

/// Find the start of a potential spec ref (INV-, ADR-, NEG-).
fn find_spec_ref_start(s: &str) -> Option<usize> {
    let prefixes = ["INV-", "ADR-", "NEG-"];
    let mut earliest = None;

    for prefix in &prefixes {
        if let Some(pos) = s.find(prefix) {
            match earliest {
                None => earliest = Some(pos),
                Some(e) if pos < e => earliest = Some(pos),
                _ => {}
            }
        }
    }

    earliest
}

/// Parse a spec ref starting at the current position.
/// Expected format: (INV|ADR|NEG)-NAMESPACE-NNN where NAMESPACE is uppercase letters
/// and NNN is 1-4 digits.
fn parse_spec_ref(s: &str) -> Option<String> {
    // Must start with INV-, ADR-, or NEG-
    let prefix = if s.starts_with("INV-") {
        "INV"
    } else if s.starts_with("ADR-") {
        "ADR"
    } else if s.starts_with("NEG-") {
        "NEG"
    } else {
        return None;
    };

    let rest = &s[4..]; // skip "XXX-"

    // Parse namespace: one or more uppercase letters
    let ns_end = rest
        .find(|c: char| !c.is_ascii_uppercase())
        .unwrap_or(rest.len());
    if ns_end == 0 {
        return None;
    }
    let namespace = &rest[..ns_end];

    // Must be followed by a hyphen
    let after_ns = &rest[ns_end..];
    if !after_ns.starts_with('-') {
        return None;
    }
    let digit_start = &after_ns[1..];

    // Parse digits: 1-4 digits
    let digit_end = digit_start
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digit_start.len());
    if digit_end == 0 || digit_end > 4 {
        return None;
    }
    let digits = &digit_start[..digit_end];

    Some(format!("{prefix}-{namespace}-{digits}"))
}

// ---------------------------------------------------------------------------
// Source file scanning
// ---------------------------------------------------------------------------

/// A traced reference found in a source file.
#[derive(Clone, Debug)]
struct TraceRef {
    /// The spec ref (e.g., "INV-STORE-001")
    spec_ref: String,
    /// Source file path (relative to source directory)
    file: String,
    /// Whether this was found in a test file
    is_test: bool,
}

/// Scan a Rust source file for spec references in comments.
///
/// Only extracts references from comment lines (`//`, `///`, `//!`).
/// Skips references inside code blocks in doc comments to avoid false positives
/// (FM-TRACE-001).
fn scan_file(path: &Path, relative: &str) -> Vec<TraceRef> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let is_test = is_test_file(relative);
    let mut refs = Vec::new();
    let mut in_code_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Track doc-comment code blocks (``` toggles)
        if (trimmed.starts_with("///") || trimmed.starts_with("//!")) && trimmed.contains("```") {
            in_code_block = !in_code_block;
            continue;
        }

        // Skip content inside code blocks (FM-TRACE-001)
        if in_code_block {
            continue;
        }

        // Only process comment lines
        if !is_comment_line(trimmed) {
            continue;
        }

        for spec_ref in extract_spec_refs(trimmed) {
            refs.push(TraceRef {
                spec_ref,
                file: relative.to_string(),
                is_test,
            });
        }
    }

    refs
}

/// Check if a line is a Rust comment.
fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("//") // covers //, ///, //!
}

/// Check if a file is a test file.
fn is_test_file(relative: &str) -> bool {
    relative.contains("/tests/")
        || relative.ends_with("_test.rs")
        || relative.ends_with("_tests.rs")
}

/// Recursively find all .rs files under a directory.
fn find_rust_files(dir: &Path) -> Vec<(std::path::PathBuf, String)> {
    let mut files = Vec::new();
    walk_dir(dir, dir, &mut files);
    files.sort_by(|a, b| a.1.cmp(&b.1));
    files
}

fn walk_dir(root: &Path, current: &Path, out: &mut Vec<(std::path::PathBuf, String)>) {
    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip target/ and hidden directories
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.') || name == "target" {
                continue;
            }
            walk_dir(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            out.push((path, relative));
        }
    }
}

// ---------------------------------------------------------------------------
// Resolution: match spec refs to store entities
// ---------------------------------------------------------------------------

/// Build a lookup map from spec ref strings to store entity IDs.
///
/// Scans the store for entities whose `:element/id` matches the spec ref,
/// or whose `:db/ident` is `:spec/{ref_lowered}`.
fn build_spec_ref_map(store: &Store) -> HashMap<String, EntityId> {
    let mut map = HashMap::new();
    let ident_attr = Attribute::from_keyword(":db/ident");
    let element_id_attr = Attribute::from_keyword(":element/id");

    // Method 1: Match via :element/id (the canonical spec ID)
    for datom in store.attribute_datoms(&element_id_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        if let Value::String(ref id) = datom.value {
            // element/id stores the full ID like "INV-STORE-001"
            map.insert(id.clone(), datom.entity);
        }
    }

    // Method 2: Match via :db/ident (e.g., ":spec/inv-store-001")
    for datom in store.attribute_datoms(&ident_attr) {
        if datom.op != Op::Assert {
            continue;
        }
        if let Value::Keyword(ref kw) = datom.value {
            if let Some(ref_part) = kw.strip_prefix(":spec/") {
                // Convert ident to uppercase spec ref for lookup
                // ":spec/inv-store-001" → "INV-STORE-001"
                let upper = ref_part.to_uppercase();
                map.entry(upper).or_insert(datom.entity);
            }
        }
    }

    map
}

// ---------------------------------------------------------------------------
// Datom generation
// ---------------------------------------------------------------------------

/// A resolved trace link ready for datom generation.
#[derive(Clone, Debug)]
struct ResolvedLink {
    /// The impl entity ident
    ident: String,
    /// The impl entity ID
    entity: EntityId,
    /// The spec entity ID being implemented
    spec_entity: EntityId,
    /// Source file
    file: String,
    /// Module path (derived from file path)
    module: String,
}

/// Generate datoms for resolved trace links.
fn generate_impl_datoms(links: &[ResolvedLink], tx_id: TxId) -> Vec<braid_kernel::datom::Datom> {
    let mut datoms = Vec::new();

    for link in links {
        // :db/ident — entity identity
        datoms.push(braid_kernel::datom::Datom::new(
            link.entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(link.ident.clone()),
            tx_id,
            Op::Assert,
        ));

        // :impl/implements — ref to spec entity
        datoms.push(braid_kernel::datom::Datom::new(
            link.entity,
            Attribute::from_keyword(":impl/implements"),
            Value::Ref(link.spec_entity),
            tx_id,
            Op::Assert,
        ));

        // :impl/file — source file path
        datoms.push(braid_kernel::datom::Datom::new(
            link.entity,
            Attribute::from_keyword(":impl/file"),
            Value::String(link.file.clone()),
            tx_id,
            Op::Assert,
        ));

        // :impl/module — module path
        datoms.push(braid_kernel::datom::Datom::new(
            link.entity,
            Attribute::from_keyword(":impl/module"),
            Value::String(link.module.clone()),
            tx_id,
            Op::Assert,
        ));
    }

    datoms
}

/// Generate witness datoms: assert :spec/witnessed = true on spec entities
/// that have test evidence.
fn generate_witness_datoms(
    witnessed_specs: &BTreeSet<EntityId>,
    tx_id: TxId,
) -> Vec<braid_kernel::datom::Datom> {
    witnessed_specs
        .iter()
        .map(|spec_entity| {
            braid_kernel::datom::Datom::new(
                *spec_entity,
                Attribute::from_keyword(":spec/witnessed"),
                Value::Boolean(true),
                tx_id,
                Op::Assert,
            )
        })
        .collect()
}

/// Derive a module path from a relative file path.
///
/// `crates/braid-kernel/src/store.rs` → `braid_kernel::store`
/// `crates/braid/src/commands/trace.rs` → `braid::commands::trace`
fn module_from_path(relative: &str) -> String {
    let path = relative
        .trim_start_matches("crates/")
        .replace('/', "::")
        .replace('-', "_");

    // Strip src:: prefix and .rs suffix
    let path = path.replace("::src::", "::");
    path.trim_end_matches(".rs")
        .trim_end_matches("::mod")
        .trim_end_matches("::lib")
        .to_string()
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

/// Run the trace scanner.
///
/// - Dry-run (default): shows what would be linked, no store mutation.
/// - Commit (`--commit`): writes traceability datoms to the store.
///
/// INV-TRACE-001: Completeness — every resolved spec ref produces a datom.
/// INV-TRACE-002: Idempotency — content-addressed entities, running twice = same count.
pub fn run(
    path: &Path,
    source: &Path,
    agent_name: &str,
    commit: bool,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Build the spec ref → entity lookup
    let spec_map = build_spec_ref_map(&store);

    // Scan all Rust source files
    let files = find_rust_files(source);
    let mut all_refs: Vec<TraceRef> = Vec::new();
    for (abs_path, relative) in &files {
        let file_refs = scan_file(abs_path, relative);
        all_refs.extend(file_refs);
    }

    // Build set of entities already in the store (for idempotency, INV-TRACE-002)
    let existing_entities: BTreeSet<EntityId> = store.entities();

    // Build set of already-witnessed spec entities
    let witnessed_attr = Attribute::from_keyword(":spec/witnessed");
    let already_witnessed: BTreeSet<EntityId> = store
        .attribute_datoms(&witnessed_attr)
        .iter()
        .filter(|d| d.op == Op::Assert && d.value == Value::Boolean(true))
        .map(|d| d.entity)
        .collect();

    // Resolve: match refs to store entities
    let mut resolved_links: Vec<ResolvedLink> = Vec::new();
    let mut witnessed_specs: BTreeSet<EntityId> = BTreeSet::new();
    let mut unresolved: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen_idents: BTreeSet<String> = BTreeSet::new();
    let mut skipped_existing = 0usize;

    for trace_ref in &all_refs {
        match spec_map.get(&trace_ref.spec_ref) {
            Some(&spec_entity) => {
                // Track witness marking for test files (A2)
                // Skip if already witnessed
                if trace_ref.is_test && !already_witnessed.contains(&spec_entity) {
                    witnessed_specs.insert(spec_entity);
                }

                // Create impl link (deduplicate by ident for idempotency)
                // Use '.' separator (not ':') to avoid confusing EDN keyword parser
                let ident = format!(
                    ":impl/{}.{}",
                    trace_ref.file.replace('/', ".").trim_end_matches(".rs"),
                    trace_ref.spec_ref.to_lowercase()
                );

                if seen_idents.insert(ident.clone()) {
                    let entity = EntityId::from_ident(&ident);
                    // INV-TRACE-002: Skip entities that already exist in the store
                    if existing_entities.contains(&entity) {
                        skipped_existing += 1;
                        continue;
                    }
                    resolved_links.push(ResolvedLink {
                        entity,
                        ident,
                        spec_entity,
                        file: trace_ref.file.clone(),
                        module: module_from_path(&trace_ref.file),
                    });
                }
            }
            None => {
                *unresolved.entry(trace_ref.spec_ref.clone()).or_insert(0) += 1;
            }
        }
    }

    // Count unique spec entities covered (both new and existing links)
    let implements_attr = Attribute::from_keyword(":impl/implements");
    let existing_impl_targets: BTreeSet<EntityId> = store
        .attribute_datoms(&implements_attr)
        .iter()
        .filter(|d| d.op == Op::Assert)
        .filter_map(|d| {
            if let Value::Ref(target) = &d.value {
                Some(*target)
            } else {
                None
            }
        })
        .collect();
    let new_targets: BTreeSet<EntityId> = resolved_links.iter().map(|l| l.spec_entity).collect();
    let covered_specs: BTreeSet<EntityId> =
        existing_impl_targets.union(&new_targets).copied().collect();

    // Format output
    let mut out = String::new();
    out.push_str(&format!(
        "Trace scan: {} files, {} refs found, {} new, {} existing, {} unresolved\n",
        files.len(),
        all_refs.len(),
        resolved_links.len(),
        skipped_existing,
        unresolved.len(),
    ));
    out.push_str(&format!(
        "Spec coverage: {}/{} spec entities have :impl/implements links\n",
        covered_specs.len(),
        spec_map.len(),
    ));

    if !witnessed_specs.is_empty() {
        out.push_str(&format!(
            "Witnessed: {}/{} spec entities have test evidence\n",
            witnessed_specs.len(),
            spec_map.len(),
        ));
    }

    // Show unresolved refs (FM-TRACE-002 warnings)
    if !unresolved.is_empty() {
        out.push_str(&format!("\nUnresolved refs ({}):\n", unresolved.len()));
        for (ref_str, count) in &unresolved {
            out.push_str(&format!("  {} ({}x)\n", ref_str, count));
        }
    }

    // Commit if requested
    if commit {
        let agent = AgentId::from_name(agent_name);
        let tx_id = super::write::next_tx_id(&store, agent);

        let mut datoms = generate_impl_datoms(&resolved_links, tx_id);
        let witness_datoms = generate_witness_datoms(&witnessed_specs, tx_id);
        let witness_count = witness_datoms.len();
        datoms.extend(witness_datoms);

        let datom_count = datoms.len();
        if datom_count > 0 {
            let tx = TxFile {
                tx_id,
                agent,
                provenance: ProvenanceType::Derived,
                rationale: format!(
                    "braid trace: {} impl links, {} witness marks",
                    resolved_links.len(),
                    witness_count,
                ),
                causal_predecessors: vec![],
                datoms,
            };

            let file_path = layout.write_tx(&tx)?;
            out.push_str(&format!(
                "\nCommitted: {} datoms ({} impl + {} witness) \u{2192} {}\n",
                datom_count,
                datom_count - witness_count,
                witness_count,
                file_path.relative_path(),
            ));
        } else {
            out.push_str("\nNothing to commit (no resolved refs).\n");
        }
    } else {
        out.push_str("\nDry run. Use --commit to write traceability datoms.\n");
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_spec_refs_basic() {
        let refs = extract_spec_refs("// INV-STORE-001: Append-only immutability");
        assert_eq!(refs, vec!["INV-STORE-001"]);
    }

    #[test]
    fn extract_spec_refs_multiple() {
        let refs = extract_spec_refs("/// Traces to: INV-STORE-001, ADR-SEED-002");
        assert_eq!(refs, vec!["INV-STORE-001", "ADR-SEED-002"]);
    }

    #[test]
    fn extract_spec_refs_neg() {
        let refs = extract_spec_refs("// NEG-MUTATION-001: No in-place mutation");
        assert_eq!(refs, vec!["NEG-MUTATION-001"]);
    }

    #[test]
    fn extract_spec_refs_no_match() {
        let refs = extract_spec_refs("// This is a normal comment");
        assert!(refs.is_empty());
    }

    #[test]
    fn extract_spec_refs_in_prose() {
        // Should match spec refs even in prose
        let refs = extract_spec_refs("// See INV-QUERY-024 and ADR-BILATERAL-003 for details");
        assert_eq!(refs, vec!["INV-QUERY-024", "ADR-BILATERAL-003"]);
    }

    #[test]
    fn extract_spec_refs_no_false_positive_from_non_comment() {
        // The extract function only gets comment content — but it shouldn't
        // match things that look like refs but aren't well-formed
        let refs = extract_spec_refs("INV-lowercase-001");
        assert!(refs.is_empty());
    }

    #[test]
    fn parse_spec_ref_valid() {
        assert_eq!(
            parse_spec_ref("INV-STORE-001"),
            Some("INV-STORE-001".to_string())
        );
        assert_eq!(
            parse_spec_ref("ADR-SEED-002"),
            Some("ADR-SEED-002".to_string())
        );
        assert_eq!(
            parse_spec_ref("NEG-MUTATION-001"),
            Some("NEG-MUTATION-001".to_string())
        );
    }

    #[test]
    fn parse_spec_ref_invalid() {
        assert_eq!(parse_spec_ref("FOO-BAR-001"), None);
        assert_eq!(parse_spec_ref("INV-lower-001"), None);
        assert_eq!(parse_spec_ref("INV-STORE"), None);
        assert_eq!(parse_spec_ref("INV-STORE-"), None);
    }

    #[test]
    fn module_from_path_kernel() {
        assert_eq!(
            module_from_path("crates/braid-kernel/src/store.rs"),
            "braid_kernel::store"
        );
    }

    #[test]
    fn module_from_path_cli() {
        assert_eq!(
            module_from_path("crates/braid/src/commands/trace.rs"),
            "braid::commands::trace"
        );
    }

    #[test]
    fn module_from_path_mod() {
        assert_eq!(
            module_from_path("crates/braid/src/commands/mod.rs"),
            "braid::commands"
        );
    }

    #[test]
    fn is_test_file_positive() {
        assert!(is_test_file("crates/braid-kernel/tests/store_test.rs"));
        assert!(is_test_file("crates/braid/src/tests/integration.rs"));
        assert!(is_test_file("tests/bilateral_tests.rs"));
    }

    #[test]
    fn is_test_file_negative() {
        assert!(!is_test_file("crates/braid/src/commands/trace.rs"));
        assert!(!is_test_file("crates/braid-kernel/src/store.rs"));
    }

    #[test]
    fn is_comment_line_true() {
        assert!(is_comment_line("// INV-STORE-001"));
        assert!(is_comment_line("/// Doc comment"));
        assert!(is_comment_line("//! Module doc"));
    }

    #[test]
    fn is_comment_line_false() {
        assert!(!is_comment_line("let x = 1;"));
        assert!(!is_comment_line("fn main() {"));
    }
}
