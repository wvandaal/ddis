# Braid LLM-Native Interface Design

> **Principle**: Braid's primary users are AI coding agents. Every interface surface is
> optimized for LLM cognition first, human readability second.

---

## 1. The Cognitive Model

LLMs are **gestalt processors that activate latent knowledge through prompt structure.**
(Ref: `prompt-optimization` skill, §Core Model). Interface design implications:

### 1.1 Token Budget Is Cognitive Budget

Every token of Braid output consumed by the agent is a token NOT available for reasoning.
Verbose output doesn't just look bad — it literally degrades the agent's analytical depth.

**Design rule**: Default output ≤ 20 lines. `--verbose` for full. `--budget N` for exact.
The default is the **terse, structured** variant, not the full dashboard.

### 1.2 Examples > Documentation

LLMs learn tool usage from worked examples, not from prose descriptions. A single
copy-pasteable example teaches syntax, semantics, output format, and domain conventions
in one shot. (Ref: `prompt-optimization` §Rules, "constraints restrict; demonstrations
navigate.")

**Design rule**: Every `--help` includes 2-3 realistic examples with expected output.

### 1.3 Structure > Prose

LLMs parse `key: value` pairs more reliably than paragraphs. JSON is better than prose
for programmatic chaining. Tables are better than lists for comparative data.

**Design rule**: Default output is `key: value` pairs. `--json` for programmatic use.
Never output unstructured paragraphs as primary data.

### 1.4 Errors Are Instructions

A human reads an error, thinks, and adjusts. An LLM needs the error to TELL it
what to do next. Dead-end errors force the agent into exploration mode, consuming
tokens on recovery instead of progress.

**Design rule**: `Error: <what>. Fix: <command>.` Always.

### 1.5 Composability Over Completeness

The output of one Braid command becomes input to the agent's reasoning about the
next command. Entity IDs in output must be directly usable as input. Operations
should chain: `observe → query → seed → analyze`.

**Design rule**: Entity IDs in output are copy-pasteable into other commands.

### 1.6 Ambient Awareness vs Active Context

Braid's guidance footer is **ambient context** — low-cost, always-present awareness
that shapes behavior without consuming significant k*. Full dashboards and analysis
reports are **active context** — expensive, phase-specific. Keep ambient cheap (≤ 2 lines).
Make active budget-aware.

---

## 2. Command Surface Design

### 2.1 Naming Convention

**Verb-first, single-word, predictable.**

| Command | Verb | Purpose | Semantic Hint |
|---------|------|---------|---------------|
| `observe` | capture | Record a single fact | "I noticed..." |
| `query` | retrieve | Ask structured questions | "Show me..." |
| `harvest` | extract | Batch end-of-session | "Summarize what I learned" |
| `seed` | orient | Start-of-session context | "What should I know?" |
| `analyze` | diagnose | Store structural health | "Is everything coherent?" |
| `guidance` | steer | Methodology recommendations | "What should I do?" |
| `status` | summarize | Quick state overview | "Where am I?" |
| `show` | inspect | Single entity details | "Tell me about X" |

**Why these verbs**: They map to the natural workflow cycle —
observe → query → analyze → guidance → (do work) → harvest → seed.

### 2.2 Flag Conventions

| Flag | Meaning | Universal? |
|------|---------|------------|
| `--json` | Machine-parseable output | Yes (every command) |
| `--budget N` | Token limit for output | Yes (every command) |
| `--verbose` | Full output (no truncation) | Yes |
| `--for-human` | Natural language output | Where applicable |
| `--format <fmt>` | Output format (table, json, edn, plain) | Where applicable |
| `--confidence N` | Confidence level 0.0-1.0 | observe, harvest |
| `--tag <tag>` | Categorization | observe, harvest |
| `--relates-to <id>` | Cross-reference | observe, harvest |

### 2.3 Output Format Tiers

Every command supports 4 tiers:

**π₀ (one-liner)**: `--budget 50` or when embedded in guidance footer
```
store: 2679 datoms | Φ=210.6 | M(t)=0.00 | 4 actions
```

**π₁ (summary)**: `--budget 200` or default for most commands
```
store: 2679 datoms, 667 entities, 7 txns
phi: 210.6 (GapsAndCycles)
methodology: 0.00 (tx: ✗ | spec: ✗ | query: ✗ | harvest: ✗)
topology: 221 components, 83 cycles
action: fix Datalog joins (blocks all downstream)
```

**π₂ (detailed)**: `--verbose` or `--budget 1000`
```
[Full current output with sections but truncated lists]
```

**π₃ (exhaustive)**: `--verbose --budget 0` (unlimited)
```
[Everything, including entity lists and raw metrics]
```

---

## 3. Help Text Template

Every command's `--help` follows this structure:

```
<command> — <one-line purpose>

USAGE:
  braid <command> [flags]

EXAMPLES:
  braid <command> <example1>
  → <expected output line 1>

  braid <command> <example2>
  → <expected output line 1>

FLAGS:
  <flag>  <description>

SEE ALSO:
  <related command> — <when to use instead>
```

Target: ≤ 30 lines total. The EXAMPLES section is the most important.

---

## 4. Error Message Template

```
error: <concise description>
  fix: <exact command to run>
  see: braid <related-command> --help
```

Example:
```
error: no store found at .braid/
  fix: braid init
  see: braid init --help
```

Example:
```
error: unknown attribute ':spec/typo'
  fix: braid query -a ':db/ident' | grep spec  (list valid attributes)
  see: braid query --help
```

---

## 5. MCP Tool Description Template

```json
{
  "name": "braid_<verb>",
  "description": "<purpose>. Use when <trigger>. For <alternative>, use braid_<other> instead.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "<param>": {
        "type": "<type>",
        "description": "<what it is> (e.g., '<realistic example>')"
      }
    }
  }
}
```

Key principle: MCP descriptions include **when to use** and **when NOT to use** (prefer
another tool). This prevents LLMs from calling the wrong tool.

---

## 6. Guidance Footer Design

The guidance footer is **ambient context** — appended to every command output.
Budget: ≤ 2 lines, ≤ 80 tokens.

```
↳ Φ=210.6 | M(t)=0.00 | 4 actions | harvest due (15/20 turns)
↳ next: fix Datalog joins → braid query --datalog '[:find ?e :where [?e :db/doc _]]'
```

Line 1: metrics. Line 2: top action with executable command.

When M(t) > 0.7 and Φ < 50 (healthy): compress to 1 line:
```
↳ Φ=42 | M(t)=0.85 | coherent | 0 actions
```

---

## 7. Composability Patterns

### Chain: observe → query → seed

```bash
braid observe "merge is bottleneck"    # → :observation/a3f8...
braid query -e ':observation/a3f8'     # → full entity with all attributes
braid seed --for-human                 # → briefing includes recent observations
```

### Chain: analyze → guidance → harvest

```bash
braid analyze --budget 200             # → key metrics + actions
braid guidance                         # → prioritized action list
braid harvest --task "fixing X"        # → candidates from session work
```

### Chain: promote → query → bilateral

```bash
braid promote spec/01-store.md         # → spec elements as datoms
braid query -a ':spec/namespace'       # → all namespaces populated
braid bilateral                        # → F(S) coherence statement
```

---

*This design document is itself structured for LLM consumption: short sections,
key:value conventions, copy-pasteable examples, and a clear hierarchy. It practices
what it preaches.*
