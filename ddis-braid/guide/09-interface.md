# §9. INTERFACE — Build Plan

> **Spec reference**: [spec/14-interface.md](../spec/14-interface.md) — read FIRST
> **Stage 0 elements**: INV-INTERFACE-001–003, 008–009 (5 INV), ADR-INTERFACE-001–003, NEG-INTERFACE-003–004
> **Dependencies**: All prior namespaces (INTERFACE is the outermost layer)
> **Traces to SEED.md**: §8 (Interface Principles — five-layer model, guidance injection),
>   §10 (Stage 0 deliverables — CLI and MCP tool surface), §11 (Design Rationale — output budget)
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
├── mcp.rs                      ← MCP JSON-RPC server (6 tools)
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
  Display structure maps the spec's 5 semantic components: context=headline,
  content=entities+signals, footer=guidance+pointers.
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
- INV-INTERFACE-002: MCP wrapper — all six tools accessible via MCP JSON-RPC.
- INV-INTERFACE-003: Exactly six tools at Stage 0. Fixed-size at compile time.
  `const MCP_TOOLS: [MCPTool; 6]` — adding/removing a tool is a compile error unless the
  array size changes.
- INV-INTERFACE-008: MCP tool description quality — every tool description satisfies the
  quality predicate `Q(D) = navigative(D) ∧ has_example(D) ∧ |D| ≤ 100 tokens ∧ has_semantic_types(D)`.
  Descriptions are compile-time constants; quality is verified via tests.
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
                _ => Err(jsonrpc_error(-32601, "Method not found")),
            };
            write_jsonrpc_response(request.id, result).await;
        }
    }
}

/// Compile-time tool count enforcement (INV-INTERFACE-003).
const MCP_TOOLS: [&str; 6] = [
    "braid_transact", "braid_query", "braid_status",
    "braid_harvest", "braid_seed", "braid_guidance",
];

/// Tool description quality enforcement (INV-INTERFACE-008).
/// Each description is a compile-time constant satisfying Q(D).
pub struct ToolDescription {
    pub name: &'static str,
    pub purpose: &'static str,      // navigative, ≤20 tokens
    pub inputs: &'static [TypedParam],
    pub output: &'static str,
    pub example: &'static str,      // ≤30 tokens
}
// Test: assert all descriptions satisfy Q(D) — navigative, has_example, ≤100 tokens, semantic types
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

### Error Protocol (INV-INTERFACE-009, NEG-INTERFACE-004)

Every error follows the four-part protocol defined in INV-INTERFACE-009:

```
M(e) = what_failed(e) ⊕ why(e) ⊕ R(e) ⊕ spec_ref(e)
```

INV-INTERFACE-009 requires a **total recovery function** `R: E → RecoveryHint` — every error
variant maps to an executable recovery action. NEG-INTERFACE-004 is the safety property:
`□(error_emitted → recovery_hint_present)`. No error may be surfaced without all four parts.

The `RecoveryAction` enum guarantees totality at the type level:

```rust
pub struct RecoveryHint {
    pub action: RecoveryAction,     // Enum, not String
    pub spec_ref: &'static str,     // e.g., "INV-STORE-001"
}

pub enum RecoveryAction {
    RetryWith(String),              // Retry with corrected input
    CheckPrecondition(String),      // Verify a precondition
    UseAlternative(String),         // Use alternative approach
    EscalateToHuman(String),        // Only for truly unrecoverable
}

// Every KernelError variant must map to a RecoveryHint — total, no None arms
impl KernelError {
    pub fn recovery(&self) -> RecoveryHint;
}
```

The interface layer wraps kernel errors via this protocol:

```rust
fn format_error(e: KernelError) -> String {
    let hint = e.recovery();
    match e {
        KernelError::TxValidation(v) => format!(
            "Tx error: {} — {} — {} — See: {}",
            v.what, v.why, hint.action, hint.spec_ref
        ),
        // ... other error types — all arms produce four-part messages
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

    // INV-INTERFACE-003: Exactly 6 MCP tools
    fn inv_interface_003() {
        assert_eq!(MCP_TOOLS.len(), 6);
    }

    // INV-INTERFACE-008: Tool descriptions satisfy quality predicate Q(D)
    fn inv_interface_008() {
        for desc in &MCP_TOOL_DESCRIPTIONS {
            prop_assert!(desc.total_tokens() <= 100);
            prop_assert!(!desc.example.is_empty());         // has_example
            prop_assert!(!desc.inputs.is_empty() || desc.name == "braid_status");
            // navigative: no imperative "you must" / "do not" without demonstration
            prop_assert!(!desc.purpose.contains("you must") && !desc.purpose.contains("do not"));
        }
    }

    // INV-INTERFACE-002: MCP wrapper — all six tools accessible via MCP JSON-RPC
    fn inv_interface_002() {
        let server = MCPServer::new(":memory:");
        for tool_name in &MCP_TOOLS {
            let request = jsonrpc_request(tool_name, serde_json::json!({}));
            let result = server.dispatch(&request);
            // Every registered tool must be dispatchable (no "Method not found" for known tools)
            prop_assert!(
                result.is_ok() || !result.as_ref().unwrap_err().message.contains("Method not found"),
                "MCP tool {} not registered: {:?}", tool_name, result
            );
        }
    }

    // INV-INTERFACE-009: Every error variant produces a four-part message
    fn inv_interface_009(err in arb_kernel_error()) {
        let hint = err.recovery();
        let msg = format_error(err);
        prop_assert!(msg.contains("—"));            // four-part separator
        prop_assert!(!hint.spec_ref.is_empty());     // spec_ref present
        // NEG-INTERFACE-004: recovery hint always present
        prop_assert!(matches!(hint.action,
            RecoveryAction::RetryWith(_) | RecoveryAction::CheckPrecondition(_) |
            RecoveryAction::UseAlternative(_) | RecoveryAction::EscalateToHuman(_)));
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
- [ ] MCP server: JSON-RPC over stdio, 6 tool handlers
- [ ] MCP tool descriptions match guide/00-architecture.md §0.5
- [ ] INV-INTERFACE-008: `ToolDescription` struct with quality predicate Q(D) enforced by test
- [ ] INV-INTERFACE-009: `RecoveryHint` + `RecoveryAction` enum, total `KernelError::recovery()`
- [ ] NEG-INTERFACE-004: proptest verifying all error variants produce four-part messages
- [ ] Persistence: redb load/save round-trip
- [ ] Error messages follow four-part protocol (what/why/recovery/spec_ref)
- [ ] Guidance footer injected into every agent-mode response
- [ ] NEG-INTERFACE-003: harvest warnings never suppressed
- [ ] Integration: full CLI round-trip (transact → query → status → harvest → seed)
- [ ] Integration: MCP round-trip (JSON-RPC request → response)

---
