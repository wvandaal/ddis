//! Minimal MCP (Model Context Protocol) server over JSON-RPC stdio.
//!
//! Implements the MCP protocol using newline-delimited JSON-RPC over stdin/stdout.
//! Exposes braid-kernel functionality as MCP tools:
//!
//! - `braid_status`   — Store status, coherence, methodology, and next actions
//! - `braid_query`    — Query the store by entity/attribute filter
//! - `braid_write`    — Assert a datom into the store
//! - `braid_harvest`  — Run the harvest pipeline
//! - `braid_seed`     — Generate a seed context for a new session
//! - `braid_observe`  — Capture a knowledge observation
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
use braid_kernel::harvest::{harvest_pipeline, SessionContext};
use braid_kernel::layout::TxFile;
use braid_kernel::seed::{assemble_seed, ContextSection};
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
///
/// Tool descriptions follow LLM-native design principles:
/// - Lead with WHEN to use this tool (activation pattern)
/// - Show a concrete example (demonstrations > constraints)
/// - End with what the output looks like (set expectations)
fn tool_definitions() -> JsonValue {
    json!({
        "tools": [
            {
                "name": "braid_status",
                "description": "Use at session start to orient. Returns: datom count, entity count, transaction count, schema size, agent frontier. Example output: datoms: 2687, entities: 671, transactions: 8",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                }
            },
            {
                "name": "braid_query",
                "description": "Search the knowledge store by entity and/or attribute. Use to find specific facts, check what's known about an entity, or list all values of an attribute. Example: entity=':spec/inv-store-001' returns all datoms about that invariant. Omit both filters to scan all asserted datoms (capped at 100).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "entity": {
                            "type": "string",
                            "description": "Entity keyword, e.g. ':spec/inv-store-001' or ':observation/my-note'"
                        },
                        "attribute": {
                            "type": "string",
                            "description": "Attribute keyword, e.g. ':db/doc', ':spec/namespace', ':observation/confidence'"
                        }
                    },
                    "required": [],
                }
            },
            {
                "name": "braid_write",
                "description": "Assert a fact (datom) into the append-only store. Use to record decisions, link entities, or update attributes. The store never deletes — retractions are separate datoms. Example: entity=':adr/use-lanczos', attribute=':db/doc', value='Use Lanczos for spectral analysis'.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "entity": {
                            "type": "string",
                            "description": "Entity keyword (content-addressed: same keyword = same entity)"
                        },
                        "attribute": {
                            "type": "string",
                            "description": "Attribute keyword. Common: :db/doc, :db/ident, :spec/namespace, :intent/rationale"
                        },
                        "value": {
                            "type": "string",
                            "description": "Value (auto-parsed: integers, floats, booleans, :keywords, or strings)"
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Why this fact is being asserted (becomes transaction provenance)"
                        }
                    },
                    "required": ["entity", "attribute", "value"],
                }
            },
            {
                "name": "braid_harvest",
                "description": "End-of-session: extract what you learned into the store. Use after 8+ transactions or when switching tasks. Detects knowledge gaps and produces harvest candidates. Call this before ending any work session.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "What you were working on (becomes harvest provenance)"
                        },
                        "knowledge": {
                            "type": "object",
                            "description": "Key discoveries to persist, e.g. {\"performance\": \"Lanczos converges in 50 steps\"}",
                            "additionalProperties": { "type": "string" }
                        }
                    },
                    "required": ["task"],
                }
            },
            {
                "name": "braid_seed",
                "description": "Start-of-session: load relevant context from the store. Returns orientation, constraints, state, warnings, and a directive — assembled by relevance to your task. Use this instead of manually reading files.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "What you're about to work on (drives relevance scoring)"
                        },
                        "budget": {
                            "type": "integer",
                            "description": "Max tokens for output (default 2000, use 500 for quick orientation)"
                        }
                    },
                    "required": ["task"],
                }
            },
            {
                "name": "braid_observe",
                "description": "Fastest way to capture knowledge. Records an observation with epistemic confidence (0.0-1.0). Use whenever you learn something, make a decision, or notice a pattern. Example: text='CRDT merge is commutative', confidence=0.9, category='theorem'.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The observation text — what you learned or noticed"
                        },
                        "confidence": {
                            "type": "number",
                            "description": "Epistemic confidence 0.0-1.0 (default 0.7). 1.0=certain, 0.5=unsure, 0.0=wild guess"
                        },
                        "category": {
                            "type": "string",
                            "description": "Optional: observation, conjecture, theorem, definition, algorithm, design-decision, open-question"
                        },
                        "relates_to": {
                            "type": "string",
                            "description": "Optional cross-reference to a spec element (e.g., ':spec/inv-store-001')"
                        }
                    },
                    "required": ["text"],
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
        "braid_write" => tool_write(layout, arguments),
        "braid_harvest" => tool_harvest(layout, arguments),
        "braid_seed" => tool_seed(layout, arguments),
        "braid_observe" => tool_observe(layout, arguments),
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

/// `braid_write` — Assert a datom into the store.
fn tool_write(layout: &DiskLayout, args: &JsonValue) -> Result<JsonValue, BraidError> {
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

    let tx_id = crate::commands::write::next_tx_id(&store, agent);

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

    let session_boundary = braid_kernel::guidance::last_harvest_wall_time(&store);

    let context = SessionContext {
        agent,
        agent_name: "braid:mcp".into(),
        session_start_tx: TxId::new(session_boundary, 0, agent),
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

/// `braid_observe` — Capture a knowledge observation.
fn tool_observe(layout: &DiskLayout, args: &JsonValue) -> Result<JsonValue, BraidError> {
    let text = args
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BraidError::Parse("missing required parameter: text".into()))?;

    let confidence = args
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.7);

    let category = args.get("category").and_then(|v| v.as_str());
    let relates_to = args.get("relates_to").and_then(|v| v.as_str());

    let rationale = args.get("rationale").and_then(|v| v.as_str());
    let alternatives = args.get("alternatives").and_then(|v| v.as_str());

    let result = crate::commands::observe::run(crate::commands::observe::ObserveArgs {
        path: &layout.root,
        text,
        confidence,
        tags: &[],
        category,
        agent: "braid:mcp",
        relates_to,
        rationale,
        alternatives,
    })?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": result,
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
/// sees any writes from `braid_write`).
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

    /// INV-INTERFACE-003: Expected number of tools exposed via MCP.
    #[test]
    fn expected_tool_count() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().expect("tools must be an array");
        assert_eq!(
            tools.len(),
            6,
            "INV-INTERFACE-003: must expose expected number of tools"
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
            "braid_write",
            "braid_harvest",
            "braid_seed",
            "braid_observe",
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
        let no_arg_tools = ["braid_status"];
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

        // braid_write
        let result = call_tool(
            &layout,
            "braid_write",
            &json!({
                "entity": ":test/entity",
                "attribute": ":db/doc",
                "value": "test value"
            }),
        );
        assert!(result.is_ok(), "braid_write should not error");

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

        // Write a datom
        let _tx = call_tool(
            &layout,
            "braid_write",
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
