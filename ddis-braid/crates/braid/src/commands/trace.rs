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
use std::time::SystemTime;

use braid_kernel::datom::{AgentId, Attribute, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::layout::TxFile;
use braid_kernel::store::Store;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

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

/// Verification depth levels (INV-DEPTH-001: monotonically non-decreasing lattice).
///
/// Level 0 — Unverified:    Comment contains spec ref, no validation
/// Level 1 — Syntactic:     Spec ref found AND impl entity references the right file/module
/// Level 2 — Structural:    Level 1 AND test exists in the same module
/// Level 3 — Property:      Level 2 AND test directly names the spec element
/// Level 4 — Formal:        Level 3 AND Kani proof or Stateright model covers the invariant
const DEPTH_SYNTACTIC: i64 = 1;
const DEPTH_STRUCTURAL: i64 = 2;
const DEPTH_PROPERTY: i64 = 3;
const DEPTH_FORMAL: i64 = 4;

/// A traced reference found in a source file.
#[derive(Clone, Debug)]
struct TraceRef {
    /// The spec ref (e.g., "INV-STORE-001")
    spec_ref: String,
    /// Source file path (relative to source directory)
    file: String,
    /// Whether this was found in a test file
    is_test: bool,
    /// Verification depth (1-4), computed from context analysis
    verification_depth: i64,
    /// Evidence for the depth determination
    verification_evidence: String,
}

/// Scan a Rust source file for spec references in comments.
///
/// Only extracts references from comment lines (`//`, `///`, `//!`).
/// Skips references inside code blocks in doc comments to avoid false positives
/// (FM-TRACE-001).
///
/// WP9: Also computes verification depth per reference:
/// - Level 1 (Syntactic): Comment contains spec ref
/// - Level 2 (Structural): Level 1 AND test exists in the same file/module
/// - Level 3 (Property): Level 2 AND a test names the spec element
/// - Level 4 (Formal): Level 3 AND a Kani proof or Stateright model covers it
fn scan_file(path: &Path, relative: &str) -> Vec<TraceRef> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let is_test_path = is_test_file(relative);
    let has_cfg_test = content_has_test_module(&content);

    // Find the line number of the first #[cfg(test)] to establish the test boundary.
    // Comments immediately before #[cfg(test)] (e.g., `// Witnesses:` blocks) are
    // considered test evidence, so we look back up to 20 lines.
    let cfg_test_line = if has_cfg_test {
        content
            .lines()
            .enumerate()
            .find(|(_, l)| l.trim().contains("#[cfg(test)]"))
            .map(|(i, _)| i)
    } else {
        None
    };

    // The test evidence boundary: lines at or after (cfg_test_line - 20) are test evidence.
    let test_start_line = cfg_test_line.map(|l| l.saturating_sub(20));

    // WP9: Pre-scan for depth detection evidence
    let has_kani = content.contains("#[kani::proof]");
    let has_stateright = content.contains("stateright") && content.contains("Checker");
    let has_proptest = content.contains("proptest!") || content.contains("prop_assert");

    let mut refs = Vec::new();
    let mut in_code_block = false;

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        let is_test = is_test_path || test_start_line.is_some_and(|start| line_num >= start);

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
            // WP9: Compute verification depth
            let ref_lower = spec_ref.to_lowercase().replace('-', "_");
            let (depth, evidence) = compute_depth(
                &spec_ref,
                &ref_lower,
                &content,
                is_test,
                has_kani,
                has_stateright,
                has_proptest,
                relative,
                line_num,
            );

            refs.push(TraceRef {
                spec_ref,
                file: relative.to_string(),
                is_test,
                verification_depth: depth,
                verification_evidence: evidence,
            });
        }
    }

    refs
}

/// Compute verification depth for a spec reference in context (WP9).
///
/// Returns (depth, evidence_description).
#[allow(clippy::too_many_arguments)]
fn compute_depth(
    spec_ref: &str,
    ref_lower: &str,
    content: &str,
    is_test: bool,
    has_kani: bool,
    has_stateright: bool,
    has_proptest: bool,
    file: &str,
    _line_num: usize,
) -> (i64, String) {
    // Level 4: Formal verification (Kani proof or Stateright model naming the spec ref)
    if has_kani && content.contains(ref_lower) && content.contains("#[kani::proof]") {
        return (
            DEPTH_FORMAL,
            format!("Kani proof references {spec_ref} in {file}"),
        );
    }
    if has_stateright && content.contains(ref_lower) {
        return (
            DEPTH_FORMAL,
            format!("Stateright model references {spec_ref} in {file}"),
        );
    }

    // Level 3: Property-based test naming the spec element
    if is_test && has_proptest && content.contains(ref_lower) {
        return (
            DEPTH_PROPERTY,
            format!("Proptest names {spec_ref} in {file}"),
        );
    }
    // Also Level 3: #[test] function whose name contains the spec ref
    if is_test && content.contains(&format!("fn test_{ref_lower}"))
        || content.contains(&format!("fn {ref_lower}"))
    {
        return (
            DEPTH_PROPERTY,
            format!("Test function names {spec_ref} in {file}"),
        );
    }

    // Level 2: Structural — test exists in same module (but doesn't name the spec ref)
    if is_test {
        return (
            DEPTH_STRUCTURAL,
            format!("Test file references {spec_ref} in {file}"),
        );
    }

    // Level 1: Syntactic — comment reference only, no test evidence
    (
        DEPTH_SYNTACTIC,
        format!("Comment references {spec_ref} in {file}"),
    )
}

/// Check if a line is a Rust comment.
fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("//") // covers //, ///, //!
}

/// Check if a file is a test file (by path pattern).
fn is_test_file(relative: &str) -> bool {
    relative.contains("/tests/")
        || relative.ends_with("_test.rs")
        || relative.ends_with("_tests.rs")
}

/// Check if file content contains `#[cfg(test)]` module.
fn content_has_test_module(content: &str) -> bool {
    content.contains("#[cfg(test)]")
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

/// Filter a list of Rust files to only those modified after `threshold` (seconds
/// since UNIX epoch). Used by `--incremental` to skip unchanged files.
fn filter_stale_files(
    files: Vec<(std::path::PathBuf, String)>,
    threshold: i64,
) -> Vec<(std::path::PathBuf, String)> {
    files
        .into_iter()
        .filter(|(abs_path, _)| {
            std::fs::metadata(abs_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|modified| {
                    let mtime_secs = modified
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    mtime_secs > threshold
                })
                .unwrap_or(true) // If we can't read mtime, include it (conservative)
        })
        .collect()
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
    /// Verification depth (WP9)
    verification_depth: i64,
    /// How depth was determined
    verification_evidence: String,
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

        // WP9: :impl/verification-depth — how deeply verified this link is
        datoms.push(braid_kernel::datom::Datom::new(
            link.entity,
            Attribute::from_keyword(":impl/verification-depth"),
            Value::Long(link.verification_depth),
            tx_id,
            Op::Assert,
        ));

        // WP9: :impl/verification-evidence — how depth was determined
        datoms.push(braid_kernel::datom::Datom::new(
            link.entity,
            Attribute::from_keyword(":impl/verification-evidence"),
            Value::String(link.verification_evidence.clone()),
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
// Trace staleness detection (SC-1, INV-TRACE-003)
// ---------------------------------------------------------------------------

/// Whether trace data is fresh or stale relative to source file mtimes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceStaleStatus {
    /// All .rs files have mtimes <= the last scan timestamp.
    /// `total` is the number of .rs files scanned.
    Fresh { total: usize },
    /// `stale` .rs files have mtimes > the last scan timestamp.
    /// `total` is the number of .rs files scanned.
    Stale { stale: usize, total: usize },
}

/// The well-known entity ident for the trace scan clock.
const TRACE_CLOCK_IDENT: &str = ":system/trace-clock";

/// Return the last-scan mtime threshold (seconds since UNIX epoch) from the store,
/// or `None` if no scan has been recorded yet.
///
/// Used by `--incremental` to filter files: only files with mtime > threshold
/// need re-scanning.
fn last_scan_mtime(store: &Store) -> Option<i64> {
    let clock_entity = EntityId::from_ident(TRACE_CLOCK_IDENT);
    let mtime_attr = Attribute::from_keyword(":trace/last-scan-mtime");

    store
        .entity_datoms(clock_entity)
        .into_iter()
        .filter(|d| d.attribute == mtime_attr && d.op == Op::Assert)
        .filter_map(|d| match &d.value {
            Value::Long(v) => Some(*v),
            _ => None,
        })
        .max() // LWW semantics — take the latest value
}

/// Check whether trace data is stale by comparing .rs file mtimes against
/// the stored `:trace/last-scan-mtime` on the `:system/trace-clock` entity.
///
/// Returns `Stale(n)` if `n` files have been modified since the last scan,
/// or `Fresh` if no files are newer. If no scan timestamp exists in the store,
/// all files are considered stale.
pub fn check_staleness(store: &Store, source_root: &Path) -> TraceStaleStatus {
    let threshold = match last_scan_mtime(store) {
        Some(t) => t,
        // No scan timestamp recorded yet — everything is stale.
        None => {
            let files = find_rust_files(source_root);
            let total = files.len();
            return if total == 0 {
                TraceStaleStatus::Fresh { total: 0 }
            } else {
                TraceStaleStatus::Stale {
                    stale: total,
                    total,
                }
            };
        }
    };

    let files = find_rust_files(source_root);
    let total = files.len();
    let mut stale_count = 0usize;

    for (abs_path, _relative) in &files {
        if let Ok(meta) = std::fs::metadata(abs_path) {
            if let Ok(modified) = meta.modified() {
                let mtime_secs = modified
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                if mtime_secs > threshold {
                    stale_count += 1;
                }
            }
        }
    }

    if stale_count > 0 {
        TraceStaleStatus::Stale {
            stale: stale_count,
            total,
        }
    } else {
        TraceStaleStatus::Fresh { total }
    }
}

/// Generate a datom recording the current unix time as the trace scan mtime.
///
/// Stored on the `:system/trace-clock` entity with attribute `:trace/last-scan-mtime`.
fn generate_scan_mtime_datom(tx_id: TxId) -> braid_kernel::datom::Datom {
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let clock_entity = EntityId::from_ident(TRACE_CLOCK_IDENT);
    braid_kernel::datom::Datom::new(
        clock_entity,
        Attribute::from_keyword(":trace/last-scan-mtime"),
        Value::Long(now_secs),
        tx_id,
        Op::Assert,
    )
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

/// Run the trace scanner.
///
/// - Dry-run (default): shows what would be linked, no store mutation.
/// - Commit (`--commit`): writes traceability datoms to the store.
/// - Incremental (`--incremental`): only scan files modified since last scan.
///
/// INV-TRACE-001: Completeness — every resolved spec ref produces a datom.
/// INV-TRACE-002: Idempotency — content-addressed entities, running twice = same count.
pub fn run(
    path: &Path,
    source: &Path,
    agent_name: &str,
    commit: bool,
    incremental: bool,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // SC-3: Incremental mode — only scan files modified since last trace scan.
    // When no files are stale, short-circuit with an up-to-date message.
    let all_files = find_rust_files(source);
    let files = if incremental {
        if let Some(threshold) = last_scan_mtime(&store) {
            let stale = filter_stale_files(all_files, threshold);
            if stale.is_empty() {
                let json = serde_json::json!({
                    "incremental": true,
                    "files_scanned": 0,
                    "up_to_date": true,
                });
                let human = "trace: up to date (0 files modified)\n".to_string();
                let agent_out = AgentOutput {
                    context: String::new(),
                    content: human.clone(),
                    footer: String::new(),
                };
                return Ok(CommandOutput {
                    json,
                    agent: agent_out,
                    human,
                });
            }
            stale
        } else {
            // No prior scan recorded — fall back to full scan
            all_files
        }
    } else {
        all_files
    };

    // Build the spec ref → entity lookup
    let spec_map = build_spec_ref_map(&store);

    // Scan Rust source files (all, or only stale if incremental)
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
                        verification_depth: trace_ref.verification_depth,
                        verification_evidence: trace_ref.verification_evidence.clone(),
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
    let mut human = String::new();
    human.push_str(&format!(
        "Trace scan: {} files, {} refs found, {} new, {} existing, {} unresolved\n",
        files.len(),
        all_refs.len(),
        resolved_links.len(),
        skipped_existing,
        unresolved.len(),
    ));
    human.push_str(&format!(
        "Spec coverage: {}/{} spec entities have :impl/implements links\n",
        covered_specs.len(),
        spec_map.len(),
    ));

    let total_witnessed = already_witnessed.len() + witnessed_specs.len();
    human.push_str(&format!(
        "Witnessed: {}/{} spec entities have test evidence",
        total_witnessed,
        spec_map.len(),
    ));
    if !witnessed_specs.is_empty() {
        human.push_str(&format!(" (+{} new)", witnessed_specs.len()));
    }
    human.push('\n');

    // WP9: Show verification depth distribution
    let mut depth_counts = [0usize; 5]; // depth 0-4
    if !resolved_links.is_empty() {
        for link in &resolved_links {
            let idx = (link.verification_depth as usize).min(4);
            depth_counts[idx] += 1;
        }
        human.push_str("Verification depth: ");
        let mut parts = Vec::new();
        let labels = [
            "L0:Unverified",
            "L1:Syntactic",
            "L2:Structural",
            "L3:Property",
            "L4:Formal",
        ];
        for (i, label) in labels.iter().enumerate() {
            if depth_counts[i] > 0 {
                parts.push(format!("{}={}", label, depth_counts[i]));
            }
        }
        human.push_str(&parts.join(", "));
        human.push('\n');
    }

    // Show unresolved refs (FM-TRACE-002 warnings)
    let unresolved_list: Vec<serde_json::Value> = unresolved
        .iter()
        .map(|(r, c)| serde_json::json!({ "ref": r, "count": c }))
        .collect();
    if !unresolved.is_empty() {
        human.push_str(&format!("\nUnresolved refs ({}):\n", unresolved.len()));
        for (ref_str, count) in &unresolved {
            human.push_str(&format!("  {} ({}x)\n", ref_str, count));
        }
    }

    // Commit if requested
    let mut committed_datoms = 0usize;
    if commit {
        let agent = AgentId::from_name(agent_name);
        let tx_id = super::write::next_tx_id(&store, agent);

        let mut datoms = generate_impl_datoms(&resolved_links, tx_id);
        let witness_datoms = generate_witness_datoms(&witnessed_specs, tx_id);
        let witness_count = witness_datoms.len();
        datoms.extend(witness_datoms);

        // COTX-4: Auto-generate L1 FBW witness skeletons for new impl links.
        // Every new :impl/implements creates a witness in the same transaction,
        // permanently compounding the witness closure rate.
        let existing_witnesses = braid_kernel::witness::all_witnesses(&store);
        let witnessed_invs: std::collections::BTreeSet<braid_kernel::EntityId> =
            existing_witnesses.iter().map(|w| w.inv_ref).collect();
        let mut cotx4_count = 0usize;
        for link in &resolved_links {
            // Skip if spec entity already has a witness
            if witnessed_invs.contains(&link.spec_entity) {
                continue;
            }
            let entity_datoms = store.entity_datoms(link.spec_entity);
            let stmt_attr = Attribute::from_keyword(":spec/statement");
            let elem_stmt_attr = Attribute::from_keyword(":element/statement");
            let body_attr = Attribute::from_keyword(":element/body");
            let doc_attr = Attribute::from_keyword(":db/doc");
            let fals_attr = Attribute::from_keyword(":spec/falsification");

            let statement = entity_datoms
                .iter()
                .find(|d| d.attribute == stmt_attr && d.op == Op::Assert)
                .or_else(|| entity_datoms.iter().find(|d| d.attribute == elem_stmt_attr && d.op == Op::Assert))
                .or_else(|| entity_datoms.iter().find(|d| d.attribute == body_attr && d.op == Op::Assert))
                .or_else(|| entity_datoms.iter().find(|d| d.attribute == doc_attr && d.op == Op::Assert))
                .and_then(|d| if let Value::String(s) = &d.value { Some(s.clone()) } else { None })
                .unwrap_or_default();

            let falsification = entity_datoms
                .iter()
                .find(|d| d.attribute == fals_attr && d.op == Op::Assert)
                .and_then(|d| if let Value::String(s) = &d.value { Some(s.clone()) } else { None })
                .unwrap_or_default();

            if statement.is_empty() {
                continue;
            }

            let test_body = if falsification.is_empty() { &statement } else { &falsification };
            let fbw = braid_kernel::witness::create_fbw(
                link.spec_entity,
                &statement,
                &falsification,
                test_body,
                &link.file,
                link.verification_depth.max(1),
                agent_name,
            );
            datoms.extend(braid_kernel::witness::fbw_to_datoms(&fbw, tx_id));
            cotx4_count += 1;
        }

        // SC-1: Record scan timestamp for staleness detection (INV-TRACE-003)
        datoms.push(generate_scan_mtime_datom(tx_id));

        let datom_count = datoms.len();
        committed_datoms = datom_count;
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
            human.push_str(&format!(
                "\nCommitted: {} datoms ({} impl + {} witness + {} FBW cotx) \u{2192} {}\n",
                datom_count,
                datom_count - witness_count - (cotx4_count * 12), // ~12 datoms per FBW
                witness_count,
                cotx4_count,
                file_path.relative_path(),
            ));
        } else {
            human.push_str("\nNothing to commit (no resolved refs).\n");
        }
    } else {
        human.push_str("\nDry run. Use --commit to write traceability datoms.\n");
    }

    let coverage_pct = if spec_map.is_empty() {
        0.0
    } else {
        (covered_specs.len() as f64 / spec_map.len() as f64) * 100.0
    };

    let mut json = serde_json::json!({
        "files_scanned": files.len(),
        "refs_found": all_refs.len(),
        "new_links": resolved_links.len(),
        "existing_links": skipped_existing,
        "unresolved_count": unresolved.len(),
        "unresolved": unresolved_list,
        "spec_coverage": {
            "covered": covered_specs.len(),
            "total": spec_map.len(),
            "pct": (coverage_pct * 100.0).round() / 100.0,
        },
        "witnessed": {
            "total": total_witnessed,
            "new": witnessed_specs.len(),
            "spec_total": spec_map.len(),
        },
        "depth": {
            "L0": depth_counts[0],
            "L1": depth_counts[1],
            "L2": depth_counts[2],
            "L3": depth_counts[3],
            "L4": depth_counts[4],
        },
        "committed": commit,
        "committed_datoms": committed_datoms,
    });

    // --- ACP: Build ActionProjection (INV-BUDGET-007) ---
    let action = if commit {
        braid_kernel::budget::ProjectedAction {
            command: "braid status".to_string(),
            rationale: format!("committed {} trace datoms", committed_datoms),
            impact: 0.4,
        }
    } else {
        braid_kernel::budget::ProjectedAction {
            command: "braid trace --commit".to_string(),
            rationale: "commit trace links to store".to_string(),
            impact: 0.5,
        }
    };

    let mut context_blocks = Vec::new();

    // Summary (System — always shown)
    context_blocks.push(braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::System,
        content: format!(
            "trace: {} files, {} refs, {}/{} spec coverage ({:.0}%)",
            files.len(),
            all_refs.len(),
            covered_specs.len(),
            spec_map.len(),
            coverage_pct,
        ),
        tokens: 15,
    });

    // Link counts (Methodology)
    context_blocks.push(braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::Methodology,
        content: format!(
            "{} new links, {} existing, {} unresolved | witnessed: {}/{}",
            resolved_links.len(),
            skipped_existing,
            unresolved.len(),
            total_witnessed,
            spec_map.len(),
        ),
        tokens: 12,
    });

    // Depth distribution (UserRequested — when non-empty)
    if resolved_links.iter().any(|l| l.verification_depth > 0) {
        let labels = ["L0", "L1", "L2", "L3", "L4"];
        let depth_parts: Vec<String> = labels
            .iter()
            .enumerate()
            .filter(|(i, _)| depth_counts[*i] > 0)
            .map(|(i, label)| format!("{}={}", label, depth_counts[i]))
            .collect();
        if !depth_parts.is_empty() {
            context_blocks.push(braid_kernel::budget::ContextBlock {
                precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
                content: format!("depth: {}", depth_parts.join(", ")),
                tokens: 8,
            });
        }
    }

    // Unresolved refs (Speculative — only if present)
    for (ref_str, count) in &unresolved {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::Speculative,
            content: format!("unresolved: {} ({}x)", ref_str, count),
            tokens: 5,
        });
    }

    // Commit result (System — when committed)
    if commit && committed_datoms > 0 {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::System,
            content: format!("committed: {} datoms", committed_datoms),
            tokens: 5,
        });
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: "details: braid query --attribute :impl/implements".to_string(),
    };

    // Human output uses ACP full projection
    let human = projection.project(usize::MAX);

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent_out = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // Merge _acp into JSON
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput {
        json,
        agent: agent_out,
        human,
    })
}

// ---------------------------------------------------------------------------
// Auto-trace scan for harvest integration (T7-1, INV-WITNESS-011)
// ---------------------------------------------------------------------------

/// Result of an automatic trace scan during harvest.
pub struct AutoTraceResult {
    /// Number of .rs files scanned.
    pub files_scanned: usize,
    /// Number of spec refs found.
    pub refs_found: usize,
    /// Number of new :impl/* datoms committed.
    pub new_links: usize,
    /// Number of new :spec/witnessed marks.
    pub new_witnesses: usize,
}

/// Run trace scan as a harvest side-effect (T7-1, INV-WITNESS-011).
///
/// Uses incremental mode by default (SC-3): only scans files modified since
/// the last trace scan. Falls back to full scan if no prior scan exists.
///
/// Returns `None` if no trace datoms were produced.
/// INV-TRACE-002: Idempotent — content-addressed entities skip duplicates.
pub fn auto_trace_scan(
    layout: &DiskLayout,
    store: &braid_kernel::Store,
    source_root: &Path,
    agent_name: &str,
) -> Result<Option<AutoTraceResult>, crate::error::BraidError> {
    use braid_kernel::datom::{AgentId, ProvenanceType};
    use braid_kernel::layout::TxFile;
    use std::collections::BTreeSet;

    let spec_map = build_spec_ref_map(store);
    if spec_map.is_empty() {
        return Ok(None);
    }

    // SC-3: Incremental by default — only scan files modified since last scan.
    let all_files = find_rust_files(source_root);
    let files = if let Some(threshold) = last_scan_mtime(store) {
        let stale = filter_stale_files(all_files, threshold);
        if stale.is_empty() {
            return Ok(Some(AutoTraceResult {
                files_scanned: 0,
                refs_found: 0,
                new_links: 0,
                new_witnesses: 0,
            }));
        }
        stale
    } else {
        all_files
    };
    if files.is_empty() {
        return Ok(None);
    }

    let mut all_refs: Vec<TraceRef> = Vec::new();
    for (abs_path, relative) in &files {
        all_refs.extend(scan_file(abs_path, relative));
    }

    if all_refs.is_empty() {
        return Ok(Some(AutoTraceResult {
            files_scanned: files.len(),
            refs_found: 0,
            new_links: 0,
            new_witnesses: 0,
        }));
    }

    let existing_entities: BTreeSet<EntityId> = store.entities();
    let witnessed_attr = Attribute::from_keyword(":spec/witnessed");
    let already_witnessed: BTreeSet<EntityId> = store
        .attribute_datoms(&witnessed_attr)
        .iter()
        .filter(|d| d.op == Op::Assert && d.value == Value::Boolean(true))
        .map(|d| d.entity)
        .collect();

    let mut resolved_links: Vec<ResolvedLink> = Vec::new();
    let mut witnessed_specs: BTreeSet<EntityId> = BTreeSet::new();
    let mut seen_idents: BTreeSet<String> = BTreeSet::new();

    for trace_ref in &all_refs {
        if let Some(&spec_entity) = spec_map.get(&trace_ref.spec_ref) {
            if trace_ref.is_test && !already_witnessed.contains(&spec_entity) {
                witnessed_specs.insert(spec_entity);
            }
            let ident = format!(
                ":impl/{}.{}",
                trace_ref.file.replace('/', ".").trim_end_matches(".rs"),
                trace_ref.spec_ref.to_lowercase()
            );
            if seen_idents.insert(ident.clone()) {
                let entity = EntityId::from_ident(&ident);
                if existing_entities.contains(&entity) {
                    continue;
                }
                resolved_links.push(ResolvedLink {
                    entity,
                    ident,
                    spec_entity,
                    file: trace_ref.file.clone(),
                    module: module_from_path(&trace_ref.file),
                    verification_depth: trace_ref.verification_depth,
                    verification_evidence: trace_ref.verification_evidence.clone(),
                });
            }
        }
    }

    if resolved_links.is_empty() && witnessed_specs.is_empty() {
        return Ok(Some(AutoTraceResult {
            files_scanned: files.len(),
            refs_found: all_refs.len(),
            new_links: 0,
            new_witnesses: 0,
        }));
    }

    let agent = AgentId::from_name(agent_name);
    let tx_id = super::write::next_tx_id(store, agent);

    let mut datoms = generate_impl_datoms(&resolved_links, tx_id);
    let witness_datoms = generate_witness_datoms(&witnessed_specs, tx_id);
    let witness_count = witness_datoms.len();
    datoms.extend(witness_datoms);

    // SC-1: Record scan timestamp for staleness detection (INV-TRACE-003)
    datoms.push(generate_scan_mtime_datom(tx_id));

    let new_links = resolved_links.len();
    let tx = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: format!(
            "T7-1 auto-trace: {} impl links, {} witness marks (harvest side-effect)",
            new_links, witness_count,
        ),
        causal_predecessors: vec![],
        datoms,
    };

    layout.write_tx(&tx)?;

    Ok(Some(AutoTraceResult {
        files_scanned: files.len(),
        refs_found: all_refs.len(),
        new_links,
        new_witnesses: witness_count,
    }))
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

    // --- Staleness detection tests (SC-1) ---

    #[test]
    fn trace_clock_entity_is_deterministic() {
        // The entity ID for :system/trace-clock must be stable across calls.
        let e1 = EntityId::from_ident(TRACE_CLOCK_IDENT);
        let e2 = EntityId::from_ident(TRACE_CLOCK_IDENT);
        assert_eq!(e1, e2);
    }

    #[test]
    fn generate_scan_mtime_datom_is_well_formed() {
        let agent = AgentId::from_name("test-agent");
        let tx = TxId::new(100, 0, agent);
        let datom = generate_scan_mtime_datom(tx);

        assert_eq!(datom.entity, EntityId::from_ident(TRACE_CLOCK_IDENT));
        assert_eq!(
            datom.attribute,
            Attribute::from_keyword(":trace/last-scan-mtime")
        );
        assert_eq!(datom.op, Op::Assert);
        // Value must be a positive Long (unix seconds).
        match &datom.value {
            Value::Long(v) => assert!(*v > 0, "mtime should be positive unix seconds"),
            other => panic!("expected Value::Long, got {:?}", other),
        }
    }

    #[test]
    fn check_staleness_no_store_timestamp_all_stale() {
        // With an empty store (no :trace/last-scan-mtime), every file is stale.
        let store = Store::from_datoms(BTreeSet::new());
        // Use the crates/ directory which definitely has .rs files.
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let status = check_staleness(&store, source);
        match status {
            TraceStaleStatus::Stale { stale, total } => {
                assert!(stale > 0, "should report >0 stale files");
                assert!(total >= stale, "total >= stale");
            }
            TraceStaleStatus::Fresh { .. } => panic!("expected Stale, got Fresh"),
        }
    }

    #[test]
    fn check_staleness_empty_source_root_is_fresh() {
        // A directory with no .rs files should be Fresh even with no store timestamp.
        let store = Store::from_datoms(BTreeSet::new());
        let tmp = std::env::temp_dir().join("braid-trace-test-empty");
        let _ = std::fs::create_dir_all(&tmp);
        let status = check_staleness(&store, &tmp);
        assert_eq!(status, TraceStaleStatus::Fresh { total: 0 });
    }

    #[test]
    fn check_staleness_future_timestamp_is_fresh() {
        // If the stored timestamp is far in the future, all files should be Fresh.
        let clock_entity = EntityId::from_ident(TRACE_CLOCK_IDENT);
        let agent = AgentId::from_name("test-agent");
        let tx = TxId::new(1, 0, agent);

        // Set the scan mtime to year 2100 (unix seconds).
        let far_future: i64 = 4_102_444_800;
        let datom = braid_kernel::datom::Datom::new(
            clock_entity,
            Attribute::from_keyword(":trace/last-scan-mtime"),
            Value::Long(far_future),
            tx,
            Op::Assert,
        );

        let datom_set: BTreeSet<braid_kernel::datom::Datom> = std::iter::once(datom).collect();
        let store = Store::from_datoms(datom_set);

        // Use the crates/ directory — all files should be older than year 2100.
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let status = check_staleness(&store, source);
        match status {
            TraceStaleStatus::Fresh { total } => {
                assert!(total > 0, "crates/ should have .rs files");
            }
            TraceStaleStatus::Stale { .. } => panic!("expected Fresh, got Stale"),
        }
    }
}
