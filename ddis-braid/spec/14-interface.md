> **Namespace**: INTERFACE | **Wave**: 3 (Intelligence) | **Stage**: 0
> **Shared definitions**: [00-preamble.md](00-preamble.md) (conventions, verification tags, constraints)

## §14. INTERFACE — CLI/MCP/TUI Layers

> **Purpose**: The interface layers are the graded information channels through which
> agents, humans, and machines interact with the store. Each layer serves a different
> consumer at a different frequency, all backed by the same datom store.
>
> **Traces to**: SEED.md §8 (Interface Principles), ADRS IB-001–003, IB-008–009,
> SR-011, AA-003

### §14.1 Level 0: Algebraic Specification

The interface is a **five-layer graded information channel**:

```
Layer 0 (Ambient):    CLAUDE.md — ~80 tokens, k*-exempt, always present
Layer 1 (CLI):        Rust binary — primary agent interface, budget-aware
Layer 2 (MCP):        Thin wrapper — machine-to-machine, six tools
Layer 3 (Guidance):   Comonadic — spec-language, injected in every response
Layer 4 (TUI):        Subscription-driven — human monitoring dashboard
Layer 4.5 (Statusline): Bridge — persistent low-bandwidth agent↔human signal
```

**Information flow**:
```
Store → CLI (agent reads/writes)
Store → MCP → Agent (machine-to-machine)
Store → TUI (human reads)
TUI → Store (human injects signals via IB-009)
Statusline → Session State → CLI (budget measurement)
```

**Laws**:
- **L1 (Layer independence)**: Each layer can operate independently of other layers
- **L2 (Store as sole truth)**: All layers read from and write to the same datom store. No layer-local state that isn't a projection of the store.
- **L3 (Budget awareness)**: Layers 1–3 respect the attention budget. Layer 0 is k*-exempt (always present). Layer 4/4.5 is unconstrained (human, not agent).

### §14.2 Level 1: State Machine Specification

**State**: `Σ_interface = (cli: CLIState, mcp: MCPState, tui: TUIState, statusline: StatuslineState)`

**Transitions**:

```
CLI_COMMAND(Σ, command, args, budget) → (output, Σ') where:
  PRE:  command ∈ known_commands
  POST: output = execute(command, args, store)
  POST: |output| ≤ budget (truncated per precedence)
  POST: output includes guidance footer
  POST: store updated if command is META type

MCP_INIT(Σ) → Σ' where:
  PRE:  Σ.mcp.phase = Uninitialized
  POST: store loaded from layout directory once, held via ArcSwap for session lifetime
  POST: Σ'.mcp.phase = Initialized
  POST: responds with server capabilities (tools, notifications)
  NOTE: MCP protocol 3-phase: client sends `initialize` →
        server responds with capabilities → client sends `initialized` notification.
        Transport layer (rmcp crate) handles framing; Braid implements the handler.

MCP_TOOLS_LIST(Σ) → tool_list where:
  PRE:  Σ.mcp.phase = Initialized
  POST: returns descriptions for all 6 Stage 0 tools
  POST: each description satisfies Q(D) (INV-INTERFACE-008)

MCP_CALL(Σ, tool_name, params) → (result, Σ') where:
  PRE:  Σ.mcp.phase = Initialized
  PRE:  tool_name ∈ {braid_transact, braid_query, braid_status,
                      braid_harvest, braid_seed, braid_guidance}
  POST: loads current Store snapshot from ArcSwap (lock-free)
  POST: dispatches to kernel function via &Store reference (no subprocess, no reload)
  POST: if tool is transact or harvest: swaps in new Store via ArcSwap
  POST: reads session state, computes Q(t), passes budget to kernel
  POST: appends pending notifications to MCP notification queue
  POST: updates session state
  POST: checks harvest warning thresholds

MCP_NOTIFY(Σ) → Σ' where:
  PRE:  Σ.mcp.notification_queue is non-empty
  POST: server-to-client notifications sent for pending signals
  POST: Σ'.mcp.notification_queue = []
  NOTE: Notifications are piggybacked on tool responses (MCP allows server→client
        notifications; signals queued between calls are flushed with next response)

MCP_SHUTDOWN(Σ) → Σ' where:
  PRE:  Σ.mcp.phase = Initialized
  POST: Σ'.mcp.phase = Shutdown
  POST: store reference dropped, session state persisted
  NOTE: Triggered by client `shutdown` request or stdio EOF

TUI_UPDATE(Σ, subscriptions) → display where:
  POST: continuous projection via SUBSCRIBE
  POST: NOT k*-constrained (human interface)
  POST: delegation changes and conflicts above threshold trigger notification

SIGNAL_INJECT(Σ, signal_from_human) → Σ' where:
  POST: signal recorded as datom (high authority — human source)
  POST: queued in MCP notification queue for agent's next tool response
  POST: entity type `:signal/*` with provenance `:observed`

STATUSLINE_TICK(Σ, context_data) → Σ' where:
  POST: writes session state to .ddis/session/context.json
  POST: fields: used_percentage, input_tokens, remaining_tokens,
        k_eff, quality_adjusted, output_budget, timestamp, session_id
  POST: zero cost to agent context (side effect only)
```

### §14.3 Level 2: Implementation Contract

```rust
/// CLI output modes (IB-002)
pub enum OutputMode {
    Json,        // JSON — machine-parseable (structured output)
    Agent,       // 100–300 tokens, headline + entities + signals + guidance + pointers
    Human,       // TTY — full formatting, color, tables
}

/// MCP server — persistent process, thin wrapper calling kernel for all computation.
/// Transport (stdio JSON-RPC, initialize handshake, tools/list) handled by rmcp crate.
/// Braid implements: tool handlers, notification dispatch, session state.
///
/// Store is held via ArcSwap<Store> (Datomic connection model): the Store value is
/// immutable (C1), but the pointer swaps atomically after each transact(). In-flight
/// queries see a consistent snapshot (the Store they loaded before the swap).
pub struct MCPServer {
    pub store: ArcSwap<Store>,              // Loaded once at MCP_INIT; swapped on transact
    pub session_state: SessionState,
    pub notification_queue: Vec<Signal>,
    pub phase: MCPPhase,                    // Uninitialized → Initialized → Shutdown
}

pub enum MCPPhase {
    Uninitialized,
    Initialized,
    Shutdown,
}

/// Six MCP tools (IB-003) — Stage 0 surface
pub enum MCPTool {
    Transact,   // meta: side effect — assert/retract datoms
    Query,      // moderate: 50–300 tokens — Datalog query
    Status,     // cheap: ≤50 tokens — store summary + M(t) + drift
    Harvest,    // meta: side effect — extract session knowledge
    Seed,       // expensive: 300+ tokens — session initialization
    Guidance,   // cheap: ≤50 tokens — methodology steering + R(t)
}

/// Session state file (SR-011)
#[derive(Serialize, Deserialize)]
pub struct SessionState {
    pub used_percentage: f64,
    pub input_tokens: u64,
    pub remaining_tokens: u64,
    pub k_eff: f64,
    pub quality_adjusted: f64,
    pub output_budget: u32,
    pub timestamp: u64,
    pub session_id: String,
}

/// TUI — subscription-driven push projection
pub struct TUIState {
    pub subscriptions: Vec<Subscription>,
    pub active_display: DisplayState,
}
```

### §14.4 Invariants

### INV-INTERFACE-001: Three CLI Output Modes

**Traces to**: ADRS IB-002
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
The CLI produces output in exactly one of three modes per invocation:
Json (machine-parseable), Agent (budget-constrained), Human (TTY-formatted).
Mode selection is explicit (flag) or inferred from terminal context.

#### Level 1 (State Invariant)
Every CLI_COMMAND invocation selects exactly one mode. The mode determines
formatting, token budget, and content selection.

**Falsification**: A CLI command produces mixed-mode output (e.g., JSON with
TTY escape codes, or agent-mode output without budget constraint).

---

### INV-INTERFACE-002: MCP as Thin Wrapper

**Traces to**: ADRS IB-003
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)
The MCP server performs no domain computation. All domain logic is delegated to
the kernel via direct function calls. MCP adds: protocol lifecycle (initialize/
tools_list/shutdown), session state management, budget adjustment, tool descriptions,
notification queuing.

`∀ mcp_call: result = kernel_dispatch(mcp_call.params) + mcp_metadata`

#### Level 1 (State Invariant)
The MCP server is a persistent process that loads the store once at initialization
(MCP_INIT) and holds it via `ArcSwap<Store>` for the session lifetime. Reads load
the current Store snapshot (lock-free); writes (transact, harvest) swap in a new
Store atomically. In-flight queries see a consistent point-in-time snapshot — the
Store value they loaded before any concurrent swap (snapshot isolation). Every
MCP_CALL transition dispatches to a kernel function via direct Rust function call.
The MCP server formats the response — but does not duplicate any kernel logic. The
rmcp crate handles transport (stdio JSON-RPC framing, initialize handshake
negotiation).

**Falsification**: The MCP server implements query parsing, resolution logic, or
any other domain computation that exists in the kernel crate. OR: the MCP server
reloads the store from disk on every tool call instead of holding a session-scoped
reference. OR: the MCP server uses `&mut Store` or interior mutability instead of
the ArcSwap model (which would violate C1 value-type semantics).

---

### INV-INTERFACE-003: Six MCP Tools

**Traces to**: ADRS IB-003
**Verification**: `V:PROP`, `V:TYPE`
**Stage**: 0

#### Level 0 (Algebraic Law)
The MCP server exposes exactly six tools at Stage 0:
`{braid_transact, braid_query, braid_status, braid_harvest, braid_seed, braid_guidance}`

The tool set partitions by cost class:
- **Meta** (side effects): `braid_transact`, `braid_harvest`
- **Read** (moderate budget): `braid_query`, `braid_seed`
- **Cheap** (≤50 tokens): `braid_status`, `braid_guidance`

Stage 2+ may add tools (e.g., `braid_branch`, `braid_signal`) via spec update.

#### Level 1 (State Invariant)
The tool set is fixed per stage. Adding tools requires a spec update with
a new invariant version. Each tool maps to a specific CLI command.
Tools removed from Stage 0 (Associate, Branch, Signal) are available
via `braid query` Datalog expressions or at later stages.

#### Level 2 (Implementation Contract)
```rust
// Type-level guarantee: exactly 6 tools at Stage 0
const MCP_TOOLS: [MCPTool; 6] = [
    MCPTool::Transact, MCPTool::Query, MCPTool::Status,
    MCPTool::Harvest, MCPTool::Seed, MCPTool::Guidance,
];

// tools/list handler returns these 6 tools with Q(D)-satisfying descriptions.
// The rmcp crate's #[tool] macro generates the tools/list response from
// annotated handler functions; the fixed-size array enforces the count at
// compile time.
```

**Falsification**: The MCP server exposes a tool not in the defined set of six,
or fewer than six tools are registered. OR: a `tools/list` request returns a
tool set that differs from `MCP_TOOLS`.

---

### INV-INTERFACE-004: Statusline Zero-Cost to Agent

**Traces to**: ADRS IB-001 (Layer 4.5), SR-011
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
The statusline hook produces side effects (writes session state file) but
consumes zero tokens from the agent's context window.

#### Level 1 (State Invariant)
STATUSLINE_TICK writes to `.ddis/session/context.json` as an external side effect.
The statusline output is consumed by the human display and the CLI budget system,
never by the agent's context.

**Falsification**: Statusline output appears in the agent's context window,
consuming attention budget.

---

### INV-INTERFACE-005: TUI Subscription Liveness

**Traces to**: ADRS IB-008
**Verification**: `V:PROP`
**Stage**: 4

#### Level 0 (Algebraic Law)
Delegation changes and conflicts above severity threshold trigger TUI notification
within one refresh cycle.

`∀ event e where severity(e) ≥ threshold: ◇ displayed_in_tui(e)`

#### Level 1 (State Invariant)
The TUI subscribes to store changes. When a matching event occurs (delegation change,
conflict above threshold), the TUI display updates within the subscription's
refresh interval.

**Falsification**: A High-severity conflict is recorded in the store but the TUI
does not display a notification.

---

### INV-INTERFACE-006: Human Signal Injection

**Traces to**: ADRS IB-009
**Verification**: `V:PROP`
**Stage**: 3

#### Level 0 (Algebraic Law)
A human can inject signals from the TUI. The signal is:
1. Recorded as a datom with human provenance (`:observed`, axiomatically high authority)
2. Queued in the MCP notification queue
3. Delivered to the agent in the next tool response

#### Level 1 (State Invariant)
SIGNAL_INJECT always produces both a datom and a notification queue entry.
The agent receives the signal at the next MCP_CALL.

**Falsification**: A human injects a signal from TUI and the agent never receives it.

---

### INV-INTERFACE-007: Proactive Harvest Warning

**Traces to**: ADRS IB-012
**Verification**: `V:PROP`
**Stage**: 1

#### Level 0 (Algebraic Law)
```
Q(t) < 0.15 (~75% consumed) ⟹ every response includes harvest warning
Q(t) < 0.05 (~85% consumed) ⟹ CLI emits ONLY the harvest imperative
```

#### Level 1 (State Invariant)
When k*_eff drops below thresholds, the response format changes:
below 0.15, a harvest warning is appended; below 0.05, only the harvest
imperative is emitted, suppressing all other output.

**Falsification**: k*_eff = 0.03 and the CLI still produces full output without
a harvest warning.

---

### INV-INTERFACE-008: MCP Tool Description Quality

**Traces to**: ADRS IB-002, IB-011
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)

A tool description `D` is a mapping from tool identity to an activation prompt.
Quality is a predicate `Q(D)`:

```
Q(D) = navigative(D) ∧ has_example(D) ∧ |D| ≤ 100 tokens ∧ has_semantic_types(D)
```

Where:
- `navigative(D)`: description activates deep reasoning (pattern matching, analogy),
  not surface compliance ("you must", "do not")
- `has_example(D)`: at least one micro-example (≤30 tokens) demonstrating usage
- `|D| ≤ 100 tokens`: total description fits within ambient attention budget
- `has_semantic_types(D)`: inputs and outputs carry semantic type annotations,
  not raw `String`

Information density: `ρ(D) = unique_concepts(D) / |D|` — maximized by navigative
structure and demonstration density.

#### Level 1 (State Invariant)

Every MCP tool served via `MCP_TOOLS_LIST` includes a description satisfying
`Q(D)`. Descriptions are compile-time constants (generated by rmcp `#[tool]`
annotations or static `const` array), so `Q(D)` is verified at development
time via tests, not at runtime.

#### Level 2 (Implementation Contract)

```rust
pub struct ToolDescription {
    pub name: &'static str,
    pub purpose: &'static str,      // 1 sentence, navigative, ≤20 tokens
    pub inputs: &'static [TypedParam], // semantic types, not raw String
    pub output: &'static str,        // semantic description
    pub example: &'static str,       // 1 micro-example, ≤30 tokens
}

const_assert!(MCP_TOOLS.iter().all(|t| t.total_tokens() <= 100));
```

**Falsification**: A tool description exceeds 100 tokens, OR uses imperative
language ("you must", "do not") without a demonstration, OR omits semantic
type annotations for inputs/outputs, OR contains no micro-example.

---

### INV-INTERFACE-009: Error Recovery Protocol Completeness

**Traces to**: ADRS IB-011
**Verification**: `V:PROP`, `V:TYPE`
**Stage**: 0

#### Level 0 (Algebraic Law)

The error algebra `E` has a total recovery function `R: E → RecoveryHint`.
For every error variant `e ∈ E`, `R(e)` is defined and executable.

The error message function `M: E → String` produces a four-part string:

```
M(e) = what_failed(e) ⊕ why(e) ⊕ R(e) ⊕ spec_ref(e)
```

Totality: `∀ e ∈ E: R(e) ≠ ⊥ ∧ executable(R(e))`

#### Level 1 (State Invariant)

Every error transition in the state machine produces a message via `M(e)`.
The recovery action `R(e)` is a valid state transition back to a non-error
state. No error message is emitted without all four parts.

#### Level 2 (Implementation Contract)

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

// Every KernelError variant must map to a RecoveryHint
impl KernelError {
    pub fn recovery(&self) -> RecoveryHint; // Total — no match arm returns None
}
```

**Falsification**: Any error variant `e` where `R(e)` is undefined, OR an error
message missing any of the four parts (what/why/recovery/spec_ref), OR a recovery
action that leads to another error without its own recovery path.

---

### INV-INTERFACE-010: CLI and MCP Semantic Equivalence

**Traces to**: ADRS IB-003, ADR-INTERFACE-004
**Verification**: `V:PROP`
**Stage**: 0

#### Level 0 (Algebraic Law)

For every MCP tool `T` and its corresponding CLI command `C`, the kernel function
invoked is identical. The CLI and MCP are two serialization frontends to a single
dispatch surface (the kernel). Semantic equivalence:

```
∀ tool T, ∀ params P:
  T(P) ≡ C(deserialize_cli(serialize_mcp(P)))  (modulo output format)
```

Both paths call the same kernel function with the same arguments. Output differs
only in serialization format (JSON-RPC response vs. CLI stdout).

#### Level 1 (State Invariant)

The CLI command handlers and MCP tool handlers both invoke kernel functions from
`braid-kernel`. Neither CLI nor MCP contains domain logic — both are thin adapters
that parse input, dispatch to a kernel function, and format output. The kernel is
the single source of semantic truth. Adding or modifying behavior in one path
without the corresponding change in the other is a parity violation.

This is structurally enforced at Stage 0 by keeping the kernel function set small
(6 operations) and by having both CLI and MCP call the same functions:
`kernel::transact`, `kernel::query`, `kernel::status`, `kernel::harvest`,
`kernel::seed`, `kernel::guidance`.

#### Level 2 (Implementation Contract)

```rust
// Both CLI and MCP call these same kernel functions:
// CLI (commands/transact.rs):
//   let receipt = kernel::transact(&store, datoms, provenance)?;
// MCP (mcp.rs):
//   let receipt = kernel::transact(&self.store.load(), datoms, provenance)?;
//
// The kernel function is identical. Only the input parsing (clap vs JSON-RPC)
// and output formatting (OutputMode vs ToolResult) differ.

// Parity test: for every MCP tool, construct equivalent CLI args,
// run both, verify identical kernel-level results.
#[cfg(test)]
fn verify_cli_mcp_parity<P: Into<KernelParams>>(
    tool: MCPTool,
    params: P,
    store: &Store,
) {
    let mcp_result = mcp_dispatch(tool, params.into(), store);
    let cli_result = cli_dispatch(tool.cli_command(), params.into(), store);
    assert_eq!(mcp_result.kernel_output, cli_result.kernel_output);
}
```

**Falsification**: Any input where CLI and MCP produce semantically different
kernel-level results (different datoms stored, different query bindings returned,
different harvest candidates). Output formatting differences (JSON vs agent-mode
text) are NOT violations.

---

### §14.5 ADRs

### ADR-INTERFACE-001: Five Layers Plus Statusline Bridge

**Traces to**: ADRS IB-001
**Stage**: 0–4 (layers implemented across stages)

#### Problem
How many interface layers are needed, and what does each serve?

#### Decision
Five layers plus a Layer 4.5 statusline bridge:
- Layer 0 (Ambient): CLAUDE.md — always-present, ~80 tokens, most important
  ("agents fail to invoke tools 56% without ambient awareness")
- Layer 1 (CLI): Primary agent interface, budget-aware
- Layer 2 (MCP): Machine-to-machine, thin wrapper
- Layer 3 (Guidance): Comonadic, injected in responses
- Layer 4 (TUI): Human monitoring, subscription-driven
- Layer 4.5 (Statusline): Zero-cost bridge, writes session state

#### Formal Justification
Each layer serves a different consumer (agent/machine/human) at a different
frequency (always/per-command/continuous). The statusline bridge is the critical
innovation: it connects the human display (Layer 4) to the agent budget system
(Layer 1) with zero context cost to the agent.

---

### ADR-INTERFACE-002: Agent-Mode Demonstration Style

**Traces to**: ADRS IB-002
**Stage**: 0

#### Problem
How should agent-mode CLI output be structured?

#### Decision
Demonstration style: headline + entities (3–7) + signals (0–3) + guidance (1–3)
+ pointers (1–3). Total: 100–300 tokens. "Demonstration, not constraint list."

#### Formal Justification
Demonstration-style output activates the deep reasoning substrate of LLMs
(pattern matching, analogy, formal inference). Constraint-style output
("DO NOT do X, MUST do Y") activates the surface compliance substrate,
which produces brittle behavior under context pressure.

---

### ADR-INTERFACE-003: Store-Mediated Trajectory Management

**Traces to**: ADRS IB-010
**Stage**: 0

#### Problem
How should agent work sessions be managed across conversation boundaries?

#### Decision
Store-mediated: `ddis harvest` extracts durable facts, `ddis seed` generates
carry-over. Agent lifecycle: SEED → work 20–30 turns → HARVEST → reset → GOTO SEED.

Seed output follows a five-part template (ADR-SEED-004):
1. Orientation (project identity, phase, recent history)
2. Constraints (relevant INVs, ADRs, negative cases)
3. State (datoms, artifacts, frontier, changes)
4. Warnings (drift, questions, uncertainties)
5. Directive (next task, acceptance criteria, corrections)

#### Formal Justification
The store is the sole truth (FD-012). Trajectory management through the store
means conversation boundaries become knowledge extraction points, not knowledge
loss points. The five-part template provides structure for the seed while
keeping it within budget.

---

### ADR-INTERFACE-004: Library-Mode Persistent MCP Server via rmcp

**Traces to**: ADRS IB-003, FD-001
**Stage**: 0

#### Problem
Should the MCP server invoke CLI commands as subprocesses or call kernel
functions directly via Rust function calls? How should MCP protocol transport
(JSON-RPC framing, initialize handshake, tools/list) be handled?

#### Options
A) **Subprocess model** — MCP server spawns `braid transact`, `braid query`, etc.
   as child processes. Full process isolation, each call is stateless.
B) **Library model** — MCP server holds a `&Store` reference and calls kernel
   functions directly. Shared address space, near-zero overhead.
C) **Library model + rmcp crate** — Same as (B) but delegates MCP protocol transport
   (stdio JSON-RPC, initialize/initialized handshake, tools/list, shutdown) to the
   `rmcp` crate. Braid implements only the tool handler functions.

#### Decision
**Option C.** Library model with rmcp for transport. C1 (append-only) and C4
(CRDT merge by set union) make the Store effectively immutable — process isolation
solves a problem that the CRDT architecture already solves. The subprocess model
would add ~50-100ms overhead per call and require serialization/deserialization
through CLI args and stdout, degrading agent experience. The rmcp crate provides
the MCP protocol machinery (3-phase initialization handshake, tools/list handler
generation, JSON-RPC framing over stdio, shutdown lifecycle) so Braid focuses on
domain logic.

**Store mutability model (Datomic connection pattern)**: The Store is immutable
(C1 value-type semantics). The MCP server holds an `ArcSwap<Store>` — reads load
the current snapshot (lock-free via hazard pointers), writes (`transact`,
`harvest`) swap in a new Store atomically. This is the direct Rust equivalent of
Datomic's connection model where the "db value" is immutable and the "connection"
holds a mutable pointer to the latest db value. Snapshot isolation falls out
naturally: a query that starts before a transact sees the pre-transact Store.

**Universality preservation**: The CLI remains the universal interface. Any agent
(Python, Go, JS, bash script) can call `braid transact`, `braid query`, etc. via
subprocess. The MCP server is an additional, optimized entry point — not a
replacement. Both CLI and MCP dispatch to the same kernel functions
(INV-INTERFACE-010). The system works without MCP; MCP is an optimization, not
a requirement.

MCP requires a persistent server process with a 3-phase initialization lifecycle:
1. Client sends `initialize` with `clientInfo` and `capabilities`
2. Server responds with `serverInfo` and `capabilities` (including tool list)
3. Client sends `initialized` notification (handshake complete, tool calls may begin)

The rmcp `#[tool]` macro generates the tools/list response from annotated Rust
functions, ensuring compile-time agreement between handler implementations and
tool descriptions.

#### Formal Justification
The Store is a G-Set CvRDT (INV-STORE-003). Reads via `&Store` are always safe
because the store never mutates existing datoms (C1). Writes go through
`transact()` which returns a new Store — the ArcSwap atomically replaces the
pointer while preserving C1: the Store value never mutates, only the reference
changes. The MCP server cannot corrupt the store because it never holds
`&mut Store`. Process isolation would add overhead without improving safety.

#### Consequences
- Single binary deployment (MCP server and kernel in one process)
- Kernel functions called directly: `kernel::query(&store, expr)`, etc.
- Error handling via Rust `Result<T, E>` types (not exit codes)
- Store loaded once at initialization (MCP_INIT), held via `ArcSwap<Store>`;
  swapped atomically on write operations (transact, harvest)
- Snapshot isolation: concurrent reads see a consistent Store snapshot
- CLI universality preserved: CLI and MCP both dispatch to kernel (INV-INTERFACE-010)
- rmcp handles: stdio JSON-RPC framing, initialize/initialized handshake, tools/list
  response generation, shutdown lifecycle
- Braid handles: 6 tool handler functions, session state, notification dispatch
- External dependency: `arc-swap` crate (one dependency, no transitive deps)

---

### ADR-INTERFACE-005: Configurable Heuristic Parameters with Progressive Disclosure

**Traces to**: ADRS IB-013, C3
**Stage**: 0

#### Problem
Stage 0 introduces multiple heuristic proxies (betweenness default=0.5, harvest
warn-at-turn=20, harvest-imperative-at-turn=40, cascade stub behavior). These values
will need tuning during real usage. How should they be exposed?

#### Options
A) **Hard-coded constants** — simplest, but requires code changes for operational tuning.
B) **Environment variables** — not portable across sessions, not stored in datoms.
C) **TOML/YAML config file** — external to the store, doesn't participate in merge/harvest.
D) **Datom-stored configuration with progressive disclosure CLI** — parameters are datoms
   in the store, exposed via `braid config` CLI command with smart defaults.

#### Decision
**Option D.** Configuration parameters are stored as datoms with attribute namespace
`:config/` (e.g., `:config/harvest.warn-turn`, `:config/guidance.betweenness-default`).
This means they participate in the store's append-only, content-addressed, mergeable
infrastructure. Smart defaults are compiled into the kernel; user overrides are
transacted as datoms that take precedence.

**Progressive disclosure**:
- **Casual user**: System works with no configuration. Defaults are chosen conservatively.
- **Standard user**: `braid config show` displays current effective values (defaults +
  overrides). `braid config set harvest.warn-turn 25` transacts an override.
- **Expert user**: `braid config show --all` shows all parameters including internal
  tuning knobs. Parameters are typed (integer, float, enum) with validation.

**Portability**: Since overrides are datoms, they survive harvest/seed cycles, merge
across stores, and are queryable via Datalog. Two agents merging stores merge their
configuration; conflicts are resolved by the per-attribute resolution mode (LWW by
default for config parameters — latest override wins).

**Ergonomic access**: The CLI `braid config` subcommand provides tab completion.
The MCP `braid_config` tool exposes the same interface. The dynamic CLAUDE.md
(INV-GUIDANCE-007) references current config values when they affect guidance behavior.

#### Formal Justification
C3 (schema-as-data) requires that configuration, like schema, is stored as datoms.
This is not merely convenient — it is structurally necessary. Configuration parameters
affect system behavior, which means they must be part of the auditable, queryable,
mergeable store to preserve traceability (C5). External configuration files would
create a second source of truth outside the store, violating C3 and fragmenting the
single-substrate property.

#### Consequences
- New attribute namespace `:config/` with ~15-20 parameters at Stage 0
- `braid config show|set|reset` CLI subcommand
- `braid_config` MCP tool
- Smart defaults compiled into kernel; overrides transacted as datoms
- Configuration conflicts resolved via LWW (latest override wins)
- No config files external to the store

---

### ADR-INTERFACE-006: Ten Protocol Primitives

**Traces to**: SEED §8, ADRS AA-006
**Stage**: 0

#### Problem
How many protocol primitives should the system expose? Too few and agents cannot
express necessary operations; too many and the interface becomes a complex API
that wastes attention budget on discovery.

#### Options
A) **Minimal (3-4 primitives)** — CRUD operations only: create, read, update, delete.
B) **Ten primitives** — a curated set covering store operations, association, assembly, branching, synchronization, and guidance.
C) **Extensible registry** — start with a small set and allow dynamic registration of new primitives.

#### Decision
**Option B.** Ten protocol primitives: TRANSACT, QUERY, ASSOCIATE, ASSEMBLE,
BRANCH, MERGE, SYNC-BARRIER, SIGNAL, SUBSCRIBE, GUIDANCE. This set is derived
from the algebraic operations needed to interact with the store, the bilateral
loop, and the agent coordination layer. Each primitive maps to a specific
algebraic operation on the datom set.

The ten primitives partition into three groups:
- **Core store** (4): TRANSACT (write), QUERY (read), ASSOCIATE (link), ASSEMBLE (compose)
- **Branching** (2): BRANCH (fork), MERGE (join)
- **Coordination** (4): SYNC-BARRIER (wait), SIGNAL (notify), SUBSCRIBE (listen), GUIDANCE (steer)

At Stage 0, six of these are exposed as MCP tools (INV-INTERFACE-003). The
remaining four (ASSOCIATE, BRANCH, SIGNAL, SUBSCRIBE) are available as Datalog
query patterns or deferred to later stages.

#### Formal Justification
A minimal set (Option A) conflates distinct semantic operations — TRANSACT and
ASSOCIATE are both "writes" but have fundamentally different purposes (asserting
facts vs. linking entities). Conflating them loses the semantic distinction that
enables guidance to steer agents toward the right operation. An extensible
registry (Option C) introduces discovery overhead that wastes attention budget.
The ten-primitive set is sufficient to express all operations in the protocol
(proved by coverage analysis of the transcript discussions) and small enough to
fit within ambient attention budget.

#### Consequences
- Ten primitives form the complete operation vocabulary for all stages
- Stage 0 exposes 6 as MCP tools; remaining 4 are queryable or deferred
- Each primitive has a defined attention cost class (CHEAP/MODERATE/EXPENSIVE/META)
- The primitive set is fixed — adding an 11th requires an ADR with justification

#### Falsification
A necessary operation exists that cannot be expressed as a composition of the
ten primitives, OR agents consistently need to compose 3+ primitives for common
tasks (indicating a missing atomic primitive), OR the ten primitives exceed
ambient attention budget when listed in tool descriptions.

---

### ADR-INTERFACE-007: Rust as Implementation Language

**Traces to**: SEED §4, ADRS FD-011
**Stage**: 0

#### Problem
What language should Braid be implemented in? The implementation language
determines the safety guarantees, performance characteristics, dependency
ecosystem, and deployment model.

#### Options
A) **Go** — the language of the existing ddis-cli (~62,500 LOC). Continuing in Go would allow incremental evolution.
B) **Rust** — a systems language with ownership-based memory safety, zero-cost abstractions, and strong type system.
C) **Python** — rapid prototyping, extensive ML/NLP ecosystem, but performance limitations.

#### Decision
**Option B.** Braid is implemented in Rust. The query engine targets a
purpose-built Rust binary as the final form. The user explicitly confirmed
the Rust approach ("I want the option a) approach" referring to Rust binary).

Key advantages:
- **Safety guarantees**: Ownership and lifetimes encode invariants at compile time (V:TYPE). The typestate patterns (§16.3) that enforce transaction lifecycle, entity ID construction, and store immutability are only possible in a language with Rust's type system.
- **Performance**: Zero-cost abstractions for index operations. The LIVE index (INV-STORE-012) and in-memory datom operations benefit from cache-friendly data layouts.
- **Ecosystem**: `blake3` for content-addressed storage, `arc-swap` for the Datomic connection model, `rmcp` for MCP protocol transport, `proptest` and `kani` for verification.

#### Formal Justification
The substrate divergence principle (LM-001) establishes that the existing Go CLI
represents a fundamental substrate divergence — the architectural model of the
Go CLI does not match the algebraic foundations described in SEED.md. Continuing
in Go (Option A) would mean fighting the substrate rather than building on it.
Python (Option C) lacks the performance characteristics needed for in-memory
store operations and cannot express the typestate patterns that enforce C1
(append-only) at compile time. Rust uniquely provides both the safety guarantees
and the performance characteristics the design requires.

#### Consequences
- Full rewrite rather than incremental evolution of the Go CLI
- Typestate patterns encode invariants at compile time (zero runtime cost)
- Kani bounded model checking available for critical properties
- `#![forbid(unsafe_code)]` enforced project-wide (§16.2 Gate 7)
- Single binary deployment (CLI + MCP server in one binary)
- Learning curve for contributors not familiar with Rust

#### Falsification
The Rust type system proves insufficient to encode a critical typestate pattern
(requiring runtime enforcement instead of compile-time), OR Rust ecosystem
libraries for embedded storage, MCP transport, or verification are unavailable
or inadequate, OR compilation times become a development bottleneck that
significantly slows the bilateral cycle.

---

### ADR-INTERFACE-008: Agent Cycle as Ten-Step Composition

**Traces to**: SEED §7, SEED §8, ADRS PO-011
**Stage**: 1

#### Problem
How does an agent compose the ten protocol primitives into a coherent work
session? Without a defined cycle, agents will use primitives ad-hoc, missing
critical steps (e.g., skipping GUIDANCE, forgetting to TRANSACT observations).

#### Options
A) **No prescribed cycle** — agents use primitives freely; guidance nudges them toward good patterns.
B) **Strict sequential pipeline** — enforce a fixed order of operations per turn.
C) **Ten-step composition with flexible ordering** — define a canonical cycle that composes primitives, with defined fallback paths for confusion or subtask discovery.

#### Decision
**Option C.** The agent cycle is a ten-step composition of protocol primitives:

1. **ASSOCIATE** — retrieve relevant entities from the store based on current context
2. **QUERY** — execute specific Datalog queries for precise information
3. **ASSEMBLE** with guidance+intentions — compose a working context from retrieved entities, active guidance, and current intentions
4. **GUIDANCE** lookahead=2 — query the guidance graph for recommended next actions with 2-step lookahead
5. **Agent policy evaluates** — the agent's internal reasoning applies to the assembled context
6a. **Action** — if the agent decides to act: TRANSACT (record the action and its results)
6b. **Confusion** — if the agent is uncertain: re-ASSOCIATE and re-ASSEMBLE with broader context, then retry
7. **Learned association** — if the agent discovers a new useful association: TRANSACT with `:inferred` provenance
8. **Subtask discovery** — if the agent discovers a subtask: TRANSACT an intention update
9. **Check incoming** — process MERGE results and incoming signals from other agents
10. **Repeat** — return to step 1

#### Formal Justification
No prescribed cycle (Option A) relies entirely on guidance nudges, which fail
when the agent is already in Basin B (the guidance itself is ignored). A strict
pipeline (Option B) cannot handle the non-linear nature of agent work — confusion
requires re-association, subtask discovery requires intention updates, incoming
signals require immediate processing. The ten-step composition (Option C) provides
structure (each step maps to a specific primitive) with flexibility (steps 6b, 7,
8 are conditional; step 9 is event-driven). The cycle is self-reinforcing: step 4
(GUIDANCE) steers toward the methodology, step 7 captures emergent knowledge, and
step 8 prevents scope creep by explicitly recording subtasks as intentions rather
than silently expanding the current task.

#### Consequences
- Every agent turn maps to a traversal of (a subset of) the ten steps
- The guidance footer (step 4) indicates which step the agent should be in
- Confusion (step 6b) triggers re-association rather than degraded output
- Learned associations (step 7) use `:inferred` provenance to distinguish agent-discovered knowledge from human-asserted knowledge
- The cycle integrates with multi-agent coordination via step 9 (MERGE/signals)

#### Falsification
Agents following the ten-step cycle produce worse outcomes than agents using
primitives ad-hoc, OR the cycle cannot accommodate a common agent workflow
pattern (requiring ad-hoc primitive use outside the cycle), OR the GUIDANCE
step (step 4) consistently fails to keep agents in Basin A.

---

### ADR-INTERFACE-009: Staged Alignment Strategy for Existing Codebase

**Traces to**: SEED §10, ADRS LM-015
**Stage**: 0

#### Problem
The existing Go CLI (~62,500 LOC) represents significant engineering investment.
How should Braid relate to this codebase? A full rewrite discards working code;
incremental migration risks inheriting architectural debt.

#### Options
A) **Full rewrite from scratch** — ignore the existing codebase entirely and build from the specification.
B) **Incremental migration** — gradually replace Go modules with Rust equivalents, maintaining backward compatibility.
C) **Staged alignment with four strategies** — categorize each existing module by its stability and correctness, then apply the appropriate strategy from a preference-ordered set.

#### Decision
**Option C.** Four alignment strategies in preference order:

1. **THIN WRAPPER** — the existing module has correct behavior but a different interface. Wrap it with an adapter layer. Lowest cost.
2. **SURGICAL EDIT** — the existing module has mostly correct behavior with specific divergences. Fix the divergences directly. Moderate cost.
3. **PARALLEL IMPLEMENTATION** — the existing module is partially correct but the divergences are deep enough that editing in-place is risky. Build alongside, migrate consumers, then remove the original. Higher cost.
4. **REWRITE** — the existing module is fundamentally incompatible with the specification. Replace entirely. Highest cost.

Priority matrix for module categorization:
- **stable + working** = optimize freely (THIN WRAPPER or SURGICAL EDIT)
- **stable + broken** = fix now (SURGICAL EDIT or PARALLEL IMPLEMENTATION)
- **changing-soon + working** = leave alone (defer alignment until after changes)
- **changing-soon + broken** = defer (neither fix nor align — wait for stabilization)

Governing principle: "Never rewrite what you can align incrementally."

#### Formal Justification
A full rewrite (Option A) discards validated behavior — the existing CLI has
620+ tests and 97/97 witnessed invariants. This is working code that has proven
the DDIS concepts in practice. Discarding it entirely wastes the empirical
validation. Incremental migration (Option B) risks inheriting architectural
decisions that conflict with the new specification (the substrate divergence
from LM-001). The staged approach (Option C) is precise: each module is
independently evaluated against the specification, and the alignment strategy
matches the effort to the divergence. Modules that already work correctly get
the lightest treatment; modules with fundamental incompatibilities get rewritten.
The preference ordering minimizes total effort while ensuring all modules
converge to specification compliance.

#### Consequences
- docs/audits/GAP_ANALYSIS.md categorizes every existing module as ALIGNED/DIVERGENT/EXTRA/BROKEN/MISSING
- Each module receives one of the four strategies based on the priority matrix
- The existing test suite is preserved and extended, not discarded
- Braid's Rust implementation may wrap some Go modules initially (THIN WRAPPER) before eventually replacing them
- No backward compatibility shims — aligned modules either work correctly or are replaced

#### Falsification
The staged approach takes longer than a full rewrite would have (measured by
time to equivalent functionality and test coverage), OR modules classified as
"stable + working" require deeper intervention than THIN WRAPPER or SURGICAL
EDIT (indicating the classification criteria are wrong), OR the priority matrix
fails to correctly predict the appropriate strategy for most modules.

---

### ADR-INTERFACE-010: Harvest Warning Turn-Count Proxy at Stage 0

**Traces to**: SEED §10 (staged roadmap), NEG-INTERFACE-003, ADR-HARVEST-007, ADRS IB-012
**Stage**: 0

#### Problem

NEG-INTERFACE-003 defines a safety property: `□ ¬(Q(t) < 0.15 ∧ response_without_harvest_warning)`.
When context quality Q(t) drops below 0.15 (critically low), harvest warnings must appear in
every response to prevent knowledge loss. This is a hard safety guarantee — no configuration,
flag, or output mode may suppress it.

At Stage 0, Q(t) is unavailable because the attention budget framework (BUDGET, Stage 1)
has not been implemented. Without Q(t), the safety property cannot be evaluated directly.
The question is how to preserve the safety guarantee without the formal trigger mechanism.

This is the same cross-stage dependency pattern as ADR-HARVEST-007, which resolves INV-HARVEST-005's
dependence on Q(t) for harvest urgency. The solutions should be consistent: if ADR-HARVEST-007
uses turn-count as a proxy for Q(t), then NEG-INTERFACE-003 should use the same proxy to
avoid contradictory heuristics operating on the same underlying signal.

#### Options
A) **Pull BUDGET into Stage 0** — implement the full Q(t) computation pipeline to enable
   the formal safety property trigger. This requires the attention budget framework
   (INV-BUDGET-001 through 007), the k*_eff estimation pipeline, and the Q(t) decay model.
   The same massive dependency chain that ADR-HARVEST-007 and ADR-GUIDANCE-008 avoid.
B) **Turn-count proxy matching ADR-HARVEST-007 thresholds** — replace Q(t) < 0.15 with
   turn >= 20 for warning and turn >= 40 for harvest-only-mode. This is conservative:
   empirically, Q(t) drops below 0.15 around turn 25-30 for typical sessions, so
   triggering at turn 20 warns earlier than Q(t) would. The safety property becomes:
   `□ ¬(turn ≥ 20 ∧ response_without_harvest_warning)`.
C) **Defer NEG-INTERFACE-003 to Stage 1** — no harvest warning safety guarantee at Stage 0.
   This loses the safety property entirely during the bootstrap phase, which is when
   sessions are longest (specification work) and context loss is most costly.
D) **Always show harvest warning** — include a harvest warning in every response from
   turn 1. This is trivially safe but noisy: agents receive warnings when context is
   fresh, which degrades trust in the warning system (cry-wolf effect). After enough
   false alarms, agents learn to ignore the warning, defeating its purpose.

#### Decision
**Option B.** The turn-count proxy preserves the safety property conservatively and
maintains consistency with ADR-HARVEST-007.

The Stage 0 safety property becomes:
```
□ ¬(turn ≥ 20 ∧ response_without_harvest_warning)
```

At turn 20, a warning footer is added to every tool response:
```
⚠ Context aging — consider: braid harvest (turn 20/40)
```

At turn 40, the warning escalates to harvest-only-mode indication:
```
⚠ HARVEST RECOMMENDED — context critically aged (turn 40+). Run: braid harvest
```

The two thresholds (20 and 40) correspond to the Q(t) thresholds from the formal
property: Q(t) < 0.30 triggers advisory warning, Q(t) < 0.15 triggers urgent warning.
The turn-count proxy maps these to fixed turn numbers based on empirical observation
of typical Q(t) decay curves.

#### Formal Justification
The safety property `□ ¬(Q(t) < 0.15 ∧ response_without_harvest_warning)` is
STRENGTHENED by the turn-count proxy, not weakened. The proxy triggers at turn 20,
which is before Q(t) typically reaches 0.15 (empirically around turn 25-30). This
means the proxy warns in a strict superset of the cases where Q(t) < 0.15 — it may
produce false positives (warning when Q(t) > 0.15) but never false negatives (failing
to warn when Q(t) < 0.15).

Formally: `turn ≥ 20 ⊇ {t | Q(t) < 0.15}` for typical sessions, therefore:
`□ ¬(turn ≥ 20 ∧ ¬warning) ⟹ □ ¬(Q(t) < 0.15 ∧ ¬warning)`

The conservative proxy satisfies the original safety property as a logical consequence.

Consistency with ADR-HARVEST-007: using the same thresholds (20/40) for both harvest
urgency and harvest warning avoids the hazard of contradictory signals — where one
subsystem says "harvest is urgent" but the interface layer suppresses the warning,
or vice versa. The thresholds being identical means the warning system and the harvest
urgency system agree on when action is needed.

#### Consequences
- Harvest warnings appear at turn 20 (advisory) and turn 40 (urgent) at Stage 0
- The thresholds match ADR-HARVEST-007, ensuring consistency across subsystems
- False positive rate: sessions that end before turn 25 will receive warnings that
  Q(t) would not have triggered. This is acceptable — early warnings are strictly
  safer than late warnings for a safety property
- Stage 1 replaces the turn-count proxy with formal Q(t) < 0.15 thresholds, which
  adapts to actual context decay rather than using fixed turn numbers
- The proptest strategy has a Stage 0 variant: set turn count >= 20, verify harvest
  warnings appear in every response
- Risk: the fixed thresholds may not match actual Q(t) decay for all session types
  (short investigative sessions vs. long implementation sessions have different decay
  rates). Mitigated by the conservative direction — the proxy warns too early, never
  too late
- Reversibility: fully reversible — the turn-count check is a single conditional that
  Stage 1 replaces with the Q(t) comparison

#### Falsification
The turn-count proxy fails to warn before Q(t) reaches 0.15 in > 10% of observed
sessions (proving the proxy is not conservative enough), OR agents consistently harvest
before turn 20 in typical sessions (proving the warning threshold is too high and the
proxy never activates), OR the cry-wolf effect from early warnings causes agents to
ignore harvest warnings at a rate > 50% (proving Option D's failure mode applies to
Option B as well despite the later trigger point).

---

### §14.6 Negative Cases

### NEG-INTERFACE-001: No Authoritative Non-Store State

**Traces to**: ADRS AA-003
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ authoritative_state that is not a projection of the store ∧ authoritative_state ∉ external_measurements)`

No interface layer maintains authoritative state that isn't derivable from the
store, with the explicit exception of **ephemeral runtime telemetry** —
external measurements (API token consumption, timing, k_eff) that originate
from actual API calls and are used for budget computation.

This exemption is narrowly scoped:
- **Exempted**: API telemetry in `context.json` (token counts, timing data,
  k_eff measurements). These originate outside the store's domain — they are
  sensor readings from the runtime environment, not store-derivable facts.
- **NOT exempted**: Spec elements, coordination state, provenance, harvest
  candidates, frontier data, entity state, signals, or any domain-relevant
  knowledge. These must be store-derivable.

Session state (SR-011) combines both: measured telemetry (exempted) and
store-derived projections (not exempted — must trace to datoms).
MCP notification queues are projections of pending signals (not exempted).

**proptest strategy**: After any sequence of interface operations, verify
that all non-telemetry layer state can be reconstructed from the store alone.
Verify that telemetry state consists only of external measurement values
(token counts, timing, k_eff) and not domain knowledge.

---

### NEG-INTERFACE-002: No MCP Logic Duplication

**Traces to**: ADRS IB-003
**Verification**: `V:PROP`

**Safety property**: `□ ¬(mcp_server implements logic that exists in cli_binary)`

The MCP server is a thin wrapper. Any computation that appears in both the
MCP server and the CLI binary is a duplication bug.

**proptest strategy**: Structural analysis — verify MCP tool handlers
contain only: kernel function dispatch, session state read/write, notification
queue management. No query parsing, resolution logic, or domain computation.

---

### NEG-INTERFACE-003: No Harvest Warning Suppression

**Traces to**: ADRS IB-012
**Verification**: `V:PROP`

**Safety property**: `□ ¬(Q(t) < 0.15 ∧ response_without_harvest_warning)`

When context is critically low, the harvest warning must appear. No configuration,
flag, or output mode may suppress it.

**Stage 0 simplification** (ADR-INTERFACE-010): At Stage 0, Q(t) is not yet available
(BUDGET is Stage 1). The harvest warning trigger reduces to a turn-count heuristic
matching ADR-HARVEST-007 thresholds: warn at turn 20, emit harvest-only-mode at turn 40.
This is conservative (warns earlier than Q(t) would) but safe — it preserves the
safety property by ensuring warnings cannot be suppressed. The turn-count proxy is the
same underlying decision as ADR-HARVEST-007 applied to the interface layer's safety
property. Stage 1 replaces the heuristic with formal Q(t) < 0.15 thresholds and the
proptest strategy below becomes executable.

**proptest strategy**: Set k*_eff to values below 0.15. Invoke all CLI commands.
Verify every response contains a harvest warning. (Stage 0 variant: set turn count
to values >= 20, verify harvest warnings appear in every response.)

---

### NEG-INTERFACE-004: No Error Without Recovery Hint

**Traces to**: ADRS IB-011
**Verification**: `V:PROP`

**Safety property**: `□(error_emitted → recovery_hint_present)`

Every error message emitted by the CLI or MCP layer includes a recovery hint.
No error is surfaced to the agent with only "operation failed" — every error
carries actionable recovery information following the four-part protocol
(what/why/recovery/spec_ref) defined in INV-INTERFACE-009.

**proptest strategy**: Generate arbitrary `KernelError` variants, format them
via the error formatter, assert all four parts are present in the output.

---

---

