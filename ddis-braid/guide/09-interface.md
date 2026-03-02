# §9. INTERFACE — Build Plan

> **Spec reference**: [spec/14-interface.md](../spec/14-interface.md) — read FIRST
> **Stage 0 elements**: INV-INTERFACE-001–003 (3 INV), ADR-INTERFACE-001–003, NEG-INTERFACE-003
> **Dependencies**: All prior namespaces (INTERFACE is the outermost layer)
> **Cognitive mode**: Prompt-engineering — LLM activation, output algebra, token budgets

---

## §9.1 Module Structure

```
braid/src/                      ← Binary crate (not kernel — this is the IO boundary)
├── main.rs                     ← clap CLI entry point
├── commands/
│   ├── mod.rs                  ← Command dispatch
│   ├── transact.rs             ← braid transact
│   ├── query.rs                ← braid query
│   ├── status.rs               ← braid status
│   ├── harvest.rs              ← braid harvest
│   ├── seed.rs                 ← braid seed
│   ├── guidance.rs             ← braid guidance
│   └── entity.rs               ← braid entity, braid history
├── output.rs                   ← OutputMode dispatch (json/agent/human) + footer injection
├── mcp.rs                      ← MCP JSON-RPC server (9 tools)
├── persistence.rs              ← redb ↔ kernel Store bridge
└── claude_md.rs                ← Dynamic CLAUDE.md file generation
```

### Public API Surface (binary crate — CLI + MCP)

The interface layer has no kernel types. It wraps kernel calls with:
1. IO (file read/write, redb persistence, stdout)
2. Output formatting (json/agent/human)
3. Guidance footer injection (every response)
4. MCP JSON-RPC protocol handling

---

## §9.2 Three-Box Decomposition

### Output Mode Dispatch (INV-INTERFACE-001)

**Black box** (contract):
- INV-INTERFACE-001: Three output modes — json (structured), agent (LLM-optimized), human (terminal).
  Every command supports `--format {json,agent,human}`.
- Default: `agent` when `$BRAID_AGENT=1`, `human` otherwise.

**State box** (internal design):
- `OutputMode` enum dispatches formatting.
- Agent mode: `context (≤50 tok) + content (≤200 tok) + footer (≤50 tok)`.
- JSON mode: full structured data with semantic keys.
- Human mode: tables, colors (via termcolor), abbreviated.

**Clear box** (implementation):
```rust
pub enum OutputMode { Json, Agent, Human }

pub struct ToolResponse {
    pub structured: serde_json::Value,  // Full data
    pub agent_context: String,          // ≤50 tokens
    pub agent_content: String,          // ≤200 tokens
    pub human_display: String,          // Formatted table
}

pub fn format_output(response: &ToolResponse, mode: OutputMode, footer: &GuidanceFooter) -> String {
    match mode {
        OutputMode::Json => serde_json::to_string_pretty(&response.structured).unwrap(),
        OutputMode::Agent => format!(
            "{}\n{}\n---\n↳ {}",
            response.agent_context,
            response.agent_content,
            footer.text
        ),
        OutputMode::Human => response.human_display.clone(),
    }
}
```

### MCP Server (INV-INTERFACE-002, INV-INTERFACE-003)

**Black box** (contract):
- INV-INTERFACE-002: MCP wrapper — all nine tools accessible via MCP JSON-RPC.
- INV-INTERFACE-003: Exactly nine tools. Fixed-size at compile time.
  `const MCP_TOOLS: [MCPTool; 9]` — adding/removing a tool is a compile error unless the
  array size changes.
- NEG-INTERFACE-003: No harvest warning suppression — the interface must never suppress or
  delay a harvest warning triggered by Q(t) threshold.

**State box** (internal design):
- JSON-RPC 2.0 over stdio.
- Each tool: validate input → load store from redb → call kernel function →
  format response → return JSON-RPC result.
- Tool descriptions are the optimized prompts from guide/00-architecture.md §0.5.

**Clear box** (implementation):
```rust
pub struct MCPServer {
    store_path: PathBuf,
}

impl MCPServer {
    pub async fn serve(&self) {
        // Read JSON-RPC from stdin, dispatch to tool handler, write result to stdout
        loop {
            let request = read_jsonrpc_request().await;
            let result = match request.method.as_str() {
                "braid_transact" => self.handle_transact(request.params).await,
                "braid_query"    => self.handle_query(request.params).await,
                "braid_status"   => self.handle_status(request.params).await,
                "braid_harvest"  => self.handle_harvest(request.params).await,
                "braid_seed"     => self.handle_seed(request.params).await,
                "braid_guidance" => self.handle_guidance(request.params).await,
                "braid_entity"   => self.handle_entity(request.params).await,
                "braid_history"  => self.handle_history(request.params).await,
                "braid_claude_md"=> self.handle_claude_md(request.params).await,
                _ => Err(jsonrpc_error(-32601, "Method not found")),
            };
            write_jsonrpc_response(request.id, result).await;
        }
    }
}

/// Compile-time tool count enforcement.
const MCP_TOOLS: [&str; 9] = [
    "braid_transact", "braid_query", "braid_status",
    "braid_harvest", "braid_seed", "braid_guidance",
    "braid_entity", "braid_history", "braid_claude_md",
];
```

### Persistence Bridge

**Black box**: Translate between kernel's in-memory `Store` and redb on-disk tables.

**Clear box**:
```rust
pub fn load_store(path: &Path) -> Result<Store, PersistenceError> {
    let db = redb::Database::open(path)?;
    // Read all datoms from "datoms" table → construct Store
    // Read frontier from "frontier" table
    // Reconstruct schema from store datoms
}

pub fn save_store(store: &Store, path: &Path) -> Result<(), PersistenceError> {
    let db = redb::Database::create(path)?;
    // Write new datoms to "datoms" table
    // Write frontier to "frontier" table
    // Indexes maintained in redb tables for query performance
}
```

---

## §9.3 LLM-Facing Outputs

### Command → Output Mode Mapping

Every command produces a `ToolResponse` with all three representations. The `--format` flag
(or `$BRAID_AGENT` env var) selects which representation to emit.

| Command | Agent Context (≤50 tok) | Agent Content (≤200 tok) |
|---------|------------------------|-------------------------|
| transact | `[STORE] Transacted {N} datoms in tx {id}` | Summary of what changed |
| query | `[QUERY] {N} results (Stratum {S}, {mode})` | Result bindings |
| status | `[STATUS] Store: {N} datoms, {M} entities` | Frontier, schema stats |
| harvest | `[HARVEST] {N} candidates ({H} high confidence)` | Candidate list |
| seed | `[SEED] Session context assembled` | Five-part seed |
| guidance | `[GUIDANCE] Drift: {assessment}` | Recommendation |
| entity | `[ENTITY] {eid}: {attr_count} attributes` | Attribute values |
| history | `[HISTORY] {attr}: {change_count} changes` | Value timeline |

### Error Protocol

Every error follows the four-part structure (guide/00-architecture.md §0.6):

```
{what_failed} — {why} — {recovery_action} — {spec_ref}
```

The interface layer wraps kernel errors with recovery hints:

```rust
fn format_error(e: KernelError) -> String {
    match e {
        KernelError::TxValidation(v) => format!(
            "Tx error: {} — {} — {} — See: {}",
            v.what, v.why, v.recovery, v.spec_ref
        ),
        // ... other error types
    }
}
```

---

## §9.4 Verification

### Key Properties

```rust
proptest! {
    // INV-INTERFACE-001: All three output modes produce non-empty output
    fn inv_interface_001(response in arb_tool_response(), mode in arb_output_mode()) {
        let footer = GuidanceFooter::default();
        let output = format_output(&response, mode, &footer);
        prop_assert!(!output.is_empty());
    }

    // INV-INTERFACE-003: Exactly 9 MCP tools
    fn inv_interface_003() {
        assert_eq!(MCP_TOOLS.len(), 9);
    }
}

// Integration test: MCP round-trip
#[test]
fn mcp_transact_round_trip() {
    // Send JSON-RPC transact request → receive response → verify datoms stored
}

// Integration test: agent mode includes footer
#[test]
fn agent_mode_has_footer() {
    let response = execute_command("braid status --format agent");
    assert!(response.contains("↳"));
}
```

---

## §9.5 Implementation Checklist

- [ ] clap CLI with all command subcommands
- [ ] `OutputMode` enum with json/agent/human dispatch
- [ ] Agent-mode output: context + content + footer
- [ ] `$BRAID_AGENT` env var detection for default mode
- [ ] MCP server: JSON-RPC over stdio, 9 tool handlers
- [ ] MCP tool descriptions match guide/00-architecture.md §0.5
- [ ] Persistence: redb load/save round-trip
- [ ] Error messages follow four-part protocol
- [ ] Guidance footer injected into every agent-mode response
- [ ] NEG-INTERFACE-003: harvest warnings never suppressed
- [ ] Integration: full CLI round-trip (transact → query → status → harvest → seed)
- [ ] Integration: MCP round-trip (JSON-RPC request → response)

---
