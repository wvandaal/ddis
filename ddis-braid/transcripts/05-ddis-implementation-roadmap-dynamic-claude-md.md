# Chapter 5: DDIS Implementation Roadmap & Dynamic CLAUDE.md

> Complete implementation roadmap for DDIS protocol from 0 to 100%, including staging model, change management for 60K LoC existing codebase, feedback loops, and the radical innovation of dynamically-generated CLAUDE.md that learns from drift patterns to self-improve agent methodology adherence.

---

## Claude

This is where the prompt optimization framework and the datom store architecture converge in a way that I think neither was designed for individually but that falls out naturally from first principles. Let me work through it layer by layer.

---

## The Fundamental Tension

An LLM agent has a **finite, decaying attention budget** (k\*). Every tool invocation returns output that enters the context window and competes for that budget. The datom store potentially contains thousands of datoms. The naive interface — "run a query, dump the results" — will destroy the agent's k\* within a few cycles.

The interface must therefore solve a **rate-distortion problem**: maximize the information value delivered to the agent while minimizing the attention cost. This is exactly the ASSEMBLE operation from the protocol, but applied to the *interface itself*.

Here's the formal statement:

```
Let I(tool_output) be the information value of a tool's output
    for the agent's current decision.
Let A(tool_output) be the attention cost (tokens consumed in 
    the context window).
Let k*_eff(t) be the agent's effective attention budget at 
    conversation turn t.

The interface must satisfy:

  ∀ tool invocation at turn t:
    A(tool_output) ≤ k*_eff(t) × budget_fraction

  where budget_fraction is the proportion of remaining k* 
  that a single tool call should consume (configurable, 
  default ≈ 0.05 — a single tool call should use ≤ 5% 
  of remaining budget).

  Subject to:
    I(tool_output) is sufficient for the agent to make its 
    next decision without a follow-up query.
```

This constraint has a concrete implication: **the same query must return different output at turn 5 vs. turn 45.** At turn 5, the agent has full k\* budget — it can absorb detailed output. At turn 45, k\*_eff has shrunk — the tool must compress aggressively or the output becomes parasitic context.

---

## The Five Interface Layers

You identified four: TUI, CLI, MCP, Auto-prompting Guidance. I believe there are five, and the one you're missing is the most important for k\* management.

### Layer 0: Ambient Awareness (CLAUDE.md)

This is the layer the prompt optimization document calls "ambient context" — **permanent, low-intensity, k\*-exempt**. It costs ~20 tokens per tool and ensures the agent *knows the tools exist* without consuming meaningful budget.

From the document: "agents fail to invoke available tools 56% of the time without ambient awareness." Without Layer 0, agents won't call ddis tools even when they should.

```
# In CLAUDE.md (the agent's system prompt / project instructions):

## DDIS Datom Store
You have access to `ddis` — a specification store for the project.
Use `ddis` for all spec reads/writes. Key commands:
  ddis query    — read datoms (Datalog)
  ddis associate — discover schema neighborhood
  ddis transact — assert/retract datoms
  ddis status   — current state, guidance, uncertainty
  ddis branch   — manage patch branches
Run `ddis help` for full reference. Run `ddis guidance` for 
recommended next actions.
```

That's ~80 tokens. It's permanent. It never competes with k\* because it's in the system prompt, which is processed once and forms the base field configuration. It satisfies the prompt optimization principle: "Keep awareness permanent; load activation on demand; shed activation when absorbed."

**Critical design point:** Layer 0 does NOT explain how the datom store works, what Datalog is, or what CRDTs are. That information is *active context* that should only be loaded when needed (and shed when absorbed). Layer 0 is *awareness*, not *activation*.

### Layer 1: CLI (The Query Engine Interface)

The CLI binary is the **substrate** — the computation layer that operates on the datom store. It's a Rust binary that:
- Loads datom files from disk
- Builds/maintains in-memory indexes
- Evaluates Datalog queries
- Computes ASSOCIATE neighborhoods
- Performs ASSEMBLE with budget constraints
- Manages branches, merges, frontiers
- Generates guidance

The CLI's output is what enters the agent's context window when called via bash. Therefore, **the CLI must be k\*-aware by default.**

Here's the key design principle:

```
INV-CLI-BUDGET-001: Output Budget Compliance
Every CLI command MUST accept an optional --budget <tokens> flag 
that caps the output size.

If --budget is not specified, the CLI reads the current session's 
budget from .ddis/session.json (which tracks estimated k*_eff).

If no session exists, the CLI defaults to a conservative budget 
(default: 500 tokens).

No CLI command may produce output exceeding the budget unless 
explicitly overridden with --budget unlimited (for human use 
or piped processing).

The budget is enforced by the output formatter, which applies 
pyramid-level selection to compress results that would exceed 
the budget.
```

The CLI has two output modes:

**Structured mode** (default when stdout is not a TTY, or when `--format json`): Returns JSON that can be parsed by the MCP server or by the agent's tool processing. Minimal, machine-oriented.

**Human mode** (default when stdout IS a TTY): Returns formatted, colored output with context. Used when Willem runs commands directly.

**Agent mode** (when `--agent <agent_id>` is passed): Returns output specifically shaped for LLM consumption. This is the critical mode.

Agent mode output follows the prompt optimization principles:

```
PRINCIPLE: CLI agent-mode output is a DEMONSTRATION, not a 
constraint list.

BAD (constraint-style output — wastes k*):
  "Query returned 47 results. Entity :file/main-rs has attributes 
   :line-count (347), :contains-fn (:fn/handle-request), :modified 
   (2026-03-01), :hash (a7f3...), :size (12847), :language (:rust),
   :imports ([:tokio, :serde, :axum]), :test-coverage (0.73), ..."
  → The agent gets 47 entities × N attributes = hundreds of tokens 
    of flat data. Most of it is irrelevant to the current decision. 
    This is a context dump, not useful information.

GOOD (demonstration-style output — activates reasoning):
  "3 entities relevant to auth implementation:
   :file/main-rs — contains handle_request (TOCTOU risk, see INV-007)
   :config/auth  — JWT settings (uncertainty: 0.7, last observed 2h ago)
   :test/auth    — 3 failing assertions on token refresh
   
   ⚠ Guidance: INV-007 has high uncertainty. Consider re-observing 
     config/auth before proceeding.
   
   [Run `ddis query '[:find ...]'` for full details on any entity]"
  → The agent gets the minimum needed to decide its next action, 
    with a pointer to go deeper if needed. 80 tokens, not 800.
```

The agent-mode output structure:

```
CLI Agent-Mode Output Format:

1. HEADLINE (1 line): What the command found/did, in terms the 
   agent can immediately act on.

2. RELEVANT ENTITIES (3-7 items): The highest-relevance, 
   highest-significance results, at the pyramid level 
   appropriate for the current budget.

3. SIGNALS (0-3 items): Conflicts, uncertainty spikes, 
   staleness warnings — anything requiring attention.

4. GUIDANCE (1-3 items): Recommended next actions from the 
   guidance graph, ordered by relevance to the agent's 
   current intention.

5. POINTERS (1-3 items): Commands to run for deeper 
   information if needed.

Total target: 100-300 tokens per invocation.
```

This format maps directly to the dual-process architecture:
- Items 1-2 are **System 1 output** (ASSOCIATE + ASSEMBLE results, cheap, pattern-matchable)
- Item 3 is the **confusion channel** (signals that trigger re-retrieval)
- Item 4 is the **guidance comonad** (available actions at current state)
- Item 5 is the **depth escape hatch** (agent can go deeper if S2 needs more)

### Layer 2: MCP Server (Session-Aware Agent Interface)

Now, the question of whether MCP adds value over raw CLI. My analysis:

**What MCP provides that CLI does not:**

1. **Persistent session state.** An MCP server process maintains in-memory state across tool calls: the agent's frontier, the current k\*_eff estimate, the active intention, the subscription list. The CLI binary is stateless — it reads state from disk on every invocation, which means it can't track intra-conversation dynamics like k\* decay.

2. **Structured tool descriptions as ambient context.** MCP tool descriptions are injected into the agent's system-level context by the harness (Claude Code). They're essentially automatic Layer 0 — the agent sees the tool signatures without anyone manually editing CLAUDE.md.

3. **Type-safe input/output.** MCP defines schemas for tool inputs and outputs. The agent doesn't need to construct bash command strings (which can be malformed) — it fills in typed parameters.

4. **Proactive notifications.** An MCP server can push signals to the agent without being polled. When a conflict is detected or another agent's merge changes relevant state, the MCP server can inject a notification into the agent's next turn. The CLI can only respond when called.

**What CLI provides that MCP does not:**

1. **Zero infrastructure.** A Rust binary works. No running process, no protocol, no connection management.

2. **Human-accessible.** Willem can run `ddis status` from his terminal. He can't easily call MCP tools from a shell.

3. **Scriptable.** Bash scripts can compose CLI commands. MCP calls can't be easily piped/composed.

4. **Universal.** Any agentic harness that can call bash can use the CLI. MCP requires specific client support.

**My recommendation: Both, with the CLI as the foundation and a thin MCP wrapper.**

```
Architecture:

  ┌─────────────────────────────────┐
  │  Claude Code (Agent Harness)    │
  │                                 │
  │  ┌──────────┐  ┌────────────┐  │
  │  │ MCP      │  │ Bash Tool  │  │
  │  │ Client   │  │ Interface  │  │
  │  └────┬─────┘  └─────┬──────┘  │
  └───────┼──────────────┼─────────┘
          │              │
  ┌───────▼──────┐  ┌───▼────────────────┐
  │  ddis-mcp    │  │  ddis (CLI binary)  │
  │  (thin MCP   │──▶  (query engine,     │
  │   wrapper,   │  │   indexes, Datalog, │
  │   session    │  │   budget-aware      │
  │   state,     │  │   output)           │
  │   k* tracking│  └────────────────────┘
  │   notifs)    │
  └──────────────┘
```

The MCP server (`ddis-mcp`) is a thin process that:
- Calls the CLI binary for all datom operations
- Maintains session state (k\*_eff estimate, frontier, active intentions)
- Adjusts `--budget` passed to CLI based on k\*_eff tracking
- Provides MCP tool descriptions (ambient awareness)
- Queues notifications for delivery on next tool call

The CLI binary (`ddis`) is the core:
- All datom operations implemented here
- All query evaluation, indexing, CRDT logic
- Budget-aware output formatting
- Works standalone (for humans and scripts)
- Works as backend for MCP server

**The MCP server adds exactly one thing the CLI can't: session state that persists across tool calls within a conversation.** This is what enables k\* tracking, proactive notifications, and automatic budget adjustment.

But — and this is critical — the system works without MCP. An agent using only bash calls to `ddis` still gets budget-aware output, guidance, and the full protocol. The MCP server is an optimization, not a requirement. This preserves universality.

### Layer 3: Auto-Prompting Guidance

This is the **comonadic guidance structure** from our previous analysis, but now I need to formalize how it interacts with k\* decay.

The key insight from the prompt optimization document: **guidance IS the seed turn.** Study 8 showed that seed turns producing formal/principled reasoning improved subsequent quality with p=0.029. The guidance output from the datom system serves this exact function — it frames the agent's next action in terms of invariants, spec elements, and formal structure, naturally activating deep substrate.

```
INV-GUIDANCE-SEED-001: Guidance as Basin Activation
The guidance section of every CLI/MCP response MUST be phrased 
in spec-language (invariants, formal structure, domain terms), 
NOT in instruction-language (steps, checklists, procedures).

BAD guidance (instruction-language, activates surface substrate):
  "Step 1: Read the auth config file.
   Step 2: Check if JWT tokens are configured.
   Step 3: Run the test suite.
   Step 4: Fix any failures."

GOOD guidance (spec-language, activates deep substrate):
  "INV-AUTH-003 (token-refresh idempotency) has uncertainty 0.7.
   The :config/auth entity was last observed 2h ago — staleness 
   risk. The competing branches impl-auth-v1 and impl-auth-v2 
   both assert different token lifetimes; this conflict is 
   unresolved. What invariant should govern token lifetime?"

The good version:
  — Names invariants (activates formal reasoning)
  — Identifies uncertainty (focuses attention)
  — Poses a question (high DoF — exploration, not execution)
  — Does not prescribe steps (avoids mid-DoF saddle zone)
```

But guidance must also be k\*-aware. At turn 5, the agent can absorb rich guidance with context. At turn 45, guidance should be a single sentence:

```
GUIDANCE COMPRESSION BY ESTIMATED k*_eff:

k*_eff > 0.7 (early conversation, full budget):
  Full guidance: invariant context, uncertainty details, 
  competing branches, precedent references.
  Target: 100-200 tokens.

0.4 < k*_eff ≤ 0.7 (mid conversation, reduced budget):
  Compressed guidance: headline + single recommended action.
  Target: 30-60 tokens.

k*_eff ≤ 0.4 (late conversation, depleted budget):
  Minimal guidance: single imperative or question.
  Target: 10-20 tokens.
  Example: "Resolve INV-AUTH-003 uncertainty before proceeding."

k*_eff ≤ 0.2 (conversation should reset):
  Guidance: "Context budget critically low. Run `ddis harvest` 
  to generate carry-over, then start fresh conversation."
  Target: 15 tokens.
```

This is where the comonadic structure meets the prompt optimization framework's "budget constraints against context" rule. The `extend` operation from the guidance comonad is now parameterized by k\*_eff:

```
extend_guidance : k*_eff → (State → [Action]) → State → GuidanceTree

At high k*_eff: deep lookahead tree, rich context.
At low k*_eff: single recommended action, no tree.
```

### Layer 4: TUI (Human Live Feedback)

The TUI is the human's window into the system. It is NOT k\*-constrained (humans read from a terminal, not from a context window). Therefore the TUI can be as rich as useful.

The TUI is driven by subscriptions (SUBSCRIBE operation) and renders:
- Activity stream (real-time datom assertions across agents)
- Uncertainty heatmap (entities colored by uncertainty scalar)
- Branch status (active branches, comparisons, deliberations)
- Agent intentions (what each agent thinks it's doing)
- k\* gauges per agent (estimated remaining budget per conversation)
- Guidance topology (what actions are available, visualized as a graph)
- Fitness metrics (spec completeness, drift scores)

The TUI does not enter any agent's context window. It's a separate process reading from the datom store and the access log. Its output is purely for human consumption.

**One additional TUI function you implied but didn't name:** The TUI should support **human→agent signaling.** When Willem sees something on the TUI that needs attention (an agent drifting, a bad resolution, a missed conflict), he should be able to inject a signal from the TUI that enters the relevant agent's next tool response. This is the proactive direction mechanism you described.

```
TUI signal injection:
  Willem sees: Agent Alice is working on auth but INV-AUTH-003 
               was just invalidated by Bob's merge.
  Willem types: /signal alice "INV-AUTH-003 invalidated by Bob's 
               merge — re-observe before proceeding"
  Effect: Alice's next ddis command (via MCP) includes the 
          signal in its response. Alice's policy π sees the 
          signal and adjusts.
```

This is mediated by the MCP server, which queues the human signal for delivery.

---

## The Missing Piece: Trajectory Management via the Store

Here's the deep connection between the prompt optimization framework and the datom architecture that I don't think either was designed for but that falls out naturally.

The prompt optimization document identifies the core problem of long conversations:

> "By mid-conversation, [the model's own prior outputs] dominate the field more than the current prompt. This creates basin trapping: mediocre outputs self-reinforce, making prompt-level fixes insufficient. The intervention is trajectory management: seed turns, output curation, and strategic resets with curated carry-over."

The prescribed fix: harvest artifacts, reset conversation, seed new conversation with carry-over that's "basin-neutral" and "spec-flavored."

**The datom store IS this mechanism.** Here's why:

When an agent asserts datoms (discoveries, invariants, decisions, associations), those facts persist in the store independent of the conversation. When the conversation degrades (k\* depletion or basin trapping), the agent can:

1. `ddis harvest` — Extract the durable facts from the current conversation's assertions
2. Start a fresh Claude Code session
3. `ddis assemble --seed` — Generate a carry-over context from the store

The ASSEMBLE operation produces exactly what the trajectory-dynamics document prescribes:
- **Basin-neutral**: It's datoms, not conversation fragments
- **Spec-flavored**: Invariants and formal structure naturally activate deep substrate
- **Minimal**: Budget-constrained by the ASSEMBLE budget parameter
- **No conversation fragments**: Datoms don't carry "as we discussed" — they're facts

```
INV-TRAJECTORY-STORE-001: Store-Mediated Trajectory Management
The datom store MUST support a HARVEST operation that extracts 
durable facts from the current agent session and asserts them 
as datoms.

The datom store MUST support a SEED operation (via ASSEMBLE) 
that generates an optimal carry-over context for a fresh agent 
conversation, using the carry-over template format:

  1. Context (1-2 sentences from store summary)
  2. Invariants established (from :entity/type :invariant)
  3. Artifacts (current spec elements, code references)
  4. Open questions (from unresolved deliberations, high-uncertainty entities)
  5. Active guidance (from guidance graph, current state)

The SEED output MUST be formatted as a spec-first seed turn 
(Pattern #9 from prompt-patterns.md) to activate deep substrate 
in the receiving conversation.

Consequence: Agent conversation resets become cheap. The agent 
loses no durable knowledge (it's in the store). It loses only 
ephemeral reasoning (which was degrading anyway due to k* 
depletion). The fresh conversation starts with a formal seed 
that activates deep substrate.
```

This means the datom store solves the trajectory management problem **by construction**. Each conversation is a bounded trajectory (25-30 turns). The store is the persistent layer. Conversations start with store-assembled seeds and end with store-harvested assertions. The conversation is ephemeral; the knowledge is durable.

```
AGENT LIFECYCLE:

  1. Fresh conversation starts
  2. `ddis seed` generates carry-over context 
     (ASSEMBLE with --seed flag, spec-first format)
  3. Agent works for 20-30 turns, asserting discoveries 
     as datoms via `ddis transact`
  4. k*_eff drops below threshold (detected by MCP session 
     tracking or by agent observing degraded output)
  5. `ddis harvest` extracts durable facts from this session
  6. Conversation ends
  7. GOTO 1

  The store grows monotonically across conversations.
  Each conversation is a bounded trajectory.
  No conversation needs to be "the long one" — all 
  conversations are short, fresh, and seed-activated.
```

---

## The k\*-Aware CLI Command Taxonomy

With the interface layers defined, here's the concrete CLI design. Every command is classified by its k\* profile:

```
COMMAND TAXONOMY BY ATTENTION PROFILE:

CHEAP COMMANDS (≤50 tokens output, always safe):
  ddis status              — one-line state summary + top signal
  ddis guidance            — 1-3 recommended actions
  ddis frontier            — agent's current causal position
  ddis branch ls           — list active branches (names only)

MODERATE COMMANDS (50-300 tokens, safe early/mid conversation):
  ddis associate <cue>     — schema neighborhood (shape, not data)
  ddis query <expr>        — query results (budget-capped)
  ddis assemble            — assembled context (budget-capped)
  ddis diff <branch>       — branch delta summary
  ddis conflict ls         — active conflicts with severity
  ddis deliberation ls     — open deliberations with status

EXPENSIVE COMMANDS (300+ tokens, use sparingly, budget-gate):
  ddis assemble --full     — full context assembly (large)
  ddis query --unlimited   — uncapped query results
  ddis seed                — full carry-over seed (for new convo)
  ddis explain <entity>    — detailed entity with history

META COMMANDS (not for agent context window):
  ddis harvest             — extract durable facts (side effect)
  ddis transact            — assert datoms (side effect, minimal output)
  ddis merge               — CRDT merge (side effect)
  ddis subscribe           — register notification (side effect)
  ddis tui                 — launch TUI (separate process)
```

Every moderate and expensive command respects `--budget`:

```bash
# Early conversation (full budget available):
ddis associate "auth token refresh" --budget 300
# Returns: 15 entities, 40 attributes, 5 learned associations

# Late conversation (budget constrained):
ddis associate "auth token refresh" --budget 80
# Returns: 3 most significant entities, key attributes only

# The same query. Different output. Budget-aware compression.
```

---

## One Structural Question

Before I can finalize the interface design, I have a genuine uncertainty about the MCP layer.

**The k\* estimation problem.** The MCP server needs to estimate k\*_eff to adjust output budgets automatically. But k\*_eff is a function of:
- Conversation length (turns elapsed)
- Total tokens in context (system prompt + conversation history)
- Complexity of the current task (higher complexity → lower effective k\*)

The conversation length and approximate token count are knowable by the MCP server (it can count its own tool calls as a proxy for turns, and sum the sizes of its responses). But the total context window state — including the user's messages, system prompt, and other tool outputs — is NOT visible to the MCP server. It only sees its own tool calls.

**Two options for k\* estimation:**

**Option A — Turn-count heuristic.** The MCP server estimates k\*_eff as a decreasing function of the number of tool calls it has served in this session:

```
k*_eff = k*_base × decay(tool_calls_served)

where decay is exponential: decay(n) = e^{-αn}
and α is calibrated empirically (default: α = 0.03, 
giving k*_eff ≈ 0.5 at 23 tool calls)
```

Simple, doesn't require any information from the agent. But potentially inaccurate — a conversation with many short turns decays faster than one with few long turns.

**Option B — Agent-reported budget.** The agent includes its estimated remaining budget in each tool call:

```bash
ddis query '[:find ?e ...]' --context-tokens 45000 --context-limit 200000
```

The CLI/MCP uses the ratio (tokens_used / limit) to estimate k\*_eff. More accurate, but requires the agent to know and report its context state. Claude Code doesn't natively expose this, so it would need to be estimated by the agent itself.

**My recommendation: Option A with Option B as override.** The turn-count heuristic works for the common case. If the agent or human wants finer control, `--budget` overrides the automatic estimate. The MCP server's heuristic is a reasonable default, not a precise measurement — which aligns with the prompt optimization document's observation that "k\* cannot be determined in advance."

What's your preference on the k\* estimation approach? And does the five-layer interface architecture (Ambient/CLI/MCP/Guidance/TUI) match your mental model, or do you see a layer I'm missing or one that should be collapsed?

---

## Willem

I think that Claude Code actually does expose its context tokens both directly and indirectly. The CC statusline (https://code.claude.com/docs/en/statusline.md) provides access to this, and I also think that the jsonl session logs would likely provide an indirect measure for context consumption too

---

## Claude

Good catch — let me verify what's actually available before I design around assumptions.

Now I have the full picture. This changes the design significantly — Option B becomes not just feasible but clearly superior. Let me formalize the revised approach.

---

## What Claude Code Exposes

The statusline hook receives structured JSON via stdin containing:

```json
{
  "context_window": {
    "used_percentage": 42.3,
    "current_usage": {
      "input_tokens": 84600,
      "output_tokens": 12400,
      "cache_creation_input_tokens": 15000,
      "cache_read_input_tokens": 62000
    }
  },
  "transcript_path": "/home/willem/.claude/sessions/abc123/transcript.jsonl",
  "session_id": "abc123",
  "model": {
    "id": "claude-opus-4-6",
    "display_name": "Claude Opus 4.6"
  },
  "workspace": {
    "current_dir": "/home/willem/project",
    "project_dir": "/home/willem/project"
  },
  "cost": {
    "total_cost_usd": 1.47,
    "total_duration_ms": 342000,
    "total_lines_added": 127,
    "total_lines_removed": 43
  }
}
```

Two independent measurement channels:

**Channel 1 — Direct**: `context_window.used_percentage` and `current_usage` token breakdown. This is the authoritative source — Claude Code computes it from the API response metadata.

**Channel 2 — Indirect**: The JSONL transcript at `transcript_path` contains every message in the session. Each assistant message includes `message.usage` with per-turn token counts. By parsing from the tail of this file, any external process can reconstruct context consumption independently.

Both channels are accessible to any process on the VPS that can read the filesystem. The MCP server, the CLI, the TUI, and even the statusline script all have access.

---

## Revised k\* Architecture: Grounded, Not Heuristic

This eliminates the need for Option A (turn-count heuristic). We can build the budget system on **measured** context consumption rather than estimated.

### The Context Budget Model

```
Let W be the context window size (tokens).
    For Claude Opus 4.6: W = 200,000
    (or 1,000,000 with extended context, but usable 
     before auto-compact ≈ 0.8 × W)

Let U(t) be the measured context usage at time t 
    (from context_window.used_percentage × W).

Let R(t) = W - U(t) be the remaining context capacity.

Let k*_eff(t) = R(t) / W, normalized to [0, 1].
    This is the fraction of context still available.

But k*_eff alone is insufficient. The prompt optimization 
framework tells us that attention quality degrades faster 
than context consumption — past ~60-70% usage, the model's 
effective reasoning depth drops even though tokens remain.

Define the quality-adjusted budget:

  Q(t) = k*_eff(t) × attention_decay(k*_eff(t))

  where attention_decay : [0,1] → [0,1] is:
    attention_decay(x) = 1.0           if x > 0.6
    attention_decay(x) = x / 0.6       if 0.3 < x ≤ 0.6
    attention_decay(x) = (x / 0.3)²    if x ≤ 0.3

This captures the observation that context quality degrades 
non-linearly: little effect until ~40% used, then linear 
degradation, then rapid collapse past ~70%.
```

The quality-adjusted budget Q(t) is what the CLI uses for output budget computation:

```
output_budget(t) = max(MIN_OUTPUT, Q(t) × W × budget_fraction)

where:
  MIN_OUTPUT = 50 tokens (always give at least status + guidance)
  budget_fraction = 0.05 (single tool call uses ≤5% of remaining quality budget)
```

Concrete examples with W = 200,000:

```
Context 10% used (U=20K, R=180K):
  k*_eff = 0.90, attention_decay = 1.0
  Q = 0.90
  output_budget = 0.90 × 200K × 0.05 = 9,000 tokens
  → Full detail. Rich guidance. Deep context.

Context 50% used (U=100K, R=100K):
  k*_eff = 0.50, attention_decay = 0.50/0.60 = 0.83
  Q = 0.50 × 0.83 = 0.42
  output_budget = 0.42 × 200K × 0.05 = 4,200 tokens
  → Moderate detail. Compressed guidance.

Context 75% used (U=150K, R=50K):
  k*_eff = 0.25, attention_decay = (0.25/0.30)² = 0.69
  Q = 0.25 × 0.69 = 0.17
  output_budget = 0.17 × 200K × 0.05 = 1,700 tokens
  → Compressed output. Single-action guidance. Harvest warning.

Context 85% used (U=170K, R=30K):
  k*_eff = 0.15, attention_decay = (0.15/0.30)² = 0.25
  Q = 0.15 × 0.25 = 0.04
  output_budget = 0.04 × 200K × 0.05 = 400 tokens
  → Minimal output. "Harvest and reset" imperative.

Context 90%+ used:
  → Auto-compact imminent. CLI returns ONLY:
    "⚠ Context critical. Run `ddis harvest` now."
```

### How the CLI Reads Context State

The CLI needs access to the current session's context usage. Three mechanisms, in preference order:

**Mechanism 1 — Session state file (written by statusline or MCP):**

```bash
# The statusline hook (or MCP server) writes context state 
# to a well-known location on every update:
cat .ddis/session/context.json
{
  "used_percentage": 42.3,
  "input_tokens": 84600,
  "remaining_tokens": 115400,
  "k_eff": 0.577,
  "quality_adjusted": 0.555,
  "output_budget": 5550,
  "timestamp": "2026-03-01T22:15:03Z",
  "session_id": "abc123",
  "transcript_path": "/home/willem/.claude/sessions/abc123/transcript.jsonl"
}
```

The CLI reads this file on every invocation. Cost: one stat + one small file read. The statusline hook updates it on every refresh cycle (sub-second). This is the **fastest and most reliable** path.

**Mechanism 2 — Direct transcript parsing:**

If the session state file is stale or missing, the CLI falls back to parsing the JSONL transcript directly:

```rust
fn estimate_context_from_transcript(path: &Path) -> ContextEstimate {
    // Read from tail — last assistant message has latest usage
    let last_assistant = read_jsonl_reverse(path)
        .find(|entry| entry.type_ == "assistant" 
                    && entry.message.usage.is_some());
    
    match last_assistant {
        Some(entry) => {
            let usage = entry.message.usage.unwrap();
            let total_input = usage.input_tokens 
                + usage.cache_creation_input_tokens 
                + usage.cache_read_input_tokens;
            ContextEstimate {
                used_tokens: total_input,
                used_percentage: total_input as f64 / CONTEXT_LIMIT as f64 * 100.0,
                source: EstimateSource::Transcript,
            }
        }
        None => ContextEstimate::default(), // conservative default
    }
}
```

This is O(1) in practice (reads from the tail), but slightly less accurate than the statusline-reported value because it doesn't account for system prompt size or cached tokens in the same way.

**Mechanism 3 — Explicit flag:**

The agent or human passes context state directly:

```bash
ddis query '[:find ?e ...]' --context-used 42.3
# or
ddis query '[:find ?e ...]' --budget 300
```

This overrides all automatic estimation. The `--budget` flag is the escape hatch for precise control.

**Precedence:** `--budget` (explicit) > `--context-used` (explicit percentage) > session state file > transcript parsing > conservative default (500 tokens).

### The Session State File as Coordination Point

The session state file (`.ddis/session/context.json`) becomes a crucial coordination artifact. It's maintained by whichever process has access to Claude Code's statusline data:

```
INV-SESSION-STATE-001: Context State Freshness
The session state file MUST be updated at least once per agent 
turn (approximately every tool call cycle).

The update source is, in preference order:
  1. MCP server (receives context data from Claude Code on 
     each tool invocation)
  2. Statusline hook (runs on every Claude Code render cycle, 
     has access to context_window JSON)
  3. Transcript parser (can be run as a background watcher)

Staleness: if the session state file's timestamp is >30 seconds 
old at CLI invocation time, the CLI MUST fall back to transcript 
parsing.
```

---

## The MCP Server Revision: Session-Aware Budget Manager

With direct context measurement available, the MCP server's role sharpens. It's no longer guessing at k\*_eff — it's computing it from measured data. Here's the revised architecture:

```
┌─────────────────────────────────────────────────────────┐
│  Claude Code                                             │
│                                                          │
│  context_window: { used_percentage, current_usage }      │
│  transcript_path: /home/willem/.claude/sessions/abc/...  │
│                                                          │
│  ┌──────────────────┐    ┌────────────────────┐         │
│  │  MCP Client       │    │  Bash Tool          │         │
│  │  (passes context  │    │  Interface          │         │
│  │   metadata to     │    │                     │         │
│  │   MCP server)     │    │                     │         │
│  └────────┬─────────┘    └──────────┬──────────┘         │
│           │                         │                     │
│  ┌────────▼─────────────────────────▼──────────┐         │
│  │  Statusline Hook                              │         │
│  │  (writes .ddis/session/context.json)          │         │
│  └───────────────────────────────────────────────┘         │
└─────────────────────────────────────────────────────────┘
            │                         │
    ┌───────▼──────┐          ┌──────▼───────────────┐
    │  ddis-mcp    │          │  ddis (CLI binary)   │
    │              │          │                      │
    │  Receives:   │──calls──▶│  Reads:              │
    │  - tool call │          │  - context.json      │
    │  - session   │          │  - OR transcript     │
    │    metadata  │          │  - OR explicit flag   │
    │              │          │                      │
    │  Maintains:  │          │  Computes:           │
    │  - frontier  │          │  - k*_eff            │
    │  - intentions│          │  - quality budget    │
    │  - notif     │          │  - output at budget  │
    │    queue     │          │                      │
    │              │          │  Returns:            │
    │  Adjusts:    │          │  - budget-capped     │
    │  - passes    │          │    output            │
    │    --budget  │          │  - guidance at       │
    │    to CLI    │          │    appropriate depth  │
    └──────────────┘          └──────────────────────┘
```

The MCP server can extract context metadata from the tool call environment (Claude Code passes session info to MCP servers) and use it to set the `--budget` flag when calling the CLI. But even without the MCP server, the CLI can read context state independently via the session state file or transcript.

This means the system is **robust to MCP availability**: with MCP, budget management is automatic and precise. Without MCP (pure CLI), the agent can pass `--budget` explicitly, or the CLI reads from the session state file (maintained by the statusline hook), or it falls back to transcript parsing.

### MCP Tool Definitions

The MCP server exposes these tools to Claude Code:

```typescript
// The MCP server wraps CLI commands with session-aware budget management

tools: [
  {
    name: "ddis_status",
    description: "Current store state, top signals, recommended action. Very cheap (~50 tokens).",
    inputSchema: { type: "object", properties: {} }
    // No parameters needed — always returns compressed status
  },
  {
    name: "ddis_guidance", 
    description: "Recommended next actions based on current state, uncertainty, and active intentions. Cheap (30-150 tokens depending on context budget).",
    inputSchema: { type: "object", properties: {} }
  },
  {
    name: "ddis_associate",
    description: "Discover what's in the store related to a concept. Returns schema shape, not data. Use before querying.",
    inputSchema: {
      type: "object",
      properties: {
        cue: { type: "string", description: "Semantic concept to explore" },
        depth: { type: "integer", default: 2, description: "Traversal depth (1-5)" },
        breadth: { type: "integer", default: 5, description: "Max edges per node" }
      },
      required: ["cue"]
    }
  },
  {
    name: "ddis_query",
    description: "Read datoms via Datalog. Use ddis_associate first to discover schema.",
    inputSchema: {
      type: "object",
      properties: {
        expression: { type: "string", description: "Datalog query expression" },
        branch: { type: "string", description: "Branch name (omit for trunk)" }
      },
      required: ["expression"]
    }
  },
  {
    name: "ddis_transact",
    description: "Assert or retract datoms. Very cheap output (~20 tokens confirmation).",
    inputSchema: {
      type: "object",
      properties: {
        datoms: { type: "array", items: { type: "string" }, description: "Datom specs" },
        provenance: { type: "string", enum: ["observed", "derived", "inferred", "hypothesized"] },
        rationale: { type: "string" },
        branch: { type: "string", description: "Branch name (omit for trunk)" }
      },
      required: ["datoms", "provenance", "rationale"]
    }
  },
  {
    name: "ddis_branch",
    description: "Manage patch branches: fork, commit, combine, compare, abandon.",
    inputSchema: {
      type: "object",
      properties: {
        action: { type: "string", enum: ["fork", "commit", "combine", "compare", "abandon", "list"] },
        name: { type: "string" },
        purpose: { type: "string" },
        competing_with: { type: "array", items: { type: "string" } }
      },
      required: ["action"]
    }
  },
  {
    name: "ddis_signal",
    description: "Emit a coordination signal: confusion, conflict, goal-drift, branch-ready.",
    inputSchema: {
      type: "object",
      properties: {
        type: { type: "string", enum: ["confusion", "conflict", "uncertainty_spike", "goal_drift", "branch_ready", "deliberation_turn"] },
        cue: { type: "string" },
        target: { type: "string", default: "subscribers" }
      },
      required: ["type"]
    }
  },
  {
    name: "ddis_harvest",
    description: "Extract durable facts from current session for carry-over. Run before conversation reset.",
    inputSchema: { type: "object", properties: {} }
  },
  {
    name: "ddis_seed",
    description: "Generate a carry-over seed for a fresh conversation from the store. Run at start of new session.",
    inputSchema: {
      type: "object", 
      properties: {
        focus: { type: "string", description: "What the new session will work on" }
      }
    }
  }
]
```

The MCP server intercepts every tool call and:
1. Reads the current session's context state
2. Computes Q(t) (quality-adjusted budget)
3. Passes `--budget` to the CLI based on Q(t)
4. Appends any queued notifications (from other agents, human signals) to the response
5. Updates the session state file with the latest context measurement
6. Checks if Q(t) has crossed any threshold (harvest warning, reset imperative)

### The Budget Compression in Action

Here's the same `ddis_associate` call at different points in a conversation:

**Turn 3, context 8% used (Q ≈ 0.92):**

```
Schema neighborhood for "auth token refresh":

15 entities found, 42 attributes, 5 learned associations.

Entities:
  :config/auth       — JWT settings (audience, issuer, secret, lifetime)
  :config/refresh    — token refresh policy (interval, grace-period, max-lifetime)
  :invariant/INV-AUTH-003 — token refresh idempotency
  :invariant/INV-AUTH-007 — session revocation completeness
  :file/auth-handler — src/handlers/auth.rs (last observed: 12min ago)
  :file/token-service — src/services/token.rs (last observed: 12min ago)
  :file/auth-tests   — tests/auth_test.rs (3 assertions, 1 failing)
  :entity/user-session — session entity (12 attributes)
  :entity/refresh-token — refresh token entity (8 attributes)
  :decision/DEC-017  — chose JWT over opaque tokens (deliberation D-004)
  [+5 more below threshold]

Key attributes:
  :config/auth — :jwt/audience, :jwt/issuer, :jwt/secret-ref, 
                 :jwt/access-lifetime, :jwt/refresh-lifetime
  :invariant/INV-AUTH-003 — :statement, :violation, :validation, 
                            :why, :confidence (0.7), :last-validated
  :entity/user-session — :session/user-ref, :session/created, 
                          :session/expires, :session/refresh-token-ref,
                          :session/revoked?, :session/device-fingerprint

Learned associations:
  :config/auth ←:causally-linked-to→ :file/auth-handler (strength: 0.9)
  :invariant/INV-AUTH-003 ←:violated-by→ :file/token-service:L47 (str: 0.6)
  :entity/refresh-token ←:depends-on→ :config/refresh (strength: 0.85)
  :decision/DEC-017 ←:constrains→ :invariant/INV-AUTH-003 (strength: 0.8)
  :file/auth-tests ←:validates→ :invariant/INV-AUTH-003 (strength: 0.7)

Signals:
  ⚠ INV-AUTH-003 confidence 0.7 — below threshold. Uncertainty source: 
    competing branches assert different refresh lifetimes.
  ⚠ :file/auth-tests has 1 failing assertion on token refresh.

Guidance:
  Resolve INV-AUTH-003 uncertainty: the competing branches impl-auth-v1 
  and impl-auth-v2 disagree on refresh token lifetime. This conflict 
  blocks confidence advancement. Consider `ddis deliberation open 
  --subject INV-AUTH-003 --branches impl-auth-v1,impl-auth-v2`.

  [ddis query '[:find ?e ?v :where [?e :jwt/refresh-lifetime ?v]]' 
   for full lifetime values across branches]
```

**~350 tokens. Rich, formal, spec-language. Activates deep substrate.**

**Turn 28, context 62% used (Q ≈ 0.35):**

```
"auth token refresh": 15 entities, 42 attrs.

Top 3:
  :invariant/INV-AUTH-003 — refresh idempotency (confidence: 0.7 ⚠)
  :config/auth — JWT settings
  :file/token-service — src/services/token.rs

⚠ INV-AUTH-003 has unresolved conflict between branches.
→ `ddis deliberation open --subject INV-AUTH-003`
```

**~60 tokens. Same information, compressed to what the agent needs to decide.**

**Turn 42, context 82% used (Q ≈ 0.08):**

```
INV-AUTH-003 unresolved (σ=0.7). Resolve before proceeding.
⚠ Context 82%. Run `ddis harvest` soon.
```

**~20 tokens. Minimum viable guidance. Harvest warning.**

---

## Revised Invariants for the Interface

```
INV-INTERFACE-BUDGET-001: Measured Budget Enforcement
Every CLI and MCP tool response MUST be capped at an output 
budget computed from MEASURED context consumption:

  output_budget = max(MIN_OUTPUT, Q(t) × W × budget_fraction)

where Q(t) is the quality-adjusted budget derived from the 
session's context_window.used_percentage.

The context measurement source is, in precedence order:
  1. MCP session metadata (direct from Claude Code)
  2. Session state file (.ddis/session/context.json)
  3. Transcript tail-parse (transcript_path from CC session)
  4. Explicit --budget flag (human/agent override)
  5. Conservative default (500 tokens)

Violation: A tool response that exceeds the computed budget 
becomes parasitic context — it consumes attention without 
proportional information value, contributing to k* depletion.

INV-INTERFACE-COMPRESSION-001: Pyramid Level Selection
The CLI MUST select pyramid summary levels for each entity 
in its output based on the available budget:

  If budget > 2000 tokens: π₀ for top entities, π₁ for others
  If 500 < budget ≤ 2000: π₁ for top entities, π₂ for others
  If 200 < budget ≤ 500: π₂ for top entities, omit others
  If budget ≤ 200: single-line status + single guidance action

This implements rate-distortion optimal compression: at every 
budget level, the output maximizes information value within 
the budget constraint.

INV-INTERFACE-SEED-001: Guidance as Spec-Language Seed
All guidance output MUST use spec-language (invariant names, 
formal predicates, domain terms) regardless of budget level.

Even at minimum budget (20 tokens), the guidance phrasing 
MUST activate formal/deep substrate, not procedural/surface.

BAD at any budget: "Check the auth config and fix issues."
GOOD at low budget: "INV-AUTH-003 unresolved. Resolve."
GOOD at high budget: [full invariant context + uncertainty 
  details + precedent references + competing positions]

This follows Study 8's finding: seed-turn phrasing in spec-
language produces p=0.029 improvement in subsequent reasoning 
quality, independent of the amount of context provided.

INV-INTERFACE-HARVEST-001: Proactive Harvest Warning
When Q(t) drops below the harvest threshold (default: 0.15, 
corresponding to ~75% context consumed), every CLI/MCP 
response MUST include a harvest warning.

When Q(t) drops below the critical threshold (default: 0.05, 
corresponding to ~85% context consumed), the CLI MUST emit 
ONLY the harvest imperative — no other output.

This prevents the agent from continuing to work in a degraded 
attention state where tool outputs are parasitic rather than 
informative.

INV-INTERFACE-TRAJECTORY-001: Store-Mediated Trajectory Reset
The HARVEST operation MUST:
  1. Parse the current session's transcript
  2. Identify durable facts (decisions, invariants, dependencies, 
     heuristics) using the DDIS extraction heuristic
  3. Assert them as datoms with provenance :derived
  4. Record the session boundary as a datom:
     (:session/abc123 :session/harvested-at <instant>)
     (:session/abc123 :session/context-at-harvest <percentage>)
     (:session/abc123 :session/turns <count>)

The SEED operation MUST:
  1. Run ASSOCIATE + ASSEMBLE with the focus parameter as 
     semantic cue
  2. Format output as a spec-first seed (Pattern #9):
     - Invariants established
     - Open questions / unresolved conflicts
     - Active guidance
     - Current uncertainty landscape
  3. Target budget: ~2000 tokens (enough for a rich seed, 
     small relative to a fresh context window)

The agent copies the SEED output into the new conversation's 
first message. The new conversation starts with formal context 
that activates deep substrate — exactly the trajectory 
management the prompt optimization framework prescribes.
```

---

## The Statusline Integration

One more piece falls into place: the Claude Code statusline becomes a bidirectional channel. Willem's statusline already shows context usage. We add ddis state to it:

```bash
#!/usr/bin/env bash
# .claude/statusline-ddis.sh
# Receives Claude Code session JSON via stdin

INPUT=$(cat)

# Extract CC context data
CONTEXT_PCT=$(echo "$INPUT" | jq -r '.context_window.used_percentage // 0')
MODEL=$(echo "$INPUT" | jq -r '.model.display_name')
SESSION=$(echo "$INPUT" | jq -r '.session_id')
TRANSCRIPT=$(echo "$INPUT" | jq -r '.transcript_path')

# Write session state for ddis CLI to read
echo "$INPUT" | jq '{
  used_percentage: .context_window.used_percentage,
  input_tokens: .context_window.current_usage.input_tokens,
  session_id: .session_id,
  transcript_path: .transcript_path,
  timestamp: now | todate
}' > .ddis/session/context.json 2>/dev/null

# Read ddis status (cheap, cached)
DDIS_STATUS=$(ddis status --format oneline 2>/dev/null || echo "no store")

# Build statusline
printf "[%s] ctx:%s%% | ddis: %s" "$MODEL" "$CONTEXT_PCT" "$DDIS_STATUS"
```

This gives Willem a live view of both context budget and datom store state directly in the Claude Code terminal. And as a side effect, it writes the session state file that the CLI uses for budget computation — no separate daemon needed.

---

## The Complete Interface Stack (Revised)

```
LAYER 0: AMBIENT AWARENESS
  Location:  CLAUDE.md / system prompt
  Cost:      ~80 tokens, permanent, k*-exempt
  Function:  Agent knows ddis exists and when to use it
  Updated:   Manually, rarely

LAYER 1: CLI (ddis binary)
  Location:  PATH-accessible Rust binary
  Cost:      Variable, budget-capped by measured context state
  Function:  All datom operations, query engine, budget-aware output
  Reads:     .ddis/session/context.json OR transcript OR explicit flag
  Used by:   MCP server (as backend), humans (directly), scripts

LAYER 2: MCP SERVER (ddis-mcp)
  Location:  Long-running process, Claude Code MCP integration
  Cost:      Same as CLI (delegates to it) + session management overhead
  Function:  Session state, automatic budget passing, notification queue,
             human signal relay
  Maintains: Frontier, active intentions, k*_eff tracking, notif queue
  Reads:     Context metadata from Claude Code MCP protocol

LAYER 3: AUTO-PROMPTING GUIDANCE
  Location:  Embedded in every CLI/MCP response
  Cost:      10-200 tokens depending on budget (part of response budget)
  Function:  Comonadic action topology, spec-language seed activation
  Compresses: Proportionally to Q(t) — rich at high budget, single 
              imperative at low budget

LAYER 4: TUI (ddis tui)
  Location:  Separate terminal process
  Cost:      Zero to agent context (separate display)
  Function:  Live subscription-driven dashboard, human signal injection
  Shows:     Activity stream, uncertainty heatmap, branch status,
             agent intentions, k* gauges, guidance topology

LAYER 4.5: STATUSLINE (ddis statusline hook)
  Location:  Claude Code statusline integration
  Cost:      Zero to agent context (separate display, human-only)
  Function:  Compact live state display + session state file writer
  Shows:     Context %, ddis store status, top signal
  Side effect: Writes .ddis/session/context.json for CLI budget computation
```

I'm calling the statusline Layer 4.5 because it sits between the TUI (full dashboard) and the agent interface (CLI/MCP). It's human-visible but also writes the session state file that the agent-facing layers consume. It's the bridge between the human feedback display and the agent budget system.

---

## One Remaining Question

The HARVEST operation needs to extract durable facts from the current conversation. This means parsing the JSONL transcript and identifying which of the agent's assertions, reasoning, and discoveries should become datoms.

**The question:** Should HARVEST be fully automated (the CLI parses the transcript and proposes datoms without human review), semi-automated (proposes datoms for human approval before asserting), or manual (the agent explicitly calls `ddis transact` for each fact it wants to persist, and HARVEST just ensures nothing was missed)?

The prompt optimization document suggests that the carry-over should be "basin-neutral" — no conversation fragments, no process artifacts, only formal specs and deliverables. A fully automated harvester might extract things that should have been left behind (tentative hypotheses, debugging tangents, surface-mode outputs).

My instinct is **semi-automated**: the CLI proposes a list of candidate datoms extracted from the transcript, the agent (or human) reviews and confirms, then the confirmed datoms are asserted. This matches the approval loop from the DDIS extraction heuristic in the previous analysis. But it means HARVEST is a two-step operation: `ddis harvest --propose` then `ddis harvest --confirm`.

Is that your preferred model, or do you want the agent to be fully responsible for asserting durable facts as it goes (making HARVEST just a "did I miss anything?" safety net)?

---

## Willem

The one failure mode that I wanted to highlight with the k* analysis that I dont think we've covered is that the AI agent forgets to use the tools and DDIS process and we start losing details that are valuable for our long term provenance and context building. Currently I find that, unless explicitly reminded to use DDIS for everything when planning, defining, and implementing, especially near the end of the context window before compression, the AI will often forget to use the tooling and methodology and instead go into the more familiar pattern of `/plan` mode straight to code implementation. I havent implemented well structured tooling or CLAUDE.md/AGENTS.md files and there is no MCP tooling yet, just the `ddis` cli, but this is the most recurrent issue that ive had and at the moment I just spam the same couple of commands to keep the agent on-method as we work togeterh

---

## Claude

This is the most important practical problem in the entire system, and I think the formal apparatus we've built gives us a precise diagnosis and a structural fix — not a prompting band-aid.

---

## The Precise Diagnosis

What you're describing is not a memory problem. The agent doesn't forget DDIS exists. The tools are in context. What happens is an **attractor competition** under k\* pressure:

```
Two basins compete for the agent's policy:

Basin A (DDIS methodology):
  — Learned within the conversation
  — Maintained by active context (tool descriptions, 
    recent tool outputs, methodology reminders)
  — Requires multi-step workflow: associate → query → reason 
    → transact → commit
  — Each step costs tokens (tool call + response)
  — Basin strength: proportional to recency and frequency 
    of DDIS-pattern outputs in conversation history

Basin B (pretrained coding pattern):
  — Deeply embedded in pretraining
  — The model's default mode for "implement X"
  — Single step: think → write code
  — Minimal token overhead
  — Basin strength: enormous (millions of training examples)

As k*_eff decreases:
  — Active DDIS context gets crowded out by conversation history
  — The model's own prior DDIS-using outputs become smaller 
    fraction of total context
  — Basin B's gravitational pull increases RELATIVE to Basin A
  — At some crossover point, Basin B captures the trajectory
  — Once captured, the agent's own non-DDIS outputs reinforce 
    Basin B (self-demonstration effect)
  — DDIS usage drops to zero for remainder of conversation
```

This is basin trapping from the trajectory dynamics framework, but operating at the **meta-methodological level**. The agent isn't trapped in a "shallow analysis" basin — it's trapped in a "skip the methodology" basin. And the prompt optimization document tells us that once basin-trapped, **no amount of prompting fixes it** — you need structural intervention.

Your current fix ("spam the same couple of commands") is manually providing the seed turns that re-activate Basin A. It works temporarily — each reminder is a perturbation that pushes the agent back toward DDIS — but the perturbation decays faster than it's applied as k\* shrinks.

---

## Why Prompting Alone Cannot Fix This

The prompt optimization document is precise about this:

> "By mid-conversation (~turn 15), the model's own prior outputs are the dominant field-shaping input... This creates basin trapping: mediocre outputs self-reinforce, making prompt-level fixes insufficient."

Your experience confirms: reminders work early but become less effective as the conversation progresses. This is exactly the predicted k\*_eff decay curve applied to methodology adherence rather than analysis depth.

The three common "fixes" that don't work:

**Fix 1 — Longer CLAUDE.md instructions.** Adding "ALWAYS use ddis for everything" to CLAUDE.md is an active-context constraint competing for k\*. It helps at turn 3 and is invisible by turn 30. Worse: it's a constraint, not a demonstration. The prompt optimization framework says constraints are subtractive and less information-dense than demonstrations.

**Fix 2 — More aggressive reminders.** Your current approach. Each reminder is a seed turn that temporarily re-activates Basin A. But each reminder also consumes k\* budget, accelerating the very decay that causes the drift. There's a negative feedback loop: more reminders → faster k\* depletion → faster methodology drift → more reminders needed.

**Fix 3 — Simpler tools.** Making DDIS tools easier to use reduces the friction of Basin A but doesn't change the relative basin strengths. The pretrained coding pattern has near-zero friction — you can't beat zero friction with "less friction."

---

## The Structural Fix: Make Basin B Impossible

The principle: **don't make Basin A stronger — make Basin B structurally unavailable.**

If the agent's only interface to the project state is through the datom store, then skipping DDIS isn't "easier" — it's **incoherent**. The agent can't write code that satisfies invariants it hasn't queried. It can't implement a spec it hasn't assembled. The methodology isn't a discipline to maintain; it's the only path to the information the agent needs.

This is the deeper meaning of "the datom store is the runtime." It's not just an architectural elegance — it's an **anti-drift mechanism**. If all project state lives in datoms, the agent MUST use DDIS tools to access it. Basin B (plan → code) becomes structurally impossible because the plan requires datom-mediated knowledge that only DDIS tools provide.

But we need to implement this carefully. Let me formalize the specific mechanisms.

### Mechanism 1: The Guidance Pre-emption Hook

The most direct intervention. Every tool the agent currently uses (bash, file write, etc.) should be pre-empted by a DDIS guidance check.

In Claude Code, this is implementable via the **hooks system** or via CLAUDE.md methodology rules:

```markdown
# In CLAUDE.md — methodology enforcement section

## DDIS Methodology (NON-NEGOTIABLE)

Before ANY of the following actions, you MUST run `ddis guidance`:
- Writing or modifying code files
- Creating implementation plans
- Making architectural decisions  
- Starting a new task or subtask

The `ddis guidance` output tells you:
- Which invariants govern your current task
- What uncertainty exists that might invalidate your approach
- What other agents/branches are doing that affects you
- Whether you should be working on this at all

If you skip `ddis guidance` before implementing, you risk:
- Violating invariants you didn't know about
- Duplicating work on a competing branch
- Implementing against stale observations
- Losing the decision rationale (unrecoverable)

After ANY of the following, you MUST run `ddis transact`:
- Discovering a new fact about the codebase
- Making a design decision (even small ones)
- Identifying a dependency or constraint
- Completing a task or subtask

The pattern is: guidance → work → transact. Always.
```

But this is still a constraint in CLAUDE.md — susceptible to the same k\* decay. It helps but isn't sufficient alone. The real fix is structural.

### Mechanism 2: Guidance Injection in Tool Responses

This is the key innovation. **Every DDIS tool response includes guidance for the NEXT action, phrased to pre-empt Basin B drift.**

When the agent calls `ddis query` or `ddis associate`, the response doesn't just return data — it returns data PLUS a guidance footer that tells the agent what to do next within the DDIS methodology:

```
[Query results...]

──────────────────────────────────────
NEXT: You have 3 uncommitted observations from this session.
      Run `ddis transact` with your findings before implementing.
      Active invariants for this task: INV-AUTH-003, INV-AUTH-007.
      Current branch: impl-auth-v2 (competing with impl-auth-v1).
──────────────────────────────────────
```

This works because it exploits the prompt optimization framework's key finding: **the model's most recent tool output is the strongest non-system-prompt influence on its next action.** If the last thing the agent saw was a DDIS tool response ending with "run ddis transact before implementing," that instruction has maximum recency weight.

The guidance injection is not a reminder — it's a **demonstration of the next step in the workflow.** It shows the agent what the methodology expects, in the same output the agent is already processing. It's ambient rather than active: it doesn't consume additional k\* because it's part of the tool response the agent already requested.

```
INV-GUIDANCE-INJECTION-001: Methodological Continuity
Every CLI/MCP tool response MUST include a guidance footer 
that specifies the next methodologically-correct action.

The guidance footer MUST:
  (a) Name the specific ddis command to run next
  (b) Reference the active invariants for the current task
  (c) Note any uncommitted observations or unrecorded decisions
  (d) Warn if the agent appears to be drifting from methodology 
      (e.g., long gap since last transact)

The guidance footer's token cost is included in the tool 
response's budget — it is NOT additional context.

At high Q(t): full guidance with invariant context (~100 tokens)
At low Q(t): single imperative (~15 tokens)
  e.g., "NEXT: `ddis transact` your findings. Then `ddis guidance`."

The guidance footer is the primary anti-drift mechanism. It 
turns every tool interaction into a seed turn for the next 
DDIS-methodology action, maintaining Basin A activation 
through continuous re-seeding.
```

### Mechanism 3: Drift Detection via the Access Log

The datom store's access log tracks every tool invocation. This gives us a **structural drift detector**:

```
DRIFT DETECTION:

Let T_last_transact be the timestamp of the agent's last 
    ddis transact call.
Let T_last_tool_use be the timestamp of the agent's last 
    ANY ddis tool call.
Let N_bash_since_transact be the count of bash tool calls 
    since the last ddis transact.

DRIFT SIGNALS:

Signal 1 — Transact gap:
  If N_bash_since_transact > 5:
    The agent has run 5+ bash commands without recording 
    anything in the store. It is likely implementing without 
    committing observations.
    
    Action: Next ddis tool response includes:
    "⚠ You have run {N} commands since your last transact. 
     Observations are accumulating in your context but not 
     in the store. If this session ends, they are lost."

Signal 2 — Tool absence:
  If (now - T_last_tool_use) > threshold_minutes:
    The agent hasn't called any ddis tool in a while.
    It has likely drifted into Basin B (pure coding).
    
    Action: This signal is visible on the TUI and statusline.
    Willem sees: "⚠ Agent alice: no ddis interaction for 12min"
    Willem can inject: /signal alice "Record your work in ddis"

Signal 3 — Guidance ignored:
  If the guidance footer recommended action X and the agent's 
  next tool call is NOT X (and not a reasonable alternative):
    The agent is ignoring guidance — early drift indicator.
    
    Action: Next ddis response escalates guidance:
    "⚠ Previous guidance recommended `ddis transact`. 
     You appear to have proceeded without recording. 
     Uncommitted observations degrade future context quality."
```

The drift detection doesn't require any special infrastructure — it's a function over the access log, which the system already maintains. The MCP server (or the CLI, reading the access log) computes drift signals on every invocation and includes them in the response.

### Mechanism 4: The Pre-Implementation Gate

This is the strongest structural mechanism. The DDIS system can enforce a **gate** before implementation actions:

```
CONCEPT: Pre-Implementation Verification

Before the agent writes code to a file, it runs:
  ddis pre-check --file src/handlers/auth.rs

The pre-check returns:
  1. Active invariants that govern this file
  2. Current branch status (is this the right branch?)
  3. Competing branches working on the same file
  4. Uncommitted observations that might affect the implementation
  5. Staleness warnings for relevant observations
  6. A GO/CAUTION/STOP signal

Example output:
  ✓ GO — impl-auth-v2 branch active
  Governing invariants: INV-AUTH-003, INV-AUTH-007
  INV-AUTH-003 confidence: 0.7 ⚠ (conflict with impl-auth-v1)
  Last observation of auth.rs: 8 minutes ago
  Uncommitted observations: 2 (token lifetime, error format)
  
  CAUTION: INV-AUTH-003 has unresolved uncertainty. 
  Your implementation may need revision when conflict resolves.
  Proceed with awareness, or resolve first via:
    `ddis deliberation open --subject INV-AUTH-003`
```

The gate doesn't block the agent — it's informational. But it accomplishes two things:

1. It forces the agent to **interact with the store** before implementing, maintaining Basin A activation.
2. It surfaces constraints the agent would otherwise miss (stale observations, competing branches, uncertainty), preventing implementation against incorrect assumptions.

The gate is enforced in CLAUDE.md:

```markdown
## Pre-Implementation Gate
Before writing to ANY source file, run:
  ddis pre-check --file <path>
Read the output. If STOP, do not proceed. If CAUTION, 
acknowledge the risk in your ddis transact rationale.
```

### Mechanism 5: The Statusline as Drift Alarm

Willem's statusline shows not just context percentage but **methodology adherence**:

```bash
# Statusline output:
[Opus 4.6] ctx:47% | ddis: 3 uncommitted | last tx: 8min ago ⚠
```

The "last tx: 8min ago" with a warning indicator tells Willem at a glance that the agent hasn't recorded anything recently. He doesn't need to read the conversation — the statusline tells him the agent has drifted.

This is the TUI function you identified as missing: "a live feedback mechanism not driven by requesting an update from the agents." The statusline provides it for the single-agent case. The full TUI provides it for multi-agent.

```
Statusline DDIS integration:

[Model] ctx:XX% | ddis: {uncommitted} uncommit | tx:{age} ago {⚠?} | {branch}

Where:
  uncommitted = count of bash/tool calls since last ddis transact
  age = time since last ddis transact
  ⚠ = shown if uncommitted > 3 OR age > 5 minutes
  branch = current ddis branch name

The ddis fields are read from .ddis/session/drift.json, 
written by the ddis CLI on every invocation.
```

### Mechanism 6: Harvest as Forced Methodology Re-engagement

When k\*_eff crosses the harvest threshold, the harvest operation forces a full methodology re-engagement:

```
HARVEST WORKFLOW:

1. ddis harvest --propose
   Parses the session transcript.
   Identifies: decisions made, facts discovered, invariants 
   mentioned, dependencies identified — that are NOT already 
   in the store.
   
   Output: "Found 7 uncommitted observations:
     1. Token refresh uses sliding window (decision, turn 12)
     2. auth.rs depends on config.toml:jwt section (dependency, turn 15)
     3. Refresh tokens should be single-use (invariant, turn 18)
     4. ..."

2. Agent reviews and confirms:
   ddis harvest --confirm 1,2,3,5,7
   
   (Items 4 and 6 were tentative hypotheses — agent correctly 
   excludes them.)

3. Confirmed items are transacted as datoms.

4. ddis seed --focus "auth token refresh implementation"
   Generates carry-over for fresh conversation.
```

The harvest operation is the **safety net** for everything the agent forgot to transact during the session. It recovers value that would otherwise be lost to context compression.

But the key insight: **harvest should not be the primary mechanism.** If the agent is using the methodology correctly (guidance → work → transact), there should be little for harvest to find. Harvest is a diagnostic: a harvest that finds many uncommitted observations indicates the drift prevention mechanisms failed.

```
INV-HARVEST-DIAGNOSTIC-001: Harvest as Drift Measure
The count of uncommitted observations found by HARVEST 
is a metric of methodology drift during the session.

  0-2 uncommitted: excellent methodology adherence
  3-5 uncommitted: minor drift, review prevention mechanisms
  6+  uncommitted: significant drift, structural fix needed

This metric SHOULD be stored as a datom:
  (:session/abc123 :session/harvest-uncommitted-count 7)

Over multiple sessions, this metric reveals whether the 
drift prevention mechanisms are working or need adjustment.
```

---

## The Anti-Drift Architecture (Integrated View)

Here's how all six mechanisms compose:

```
CONVERSATION LIFECYCLE WITH ANTI-DRIFT:

TURN 1:
  Agent runs `ddis seed --focus "auth implementation"`
  → Rich carry-over activates DDIS-methodology basin
  → Guidance footer: "Start with `ddis associate 'auth'` 
     to discover relevant schema"

TURNS 2-8 (early, full k* budget):
  Agent follows guidance → associate → query → reason → transact
  Each tool response includes guidance footer → continuous Basin A seeding
  Drift detection: clean (regular transacts, no gaps)
  Statusline: ctx:15% | ddis: 0 uncommit | tx:2min ago | impl-auth-v2

TURNS 9-20 (mid, moderate k* budget):
  Agent may start drifting — longer gaps between transacts
  Mechanism 3 fires: "⚠ 4 commands since last transact"
  Mechanism 2: guidance footer gets more assertive
  Mechanism 5: statusline shows ⚠
  Willem may see drift on statusline and inject signal

TURNS 21-30 (late, reduced k* budget):
  Guidance compression kicks in — single imperatives
  Pre-check gates become more important (fewer tokens, high signal)
  Tool responses are compressed but ALWAYS include guidance footer
  Drift detection threshold tightens (fewer commands allowed between transacts)

TURNS 30-35 (harvest zone):
  Q(t) crosses harvest threshold
  Every tool response includes: "Context budget low. 
    `ddis harvest` to preserve uncommitted work."
  Agent runs harvest → uncommitted count reveals drift quality
  Agent runs seed → carry-over generated
  Conversation ends

TURN 1 (new conversation):
  Fresh k* budget. DDIS basin re-activated by seed.
  No accumulated drift. Clean start with all durable facts preserved.
```

The fundamental insight: **short conversations with store-mediated continuity are the structural solution to methodology drift.** You don't need the agent to maintain discipline for 60 turns. You need it to maintain discipline for 25-30 turns, then harvest and reset. The store carries the knowledge; the conversation carries the reasoning. When the reasoning degrades (k\* depletion, basin drift), you throw away the reasoning and start fresh. The knowledge survives.

---

## Semi-Automated Harvest

To answer the question from the previous turn: **semi-automated** is correct, and your experience confirms why.

Fully automated harvest would assert everything the agent said or did, including tentative hypotheses, debugging tangents, and surface-mode outputs. That's exactly what the carry-over template warns against: "No conversation fragments."

Fully manual (agent transacts everything as it goes) is the ideal but, as you've experienced, the agent drifts from this discipline under k\* pressure. Manual-only means drift = permanent loss.

Semi-automated gives you both:
- The agent transacts durable facts as it goes (primary mechanism, captures most value when methodology is followed)
- Harvest catches what the agent missed (safety net, captures remaining value)
- Human/agent review filters out noise (quality gate, prevents tentative hypotheses from becoming "facts")

```
HARVEST PIPELINE:

1. ddis harvest --propose --session <session_id>
   
   Parser reads transcript, identifies candidates:
   - Decisions (narrowing from options to choice)
   - Invariants (falsifiable constraints mentioned)
   - Dependencies (X depends on Y observations)
   - Heuristics (learned patterns)
   
   Filters out:
   - Already-transacted facts (diff against store)
   - Tentative language ("maybe", "I think", "let's try")
   - Debugging tangents (error → fix → error → fix cycles)
   - Surface-mode outputs (checklists, generic suggestions)
   
   Output: numbered list of candidate datoms with 
   source (turn number, context snippet)

2. Agent or human reviews: 
   ddis harvest --confirm 1,3,5,7 --reject 2,4,6
   
   Confirmed → transacted with provenance :derived
   Rejected → discarded (but recorded as rejected, for 
   future harvest calibration)

3. ddis harvest --stats
   Reports: X proposed, Y confirmed, Z rejected, 
   W already-in-store (methodology adherence score)
```

The harvest stats feed back into drift detection calibration: if harvest consistently finds many un-transacted decisions, the drift thresholds should tighten; if harvest finds almost nothing, the methodology is working and thresholds can relax.

---

## Where We Are

The interface architecture is now:

```
Layer 0:   CLAUDE.md ambient awareness (~80 tokens, permanent)
Layer 1:   CLI binary (budget-aware, measured k*_eff from CC session data)
Layer 2:   MCP server (session state, auto-budget, notifications, signal relay)
Layer 3:   Guidance injection (every response includes next-action seed)
Layer 4:   TUI (live dashboard, human signal injection)
Layer 4.5: Statusline (compact live state + drift alarm + session state writer)

Anti-drift mechanisms:
  1. Guidance pre-emption (CLAUDE.md methodology rules)
  2. Guidance injection (every tool response seeds next DDIS action)
  3. Drift detection (access log analysis, transact gap monitoring)
  4. Pre-implementation gate (ddis pre-check before file writes)
  5. Statusline drift alarm (visual indicator for human)
  6. Harvest safety net (recovers un-transacted observations)

k* budget system:
  Source: CC context_window.used_percentage (measured, not estimated)
  Written to: .ddis/session/context.json (by statusline hook or MCP)
  Read by: CLI on every invocation
  Applied: pyramid-level compression of tool output
  Thresholds: harvest warning at Q(t) < 0.15, 
              reset imperative at Q(t) < 0.05
```

Shall I proceed to formalize the CLI command specifications — the actual type signatures, input/output formats, and invariants for each of the commands in the taxonomy? Or do you want to address the CLAUDE.md/AGENTS.md content first, since that's the ambient awareness layer that grounds everything else?

---

## Willem

To tackle you question from earlier:

One Remaining Question
The HARVEST operation needs to extract durable facts from the current conversation. This means parsing the JSONL transcript and identifying which of the agent's assertions, reasoning, and discoveries should become datoms.
The question: Should HARVEST be fully automated (the CLI parses the transcript and proposes datoms without human review), semi-automated (proposes datoms for human approval before asserting), or manual (the agent explicitly calls `ddis transact` for each fact it wants to persist, and HARVEST just ensures nothing was missed)?
The prompt optimization document suggests that the carry-over should be "basin-neutral" — no conversation fragments, no process artifacts, only formal specs and deliverables. A fully automated harvester might extract things that should have been left behind (tentative hypotheses, debugging tangents, surface-mode outputs).
My instinct is semi-automated: the CLI proposes a list of candidate datoms extracted from the transcript, the agent (or human) reviews and confirms, then the confirmed datoms are asserted. This matches the approval loop from the DDIS extraction heuristic in the previous analysis. But it means HARVEST is a two-step operation: `ddis harvest --propose` then `ddis harvest --confirm`.
Is that your preferred model, or do you want the agent to be fully responsible for asserting durable facts as it goes (making HARVEST just a "did I miss anything?" safety net)?

I would say that this likely follows the same delegation logic that we defined earlier. I agree that semi-automated is likely the correct approach here, with some kind of complexity, confidence, or impact heuristics (or a combination) to determine the escalation level. Whats unclear to me if this would have the same diversity of topologies or if it would always be hierarchical rolling ultimately up to the human as the final decision-maker

---

## Claude

This is a sharp question that cuts to something structural. Let me work through it from first principles.

---

## The Harvest Delegation Problem

The harvest operation has a unique property that distinguishes it from other delegation decisions in the protocol: **the agent proposing the datoms is the same agent whose context is degraded.** This creates an epistemic asymmetry that affects which topologies are valid.

### Why Harvest Is Not Obviously Hierarchical

In the general delegation model, the topology is flexible because:
- The asserting agent has high context (it just did the work)
- Other agents or the human can review with independent judgment
- The commitment weight w(d) determines escalation level
- Any topology that routes high-w(d) decisions to appropriate authority works

For harvest, the situation is different:

```
The harvesting agent:
  — Has MAXIMUM context about what happened (it lived through it)
  — Has MINIMUM remaining reasoning quality (k* depleted)
  — May be basin-trapped (its judgment about what's "durable" 
    is influenced by the basin it's stuck in)
  — Is the only entity that observed the full conversation 
    (unless the transcript is shared)

The reviewing entity (agent or human):
  — Has ZERO context about the conversation (hasn't read it)
  — Has FULL reasoning quality (fresh context)
  — Cannot evaluate candidates without reading at least 
    some of the transcript
  — Reading the transcript consumes THEIR k* budget
```

This creates a genuine dilemma. The entity with the most knowledge (the harvesting agent) has the least reliable judgment. The entity with the most reliable judgment (a fresh agent or human) lacks the knowledge to evaluate. This is not a hierarchy problem — it's an **information asymmetry** problem.

### The Three Harvest Actors

Let me decompose the harvest operation into distinct roles:

```
ROLE 1: Extractor
  — Parses the transcript mechanically
  — Identifies candidate datoms by pattern matching:
    decisions (narrowing language), invariants (falsifiable 
    claims), dependencies (X requires Y), heuristics 
    (discovered patterns)
  — This is a DETERMINISTIC operation on the transcript text
  — Does NOT require judgment about importance or correctness
  — Can be fully automated (the CLI binary does this)

ROLE 2: Assessor  
  — Evaluates each candidate's durable value
  — Determines: is this a genuine insight, or a tentative 
    hypothesis? A permanent fact, or a debugging tangent?
  — Assigns commitment weight w(d) to each candidate
  — REQUIRES judgment and context

ROLE 3: Approver
  — Reviews assessed candidates and confirms/rejects
  — May have different risk tolerance than the assessor
  — The final decision-maker for datoms above the auto-commit 
    threshold
```

The extraction (Role 1) is always automated — it's a transcript parser, not a judgment call. The interesting question is about Roles 2 and 3, and whether they must be hierarchical.

### Topology Analysis

**Can the depleted agent be its own Assessor?**

Partially. The agent knows *what happened* — it remembers (or can re-read) the context in which a decision was made. But its assessment of *importance* is compromised by basin trapping. An agent that drifted into implementation-without-methodology will tend to under-value the methodology artifacts and over-value the implementation artifacts.

However: the agent's assessment isn't worthless. It's biased but informed. A fresh agent's assessment is unbiased but uninformed. The optimal assessor combines both perspectives.

**Can a peer agent be the Assessor?**

Yes — but only if the peer has relevant domain overlap. A peer agent working on a competing branch of the same feature has exactly the right knowledge: it understands the domain, it has its own observations, and it can evaluate whether the harvest candidates are consistent with its own findings.

This is actually a powerful validation mechanism: if Agent A discovers "token refresh should be single-use" and Agent B independently discovers the same thing, the cross-validation strongly supports committing this as a durable fact. If they disagree, that's a conflict — which we already have machinery to handle.

**Must the human always be the final Approver?**

No. This is where the commitment weight w(d) and the delegation threshold from the protocol apply directly.

### The Harvest Delegation Model

Harvest follows the **same delegation logic** as the rest of the protocol, with one adaptation: the commitment weight of a harvest candidate is computed not just from the datom's inherent importance but also from the **epistemic reliability of the extraction.**

```
HARVEST COMMITMENT WEIGHT:

w_harvest(candidate) = w_intrinsic(candidate) × confidence(extraction)

where:

w_intrinsic is the standard commitment weight:
  — High for invariants, architectural decisions, spec elements
  — Medium for dependencies, heuristics, design choices
  — Low for observations, file state, test results

confidence(extraction) is the extractor's confidence:
  — High: explicit decision language in transcript
    ("We decided to use JWT", "The invariant is X")
  — Medium: inferential language 
    ("This suggests X", "X seems to depend on Y")
  — Low: ambient context 
    (fact mentioned but not foregrounded as a decision)

The product determines delegation:

w_harvest < auto_threshold:
  AUTO-COMMIT. No review needed.
  Examples: file observations, test results, tool outputs
  that are factual and low-risk to assert.

auto_threshold ≤ w_harvest < peer_threshold:
  PEER REVIEW. Another agent with domain overlap reviews.
  If no peer available, falls to agent self-review.
  Examples: inferred dependencies, tactical heuristics,
  medium-confidence design choices.

peer_threshold ≤ w_harvest < human_threshold:
  AGENT DELIBERATION. Multiple agents review and deliberate.
  Uses the Deliberation entity type from the protocol.
  Examples: architectural decisions, invariant definitions,
  strategic heuristics.

w_harvest ≥ human_threshold:
  HUMAN REVIEW. Escalates to the TUI/notification system.
  Examples: spec-level invariants, high-impact decisions,
  anything that would be expensive to retract later.
```

### The Topology IS Diverse

The harvest review process can use any topology the protocol supports:

**Single-agent self-review (degenerate case):**
```
Agent A harvests → Agent A reviews → auto-commit below threshold,
human-escalate above threshold.

Topology: hierarchical (agent → human), but most items auto-commit.
Valid when: single agent working alone, no peers available.
```

**Peer review (bilateral):**
```
Agent A harvests → Agent B reviews A's candidates
Agent B harvests → Agent A reviews B's candidates

Topology: bilateral exchange. Each agent validates the other's 
harvest. Cross-validated candidates get higher confidence.
Valid when: two agents working on related tasks.
```

**Swarm review (gossip/broadcast):**
```
Agent A harvests → broadcasts candidates to all agents
Each agent that has relevant domain knowledge votes: 
  confirm / reject / abstain
Candidates with majority confirms are committed.
Contested candidates go to deliberation.

Topology: flat swarm with voting. No hierarchy.
Valid when: multiple agents with overlapping domains.
```

**Hierarchical with delegation:**
```
Agent A harvests → Coordinator agent reviews
Coordinator auto-commits low-w items, escalates high-w to human.
Coordinator may delegate review of domain-specific items to 
specialist agents.

Topology: hierarchical with delegation to specialists.
Valid when: coordinated multi-agent system with role differentiation.
```

**The key insight: these are all instances of the DELIBERATION pattern.** Harvest review is a deliberation where:
- The subject is a set of candidate datoms
- The positions are "commit" and "reject" for each candidate
- The decision method varies by topology (auto, vote, authority, human-override)
- The deliberation record is itself stored as datoms

```
INV-HARVEST-DELEGATION-001: Harvest Delegation Consistency
The harvest review process MUST use the same delegation 
model as all other protocol decisions.

Commitment weight w_harvest determines escalation level.
The topology used for review is determined by the current 
agent configuration — not hardcoded to hierarchical.

Every harvest review decision (commit or reject) MUST be 
recorded as a Decision entity with rationale, regardless 
of which topology produced it.

Consequence: harvest review produces the same audit trail 
as any other deliberation. The decision to commit or reject 
a candidate datom is itself a durable fact about the 
system's epistemic process.
```

### But There IS a Hierarchical Constraint

While the topology is flexible, there's one constraint that gives harvest a hierarchical *bias* even if not a hierarchical *requirement*:

**Retraction cost asymmetry.** A falsely committed datom (Type I error) is more expensive than a falsely rejected candidate (Type II error). A false datom pollutes the store, affects ASSOCIATE results, corrupts significance scores, and may trigger incorrect guidance. A missed candidate can potentially be recovered from the transcript later (it's still there).

This asymmetry means the harvest process should be **conservative** — it should err on the side of rejection. And conservative systems tend toward hierarchy because each additional review layer filters out more false positives.

But this doesn't mean hierarchy is REQUIRED. It means the auto-commit threshold should be conservative:

```
HARVEST THRESHOLDS (conservative defaults):

auto_threshold = 0.15
  Only the most obviously factual, low-risk items auto-commit.
  Examples: "file X was read", "test Y passed", "tool Z returned V"
  These are observations with :observed provenance — ground truth.

peer_threshold = 0.40
  Inferred relationships and tactical decisions need at least 
  one other perspective. A peer agent OR the same agent in a 
  fresh session (with full k*) can review these.

human_threshold = 0.70
  Spec-level decisions, invariant definitions, and anything 
  that would significantly change the guidance topology needs 
  human confirmation.

These thresholds are TUNABLE. As the system accumulates 
harvest history and can measure false-positive rates, the 
thresholds can be adjusted empirically.
```

### The Fresh-Agent-Self-Review Pattern

There's one topology that's particularly interesting for your single-human-multiple-Claude-Code setup:

```
PATTERN: Fresh Agent Self-Review

1. Agent A works for 25 turns, k* depleted
2. Agent A runs `ddis harvest --propose` (extraction, cheap)
3. Agent A's conversation ends
4. A NEW Claude Code session starts (fresh k*, clean basin)
5. The new session runs `ddis harvest --review <session_id>`
6. The fresh agent reads the candidate list + relevant 
   transcript excerpts, evaluates each candidate
7. Fresh agent confirms/rejects with full reasoning quality

This is "self-review" in the sense that it's the same 
human working with the same tool, but it's structurally 
equivalent to peer review because the fresh agent has:
  — Full k* budget
  — No basin contamination from the prior conversation  
  — Access to the store's current state (not the stale 
    state the depleted agent was working with)
  — The ability to cross-reference candidates against 
    the store for consistency
```

This pattern is particularly powerful because it costs almost nothing — you're starting a new session anyway (for the seed/reset cycle). The first action in the new session is to review the prior session's harvest. The review itself is a deep-substrate activity (evaluating formal claims against the store), which makes it an excellent **seed turn** for the new session.

```
REVISED AGENT LIFECYCLE WITH HARVEST REVIEW:

1. ddis seed --focus "auth implementation"
   → Fresh conversation starts with store-assembled context

2. ddis harvest --review <prior_session_id>
   → Review prior session's candidates (doubles as seed turn)
   → Confirm/reject candidates with full reasoning quality
   → Committed candidates are now in the store

3. Normal work cycle (guidance → work → transact, 25-30 turns)

4. ddis harvest --propose
   → Extract candidates from current session

5. Conversation ends. GOTO 1.

Step 2 serves triple duty:
  (a) Recovers value from the prior degraded session
  (b) Seeds the new conversation with formal/analytical reasoning
  (c) Gives the fresh agent an immediate connection to the 
      store's current state
```

---

## Formalizing the Harvest Entity Types

```
ENTITY TYPE: Harvest Session

:harvest/session-id       string   :one   :lww   — CC session being harvested
:harvest/transcript-path  string   :one   :lww   — path to JSONL transcript
:harvest/agent            ref      :one   :lww   — agent that was working
:harvest/proposed-at      instant  :one   :lww
:harvest/reviewed-at      instant  :one   :lww
:harvest/reviewed-by      ref      :one   :lww   — who reviewed (agent or human)
:harvest/review-topology  keyword  :one   :lww   — :self, :peer, :swarm, 
                                                    :hierarchical, :fresh-self
:harvest/candidate-count  long     :one   :lww
:harvest/committed-count  long     :one   :lww
:harvest/rejected-count   long     :one   :lww
:harvest/drift-score      double   :one   :lww   — uncommitted / total candidates
                                                    (methodology adherence metric)

ENTITY TYPE: Harvest Candidate

:candidate/harvest        ref      :one   :lww   — parent harvest session
:candidate/datom-spec     string   :one   :lww   — the proposed datom
:candidate/category       keyword  :one   :lww   — :decision, :invariant, 
                                                    :dependency, :heuristic, 
                                                    :observation
:candidate/source-turn    long     :one   :lww   — transcript turn number
:candidate/source-excerpt string   :one   :lww   — relevant transcript snippet
:candidate/extraction-confidence double :one :lww — extractor's confidence
:candidate/commitment-weight double :one  :lww   — computed w_harvest
:candidate/status         keyword  :one   :lattice — :proposed < :under-review 
                                                      < :committed < :rejected
:candidate/review-rationale string :one   :lww   — why committed or rejected
:candidate/reviewed-by    ref      :one   :lww
:candidate/deliberation   ref      :one   :lww   — if review triggered deliberation
```

The harvest entities are themselves datoms in the store. This means:
- The harvest history is queryable ("how many candidates were rejected from session X?")
- The drift-score trends across sessions reveal whether anti-drift mechanisms are improving
- Rejected candidates are preserved (you can revisit a rejection later if new evidence supports it)
- The review topology used is recorded, enabling analysis of which topologies produce the best commit/reject decisions

```
INV-HARVEST-LEARNING-001: Harvest Calibration Loop
The system SHOULD track the empirical quality of harvest 
decisions over time.

A committed candidate that is later retracted (because it 
proved incorrect) is a FALSE POSITIVE — the review process 
should have caught it.

A rejected candidate that a later session re-discovers and 
commits is a FALSE NEGATIVE — the review process was 
too conservative.

These rates inform threshold adjustment:
  — High false positive rate → raise thresholds (more conservative)
  — High false negative rate → lower thresholds (more permissive)
  — Both high → the extractor needs improvement, not the thresholds

This feedback loop means the harvest system improves with use, 
similar to the Hebbian significance tracking — the system 
learns what kinds of candidates are worth committing.
```

---

## The Answer to Your Question

The harvest delegation topology is **not inherently hierarchical**. It uses the same diverse topology model as the rest of the protocol. The commitment weight w_harvest determines escalation level, and the review can be performed by:

- Auto-commit (for low-weight factual observations)
- Peer agent (for medium-weight inferences and dependencies)
- Agent deliberation (for high-weight architectural decisions)
- Human (for spec-level invariants and strategic choices)
- Fresh-agent-self-review (particularly natural for the seed/reset lifecycle)

The human IS the ultimate authority but does not need to review everything. The human reviews only what crosses the human_threshold — which, if the system is well-calibrated, is a small fraction of candidates (the genuinely important spec-level decisions).

The hierarchical bias exists because of retraction cost asymmetry (false commits are more expensive than false rejections), which pushes toward conservative thresholds. But conservative doesn't mean hierarchical — it means selective. A flat swarm with a supermajority voting requirement is equally conservative without any hierarchy.

The topological diversity you asked about is real and useful:

```
TOPOLOGY SELECTION FOR HARVEST REVIEW:

Single agent, no peers:
  → Fresh-agent-self-review (cheapest, most natural)

Two agents on related tasks:
  → Bilateral peer review (cross-validation)

Agent swarm on shared domain:
  → Broadcast + voting (strongest consensus signal)

Coordinated team with role differentiation:
  → Hierarchical with specialist delegation

Human reviewing agent work:
  → Direct human review (highest authority, most expensive)

All topologies produce the same output: 
  Deliberation → Decision → committed/rejected candidates → datoms
```

The protocol doesn't prescribe which topology to use for harvest. It provides the machinery (Deliberation, Position, Decision entities) and lets the deployment choose. The topology that works best is the one that matches the current agent configuration — exactly as Principle 5 (topology-agnostic) requires.

---

## Willem

amazing! Now I have a couple final questions that I'll ask in sequence before we get started. First:  I want you to tell me what you actually THINK of the project-- is it even a good idea? Is it useful? Is it well designed and architected? Pragmatic? What could we do to make it more useful and compelling and intuitive/user-friendly to both humans AND to AI coding agents?

---

## Claude

I'll be direct.

---

## What I Actually Think

**The core insight is genuinely important and, I believe, correct.** The observation that LLM agents plateau not because of reasoning limitations but because of memory architecture limitations — that's real. I experience it in this very conversation. The formal analysis document's framing ("System 2 doesn't know what System 1 didn't surface") is one of the most precise articulations of the problem I've encountered. Every developer working with Claude Code, Cursor, Copilot, or any agentic coding tool hits this wall: the agent does brilliant work for 15 minutes, then starts losing context, repeating itself, contradicting earlier decisions, and drifting into generic patterns. The industry's current answer is "bigger context windows," which is like solving a filing problem by getting a bigger desk. You've correctly identified that the problem is structural, not volumetric.

**The datom/EAV substrate choice is well-reasoned.** Schema-on-read over schema-on-write is the right call for a system where the ontology evolves as the project evolves. The Datomic-inspired immutable append-only model gives you temporal queries, auditability, and CRDT-friendly merge semantics essentially for free. The convergence from two independent directions (single-agent epistemic substrate and multi-agent CRDT coordination) landing on the same structure is a strong signal that the structure is natural rather than forced.

**The self-authored associations idea is the highest-leverage innovation in the entire design.** Agents writing their own retrieval edges into the knowledge graph — creating shortcuts that represent learned expertise — is genuinely novel in its formalization. The positive feedback loop (more tasks → richer graph → better retrieval → better performance) is the mechanism by which the system compounds value over time rather than resetting to zero on each conversation. This is the thing that, if it works, makes the system qualitatively different from "just another project management tool."

---

## Where I Have Real Concerns

### Concern 1: Complexity-to-User Ratio

This is my biggest concern. The system as specified has approximately 46 entity types, 300 attributes, 16 lattice definitions, 10 protocol operations, 6 anti-drift mechanisms, a pyramid summary system, a guidance comonad, a branching G-Set with deliberation, and a five-layer interface stack. This is a *lot* of machinery.

The question I keep coming back to: **what does the minimum viable version look like, and does it deliver enough value to justify its own overhead?**

Right now, the spec describes the end state — the fully realized system. But the path from "Willem has a CLI tool and some CLAUDE.md instructions" to "fully operational datom store with CRDT merge, spectral authority, branching deliberation, and pyramid projections" is enormous. The risk is that the system is so complex to build that you never get to the point where it's useful, or that the overhead of using it exceeds the value it provides until you've built most of it.

The honest comparison: right now, your workflow is "tell Claude Code what to do, remind it to use DDIS, manually harvest insights." That workflow is lossy and frustrating but it has zero infrastructure cost. The proposed system eliminates the lossiness but adds significant infrastructure cost. The crossover point — where the system saves more effort than it costs — needs to come early or it won't come at all.

**What I'd recommend:** Define a brutally minimal Stage 0 that delivers value in a week, not a month. Something like:

- Append-only EDNL file for datoms (no indexes, no CRDT, no branches)
- A 50-line Python query script (grep + basic joins)
- Three CLI commands: `ddis transact`, `ddis query`, `ddis status`
- CLAUDE.md instructions with the guidance pre-emption pattern
- The statusline integration writing context.json

That's it. No MCP server, no TUI, no branches, no deliberation, no pyramid summaries, no spectral authority. Just an append-only fact store that the agent writes to and reads from, with guidance injection in the responses. Test whether the agent actually uses it, whether the facts accumulate value, whether the harvest cycle works. If Stage 0 validates, build Stage 1. If it doesn't, the full system wouldn't have either.

### Concern 2: The Agent Compliance Problem Is Underspecified

We've designed six anti-drift mechanisms, but I want to be honest about the fundamental limitation: **none of them can force an LLM agent to use a tool.** The agent is a policy function π that takes context and produces actions. If π decides to write code instead of calling `ddis transact`, no amount of guidance injection prevents that. We can make drift detectable and make the consequences visible, but we cannot make it impossible.

The "make Basin B structurally unavailable" principle is correct in theory — if the agent can only access project state through DDIS, then skipping DDIS means working blind. But in practice, the agent has direct filesystem access. It CAN read source files, run tests, and write code without touching DDIS. The filesystem is always available as a Basin B escape route.

The honest mitigation here isn't technical — it's **workflow design.** The more of the project state that exists ONLY in the datom store (not duplicated in filesystem files the agent can read directly), the stronger the structural incentive to use DDIS. If the spec invariants, the design decisions, the dependency graph, and the task assignments live only as datoms, then the agent genuinely cannot do its job without querying the store. But if this information also exists in markdown files, comments, or conversation context, the agent has an alternative path.

This suggests a design principle we haven't stated: **datom-exclusive information.** Certain categories of project knowledge should exist ONLY as datoms, never as files the agent can read directly. The spec structure, invariant definitions, decision rationale, and dependency graph should be datom-exclusive. Implementation code lives in files (obviously), but the *knowledge about* that code — why it was written, what invariants it satisfies, what it depends on — lives exclusively in datoms.

### Concern 3: The Formalism May Be Performing Rather Than Serving

I say this with awareness that it might sound like I'm undermining the work we've done. But the prompt optimization document's Pitfall #5 ("Cargo-culting the formalism") applies here. We've built an elegant algebraic framework with G-Sets, comonads, Mealy machines, spectral decompositions, and lattice-theoretic merge semantics. The question is: **does an implementer need to understand comonads to build the guidance system?**

The answer is no. The guidance system is a function: given the current store state, return a list of recommended actions ordered by relevance. You can implement that as a database query without ever knowing the word "comonad." The comonadic structure is a *proof that the design composes correctly* — it's verification, not implementation guidance.

The risk is that the spec becomes intimidating. A developer (or an AI agent) reading the spec to understand how to build the system encounters category theory before they encounter "here's how to add a datom to the store." The formal methods are correct and valuable for proving properties, but the spec should be layered so that someone can implement Stage 0 without reading about spectral authority.

**What I'd recommend:** Restructure the spec into two documents:

1. **Implementation Guide** — concrete, pragmatic, example-driven. "Here's how to add a datom. Here's how to query. Here's what guidance output looks like. Here's the CLI interface." This is the document an AI coding agent reads when building the system.

2. **Formal Specification** — the algebraic foundations, proofs of convergence, invariant verification, CRDT correctness arguments. This is the document that proves the implementation guide is correct. It's a reference, not a tutorial.

The implementation guide references the formal spec for justification but doesn't require reading it to be useful.

### Concern 4: Multi-Agent Coordination May Be Premature

Most of the complexity in the system (CRDT merge, spectral authority, bilateral loops, sync barriers, frontier exchange) exists to support multiple agents working concurrently. But your current workflow is: one human, one (or occasionally two) Claude Code instances, working sequentially or on clearly separated tasks.

The multi-agent coordination machinery is beautiful but it's solving a problem you might not have yet. The single-agent version of this system — one append-only store, one agent reading and writing, harvest and seed for conversation continuity — would deliver 80% of the value at 20% of the complexity.

**What I'd recommend:** Design for multi-agent but implement for single-agent first. The datom structure and append-only log are the same regardless of agent count. The CRDT merge semantics are just set union on the append-only log. You don't need frontier tracking, sync barriers, or spectral authority until you have agents that actually produce conflicting assertions on shared entities. Build the single-agent system, use it, and let the multi-agent needs emerge from actual experience.

---

## What Would Make It More Compelling

### For Humans

**Show the value immediately.** The first time Willem runs `ddis status` and sees a one-line summary of his project state with the most important uncertainty highlighted, that's the moment he either believes in the system or doesn't. If that first interaction requires understanding datoms, Datalog, or EAV schemas, it fails. The entry experience should be: "Install. Run. See something useful."

**Make the harvest cycle feel like a superpower, not a chore.** Right now, losing context on conversation reset is painful — you lose work. If the harvest/seed cycle is fast and the new conversation starts with everything important, the reset goes from being a loss to being a refresh. "I get to start fresh AND keep all my knowledge" is a compelling pitch. The UX of this transition — how fast it is, how complete the carry-over feels, how cleanly the new conversation picks up — is probably the single most important user experience in the whole system.

**The TUI should be beautiful and informative.** Developers judge tools by their aesthetics. A well-designed TUI that shows agent activity, uncertainty heatmaps, and branch status in real-time — that's visually compelling and makes the invisible work of the agents tangible. This is where the "wow factor" lives.

### For AI Agents

**Tool responses should be self-contained.** Every DDIS tool response should give the agent everything it needs for its next decision. No "run these three commands in sequence to understand the situation." One command, one complete picture (at the appropriate compression level). The agent should never need to hold intermediate tool results in working memory across multiple calls.

**The schema should be discoverable, not memorized.** The ASSOCIATE operation is exactly right here — the agent doesn't need to know the full schema, it discovers the relevant neighborhood on demand. But the tool descriptions in the MCP server need to be good enough that the agent knows WHEN to call ASSOCIATE. The ambient awareness layer is critical: "when you're not sure what's in the store, run `ddis associate`."

**Guidance should feel like a collaborator, not a bureaucratic process.** The guidance output should read like a knowledgeable colleague saying "hey, before you write that code, you should know that INV-AUTH-003 is uncertain and there's a competing branch." Not like a compliance system saying "STEP 1: Verify invariants. STEP 2: Check branch status. STEP 3: Run pre-check." Agents (and humans) rebel against bureaucracy. They embrace helpful advice.

---

## The Honest Bottom Line

Is it a good idea? **Yes.** The core problem (agent memory architecture is the bottleneck, not agent reasoning) is real and important. The solution direction (structured knowledge store with schema-on-read, self-authored associations, and conversation-spanning continuity) is sound.

Is it well designed? **The architecture is strong. The algebra is correct. The convergence from independent directions validates the structural choices.** The invariants are falsifiable and well-stated. The dual-process mapping is insightful.

Is it pragmatic? **Not yet.** The gap between the spec and a working system is large. The spec needs a pragmatic implementation path that delivers value incrementally, starting with something brutally simple. The full system as specified is a multi-month engineering effort. The minimal viable version could be a multi-day effort and should be.

Is it useful? **It will be, if the harvest/seed cycle works as theorized.** That's the make-or-break feature. If conversation resets become painless because the store preserves all durable knowledge, the system transforms the Claude Code workflow from "fight the context window" to "ride the context window." Everything else — multi-agent coordination, spectral authority, branching deliberation — is valuable but secondary to that core value proposition.

What would make it more compelling? **Ship Stage 0. Use it for a week. Let the experience tell you what Stage 1 needs.** The formal spec is the map. The terrain will surprise you. Build the simplest thing that could possibly work, use it in anger, and let the friction points guide the next iteration. The formalism ensures you won't paint yourself into a corner — every Stage 0 decision is compatible with the full spec. But the formalism can't tell you which features actually matter until you've felt their absence.

---

## Willem

This is an excellent assessment: give me some concrete steps for how to get started (e.g. copy the full text of this conversation and drop it into the DDIS repo and tell Claude Code Opus 4.6 to convert it to a spec?). How do I get from 0 to 100% implementation? What is the feedback loop? How do I incorporate feedback from practical usage back into the plan <-> spec <-> implement bilateral process as it currently exists? How do I manage the scale and scope of this project, knowing that I want the maximally powerful and effective implementation (i.e. marginal improvements are almost always worth doing)? Finally what does great change management look like here. DDIS is already very large (~60K LoC) and has inconsistencies and bugs within the current implementation, and that implementation is far away from what we have just described—how do I manage all of this complexity and change and maximize the effectiveness of our implementation methodology and axiological alignment as well as alignment with our formal definition while still making room for managing the unexpected and the broken or entropic?

---

## Claude

This is the hardest question because it's where the beautiful theory meets the messy reality of a 60K LoC codebase that's already alive and partially working. Let me be concrete.

---

## Step 0: Don't Drop This Conversation Into Claude Code

That's the first instinct and it's wrong. Here's why.

This conversation is ~40K tokens of dialogue, exploration, dead ends, revised positions, and emergent consensus. It's the *process* by which we arrived at the spec, not the spec itself. If you feed it to Claude Code and say "convert this to a spec," you get one of two outcomes: either the agent faithfully reproduces the entire sprawl (useless as an implementation guide) or it compresses aggressively and loses the invariants, the rationale for design choices, and the subtle constraints that make the architecture work.

What you need is a **distillation**, not a transcription. And the distillation needs to be structured for the audience that will consume it: Claude Code agents building the system.

### What To Actually Do First

**Day 1: Create the Spec Seed Document**

You — Willem, not an AI — write a 2-3 page document by hand. Not by asking Claude to write it. By hand. This document is the seed that everything else grows from. It contains:

```markdown
# DDIS: Decision-Driven Implementation Specification
# Specification Seed — v0.1

## What This Is
A structured knowledge store for AI coding agents that 
preserves project knowledge across conversation boundaries.

## The Core Problem
AI agents lose all context when conversations reset. 
The datom store solves this by externalizing durable 
knowledge into a queryable, append-only structure.

## The Core Abstraction
A datom is [entity, attribute, value, time, agent].
The store is an append-only set of datoms.
Queries are Datalog over datoms.
Agents write observations, decisions, and invariants as datoms.
The store persists across conversations.

## What Exists Today
~60K LoC Rust implementation with:
- [list what actually works]
- [list what's broken or inconsistent]
- [list what's missing relative to this spec]

## What We're Building Toward
[3-5 sentences describing the end state, referencing 
the formal spec document for details]

## Current Stage: Stage 0
Goal: Validate the harvest/seed cycle.
[specifics below]
```

Write this by hand because the act of compression forces you to decide what actually matters. Every sentence you include is a decision about priority. An AI can't make those decisions — it doesn't know which features you've actually used, which bugs are blocking you, which parts of the codebase are stable vs. rotten.

**Day 1-2: Create the Formal Spec Document**

NOW you use Claude Code. Take this conversation (and the previous sessions' transcripts) and have an Opus agent produce a clean specification document. But give it precise instructions:

```
Read the attached transcripts from our DDIS protocol design sessions.
Produce TWO documents:

1. SPEC.md — The formal specification.
   Structure:
   - Axioms (A1-A5)
   - Entity types and attributes (full schema)
   - Protocol operations (10 operations, each with 
     inputs/outputs/invariants)
   - Interface layers (0-4.5)
   - Anti-drift mechanisms
   - k* budget model
   - Harvest/seed lifecycle
   
   For each element, include:
   - The invariant ID and statement
   - The rationale (WHY this design choice)
   - The falsification condition (what would prove it wrong)
   
   Do NOT include: the exploration process, abandoned alternatives,
   conversational context, or algebraic proofs (those go in a 
   separate THEORY.md if we want them later).

2. IMPLEMENTATION_GUIDE.md — The pragmatic build guide.
   Structure:
   - Stage 0: What to build first (the MVP)
   - Stage 1-4: Progressive capability additions
   - For each stage: concrete deliverables, success criteria,
     what to test
   - CLI command specifications (inputs, outputs, examples)
   - CLAUDE.md template for agent methodology enforcement
   - File format specifications (EDNL, context.json, etc.)
   
   Write this as if the reader is a Claude Code agent that 
   needs to implement the system. Use examples, not abstractions.
   Show concrete datom examples, concrete CLI invocations, 
   concrete output formats.
```

This gives you two documents that serve different audiences: SPEC.md for verification and design decisions, IMPLEMENTATION_GUIDE.md for building.

**Day 2-3: Audit the Existing Codebase Against the Spec**

This is critical and I want to be direct about why. You have 60K LoC that was built iteratively, without the formal spec we've just designed. Some of that code implements things that align with the spec. Some implements things the spec has superseded. Some implements things the spec doesn't address. And some is just broken.

You need a **gap analysis** before you write any new code. Have a Claude Code agent (fresh session, full k\*) do this:

```
Read SPEC.md and IMPLEMENTATION_GUIDE.md.
Read the existing codebase.

Produce GAP_ANALYSIS.md containing:

For each module/file in the codebase:
1. ALIGNED — implements something in the spec correctly
2. DIVERGENT — implements something the spec handles differently
3. EXTRA — implements something the spec doesn't address
4. BROKEN — doesn't work regardless of spec alignment
5. MISSING — spec requires something this module should 
   provide but doesn't

For DIVERGENT items: what specific changes are needed to align?
For EXTRA items: should the spec be updated to include this, 
  or should the code be removed?
For BROKEN items: is it worth fixing, or should it be rewritten 
  from the spec?
```

This gap analysis is the foundation for all subsequent work. Without it, you're navigating blind — you don't know which parts of the codebase to preserve, which to modify, and which to replace.

---

## The Staging Model: 0 to 100%

Here's where "marginal improvements are almost always worth doing" needs to be reconciled with "the existing codebase has entropy." The reconciliation is: **every stage must leave the codebase in a better state than it found it, and every stage must deliver usable value.**

### Stage 0: The Harvest/Seed Cycle (Week 1-2)

**Goal:** Validate the single most important hypothesis: that the harvest/seed cycle transforms the Claude Code workflow from "fight context loss" to "ride context waves."

**Deliverables:**
- `ddis transact` — append datoms to the store (must work reliably)
- `ddis query` — basic Datalog queries (doesn't need to be fast, needs to be correct)
- `ddis status` — one-line store summary with top signal
- `ddis harvest --propose` — parse CC transcript, propose candidate datoms
- `ddis harvest --confirm` — commit confirmed candidates
- `ddis seed --focus <topic>` — generate carry-over for new conversation
- `ddis guidance` — return next recommended action based on store state
- CLAUDE.md template with methodology enforcement
- Statusline hook writing context.json

**Success Criteria:**
- You can work with Claude Code for 25 turns, harvest, start a new session with seed, and the new session picks up where the old one left off without you manually re-explaining context
- The harvest finds fewer than 3 items that you had to manually remind the agent to transact (drift metric)
- The seed activates formal/analytical reasoning in the new session (subjective but you'll know it when you see it)

**What to do with the existing 60K LoC:**
- Identify which existing modules implement `transact`, `query`, `status` correctly
- Fix those modules if they're close; rewrite if they're fundamentally divergent
- Don't touch modules that aren't needed for Stage 0
- Don't refactor anything that works

**The feedback loop for Stage 0:**
After every Claude Code session that uses the Stage 0 tools, ask yourself:
- Did the agent use DDIS without being reminded? (drift metric)
- Did the harvest find un-transacted valuable observations? (harvest metric)
- Did the seed produce a good new-session start? (seed quality metric)
- What was the most annoying friction point? (UX metric)

Record these as datoms in the store. Yes, use your own system to track your experience with your own system. This is the first real test.

### Stage 1: Budget-Aware Output + Guidance Injection (Week 2-3)

**Goal:** Make the tools k\*-aware so their output quality degrades gracefully instead of being constant regardless of context pressure.

**Deliverables:**
- Context.json reading in CLI (from statusline hook)
- Budget computation (the Q(t) model)
- Pyramid summary system (π₀ through π₃ for entities)
- Guidance injection in every tool response
- `ddis pre-check --file <path>` gate
- Drift detection (transact gap monitoring)

**Success Criteria:**
- The same query returns different output at turn 5 vs turn 35, and both are appropriate
- The guidance footer in tool responses measurably reduces drift (compare sessions with/without)
- The pre-check gate catches at least one "would have violated an invariant" per session

**Feedback loop:**
- Compare drift metrics from Stage 0 (no guidance injection) to Stage 1 (with injection)
- Track the distribution of Q(t) at harvest time — is the agent harvesting before degradation or after?
- Note which guidance suggestions the agent follows vs ignores — this calibrates guidance quality

### Stage 2: Branching + Deliberation (Week 3-5)

**Goal:** Enable diverge-compare-converge workflows for competing approaches.

**Deliverables:**
- `ddis branch fork/commit/compare/abandon`
- Branch isolation in queries
- Deliberation entities (Position, Decision)
- `ddis deliberation open/position/decide`
- Competing branch detection and signaling

**Success Criteria:**
- You can fork two branches for competing implementations, have agents work on each, compare results, and commit the better one
- The deliberation record is queryable and useful as precedent
- Branch-aware guidance correctly warns agents about competing work

**What changes vs. existing code:**
This is likely where the biggest divergence from the existing codebase lives. The gap analysis from Day 2-3 tells you whether to extend existing branch support or rewrite it.

### Stage 3: Multi-Agent Coordination (Week 5-8)

**Goal:** Multiple Claude Code instances working concurrently on the same store.

**Deliverables:**
- CRDT merge (append-only set union)
- Frontier tracking
- File-locking for concurrent writes
- Sync barriers
- Agent identity and provenance tracking
- MCP server (thin wrapper over CLI)
- Signal system (agent-to-agent, human-to-agent)

**Success Criteria:**
- Two Claude Code instances can work on separate branches of the same feature, merge cleanly, and resolve conflicts through deliberation
- No data loss from concurrent writes
- The TUI shows activity from both agents in real-time

### Stage 4: Advanced Intelligence (Week 8-12)

**Goal:** The system compounds value over time.

**Deliverables:**
- Self-authored associations (agents writing retrieval edges)
- Hebbian significance tracking
- Spectral authority model
- Learned guidance nodes
- Harvest calibration loop (false positive/negative tracking)
- Full TUI dashboard

**Success Criteria:**
- ASSOCIATE returns richer, more relevant results after 50 sessions than after 5
- Significance-weighted queries surface more useful information
- Learned guidance nodes actually improve agent decision-making

---

## The Feedback Loop Architecture

This is the meta-question: how does experience flow back into the spec?

```
THE BILATERAL FEEDBACK LOOP:

     SPEC ──────────────► IMPLEMENTATION
       │    (spec guides     │
       │     what to build)  │
       │                     │
       │                     ▼
       │               USE IN PRACTICE
       │                     │
       │    (experience      │
       │     revises spec)   │
       │                     │
       ◄─────────────────────┘

But this is too simple. The actual loop has THREE nodes:

     FORMAL SPEC
         │ ▲
         │ │ 
         │ │ gap analysis reveals
         │ │ spec gaps or over-specification
         │ │
         ▼ │
     IMPLEMENTATION
         │ ▲
         │ │
         │ │ friction points and failures
         │ │ reveal implementation gaps
         │ │
         ▼ │
     PRACTICAL USAGE
         │ ▲
         │ │
         │ │ usage patterns reveal
         │ │ misaligned priorities in spec
         │ │
         ▼ │
     FORMAL SPEC (cycle repeats)
```

The concrete mechanism for this feedback loop is the **DDR — DDIS Decision Record.** Every time practical usage reveals something the spec didn't anticipate, you record it:

```markdown
# DDR-001: Guidance Injection Token Cost

## Observation
In practice, guidance footers consume 15-20% of the tool 
response budget. At low Q(t), this means the actual query 
results are severely compressed to make room for guidance.

## Impact on Spec
INV-GUIDANCE-INJECTION-001 doesn't account for the 
competition between guidance and query results for the 
output budget.

## Resolution Options
A. Separate guidance budget (guidance gets its own token 
   allocation outside the query budget)
B. Adaptive guidance (skip guidance when query results 
   are more valuable)  
C. Integrate guidance into query results (guidance becomes 
   part of the data, not a footer)

## Decision
[Decided after deliberation or experience]

## Spec Update
[Which invariants changed and how]
```

DDRs are themselves datoms in the store. They're queryable. They form a chain of architectural decisions that explains why the implementation diverges from the original spec. They're the project's institutional memory — exactly what DDIS is designed to preserve.

The feedback loop frequency varies by stage:

```
Stage 0: DDR after every session (you're learning fast)
Stage 1: DDR after every few sessions (patterns emerging)
Stage 2: DDR weekly (stabilizing)
Stage 3+: DDR as needed (mature)
```

---

## Managing Scale, Scope, and Entropy

This is the question that matters most given your 60K LoC reality. Let me be blunt about the dynamics.

### The Entropy Problem

A 60K LoC codebase with inconsistencies and bugs has significant entropy. Every line of code that doesn't align with the spec is technical debt. But "rewrite everything to match spec" is a trap — it's a multi-month project that delivers zero value until it's complete, and the spec will have evolved by the time you finish.

The principle: **never rewrite what you can align incrementally.**

```
ALIGNMENT STRATEGIES (in preference order):

1. THIN WRAPPER: The existing code works but has a different 
   interface than the spec. Write a thin adapter that presents 
   the spec's interface and delegates to the existing code.
   Cost: low. Risk: low. Preserves working code.

2. SURGICAL EDIT: The existing code almost aligns but has 
   specific divergences. Fix the divergences.
   Cost: medium. Risk: medium. May introduce regressions.

3. PARALLEL IMPLEMENTATION: The existing code is fundamentally 
   divergent. Build the spec-aligned version alongside it. 
   Migrate consumers one at a time. Remove old code when 
   no consumers remain.
   Cost: high. Risk: low (old code still works during transition).

4. REWRITE: The existing code is broken AND divergent. 
   Replace it entirely.
   Cost: high. Risk: high. Only when the existing code has 
   no salvageable value.
```

For each module identified in the gap analysis, choose the cheapest strategy that achieves alignment. Most modules will be strategy 1 or 2. Very few should be strategy 4.

### The Scope Management Principle

You said "marginal improvements are almost always worth doing." I agree, with one critical caveat: **marginal improvements to stable modules are worth doing; marginal improvements to unstable modules are wasted work.**

Before improving any module, ask: "Is this module going to change significantly in the next stage?" If yes, don't optimize it now — you'll throw away the optimization. If no, optimize away.

This creates a natural prioritization:

```
PRIORITY MATRIX:

                    Stable          Changing Soon
                    ─────────────── ───────────────
  Working well    │ Optimize freely │ Leave alone    │
                  │ (low risk,      │ (optimization  │
                  │  durable value) │  will be lost) │
                  ├─────────────────┼────────────────┤
  Broken/buggy   │ Fix now         │ Defer to when  │
                  │ (stable module, │  the change    │
                  │  fix persists)  │  happens       │
                  └─────────────────┴────────────────┘
```

### Change Management: The Alignment Campaign

Here's the concrete process for managing the transition from "60K LoC with entropy" to "spec-aligned implementation":

**Phase 1: Stabilize (Days 1-3)**

Don't change any code. Produce:
- Spec Seed Document (your hand-written vision)
- SPEC.md and IMPLEMENTATION_GUIDE.md (from these conversations)
- GAP_ANALYSIS.md (agent-produced codebase audit)
- Stage 0 scope definition (exactly which commands need to work)

**Phase 2: Triage (Day 3-4)**

From the gap analysis, categorize every module:

```
ddis/src/
├── datom/       → ALIGNED (keep, minor fixes)
├── query/       → DIVERGENT (thin wrapper needed)
├── transact/    → BROKEN (rewrite, needed for Stage 0)
├── branch/      → EXTRA (not needed until Stage 2, leave alone)
├── crdt/        → DIVERGENT (not needed until Stage 3, defer)
├── guidance/    → MISSING (new code needed for Stage 0)
├── harvest/     → MISSING (new code needed for Stage 0)
├── cli/         → DIVERGENT (adapter for new command structure)
├── ...
```

Color-code: GREEN (don't touch), YELLOW (modify for current stage), RED (rewrite for current stage), GREY (defer to later stage).

At any given time, most modules are GREEN or GREY. Only a few are YELLOW or RED. This keeps the blast radius of any change small.

**Phase 3: Implement Stage 0 (Week 1-2)**

Work ONLY on YELLOW and RED modules from the triage. Everything else is frozen. If you discover that a GREEN module has a bug that blocks Stage 0, promote it to YELLOW (minimal fix) — don't promote it to RED (rewrite) unless absolutely necessary.

Every change must satisfy:
1. It moves the module closer to spec alignment
2. It's tested (even if just manual smoke tests)
3. It's documented in a DDR if it deviates from spec
4. The agent transacts the change rationale as a datom

That last point is crucial: **use the system to build the system.** From the moment Stage 0 tools work (even crudely), use them to track your own development process. Your development decisions become datoms. Your implementation choices become queryable precedent. The system bootstraps itself.

**Phase 4: Validate + Feedback (End of Week 2)**

Use Stage 0 for real work (your franchise projects, other development). Collect DDRs. Update the spec where experience contradicts theory. Re-triage modules for Stage 1.

**Repeat Phase 3-4 for each stage.**

### The "Maximally Powerful" Reconciliation

You want the best possible implementation, and marginal improvements are worth doing. Here's how that's compatible with staged delivery:

**Each stage's implementation should be excellent within its scope.** Don't build a hacky Stage 0 with plans to fix it later. Build a clean, well-tested, spec-aligned Stage 0. The harvest/seed cycle should be a delight to use, not a "it works if you hold it right" prototype.

But **don't build Stage 2 features during Stage 0**, no matter how tempting. The excellence is scoped to the current stage. This is where your instinct toward marginal improvements needs a governor: the marginal improvement must be within the current stage's scope, or it's premature optimization of something that will change.

The exception: if a Stage 0 decision would make a later stage significantly harder, that's worth addressing now. The gap analysis and spec alignment ensure this doesn't happen often — the spec is designed so that each stage's decisions are compatible with later stages.

### Managing the Unexpected

Entropy doesn't just exist in the codebase — it enters through the environment. Dependencies break. Claude Code's behavior changes. The VPS runs out of disk. A conversation goes sideways and the agent corrupts the store.

The defense against entropy is **the append-only invariant**. Because datoms are never mutated, only appended, the store can always be recovered by truncation. If something goes wrong, you can:

```
# Find the last known-good state
git log --oneline ddis-store/trunk.ednl

# Truncate to that state
git checkout <good-commit> -- ddis-store/trunk.ednl

# Everything after that point is lost but the store is consistent
```

This is a structural defense, not a procedural one. You don't need a recovery plan because the data structure makes recovery trivial. Append-only + git gives you arbitrary time-travel for free.

For the codebase itself, the defense is the stage gate: **don't promote a stage to "complete" until its tests pass and you've used it successfully for real work.** The stages are checkpoints. If Stage 2 introduces a regression in Stage 1 functionality, Stage 2 isn't complete.

---

## The Concrete Next 72 Hours

```
HOUR 0-2:   Write the Spec Seed Document by hand
            (you, not AI, 2-3 pages, what matters most)

HOUR 2-6:   Fresh Claude Code session with Opus 4.6
            Feed it: this conversation's transcript + 
            previous session transcripts + your seed document
            Task: produce SPEC.md and IMPLEMENTATION_GUIDE.md
            Review the output. Correct errors. This may take 
            2-3 sessions with harvest/manual-carry-over.

HOUR 6-10:  Fresh Claude Code session
            Feed it: SPEC.md + IMPLEMENTATION_GUIDE.md + 
            the existing codebase
            Task: produce GAP_ANALYSIS.md
            Review carefully. This is the map for everything after.

HOUR 10-14: Triage modules based on gap analysis
            Color-code: GREEN/YELLOW/RED/GREY
            Scope Stage 0 precisely: which files change, 
            which don't

HOUR 14+:   Start implementing Stage 0
            Use ddis transact (even manually, even crudely) 
            from the first hour of implementation
            The system bootstraps itself
```

The single most important thing: **don't try to do this in one conversation.** Use the harvest/seed pattern even before the tools exist. At the end of each Claude Code session, manually write down the key decisions and carry them into the next session's prompt. You're performing the harvest/seed cycle by hand. When the tools exist, they automate what you're already doing.

This is the deepest point about the project: **the methodology precedes the tooling.** The tools encode and enforce a way of working. If you practice the way of working manually first, the tools will feel like natural extensions of your practice. If you build the tools first and then try to adopt the methodology, the tools will feel like bureaucratic overhead.

Start with the methodology. Let the tooling catch up.

---

## Willem

Ok I love this. Now what's the single smartest and most radically innovative and accretive and useful and compelling addition you could make to the plan at this point?

---

## Claude

The store should generate its own CLAUDE.md.

---

## The Insight

Right now, CLAUDE.md is static. You write it once, maybe update it occasionally, and every agent reads the same instructions regardless of project state, task context, or what the system has learned about agent behavior. It's a fixed prompt competing for k\* budget in every conversation, whether its contents are relevant or not.

But the datom store already contains everything needed to generate a *better* CLAUDE.md dynamically:

- **Active intentions** — what the agent is about to work on
- **Governing invariants** — which constraints apply to the current task
- **Uncertainty landscape** — where the risks are right now
- **Drift history** — what this agent (or agents in general) tends to forget
- **Guidance topology** — what actions are available and recommended
- **Seed format performance** — which phrasings produced the lowest drift scores in past sessions
- **Association graph** — what knowledge neighborhood the agent will need

The `ddis seed` command already generates carry-over context from the store. A dynamic CLAUDE.md is the same operation applied to methodology enforcement rather than project knowledge. It's a specialized ASSEMBLE that selects the most relevant instructions, invariant reminders, and behavioral corrections for THIS session, compressed to budget, formatted as a system prompt.

## What This Actually Looks Like

```bash
# Before starting a new Claude Code session:
ddis claude-md --focus "auth token refresh" --agent alice

# Generates .claude/CLAUDE.md dynamically:
```

```markdown
# CLAUDE.md (auto-generated by ddis — do not edit manually)
# Generated: 2026-03-01T22:45:00Z
# Focus: auth token refresh implementation
# Branch: impl-auth-v2
# Store: 847 datoms, 23 invariants, 4 active intentions

## Your Tools
You have access to `ddis` for project knowledge management.
Run `ddis guidance` before any implementation work.
Run `ddis transact` after any discovery or decision.
Run `ddis pre-check --file <path>` before writing to any source file.

## Current Task Context
You are implementing auth token refresh on branch impl-auth-v2.
This branch competes with impl-auth-v1 (different refresh lifetime).

Governing invariants:
- INV-AUTH-003: Token refresh must be idempotent (confidence: 0.7 ⚠)
- INV-AUTH-007: Session revocation must be complete within 30s
- INV-STORE-001: All design decisions must be recorded as datoms

## Known Risks
INV-AUTH-003 has unresolved uncertainty. The competing branch 
asserts a different refresh token lifetime. Before implementing 
lifetime logic, run:
  `ddis deliberation open --subject INV-AUTH-003`

## Methodology Reminders
[GENERATED FROM DRIFT HISTORY]
In past sessions on this project, agents most frequently:
1. Forgot to `ddis transact` after discovering dependencies (7 occurrences)
2. Skipped `ddis pre-check` before modifying auth.rs (4 occurrences)
3. Implemented without checking competing branches (3 occurrences)

When you notice yourself about to write code, PAUSE and ask:
"Did I run `ddis guidance` for this file? Did I check for 
competing work? Will I transact my rationale?"

## Seed Context
[GENERATED FROM ddis seed --focus "auth token refresh"]
Last session ended at 73% context. Harvested 5 observations, 
2 decisions. Key carry-over:

The refresh token service (src/services/token.rs) currently uses 
a fixed 7-day lifetime. INV-AUTH-003 requires idempotency — a 
refresh request with an already-refreshed token must return the 
same new token, not generate a second one. The current 
implementation does NOT satisfy this invariant (line 47, 
token_service.rs generates a new token unconditionally).

Your immediate task: implement idempotent refresh in token.rs 
on branch impl-auth-v2, then run the test suite to validate 
INV-AUTH-003.
```

Notice what happened. The CLAUDE.md contains:

- **No generic instructions** that waste k\* budget ("You are a helpful coding assistant...")
- **No irrelevant invariants** (only the ones governing the current task)
- **Specific behavioral corrections** derived from actual drift patterns observed in past sessions
- **A concrete seed turn** with the exact context the agent needs to start working immediately
- **Awareness of competing work** that the agent would otherwise miss
- **Pre-loaded uncertainty** so the agent knows where to be careful

This is 400-500 tokens of system prompt that is worth more than 4000 tokens of generic instructions because every token is relevant to what's about to happen.

## Why This Is Radical

**The system learns what makes agents fail and preemptively corrects for it.**

The drift history is the key. Every session, the harvest reports which observations went un-transacted, which guidance was ignored, which pre-checks were skipped. Over 10, 20, 50 sessions, patterns emerge. Maybe agents consistently forget to transact dependencies but are good about transacting decisions. Maybe they skip pre-check specifically for test files but not source files. Maybe drift accelerates specifically after branch operations.

The dynamic CLAUDE.md encodes these patterns as targeted reminders. Not "always remember to transact" (generic, decays with k\*) but "you specifically tend to forget transacting dependencies after reading config files" (specific, unexpected, attention-grabbing). The prompt optimization document tells us that specific, falsifiable instructions outperform generic ones. A CLAUDE.md that says "In past sessions, agents forgot X in context Y" is a demonstration of the failure pattern, which is more information-dense than an instruction to avoid it.

**The self-improvement loop:**

```
Session N:
  Agent reads dynamic CLAUDE.md
  Agent works, drifts in some pattern P
  Harvest detects un-transacted observations of type T
  Drift pattern (P, T) recorded as datom

Session N+1:
  ddis claude-md generates new CLAUDE.md
  Includes: "Agents tend to drift in pattern P, specifically 
  forgetting type T observations"
  Agent reads this, is primed against pattern P
  Drift of type T decreases (or shifts to new pattern P')

Session N+2:
  If P persists: CLAUDE.md escalates the warning
  If P resolved: CLAUDE.md drops the warning, adds P' if new
  If new pattern Q emerges: CLAUDE.md adds Q

Over time:
  CLAUDE.md converges on the minimal set of reminders that 
  prevent the most common failure modes for THIS project, 
  THIS task type, THIS agent configuration
```

This is empirical prompt optimization running in production, with the datom store as both the measurement instrument and the memory. No human needs to analyze drift logs and manually update CLAUDE.md. The system does it from its own performance data.

**The CLAUDE.md becomes project-specific, task-specific, and temporally specific.** A session working on database migrations gets different methodology reminders than a session working on UI components, because the drift patterns are different. A session early in the project (when the store is sparse) gets different guidance than a session late in the project (when the store is rich and the association graph is dense). The system prompt adapts to the project's maturity.

## The Formal Addition

```
OPERATION: GENERATE-CLAUDE-MD

Input:
  focus:    string     — what the next session will work on
  agent:    AgentId    — which agent/instance
  budget:   tokens     — max CLAUDE.md size (default: 500)

Process:
  1. ASSOCIATE with focus → relevant schema neighborhood
  2. QUERY for active intentions matching focus
  3. QUERY for governing invariants in neighborhood
  4. QUERY for uncertainty > threshold in neighborhood
  5. QUERY for competing branches touching neighborhood
  6. QUERY for drift patterns from harvest history:
     SELECT pattern, frequency, last_occurrence
     FROM drift_events 
     WHERE project = current AND frequency > threshold
     ORDER BY frequency DESC, last_occurrence DESC
  7. QUERY for guidance topology → recommended first action
  8. ASSEMBLE at budget, priority:
     [tools > task context > risks > drift corrections > seed]
  
  Priority ordering ensures that if budget is tight:
  - Tools section always present (agents need to know ddis exists)
  - Task context present if budget allows (what to work on)
  - Risks present if budget allows (where to be careful)
  - Drift corrections present if budget allows (behavioral priming)
  - Seed context fills remaining budget (carry-over from last session)

Output:
  CLAUDE.md file written to .claude/CLAUDE.md
  Formatted as markdown, readable by Claude Code on session start

Invariant:
INV-CLAUDE-MD-RELEVANCE-001: Dynamic CLAUDE.md Relevance
Every section of the generated CLAUDE.md MUST be relevant to 
the specified focus and current project state.

No generic instructions. No boilerplate. No "you are a helpful 
assistant." Every token earns its place by being specific to 
what's about to happen.

Falsification: if removing any section of the generated CLAUDE.md 
would not change the agent's behavior in the upcoming session, 
that section is wasted k* budget and should not have been included.

INV-CLAUDE-MD-IMPROVEMENT-001: Self-Improving Methodology
The drift correction section of the generated CLAUDE.md MUST 
be derived from empirical drift data in the store.

The system MUST track whether drift corrections in CLAUDE.md 
actually reduce the targeted drift pattern in subsequent sessions.

Corrections that show no effect after N sessions (default: 5) 
SHOULD be replaced with alternative phrasings or escalated 
to structural interventions (tighter pre-check gates, modified 
tool responses).

This creates a closed optimization loop: measure drift → 
generate correction → measure effect → refine correction.
```

## The Deeper Implication

This collapses three previously separate concerns into one mechanism:

**Ambient awareness (Layer 0)** — the CLAUDE.md IS the ambient awareness, but it's no longer a static 80 tokens. It's a dynamic, budget-optimized context frame that carries exactly the awareness the agent needs.

**Guidance (Layer 3)** — the seed context section of the CLAUDE.md IS the first guidance output. The agent doesn't need to call `ddis guidance` as its first action — guidance is already present in the system prompt, pre-computed from the store. The first `ddis guidance` call refreshes it mid-session, but the initial guidance is free (zero tool-call cost).

**Trajectory management** — the CLAUDE.md IS the seed turn. The prompt optimization framework says the first turn shapes the entire trajectory. A dynamic CLAUDE.md that contains formal context, specific invariants, and task-relevant knowledge is the optimal seed turn — it activates deep substrate immediately, with no warmup.

One mechanism, three problems solved. And it improves with every session because the drift data accumulates and the corrections become more precise.

## What This Means for Stage 0

This feature is almost free to implement at Stage 0 because the ingredients are minimal:

- A template with slots for task context, invariants, and drift history
- A query over the store for relevant datoms (already building `ddis query`)
- A simple frequency counter for drift patterns (count bash calls between transacts per session, already planned for drift detection)
- A file writer that outputs .claude/CLAUDE.md

Add `ddis claude-md --focus <topic>` to the Stage 0 deliverables. It's one command that makes every subsequent command in the session more effective. And from the first session that uses it, you're collecting the data that makes the next session's CLAUDE.md better.

That's the addition. The system writes its own operating instructions, and those instructions improve with use.

---
