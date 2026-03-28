//! Minimal MCP (Model Context Protocol) server over JSON-RPC stdio.
//!
//! Implements the MCP protocol using newline-delimited JSON-RPC over stdin/stdout.
//! Exposes braid-kernel functionality as MCP tools:
//!
//! - `braid_status`   — Store status, coherence, methodology, and next actions
//! - `braid_query`    — Query the store by Datalog or entity/attribute filter
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
//! - **INV-INTERFACE-002**: MCP as thin wrapper — delegates to kernel functions.
//! - **INV-INTERFACE-004**: Statusline zero-cost to agent.
//! - **INV-INTERFACE-005**: TUI subscription liveness (via MCP server).
//! - **INV-INTERFACE-006**: Human signal injection (via MCP write tool).
//! - **INV-INTERFACE-007**: Proactive harvest warning (in status response).
//! - **INV-INTERFACE-009**: Three output modes (JSON for MCP, agent, human).
//! - **INV-STORE-001**: All mutations go through the append-only store.
//!
//! # Design Decisions
//!
//! - ADR-INTERFACE-001: Five layers plus statusline bridge.
//! - ADR-INTERFACE-003: Store-mediated trajectory management.
//! - ADR-INTERFACE-004: Library-mode persistent MCP server via rmcp.
//! - ADR-INTERFACE-005: Configurable heuristic parameters with progressive disclosure.
//! - ADR-INTERFACE-006: Ten protocol primitives.
//! - ADR-INTERFACE-009: Staged alignment strategy for existing codebase.
//! - ADR-INTERFACE-010: Harvest warning turn-count proxy at Stage 0.
//! - ADR-INTERFACE-011: Command help as agent context.
//!
//! # Negative Cases
//!
//! - NEG-INTERFACE-001: No authoritative non-store state.
//! - NEG-INTERFACE-002: No MCP logic duplication (all logic in kernel).
//! - NEG-INTERFACE-003: No harvest warning suppression.
//! - NEG-INTERFACE-004: No error without recovery hint.

use std::io::{self, BufRead, Write};
use std::path::Path;

use serde_json::{json, Value as JsonValue};

use braid_kernel::datom::{AgentId, Attribute, EntityId, Op, ProvenanceType, Value as DatomValue};
use braid_kernel::layout::TxFile;
use braid_kernel::query::evaluator::{evaluate_with_frontier, QueryResult};
use braid_kernel::query::FindSpec;
use braid_kernel::seed::{assemble_seed, ContextSection};
use ordered_float::OrderedFloat;

use crate::commands::query::{format_value, parse_datalog};
use crate::error::BraidError;
use crate::live_store::LiveStore;

// ---------------------------------------------------------------------------
// Protocol constants
// ---------------------------------------------------------------------------

pub(crate) const SERVER_NAME: &str = "braid";
pub(crate) const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const PROTOCOL_VERSION: &str = "2024-11-05";

// ---------------------------------------------------------------------------
// JSON-RPC helpers
// ---------------------------------------------------------------------------

/// Build a JSON-RPC success response.
pub(crate) fn jsonrpc_ok(id: &JsonValue, result: JsonValue) -> JsonValue {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

/// Build a JSON-RPC error response.
pub(crate) fn jsonrpc_error(id: &JsonValue, code: i64, message: &str) -> JsonValue {
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
pub(crate) const METHOD_NOT_FOUND: i64 = -32601;
pub(crate) const INVALID_PARAMS: i64 = -32602;
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
pub(crate) fn tool_definitions() -> JsonValue {
    json!({
        "tools": [
            {
                "name": "braid_status",
                "description": "Session orientation dashboard. Returns F(S) fitness, M(t) methodology score, coherence metrics, task counts, and next R(t)-routed action. Example: → F(S)=0.66, M(t)=0.50, 231 open tasks, next: braid go t-aB3c",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                }
            },
            {
                "name": "braid_query",
                "description": "Query the datom store. Use datalog for joins/patterns, or entity/attribute for simple lookups. Example: datalog='[:find ?e ?v :where [?e :db/doc ?v]]'",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "datalog": {
                            "type": "string",
                            "description": "Datalog query string. Example: '[:find ?e ?v :where [?e :db/doc ?v]]'"
                        },
                        "entity": {
                            "type": "string",
                            "description": "Entity keyword filter (ignored if datalog is set). Example: ':spec/inv-store-001'"
                        },
                        "attribute": {
                            "type": "string",
                            "description": "Attribute keyword filter (ignored if datalog is set). Example: ':db/doc'"
                        }
                    },
                    "required": [],
                }
            },
            {
                "name": "braid_write",
                "description": "Assert a datom [entity, attribute, value] into the append-only store. For schema links and metadata. Example: entity=':adr/use-lanczos', attribute=':db/doc', value='Use Lanczos'. For knowledge capture, prefer braid_observe.",
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
                "description": "End-of-session: extract knowledge into the store. Example: task='fix merge' → 5 candidates, 12 datoms persisted. Detects knowledge gaps. Call before ending any work session.",
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
                        },
                        "commit": {
                            "type": "boolean",
                            "description": "Persist approved candidates to the store"
                        },
                        "force": {
                            "type": "boolean",
                            "description": "Bypass crystallization guard"
                        },
                        "no_reconcile": {
                            "type": "boolean",
                            "description": "Skip harvest reconciliation (auto-close)"
                        }
                    },
                    "required": ["task"],
                }
            },
            {
                "name": "braid_seed",
                "description": "Start-of-session: load task-relevant context. Returns Identity + Demonstration + Constraints + State + Directive (5 sections, <=budget tokens). Example: task='fix merge', budget=2000 → targeted context assembly.",
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
                "name": "braid_task_ready",
                "description": "List unblocked tasks ranked by R(t) impact. Returns task IDs, titles, priority, and claim commands. Example: → [{id:'t-aB3c', title:'Fix merge', priority:1, claim:'braid go t-aB3c'}]",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                }
            },
            {
                "name": "braid_task_go",
                "description": "Claim a task and set status to in-progress. Use after picking from braid_task_ready. Example: id='t-aB3c' → 'claimed: t-aB3c'",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "Task ID (e.g., 't-aB3c')"
                        }
                    },
                    "required": ["id"],
                }
            },
            {
                "name": "braid_task_close",
                "description": "Close a completed task. Example: id='t-aB3c', reason='Implemented and tested' → 'closed: t-aB3c'",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "description": "Task ID to close"
                        },
                        "reason": {
                            "type": "string",
                            "description": "Why this task is complete (becomes provenance)"
                        }
                    },
                    "required": ["id"],
                }
            },
            {
                "name": "braid_task_create",
                "description": "Create a new task in the store. Returns the generated task ID. Example: title='Fix merge cascade', priority=1, type='bug' → 'created: t-xY9z'",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "title": {
                            "type": "string",
                            "description": "Task title — concise, actionable"
                        },
                        "priority": {
                            "type": "integer",
                            "description": "0=critical, 1=high, 2=medium (default), 3=low, 4=backlog"
                        },
                        "task_type": {
                            "type": "string",
                            "description": "task (default), bug, feature, epic, test, docs"
                        },
                        "description": {
                            "type": "string",
                            "description": "Detailed task description"
                        },
                        "traces_to": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Spec element references"
                        },
                        "labels": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Labels for categorization"
                        },
                        "force": {
                            "type": "boolean",
                            "description": "Skip duplicate preview"
                        }
                    },
                    "required": ["title"],
                }
            },
            {
                "name": "braid_observe",
                "description": "Capture knowledge with epistemic confidence. Use for decisions, questions, findings. Example: text='CRDT merge commutes', confidence=0.9, category='theorem' → datom stored.",
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
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Tags for filtering (repeatable)"
                        },
                        "no_auto_crystallize": {
                            "type": "boolean",
                            "description": "Suppress auto-crystallization of spec findings"
                        },
                        "rationale": {
                            "type": "string",
                            "description": "Why this decision was made (for design-decisions)"
                        },
                        "alternatives": {
                            "type": "string",
                            "description": "Alternatives considered (for design-decisions)"
                        }
                    },
                    "required": ["text"],
                }
            },
            {
                "name": "braid_guidance",
                "description": "Full methodology dashboard: M(t) sub-metrics, all R(t) actions with commands, drift status. Example: → M(t)=0.50, 5 actions ranked by impact.",
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
pub(crate) fn call_tool(
    live: &mut LiveStore,
    name: &str,
    arguments: &JsonValue,
) -> Result<JsonValue, BraidError> {
    match name {
        "braid_status" => tool_status(live),
        "braid_query" => tool_query(live, arguments),
        "braid_write" => tool_write(live, arguments),
        "braid_harvest" => tool_harvest(live, arguments),
        "braid_seed" => tool_seed(live, arguments),
        "braid_observe" => tool_observe(live, arguments),
        "braid_guidance" => tool_guidance(live),
        "braid_task_ready" => tool_task_ready(live),
        "braid_task_go" => tool_task_go(live, arguments),
        "braid_task_close" => tool_task_close(live, arguments),
        "braid_task_create" => tool_task_create(live, arguments),
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
///
/// Returns the same rich dashboard as `braid status` CLI (agent mode):
/// F(S), M(t), coherence, harvest warning, task counts, and next action.
/// This is the primary orientation tool — use at session start.
fn tool_status(live: &mut LiveStore) -> Result<JsonValue, BraidError> {
    let root = live.layout().root.clone();
    let output = crate::commands::status::run(
        root.as_path(),
        "braid:mcp",
        false, // json
        false, // verbose
        false, // deep
        false, // spectral
        false, // full
        false, // verify
        false, // commit
        Some(live),
        false, // quiet
    )?;

    // Render agent-mode output as MCP text content
    use crate::output::OutputMode;
    let text = output.render(OutputMode::Agent);

    Ok(json!({
        "content": [{
            "type": "text",
            "text": text,
        }],
    }))
}

/// `braid_query` — Query the store by entity/attribute filter.
fn tool_query(live: &mut LiveStore, args: &JsonValue) -> Result<JsonValue, BraidError> {
    let store = live.store();

    // INV-QUERY-002, INV-INTERFACE-010: Datalog parameter takes priority.
    // If present, parse and evaluate the Datalog query against the store.
    if let Some(datalog_src) = args.get("datalog").and_then(|v| v.as_str()) {
        return tool_query_datalog(store, datalog_src);
    }

    // Fallback: entity/attribute filter scan.
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

/// Evaluate a Datalog query via MCP and format results as text.
///
/// INV-QUERY-002: CALM-compliant monotonic evaluation.
/// INV-INTERFACE-010: Semantic equivalence with CLI `braid query --datalog`.
/// INV-INTERFACE-002: Thin wrapper — delegates to kernel evaluate + CLI parse_datalog.
fn tool_query_datalog(
    store: &braid_kernel::Store,
    datalog_src: &str,
) -> Result<JsonValue, BraidError> {
    let query = parse_datalog(datalog_src)?;
    let result = evaluate_with_frontier(store, &query, None);

    let text = match &result {
        QueryResult::Rel(rows) => {
            let mut out = String::new();
            // Header: variable names from the find spec
            if let FindSpec::Rel(vars) = &query.find {
                out.push_str(&vars.join("\t"));
                out.push('\n');
            }
            for row in rows {
                let formatted: Vec<String> = row.iter().map(|v| format_value(store, v)).collect();
                out.push_str(&formatted.join("\t"));
                out.push('\n');
            }
            out.push_str(&format!("\n{} result(s)", rows.len()));
            out
        }
        QueryResult::Scalar(val) => match val {
            Some(v) => format_value(store, v),
            None => "(no result)".to_string(),
        },
    };

    Ok(json!({
        "content": [{
            "type": "text",
            "text": text,
        }],
    }))
}

/// `braid_write` — Assert a datom into the store.
fn tool_write(live: &mut LiveStore, args: &JsonValue) -> Result<JsonValue, BraidError> {
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

    let agent = AgentId::from_name("braid:mcp");

    let entity = EntityId::from_ident(entity_str);
    let attribute = Attribute::from_keyword(attribute_str);
    let value = parse_value(value_str);

    let tx_id = crate::commands::write::next_tx_id(live.store(), agent);

    let datom = braid_kernel::datom::Datom::new(entity, attribute, value, tx_id, Op::Assert);

    let tx = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Observed,
        rationale: rationale.to_string(),
        causal_predecessors: vec![],
        datoms: vec![datom],
    };

    let file_path = live.write_tx(&tx)?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": format!("transacted 1 datom(s) -> {}", file_path.relative_path()),
        }],
    }))
}

/// `braid_harvest` — Run the harvest pipeline.
fn tool_harvest(live: &mut LiveStore, args: &JsonValue) -> Result<JsonValue, BraidError> {
    let task = args
        .get("task")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BraidError::Parse("missing required parameter: task".into()))?;

    let commit = args
        .get("commit")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    let no_reconcile = args
        .get("no_reconcile")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Convert knowledge object to the key-value pairs format expected by harvest::run
    let mut knowledge_raw: Vec<String> = Vec::new();
    if let Some(knowledge) = args.get("knowledge").and_then(|v| v.as_object()) {
        for (k, v) in knowledge {
            if let Some(vs) = v.as_str() {
                knowledge_raw.push(k.clone());
                knowledge_raw.push(vs.to_string());
            }
        }
    }

    let harvest_path = live.layout().root.clone();
    let output = crate::commands::harvest::run(
        harvest_path.as_path(),
        "braid:mcp",
        Some(task),
        &knowledge_raw,
        commit,
        force,
        no_reconcile,
        None, // MCP has its own shared LiveStore
    )?;

    // Render agent-mode output as MCP text content
    use crate::output::OutputMode;
    let text = output.render(OutputMode::Agent);

    Ok(json!({
        "content": [{
            "type": "text",
            "text": text,
        }],
    }))
}

/// `braid_seed` — Generate a seed context for a new session.
fn tool_seed(live: &mut LiveStore, args: &JsonValue) -> Result<JsonValue, BraidError> {
    let task = args
        .get("task")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BraidError::Parse("missing required parameter: task".into()))?;
    let budget = args.get("budget").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;

    let store = live.store();
    let agent = AgentId::from_name("braid:mcp");
    let seed = assemble_seed(store, task, budget, agent);

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
fn tool_observe(live: &mut LiveStore, args: &JsonValue) -> Result<JsonValue, BraidError> {
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

    let tags: Vec<String> = args
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let no_auto_crystallize = args
        .get("no_auto_crystallize")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let observe_path = live.layout().root.clone();
    let result = crate::commands::observe::run(crate::commands::observe::ObserveArgs {
        path: &observe_path,
        text,
        confidence,
        tags: &tags,
        category,
        agent: "braid:mcp",
        relates_to,
        rationale,
        alternatives,
        no_auto_crystallize,
        pre_opened: None, // MCP has its own shared LiveStore
    })?;

    Ok(json!({
        "content": [{
            "type": "text",
            "text": result.human,
        }],
    }))
}

/// `braid_guidance` — Full methodology dashboard (INV-INTERFACE-003).
///
/// Returns M(t) score with sub-metric breakdown, all R(t)-routed actions
/// with commands, F(S) fitness summary, and drift status. This is the
/// verbose methodology view — use `braid_status` for quick orientation.
fn tool_guidance(live: &mut LiveStore) -> Result<JsonValue, BraidError> {
    use braid_kernel::guidance::{
        compute_methodology_score, compute_routing_with_calibration, derive_actions_with_routing,
        format_actions, telemetry_from_store, Trend,
    };

    let store = live.store();
    let telemetry = telemetry_from_store(store);
    let score = compute_methodology_score(&telemetry);
    let (routings, _calibration) = compute_routing_with_calibration(store);
    let actions = derive_actions_with_routing(store, &routings, None);
    let fitness = store.fitness();

    let mut out = String::new();

    // M(t) headline
    let trend_str = match score.trend {
        Trend::Up => "up",
        Trend::Down => "down",
        Trend::Stable => "stable",
    };
    out.push_str(&format!(
        "methodology: M(t)={:.2} trend={}\n",
        score.score, trend_str,
    ));
    if score.drift_signal {
        out.push_str("WARNING: drift signal active (M(t) < 0.5)\n");
    }

    // M(t) sub-metric breakdown
    let m = &score.components;
    let sub_metrics: [(&str, f64, f64, f64); 4] = [
        ("transact_frequency", m.transact_frequency, 0.30, 0.40),
        ("spec_language_ratio", m.spec_language_ratio, 0.23, 0.30),
        ("query_diversity", m.query_diversity, 0.17, 0.25),
        ("harvest_quality", m.harvest_quality, 0.30, 0.50),
    ];
    out.push_str("M(t) sub-metrics:\n");
    for (name, val, weight, threshold) in &sub_metrics {
        let status = if *val >= *threshold { "above" } else { "below" };
        out.push_str(&format!(
            "  {}: {:.2} (weight: {:.2}, threshold: {:.2}) \u{2014} {}\n",
            name, val, weight, threshold, status,
        ));
    }

    // F(S) summary
    out.push_str(&format!("fitness: F(S)={:.2}\n", fitness.total));

    // All guidance-derived actions
    out.push_str(&format_actions(&actions));

    // R(t) task routing
    if !routings.is_empty() {
        out.push_str("R(t) task routing:\n");
        for (i, r) in routings.iter().enumerate() {
            out.push_str(&format!(
                "  [{}] \"{}\" (impact={:.2}) \u{2192} braid go {}\n",
                i + 1,
                r.label,
                r.impact,
                r.label,
            ));
        }
    }

    Ok(json!({
        "content": [{
            "type": "text",
            "text": out,
        }],
    }))
}

// ---------------------------------------------------------------------------
// Task management tools (t-a0df: INV-TASK-001..004)
// ---------------------------------------------------------------------------

/// `braid_task_ready` — List unblocked tasks ranked by R(t) impact.
fn tool_task_ready(live: &mut LiveStore) -> Result<JsonValue, BraidError> {
    let store = live.store();
    let ready = braid_kernel::compute_ready_set(store);

    if ready.is_empty() {
        return Ok(json!({
            "content": [{"type": "text", "text": "No ready tasks (all blocked or closed)."}],
        }));
    }

    let mut lines = Vec::new();
    lines.push(format!("{} task(s) ready:\n", ready.len()));
    for t in ready.iter().take(15) {
        let type_label = t.task_type.trim_start_matches(":task.type/");
        lines.push(format!(
            "  [P{}] {} \"{}\" ({}) → braid go {}",
            t.priority,
            t.id,
            braid_kernel::safe_truncate_bytes(&t.title, 80),
            type_label,
            t.id,
        ));
    }

    Ok(json!({
        "content": [{"type": "text", "text": lines.join("\n")}],
    }))
}

/// `braid_task_go` — Claim a task (set status=in-progress).
fn tool_task_go(live: &mut LiveStore, args: &JsonValue) -> Result<JsonValue, BraidError> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BraidError::Parse("missing required parameter: id".into()))?;

    let agent = AgentId::from_name("braid:mcp");

    let task_entity = braid_kernel::find_task_by_id(live.store(), id)
        .ok_or_else(|| BraidError::Parse(format!("task not found: {id}")))?;

    let tx_id = crate::commands::write::next_tx_id(live.store(), agent);
    let datom =
        braid_kernel::update_status_datom(task_entity, braid_kernel::TaskStatus::InProgress, tx_id);
    let tx = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Observed,
        rationale: format!("MCP: claim task {id}"),
        causal_predecessors: vec![],
        datoms: vec![datom],
    };
    live.write_tx(&tx)?;

    // Get title for confirmation message
    let title = braid_kernel::task_summary(live.store(), task_entity)
        .map(|t| t.title.clone())
        .unwrap_or_else(|| id.to_string());

    Ok(json!({
        "content": [{"type": "text", "text": format!("claimed: {id} \"{title}\"")}],
    }))
}

/// `braid_task_close` — Close a completed task.
fn tool_task_close(live: &mut LiveStore, args: &JsonValue) -> Result<JsonValue, BraidError> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BraidError::Parse("missing required parameter: id".into()))?;
    let reason = args
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("completed");

    let agent = AgentId::from_name("braid:mcp");

    let task_entity = braid_kernel::find_task_by_id(live.store(), id)
        .ok_or_else(|| BraidError::Parse(format!("task not found: {id}")))?;

    let tx_id = crate::commands::write::next_tx_id(live.store(), agent);
    let datoms = braid_kernel::close_task_datoms(task_entity, reason, tx_id);
    let tx = TxFile {
        tx_id,
        agent,
        provenance: ProvenanceType::Observed,
        rationale: format!("MCP: close task {id} — {reason}"),
        causal_predecessors: vec![],
        datoms,
    };
    live.write_tx(&tx)?;

    let title = braid_kernel::task_summary(live.store(), task_entity)
        .map(|t| t.title.clone())
        .unwrap_or_else(|| id.to_string());

    Ok(json!({
        "content": [{"type": "text", "text": format!("closed: {id} \"{title}\"")}],
    }))
}

/// `braid_task_create` — Create a new task.
fn tool_task_create(live: &mut LiveStore, args: &JsonValue) -> Result<JsonValue, BraidError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BraidError::Parse("missing required parameter: title".into()))?;
    let priority = args.get("priority").and_then(|v| v.as_i64()).unwrap_or(2);
    let task_type_str = args
        .get("task_type")
        .and_then(|v| v.as_str())
        .unwrap_or("task");

    let description = args.get("description").and_then(|v| v.as_str());

    let traces_to: Vec<String> = args
        .get("traces_to")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let labels: Vec<String> = args
        .get("labels")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

    let create_path = live.layout().root.clone();
    let output = crate::commands::task::create(crate::commands::task::CreateArgs {
        path: create_path.as_path(),
        title,
        description,
        priority,
        task_type: task_type_str,
        agent: "braid:mcp",
        traces_to: &traces_to,
        labels: &labels,
        force,
        pre_opened: None, // MCP has its own shared LiveStore
    })?;

    // Render agent-mode output as MCP text content
    use crate::output::OutputMode;
    let text = output.render(OutputMode::Agent);

    Ok(json!({
        "content": [{"type": "text", "text": text}],
    }))
}

// ---------------------------------------------------------------------------
// Value parsing (reused from transact command)
// ---------------------------------------------------------------------------

/// Parse a string into a DatomValue (integer, float, boolean, keyword, or string).
pub(crate) fn parse_value(s: &str) -> DatomValue {
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
/// The server is stateful: it keeps a `LiveStore` open for the lifetime
/// of the process. The store is kept in memory with write-through to disk.
/// External changes (from CLI commands) are detected via stat() on the
/// txns/ directory and applied incrementally (LIVESTORE-6).
///
/// # Store Strategy (ADR-STORE-011, INV-STORE-020)
///
/// One LiveStore, created at startup. Read tools use `live.store()` (O(1)
/// pointer dereference). Write tools use `live.write_tx()` (disk + in-memory).
/// Before each request, `refresh_if_needed()` picks up external CLI writes
/// via O(1) stat() fast-path. This replaces the old strategy of reloading
/// the entire store from disk on every tool call.
///
/// Protocol: newline-delimited JSON-RPC (one JSON object per line).
pub fn serve(path: &Path) -> Result<(), BraidError> {
    let mut live = LiveStore::open(path)?;

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

        // LIVESTORE-6: Detect and apply external transactions (from CLI)
        // before dispatching. O(1) stat() fast-path when nothing changed.
        let _ = live.refresh_if_needed();

        let response = match method {
            "initialize" => handle_initialize(&id, &params, &mut live),
            "initialized" => {
                // Client acknowledgement — no response needed.
                if is_notification {
                    continue;
                }
                jsonrpc_ok(&id, json!({}))
            }
            "tools/list" => handle_tools_list(&id),
            "tools/call" => handle_tools_call(&id, &params, &mut live),
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
pub(crate) fn write_response(writer: &mut impl Write, response: &JsonValue) {
    let bytes = serde_json::to_vec(response).expect("JSON serialization cannot fail");
    // Write as a single line followed by newline.
    let _ = writer.write_all(&bytes);
    let _ = writer.write_all(b"\n");
    let _ = writer.flush();
}

/// Handle `initialize` — return server info and capabilities.
///
/// INV-INTERFACE-008: The instructions field provides basin activation —
/// a ~100 token orientation that anchors the agent's reasoning trajectory
/// before any tool calls. Uses live store metrics when available.
pub(crate) fn handle_initialize(
    id: &JsonValue,
    _params: &JsonValue,
    live: &mut LiveStore,
) -> JsonValue {
    // Build dynamic instructions from store state.
    let instructions = {
        let store = live.store();
        let datoms = store.len();
        let entities = store.entity_count();
        let telemetry = braid_kernel::guidance::telemetry_from_store(store);
        let m = braid_kernel::compute_methodology_score(&telemetry);
        format!(
            "Braid: append-only datom store (CRDT merge, content-addressed). \
             {} datoms, {} entities, M(t)={:.2}. \
             Workflow: braid_status (orient) → braid_task_ready (pick) → \
             braid_task_go (claim) → work → braid_observe (capture) → \
             braid_harvest (persist). Use spec-language: reference INV/ADR IDs.",
            datoms, entities, m.score,
        )
    };

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
                "instructions": instructions,
            },
        }),
    )
}

/// Handle `tools/list` — return tool definitions.
pub(crate) fn handle_tools_list(id: &JsonValue) -> JsonValue {
    jsonrpc_ok(id, tool_definitions())
}

/// Handle `tools/call` — dispatch to the appropriate tool.
///
/// INV-GUIDANCE-001: Every tool response includes an M(t) guidance footer.
/// This is the MCP equivalent of the CLI's `try_build_footer` — ensuring
/// methodology adherence signals are continuous, not optional.
pub(crate) fn handle_tools_call(
    id: &JsonValue,
    params: &JsonValue,
    live: &mut LiveStore,
) -> JsonValue {
    let name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => return jsonrpc_error(id, INVALID_PARAMS, "missing 'name' in tools/call params"),
    };

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match call_tool(live, name, &arguments) {
        Ok(mut result) => {
            // INV-GUIDANCE-001: Append M(t) footer to every successful response.
            append_guidance_footer(&mut result, live);
            jsonrpc_ok(id, result)
        }
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

/// Append the guidance footer (M(t) + next action) to an MCP tool response.
///
/// Modifies the last text content block in the response by appending the
/// footer string. Best-effort: if store load fails, no footer is appended
/// (graceful degradation per ADR-INTERFACE-010).
///
/// INV-INTERFACE-010 anti-drift injection: when M(t) < 0.5 (drift signal
/// active), an additional anti-drift warning is prepended before the normal
/// M(t) footer to redirect the agent back to methodology.
pub(crate) fn append_guidance_footer(result: &mut JsonValue, live: &LiveStore) {
    let store = live.store();

    // Compute M(t) to check for drift signal.
    let telemetry = braid_kernel::guidance::telemetry_from_store(store);
    let methodology = braid_kernel::guidance::compute_methodology_score(&telemetry);

    let footer = braid_kernel::guidance::build_command_footer(store, None);
    if footer.is_empty() {
        return;
    }

    // Build the combined footer: anti-drift warning (if needed) + normal footer.
    let combined = if methodology.drift_signal {
        // INV-INTERFACE-010: Anti-drift injection when M(t) < 0.5.
        let anti_drift = format!(
            "\u{26a0} Methodology drift (M(t)={:.2}). Before continuing: braid bilateral --verbose",
            methodology.score
        );
        format!("{anti_drift}\n{footer}")
    } else {
        footer
    };

    // Append footer to the last text content block.
    if let Some(content) = result.get_mut("content").and_then(|c| c.as_array_mut()) {
        if let Some(last) = content.last_mut() {
            if let Some(text) = last.get_mut("text") {
                if let Some(s) = text.as_str() {
                    *text = JsonValue::String(format!("{s}\n\n{combined}"));
                }
            }
        }
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
            11,
            "INV-INTERFACE-003: must expose expected number of tools (7 core + 4 task)"
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
            "braid_guidance",
            "braid_task_ready",
            "braid_task_go",
            "braid_task_close",
            "braid_task_create",
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
        let mut live = LiveStore::create(dir.path()).unwrap();

        // Tools that require no arguments
        let no_arg_tools = ["braid_status"];
        for tool_name in &no_arg_tools {
            let result = call_tool(&mut live, tool_name, &json!({}));
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
        let result = call_tool(&mut live, "braid_query", &json!({}));
        assert!(result.is_ok(), "braid_query should not error");

        // braid_write
        let result = call_tool(
            &mut live,
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
            &mut live,
            "braid_harvest",
            &json!({ "task": "integration test" }),
        );
        assert!(result.is_ok(), "braid_harvest should not error");

        // braid_seed
        let result = call_tool(&mut live, "braid_seed", &json!({ "task": "continue work" }));
        assert!(result.is_ok(), "braid_seed should not error");

        // braid_guidance
        let result = call_tool(&mut live, "braid_guidance", &json!({}));
        assert!(result.is_ok(), "braid_guidance should not error");
        let response = result.unwrap();
        let content = response["content"].as_array().expect("content must exist");
        assert!(!content.is_empty(), "braid_guidance must produce content");
        assert_ne!(
            response.get("isError").and_then(|v| v.as_bool()),
            Some(true),
            "braid_guidance should not be an error"
        );
    }

    /// INV-INTERFACE-010: Unknown tool returns isError=true.
    #[test]
    fn unknown_tool_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut live = LiveStore::create(dir.path()).unwrap();

        let result = call_tool(&mut live, "nonexistent_tool", &json!({}));
        assert!(result.is_ok()); // Should return Ok with isError in content
        let response = result.unwrap();
        assert_eq!(
            response["isError"].as_bool(),
            Some(true),
            "Unknown tool must set isError=true"
        );
    }

    /// INV-INTERFACE-003: braid_guidance returns M(t), sub-metrics, actions.
    #[test]
    fn guidance_tool_returns_methodology_dashboard() {
        let dir = tempfile::tempdir().unwrap();
        let mut live = LiveStore::create(dir.path()).unwrap();

        let result = call_tool(&mut live, "braid_guidance", &json!({}));
        assert!(
            result.is_ok(),
            "braid_guidance should succeed on fresh store"
        );
        let response = result.unwrap();
        let text = response["content"][0]["text"].as_str().unwrap();

        // Must contain M(t) headline
        assert!(
            text.contains("methodology: M(t)="),
            "guidance must show M(t) score. Got: {text}"
        );
        // Must contain sub-metrics
        assert!(
            text.contains("M(t) sub-metrics:"),
            "guidance must show sub-metric breakdown. Got: {text}"
        );
        assert!(
            text.contains("transact_frequency"),
            "guidance must show transact_frequency. Got: {text}"
        );
        // Must contain F(S)
        assert!(
            text.contains("fitness: F(S)="),
            "guidance must show fitness score. Got: {text}"
        );
        // Must contain actions section
        assert!(
            text.contains("actions:"),
            "guidance must show actions section. Got: {text}"
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
        let mut live = LiveStore::create(dir.path()).unwrap();

        // Get initial status
        let initial = call_tool(&mut live, "braid_status", &json!({})).unwrap();
        let initial_text = initial["content"][0]["text"].as_str().unwrap();

        // Write a datom
        let _tx = call_tool(
            &mut live,
            "braid_write",
            &json!({
                "entity": ":test/mcp-equiv",
                "attribute": ":db/doc",
                "value": "MCP equivalence test"
            }),
        )
        .unwrap();

        // Get updated status
        let updated = call_tool(&mut live, "braid_status", &json!({})).unwrap();
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
        // Create a temp store for the test
        let dir = tempfile::tempdir().unwrap();
        let mut live = LiveStore::create(dir.path()).unwrap();
        let response = handle_initialize(&id, &json!({}), &mut live);
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        let result = &response["result"];
        assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
        assert!(result["capabilities"].is_object());
        assert!(result["serverInfo"].is_object());
        assert_eq!(result["serverInfo"]["name"], SERVER_NAME);
        // Task 2: instructions field must be present with orientation
        let instructions = result["serverInfo"]["instructions"].as_str();
        assert!(
            instructions.is_some(),
            "serverInfo.instructions must be present"
        );
        assert!(
            instructions.unwrap().contains("Braid"),
            "instructions must contain orientation"
        );
    }

    /// tools/list response contains all tool definitions.
    #[test]
    fn tools_list_response() {
        let id = json!(2);
        let response = handle_tools_list(&id);
        assert_eq!(response["jsonrpc"], "2.0");
        let tools = response["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 11, "7 core + 4 task tools");
    }

    // -----------------------------------------------------------------------
    // Datalog parameter tests (INV-QUERY-002, INV-INTERFACE-010)
    // -----------------------------------------------------------------------

    /// INV-INTERFACE-010: braid_query tool schema includes the datalog parameter.
    #[test]
    fn query_tool_has_datalog_parameter() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();
        let query_tool = tools
            .iter()
            .find(|t| t["name"] == "braid_query")
            .expect("braid_query tool must exist");

        let props = &query_tool["inputSchema"]["properties"];
        assert!(
            props.get("datalog").is_some(),
            "braid_query must have a 'datalog' property in its schema"
        );
        assert_eq!(
            props["datalog"]["type"].as_str(),
            Some("string"),
            "datalog parameter must be type: string"
        );
        // entity and attribute should still be present as fallback
        assert!(props.get("entity").is_some(), "entity parameter preserved");
        assert!(
            props.get("attribute").is_some(),
            "attribute parameter preserved"
        );
    }

    /// INV-QUERY-002: Datalog query evaluates against a fresh store.
    #[test]
    fn datalog_query_returns_results() {
        let dir = tempfile::tempdir().unwrap();
        let mut live = LiveStore::create(dir.path()).unwrap();

        let result = call_tool(
            &mut live,
            "braid_query",
            &json!({
                "datalog": "[:find ?e ?v :where [?e :db/ident ?v]]"
            }),
        );

        assert!(result.is_ok(), "Datalog query should not error");
        let response = result.unwrap();
        let text = response["content"][0]["text"]
            .as_str()
            .expect("response must have text content");
        // Genesis store has axiomatic :db/ident datoms — strict assertion.
        // Match "\n0 result(s)" (not bare "0 result(s)") to avoid false positives
        // from counts like "20 result(s)" whose trailing digit contains "0".
        assert!(
            text.contains("result(s)"),
            "output must contain result count, got: {text}"
        );
        assert!(
            !text.contains("\n0 result(s)"),
            "genesis store should have :db/ident datoms, got: {text}"
        );
    }

    /// INV-INTERFACE-010: Datalog takes priority over entity/attribute when both provided.
    #[test]
    fn datalog_takes_priority_over_entity_attribute() {
        let dir = tempfile::tempdir().unwrap();
        let mut live = LiveStore::create(dir.path()).unwrap();

        // Provide both datalog and entity — datalog should win
        let result = call_tool(
            &mut live,
            "braid_query",
            &json!({
                "datalog": "[:find ?e ?v :where [?e :db/ident ?v]]",
                "entity": ":nonexistent/entity"
            }),
        );

        assert!(result.is_ok(), "Datalog path should execute");
        let response = result.unwrap();
        let text = response["content"][0]["text"]
            .as_str()
            .expect("response must have text");
        // If entity filter were active, we'd get 0 results for :nonexistent/entity.
        // Datalog ignores it and returns real results — strict assertion.
        // Match "\n0 result(s)" (not bare "0 result(s)") to avoid false positives
        // from counts like "20 result(s)" whose trailing digit contains "0".
        assert!(
            !text.contains("\n0 result(s)"),
            "datalog should override entity filter and return results, got: {text}"
        );
    }

    /// INV-INTERFACE-010: Invalid Datalog syntax returns an error.
    #[test]
    fn invalid_datalog_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut live = LiveStore::create(dir.path()).unwrap();

        let result = call_tool(
            &mut live,
            "braid_query",
            &json!({
                "datalog": "not valid datalog"
            }),
        );

        // The error propagates as Err from call_tool
        assert!(result.is_err(), "Invalid Datalog must return an error");
    }

    /// INV-QUERY-002: Scalar Datalog query works via MCP.
    #[test]
    fn datalog_scalar_query() {
        let dir = tempfile::tempdir().unwrap();
        let mut live = LiveStore::create(dir.path()).unwrap();

        let result = call_tool(
            &mut live,
            "braid_query",
            &json!({
                "datalog": "[:find ?doc . :where [:db/ident :db/doc ?doc]]"
            }),
        );

        assert!(result.is_ok(), "Scalar Datalog query should succeed");
        let response = result.unwrap();
        let text = response["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("Attribute"),
            "scalar result for :db/ident's :db/doc should contain 'Attribute'"
        );
    }

    /// INV-INTERFACE-010: Anti-drift injection appears when M(t) < 0.5.
    ///
    /// On a freshly-initialized store with no harvests and minimal activity,
    /// M(t) will be below 0.5 (drift signal active). The guidance footer
    /// should include the anti-drift warning message.
    #[test]
    fn anti_drift_injection_on_low_methodology() {
        let dir = tempfile::tempdir().unwrap();
        let mut live = LiveStore::create(dir.path()).unwrap();

        // Get status — on a fresh store, M(t) should be low enough to
        // trigger the drift signal (< 0.5) because there's no harvest.
        // However, genesis creates a store with harvest_is_recent potentially
        // clamping M(t). We need a store state where drift_signal fires.
        //
        // Strategy: create several transactions without ever harvesting,
        // pushing past the harvest_is_recent threshold (>= 10 txns).
        for i in 0..12 {
            let _ = call_tool(
                &mut live,
                "braid_write",
                &json!({
                    "entity": format!(":test/drift-{i}"),
                    "attribute": ":db/doc",
                    "value": format!("padding transaction {i}")
                }),
            )
            .unwrap();
        }

        // Now check M(t) directly to confirm drift signal.
        let telemetry = braid_kernel::guidance::telemetry_from_store(live.store());
        let methodology = braid_kernel::guidance::compute_methodology_score(&telemetry);

        // If M(t) >= 0.5, the A3 floor clamp is active; skip this test
        // rather than produce a false failure. The invariant is tested
        // by the condition below.
        if !methodology.drift_signal {
            // M(t) is above threshold — anti-drift won't fire. This can
            // happen if the store's initial harvest counts as recent.
            // The test is still valid: verify no spurious anti-drift message.
            let result = call_tool(&mut live, "braid_status", &json!({})).unwrap();
            let mut result_with_footer = result;
            append_guidance_footer(&mut result_with_footer, &live);
            let text = result_with_footer["content"][0]["text"].as_str().unwrap();
            assert!(
                !text.contains("Methodology drift"),
                "Anti-drift message should NOT appear when M(t) >= 0.5"
            );
            return;
        }

        // M(t) < 0.5 confirmed — anti-drift injection should fire.
        let result = call_tool(&mut live, "braid_status", &json!({})).unwrap();
        let mut result_with_footer = result;
        append_guidance_footer(&mut result_with_footer, &live);
        let text = result_with_footer["content"][0]["text"].as_str().unwrap();

        assert!(
            text.contains("Methodology drift"),
            "Anti-drift message should appear when M(t) < 0.5, got: {text}"
        );
        assert!(
            text.contains("braid bilateral --verbose"),
            "Anti-drift message should suggest bilateral command, got: {text}"
        );
    }

    /// Fallback: entity/attribute filter still works when no datalog parameter.
    #[test]
    fn entity_attribute_filter_still_works() {
        let dir = tempfile::tempdir().unwrap();
        let mut live = LiveStore::create(dir.path()).unwrap();

        // Write a datom first
        let _ = call_tool(
            &mut live,
            "braid_write",
            &json!({
                "entity": ":test/datalog-fallback",
                "attribute": ":db/doc",
                "value": "fallback test"
            }),
        )
        .unwrap();

        // Query using entity filter (no datalog)
        let result = call_tool(
            &mut live,
            "braid_query",
            &json!({
                "attribute": ":db/doc"
            }),
        );

        assert!(result.is_ok(), "entity/attribute filter should still work");
        let response = result.unwrap();
        let text = response["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("datom(s)"),
            "fallback path must produce datom count"
        );
    }
}
