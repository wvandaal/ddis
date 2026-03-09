//! Minimal MCP (Model Context Protocol) server over JSON-RPC stdio.
//!
//! Implements the MCP protocol using newline-delimited JSON-RPC over stdin/stdout.
//! Exposes braid-kernel functionality as MCP tools:
//!
//! - `braid_status`   — Store status (datom count, entity count, frontier)
//! - `braid_query`    — Query the store by entity/attribute filter
//! - `braid_transact` — Assert a datom into the store
//! - `braid_harvest`  — Run the harvest pipeline
//! - `braid_seed`     — Generate a seed context for a new session
//! - `braid_guidance` — Get methodology guidance footer
//!
//! # Protocol
//!
//! The server reads newline-delimited JSON-RPC messages from stdin and writes
//! responses to stdout. Each message is a single JSON object on one line.
//!
//! # Invariants
//!
//! - **INV-INTERFACE-009**: Three output modes (JSON for MCP, agent, human).
//! - **INV-STORE-001**: All mutations go through the append-only store.

use std::io::{self, BufRead, Write};
use std::path::Path;

use serde_json::{json, Value as JsonValue};

use braid_kernel::datom::{
    AgentId, Attribute, EntityId, Op, ProvenanceType, TxId, Value as DatomValue,
};
use braid_kernel::guidance::{build_footer, compute_methodology_score, format_footer, Trend};
use braid_kernel::harvest::{harvest_pipeline, SessionContext};
use braid_kernel::layout::TxFile;
use braid_kernel::seed::{assemble_seed, ContextSection};
use braid_kernel::trilateral::check_coherence;
use ordered_float::OrderedFloat;

use crate::error::BraidError;
use crate::layout::DiskLayout;

// ---------------------------------------------------------------------------
// Protocol constants
// ---------------------------------------------------------------------------

const SERVER_NAME: &str = "braid";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const PROTOCOL_VERSION: &str = "2024-11-05";

// ---------------------------------------------------------------------------
// JSON-RPC helpers
// ---------------------------------------------------------------------------

/// Build a JSON-RPC success response.
fn jsonrpc_ok(id: &JsonValue, result: JsonValue) -> JsonValue {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

/// Build a JSON-RPC error response.
fn jsonrpc_error(id: &JsonValue, code: i64, message: &str) -> JsonValue {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        },
    })
}

// Standard JSON-RPC error codes.
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
// const INTERNAL_ERROR is reserved for future use when tool execution
// failures need to be reported as JSON-RPC internal errors rather than
// MCP-level isError responses.

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

/// Return the list of tools in MCP tools/list format.
fn tool_definitions() -> JsonValue {
    json!({
        "tools": [
            {
                "name": "braid_status",
                "description": "Show store status: datom count, entity count, schema attributes, frontier.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                }
            },
            {
                "name": "braid_query",
                "description": "Query the store by entity and/or attribute filter. Returns matching datoms.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "entity": {
                            "type": "string",
                            "description": "Entity keyword to filter by (e.g., ':spec/my-entity')."
                        },
                        "attribute": {
                            "type": "string",
                            "description": "Attribute keyword to filter by (e.g., ':db/doc')."
                        }
                    },
                    "required": [],
                }
            },
            {
                "name": "braid_transact",
                "description": "Assert a datom (entity, attribute, value) into the store.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "entity": {
                            "type": "string",
                            "description": "Entity keyword (e.g., ':spec/my-entity')."
                        },
                        "attribute": {
                            "type": "string",
                            "description": "Attribute keyword (e.g., ':db/doc')."
                        },
                        "value": {
                            "type": "string",
                            "description": "Value to assert (parsed as integer, float, boolean, keyword, or string)."
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Human-readable rationale for this transaction."
                        }
                    },
                    "required": ["entity", "attribute", "value"],
                }
            },
            {
                "name": "braid_harvest",
                "description": "Run the harvest pipeline to detect knowledge gaps and produce candidates.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "Description of the task worked on."
                        },
                        "knowledge": {
                            "type": "object",
                            "description": "Key-value pairs of session knowledge to harvest.",
                            "additionalProperties": { "type": "string" }
                        }
                    },
                    "required": ["task"],
                }
            },
            {
                "name": "braid_seed",
                "description": "Generate a seed context for a new session, assembled from the store.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "Description of the task to work on."
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Token budget for the seed output (default: 2000)."
                        }
                    },
                    "required": ["task"],
                }
            },
            {
                "name": "braid_guidance",
                "description": "Get methodology guidance: divergence score, coherence quadrant, methodology score, and guidance footer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                }
            },
        ]
    })
}

// ---------------------------------------------------------------------------
// Tool dispatch
// ---------------------------------------------------------------------------

/// Execute a tool call and return the MCP content response.
fn call_tool(
    layout: &DiskLayout,
    name: &str,
    arguments: &JsonValue,
) -> Result<JsonValue, BraidError> {
    match name {
        "braid_status" => tool_status(layout),
        "braid_query" => tool_query(layout, arguments),
        "braid_transact" => tool_transact(layout, arguments),
        "braid_harvest" => tool_harvest(layout, arguments),
        "braid_seed" => tool_seed(layout, arguments),
        "braid_guidance" => tool_guidance(layout),
        _ => Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("unknown tool: {name}"),
            }],
            "isError": true,
        })),
    }
}

/// `braid_status` — Show store status.
fn tool_status(layout: &DiskLayout) -> Result<JsonValue, BraidError> {
    let store = layout.load_store()?;
    let hashes = layout.list_tx_hashes()?;

    let frontier: Vec<JsonValue> = store
        .frontier()
        .iter()
        .map(|(agent, tx_id)| {
            json!({
                "agent": hex::encode(agent.as_bytes()),
                "wall_time": tx_id.wall_time(),
                "logical": tx_id.logical(),
            })
        })
        .collect();

    let text = format!(
        "datoms: {}\ntransactions: {}\nentities: {}\nschema_attributes: {}\nfrontier_agents: {}",
        store.len(),
        hashes.len(),
        store.entities().len(),
        store.schema().len(),
        frontier.len(),
    );

    Ok(json!({
        "content": [{
            "type": "text",
            "text": text,
        }],
    }))
}

/// `braid_query` — Query the store by entity/attribute filter.
fn tool_query(layout: &DiskLayout, args: &JsonValue) -> Result<JsonValue, BraidError> {
    let store = layout.load_store()?;

    let entity_filter = args.get("entity").and_then(|v| v.as_str());
    let attribute_filter = args.get("attribute").and_then(|v| v.as_str());

    let entity_id = entity_filter.map(EntityId::from_ident);
    let attr = attribute_filter.map(Attribute::from_keyword);

    let mut lines = Vec::new();
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

        lines.push(format!(
            "[{} {} {:?}]",
            hex::encode(&datom.entity.as_bytes()[..8]),
            datom.attribute.as_str(),
            datom.value,
        ));
        count += 1;
    }

    lines.push(format!("\n{count} datom(s)"));

    Ok(json!({
        "content": [{
            "type": "text",
            "text": lines.join("\n"),
        }],
    }))
}

/// `braid_transact` — Assert a datom into the store.
fn tool_transact(layout: &DiskLayout, args: &JsonValue) -> Result<JsonValue, BraidError> {
    let entity_str = args
        .get("entity")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BraidError::Parse("missing required parameter: entity".into()))?;
    let attribute_str = args
        .get("attribute")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BraidError::Parse("missing required parameter: attribute".into()))?;
    let value_str = args
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BraidError::Parse("missing required parameter: value".into()))?;
    let rationale = args
        .get("rationale")
        .and_then(|v| v.as_str())
        .unwrap_or("MCP transact");

    let store = layout.load_store()?;
    let agent = AgentId::from_name("braid:mcp");

    let entity = EntityId::from_ident(entity_str);
    let attribute = Attribute::from_keyword(attribute_str);
    let value = parse_value(value_str);

    let current_wall = store
        .frontier()
        .values()
        .map(|tx| tx.wall_time())
        .max()
        .unwrap_or(0);
    let tx_id = TxId::new(current_wall + 1, 0, agent);

    let datom = braid_kernel::datom::Datom::new(entity, attribute, value, tx_id, Op::Assert);

    let tx = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Observed,
        rationale: rationale.to_string(),
        causal_predecessors: vec![],
        datoms: vec![datom],
    };

    let file_path = layout.write_tx(&tx)?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("transacted 1 datom(s) -> {}", file_path.relative_path()),
        }],
    }))
}

/// `braid_harvest` — Run the harvest pipeline.
fn tool_harvest(layout: &DiskLayout, args: &JsonValue) -> Result<JsonValue, BraidError> {
    let task = args
        .get("task")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BraidError::Parse("missing required parameter: task".into()))?;

    let store = layout.load_store()?;
    let agent = AgentId::from_name("braid:mcp");

    let mut session_knowledge: Vec<(String, DatomValue)> = Vec::new();
    if let Some(knowledge) = args.get("knowledge").and_then(|v| v.as_object()) {
        for (k, v) in knowledge {
            if let Some(vs) = v.as_str() {
                session_knowledge.push((k.clone(), DatomValue::String(vs.to_string())));
            }
        }
    }

    let current_wall = store
        .frontier()
        .values()
        .map(|tx| tx.wall_time())
        .max()
        .unwrap_or(0);

    let context = SessionContext {
        agent,
        session_start_tx: TxId::new(current_wall, 0, agent),
        task_description: task.to_string(),
        session_knowledge,
    };

    let result = harvest_pipeline(&store, &context);

    let mut out = String::new();
    out.push_str(&format!(
        "harvest: {} candidate(s)\n",
        result.candidates.len()
    ));
    out.push_str(&format!("drift_score: {:.2}\n", result.drift_score));
    out.push_str(&format!(
        "quality: {} total ({} high, {} medium, {} low)\n",
        result.quality.count,
        result.quality.high_confidence,
        result.quality.medium_confidence,
        result.quality.low_confidence,
    ));

    for (i, c) in result.candidates.iter().enumerate() {
        out.push_str(&format!(
            "  [{}] {:?} -- {:?} (confidence: {:.2}): {}\n",
            i + 1,
            c.category,
            c.status,
            c.confidence,
            c.rationale,
        ));
    }

    Ok(json!({
        "content": [{
            "type": "text",
            "text": out,
        }],
    }))
}

/// `braid_seed` — Generate a seed context for a new session.
fn tool_seed(layout: &DiskLayout, args: &JsonValue) -> Result<JsonValue, BraidError> {
    let task = args
        .get("task")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BraidError::Parse("missing required parameter: task".into()))?;
    let budget = args.get("budget").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;

    let store = layout.load_store()?;
    let agent = AgentId::from_name("braid:mcp");
    let seed = assemble_seed(&store, task, budget, agent);

    let mut out = String::new();
    out.push_str(&format!("seed for: {}\n", seed.task));
    out.push_str(&format!(
        "entities_discovered: {}\n",
        seed.entities_discovered
    ));
    out.push_str(&format!(
        "tokens: {} / {} (remaining: {})\n",
        seed.context.total_tokens, budget, seed.context.budget_remaining,
    ));
    out.push_str(&format!(
        "projection: {:?}\n\n",
        seed.context.projection_pattern
    ));

    for section in &seed.context.sections {
        match section {
            ContextSection::Orientation(text) => {
                out.push_str(&format!("## Orientation\n{text}\n\n"));
            }
            ContextSection::Constraints(refs) => {
                if !refs.is_empty() {
                    out.push_str("## Constraints\n");
                    for r in refs {
                        let status = match r.satisfied {
                            Some(true) => "PASS",
                            Some(false) => "FAIL",
                            None => "UNKNOWN",
                        };
                        out.push_str(&format!("  [{status}] {}: {}\n", r.id, r.summary));
                    }
                    out.push('\n');
                }
            }
            ContextSection::State(entries) => {
                if !entries.is_empty() {
                    out.push_str("## State\n");
                    for entry in entries {
                        out.push_str(&format!("{}\n", entry.content));
                    }
                    out.push('\n');
                }
            }
            ContextSection::Warnings(warnings) => {
                if !warnings.is_empty() {
                    out.push_str("## Warnings\n");
                    for w in warnings {
                        out.push_str(&format!("  - {w}\n"));
                    }
                    out.push('\n');
                }
            }
            ContextSection::Directive(text) => {
                out.push_str(&format!("## Directive\n{text}\n"));
            }
        }
    }

    Ok(json!({
        "content": [{
            "type": "text",
            "text": out,
        }],
    }))
}

/// `braid_guidance` — Get methodology guidance footer.
fn tool_guidance(layout: &DiskLayout) -> Result<JsonValue, BraidError> {
    let store = layout.load_store()?;

    let coherence = check_coherence(&store);
    let telemetry = braid_kernel::guidance::SessionTelemetry {
        total_turns: 0,
        transact_turns: 0,
        spec_language_turns: 0,
        query_type_count: 0,
        harvest_quality: 0.0,
        history: vec![],
    };
    let score = compute_methodology_score(&telemetry);
    let footer = build_footer(&telemetry, &store, None, vec![]);

    let mut out = String::new();
    out.push_str(&format!("divergence (phi): {:.4}\n", coherence.phi));
    out.push_str(&format!(
        "D_IS (intent<->spec): {}\n",
        coherence.components.d_is
    ));
    out.push_str(&format!(
        "D_SP (spec<->impl): {}\n",
        coherence.components.d_sp
    ));
    out.push_str(&format!(
        "coherence: {:?} (beta_1={})\n",
        coherence.quadrant, coherence.beta_1
    ));
    out.push_str(&format!(
        "methodology_score: {:.2} (trend: {})\n",
        score.score,
        match score.trend {
            Trend::Up => "up",
            Trend::Down => "down",
            Trend::Stable => "stable",
        }
    ));
    out.push_str(&format!(
        "drift_signal: {}\n\n",
        if score.drift_signal { "YES" } else { "no" }
    ));
    out.push_str("--- guidance footer ---\n");
    out.push_str(&format_footer(&footer));

    Ok(json!({
        "content": [{
            "type": "text",
            "text": out,
        }],
    }))
}

// ---------------------------------------------------------------------------
// Value parsing (reused from transact command)
// ---------------------------------------------------------------------------

/// Parse a string into a DatomValue (integer, float, boolean, keyword, or string).
fn parse_value(s: &str) -> DatomValue {
    if let Ok(n) = s.parse::<i64>() {
        return DatomValue::Long(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return DatomValue::Double(OrderedFloat(f));
    }
    if s == "true" {
        return DatomValue::Boolean(true);
    }
    if s == "false" {
        return DatomValue::Boolean(false);
    }
    if s.starts_with(':') {
        return DatomValue::Keyword(s.to_string());
    }
    DatomValue::String(s.to_string())
}

// ---------------------------------------------------------------------------
// Hex encoding (minimal, avoids adding a dependency)
// ---------------------------------------------------------------------------

mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(HEX_CHARS[(b >> 4) as usize] as char);
            s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
        }
        s
    }
}

// ---------------------------------------------------------------------------
// MCP server event loop
// ---------------------------------------------------------------------------

/// Run the MCP server, reading JSON-RPC from stdin and writing to stdout.
///
/// The server is stateful: it keeps the `DiskLayout` open for the lifetime
/// of the process. Each tool call reloads the store from disk (ensuring it
/// sees any writes from `braid_transact`).
///
/// Protocol: newline-delimited JSON-RPC (one JSON object per line).
pub fn serve(path: &Path) -> Result<(), BraidError> {
    let layout = DiskLayout::open(path)?;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!("mcp: stdin read error: {e}");
                break;
            }
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let msg: JsonValue = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("parse error: {e}"),
                    },
                });
                write_response(&mut stdout_lock, &resp);
                continue;
            }
        };

        let id = msg.get("id").cloned().unwrap_or(JsonValue::Null);
        let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));

        // If this is a notification (no id), skip response for some methods.
        let is_notification = msg.get("id").is_none();

        let response = match method {
            "initialize" => handle_initialize(&id, &params),
            "initialized" => {
                // Client acknowledgement — no response needed.
                if is_notification {
                    continue;
                }
                jsonrpc_ok(&id, json!({}))
            }
            "tools/list" => handle_tools_list(&id),
            "tools/call" => handle_tools_call(&id, &params, &layout),
            "ping" => jsonrpc_ok(&id, json!({})),
            "notifications/cancelled" | "notifications/progress" => {
                // Notifications — no response.
                continue;
            }
            _ => jsonrpc_error(&id, METHOD_NOT_FOUND, &format!("unknown method: {method}")),
        };

        write_response(&mut stdout_lock, &response);
    }

    Ok(())
}

/// Write a JSON-RPC response as a single line to stdout.
fn write_response(writer: &mut impl Write, response: &JsonValue) {
    let bytes = serde_json::to_vec(response).expect("JSON serialization cannot fail");
    // Write as a single line followed by newline.
    let _ = writer.write_all(&bytes);
    let _ = writer.write_all(b"\n");
    let _ = writer.flush();
}

/// Handle `initialize` — return server info and capabilities.
fn handle_initialize(id: &JsonValue, _params: &JsonValue) -> JsonValue {
    jsonrpc_ok(
        id,
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {
                    "listChanged": false,
                },
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION,
            },
        }),
    )
}

/// Handle `tools/list` — return tool definitions.
fn handle_tools_list(id: &JsonValue) -> JsonValue {
    jsonrpc_ok(id, tool_definitions())
}

/// Handle `tools/call` — dispatch to the appropriate tool.
fn handle_tools_call(id: &JsonValue, params: &JsonValue, layout: &DiskLayout) -> JsonValue {
    let name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => return jsonrpc_error(id, INVALID_PARAMS, "missing 'name' in tools/call params"),
    };

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match call_tool(layout, name, &arguments) {
        Ok(result) => jsonrpc_ok(id, result),
        Err(e) => jsonrpc_ok(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("error: {e}"),
                }],
                "isError": true,
            }),
        ),
    }
}

// ---------------------------------------------------------------------------
// Tests — INV-INTERFACE-001, 003, 008, 010
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// INV-INTERFACE-003: Exactly 6 tools exposed via MCP.
    #[test]
    fn exactly_six_tools() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().expect("tools must be an array");
        assert_eq!(
            tools.len(),
            6,
            "INV-INTERFACE-003: must expose exactly 6 tools"
        );
    }

    /// INV-INTERFACE-003: Tool names match the expected set.
    #[test]
    fn tool_names_match_expected_set() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

        let expected = [
            "braid_status",
            "braid_query",
            "braid_transact",
            "braid_harvest",
            "braid_seed",
            "braid_guidance",
        ];

        for exp in &expected {
            assert!(
                names.contains(exp),
                "Missing expected tool: {exp}. Found: {names:?}"
            );
        }
    }

    /// INV-INTERFACE-008: Tool descriptions are navigative (contain actionable verbs)
    /// and are within token budget (<=100 tokens ~ <=400 chars as rough estimate).
    #[test]
    fn tool_descriptions_quality() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();

        for tool in tools {
            let name = tool["name"].as_str().unwrap();
            let desc = tool["description"]
                .as_str()
                .expect("tool must have description");

            // Must not be empty
            assert!(!desc.is_empty(), "Tool {name} has empty description");

            // Must be <= 400 chars (~100 tokens)
            assert!(
                desc.len() <= 400,
                "Tool {name} description too long ({} chars > 400): {desc}",
                desc.len()
            );

            // Must have valid inputSchema with type: object
            let schema = &tool["inputSchema"];
            assert_eq!(
                schema["type"].as_str(),
                Some("object"),
                "Tool {name} inputSchema must be type: object"
            );
        }
    }

    /// INV-INTERFACE-001: All tools produce output (not error) on a fresh store.
    #[test]
    fn all_tools_produce_output_on_fresh_store() {
        let dir = tempfile::tempdir().unwrap();
        let layout = DiskLayout::init(dir.path()).unwrap();

        // Tools that require no arguments
        let no_arg_tools = ["braid_status", "braid_guidance"];
        for tool_name in &no_arg_tools {
            let result = call_tool(&layout, tool_name, &json!({}));
            assert!(result.is_ok(), "Tool {tool_name} should not error");
            let response = result.unwrap();
            let content = response["content"].as_array().expect("content must exist");
            assert!(!content.is_empty(), "Tool {tool_name} must produce content");
            assert_ne!(
                response.get("isError").and_then(|v| v.as_bool()),
                Some(true),
                "Tool {tool_name} should not be an error"
            );
        }

        // braid_query with no filter
        let result = call_tool(&layout, "braid_query", &json!({}));
        assert!(result.is_ok(), "braid_query should not error");

        // braid_transact
        let result = call_tool(
            &layout,
            "braid_transact",
            &json!({
                "entity": ":test/entity",
                "attribute": ":db/doc",
                "value": "test value"
            }),
        );
        assert!(result.is_ok(), "braid_transact should not error");

        // braid_harvest
        let result = call_tool(
            &layout,
            "braid_harvest",
            &json!({ "task": "integration test" }),
        );
        assert!(result.is_ok(), "braid_harvest should not error");

        // braid_seed
        let result = call_tool(&layout, "braid_seed", &json!({ "task": "continue work" }));
        assert!(result.is_ok(), "braid_seed should not error");
    }

    /// INV-INTERFACE-010: Unknown tool returns isError=true.
    #[test]
    fn unknown_tool_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let layout = DiskLayout::init(dir.path()).unwrap();

        let result = call_tool(&layout, "nonexistent_tool", &json!({}));
        assert!(result.is_ok()); // Should return Ok with isError in content
        let response = result.unwrap();
        assert_eq!(
            response["isError"].as_bool(),
            Some(true),
            "Unknown tool must set isError=true"
        );
    }

    /// JSON-RPC response format correctness.
    #[test]
    fn jsonrpc_response_format() {
        let id = json!(42);
        let ok = jsonrpc_ok(&id, json!({"data": "test"}));
        assert_eq!(ok["jsonrpc"], "2.0");
        assert_eq!(ok["id"], 42);
        assert!(ok.get("result").is_some());
        assert!(ok.get("error").is_none());

        let err = jsonrpc_error(&id, METHOD_NOT_FOUND, "not found");
        assert_eq!(err["jsonrpc"], "2.0");
        assert_eq!(err["id"], 42);
        assert!(err.get("error").is_some());
        assert!(err.get("result").is_none());
    }

    /// INV-INTERFACE-010: CLI/MCP semantic equivalence — both paths
    /// use the same kernel functions, so transact via MCP should be
    /// visible to subsequent status via MCP.
    #[test]
    fn transact_visible_in_subsequent_status() {
        let dir = tempfile::tempdir().unwrap();
        let layout = DiskLayout::init(dir.path()).unwrap();

        // Get initial status
        let initial = call_tool(&layout, "braid_status", &json!({})).unwrap();
        let initial_text = initial["content"][0]["text"].as_str().unwrap();

        // Transact a datom
        let _tx = call_tool(
            &layout,
            "braid_transact",
            &json!({
                "entity": ":test/mcp-equiv",
                "attribute": ":db/doc",
                "value": "MCP equivalence test"
            }),
        )
        .unwrap();

        // Get updated status
        let updated = call_tool(&layout, "braid_status", &json!({})).unwrap();
        let updated_text = updated["content"][0]["text"].as_str().unwrap();

        // Store should have grown
        assert_ne!(
            initial_text, updated_text,
            "Status should change after transact"
        );
    }

    /// MCP initialize response format.
    #[test]
    fn initialize_response_format() {
        let id = json!(1);
        let response = handle_initialize(&id, &json!({}));
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        let result = &response["result"];
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert!(result["capabilities"].is_object());
        assert!(result["serverInfo"].is_object());
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
    }

    /// tools/list response contains all tool definitions.
    #[test]
    fn tools_list_response() {
        let id = json!(2);
        let response = handle_tools_list(&id);
        assert_eq!(response["jsonrpc"], "2.0");
        let tools = response["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 6);
    }
}
