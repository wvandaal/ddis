# CLI Coherence Specification

> **Formal specification for the braid CLI as a learnable language.**
> Derived from the Steering Manifold theory (docs/design/STEERING_MANIFOLD.md).
> Every design decision traces to a steering principle and an invariant.
>
> This is the cleanroom spec: define before building, derive tests from spec,
> verify implementation against spec.

---

## 1. DESIGN DECISION

### ADR-CLI-001: Noun-Verb Grammar with Shortcut Layer

**Context**: The braid CLI is a Markov blanket between the datom store and LLM agents.
Every token crossing this boundary steers the LLM through its knowledge manifold.
The grammar must be learnable from a single example (STEERING_MANIFOLD.md §14).

**Decision**: All commands follow `braid <resource> <action> [args] [flags]`.
A shortcut layer maps ergonomic aliases to canonical forms.
Help text, error messages, and documentation always show the canonical form.

**Rationale**: One example (`braid task list`) teaches the entire grammar.
The LLM can predict `braid observation list`, `braid session list`, `braid spec list`
from that single interaction. |GrammarSpace| = |ValidCommands|. Zero excess cardinality.

**Alternatives rejected**:
- Verb-flag (`braid observe --list`): unpredictable, N entity types = N learning events
- Mixed noun/verb: current state, fragments the LLM's in-context model
- Verb-with-subcommands (`braid observe list`): preserves existing names but mixed metaphor

**Traces to**: STEERING_MANIFOLD.md §14 (API as Formal Language), ADR-FOUNDATION-014

---

## 2. FORMAL GRAMMAR (BNF)

```bnf
<command>     ::= "braid" <top-level>
                | "braid" <resource> <action> <args> <flags>
                | "braid" <shortcut> <args> <flags>

<top-level>   ::= "init" <flags>
                | "status" <flags>
                | "shell"
                | "mcp"
                | "--help" | "--version"

<resource>    ::= "observation" | "task" | "spec" | "session"
                | "witness" | "boundary" | "trace" | "challenge"
                | "schema" | "config" | "topology" | "store"
                | "daemon" | "model"

<action>      ::= <universal-action> | <resource-specific-action>

<universal-action> ::= "list" | "show" <id> | "search" <query>

<resource-specific-action> ::=
  (* observation *) "create" <text> | "link" <from> <to> | "recent" <n>
  (* task *)        "create" <title> | "close" <id> | "claim" <id>
                  | "dep" "add" <from> <to> | "ready" | "next" | "update" <id>
  (* spec *)        "create" <id> | "review" <id> | "accept" <id> | "reject" <id>
  (* session *)     "start" | "end" | "harvest" | "seed"
  (* witness *)     "status" | "check" | "completeness"
  (* boundary *)    "status" | "evaluate"
  (* trace *)       "scan" | "verify"
  (* challenge *)   "create" <entity>
  (* schema *)      (* list and show only *)
  (* config *)      "get" <key> | "set" <key> <value>
  (* topology *)    "plan"
  (* store *)       "query" <expr> | "merge" <source> | "log" | "write" <action>
  (* daemon *)      "start" | "stop"
  (* model *)       "download"

<shortcut>    ::= "observe" <text>        (* = observation create *)
                | "go" <id>               (* = task claim *)
                | "done" <ids>            (* = task close *)
                | "next"                  (* = task next *)
                | "note" <text>           (* = observation create --confidence 0.5 *)
                | "harvest" <flags>       (* = session harvest *)
                | "seed" <flags>          (* = session seed *)
                | "query" <expr>          (* = store query *)
                | "merge" <source>        (* = store merge *)
                | "log" <flags>           (* = store log *)
                | "transact" <flags>      (* = store write assert *)

<flags>       ::= (* per-command, see §4 *)
<id>          ::= "t-" <hex8> | "o-" <hex8> | ":"<namespace>"/"<ident>
<text>        ::= <quoted-string>
<query>       ::= <quoted-string>
```

### Grammar Invariant

**INV-CLI-001 (GRAMMAR)**: For every `<resource>` R in the grammar, the commands
`braid R list`, `braid R show <id>`, and `braid R search <query>` are valid.
**Falsification**: any resource that lacks list, show, or search.
**Test**: iterate all resources in clap command tree, assert all three exist.

---

## 3. RESOURCE ONTOLOGY

### 3.1 Resources and their actions

| Resource | Universal | Specific | Current command | Migration |
|---|---|---|---|---|
| `observation` | list, show, search | create, link, recent | `observe` (has subcommands) | rename observe→observation, add `observe` shortcut |
| `task` | list, show, search | create, close, claim, dep, ready, next, update | `task` (has subcommands) | rename `dep` → `dep add` (already done), rename nothing else |
| `spec` | list, show, search | create, review, accept, reject | `spec` (has subcommands) | add `search` if missing |
| `session` | list, show, search | start, end, harvest, seed | `session` (partial), `harvest`, `seed` | absorb harvest+seed into session subcommands |
| `witness` | list, show, search | status, check, completeness | `witness` (has subcommands) | add list/show/search |
| `boundary` | list, show, search | status, evaluate | `bilateral` | rename bilateral→boundary, add list/show/search |
| `trace` | list, show, search | scan, verify | `trace` (standalone) | restructure as resource with subcommands |
| `challenge` | list, show, search | create | `challenge` (standalone) | restructure as resource with subcommands |
| `schema` | list, show, search | — | `schema` (standalone) | restructure as resource with subcommands |
| `config` | list, show, search | get, set | `config` (has subcommands) | add show/search if missing |
| `topology` | list, show, search | plan | `topology` (has subcommands) | add list/show/search |
| `store` | — | status, query, merge, log, write | various top-level | group under store, keep top-level shortcuts |
| `daemon` | — | start, stop, status | `daemon` (has subcommands) | no change needed |
| `model` | — | download, status | `model` (has subcommands) | no change needed |

### 3.2 Top-level commands (not resources)

| Command | Reason for top-level |
|---|---|
| `init` | One-time bootstrap, not a resource |
| `status` | Canonical first interaction, session-start basin-setter (shortcut for `store status`) |
| `shell` | Interactive mode, special lifecycle |
| `mcp` | Server mode, special lifecycle |

### 3.3 Shortcuts (ergonomic aliases)

| Shortcut | Canonical form | Justification |
|---|---|---|
| `observe <text>` | `observation create <text>` | Most frequent capture operation |
| `go <id>` | `task claim <id>` | Session workflow speed |
| `done <ids>` | `task close <ids>` | Session workflow speed |
| `next` | `task next` | Session workflow speed |
| `note <text>` | `observation create <text> -c 0.5` | Quick capture with lower confidence |
| `harvest [flags]` | `session harvest [flags]` | Session lifecycle speed |
| `seed [flags]` | `session seed [flags]` | Session lifecycle speed |
| `query <expr>` | `store query <expr>` | Frequent query operation |
| `merge <source>` | `store merge <source>` | Preserve existing usage |
| `log [flags]` | `store log [flags]` | Preserve existing usage |
| `transact [flags]` | `store write assert [flags]` | Preserve existing usage |

---

## 4. OUTPUT SHAPE SPECIFICATION

### 4.1 Templates

Every command output maps to exactly one template:

**ListOutput** (for all `<resource> list` commands):
```
<count> <resource_plural> [in <groups> groups]

[<table or grouped list>]

<steering_question>
```

**DetailOutput** (for all `<resource> show` commands):
```
<resource_type> <id> "<title_or_summary>"
status: <status> | <priority> <type> | deps: <n> | traces: <n>
[<body_if_present>]
[<relations>]

<steering_question>
```

**SearchOutput** (for all `<resource> search` commands):
```
search "<query>": <count> results

[<ranked_list_with_scores>]

<steering_question>
```

**CreateOutput** (for all `<resource> create` commands):
```
created: <id> "<title_or_summary>"

<type_info> | <datom_count> datoms | entity: <entity_ident>

<concept_membership>
<structural_gap>
<steering_question>
```

**ErrorOutput** (for all errors):
```
error: <what_went_wrong>

fix: <what_right_looks_like> | ref: <spec_element>
```

**SteeringVector** (appended to most responses):
```
<concept_membership_line>
<structural_gap_line>
<steering_question>
```

### 4.2 Invariants

**INV-CLI-003 (OUTPUT-SHAPE)**: All `list` commands produce ListOutput. All `show`
commands produce DetailOutput. All `search` commands produce SearchOutput.
**Falsification**: any list/show/search command that doesn't match its template.
**Test**: run every list/show/search command, parse output, verify template match.

**INV-CLI-005 (RESPONSE-STEERING)**: Every create/close/claim response includes
at minimum: (1) what happened, (2) what changed in the store, (3) what to do next.
**Falsification**: any mutation response that omits any of the three components.
**Test**: for every create/close/claim command, parse output, verify 3 components.

**INV-CLI-006 (PROGRESSIVE)**: The default output of every command fits in 10 lines.
`--verbose` expands to full detail. `--json` provides machine-parseable output.
**Falsification**: any default output exceeding 10 lines (excluding steering vector).
**Test**: run every command with defaults, count lines, assert <= 10.

---

## 5. ERROR SPECIFICATION

### 5.1 Error categories and steering templates

| Category | Template | Example |
|---|---|---|
| Unknown resource | `error: unknown resource '<input>'\nfix: valid resources: observation, task, spec, session, ...` | `braid foo list` |
| Unknown action | `error: unknown action '<action>' for <resource>\nfix: valid actions: list, show, search, create, ... \| ref: braid <resource> --help` | `braid task foo` |
| Shortcut misuse | `hint: '<shortcut>' is a shortcut for '<resource> <action>'.\n      To <intended_action>: braid <resource> <correct_action>` | `braid observe list` when observe is alias for observation create |
| Missing argument | `error: <resource> <action> requires <arg>\nfix: braid <resource> <action> <example> \| ref: braid <resource> <action> --help` | `braid task show` (no id) |
| Entity not found | `error: <type> '<id>' not found\nfix: braid <resource> search "<partial>" \| list: braid <resource> list` | `braid task show t-nonexistent` |
| Validation | `error: <field> invalid: <reason>\nfix: <valid_format> \| ref: <spec_element>` | `--confidence 2.0` |

### 5.2 Invariant

**INV-CLI-004 (ERROR-STEERING)**: Every error message contains: (a) what went wrong,
(b) how to do it correctly, (c) a runnable command or ref.
**Falsification**: any error message lacking any of the three components.
**Test**: trigger every error path, parse output, verify 3 components.

---

## 6. VOCABULARY LEXICON

### 6.1 Canonical terms

| Concept | Canonical term | NEVER use |
|---|---|---|
| Knowledge capture | observation | note, finding, insight, entry |
| Work item | task | issue, ticket, item, todo |
| Spec element | spec | invariant, decision, constraint (these are types, not the resource) |
| Agent session | session | context, conversation, turn |
| Coherence check | boundary | bilateral, alignment, coherence-check |
| Code-spec link | trace | link, reference, mapping |
| Epistemic test | challenge | test, verify, check (these are actions) |
| Knowledge extraction | harvest | commit, save, persist, flush |
| Context assembly | seed | context, brief, summary |
| Store merge | merge | sync, combine, union |

### 6.2 Invariant

**INV-CLI-002 (VOCABULARY)**: Each concept in the lexicon uses exactly one term across
all surfaces: command names, flag names, help text, error messages, output text, schema
attribute namespaces, and documentation.
**Falsification**: any surface using a non-canonical term for a lexicon concept.
**Test**: extract all user-visible strings, check against lexicon, flag violations.

---

## 7. MIGRATION PLAN

### 7.1 Phase ordering

```
Phase 0: SPEC (this document) ✓
Phase 1: SCAFFOLD — OutputFormatter types, Resource trait, shortcut dispatch
Phase 2: MIGRATE — one resource at a time, largest first
Phase 3: ERRORS — rewrite all error messages
Phase 4: VERIFY — invariant tests, E2E, AGCR, DOGFOOD
```

### 7.2 Migration strategy per resource

Each resource migration follows the same protocol:
1. Add new clap subcommand group under the noun name
2. Implement universal actions (list, show, search) using OutputFormatter
3. Move existing logic from old command into resource-specific actions
4. Add shortcut alias pointing old command name to `<resource> create`
5. Update all help text to show canonical form
6. Write unit tests for the new subcommand group
7. Verify old shortcut still works with steering error for misuse

### 7.3 Backwards compatibility via shortcuts

Old commands continue to work as shortcuts. The ONLY user-visible change is:
- `braid observe list` → works (already did)
- `braid observation list` → works (NEW canonical form)
- `braid observe "text"` → works (shortcut, help steers to canonical)
- `braid observation create "text"` → works (NEW canonical form)
- `braid harvest --commit` → works (shortcut for `session harvest --commit`)
- `braid session harvest --commit` → works (NEW canonical form)

No command stops working. New canonical forms are added alongside.

---

## 8. TEST PLAN

### 8.1 Unit tests (per invariant)

| Test | Invariant | Method |
|---|---|---|
| `test_grammar_completeness` | INV-CLI-001 | Parse clap tree, for every resource verify list+show+search exist |
| `test_vocabulary_consistency` | INV-CLI-002 | Extract all visible strings, check against lexicon |
| `test_output_list_shape` | INV-CLI-003 | Run all list commands, parse output, verify ListOutput template |
| `test_output_show_shape` | INV-CLI-003 | Run all show commands, parse output, verify DetailOutput template |
| `test_output_search_shape` | INV-CLI-003 | Run all search commands, parse output, verify SearchOutput template |
| `test_error_steering` | INV-CLI-004 | Trigger 10+ error paths, verify 3-component structure |
| `test_response_steering` | INV-CLI-005 | Run all create/close commands, verify 3-component response |
| `test_progressive_disclosure` | INV-CLI-006 | Run all commands with defaults, assert output <= 10 lines |
| `test_shortcut_dispatch` | ADR-CLI-001 | For each shortcut, verify it dispatches to canonical form |
| `test_shortcut_error_steering` | ADR-CLI-001 | Misuse each shortcut, verify error steers to canonical |

### 8.2 Property tests (proptest)

| Test | Property |
|---|---|
| `prop_resource_symmetry` | For any resource R, `braid R list` exits 0 |
| `prop_show_requires_id` | For any resource R, `braid R show` without id exits nonzero with helpful error |
| `prop_search_accepts_any_string` | For any resource R and string S, `braid R search S` exits 0 |
| `prop_shortcut_equivalence` | For any shortcut S with canonical form C, output of S == output of C |

### 8.3 Integration tests

| Test | Scope |
|---|---|
| `test_observation_lifecycle` | create → list (appears) → show (detail) → search (findable) |
| `test_task_lifecycle` | create → claim → list (in-progress) → close → list (closed) |
| `test_session_lifecycle` | start → observation create → session harvest → session seed |
| `test_cross_resource_steering` | observation create returns steering question referencing tasks |

### 8.4 E2E test script

```bash
#!/bin/bash
# E2E: Simulate a 15-command LLM session, measure grammar convergence
# Target: Agent Grammar Convergence Rate (AGCR) > 0.9 by turn 5

STORE=$(mktemp -d)/.braid
ERRORS=0
TURNS=0

run() {
    TURNS=$((TURNS+1))
    echo "--- Turn $TURNS: $@ ---"
    if ! braid -p "$STORE" "$@" 2>&1; then
        ERRORS=$((ERRORS+1))
        echo "  [ERROR at turn $TURNS]"
    fi
}

# Turn 1: Init + status (basin establishment)
braid init -p "$STORE"
run status

# Turn 2-4: Learn the grammar from task operations
run task create "First task" --type task --priority 2
run task list
run task show <id-from-previous>

# Turn 5: Predict observation grammar from task pattern
run observation create "First observation" --confidence 0.8
# If this fails, AGCR < target at turn 5

# Turn 6-8: Verify symmetric access
run observation list
run observation search "observation"
run observation show <id>

# Turn 9-11: Session lifecycle
run session start
run session harvest --commit
run session seed --task "E2E test"

# Turn 12-13: Boundary and trace
run boundary status
run trace scan --commit

# Turn 14-15: Challenge and witness
run challenge create <entity>
run witness status

echo "=== RESULTS ==="
echo "Turns: $TURNS, Errors: $ERRORS"
echo "AGCR: $(echo "scale=2; ($TURNS - $ERRORS) / $TURNS" | bc)"
echo "Target: > 0.90"
```

### 8.5 DOGFOOD protocol

DOGFOOD-5: Deploy on Go CLI project (`../ddis-cli/`).
- 3 independent agents, 30-minute sessions
- Measure: overall usefulness, discovery assistance, grammar convergence
- Target: overall > 8.0/10, AGCR > 0.9 by turn 5
- Compare against DOGFOOD baseline (6.75/10)

---

## 9. SPEC ELEMENT SUMMARY

| ID | Type | Statement |
|---|---|---|
| ADR-CLI-001 | ADR | Noun-verb grammar with shortcut layer |
| INV-CLI-001 | Invariant | Grammar completeness: every resource has list/show/search |
| INV-CLI-002 | Invariant | Vocabulary consistency: one term per concept everywhere |
| INV-CLI-003 | Invariant | Output shape consistency: all lists/shows/searches match template |
| INV-CLI-004 | Invariant | Error steering: every error teaches the correct action |
| INV-CLI-005 | Invariant | Response steering: every mutation response has what/changed/next |
| INV-CLI-006 | Invariant | Progressive disclosure: default output <= 10 lines |

All trace to: STEERING_MANIFOLD.md, ADR-FOUNDATION-014, ADR-FOUNDATION-034.
All falsifiable with automated tests (§8).

---

## 10. ENFORCEMENT (C10 — Hard Constraint)

CLI coherence is not a guideline — it is a hard constraint (C10 in AGENTS.md).
Enforcement is mechanical, not procedural. Three layers, each catching what
the previous misses:

### 10.1 Layer 1: Type System (compile-time)

Violations that cannot compile:

**ResourceCommand trait** (enforces INV-CLI-001):
```rust
/// Every CLI resource MUST implement this trait. The compiler rejects
/// any resource that lacks list/show/search. This IS INV-CLI-001.
pub trait ResourceCommand {
    fn list(&self, store: &Store, flags: &CommonFlags) -> Result<ListOutput>;
    fn show(&self, store: &Store, id: &str, flags: &CommonFlags) -> Result<DetailOutput>;
    fn search(&self, store: &Store, query: &str, flags: &CommonFlags) -> Result<SearchOutput>;
}
```

**SteeringError type** (enforces INV-CLI-004):
```rust
/// Private fields + single constructor = cannot create an error without
/// all three steering components. This IS INV-CLI-004.
pub struct SteeringError {
    what_wrong: String,    // required — private, no Default
    how_to_fix: String,    // required — private, no Default
    reference: String,     // required — private, no Default
}
impl SteeringError {
    pub fn new(what: impl Into<String>, fix: impl Into<String>, ref_: impl Into<String>) -> Self;
    // This is the ONLY constructor. No Default, no From<String>, no bare new().
}
```

**CreateResponse type** (enforces INV-CLI-005):
```rust
/// Mutation responses MUST include all three steering components.
/// Cannot construct without them. This IS INV-CLI-005.
pub struct CreateResponse {
    what_happened: String,       // required
    what_changed: StoreChange,   // required (datom count, concept membership)
    steering: SteeringVector,    // required (concept + gap + question)
}
```

**Result**: An agent adding a new resource that lacks list/show/search gets a
compilation error. An agent emitting a bare error string gets a type mismatch.
An agent returning a receipt-only response can't construct CreateResponse.
The invariants are UNBYPASSABLE without explicitly opting out.

### 10.2 Layer 2: Test Suite (test-time)

Catches semantic violations the type system can't express:

| Test | Invariant | What it catches |
|---|---|---|
| `test_grammar_completeness` | INV-CLI-001 | Resources registered in clap but missing from ResourceCommand |
| `test_vocabulary_consistency` | INV-CLI-002 | Non-canonical terms in help/error/output strings |
| `test_output_shapes` | INV-CLI-003 | List/show/search outputs that don't match templates |
| `test_error_steering` | INV-CLI-004 | Error paths that bypass SteeringError (e.g., raw anyhow) |
| `test_response_steering` | INV-CLI-005 | Mutation commands that bypass CreateResponse |
| `test_progressive_disclosure` | INV-CLI-006 | Default output exceeding 10 lines |
| `test_shortcut_equivalence` | ADR-CLI-001 | Shortcuts producing different output than canonical |

These run on every `cargo test`. Any violation fails the build.

### 10.3 Layer 3: Spec-First Gate (design-time)

Any change to the CLI surface (new command, flag, output, error) MUST:

1. **Update CLI_COHERENCE_SPEC.md FIRST** — add the resource/action to the grammar (§2),
   add any new vocabulary to the lexicon (§6), define the output template (§4),
   define error paths (§5).

2. **Derive the implementation from the spec** — the types (Layer 1) and tests (Layer 2)
   are already defined by the spec. The implementation fills them in.

3. **Pass the steering review** — every new CLI surface must answer:
   "Does this output steer the agent toward the highest-value next action?"
   "Does this command name activate the right region of the LLM's knowledge manifold?"
   "Would an LLM predict this command exists from the grammar pattern alone?"

### 10.4 Why Three Layers

| Layer | What it catches | When | Bypass cost |
|---|---|---|---|
| Type system | Structural violations | Compile time | Must change the types (visible, reviewable) |
| Test suite | Semantic violations | Test time | Must delete the test (visible, reviewable) |
| Spec-first gate | Design violations | Design time | Must update the spec (visible, reviewable) |

Every bypass is visible and reviewable. No silent degradation. The grammar
can only change through an explicit, documented spec change — never through
an implementation that "just works" but violates the coherence invariants.
