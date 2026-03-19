//! Trace link scanner — extract spec references from test source code.
//!
//! The trace scanner reads Rust test source files (as strings) and extracts
//! spec element references (INV-STORE-001, ADR-QUERY-005, etc.) from test
//! function names, doc comments, and annotations. Each reference is classified
//! by verification depth:
//!
//! - **L1 (Syntactic, 0.15)**: Comment reference only (`// Verifies: INV-STORE-001`)
//! - **L2 (Structural, 0.40)**: Unit test that names the spec element
//! - **L3 (Property, 0.70)**: Proptest or Kani harness that names the spec element
//! - **L4 (Formal, 1.00)**: Stateright model that names the spec element
//!
//! # Traces To
//!
//! - INV-BILATERAL-002 (CC — depth-weighted coverage)
//! - ADR-BILATERAL-001 (Fitness Function Weights)
//! - docs/guide/10-verification.md §10.4
//!
//! # Design
//!
//! The scanner is pure computation — it takes source strings, not file paths.
//! The CLI layer handles filesystem access and passes content to the kernel.

use std::collections::{BTreeMap, BTreeSet};

use crate::datom::{Attribute, Datom, EntityId, Op, TxId, Value};

/// A trace link between a test function and a spec element.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TraceLink {
    /// The spec element ID (e.g., "INV-STORE-001").
    pub spec_id: String,
    /// The source file where the reference was found.
    pub source_file: String,
    /// The test function name (if inside a test).
    pub test_fn: Option<String>,
    /// Verification depth level (1-4).
    pub depth: VerificationDepth,
}

/// Verification depth classification.
///
/// Higher depth = stronger verification = more F(S) credit.
/// See `bilateral::depth_weight()` for the weight mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VerificationDepth {
    /// L1: Comment reference only. Weight = 0.15.
    Syntactic = 1,
    /// L2: Unit test that exercises the property. Weight = 0.40.
    Structural = 2,
    /// L3: Property-based test (proptest) or bounded model check (Kani). Weight = 0.70.
    Property = 3,
    /// L4: Behavioral model check (Stateright). Weight = 1.00.
    Formal = 4,
}

impl VerificationDepth {
    /// Convert to integer for datom storage.
    pub fn as_i64(self) -> i64 {
        self as i64
    }
}

/// Context within a source file — what kind of code block we're in.
#[derive(Clone, Debug, PartialEq)]
enum ScanContext {
    /// Top-level code or regular function.
    TopLevel,
    /// Inside a `#[test]` function.
    UnitTest(String),
    /// Inside a `proptest!` macro block.
    PropTest(String),
    /// Inside a `#[kani::proof]` function.
    KaniProof(String),
    /// Inside a stateright model test file.
    Stateright(String),
}

/// Scan a Rust source file for spec element references.
///
/// Extracts all references to spec elements (INV-*, ADR-*, NEG-*) and
/// classifies them by the context they appear in (test type → depth level).
///
/// # Arguments
///
/// * `source` — The Rust source code as a string.
/// * `file_path` — The relative file path (for the TraceLink source_file field).
///
/// # Returns
///
/// A set of TraceLinks, deduplicated by (spec_id, test_fn, depth).
pub fn scan_source(source: &str, file_path: &str) -> BTreeSet<TraceLink> {
    let mut links = BTreeSet::new();
    let is_stateright = file_path.contains("stateright");

    // Regex-like pattern matching for spec IDs
    // Matches: INV-STORE-001, ADR-QUERY-005, NEG-MERGE-001, etc.
    let lines: Vec<&str> = source.lines().collect();

    let mut current_context = ScanContext::TopLevel;
    let mut in_proptest_block = false;
    let mut brace_depth: i32 = 0;
    let mut fn_brace_start: i32 = 0;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Track brace depth for function boundaries
        for ch in line.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    // If we return to the brace depth where the function started,
                    // the function has ended
                    if brace_depth <= fn_brace_start
                        && !matches!(current_context, ScanContext::TopLevel)
                    {
                        current_context = ScanContext::TopLevel;
                    }
                }
                _ => {}
            }
        }

        // Detect proptest! macro block
        if trimmed.starts_with("proptest!") || trimmed == "proptest! {" {
            in_proptest_block = true;
        }

        // Detect test function boundaries
        if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
            if let Some(fn_name) = extract_fn_name(trimmed) {
                // Check preceding lines for attributes
                let has_test_attr = check_preceding_attr(&lines, i, "#[test]");
                let has_kani_attr = check_preceding_attr(&lines, i, "#[kani::proof]");

                fn_brace_start = brace_depth;

                if has_kani_attr {
                    current_context = ScanContext::KaniProof(fn_name);
                } else if is_stateright && has_test_attr {
                    current_context = ScanContext::Stateright(fn_name);
                } else if in_proptest_block {
                    current_context = ScanContext::PropTest(fn_name);
                } else if has_test_attr {
                    current_context = ScanContext::UnitTest(fn_name);
                }
            }
        }

        // Extract spec element IDs from this line
        let line_spec_ids = extract_spec_ids(trimmed);
        for spec_id in &line_spec_ids {
            let (depth, test_fn) = match &current_context {
                ScanContext::TopLevel => (VerificationDepth::Syntactic, None),
                ScanContext::UnitTest(name) => (VerificationDepth::Structural, Some(name.clone())),
                ScanContext::PropTest(name) => (VerificationDepth::Property, Some(name.clone())),
                ScanContext::KaniProof(name) => (VerificationDepth::Property, Some(name.clone())),
                ScanContext::Stateright(name) => (VerificationDepth::Formal, Some(name.clone())),
            };

            links.insert(TraceLink {
                spec_id: spec_id.clone(),
                source_file: file_path.to_string(),
                test_fn,
                depth,
            });
        }

        // When a test function starts, retroactively claim spec IDs from
        // the preceding comment block (within 5 lines). This handles the
        // common pattern: `// Verifies: INV-XXX-NNN` above `#[test] fn`.
        if !matches!(current_context, ScanContext::TopLevel) && line_spec_ids.is_empty() {
            // Check if we JUST entered a test context (first line of function body)
            if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
                let (depth, test_fn) = match &current_context {
                    ScanContext::TopLevel => unreachable!(),
                    ScanContext::UnitTest(n) => (VerificationDepth::Structural, n.clone()),
                    ScanContext::PropTest(n) => (VerificationDepth::Property, n.clone()),
                    ScanContext::KaniProof(n) => (VerificationDepth::Property, n.clone()),
                    ScanContext::Stateright(n) => (VerificationDepth::Formal, n.clone()),
                };
                // Scan preceding 5 lines for spec IDs to upgrade
                let start = i.saturating_sub(5);
                for prev_line in &lines[start..i] {
                    for prev_id in extract_spec_ids(prev_line.trim()) {
                        links.insert(TraceLink {
                            spec_id: prev_id,
                            source_file: file_path.to_string(),
                            test_fn: Some(test_fn.clone()),
                            depth,
                        });
                    }
                }
            }
        }
    }

    links
}

/// Extract a function name from a line like `fn foo_bar(` or `pub fn baz(`.
fn extract_fn_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let after_fn = trimmed
        .strip_prefix("pub fn ")
        .or_else(|| trimmed.strip_prefix("fn "))?;

    // Take characters until ( or < or whitespace
    let name: String = after_fn
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Check if any of the preceding 3 lines contains the given attribute.
fn check_preceding_attr(lines: &[&str], current: usize, attr: &str) -> bool {
    let start = current.saturating_sub(3);
    for line in &lines[start..current] {
        if line.trim().contains(attr) {
            return true;
        }
    }
    false
}

/// Extract all spec element IDs from a line of source code.
///
/// Matches patterns like: INV-STORE-001, ADR-QUERY-005, NEG-MERGE-001,
/// INV-BILATERAL-001, INV-TRANSACT-COHERENCE-001, etc.
///
/// Does NOT extract IDs that look like they're in a string literal used
/// as a variable name or ident (e.g., `:spec/inv-store-001`).
fn extract_spec_ids(line: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let prefixes = ["INV-", "ADR-", "NEG-"];

    for prefix in &prefixes {
        let mut search_from = 0;
        while let Some(pos) = line[search_from..].find(prefix) {
            let abs_pos = search_from + pos;
            // Extract the full ID: PREFIX + NAMESPACE + - + DIGITS
            let id_start = abs_pos;
            let remaining = &line[id_start..];
            let id: String = remaining
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-')
                .collect();

            // Validate: must have at least PREFIX-NAMESPACE-NNN pattern
            let parts: Vec<&str> = id.split('-').collect();
            if parts.len() >= 3 {
                // Last part must be numeric
                if let Some(last) = parts.last() {
                    if last.chars().all(|c| c.is_ascii_digit()) && !last.is_empty() {
                        ids.push(id.clone());
                    }
                }
            }

            search_from = abs_pos + id.len().max(1);
        }
    }

    ids
}

/// Convert trace links to datoms for the store.
///
/// For each unique (spec_id, depth) pair, creates:
/// - An impl entity with `:impl/implements` → spec entity ref
/// - The impl entity with `:impl/verification-depth` → depth level
/// - The impl entity with `:impl/file` → source file path
/// - The impl entity with `:impl/module` → module name derived from file
///
/// The impl entity ID is content-addressed from (spec_id, source_file, test_fn).
pub fn links_to_datoms(links: &BTreeSet<TraceLink>, tx_id: TxId) -> Vec<Datom> {
    let mut datoms = Vec::new();

    // Deduplicate: for each spec_id, take the HIGHEST depth link
    let mut best_depth: BTreeMap<String, (VerificationDepth, &TraceLink)> = BTreeMap::new();
    for link in links {
        let entry = best_depth
            .entry(link.spec_id.clone())
            .or_insert((link.depth, link));
        if link.depth > entry.0 {
            *entry = (link.depth, link);
        }
    }

    for (spec_id, (depth, link)) in &best_depth {
        // Create impl entity from content
        let impl_ident = format!(
            ":impl/trace.{}.{}",
            link.source_file.replace('/', ".").replace(".rs", ""),
            link.test_fn.as_deref().unwrap_or("comment")
        );
        let impl_entity = EntityId::from_ident(&impl_ident);

        // Spec entity reference
        let spec_ident = format!(":spec/{}", spec_id.to_lowercase());
        let spec_entity = EntityId::from_ident(&spec_ident);

        // Ident
        datoms.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(impl_ident),
            tx_id,
            Op::Assert,
        ));

        // Implements link
        datoms.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":impl/implements"),
            Value::Ref(spec_entity),
            tx_id,
            Op::Assert,
        ));

        // Verification depth
        datoms.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":impl/verification-depth"),
            Value::Long(depth.as_i64()),
            tx_id,
            Op::Assert,
        ));

        // Source file
        datoms.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":impl/file"),
            Value::String(link.source_file.clone()),
            tx_id,
            Op::Assert,
        ));

        // Module (derived from file path)
        let module = link
            .source_file
            .rsplit('/')
            .next()
            .unwrap_or(&link.source_file)
            .trim_end_matches(".rs");
        datoms.push(Datom::new(
            impl_entity,
            Attribute::from_keyword(":impl/module"),
            Value::String(module.to_string()),
            tx_id,
            Op::Assert,
        ));
    }

    datoms
}

/// Extract test bodies and compute BLAKE3 hashes for witness system.
///
/// For each `#[test]` function in the source, extract the function body
/// (everything between the opening `{` and closing `}`) and compute
/// `BLAKE3(normalize(body))` where normalize strips whitespace and blank lines.
///
/// Depth classification:
/// - L2 (2): plain `#[test]` function
/// - L3 (3): `#[test]` inside `proptest!` block, or `#[kani::proof]`
/// - L4 (4): `#[test]` inside a stateright file (not detected here — caller sets)
///
/// Returns `Vec<(test_name, blake3_hash_hex, depth_level)>`.
pub fn extract_test_hashes(source: &str) -> Vec<(String, String, u32)> {
    let lines: Vec<&str> = source.lines().collect();
    let mut results = Vec::new();
    let mut in_proptest_block = false;

    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Track proptest! block entry
        if trimmed.starts_with("proptest!") || trimmed == "proptest! {" {
            in_proptest_block = true;
        }

        // Look for #[test] or #[kani::proof] attributes
        let is_test = trimmed == "#[test]";
        let is_kani = trimmed == "#[kani::proof]";

        if is_test || is_kani {
            // Scan forward for the `fn` line (may have other attrs in between)
            let attr_line = i;
            let mut fn_line = None;
            for (j, ln) in lines
                .iter()
                .enumerate()
                .take(lines.len().min(attr_line + 6))
                .skip(attr_line + 1)
            {
                let candidate = ln.trim();
                if candidate.starts_with("fn ") || candidate.starts_with("pub fn ") {
                    fn_line = Some(j);
                    break;
                }
                // Skip other attributes and cfg lines
                if !candidate.starts_with('#')
                    && !candidate.is_empty()
                    && !candidate.starts_with("//")
                {
                    break;
                }
            }

            if let Some(fn_idx) = fn_line {
                if let Some(fn_name) = extract_fn_name(lines[fn_idx].trim()) {
                    // Find the opening brace
                    let mut body_start = None;
                    let mut brace_search = fn_idx;
                    while brace_search < lines.len() {
                        if lines[brace_search].contains('{') {
                            body_start = Some(brace_search);
                            break;
                        }
                        brace_search += 1;
                    }

                    if let Some(start) = body_start {
                        // Brace-match to find the end
                        let mut depth: i32 = 0;
                        let mut body_lines = Vec::new();
                        let mut found_end = false;

                        for (k, ln) in lines.iter().enumerate().skip(start) {
                            for ch in ln.chars() {
                                match ch {
                                    '{' => depth += 1,
                                    '}' => {
                                        depth -= 1;
                                        if depth == 0 {
                                            found_end = true;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            // Collect lines inside the body (after opening brace line,
                            // before closing brace line)
                            if k > start && !found_end {
                                body_lines.push(*ln);
                            } else if k == start {
                                // First line may have content after `{`
                                if let Some(after) = ln.split_once('{') {
                                    let rest = after.1.trim();
                                    if !rest.is_empty() && rest != "}" {
                                        body_lines.push(rest);
                                    }
                                }
                            }
                            if found_end {
                                // Include content on the closing brace line before `}`
                                if k > start {
                                    if let Some(before) = ln.rsplit_once('}') {
                                        let pre = before.0.trim();
                                        if !pre.is_empty() {
                                            body_lines.push(pre);
                                        }
                                    }
                                }
                                i = k;
                                break;
                            }
                        }

                        // Normalize: strip whitespace per line, remove blank and comment-only lines
                        let normalized: String = body_lines
                            .iter()
                            .map(|l| l.trim())
                            .filter(|l| !l.is_empty())
                            .filter(|l| !l.starts_with("//"))
                            .collect::<Vec<&str>>()
                            .join("\n");

                        // Hash
                        let hash = blake3::hash(normalized.as_bytes());
                        let hash_hex = hash.to_hex().to_string();

                        // Depth: kani = L3, proptest = L3, plain test = L2
                        let depth_level = if is_kani || in_proptest_block { 3 } else { 2 };

                        results.push((fn_name, hash_hex, depth_level));
                    }
                }
            }
        }

        i += 1;
    }

    results
}

/// Summary of a trace scan across multiple source files.
#[derive(Clone, Debug)]
pub struct TraceSummary {
    /// Total trace links found.
    pub total_links: usize,
    /// Links by depth level.
    pub by_depth: BTreeMap<VerificationDepth, usize>,
    /// Unique spec elements referenced.
    pub unique_specs: usize,
    /// Files scanned.
    pub files_scanned: usize,
}

/// Summarize trace links from multiple scans.
pub fn summarize(all_links: &BTreeSet<TraceLink>) -> TraceSummary {
    let mut by_depth: BTreeMap<VerificationDepth, usize> = BTreeMap::new();
    let unique_specs: BTreeSet<&str> = all_links.iter().map(|l| l.spec_id.as_str()).collect();
    let files: BTreeSet<&str> = all_links.iter().map(|l| l.source_file.as_str()).collect();

    for link in all_links {
        *by_depth.entry(link.depth).or_insert(0) += 1;
    }

    TraceSummary {
        total_links: all_links.len(),
        by_depth,
        unique_specs: unique_specs.len(),
        files_scanned: files.len(),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_inv_from_comment() {
        let ids = extract_spec_ids("// Verifies: INV-STORE-001 — Append-Only Immutability");
        assert_eq!(ids, vec!["INV-STORE-001"]);
    }

    #[test]
    fn extract_multiple_ids() {
        let ids = extract_spec_ids("// Verifies: INV-STORE-001, INV-STORE-002, ADR-QUERY-005");
        assert_eq!(ids, vec!["INV-STORE-001", "INV-STORE-002", "ADR-QUERY-005"]);
    }

    #[test]
    fn extract_neg_case() {
        let ids = extract_spec_ids("NEG-MERGE-001: No merge data loss");
        assert_eq!(ids, vec!["NEG-MERGE-001"]);
    }

    #[test]
    fn no_false_positives_on_idents() {
        // :spec/inv-store-001 is a keyword ident, not a spec reference
        let ids = extract_spec_ids("let ident = \":spec/inv-store-001\";");
        // Should NOT match — lowercase
        assert!(ids.is_empty(), "should not match lowercase idents");
    }

    #[test]
    fn extract_fn_name_works() {
        assert_eq!(
            extract_fn_name("fn test_append_only("),
            Some("test_append_only".into())
        );
        assert_eq!(
            extract_fn_name("pub fn inv_store_001_append_only("),
            Some("inv_store_001_append_only".into())
        );
        assert_eq!(extract_fn_name("fn foo<T>("), Some("foo".into()));
        assert_eq!(extract_fn_name("let x = 5;"), None);
    }

    #[test]
    fn scan_classifies_unit_test() {
        let source = r#"
#[test]
fn test_store_001_append_only() {
    // Verifies: INV-STORE-001
    let store = Store::genesis();
}
"#;
        let links = scan_source(source, "src/store.rs");
        assert_eq!(links.len(), 1);
        let link = links.iter().next().unwrap();
        assert_eq!(link.spec_id, "INV-STORE-001");
        assert_eq!(link.depth, VerificationDepth::Structural);
        assert_eq!(link.test_fn, Some("test_store_001_append_only".into()));
    }

    #[test]
    fn scan_classifies_proptest() {
        let source = r#"
proptest! {
    #[test]
    fn inv_store_001_append_only(store in arb_store(3)) {
        // INV-STORE-001: append-only
        prop_assert!(store.len() >= 0);
    }
}
"#;
        let links = scan_source(source, "src/store.rs");
        let link = links.iter().find(|l| l.spec_id == "INV-STORE-001").unwrap();
        assert_eq!(link.depth, VerificationDepth::Property);
    }

    #[test]
    fn scan_classifies_kani() {
        let source = r#"
#[cfg(kani)]
#[kani::proof]
#[kani::unwind(3)]
fn prove_append_only() {
    // INV-STORE-001
    kani::assert!(true);
}
"#;
        let links = scan_source(source, "src/kani_proofs.rs");
        let link = links.iter().find(|l| l.spec_id == "INV-STORE-001").unwrap();
        assert_eq!(link.depth, VerificationDepth::Property);
    }

    #[test]
    fn scan_classifies_stateright() {
        let source = r#"
// Verifies: INV-RESOLUTION-002, INV-RESOLUTION-005
#[test]
fn resolution_model_commutativity() {
    ResolutionModel.checker().spawn_bfs().join().assert_properties();
}
"#;
        let links = scan_source(source, "tests/stateright_model.rs");
        assert!(links
            .iter()
            .any(|l| l.spec_id == "INV-RESOLUTION-002" && l.depth == VerificationDepth::Formal));
        assert!(links
            .iter()
            .any(|l| l.spec_id == "INV-RESOLUTION-005" && l.depth == VerificationDepth::Formal));
    }

    #[test]
    fn scan_takes_highest_depth() {
        let source = r#"
// Verifies: INV-STORE-001

proptest! {
    #[test]
    fn inv_store_001_proptest(s in arb_store(3)) {
        // INV-STORE-001
        prop_assert!(true);
    }
}
"#;
        let links = scan_source(source, "src/store.rs");
        // Should have both L1 and L3 references, but links_to_datoms will take max
        assert!(links
            .iter()
            .any(|l| l.spec_id == "INV-STORE-001" && l.depth == VerificationDepth::Property));
    }

    #[test]
    fn links_to_datoms_produces_correct_structure() {
        let mut links = BTreeSet::new();
        links.insert(TraceLink {
            spec_id: "INV-STORE-001".into(),
            source_file: "src/store.rs".into(),
            test_fn: Some("test_append_only".into()),
            depth: VerificationDepth::Structural,
        });

        let agent = crate::datom::AgentId::from_name("test");
        let tx = TxId::new(100, 0, agent);
        let datoms = links_to_datoms(&links, tx);

        // Should have 5 datoms: ident + implements + depth + file + module
        assert_eq!(datoms.len(), 5);

        // Check verification depth
        let depth_datom = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":impl/verification-depth")
            .unwrap();
        assert_eq!(depth_datom.value, Value::Long(2)); // L2 = Structural

        // Check implements link
        let impl_datom = datoms
            .iter()
            .find(|d| d.attribute.as_str() == ":impl/implements")
            .unwrap();
        assert!(matches!(impl_datom.value, Value::Ref(_)));
    }

    #[test]
    fn summarize_counts_correctly() {
        let mut links = BTreeSet::new();
        links.insert(TraceLink {
            spec_id: "INV-STORE-001".into(),
            source_file: "src/a.rs".into(),
            test_fn: Some("test_a".into()),
            depth: VerificationDepth::Structural,
        });
        links.insert(TraceLink {
            spec_id: "INV-STORE-002".into(),
            source_file: "src/a.rs".into(),
            test_fn: Some("test_b".into()),
            depth: VerificationDepth::Property,
        });
        links.insert(TraceLink {
            spec_id: "INV-MERGE-001".into(),
            source_file: "src/b.rs".into(),
            test_fn: None,
            depth: VerificationDepth::Syntactic,
        });

        let summary = summarize(&links);
        assert_eq!(summary.total_links, 3);
        assert_eq!(summary.unique_specs, 3);
        assert_eq!(summary.files_scanned, 2);
        assert_eq!(summary.by_depth[&VerificationDepth::Syntactic], 1);
        assert_eq!(summary.by_depth[&VerificationDepth::Structural], 1);
        assert_eq!(summary.by_depth[&VerificationDepth::Property], 1);
    }

    // ===================================================================
    // extract_test_hashes tests
    // ===================================================================

    #[test]
    fn extract_test_hashes_basic() {
        let source = r#"
#[test]
fn my_test() {
    assert_eq!(1 + 1, 2);
}
"#;
        let hashes = extract_test_hashes(source);
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes[0].0, "my_test");
        assert!(!hashes[0].1.is_empty()); // has a hash
        assert_eq!(hashes[0].2, 2); // L2 depth
    }

    #[test]
    fn extract_test_hashes_no_tests() {
        let source = "fn not_a_test() { }";
        assert!(extract_test_hashes(source).is_empty());
    }

    #[test]
    fn extract_test_hashes_multiple() {
        let source = r#"
#[test]
fn test_alpha() {
    assert!(true);
}

#[test]
fn test_beta() {
    assert_eq!(2 + 2, 4);
}
"#;
        let hashes = extract_test_hashes(source);
        assert_eq!(hashes.len(), 2);
        assert_eq!(hashes[0].0, "test_alpha");
        assert_eq!(hashes[1].0, "test_beta");
        // Different bodies produce different hashes
        assert_ne!(hashes[0].1, hashes[1].1);
    }

    #[test]
    fn extract_test_hashes_strips_comments() {
        // Two sources identical except for comments should hash the same
        let source_a = r#"
#[test]
fn test_x() {
    // This is a comment
    assert!(true);
}
"#;
        let source_b = r#"
#[test]
fn test_x() {
    // Different comment text entirely
    assert!(true);
}
"#;
        let ha = extract_test_hashes(source_a);
        let hb = extract_test_hashes(source_b);
        assert_eq!(ha.len(), 1);
        assert_eq!(hb.len(), 1);
        assert_eq!(
            ha[0].1, hb[0].1,
            "comment-only changes should not change hash"
        );
    }

    #[test]
    fn extract_test_hashes_proptest_depth() {
        let source = r#"
proptest! {
    #[test]
    fn prop_check(x in 0..100u32) {
        prop_assert!(x < 100);
    }
}
"#;
        let hashes = extract_test_hashes(source);
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes[0].0, "prop_check");
        assert_eq!(hashes[0].2, 3); // L3 for proptest
    }

    #[test]
    fn extract_test_hashes_kani_depth() {
        let source = r#"
#[cfg(kani)]
#[kani::proof]
fn prove_something() {
    let x: u32 = kani::any();
    kani::assume(x < 10);
    assert!(x < 10);
}
"#;
        let hashes = extract_test_hashes(source);
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes[0].0, "prove_something");
        assert_eq!(hashes[0].2, 3); // L3 for kani
    }

    #[test]
    fn extract_test_hashes_deterministic() {
        let source = r#"
#[test]
fn determinism_check() {
    let v = vec![1, 2, 3];
    assert_eq!(v.len(), 3);
}
"#;
        let h1 = extract_test_hashes(source);
        let h2 = extract_test_hashes(source);
        assert_eq!(h1[0].1, h2[0].1, "hash must be deterministic");
    }

    #[test]
    fn extract_test_hashes_whitespace_normalized() {
        // Same logical body but different indentation
        let source_a = r#"
#[test]
fn test_ws() {
    assert!(true);
}
"#;
        let source_b = r#"
#[test]
fn test_ws() {
        assert!(true);
}
"#;
        let ha = extract_test_hashes(source_a);
        let hb = extract_test_hashes(source_b);
        assert_eq!(
            ha[0].1, hb[0].1,
            "leading whitespace should not affect hash"
        );
    }

    #[test]
    fn extract_test_hashes_nested_braces() {
        let source = r#"
#[test]
fn nested() {
    if true {
        for i in 0..3 {
            assert!(i < 3);
        }
    }
}
"#;
        let hashes = extract_test_hashes(source);
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes[0].0, "nested");
        assert_eq!(hashes[0].2, 2);
    }
}
