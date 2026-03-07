# Practical Implementation: Unified Store with Three LIVE Views in Claude Code

> **Date**: 2026-03-03
> **Scope**: Concrete implementation architecture for the unified datom store
> with three LIVE views, built on Claude Code's extension points (MCP servers,
> hooks, session transcripts, CLAUDE.md).
>
> **Traces to**: TRILATERAL_COHERENCE_MODEL.md §19, spec/01-store.md §1.3 (LIVE Index),
> ADR-STORE-007 (File-Backed Store with Git), spec/03-query.md (Datalog Query Engine),
> SEED.md §5 (Harvest/Seed Lifecycle), §8 (Interface Principles)
>
> **Reading prerequisite**: TRILATERAL_COHERENCE_MODEL.md (especially §19),
> spec/01-store.md (LIVE index specification, ADR-STORE-007 for storage format),
> spec/03-query.md (Datalog query semantics and six-stratum classification)
>
> **Assumption**: The user is using Claude Code as their primary development interface.

---

## Table of Contents

1. [The Practical Architecture](#1-architecture)
2. [Storage Format: Append-Only Files with Git, Not SQLite](#2-storage)
3. [Query Engine: Embedded Datalog, Not SQL](#3-query-engine)
4. [The Braid MCP Server — Store + Indexes + Datalog](#4-mcp-server)
5. [Hooks — The Automatic Sensors](#5-hooks)
6. [The Conversation Channel — How Intent Becomes Datoms](#6-conversation)
7. [The Code Channel — How Implementation Becomes Datoms](#7-code)
8. [The Spec Channel — How Specification Becomes Datoms](#8-spec)
9. [What the Agent Actually Experiences](#9-agent-experience)
10. [What You See as the Human](#10-human-experience)
11. [The Build Order](#11-build-order)

---

## 1. The Practical Architecture

<a name="1-architecture"></a>

Four layers do all the work:

1. **Append-only EDNL files** checked into git (the durable store)
2. **Derived in-memory indexes** rebuilt from the files (the fast query layer)
3. **An embedded Datalog engine** for ontology-on-read (the query semantics)
4. **An MCP server** wrapping all three + **Claude Code hooks** as sensors

Everything else falls out.

---

## 2. Storage Format: Append-Only Files with Git, Not SQLite

<a name="2-storage"></a>

**ADR-STORE-007** (spec/01-store.md:1175) already settled this question:

> SQLite is rejected as the store backend because its mutable semantics conflict
> with C1, though it could serve as an index cache.

The store is **append-only text files** checked into git:

```
.braid/
├── trunk.ednl            ← THE STORE (checked into git, append-only, text)
├── branches/             ← agent working sets (also checked in)
│   └── {agent}.ednl      ← per-agent uncommitted datoms
├── .cache/               ← DERIVED INDEXES (gitignored, rebuilt on startup)
│   ├── eavt.idx          ← sorted by (entity, attribute, value, tx)
│   ├── aevt.idx          ← sorted by (attribute, entity, value, tx)
│   ├── vaet.idx          ← sorted by (value, attribute, entity, tx)
│   ├── avet.idx          ← sorted by (attribute, value, entity, tx)
│   └── live.idx          ← LIVE materialized current-state view
└── .gitignore            ← ignores .cache/
```

**EDNL** = EDN (Extensible Data Notation) per line. One datom per line, human-readable,
grep-able:

```clojure
{:e #blake3 "a1b2c3..." :a :spec/type :v :invariant :tx #hlc "2026-03-03T14:22:01.000Z/agent-1/42" :op :assert}
{:e #blake3 "d4e5f6..." :a :spec/statement :v "All endpoints require valid JWT" :tx #hlc "2026-03-03T14:22:01.000Z/agent-1/42" :op :assert}
{:e #blake3 "a1b2c3..." :a :spec/traces-to :v #blake3 "x7y8z9..." :tx #hlc "2026-03-03T14:22:01.000Z/agent-1/42" :op :assert}
```

(JSONL is also viable and has better tooling in the Rust/JS ecosystem. The key property is
text-per-line, not the specific serialization format.)

### Why This Works with Git

The store is a **G-Set CvRDT** (grow-only set under set union). This means:

- **Git merge = set union = CRDT merge.** Two branches appending different datoms to the
  same file produce a trivial merge — keep both lines. In the worst case (textual conflict
  on adjacent lines), the resolution is always "keep both" because the store never deletes.

- **`git diff` shows exactly what changed.** Each line is a self-contained fact. A diff
  between two commits is a list of new facts asserted — directly meaningful, not a binary
  blob delta.

- **`git log` is the audit trail.** Every datom was added in a commit with a message,
  author, and timestamp. The store's own provenance (`:tx` fields) supplements this with
  causal ordering.

- **`git clone` gives you the full store.** Any collaborator gets the complete history.
  `braid index` (or automatic startup) rebuilds the derived indexes from the EDNL files.

- **Collaboration is just git.** Push, pull, merge, branch — all work because the data
  format is append-only text. No database replication protocol needed.

---

## 3. Query Engine: Embedded Datalog, Not SQL

<a name="3-query-engine"></a>

The query engine is a **Datomic-style Datalog dialect** with semi-naive bottom-up
evaluation, as specified in `spec/03-query.md`. This is not SQL. Datalog is the right
query language because:

1. **EAV triples are Datalog's native data model.** A datom `(e, a, v, tx, op)` maps
   directly to a Datalog fact. No ORM, no schema mapping, no impedance mismatch.

2. **Ontology on read.** The store is a flat bag of facts. The *meaning* — what constitutes
   an "invariant," what "implements" means, how "divergence" is computed — emerges from
   Datalog rules, not from table schemas. The same facts can answer different questions
   with different rule sets.

3. **Recursive queries are natural.** "What is the transitive closure of all dependencies?"
   is a two-line Datalog rule. In SQL it requires CTEs or procedural code.

4. **CALM compliance.** Monotonic Datalog programs have consistent coordination-free
   distributed implementations (the CALM theorem). This matters for multi-agent
   collaboration — monotonic queries give the same answer regardless of which agent's
   frontier you evaluate at.

### The Six Strata (from spec/03-query.md)

```
Stratum 0 — Primitive (monotonic):
  Current-value over LIVE index. Entity lookup.
  Example: "What is the current statement of INV-STORE-001?"

Stratum 1 — Graph Traversal (monotonic):
  Multi-hop joins, transitive closure.
  Example: "What spec elements trace to SEED.md §4?"

Stratum 2 — Uncertainty (mixed monotonicity):
  Aggregation (count-distinct), entropy computation (FFI to Rust).
  Example: "What is the epistemic uncertainty of this spec element?"

Stratum 3 — Authority (FFI):
  SVD of agent-entity interaction matrix (FFI to Rust linear algebra).
  Example: "Which agent has highest authority on store invariants?"

Stratum 4 — Conflict Detection (conservatively monotonic):
  Concurrent assertion detection on cardinality-one attributes.
  Example: "Are there conflicting values for :spec/statement on INV-STORE-001?"

Stratum 5 — Bilateral Loop (non-monotonic, barriered):
  Fitness computation, drift measurement, crystallization readiness.
  Example: "What is the current Φ across all three LIVE views?"
```

### Implementation: Datalog Engine in Rust

| Library | Tradeoff | Fit |
|---------|----------|-----|
| **datafrog** | Minimal, proven (Rust compiler's Polonius borrow checker). Low-level: you build relations manually. | Good for Strata 0–1 |
| **crepe** | Proc-macro Datalog — rules compiled at build time. Fast, ergonomic. But compile-time only, no runtime queries. | Bad — we need runtime query programs |
| **ascent** | Datalog DSL with semi-naive evaluation, supports stratified negation. | Best fit for Strata 0–4 |
| **Custom** | Spec defines semantics precisely. Full control over FFI strata. | Best fit for Strata 3–5 (FFI boundary) |

The likely architecture: **ascent for the core Datalog evaluation** (Strata 0–2, 4),
**custom FFI bridge for Strata 3 and 5** (linear algebra, fitness computation), with the
EDNL files as the fact source loaded into ascent's working memory at startup and updated
incrementally on each `transact`.

### How Queries Actually Work

The Datalog engine operates over **in-memory indexes** rebuilt from the EDNL files. The
flow is:

```
trunk.ednl (text, on disk, checked into git)
       │
       │  braid index (startup, or incremental on transact)
       ▼
In-memory indexes: EAVT, AEVT, VAET, AVET, LIVE
       │
       │  Datalog evaluation (semi-naive, bottom-up)
       ▼
Query results (e.g., LIVE_Φ = 15, unlinked datoms = [...])
```

The EDNL file is the source of truth. The indexes are caches. If you delete `.cache/`,
`braid index` rebuilds everything from the EDNL files in seconds.

### Example Queries as Datalog Programs

**"What is Φ (total divergence)?"**:

```datalog
% Unlinked intent: decisions with no traces-to link to a spec element
unlinked_intent(E) :-
  datom(E, :intent/decision, _, _, assert),
  \+ datom(_, :spec/traces-to-intent, E, _, assert).

% Unlinked spec: spec elements with no implements link from code
unlinked_spec(E) :-
  datom(E, :spec/type, _, _, assert),
  \+ datom(_, :impl/implements, E, _, assert).

% Unlinked impl: code functions with no implements link to spec
unlinked_impl(E) :-
  datom(E, :impl/function, _, _, assert),
  \+ datom(E, :impl/implements, _, _, assert).

% Φ = count of all unlinked datoms across all three views
phi(N) :- N = #count { E : unlinked_intent(E) }
         + #count { E : unlinked_spec(E) }
         + #count { E : unlinked_impl(E) }.
```

**"What context should I seed for topic X?"**:

```datalog
% Find all spec elements mentioning the topic
relevant_spec(E, Statement) :-
  datom(E, :spec/statement, Statement, _, assert),
  contains(Statement, ?topic).

% Find intent decisions linked to those spec elements
relevant_intent(I, Decision) :-
  relevant_spec(S, _),
  datom(S, :spec/traces-to-intent, I, _, assert),
  datom(I, :intent/decision, Decision, _, assert).

% Find code implementing those spec elements
relevant_impl(C, File, Line) :-
  relevant_spec(S, _),
  datom(C, :impl/implements, S, _, assert),
  datom(C, :impl/file, File, _, assert),
  datom(C, :impl/line, Line, _, assert).
```

This is the "ontology on read" — the same flat datoms answer fundamentally different
questions depending on which rules you apply. No schema changes, no migrations, no ALTER
TABLE. Just new rules.

---

## 4. The Braid MCP Server — Store + Indexes + Datalog

<a name="4-mcp-server"></a>

A long-running process (Rust) that holds the in-memory indexes, the embedded Datalog
engine, and exposes MCP tools. Claude Code connects to it via stdio.

```json
// .mcp.json (project-scoped, checked into git)
{
  "mcpServers": {
    "braid": {
      "type": "stdio",
      "command": "/path/to/braid-mcp",
      "args": ["--store", ".braid/trunk.ednl"]
    }
  }
}
```

On startup, the MCP server:
1. Reads `trunk.ednl` and any `branches/*.ednl` files
2. Builds the four indexes + LIVE view in memory
3. Loads the Datalog rule set (built-in rules for Φ, seed, coverage, etc.)
4. Begins serving MCP tool calls

On each `transact`:
1. Appends new datoms to `trunk.ednl` (or the agent's branch file)
2. Incrementally updates the in-memory indexes
3. Recomputes affected LIVE view entries
4. Returns the updated Φ

The MCP server exposes these tools to the agent:

| Tool | What it does | When it's used |
|------|-------------|----------------|
| `braid_note` | Transact an intent datom (decision, constraint, goal) | Agent calls it mid-conversation when a substantive decision is made |
| `braid_query` | Run a Datalog query over the store | Agent checks facts, traces links, asks arbitrary questions |
| `braid_phi` | Return current LIVE_Φ (divergence count) | Agent checks coherence state |
| `braid_seed` | Return relevant context from all three LIVE views for a given topic | Session start, or when switching tasks |
| `braid_unlinked` | Return unlinked datoms (intent without spec, spec without impl, etc.) | When Φ is high and agent needs to know what to link |
| `braid_link` | Assert a `traces-to` or `implements` relationship | Agent connects an intent to a spec element, or spec to code |

The agent calls these like any other MCP tool — they appear in its tool list alongside
`Read`, `Edit`, `Bash`, etc.

---

## 5. Hooks — The Automatic Sensors

<a name="5-hooks"></a>

This is where the "invisible" part happens. Hooks fire on normal workflow actions and
datomize the results without the agent doing anything explicit.

```json
// .claude/settings.json (project scope)
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [{
          "type": "command",
          "command": "braid ingest --store .braid/trunk.ednl"
        }]
      }
    ],
    "Stop": [
      {
        "hooks": [{
          "type": "command",
          "command": "braid harvest-session --store .braid/trunk.ednl --session $CLAUDE_SESSION_ID"
        }]
      }
    ],
    "SessionStart": [
      {
        "hooks": [{
          "type": "command",
          "command": "braid phi --store .braid/trunk.ednl --format context"
        }]
      }
    ]
  }
}
```

Here's what each hook does:

**`PostToolUse` on `Edit|Write`** — The big one. Every time the agent edits or creates a
file, the hook receives the JSON input on stdin including `tool_input.file_path`. The
`braid ingest` command:

```
If file is in spec/  → parse spec elements (INV, ADR, NEG) into spec datoms
If file is in src/   → parse module/function/type structure into impl datoms
If file is in exploration/ → parse decisions/claims into intent datoms
Otherwise           → index as impl datom with file path and content hash
```

This is the equivalent of `ddis parse` but running incrementally on every file save. The
agent edits `spec/01-store.md`, the hook fires, and 30 new spec datoms appear in the store
with their IDs, types, statements, and falsification conditions. The agent edits
`src/store.rs`, the hook fires, and function signatures, trait impls, and test assertions
become impl datoms. The LIVE views update instantly.

**`Stop` hook** — When the agent finishes responding, `braid harvest-session` reads the
JSONL transcript for the current session (at
`~/.claude/projects/-data-projects-ddis-ddis-braid/{session_id}.jsonl`), finds `user` and
`assistant` messages since the last harvest timestamp, and extracts:

- Decisions ("we decided X because Y") → intent datoms with `:intent/decision`
- Constraints ("X must always Y") → intent datoms with `:intent/constraint`
- Questions ("should we do X or Y?") → intent datoms with `:intent/open-question`
- Implementation notes ("I changed X to handle Y") → impl datoms

The extraction uses an LLM call (small model, fast) or pattern matching. Each extracted
datom gets `:intent/source` pointing to the specific message UUID in the transcript for
provenance.

**`SessionStart` hook** — Outputs the current Φ value and high-priority unlinked datoms to
stdout, which Claude Code injects as `additionalContext` at the start of the conversation.
The agent starts every session knowing "there are 7 intent decisions without spec backing,
3 spec elements without implementations, and Φ = 12."

---

## 6. The Conversation Channel — How Intent Becomes Datoms

<a name="6-conversation"></a>

This is the hardest channel because conversations are unstructured. Three mechanisms work
together:

**Automatic (Stop hook)**: The `harvest-session` command processes the transcript after
every agent turn. It's lossy but catches most substantive decisions. Runs in the
background, doesn't block the agent.

**Explicit (braid_note tool)**: For important decisions, the CLAUDE.md instructs the agent
to call `braid_note` explicitly:

```
When a significant design decision is made during conversation, use the
braid_note tool to record it. Example:

  braid_note({
    type: "decision",
    statement: "Authentication will use JWT, not sessions",
    rationale: "Stateless, scales horizontally",
    confidence: 0.9
  })
```

This is low-friction — one tool call. The agent already makes dozens of tool calls per
turn. Adding one more for a decision is negligible.

**Retroactive (braid_harvest command)**: A custom slash command `/harvest` that explicitly
processes the full session transcript and shows the agent what was extracted, letting it
correct or augment:

```
User: /harvest

Agent: I found 5 decisions in this session:
  1. "Use Blake3 for content hashing" (confidence: 0.95)
  2. "Schema attributes are cardinality-many by default" (confidence: 0.7)
  3. ...

  Should I transact these? Any corrections?
```

---

## 7. The Code Channel — How Implementation Becomes Datoms

<a name="7-code"></a>

The `PostToolUse` hook on `Edit|Write` handles this automatically. But what does the
`braid ingest` command actually extract from code?

For Rust files:

```
- Module declarations (mod X) → impl datom with :impl/module
- Function signatures (fn X(...) -> Y) → impl datom with :impl/function
- Struct/enum definitions → impl datom with :impl/type
- Trait implementations (impl X for Y) → impl datom with :impl/trait-impl
- Test functions (#[test] fn X) → impl datom with :impl/test
- Annotations (// ddis:maintains INV-X) → impl datom with :impl/maintains link
- Test assertions (assert_eq!, assert!) → impl datom with :impl/assertion
```

The tool for this is `tree-sitter` or `syn` (for Rust) — AST parsing, not regex. Fast
enough to run on every file save (~10ms per file).

The `implements` links are inferred from:

1. **Explicit annotations**: `// ddis:maintains INV-STORE-001` → direct link
2. **Name matching**: `fn test_append_only_immutability()` + `INV-STORE-001: Append-Only Immutability` → candidate link (confidence: 0.7)
3. **LLM inference**: "Does this function implement this invariant?" → candidate link (confidence varies)

Candidate links with confidence < 0.9 are surfaced as suggestions, not auto-asserted.

---

## 8. The Spec Channel — How Specification Becomes Datoms

<a name="8-spec"></a>

This is the easiest channel because specs are already structured. The `braid ingest`
command for spec files:

```
Parse markdown → extract elements by pattern:
  ### INV-{NS}-{NNN}: {Title}     → spec datom with :spec/type :invariant
  ### ADR-{NS}-{NNN}: {Title}     → spec datom with :spec/type :adr
  ### NEG-{NS}-{NNN}: {Title}     → spec datom with :spec/type :negative-case
  **Traces to**: {ref}            → spec datom with :spec/traces-to
  **Falsification**: {text}       → spec datom with :spec/falsification
  **Verification**: {tags}        → spec datom with :spec/verification
```

The existing `ddis parse` command does almost exactly this — it extracts structured
elements from spec markdown. The difference: instead of a separate SQLite index, the
results go into the datom store as datoms in `trunk.ednl`, alongside intent and
implementation datoms. Queryable via Datalog, not SQL.

---

## 9. What the Agent Actually Experiences

<a name="9-agent-experience"></a>

Here's a concrete session flow:

```
SESSION START
  ← SessionStart hook fires
  ← Agent sees: "Φ = 12. 4 intent decisions unlinked, 3 spec elements
     unimplemented, 5 code functions without spec backing."

AGENT READS TASK
  Agent: "I need to implement the transaction validation logic."

  → Calls braid_seed({ topic: "transaction validation" })
  ← Gets back: relevant intent decisions, spec invariants (INV-STORE-005,
     INV-STORE-006), existing code in src/store.rs, and the link gaps.

AGENT WRITES SPEC
  Agent: Edits spec/01-store.md, adds INV-STORE-015
  ← PostToolUse hook fires → braid ingest parses the new invariant
  ← Store now has the spec datom. Φ goes UP by 1 (new unimplemented spec element)

AGENT MAKES A DECISION
  User: "Should we validate schemas at transact time or query time?"
  Agent: "Transact time — fail fast, prevent bad data from entering the store."
  → Calls braid_note({ type: "decision", statement: "Schema validation
     at transact time, not query time", rationale: "Fail fast" })
  ← Intent datom transacted. Φ goes UP by 1 (new unlinked decision)

AGENT WRITES CODE
  Agent: Edits src/store.rs, adds fn validate_schema()
  ← PostToolUse hook fires → braid ingest parses the function
  ← Agent added `// ddis:maintains INV-STORE-015`
  ← Store creates impl datom AND implements link. Φ goes DOWN by 1.

  Agent: Edits src/store.rs, adds #[test] fn test_schema_validation()
  ← PostToolUse hook fires → test datom created with link to validate_schema

AGENT CHECKS COHERENCE
  → Calls braid_phi()
  ← "Φ = 12. Was 12 at session start. Net: 0. (+2 new elements, -2 linked)"

SESSION END
  ← Stop hook fires
  ← harvest-session extracts 3 decisions from conversation, transacts them
  ← Φ updates to 15 (3 new unlinked intent datoms)
```

The agent never "does process." It writes specs, writes code, and occasionally notes a
decision. The hooks and MCP server handle everything else.

---

## 10. What You See as the Human

<a name="10-human-experience"></a>

```
$ braid status
┌─────────────────────────────────────────┐
│  Φ = 15  (↓3 from yesterday)           │
│                                         │
│  Intent:  23 decisions, 19 linked (83%) │
│  Spec:    47 elements, 41 impl'd (87%) │
│  Impl:    89 functions, 72 spec'd (81%)│
│                                         │
│  Top unlinked:                          │
│   • "Use Blake3 for hashing" (intent)   │
│   • INV-MERGE-003 (no impl)            │
│   • src/query.rs:eval_rule (no spec)   │
└─────────────────────────────────────────┘

$ braid trend
Φ: 28 → 22 → 18 → 15  (last 4 sessions, converging)

$ braid view intent --unlinked
4 decisions without spec backing:
  1. "Use Blake3 for hashing" [2026-03-03, session 011]
  2. "Schema validation at transact time" [2026-03-03, session 012]
  3. ...
```

---

## 11. The Build Order

<a name="11-build-order"></a>

What would you actually implement first?

**Week 1**: The datom store (append-only EDNL file, content-addressed entity IDs) +
`braid ingest` for spec files only. Just `braid ingest spec/01-store.md` → datoms
appended to `trunk.ednl`. Verify round-trip: ingest → build indexes → Datalog query →
same data. This validates the datom model and the EDNL format.

**Week 2**: `braid ingest` for code files (tree-sitter parsing) + the `implements` link
detection from `// ddis:maintains` annotations. Now you have S↔P in one store. `braid phi`
shows unimplemented spec elements and unannotated code. All of this is git-diffable and
collaboratable.

**Week 3**: The MCP server wrapping the store (EDNL files + in-memory indexes + Datalog
engine) + `PostToolUse` hooks for Edit/Write. Now the agent's normal editing workflow
automatically feeds the store. No behavior change required from the agent.

**Week 4**: `braid_note` tool + `Stop` hook for conversation harvesting + `SessionStart`
hook for context injection. Now all three channels feed the store, and Φ is live.

**Week 5+**: Refinement — better extraction, LLM-assisted link inference, the
`braid status` dashboard, trend tracking, multi-agent collaboration via git push/pull.

The critical insight: **weeks 1-3 deliver value without touching the conversation channel
at all.** The S↔P boundary alone — knowing which spec elements are implemented and which
code has no spec backing — is immediately useful. The conversation channel (I) is the
hardest and least critical to get right first.

---

*The unified store is an append-only EDNL file checked into git. The indexes are derived
caches rebuilt on startup. The query engine is embedded Datalog with semi-naive evaluation.
The "sensors" are Claude Code hooks on `PostToolUse` and `Stop`. The "interface" is an MCP
server. Collaboration is git push/pull — merge is set union. The formalism from
TRILATERAL_COHERENCE_MODEL.md §19 is the hidden engine; what you experience is a tool
that keeps score while you work.*
