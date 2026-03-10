//! `braid query` — Query the store by entity and/or attribute, or via Datalog.

use std::path::Path;

use braid_kernel::datom::{Attribute, EntityId, Op, Value};
use braid_kernel::query::clause::Term;
use braid_kernel::query::evaluator::QueryResult;
use braid_kernel::{evaluate, Clause, FindSpec, Pattern, QueryExpr, Store};

use crate::error::BraidError;
use crate::layout::DiskLayout;

/// Resolve an EntityId to a human-readable label.
///
/// If the entity has a `:db/ident` datom, returns the ident keyword.
/// Otherwise, returns a truncated hex representation of the entity hash.
fn resolve_entity_label(store: &Store, entity: EntityId) -> String {
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
fn format_value(store: &Store, value: &Value) -> String {
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

pub fn run(
    path: &Path,
    entity_filter: Option<&str>,
    attribute_filter: Option<&str>,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let entity_id = entity_filter.map(EntityId::from_ident);
    let attr = attribute_filter.map(Attribute::from_keyword);

    let mut out = String::new();
    let mut count = 0;

    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
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
        out.push_str(&format!(
            "[{} {} {}]\n",
            entity_label,
            datom.attribute.as_str(),
            value_str,
        ));
        count += 1;
    }

    out.push_str(&format!("\n{count} datom(s)\n"));
    Ok(out)
}

/// Execute a Datalog query against the store and format results.
pub fn run_datalog(path: &Path, datalog_src: &str) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let query = parse_datalog(datalog_src)?;
    let result = evaluate(&store, &query);

    let mut out = String::new();
    match result {
        QueryResult::Rel(rows) => {
            // Header: variable names from the find spec
            if let FindSpec::Rel(vars) = &query.find {
                out.push_str(&vars.join("\t"));
                out.push('\n');
                out.push_str(&"-".repeat(vars.len() * 16));
                out.push('\n');
            }
            for row in &rows {
                let formatted: Vec<String> = row.iter().map(|v| format_value(&store, v)).collect();
                out.push_str(&formatted.join("\t"));
                out.push('\n');
            }
            out.push_str(&format!("\n{} result(s)\n", rows.len()));
        }
        QueryResult::Scalar(val) => match val {
            Some(v) => {
                out.push_str(&format_value(&store, &v));
                out.push('\n');
            }
            None => {
                out.push_str("(no result)\n");
            }
        },
    }

    Ok(out)
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
                    return Err(BraidError::Parse("unterminated string literal".to_string()));
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
                return Err(BraidError::Parse(format!(
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
fn parse_datalog(input: &str) -> Result<QueryExpr, BraidError> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(BraidError::Parse("empty query".to_string()));
    }

    let mut pos = 0;

    // Expect opening bracket
    if tokens.get(pos).map(|s| s.as_str()) != Some("[") {
        return Err(BraidError::Parse("query must start with '['".to_string()));
    }
    pos += 1;

    // Expect :find
    if tokens.get(pos).map(|s| s.as_str()) != Some(":find") {
        return Err(BraidError::Parse("expected ':find' after '['".to_string()));
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
            return Err(BraidError::Parse(format!(
                "expected variable in :find clause, got '{tok}'"
            )));
        }
    }

    if find_vars.is_empty() {
        return Err(BraidError::Parse(
            ":find clause has no variables".to_string(),
        ));
    }

    // Expect :where
    if tokens.get(pos).map(|s| s.as_str()) != Some(":where") {
        return Err(BraidError::Parse(
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
                return Err(BraidError::Parse("unterminated where clause".to_string()));
            }
            pos += 1; // skip ']'

            if clause_tokens.len() != 3 {
                return Err(BraidError::Parse(format!(
                    "where clause must have exactly 3 terms (entity, attribute, value), got {}",
                    clause_tokens.len()
                )));
            }

            let entity = parse_term(&clause_tokens[0])?;
            let attribute = parse_term(&clause_tokens[1])?;
            let value = parse_term(&clause_tokens[2])?;

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
                return Err(BraidError::Parse(
                    "unterminated predicate clause".to_string(),
                ));
            }
            pos += 1; // skip ')'

            if pred_tokens.len() < 3 {
                return Err(BraidError::Parse(format!(
                    "predicate clause needs at least 3 tokens (op arg1 arg2), got {}",
                    pred_tokens.len()
                )));
            }

            let op = pred_tokens[0].clone();
            let args: Result<Vec<Term>, BraidError> =
                pred_tokens[1..].iter().map(|t| parse_term(t)).collect();

            clauses.push(Clause::Predicate { op, args: args? });
        } else {
            return Err(BraidError::Parse(format!(
                "expected '[' or '(' to start a where clause, got '{tok}'"
            )));
        }
    }

    if clauses.is_empty() {
        return Err(BraidError::Parse(
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

/// Parse a single token into a query `Term`.
fn parse_term(token: &str) -> Result<Term, BraidError> {
    if token.starts_with('?') {
        // Variable
        Ok(Term::Variable(token.to_string()))
    } else if token.starts_with('"') && token.ends_with('"') && token.len() >= 2 {
        // String literal — strip quotes
        let inner = &token[1..token.len() - 1];
        Ok(Term::Constant(Value::String(inner.to_string())))
    } else if token.starts_with(':') {
        // Keyword — used as Attribute in the attribute position,
        // or as a Constant(Keyword) in entity/value position.
        // The evaluator handles both Term::Attr and Term::Constant(Keyword).
        Ok(Term::Attr(Attribute::from_keyword(token)))
    } else if token == "true" {
        Ok(Term::Constant(Value::Boolean(true)))
    } else if token == "false" {
        Ok(Term::Constant(Value::Boolean(false)))
    } else if token.contains('.') {
        // Float
        token
            .parse::<f64>()
            .map(|f| Term::Constant(Value::Double(ordered_float::OrderedFloat(f))))
            .map_err(|e| BraidError::Parse(format!("invalid number '{token}': {e}")))
    } else {
        // Try integer
        token
            .parse::<i64>()
            .map(|n| Term::Constant(Value::Long(n)))
            .map_err(|e| BraidError::Parse(format!("unrecognized token '{token}': {e}")))
    }
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
}
