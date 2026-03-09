# §9. INTERFACE — Build Plan

> **Spec reference**: [spec/14-interface.md](../spec/14-interface.md) — read FIRST
> **Stage 0 elements**: INV-INTERFACE-001–003, 008–010 (6 INV), ADR-INTERFACE-001–003, NEG-INTERFACE-003–004
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
├── mcp.rs                      ← MCP server (6 tools, rmcp-based, persistent process)
├── persistence.rs              ← Layout ↔ kernel Store bridge
└── claude_md.rs                ← Dynamic CLAUDE.md file generation
```

### Public API Surface (binary crate — CLI + MCP)

The interface layer has no kernel types. It wraps kernel calls with:
1. IO (file read/write, Layout persistence, stdout)
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
  Persistent server process; store loaded once at initialization, held for session lifetime.
- INV-INTERFACE-003: Exactly six tools at Stage 0. Fixed-size at compile time.
  `const MCP_TOOLS: [MCPTool; 6]` — adding/removing a tool is a compile error unless the
  array size changes. The `tools/list` response is generated from these same definitions.
- INV-INTERFACE-008: MCP tool description quality — every tool description satisfies the
  quality predicate `Q(D) = navigative(D) ∧ has_example(D) ∧ |D| ≤ 100 tokens ∧ has_semantic_types(D)`.
  Descriptions are compile-time constants; quality is verified via tests.
- NEG-INTERFACE-003: No harvest warning suppression — the interface must never suppress or
  delay a harvest warning triggered by Q(t) threshold.

**State box** (internal design):
- JSON-RPC 2.0 over stdio, transported by `rmcp` crate (ADR-INTERFACE-004).
- MCP lifecycle: `initialize` → `initialized` → tool calls → `shutdown`.
  The rmcp crate handles the 3-phase initialization handshake and JSON-RPC framing.
  Braid implements tool handler functions annotated with rmcp's `#[tool]` macro.
- Store loaded once at `initialize` from layout directory, held via `ArcSwap<Store>` for session lifetime.
  Reads load the current snapshot (lock-free); writes swap in a new Store atomically.
- Each tool call: load Store snapshot from ArcSwap → call kernel function via `&Store` → format response → if write: swap in new Store → return.
- Tool descriptions are the optimized prompts from guide/00-architecture.md §0.5.

**Clear box** (implementation):
```rust
use arc_swap::ArcSwap;
use rmcp::{ServerHandler, tool, McpServer};

/// MCP server — persistent process, store loaded once at initialization.
/// rmcp handles: stdio JSON-RPC, initialize/initialized handshake, tools/list, shutdown.
/// Braid handles: tool handlers, session state, notification dispatch.
///
/// Store held via ArcSwap (Datomic connection model): Store values are immutable (C1),
/// the pointer swaps atomically after transact/harvest. Reads are lock-free via
/// hazard pointers. In-flight queries see a consistent snapshot.
pub struct BraidMcpServer {
    store: ArcSwap<Store>,                  // Loaded once at init; swapped on write ops
    session_state: SessionState,
    notification_queue: Vec<Signal>,
}

impl BraidMcpServer {
    /// Called during MCP initialization. Loads store from layout directory once.
    pub fn new(store_path: &Path) -> Result<Self, PersistenceError> {
        let store = ArcSwap::from_pointee(load_store(store_path)?);
        Ok(Self {
            store,
            session_state: SessionState::default(),
            notification_queue: Vec::new(),
        })
    }
}

#[tool]
impl BraidMcpServer {
    /// Assert or retract datoms. Use when you have facts to record.
    /// Loads current store snapshot, transacts, swaps in new store.
    #[tool(name = "braid_transact")]
    async fn handle_transact(&self, params: TransactParams) -> ToolResult {
        let current = self.store.load();                        // Lock-free snapshot
        let (new_store, receipt) = kernel::transact(
            &current, params.datoms, params.provenance
        )?;
        self.store.store(Arc::new(new_store));                  // Atomic swap
        Ok(receipt.into())
    }

    /// Run Datalog query or graph algorithm. Use to find facts, dependencies, metrics.
    #[tool(name = "braid_query")]
    async fn handle_query(&self, params: QueryParams) -> ToolResult {
        let store = self.store.load();                          // Lock-free snapshot
        kernel::query(&store, &params.query, params.mode)
    }

    /// Store summary: datom count, frontier, drift. Use for orientation.
    #[tool(name = "braid_status")]
    async fn handle_status(&self) -> ToolResult {
        let store = self.store.load();
        kernel::status(&store)
    }

    /// Extract session knowledge into datoms. Use near session end.
    /// Like transact, swaps in new store after harvest commits.
    #[tool(name = "braid_harvest")]
    async fn handle_harvest(&self, params: HarvestParams) -> ToolResult {
        let current = self.store.load();
        let (new_store, result) = kernel::harvest(&current, params.auto)?;
        self.store.store(Arc::new(new_store));
        Ok(result.into())
    }

    /// Assemble session context from store. Use at session start.
    #[tool(name = "braid_seed")]
    async fn handle_seed(&self, params: SeedParams) -> ToolResult {
        let store = self.store.load();
        kernel::seed(&store, &params.task)
    }

    /// Methodology guidance: M(t), R(t), drift, CLAUDE.md generation.
    #[tool(name = "braid_guidance")]
    async fn handle_guidance(&self, params: GuidanceParams) -> ToolResult {
        let store = self.store.load();
        kernel::guidance(&store, params.generate_claude_md)
    }
}

// Entry point: `braid serve` launches the MCP server.
// The CLI and MCP both dispatch to the same kernel functions (INV-INTERFACE-010).
// CLI is the universal interface; MCP is the optimized machine-to-machine path.
pub async fn serve_mcp(store_path: &Path) -> Result<(), Box<dyn Error>> {
    let server = BraidMcpServer::new(store_path)?;
    // rmcp handles: stdio transport, initialize/initialized handshake,
    // tools/list generation from #[tool] annotations, shutdown
    McpServer::serve(server).await
}

/// Six MCP tools (INV-INTERFACE-003) — Stage 0 surface.
/// Aligned with spec/14-interface.md MCPTool enum (R4.1c naming reconciliation).
pub enum MCPTool {
    Transact,   // meta: side effect — assert/retract datoms
    Query,      // moderate: 50–300 tokens — Datalog query
    Status,     // cheap: ≤50 tokens — store summary + M(t) + drift
    Harvest,    // meta: side effect — extract session knowledge
    Seed,       // expensive: 300+ tokens — session initialization
    Guidance,   // cheap: ≤50 tokens — methodology steering + R(t)
}

/// Compile-time tool count enforcement (INV-INTERFACE-003).
/// With rmcp, the #[tool] macro annotations also enforce this at compile time.
const MCP_TOOLS: [MCPTool; 6] = [
    MCPTool::Transact, MCPTool::Query, MCPTool::Status,
    MCPTool::Harvest, MCPTool::Seed, MCPTool::Guidance,
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

### MCP Tool Response Format

Every MCP tool response is a JSON-RPC 2.0 result with a structured payload. The response
always includes both the structured data (for programmatic consumption) and an `agent_summary`
field (pre-formatted agent-mode text with guidance footer). The rmcp crate handles JSON-RPC
framing; Braid produces the `result` object.

**Standard response envelope**:
```json
{
  "jsonrpc": "2.0",
  "id": "<request-id>",
  "result": {
    "<domain-fields>": "...",
    "agent_summary": "[LABEL] Context.\nContent.\n---\n↳ Guidance footer (INV ref)."
  }
}
```

**Per-tool result fields**:

| Tool | Domain Fields | Example |
|------|--------------|---------|
| `braid_transact` | `tx_id`, `datom_count`, `new_datoms`, `store_size` | See guide/11 §11.5 |
| `braid_query` | `bindings`, `stratum`, `count` | See guide/11 §11.5 |
| `braid_status` | `datom_count`, `entity_count`, `frontier`, `drift` | — |
| `braid_harvest` | `candidates`, `committed`, `drift_before`, `drift_after` | — |
| `braid_seed` | `orientation`, `constraints`, `state`, `warnings`, `directive` | — |
| `braid_guidance` | `methodology_score`, `routing`, `drift_signals` | — |

**Error response** (follows INV-INTERFACE-009 four-part protocol):
```json
{
  "jsonrpc": "2.0",
  "id": "<request-id>",
  "error": {
    "code": -32000,
    "message": "attribute `:spec/bogus` not in schema",
    "data": {
      "what": "Unknown attribute",
      "why": "Not in genesis or any schema transaction",
      "recovery": {"action": "RetryWith", "hint": "Define attribute first, then retry"},
      "spec_ref": "INV-SCHEMA-005"
    }
  }
}
```

The `data.recovery` field is always present (NEG-INTERFACE-004). The `RecoveryAction` enum
ensures totality at the type level — every `KernelError` variant maps to a recovery hint.

### Persistence Bridge

**Black box**: Translate between kernel's in-memory `Store` and the on-disk Layout directory.
Called once at startup (CLI command or MCP initialization), not per-call.

**Clear box**:
```rust
pub fn load_store(path: &Path) -> Result<Store, PersistenceError> {
    let layout = Layout::open(path)?;
    // Load all transaction files from txns/ → construct Store
    // Rebuild indexes from .cache/ or from txns/ if cache missing
    // Reconstruct schema from store datoms
    layout.load_all()
}

pub fn save_tx(layout: &Layout, tx: &TxFile) -> Result<PathBuf, PersistenceError> {
    // Write content-addressed transaction file to txns/
    // Idempotent: if file already exists (same content), no-op
    layout.write_tx(tx)
}

// Usage contexts:
// - CLI: load_store() at command start, save_store() at command end (per-invocation)
// - MCP: load_store() once at MCP_INIT, save_store() after transact/harvest (session-scoped)
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

    // INV-INTERFACE-002: MCP wrapper — all six tools accessible, store loaded once via ArcSwap
    fn inv_interface_002() {
        let server = BraidMcpServer::new(temp_store_path())?;
        // Store loaded once at init via ArcSwap, not per-call
        let store_snapshot = server.store.load();
        assert!(Arc::strong_count(&store_snapshot) >= 1);
        for tool in &MCP_TOOLS {
            let request = jsonrpc_request(tool.name(), serde_json::json!({}));
            let result = server.dispatch(&request);
            // Every registered tool must be dispatchable (no "Method not found" for known tools)
            prop_assert!(
                result.is_ok() || !result.as_ref().unwrap_err().message.contains("Method not found"),
                "MCP tool {} not registered: {:?}", tool_name, result
            );
        }
    }

    // INV-INTERFACE-010: CLI/MCP semantic equivalence — same kernel results
    fn inv_interface_010() {
        let store = test_store_with_datoms();
        // Verify transact via MCP and CLI produce identical store state
        let datoms = arb_datoms();
        let (mcp_store, mcp_receipt) = kernel::transact(&store, datoms.clone(), Observed)?;
        let (cli_store, cli_receipt) = kernel::transact(&store, datoms, Observed)?;
        prop_assert_eq!(mcp_receipt.datom_count, cli_receipt.datom_count);
        prop_assert_eq!(mcp_store.len(), cli_store.len());
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

// Integration test: MCP lifecycle + round-trip
#[test]
fn mcp_lifecycle_and_transact_round_trip() {
    // 1. Create BraidMcpServer (simulates MCP_INIT — store loaded once)
    // 2. Verify store is loaded and held via Arc
    // 3. Send transact tool call → verify datoms stored
    // 4. Send query tool call → verify datoms retrievable (same store ref)
    // 5. Simulate shutdown → verify session state persisted
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
- [ ] MCP server: rmcp-based persistent process, 6 tool handlers via `#[tool]` macro
- [ ] MCP lifecycle: initialize handshake, tools/list, shutdown (handled by rmcp)
- [ ] MCP store loading: once at initialization via `ArcSwap<Store>`, not per-call
- [ ] MCP store mutation: ArcSwap atomic swap on transact/harvest (Datomic connection model)
- [ ] MCP snapshot isolation: in-flight reads see consistent pre-swap Store
- [ ] MCP tool descriptions match guide/00-architecture.md §0.5
- [ ] INV-INTERFACE-010: CLI/MCP parity — both dispatch to same kernel functions
- [ ] INV-INTERFACE-008: `ToolDescription` struct with quality predicate Q(D) enforced by test
- [ ] INV-INTERFACE-009: `RecoveryHint` + `RecoveryAction` enum, total `KernelError::recovery()`
- [ ] NEG-INTERFACE-004: proptest verifying all error variants produce four-part messages
- [ ] Persistence: Layout load_all/write_tx round-trip
- [ ] Error messages follow four-part protocol (what/why/recovery/spec_ref)
- [ ] Guidance footer injected into every agent-mode response
- [ ] NEG-INTERFACE-003: harvest warnings never suppressed
- [ ] Integration: full CLI round-trip (transact → query → status → harvest → seed)
- [ ] Integration: MCP lifecycle test (init → tool calls → shutdown)
- [ ] Integration: MCP round-trip (transact → query via same store ref)

---
