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
Layer 2 (MCP):        Thin wrapper — machine-to-machine, nine tools
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

MCP_CALL(Σ, tool_name, params) → (result, Σ') where:
  PRE:  tool_name ∈ {ddis_status, ddis_guidance, ddis_associate, ddis_query,
                      ddis_transact, ddis_branch, ddis_signal, ddis_harvest, ddis_seed}
  POST: reads session state, computes Q(t), passes --budget to CLI
  POST: appends pending notifications
  POST: updates session state
  POST: checks harvest warning thresholds

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
    Structured,  // JSON — machine-parseable
    Agent,       // 100–300 tokens, headline + entities + signals + guidance + pointers
    Human,       // TTY — full formatting, color, tables
}

/// MCP server — thin wrapper calling CLI for all computation
pub struct MCPServer {
    pub session_state: SessionState,
    pub notification_queue: Vec<Signal>,
}

/// Nine MCP tools (IB-003)
pub enum MCPTool {
    Status,     // cheap: ≤50 tokens
    Guidance,   // cheap: ≤50 tokens
    Associate,  // moderate: 50–300 tokens
    Query,      // moderate: 50–300 tokens
    Transact,   // meta: side effect
    Branch,     // meta: side effect
    Signal,     // meta: side effect
    Harvest,    // meta: side effect
    Seed,       // expensive: 300+ tokens
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
Structured (JSON), Agent (budget-constrained), Human (TTY-formatted).
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
The MCP server performs no computation. All computation is delegated to the CLI.
MCP adds: session state management, budget adjustment, tool descriptions,
notification queuing.

`∀ mcp_call: result = cli_execute(mcp_call.to_cli_args()) + mcp_metadata`

#### Level 1 (State Invariant)
Every MCP_CALL transition invokes a CLI command as a subprocess. The MCP server
reads/writes session state and manages notifications but does not duplicate
any CLI logic.

**Falsification**: The MCP server implements query parsing, store access, or
any other logic that exists in the CLI binary.

---

### INV-INTERFACE-003: Nine MCP Tools

**Traces to**: ADRS IB-003
**Verification**: `V:PROP`, `V:TYPE`
**Stage**: 0

#### Level 0 (Algebraic Law)
The MCP server exposes exactly nine tools:
`{status, guidance, associate, query, transact, branch, signal, harvest, seed}`

#### Level 1 (State Invariant)
The tool set is fixed. Adding tools requires a spec update.
Each tool maps to a specific CLI command.

#### Level 2 (Implementation Contract)
```rust
// Type-level guarantee: exactly 9 tools
const MCP_TOOLS: [MCPTool; 9] = [
    MCPTool::Status, MCPTool::Guidance, MCPTool::Associate,
    MCPTool::Query, MCPTool::Transact, MCPTool::Branch,
    MCPTool::Signal, MCPTool::Harvest, MCPTool::Seed,
];
```

**Falsification**: The MCP server exposes a tool not in the defined set of nine.

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

Every MCP tool registration includes a description satisfying `Q(D)`. The
MCP_REGISTER transition checks `Q(D)` as a precondition. Descriptions are
compile-time constants (`const` array), so `Q(D)` is verified at development
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

Seed output follows a five-part template:
1. Context (1–2 sentences)
2. Invariants established
3. Artifacts produced
4. Open questions from deliberations
5. Active guidance

#### Formal Justification
The store is the sole truth (FD-012). Trajectory management through the store
means conversation boundaries become knowledge extraction points, not knowledge
loss points. The five-part template provides structure for the seed while
keeping it within budget.

---

### §14.6 Negative Cases

### NEG-INTERFACE-001: No Layer-Local State

**Traces to**: ADRS AA-003
**Verification**: `V:PROP`

**Safety property**: `□ ¬(∃ layer_state that is not a projection of the store)`

No interface layer maintains state that isn't derivable from the store.
Session state (SR-011) is a projection of measured context data.
MCP notification queues are projections of pending signals.

**proptest strategy**: After any sequence of interface operations, verify
that all layer state can be reconstructed from the store alone.

---

### NEG-INTERFACE-002: No MCP Logic Duplication

**Traces to**: ADRS IB-003
**Verification**: `V:PROP`

**Safety property**: `□ ¬(mcp_server implements logic that exists in cli_binary)`

The MCP server is a thin wrapper. Any computation that appears in both the
MCP server and the CLI binary is a duplication bug.

**proptest strategy**: Structural analysis — verify MCP tool handlers
contain only: subprocess call, session state read/write, notification
queue management. No query parsing, store access, or domain logic.

---

### NEG-INTERFACE-003: No Harvest Warning Suppression

**Traces to**: ADRS IB-012
**Verification**: `V:PROP`

**Safety property**: `□ ¬(Q(t) < 0.15 ∧ response_without_harvest_warning)`

When context is critically low, the harvest warning must appear. No configuration,
flag, or output mode may suppress it.

**proptest strategy**: Set k*_eff to values below 0.15. Invoke all CLI commands.
Verify every response contains a harvest warning.

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

