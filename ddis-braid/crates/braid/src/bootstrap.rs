//! Self-bootstrap: Parse spec/*.md files and extract specification elements
//! (INVs, ADRs, NEGs) as datoms for transacting into the store.
//!
//! This implements constraint C7 (Self-Bootstrap): DDIS specifies itself.
//! The specification elements become the first dataset the system manages.
//!
//! ADR-FOUNDATION-004: Specification uses DDIS formalism (INV/ADR/NEG).
//! ADR-FOUNDATION-006: Self-bootstrap fixed-point property.

use std::path::Path;

use braid_kernel::datom::{AgentId, Attribute, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::layout::TxFile;

/// A parsed specification element (INV, ADR, or NEG).
#[derive(Clone, Debug)]
pub struct SpecElement {
    /// Element ID (e.g., "INV-STORE-001", "ADR-SEED-002", "NEG-MUTATION-001").
    pub id: String,
    /// Element type.
    pub kind: SpecElementKind,
    /// Namespace (e.g., "STORE", "SEED", "GUIDANCE").
    pub namespace: String,
    /// Human-readable title.
    pub title: String,
    /// Full text body.
    pub body: String,
    /// Source file.
    pub source_file: String,
    /// Stage where this element is first relevant.
    pub stage: Option<u32>,
}

/// Types of specification elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecElementKind {
    /// Invariant — a property that must always hold.
    Invariant,
    /// Architecture Decision Record.
    Adr,
    /// Negative case — something the system must NOT do.
    NegativeCase,
}

impl SpecElementKind {
    /// Keyword representation for datom values.
    pub fn as_keyword(&self) -> &'static str {
        match self {
            SpecElementKind::Invariant => ":spec.element/invariant",
            SpecElementKind::Adr => ":spec.element/adr",
            SpecElementKind::NegativeCase => ":spec.element/negative-case",
        }
    }
}

/// Structured sub-fields extracted from a spec element body.
///
/// Parsing rules: line-by-line scan for **bold**: markers and #### headings.
/// Multi-line captures stop at the next **bold**: marker, #### heading, or double newline.
#[derive(Clone, Debug, Default)]
pub struct BodyFields {
    /// INV Level 0 law / NEG safety property.
    pub statement: Option<String>,
    /// "This is violated if..." / "This is violated when..."
    pub falsification: Option<String>,
    /// "Traces to: SEED §4, ADRS FD-001"
    pub traces_to: Option<String>,
    /// "V:TYPE, V:PROP, V:KANI"
    pub verification: Option<String>,
    /// ADR problem statement.
    pub problem: Option<String>,
    /// ADR decision text.
    pub decision: Option<String>,
    /// ADR consequences.
    pub consequences: Option<String>,
}

/// Extract structured sub-fields from a spec element body.
///
/// Uses line-by-line scanning (no regex crate) to match actual patterns in spec/*.md:
/// - `**Traces to**:` / `**Traces**:` → traces_to
/// - `**Verification**:` → verification
/// - `**Falsification**:` → falsification (multi-line capture)
/// - `**Safety property**:` → statement (for NEGs)
/// - `**Statement**:` / `**Formal statement**:` → statement
/// - `#### Level 0 (Algebraic Law)` → extract text after heading as statement
/// - `#### Problem` → problem (capture until next `####`)
/// - `#### Decision` → decision (capture until next `####`)
/// - `#### Consequences` → consequences (capture until next `####`)
pub fn extract_body_fields(body: &str, kind: SpecElementKind) -> BodyFields {
    let mut fields = BodyFields::default();
    let lines: Vec<&str> = body.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // One-line bold markers: **Key**: rest-of-line
        if let Some(rest) =
            strip_bold_prefix(trimmed, "Traces to").or_else(|| strip_bold_prefix(trimmed, "Traces"))
        {
            fields.traces_to = Some(rest.trim().to_string());
            i += 1;
            continue;
        }
        if let Some(rest) = strip_bold_prefix(trimmed, "Verification") {
            fields.verification = Some(rest.trim().to_string());
            i += 1;
            continue;
        }
        if let Some(rest) = strip_bold_prefix(trimmed, "Statement")
            .or_else(|| strip_bold_prefix(trimmed, "Formal statement"))
        {
            fields.statement = Some(rest.trim().to_string());
            i += 1;
            continue;
        }
        if let Some(rest) = strip_bold_prefix(trimmed, "Safety property") {
            fields.statement = Some(rest.trim().to_string());
            i += 1;
            continue;
        }

        // Multi-line bold markers: **Falsification**: capture until next **bold**: or blank line
        if let Some(first_line) = strip_bold_prefix(trimmed, "Falsification") {
            let captured = capture_multiline(first_line, &lines, i + 1);
            fields.falsification = Some(captured);
            i += 1;
            continue;
        }

        // #### Level 0 headings: capture text after heading as statement (for INVs)
        if kind == SpecElementKind::Invariant
            && trimmed.starts_with("#### Level 0")
            && fields.statement.is_none()
        {
            let captured = capture_until_heading(&lines, i + 1, "####");
            if !captured.is_empty() {
                fields.statement = Some(captured);
            }
            i += 1;
            continue;
        }

        // #### section headings for ADRs
        if kind == SpecElementKind::Adr {
            if trimmed.starts_with("#### Problem") && fields.problem.is_none() {
                fields.problem = Some(capture_until_heading(&lines, i + 1, "####"));
                i += 1;
                continue;
            }
            if trimmed.starts_with("#### Decision") && fields.decision.is_none() {
                fields.decision = Some(capture_until_heading(&lines, i + 1, "####"));
                i += 1;
                continue;
            }
            if trimmed.starts_with("#### Consequences") && fields.consequences.is_none() {
                fields.consequences = Some(capture_until_heading(&lines, i + 1, "####"));
                i += 1;
                continue;
            }
        }

        i += 1;
    }

    fields
}

/// Strip a `**prefix**:` pattern from a line, returning the rest after the colon.
fn strip_bold_prefix<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    // Match: **prefix**: rest  or  **prefix:**: rest
    let marker = format!("**{}**:", prefix);
    if let Some(rest) = line.strip_prefix(&marker) {
        return Some(rest);
    }
    let marker2 = format!("**{}:**", prefix);
    if let Some(rest) = line.strip_prefix(&marker2) {
        return Some(rest);
    }
    // Also match without trailing ** for "**Traces to**: SEED §4"
    let marker3 = format!("**{}**", prefix);
    if let Some(rest) = line.strip_prefix(&marker3) {
        if let Some(rest) = rest.strip_prefix(':') {
            return Some(rest);
        }
    }
    None
}

/// Capture text from a bold-marker line through subsequent lines until
/// the next **bold**: marker, #### heading, or double-blank-line.
fn capture_multiline(first_line: &str, lines: &[&str], start: usize) -> String {
    let mut parts = Vec::new();
    let first = first_line.trim();
    if !first.is_empty() {
        parts.push(first.to_string());
    }
    let mut i = start;
    let mut blank_count = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty() {
            blank_count += 1;
            if blank_count >= 2 {
                break;
            }
            i += 1;
            continue;
        }
        blank_count = 0;
        // Stop at next bold marker or heading
        if (trimmed.starts_with("**") && trimmed.contains("**:"))
            || trimmed.starts_with("####")
            || trimmed.starts_with("### ")
            || trimmed.starts_with("---")
        {
            break;
        }
        parts.push(trimmed.to_string());
        i += 1;
    }
    parts.join(" ").trim().to_string()
}

/// Capture lines from start until the next `####` heading or `---` separator.
fn capture_until_heading(lines: &[&str], start: usize, heading_prefix: &str) -> String {
    let mut parts = Vec::new();
    let mut i = start;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with(heading_prefix) || trimmed.starts_with("---") {
            break;
        }
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
        i += 1;
    }
    parts.join(" ").trim().to_string()
}

/// Parse a spec markdown file and extract all specification elements.
pub fn parse_spec_file(path: &Path) -> Vec<SpecElement> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let source_file = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut elements = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Match patterns: **INV-XXX-NNN: Title** or ### INV-XXX-NNN: Title
        if let Some(elem) = try_parse_element_header(line, &source_file) {
            // Collect body lines until next element header or section header
            let mut body_lines = Vec::new();
            let mut j = i + 1;
            while j < lines.len() {
                let next = lines[j];
                if is_element_header(next) || is_section_header(next) {
                    break;
                }
                body_lines.push(next);
                j += 1;
            }

            let body = body_lines.join("\n").trim().to_string();

            // Extract stage from body
            let stage = extract_stage(&body);

            elements.push(SpecElement {
                id: elem.0,
                kind: elem.1,
                namespace: elem.2,
                title: elem.3,
                body,
                source_file: source_file.clone(),
                stage,
            });

            i = j;
        } else {
            i += 1;
        }
    }

    elements
}

/// Try to parse an element header line, returning (id, kind, namespace, title).
fn try_parse_element_header(
    line: &str,
    _source: &str,
) -> Option<(String, SpecElementKind, String, String)> {
    let trimmed = line.trim();

    // Strip leading markdown: **...**  or ### ...
    let text = if trimmed.starts_with("**") && trimmed.ends_with("**") {
        &trimmed[2..trimmed.len() - 2]
    } else if let Some(rest) = trimmed.strip_prefix("### ") {
        rest
    } else {
        trimmed.strip_prefix("#### ")?
    };

    // Match: INV-XXX-NNN: Title  or  ADR-XXX-NNN: Title  or  NEG-XXX-NNN: Title
    let (kind, prefix) = if text.starts_with("INV-") {
        (SpecElementKind::Invariant, "INV-")
    } else if text.starts_with("ADR-") {
        (SpecElementKind::Adr, "ADR-")
    } else if text.starts_with("NEG-") {
        (SpecElementKind::NegativeCase, "NEG-")
    } else {
        return None;
    };

    // Extract ID and title
    let rest = &text[prefix.len()..];
    let colon_pos = rest.find(':')?;
    let id_suffix = &rest[..colon_pos];
    let title = rest[colon_pos + 1..].trim().to_string();

    // Extract namespace from ID suffix: "STORE-001" → "STORE"
    let dash_pos = id_suffix.rfind('-')?;
    let namespace = id_suffix[..dash_pos].to_string();

    let id = format!("{prefix}{id_suffix}");

    Some((id, kind, namespace, title))
}

/// Check if a line is an element header.
fn is_element_header(line: &str) -> bool {
    let trimmed = line.trim();
    let text = if trimmed.starts_with("**") {
        trimmed
    } else if let Some(rest) = trimmed.strip_prefix("### ") {
        rest
    } else {
        return false;
    };
    text.starts_with("INV-") || text.starts_with("ADR-") || text.starts_with("NEG-")
}

/// Check if a line is a section header (## or ###).
fn is_section_header(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("## §") || trimmed.starts_with("### §")
}

/// Extract a stage number from body text (e.g., "Stage: 0" or "Stage: 1").
fn extract_stage(body: &str) -> Option<u32> {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- Stage: ") {
            if let Ok(n) = rest.trim().parse::<u32>() {
                return Some(n);
            }
        }
        // Also match "Stage:" at start
        if let Some(rest) = trimmed.strip_prefix("Stage: ") {
            if let Ok(n) = rest.trim().parse::<u32>() {
                return Some(n);
            }
        }
    }
    None
}

/// Parse all spec files in a directory.
pub fn parse_spec_dir(dir: &Path) -> Vec<SpecElement> {
    let mut all = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return all,
    };

    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .map(|e| e.path())
        .collect();

    paths.sort();

    for path in paths {
        all.extend(parse_spec_file(&path));
    }

    all
}

/// Convert parsed spec elements into a TxFile for transacting.
pub fn elements_to_tx(elements: &[SpecElement], agent: AgentId) -> TxFile {
    let tx_id = TxId::new(1, 0, agent);
    let mut datoms = Vec::new();

    for elem in elements {
        let entity = EntityId::from_ident(&format!(":spec/{}", elem.id.to_lowercase()));

        // :spec/id
        datoms.push(braid_kernel::datom::Datom::new(
            entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(format!(":spec/{}", elem.id.to_lowercase())),
            tx_id,
            Op::Assert,
        ));

        // :spec/element-type
        datoms.push(braid_kernel::datom::Datom::new(
            entity,
            Attribute::from_keyword(":spec/element-type"),
            Value::Keyword(elem.kind.as_keyword().to_string()),
            tx_id,
            Op::Assert,
        ));

        // :spec/namespace
        datoms.push(braid_kernel::datom::Datom::new(
            entity,
            Attribute::from_keyword(":spec/namespace"),
            Value::Keyword(format!(":spec.ns/{}", elem.namespace.to_lowercase())),
            tx_id,
            Op::Assert,
        ));

        // :db/doc (title)
        datoms.push(braid_kernel::datom::Datom::new(
            entity,
            Attribute::from_keyword(":db/doc"),
            Value::String(elem.title.clone()),
            tx_id,
            Op::Assert,
        ));

        // :spec/source-file
        datoms.push(braid_kernel::datom::Datom::new(
            entity,
            Attribute::from_keyword(":spec/source-file"),
            Value::String(elem.source_file.clone()),
            tx_id,
            Op::Assert,
        ));

        // :spec/stage (if present)
        if let Some(stage) = elem.stage {
            datoms.push(braid_kernel::datom::Datom::new(
                entity,
                Attribute::from_keyword(":spec/stage"),
                Value::Long(stage as i64),
                tx_id,
                Op::Assert,
            ));
        }

        // Phase A: Store body text and structured sub-fields (self-bootstrap fix).
        // Previously only titles were stored; now the full constraint content flows
        // into the store so the seed can surface it to agents.
        if !elem.body.is_empty() {
            datoms.push(braid_kernel::datom::Datom::new(
                entity,
                Attribute::from_keyword(":element/body"),
                Value::String(elem.body.clone()),
                tx_id,
                Op::Assert,
            ));
        }

        let fields = extract_body_fields(&elem.body, elem.kind);

        if let Some(ref stmt) = fields.statement {
            datoms.push(braid_kernel::datom::Datom::new(
                entity,
                Attribute::from_keyword(":spec/statement"),
                Value::String(stmt.clone()),
                tx_id,
                Op::Assert,
            ));
        }
        if let Some(ref fals) = fields.falsification {
            datoms.push(braid_kernel::datom::Datom::new(
                entity,
                Attribute::from_keyword(":spec/falsification"),
                Value::String(fals.clone()),
                tx_id,
                Op::Assert,
            ));
        }
        if let Some(ref traces) = fields.traces_to {
            datoms.push(braid_kernel::datom::Datom::new(
                entity,
                Attribute::from_keyword(":element/traces-to"),
                Value::String(traces.clone()),
                tx_id,
                Op::Assert,
            ));
        }
        if let Some(ref verif) = fields.verification {
            datoms.push(braid_kernel::datom::Datom::new(
                entity,
                Attribute::from_keyword(":spec/verification"),
                Value::String(verif.clone()),
                tx_id,
                Op::Assert,
            ));
        }
        // ADR-specific fields
        if elem.kind == SpecElementKind::Adr {
            if let Some(ref prob) = fields.problem {
                datoms.push(braid_kernel::datom::Datom::new(
                    entity,
                    Attribute::from_keyword(":adr/problem"),
                    Value::String(prob.clone()),
                    tx_id,
                    Op::Assert,
                ));
            }
            if let Some(ref dec) = fields.decision {
                datoms.push(braid_kernel::datom::Datom::new(
                    entity,
                    Attribute::from_keyword(":adr/decision"),
                    Value::String(dec.clone()),
                    tx_id,
                    Op::Assert,
                ));
            }
        }
        // NEG-specific: falsification doubles as violation
        if elem.kind == SpecElementKind::NegativeCase {
            let violation = fields.falsification.as_ref().or(fields.statement.as_ref());
            if let Some(v) = violation {
                datoms.push(braid_kernel::datom::Datom::new(
                    entity,
                    Attribute::from_keyword(":neg/violation"),
                    Value::String(v.to_string()),
                    tx_id,
                    Op::Assert,
                ));
            }
        }
    }

    // Extract dependency edges from traces-to references (INV-SCHEMA-009)
    let dep_datoms = extract_dependency_datoms(elements, tx_id);
    datoms.extend(dep_datoms);

    TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: format!(
            "Self-bootstrap: {} spec elements from spec/*.md",
            elements.len()
        ),
        causal_predecessors: vec![],
        datoms,
    }
}

/// Extract dependency datoms from spec element bodies (INV-SCHEMA-009).
///
/// Scans element bodies for references to other spec elements (e.g., "INV-STORE-001",
/// "ADR-SEED-002") and creates `:dep/from`, `:dep/to`, `:dep/type` datoms.
///
/// NEG-BOOTSTRAP-001: All dependency information is derived from the store (spec bodies),
/// not from external hardcoded lists.
pub fn extract_dependency_datoms(
    elements: &[SpecElement],
    tx_id: TxId,
) -> Vec<braid_kernel::datom::Datom> {
    use std::collections::HashSet;

    // Build index of known element IDs
    let known_ids: HashSet<&str> = elements.iter().map(|e| e.id.as_str()).collect();
    let mut datoms = Vec::new();
    let mut dep_count = 0_u32;

    for elem in elements {
        let from_entity = EntityId::from_ident(&format!(":spec/{}", elem.id.to_lowercase()));
        let refs = extract_spec_references(&elem.body);

        for ref_id in refs {
            if ref_id == elem.id {
                continue; // Skip self-references
            }
            if !known_ids.contains(ref_id.as_str()) {
                continue; // Skip references to unknown elements
            }

            let to_entity = EntityId::from_ident(&format!(":spec/{}", ref_id.to_lowercase()));

            // Create a unique entity for this dependency edge
            let dep_entity = EntityId::from_ident(&format!(
                ":dep/{}-to-{}",
                elem.id.to_lowercase(),
                ref_id.to_lowercase()
            ));

            // :dep/from → source element
            datoms.push(braid_kernel::datom::Datom::new(
                dep_entity,
                Attribute::from_keyword(":dep/from"),
                Value::Ref(from_entity),
                tx_id,
                Op::Assert,
            ));

            // :dep/to → target element
            datoms.push(braid_kernel::datom::Datom::new(
                dep_entity,
                Attribute::from_keyword(":dep/to"),
                Value::Ref(to_entity),
                tx_id,
                Op::Assert,
            ));

            // :dep/type → traces-to (the reference type)
            let dep_type = if elem.body.contains("Traces to")
                || elem.body.contains("traces to")
                || elem.body.contains("Traces:")
            {
                ":dep.type/traces-to"
            } else {
                ":dep.type/references"
            };
            datoms.push(braid_kernel::datom::Datom::new(
                dep_entity,
                Attribute::from_keyword(":dep/type"),
                Value::Keyword(dep_type.to_string()),
                tx_id,
                Op::Assert,
            ));

            dep_count += 1;
        }
    }

    // Track dependency count for observability
    if dep_count > 0 {
        let dep_meta = EntityId::from_ident(":meta/dep-graph");
        datoms.push(braid_kernel::datom::Datom::new(
            dep_meta,
            Attribute::from_keyword(":db/doc"),
            Value::String(format!("{dep_count} dependency edges extracted")),
            tx_id,
            Op::Assert,
        ));
    }

    datoms
}

/// Extract spec element IDs referenced in a body text.
///
/// Matches patterns like "INV-STORE-001", "ADR-SEED-002", "NEG-MUTATION-001".
fn extract_spec_references(body: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for word in body.split(|c: char| !c.is_alphanumeric() && c != '-') {
        if (word.starts_with("INV-") || word.starts_with("ADR-") || word.starts_with("NEG-"))
            && word.len() >= 10
        {
            // Validate it looks like a real ID: PREFIX-NAMESPACE-NNN
            let parts: Vec<&str> = word.splitn(3, '-').collect();
            if parts.len() == 3
                && parts[2].chars().all(|c| c.is_ascii_digit())
                && seen.insert(word.to_string())
            {
                refs.push(word.to_string());
            }
        }
    }

    refs
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_inv_header() {
        let result =
            try_parse_element_header("**INV-STORE-001: Append-Only Immutability**", "test.md");
        let (id, kind, ns, title) = result.unwrap();
        assert_eq!(id, "INV-STORE-001");
        assert_eq!(kind, SpecElementKind::Invariant);
        assert_eq!(ns, "STORE");
        assert_eq!(title, "Append-Only Immutability");
    }

    #[test]
    fn parse_adr_header() {
        let result =
            try_parse_element_header("### ADR-SEED-002: Rate-Distortion Assembly", "test.md");
        let (id, kind, ns, title) = result.unwrap();
        assert_eq!(id, "ADR-SEED-002");
        assert_eq!(kind, SpecElementKind::Adr);
        assert_eq!(ns, "SEED");
        assert_eq!(title, "Rate-Distortion Assembly");
    }

    #[test]
    fn parse_neg_header() {
        let result = try_parse_element_header(
            "**NEG-GUIDANCE-001: No Tool Response Without Footer**",
            "test.md",
        );
        let (id, kind, ns, title) = result.unwrap();
        assert_eq!(id, "NEG-GUIDANCE-001");
        assert_eq!(kind, SpecElementKind::NegativeCase);
        assert_eq!(ns, "GUIDANCE");
        assert_eq!(title, "No Tool Response Without Footer");
    }

    #[test]
    fn parse_rejects_non_element() {
        assert!(try_parse_element_header("## §1. STORE", "test.md").is_none());
        assert!(try_parse_element_header("Regular text", "test.md").is_none());
    }

    #[test]
    fn extract_stage_from_body() {
        let body = "- Traces: SEED §5\n- Verification: V:PROP\n- Stage: 0\n- Some text";
        assert_eq!(extract_stage(body), Some(0));

        let body2 = "- Stage: 2\n- Other text";
        assert_eq!(extract_stage(body2), Some(2));

        let body3 = "No stage here";
        assert_eq!(extract_stage(body3), None);
    }

    #[test]
    fn elements_to_tx_produces_datoms() {
        let elements = vec![SpecElement {
            id: "INV-STORE-001".into(),
            kind: SpecElementKind::Invariant,
            namespace: "STORE".into(),
            title: "Append-Only Immutability".into(),
            body: "The store never deletes.".into(),
            source_file: "01-store.md".into(),
            stage: Some(0),
        }];

        let agent = AgentId::from_name("braid:bootstrap");
        let tx = elements_to_tx(&elements, agent);

        // 1 element × (6 core + 1 :element/body) = 7 datoms minimum
        // Body text "The store never deletes." has no sub-fields, so only body added.
        assert!(
            tx.datoms.len() >= 7,
            "expected >=7 datoms (6 core + body), got {}",
            tx.datoms.len()
        );
        assert_eq!(tx.provenance, ProvenanceType::Derived);

        // Verify :element/body datom exists
        let has_body = tx.datoms.iter().any(|d| {
            d.attribute.as_str() == ":element/body"
                && matches!(&d.value, Value::String(s) if s == "The store never deletes.")
        });
        assert!(has_body, "should have :element/body datom");
    }

    #[test]
    fn extract_body_fields_inv() {
        // Real-world INV body pattern from spec/14-interface.md
        let body = "\
**Traces to**: ADRS IB-002
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
The CLI produces output in exactly one of three modes per invocation:
Json (machine-parseable), Agent (budget-constrained), Human (TTY-formatted).
Mode selection is explicit (flag) or inferred from terminal context.

#### Level 1 (State Invariant)
Every CLI_COMMAND invocation selects exactly one mode.

**Falsification**: A CLI command produces mixed-mode output (e.g., JSON with
TTY escape codes, or agent-mode output without budget constraint).";

        let fields = extract_body_fields(body, SpecElementKind::Invariant);
        assert!(
            fields.traces_to.as_deref() == Some("ADRS IB-002"),
            "traces_to: {:?}",
            fields.traces_to
        );
        assert!(
            fields.verification.as_deref() == Some("`V:PROP`"),
            "verification: {:?}",
            fields.verification
        );
        assert!(
            fields.statement.is_some(),
            "statement should be extracted from Level 0"
        );
        assert!(
            fields.statement.as_ref().unwrap().contains("three modes"),
            "statement: {:?}",
            fields.statement
        );
        assert!(
            fields.falsification.is_some(),
            "falsification should be extracted"
        );
        assert!(
            fields
                .falsification
                .as_ref()
                .unwrap()
                .contains("mixed-mode"),
            "falsification: {:?}",
            fields.falsification
        );
    }

    #[test]
    fn extract_body_fields_adr() {
        let body = "\
**Traces to**: ADRS IB-002
**Verification**: `V:PROP`

#### Problem
How should the CLI format output for different consumers?

#### Decision
Three output modes with explicit selection.

#### Consequences
Agent mode requires token budgeting.";

        let fields = extract_body_fields(body, SpecElementKind::Adr);
        assert_eq!(
            fields.problem.as_deref(),
            Some("How should the CLI format output for different consumers?")
        );
        assert_eq!(
            fields.decision.as_deref(),
            Some("Three output modes with explicit selection.")
        );
        assert_eq!(
            fields.consequences.as_deref(),
            Some("Agent mode requires token budgeting.")
        );
        assert!(fields.traces_to.is_some());
    }

    #[test]
    fn extract_body_fields_neg() {
        let body = "\
**Safety property**: No tool response may omit the guidance footer.
**Falsification**: A CLI command in agent mode returns output without
the trailing guidance section.";

        let fields = extract_body_fields(body, SpecElementKind::NegativeCase);
        assert!(
            fields
                .statement
                .as_ref()
                .unwrap()
                .contains("guidance footer"),
            "statement: {:?}",
            fields.statement
        );
        assert!(
            fields
                .falsification
                .as_ref()
                .unwrap()
                .contains("agent mode"),
            "falsification: {:?}",
            fields.falsification
        );
    }

    #[test]
    fn extract_body_fields_empty() {
        let fields = extract_body_fields("", SpecElementKind::Invariant);
        assert!(fields.statement.is_none());
        assert!(fields.falsification.is_none());
        assert!(fields.traces_to.is_none());
        assert!(fields.verification.is_none());
        assert!(fields.problem.is_none());
        assert!(fields.decision.is_none());
        assert!(fields.consequences.is_none());
    }

    #[test]
    fn elements_to_tx_stores_body_subfields() {
        let body = "\
**Traces to**: SEED §4
**Verification**: `V:TYPE`
**Stage**: 0

#### Level 0 (Algebraic Law)
The store is append-only.

**Falsification**: Any deletion from the store.";

        let elements = vec![SpecElement {
            id: "INV-STORE-001".into(),
            kind: SpecElementKind::Invariant,
            namespace: "STORE".into(),
            title: "Append-Only".into(),
            body: body.into(),
            source_file: "01-store.md".into(),
            stage: Some(0),
        }];

        let agent = AgentId::from_name("test");
        let tx = elements_to_tx(&elements, agent);

        let has_statement = tx
            .datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":spec/statement");
        let has_falsification = tx
            .datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":spec/falsification");
        let has_body = tx
            .datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":element/body");
        let has_traces = tx
            .datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":element/traces-to");
        let has_verif = tx
            .datoms
            .iter()
            .any(|d| d.attribute.as_str() == ":spec/verification");

        assert!(has_body, "should store :element/body");
        assert!(has_statement, "should store :spec/statement");
        assert!(has_falsification, "should store :spec/falsification");
        assert!(has_traces, "should store :element/traces-to");
        assert!(has_verif, "should store :spec/verification");
    }

    #[test]
    fn extract_references_from_body() {
        let body = "Traces to: INV-STORE-001, ADR-SEED-002.\nAlso see NEG-MUTATION-001.";
        let refs = extract_spec_references(body);
        assert_eq!(refs.len(), 3);
        assert!(refs.contains(&"INV-STORE-001".to_string()));
        assert!(refs.contains(&"ADR-SEED-002".to_string()));
        assert!(refs.contains(&"NEG-MUTATION-001".to_string()));
    }

    #[test]
    fn extract_references_deduplicates() {
        let body = "INV-STORE-001 and then INV-STORE-001 again";
        let refs = extract_spec_references(body);
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn extract_references_skips_invalid() {
        let body = "INV-X is not valid. ADR- is incomplete.";
        let refs = extract_spec_references(body);
        assert!(refs.is_empty());
    }

    #[test]
    fn dependency_datoms_generated() {
        let elements = vec![
            SpecElement {
                id: "INV-STORE-001".into(),
                kind: SpecElementKind::Invariant,
                namespace: "STORE".into(),
                title: "Append-Only".into(),
                body: "The store is append-only.".into(),
                source_file: "01-store.md".into(),
                stage: Some(0),
            },
            SpecElement {
                id: "INV-STORE-002".into(),
                kind: SpecElementKind::Invariant,
                namespace: "STORE".into(),
                title: "Strict Growth".into(),
                body: "Traces to: INV-STORE-001. Must grow.".into(),
                source_file: "01-store.md".into(),
                stage: Some(0),
            },
        ];

        let agent = AgentId::from_name("test");
        let tx_id = braid_kernel::datom::TxId::new(1, 0, agent);
        let dep_datoms = extract_dependency_datoms(&elements, tx_id);

        // INV-STORE-002 references INV-STORE-001 → 3 datoms (from, to, type) + 1 meta
        assert_eq!(dep_datoms.len(), 4, "expected 3 dep datoms + 1 meta");
    }

    #[test]
    fn parse_real_spec_dir() {
        let spec_dir = Path::new("/data/projects/ddis/ddis-braid/spec");
        if spec_dir.is_dir() {
            let elements = parse_spec_dir(spec_dir);
            // The spec directory should have many elements
            assert!(
                elements.len() > 50,
                "expected >50 spec elements, got {}",
                elements.len()
            );

            // Should have all three kinds
            let invs = elements
                .iter()
                .filter(|e| e.kind == SpecElementKind::Invariant)
                .count();
            let adrs = elements
                .iter()
                .filter(|e| e.kind == SpecElementKind::Adr)
                .count();
            let negs = elements
                .iter()
                .filter(|e| e.kind == SpecElementKind::NegativeCase)
                .count();

            assert!(invs > 30, "expected >30 INVs, got {invs}");
            assert!(adrs > 20, "expected >20 ADRs, got {adrs}");
            assert!(negs > 5, "expected >5 NEGs, got {negs}");
        }
    }
}
