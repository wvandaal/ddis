//! `braid query` — Query the store by entity and/or attribute, or via Datalog.

use std::path::Path;

use braid_kernel::datom::{Attribute, EntityId, Op, Value};
#[cfg(test)]
use braid_kernel::evaluate;
use braid_kernel::query::clause::Term;
use braid_kernel::query::evaluator::{evaluate_with_frontier, QueryResult};
use braid_kernel::store::Frontier;
use braid_kernel::{Clause, FindSpec, Pattern, QueryExpr, Store};

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

/// Parse a `--frontier` flag value into a `Frontier`.
///
/// Supported values:
/// - `"current"` — snapshot of the latest tx per agent (vector clock).
/// - `"tx:N"` — all datoms up to wall-time N (e.g., `"tx:1773000000"`).
/// - `None` — no frontier (all datoms visible).
fn parse_frontier(store: &Store, spec: Option<&str>) -> Result<Option<Frontier>, BraidError> {
    match spec {
        None => Ok(None),
        Some("current") => Ok(Some(Frontier::current(store))),
        Some(s) if s.starts_with("tx:") => {
            let wall_str = &s[3..];
            let wall_time: u64 = wall_str.parse().map_err(|_| {
                BraidError::Validation(format!(
                    "invalid frontier tx wall-time '{wall_str}': expected integer (e.g., tx:1773000000)"
                ))
            })?;
            // Build a TxId at the given wall-time for cutoff comparison.
            // We use a zero agent and zero logical counter — Frontier::at compares
            // by <= on the full TxId ordering, which is wall_time-primary.
            let cutoff = braid_kernel::datom::TxId::new(
                wall_time,
                u32::MAX,
                braid_kernel::datom::AgentId::from_name("__frontier_cutoff__"),
            );
            Ok(Some(Frontier::at(store, cutoff)))
        }
        Some(other) => Err(BraidError::Validation(format!(
            "unknown frontier value '{other}': expected 'current' or 'tx:N'"
        ))),
    }
}

/// Resolve an EntityId to a human-readable label.
///
/// If the entity has a `:db/ident` datom, returns the ident keyword.
/// Otherwise, returns a truncated hex representation of the entity hash.
pub fn resolve_entity_label(store: &Store, entity: EntityId) -> String {
    for datom in store.entity_datoms(entity) {
        if datom.attribute.as_str() == ":db/ident" {
            if let Value::Keyword(kw) = &datom.value {
                return kw.clone();
            }
        }
    }
    // Fallback: truncated hex
    let bytes = entity.as_bytes();
    format!(
        "#{:02x}{:02x}{:02x}{:02x}\u{2026}",
        bytes[0], bytes[1], bytes[2], bytes[3]
    )
}

/// Format a Value for human-readable output.
///
/// Strips the variant wrapper (e.g., `String("foo")` becomes `"foo"`,
/// `Keyword(":db/ident")` becomes `:db/ident`, `Ref(entity)` resolves
/// to the target entity's ident).
pub fn format_value(store: &Store, value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s),
        Value::Keyword(kw) => kw.clone(),
        Value::Boolean(b) => b.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Double(f) => f.to_string(),
        Value::Instant(ms) => format!("#{ms}"),
        Value::Uuid(bytes) => {
            format!(
                "#{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                bytes[0], bytes[1], bytes[2], bytes[3],
                bytes[4], bytes[5], bytes[6], bytes[7],
                bytes[8], bytes[9], bytes[10], bytes[11],
                bytes[12], bytes[13], bytes[14], bytes[15],
            )
        }
        Value::Ref(target) => resolve_entity_label(store, *target),
        Value::Bytes(b) => format!("#bytes[{}]", b.len()),
    }
}

/// Parameters for a datom query (entity/attribute filter mode).
///
/// Bundled into a struct to avoid clippy's too-many-arguments warning
/// while supporting pagination (--limit, --offset, --count).
pub struct QueryParams<'a> {
    pub path: &'a Path,
    pub entity_filter: Option<&'a str>,
    pub attribute_filter: Option<&'a str>,
    pub frontier_spec: Option<&'a str>,
    pub limit: Option<usize>,
    pub offset: usize,
    pub count_only: bool,
    pub json: bool,
}

pub fn run(params: QueryParams<'_>) -> Result<CommandOutput, BraidError> {
    let QueryParams {
        path,
        entity_filter,
        attribute_filter,
        frontier_spec,
        limit,
        offset,
        count_only,
        json,
    } = params;
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let frontier = parse_frontier(&store, frontier_spec)?;

    let entity_id = entity_filter.map(EntityId::from_ident);
    let attr = attribute_filter.map(Attribute::from_keyword);

    let mut results = Vec::new();

    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
        }
        // Apply frontier filter if specified
        if let Some(ref f) = frontier {
            if !f.contains(datom) {
                continue;
            }
        }
        if let Some(eid) = entity_id {
            if datom.entity != eid {
                continue;
            }
        }
        if let Some(ref a) = attr {
            if datom.attribute != *a {
                continue;
            }
        }

        let entity_label = resolve_entity_label(&store, datom.entity);
        let value_str = format_value(&store, &datom.value);
        results.push((
            entity_label,
            datom.attribute.as_str().to_string(),
            value_str,
        ));
    }

    // --- Pagination ---
    let total_count = results.len();
    let paginated: Vec<_> = results
        .into_iter()
        .skip(offset)
        .take(limit.unwrap_or(usize::MAX))
        .collect();
    let results = paginated;
    let is_paginated = limit.is_some() || offset > 0;

    // --- Count-only mode ---
    if count_only {
        let human = format!("{total_count}\n");
        let structured_json = serde_json::json!({
            "mode": "datom",
            "count": total_count,
            "entity_filter": entity_filter,
            "attribute_filter": attribute_filter,
        });
        let agent = AgentOutput {
            context: format!("count: {total_count} datoms"),
            content: human.clone(),
            footer: String::new(),
        };
        return Ok(CommandOutput {
            json: structured_json,
            agent,
            human,
        });
    }

    // --- Human output (--json backward compat) ---
    let json_human = if json {
        let datoms_json: Vec<serde_json::Value> = results
            .iter()
            .map(|(e, a, v)| {
                serde_json::json!({
                    "entity": e,
                    "attribute": a,
                    "value": v,
                })
            })
            .collect();
        let mut result = serde_json::json!({
            "count": results.len(),
            "total": total_count,
            "datoms": datoms_json,
        });
        if is_paginated {
            result["offset"] = serde_json::json!(offset);
            if let Some(lim) = limit {
                result["limit"] = serde_json::json!(lim);
            }
        }
        Some(serde_json::to_string_pretty(&result).unwrap() + "\n")
    } else {
        None
    };

    // --- Structured JSON (always present, regardless of --json flag) ---
    let datoms_json: Vec<serde_json::Value> = results
        .iter()
        .map(|(e, a, v)| {
            serde_json::json!({
                "entity": e,
                "attribute": a,
                "value": v,
            })
        })
        .collect();
    let mut structured_json = serde_json::json!({
        "mode": "datom",
        "count": results.len(),
        "total": total_count,
        "entity_filter": entity_filter,
        "attribute_filter": attribute_filter,
        "datoms": datoms_json,
    });
    if is_paginated {
        structured_json["offset"] = serde_json::json!(offset);
        if let Some(lim) = limit {
            structured_json["limit"] = serde_json::json!(lim);
        }
    }

    // --- ACP: Build ActionProjection (INV-BUDGET-007) ---
    let entity_desc = entity_filter.unwrap_or("*");
    let attr_desc = attribute_filter.unwrap_or("*");
    let pagination_note = if is_paginated {
        format!(" [{}/{}]", results.len(), total_count)
    } else {
        String::new()
    };

    let action = braid_kernel::budget::ProjectedAction {
        command: "braid status".to_string(),
        rationale: "review store state".to_string(),
        impact: 0.2,
    };

    let mut context_blocks = Vec::new();

    // Summary (System — always shown)
    context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
        braid_kernel::budget::OutputPrecedence::System,
        format!(
            "query: {} datoms (entity={}, attribute={}){}",
            results.len(),
            entity_desc,
            attr_desc,
            pagination_note
        ),
        12,
    ));

    // Result rows as context blocks (UserRequested for first 10, Speculative beyond)
    for (i, (entity_label, attr_str, value_str)) in results.iter().enumerate() {
        let precedence = if i < 10 {
            braid_kernel::budget::OutputPrecedence::UserRequested
        } else {
            braid_kernel::budget::OutputPrecedence::Speculative
        };
        context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
            precedence,
            format!("[{} {} {}]", entity_label, attr_str, value_str),
            8,
        ));
    }

    // Build the evidence pointer from the query itself
    let mut evidence_parts = Vec::new();
    if let Some(ef) = entity_filter {
        evidence_parts.push(format!("--entity {ef}"));
    }
    if let Some(af) = attribute_filter {
        evidence_parts.push(format!("--attribute {af}"));
    }
    let evidence_cmd = if evidence_parts.is_empty() {
        "braid query".to_string()
    } else {
        format!("braid query {}", evidence_parts.join(" "))
    };

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: format!(
            "refine: {evidence_cmd} | schema: braid schema --pattern ':spec/*'"
        ),
    };

    // Human output: --json flag gets JSON text (backward compat), otherwise ACP projection
    let human = json_human.unwrap_or_else(|| projection.project(usize::MAX));

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // Merge _acp into JSON
    if let serde_json::Value::Object(ref mut map) = structured_json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput {
        json: structured_json,
        agent,
        human,
    })
}

/// Execute a Datalog query against the store and format results.
pub fn run_datalog(
    path: &Path,
    datalog_src: &str,
    frontier_spec: Option<&str>,
    json: bool,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let frontier = parse_frontier(&store, frontier_spec)?;

    let query = parse_datalog(datalog_src)?;
    let result = evaluate_with_frontier(&store, &query, frontier.as_ref());

    // --- Human output ---
    let (human, result_count) = if json {
        let json_result = match &result {
            QueryResult::Rel(rows) => {
                let columns: Vec<String> = if let FindSpec::Rel(vars) = &query.find {
                    vars.clone()
                } else {
                    vec![]
                };
                let json_rows: Vec<serde_json::Value> = rows
                    .iter()
                    .map(|row| {
                        let formatted: Vec<String> =
                            row.iter().map(|v| format_value(&store, v)).collect();
                        if columns.len() == formatted.len() {
                            let map: serde_json::Map<String, serde_json::Value> = columns
                                .iter()
                                .zip(formatted.iter())
                                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                                .collect();
                            serde_json::Value::Object(map)
                        } else {
                            serde_json::json!(formatted)
                        }
                    })
                    .collect();
                serde_json::json!({
                    "type": "rel",
                    "columns": columns,
                    "count": rows.len(),
                    "rows": json_rows,
                })
            }
            QueryResult::Scalar(val) => match val {
                Some(v) => serde_json::json!({
                    "type": "scalar",
                    "value": format_value(&store, v),
                }),
                None => serde_json::json!({
                    "type": "scalar",
                    "value": null,
                }),
            },
        };
        let count = match &result {
            QueryResult::Rel(rows) => rows.len(),
            QueryResult::Scalar(Some(_)) => 1,
            QueryResult::Scalar(None) => 0,
        };
        (
            serde_json::to_string_pretty(&json_result).unwrap() + "\n",
            count,
        )
    } else {
        let mut out = String::new();
        let count;
        match &result {
            QueryResult::Rel(rows) => {
                // Header: variable names from the find spec
                if let FindSpec::Rel(vars) = &query.find {
                    out.push_str(&vars.join("\t"));
                    out.push('\n');
                    out.push_str(&"-".repeat(vars.len() * 16));
                    out.push('\n');
                }
                for row in rows {
                    let formatted: Vec<String> =
                        row.iter().map(|v| format_value(&store, v)).collect();
                    out.push_str(&formatted.join("\t"));
                    out.push('\n');
                }
                out.push_str(&format!("\n{} result(s)\n", rows.len()));
                count = rows.len();
            }
            QueryResult::Scalar(val) => match val {
                Some(v) => {
                    out.push_str(&format_value(&store, v));
                    out.push('\n');
                    count = 1;
                }
                None => {
                    out.push_str("(no result)\n");
                    count = 0;
                }
            },
        }
        (out, count)
    };

    // --- Structured JSON (always present, regardless of --json flag) ---
    let structured_json = match &result {
        QueryResult::Rel(rows) => {
            let columns: Vec<String> = if let FindSpec::Rel(vars) = &query.find {
                vars.clone()
            } else {
                vec![]
            };
            let json_rows: Vec<serde_json::Value> = rows
                .iter()
                .map(|row| {
                    let formatted: Vec<String> =
                        row.iter().map(|v| format_value(&store, v)).collect();
                    if columns.len() == formatted.len() {
                        let map: serde_json::Map<String, serde_json::Value> = columns
                            .iter()
                            .zip(formatted.iter())
                            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                            .collect();
                        serde_json::Value::Object(map)
                    } else {
                        serde_json::json!(formatted)
                    }
                })
                .collect();
            serde_json::json!({
                "mode": "datalog",
                "count": rows.len(),
                "query": datalog_src,
                "results": json_rows,
            })
        }
        QueryResult::Scalar(val) => match val {
            Some(v) => serde_json::json!({
                "mode": "datalog",
                "count": 1,
                "query": datalog_src,
                "results": [format_value(&store, v)],
            }),
            None => serde_json::json!({
                "mode": "datalog",
                "count": 0,
                "query": datalog_src,
                "results": [],
            }),
        },
    };

    // --- Zero-result diagnostics (INV-INTERFACE-012) ---
    let mut human = human;
    if result_count == 0 {
        let diagnostics = braid_kernel::query::diagnostics::diagnose_empty_results(&store, &query);
        if !diagnostics.is_empty() {
            human.push('\n');
            for diag in &diagnostics {
                human.push_str(&format!("hint: {}\n", diag.message));
                if let Some(ref suggestion) = diag.suggestion {
                    human.push_str(&format!("  fix: {}\n", suggestion));
                }
            }
        }
    }

    // --- ACP: Build ActionProjection (INV-BUDGET-007) ---
    let action = braid_kernel::budget::ProjectedAction {
        command: "braid status".to_string(),
        rationale: "review store state".to_string(),
        impact: 0.2,
    };

    let mut context_blocks = Vec::new();

    // Summary (System — always shown)
    context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
        braid_kernel::budget::OutputPrecedence::System,
        format!("datalog: {} results", result_count),
        5,
    ));

    // Result content as a single block (UserRequested)
    // Truncate to keep within reasonable token budget for large result sets
    let content_for_block = if human.len() > 2000 {
        format!(
            "{}\n... ({} results, showing first ~2000 chars)",
            &human[..human.floor_char_boundary(2000)],
            result_count
        )
    } else {
        human.clone()
    };
    context_blocks.push(braid_kernel::budget::ContextBlock::new_scored(
        braid_kernel::budget::OutputPrecedence::UserRequested,
        content_for_block,
        (result_count * 8).clamp(10, 200),
    ));

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: format!(
            "refine: braid query '{}' | schema: braid schema",
            datalog_src
        ),
    };

    // Human output uses ACP full projection
    let human = projection.project(usize::MAX);

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // Merge _acp into JSON
    let mut structured_json = structured_json;
    if let serde_json::Value::Object(ref mut map) = structured_json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput {
        json: structured_json,
        agent,
        human,
    })
}

// ---------------------------------------------------------------------------
// Datalog parser: converts EDN-like syntax into QueryExpr
// ---------------------------------------------------------------------------

/// Tokenize a Datalog EDN-like string into a flat list of tokens.
///
/// Tokens are: `[`, `]`, `:keyword`, `?variable`, `"string"`, and bare words/numbers.
fn tokenize(input: &str) -> Result<Vec<String>, BraidError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        match chars[i] {
            ' ' | '\t' | '\n' | '\r' | ',' => {
                i += 1;
            }
            '[' => {
                tokens.push("[".to_string());
                i += 1;
            }
            ']' => {
                tokens.push("]".to_string());
                i += 1;
            }
            '(' => {
                tokens.push("(".to_string());
                i += 1;
            }
            ')' => {
                tokens.push(")".to_string());
                i += 1;
            }
            '.' => {
                // Scalar find dot
                tokens.push(".".to_string());
                i += 1;
            }
            '>' | '<' | '=' | '!' => {
                // Comparison operators: >, <, >=, <=, !=, =
                let start = i;
                i += 1;
                if i < len && chars[i] == '=' {
                    i += 1;
                }
                let op: String = chars[start..i].iter().collect();
                tokens.push(op);
            }
            '"' => {
                // Consume string literal
                i += 1; // skip opening quote
                let start = i;
                while i < len && chars[i] != '"' {
                    if chars[i] == '\\' {
                        i += 1; // skip escaped char
                    }
                    i += 1;
                }
                if i >= len {
                    return Err(BraidError::DatalogParse(
                        "unterminated string literal".to_string(),
                    ));
                }
                let s: String = chars[start..i].iter().collect();
                tokens.push(format!("\"{s}\""));
                i += 1; // skip closing quote
            }
            ':' => {
                // Keyword: consume until whitespace or bracket
                let start = i;
                i += 1;
                while i < len && !is_delimiter(chars[i]) {
                    i += 1;
                }
                let kw: String = chars[start..i].iter().collect();
                tokens.push(kw);
            }
            '?' => {
                // Variable: consume until whitespace or bracket
                let start = i;
                i += 1;
                while i < len && !is_delimiter(chars[i]) {
                    i += 1;
                }
                let var: String = chars[start..i].iter().collect();
                tokens.push(var);
            }
            '-' | '0'..='9' => {
                // Number
                let start = i;
                if chars[i] == '-' {
                    i += 1;
                }
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num: String = chars[start..i].iter().collect();
                tokens.push(num);
            }
            c if c.is_alphabetic() || c == '_' => {
                // Bare word (e.g., predicate names like "true", "false")
                let start = i;
                while i < len && !is_delimiter(chars[i]) {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                tokens.push(word);
            }
            other => {
                return Err(BraidError::DatalogParse(format!(
                    "unexpected character '{other}' at position {i}"
                )));
            }
        }
    }

    Ok(tokens)
}

fn is_delimiter(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | ',' | '[' | ']' | '(' | ')')
}

/// Parse a Datalog EDN-like string into a `QueryExpr`.
///
/// Supported syntax:
/// ```text
/// [:find ?var1 ?var2 ... :where [?e :attr ?v] [?e2 :attr2 ?v2] ...]
/// ```
///
/// Variables start with `?`, keywords start with `:`, strings are double-quoted,
/// and numbers are parsed as `Long` (integer) or `Double` (contains `.`).
pub fn parse_datalog(input: &str) -> Result<QueryExpr, BraidError> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(BraidError::DatalogParse("empty query".to_string()));
    }

    let mut pos = 0;

    // Expect opening bracket
    if tokens.get(pos).map(|s| s.as_str()) != Some("[") {
        return Err(BraidError::DatalogParse(
            "query must start with '['".to_string(),
        ));
    }
    pos += 1;

    // Expect :find
    if tokens.get(pos).map(|s| s.as_str()) != Some(":find") {
        return Err(BraidError::DatalogParse(
            "expected ':find' after '['".to_string(),
        ));
    }
    pos += 1;

    // Collect find variables until we hit :where
    let mut find_vars = Vec::new();
    let mut find_scalar = false;
    while pos < tokens.len() {
        let tok = &tokens[pos];
        if tok == ":where" {
            break;
        }
        if tok.starts_with('?') {
            // Check for scalar syntax: ?var .
            find_vars.push(tok.clone());
            pos += 1;
            // Check if next token is "." indicating scalar find
            if pos < tokens.len() && tokens[pos] == "." {
                find_scalar = true;
                pos += 1;
            }
        } else {
            return Err(BraidError::DatalogParse(format!(
                "expected variable in :find clause, got '{tok}'"
            )));
        }
    }

    if find_vars.is_empty() {
        return Err(BraidError::DatalogParse(
            ":find clause has no variables".to_string(),
        ));
    }

    // Expect :where
    if tokens.get(pos).map(|s| s.as_str()) != Some(":where") {
        return Err(BraidError::DatalogParse(
            "expected ':where' after find variables".to_string(),
        ));
    }
    pos += 1;

    // Parse where clauses
    let mut clauses = Vec::new();
    while pos < tokens.len() {
        let tok = &tokens[pos];
        if tok == "]" {
            // Closing bracket of top-level form
            break;
        }
        if tok == "[" {
            pos += 1;
            // Collect terms until matching ']'
            let mut clause_tokens = Vec::new();
            while pos < tokens.len() && tokens[pos] != "]" {
                clause_tokens.push(tokens[pos].clone());
                pos += 1;
            }
            if pos >= tokens.len() {
                return Err(BraidError::DatalogParse(
                    "unterminated where clause".to_string(),
                ));
            }
            pos += 1; // skip ']'

            if clause_tokens.len() != 3 {
                return Err(BraidError::DatalogParse(format!(
                    "where clause must have exactly 3 terms (entity, attribute, value), got {}",
                    clause_tokens.len()
                )));
            }

            let entity = parse_term_at(&clause_tokens[0], TermPosition::Entity)?;
            let attribute = parse_term_at(&clause_tokens[1], TermPosition::Attribute)?;
            let value = parse_term_at(&clause_tokens[2], TermPosition::Value)?;

            clauses.push(Clause::Pattern(Pattern::new(entity, attribute, value)));
        } else if tok == "(" {
            // Predicate clause: (op arg1 arg2)
            pos += 1;
            let mut pred_tokens = Vec::new();
            while pos < tokens.len() && tokens[pos] != ")" {
                pred_tokens.push(tokens[pos].clone());
                pos += 1;
            }
            if pos >= tokens.len() {
                return Err(BraidError::DatalogParse(
                    "unterminated predicate clause".to_string(),
                ));
            }
            pos += 1; // skip ')'

            if pred_tokens.len() < 3 {
                return Err(BraidError::DatalogParse(format!(
                    "predicate clause needs at least 3 tokens (op arg1 arg2), got {}",
                    pred_tokens.len()
                )));
            }

            let op = pred_tokens[0].clone();
            let args: Result<Vec<Term>, BraidError> =
                pred_tokens[1..].iter().map(|t| parse_term(t)).collect();

            clauses.push(Clause::Predicate { op, args: args? });
        } else {
            return Err(BraidError::DatalogParse(format!(
                "expected '[' or '(' to start a where clause, got '{tok}'"
            )));
        }
    }

    if clauses.is_empty() {
        return Err(BraidError::DatalogParse(
            ":where clause has no patterns".to_string(),
        ));
    }

    let find = if find_scalar && find_vars.len() == 1 {
        FindSpec::Scalar(find_vars.into_iter().next().unwrap())
    } else {
        FindSpec::Rel(find_vars)
    };

    Ok(QueryExpr::new(find, clauses))
}

/// Position in a pattern clause — determines how keywords are interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TermPosition {
    /// Entity position: keywords become `Term::Entity(EntityId::from_ident(kw))`.
    Entity,
    /// Attribute position: keywords become `Term::Attr(Attribute::from_keyword(kw))`.
    Attribute,
    /// Value position: keywords become `Term::Constant(Value::Keyword(kw))`.
    Value,
}

/// Counter for generating unique anonymous variable names.
static ANON_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Parse a single token into a query `Term`, using position to resolve keyword ambiguity.
///
/// Keywords (`:foo/bar`) have different semantics depending on position:
/// - **Entity**: content-addressed entity ID via `EntityId::from_ident`
/// - **Attribute**: attribute reference via `Attribute::from_keyword`
/// - **Value**: constant keyword value via `Value::Keyword`
///
/// The anonymous variable `_` is supported and generates a unique internal variable
/// name that won't appear in find results (standard Datalog convention).
fn parse_term_at(token: &str, position: TermPosition) -> Result<Term, BraidError> {
    if token == "_" {
        // Anonymous variable: match anything, don't bind to a named variable.
        // Generate a unique name so each `_` is independent.
        let n = ANON_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(Term::Variable(format!("?__anon_{n}")))
    } else if token.starts_with('?') {
        // Named variable
        Ok(Term::Variable(token.to_string()))
    } else if token.starts_with('"') && token.ends_with('"') && token.len() >= 2 {
        // String literal — strip quotes
        let inner = &token[1..token.len() - 1];
        Ok(Term::Constant(Value::String(inner.to_string())))
    } else if token.starts_with(':') {
        // Keyword — interpretation depends on position
        match position {
            TermPosition::Entity => Ok(Term::Entity(EntityId::from_ident(token))),
            TermPosition::Attribute => Ok(Term::Attr(Attribute::from_keyword(token))),
            TermPosition::Value => Ok(Term::Constant(Value::Keyword(token.to_string()))),
        }
    } else if token == "true" {
        Ok(Term::Constant(Value::Boolean(true)))
    } else if token == "false" {
        Ok(Term::Constant(Value::Boolean(false)))
    } else if token.contains('.') {
        // Float
        token
            .parse::<f64>()
            .map(|f| Term::Constant(Value::Double(ordered_float::OrderedFloat(f))))
            .map_err(|e| BraidError::DatalogParse(format!("invalid number '{token}': {e}")))
    } else {
        // Try integer
        token
            .parse::<i64>()
            .map(|n| Term::Constant(Value::Long(n)))
            .map_err(|e| BraidError::DatalogParse(format!("unrecognized token '{token}': {e}")))
    }
}

/// Parse a term in a predicate (not position-dependent — keywords default to value semantics).
fn parse_term(token: &str) -> Result<Term, BraidError> {
    parse_term_at(token, TermPosition::Value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_find_where() {
        let input = "[:find ?e ?doc :where [?e :db/doc ?doc]]";
        let query = parse_datalog(input).unwrap();

        match &query.find {
            FindSpec::Rel(vars) => {
                assert_eq!(vars, &["?e", "?doc"]);
            }
            other => panic!("expected FindSpec::Rel, got {other:?}"),
        }

        assert_eq!(query.where_clauses.len(), 1);
        match &query.where_clauses[0] {
            Clause::Pattern(p) => {
                assert!(matches!(&p.entity, Term::Variable(v) if v == "?e"));
                assert!(matches!(&p.attribute, Term::Attr(a) if a.as_str() == ":db/doc"));
                assert!(matches!(&p.value, Term::Variable(v) if v == "?doc"));
            }
            other => panic!("expected Pattern, got {other:?}"),
        }
    }

    #[test]
    fn parse_multi_clause_join() {
        let input = "[:find ?name :where [?e :db/ident ?name] [?e :db/valueType :db.type/keyword]]";
        let query = parse_datalog(input).unwrap();

        match &query.find {
            FindSpec::Rel(vars) => assert_eq!(vars, &["?name"]),
            other => panic!("expected Rel, got {other:?}"),
        }

        assert_eq!(query.where_clauses.len(), 2);

        // Verify the keyword in value position is parsed as Constant(Keyword), not Attr
        match &query.where_clauses[1] {
            Clause::Pattern(p) => {
                assert!(
                    matches!(&p.value, Term::Constant(Value::Keyword(kw)) if kw == ":db.type/keyword"),
                    "keyword in value position must be Term::Constant(Value::Keyword), got {:?}",
                    p.value
                );
                assert!(
                    matches!(&p.attribute, Term::Attr(a) if a.as_str() == ":db/valueType"),
                    "keyword in attribute position must be Term::Attr, got {:?}",
                    p.attribute
                );
            }
            other => panic!("expected Pattern, got {other:?}"),
        }
    }

    #[test]
    fn parse_scalar_find() {
        let input = "[:find ?doc . :where [?e :db/doc ?doc]]";
        let query = parse_datalog(input).unwrap();

        assert!(matches!(&query.find, FindSpec::Scalar(v) if v == "?doc"));
    }

    #[test]
    fn parse_string_constant() {
        let input = r#"[:find ?e :where [?e :db/doc "test value"]]"#;
        let query = parse_datalog(input).unwrap();

        match &query.where_clauses[0] {
            Clause::Pattern(p) => {
                assert!(matches!(&p.value, Term::Constant(Value::String(s)) if s == "test value"));
            }
            other => panic!("expected Pattern, got {other:?}"),
        }
    }

    #[test]
    fn parse_predicate_clause() {
        let input = "[:find ?e ?n :where [?e :db/doc ?n] (> ?n 10)]";
        let query = parse_datalog(input).unwrap();

        assert_eq!(query.where_clauses.len(), 2);
        match &query.where_clauses[1] {
            Clause::Predicate { op, args } => {
                assert_eq!(op, ">");
                assert_eq!(args.len(), 2);
                assert!(matches!(&args[0], Term::Variable(v) if v == "?n"));
                assert!(matches!(&args[1], Term::Constant(Value::Long(10))));
            }
            other => panic!("expected Predicate, got {other:?}"),
        }
    }

    #[test]
    fn parse_error_empty() {
        assert!(parse_datalog("").is_err());
    }

    #[test]
    fn parse_error_no_find() {
        assert!(parse_datalog("[:where [?e :a ?v]]").is_err());
    }

    #[test]
    fn parse_error_no_where() {
        assert!(parse_datalog("[:find ?e]").is_err());
    }

    #[test]
    fn tokenize_handles_commas_as_whitespace() {
        let tokens = tokenize("[:find ?e, ?v :where [?e, :a, ?v]]").unwrap();
        // Commas should be treated as whitespace separators
        assert!(tokens.contains(&"?e".to_string()));
        assert!(tokens.contains(&"?v".to_string()));
    }

    #[test]
    fn roundtrip_evaluate_against_genesis_store() {
        let store = Store::genesis();
        let input = "[:find ?e ?doc :where [?e :db/doc ?doc]]";
        let query = parse_datalog(input).unwrap();
        let result = evaluate(&store, &query);

        match result {
            QueryResult::Rel(rows) => {
                // Genesis store has axiomatic attribute docs
                assert!(
                    rows.len() >= 18,
                    "expected at least 18 rows, got {}",
                    rows.len()
                );
            }
            other => panic!("expected Rel, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Position-aware term parsing tests (the fix for the Datalog bug)
    // -----------------------------------------------------------------------

    #[test]
    fn keyword_in_entity_position_becomes_entity_id() {
        let input = "[:find ?v :where [:db/ident :db/doc ?v]]";
        let query = parse_datalog(input).unwrap();

        match &query.where_clauses[0] {
            Clause::Pattern(p) => {
                // Entity position: keyword → Term::Entity
                assert!(
                    matches!(&p.entity, Term::Entity(_)),
                    "keyword in entity position must be Term::Entity, got {:?}",
                    p.entity
                );
                // Attribute position: keyword → Term::Attr
                assert!(
                    matches!(&p.attribute, Term::Attr(a) if a.as_str() == ":db/doc"),
                    "keyword in attribute position must be Term::Attr"
                );
            }
            other => panic!("expected Pattern, got {other:?}"),
        }
    }

    #[test]
    fn keyword_in_value_position_becomes_constant_keyword() {
        let input = "[:find ?e :where [?e :db/valueType :db.type/keyword]]";
        let query = parse_datalog(input).unwrap();

        match &query.where_clauses[0] {
            Clause::Pattern(p) => {
                assert!(
                    matches!(&p.value, Term::Constant(Value::Keyword(kw)) if kw == ":db.type/keyword"),
                    "keyword in value position must be Term::Constant(Value::Keyword), got {:?}",
                    p.value
                );
            }
            other => panic!("expected Pattern, got {other:?}"),
        }
    }

    #[test]
    fn anonymous_variable_supported() {
        let input = "[:find ?e :where [?e :db/doc _]]";
        let query = parse_datalog(input).unwrap();

        match &query.where_clauses[0] {
            Clause::Pattern(p) => {
                // _ should become a unique internal variable
                match &p.value {
                    Term::Variable(v) => {
                        assert!(
                            v.starts_with("?__anon_"),
                            "anonymous variable should be ?__anon_*, got {}",
                            v
                        );
                    }
                    other => panic!("expected Variable for _, got {other:?}"),
                }
            }
            other => panic!("expected Pattern, got {other:?}"),
        }
    }

    #[test]
    fn multiple_anonymous_variables_are_independent() {
        let input = "[:find ?e :where [?e _ _]]";
        let query = parse_datalog(input).unwrap();

        match &query.where_clauses[0] {
            Clause::Pattern(p) => {
                let attr_var = match &p.attribute {
                    Term::Variable(v) => v.clone(),
                    other => panic!("expected Variable, got {other:?}"),
                };
                let val_var = match &p.value {
                    Term::Variable(v) => v.clone(),
                    other => panic!("expected Variable, got {other:?}"),
                };
                assert_ne!(
                    attr_var, val_var,
                    "each _ must generate a unique variable name"
                );
            }
            other => panic!("expected Pattern, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // THE critical regression tests: multi-clause joins via parsed Datalog
    // -----------------------------------------------------------------------

    #[test]
    fn multi_clause_join_via_parser_returns_results() {
        // This is the exact query that was failing before the fix:
        // keyword :db.type/keyword in VALUE position was parsed as Term::Attr
        // instead of Term::Constant(Value::Keyword), causing unify_value to fail.
        let store = Store::genesis();
        let input = "[:find ?name :where [?e :db/ident ?name] [?e :db/valueType :db.type/keyword]]";
        let query = parse_datalog(input).unwrap();
        let result = evaluate(&store, &query);

        match result {
            QueryResult::Rel(rows) => {
                assert!(
                    rows.len() >= 5,
                    "multi-clause join must find keyword-typed attrs, got {} results",
                    rows.len()
                );
                // Verify results contain known keyword-typed attributes
                let names: Vec<String> = rows
                    .iter()
                    .filter_map(|row| match &row[0] {
                        Value::Keyword(kw) => Some(kw.clone()),
                        _ => None,
                    })
                    .collect();
                assert!(
                    names.contains(&":db/ident".to_string()),
                    ":db/ident should be keyword-typed, found: {:?}",
                    names
                );
            }
            other => panic!("expected Rel, got {other:?}"),
        }
    }

    #[test]
    fn entity_keyword_in_query_resolves_correctly() {
        // Query: find the doc for :db/ident using keyword in entity position
        let store = Store::genesis();
        let input = "[:find ?doc . :where [:db/ident :db/doc ?doc]]";
        let query = parse_datalog(input).unwrap();
        let result = evaluate(&store, &query);

        match result {
            QueryResult::Scalar(Some(Value::String(doc))) => {
                assert_eq!(doc, "Attribute's keyword name");
            }
            other => panic!("expected Scalar(String), got {other:?}"),
        }
    }

    #[test]
    fn anonymous_variable_query_returns_results() {
        // Query with _ in value position should match any value
        let store = Store::genesis();
        let input = "[:find ?e :where [?e :db/doc _]]";
        let query = parse_datalog(input).unwrap();
        let result = evaluate(&store, &query);

        match result {
            QueryResult::Rel(rows) => {
                assert!(
                    rows.len() >= 18,
                    "anonymous variable query should match all :db/doc datoms, got {}",
                    rows.len()
                );
            }
            other => panic!("expected Rel, got {other:?}"),
        }
    }

    #[test]
    fn three_clause_join_works() {
        // Three clauses: entity has ident AND doc AND is keyword-typed
        let store = Store::genesis();
        let input = "[:find ?name ?doc :where \
                      [?e :db/ident ?name] \
                      [?e :db/doc ?doc] \
                      [?e :db/valueType :db.type/keyword]]";
        let query = parse_datalog(input).unwrap();
        let result = evaluate(&store, &query);

        match result {
            QueryResult::Rel(rows) => {
                assert!(
                    !rows.is_empty(),
                    "three-clause join must find at least one result"
                );
                // Every result should have both name (keyword) and doc (string)
                for row in &rows {
                    assert!(
                        matches!(&row[0], Value::Keyword(_)),
                        "name should be keyword"
                    );
                    assert!(matches!(&row[1], Value::String(_)), "doc should be string");
                }
            }
            other => panic!("expected Rel, got {other:?}"),
        }
    }
}
