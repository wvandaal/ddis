# D4 — K_agent Harvest Epistemology

> **Thread**: R3.4a — How does harvest detect "un-transacted knowledge"?
> **Date**: 2026-03-03
> **Sources**: spec/05-harvest.md, docs/guide/05-harvest.md

---

## Research Questions

1. How does the agent know what it knows?
2. How does harvest detect "un-transacted knowledge"?
3. What is the formal model for the epistemic gap?
4. What is the K_agent / K_store boundary and how is it operationalized?
5. Is the detection mechanism feasible at Stage 0?

---

## The Formal Model (from spec/05-harvest.md)

### Epistemic Gap Definition

```
K_agent(t) = knowledge held by the agent at time t (in conversation context)
K_store(t) = knowledge in the datom store at time t

Epistemic gap: Delta(t) = K_agent(t) \ K_store(t)
  (knowledge the agent has that the store does not)

Harvest: HARVEST(Delta(t)) -> K_store(t') where K_store(t') >= K_store(t) union Delta(t)

Perfect harvest: Delta(t') = empty
Practical harvest: |Delta(t')| <= epsilon
```

This is mathematically clean but operationally challenging: **K_agent(t) is not
directly observable.** The agent's knowledge is implicit in its conversation
history, tool call results, and internal reasoning. There is no API to query
"what does this LLM know right now?"

---

## The Detection Problem

### What K_store(t) Contains (Observable)

K_store(t) is fully known — it is the set of all datoms in the store. We can
enumerate it, query it, diff it against any baseline.

### What K_agent(t) Contains (Partially Observable)

K_agent(t) is the union of:

1. **Seed knowledge**: What was loaded at session start (from SEED operation).
   This is known because the seed was assembled from the store.

2. **Transaction knowledge**: Results of explicit `braid transact` calls during
   the session. These are in K_store(t) by definition.

3. **Query knowledge**: Results of `braid query` calls. These are projections of
   K_store(t) — the agent saw them but they are already in the store.

4. **Tool output knowledge**: Results of non-braid tool calls (bash, file reads,
   web searches). This is the **primary source of epistemic gap**. The agent
   observed something from the environment but has not transacted it.

5. **Reasoning knowledge**: Conclusions the agent derived from combining facts.
   Decisions made, patterns noticed, uncertainties identified. This is the
   **hardest to detect** because it exists only in the LLM's hidden state.

### The Gap is in Categories 4 and 5

Category 4 (tool outputs) is partially observable: we can examine the session's
tool call log and identify results that were not subsequently transacted. If the
agent ran `ls -la` and saw that a file exists, that fact is knowledge the agent
has but the store may not.

Category 5 (reasoning) is fundamentally unobservable without asking the agent.
The agent may have concluded "this approach won't work" from a chain of
observations, but that conclusion exists only in the conversation context.

---

## The Guide's Implementation Approach (from docs/guide/05-harvest.md)

The guide operationalizes detection through a five-stage pipeline:

### Stage 1: DETECT

> Compare session transactions against store state. For each tx in session,
> check: are all implied observations transacted? Are decisions recorded as
> ADR entities? Are discovered dependencies linked? Are uncertainties marked?

The guide explicitly acknowledges:

> For Stage 0: detection is LLM-assisted. The harvest command presents the
> session's transaction log and asks the agent to identify gaps. As the system
> matures, detection becomes increasingly automated.

This is the critical insight: **at Stage 0, the agent itself is the detector.**
Harvest is a semi-automated protocol, not a fully automated scanner.

### The Detection Heuristics

What can be mechanically detected (without LLM assistance):

| Signal | Detection Method | Confidence |
|--------|-----------------|------------|
| File read without observation datom | Compare tool log to store | High |
| Decision made in conversation without ADR | Parse for decision-language | Medium |
| Error encountered without uncertainty datom | Check tool exit codes | High |
| Dependency discovered without link | Compare entity references | Medium |
| Test run without result datom | Check test tool calls | High |

What requires LLM assistance:

| Signal | Why Mechanical Detection Fails |
|--------|-------------------------------|
| Implicit conclusions | No tool call to detect |
| Design trade-off assessments | Requires understanding context |
| Confidence levels on observations | Subjective assessment |
| Categorization (observation vs. decision) | Requires semantic understanding |

---

## The Boundary Problem

The formal model assumes a clean boundary between K_agent and K_store. In practice:

### Problem 1: Knowledge Granularity

When the agent reads a 500-line file, what "knowledge" was gained? The file's
existence? Its content? Specific patterns within it? A conclusion drawn from
pattern A interacting with pattern B? The granularity of "knowledge unit" is
undefined.

**Resolution**: The harvest candidate's `datom_spec` field defines the granularity.
Each candidate is a set of datoms — specific, structured facts. The agent must
decompose its unstructured knowledge into structured datom-sized claims. This is
inherently lossy: the full nuance of the agent's understanding cannot be captured
in datoms. This is by design (ADR-HARVEST-002: conversations are disposable,
knowledge is durable). The datoms capture the durable residue; the nuance dies
with the conversation.

### Problem 2: Knowledge Freshness

An observation made early in the session may be stale by session end. The spec
addresses this with the observation staleness model:

```
:observation/source    — :filesystem | :shell | :network | :git | :process
:observation/timestamp — when observed
:observation/hash      — content hash at observation time
:observation/stale-after — TTL (source-dependent)
```

But at harvest time, the agent may not remember when it made an observation.
The session context's `recent_transactions` list helps but doesn't cover
observations that were never transacted.

**Resolution**: The tool call log (accessible via session history) provides
timestamps for all tool-derived knowledge. The stale-after TTL is applied
at harvest time by comparing tool call timestamp to current time.

### Problem 3: Recursive Epistemic Uncertainty

The agent doesn't know what it doesn't know. A harvest that returns 0 candidates
could mean either (a) all knowledge is transacted (ideal) or (b) detection
missed gaps. The spec acknowledges this:

> Empty harvest: "0 candidates. Either all knowledge is already transacted (ideal)
> or detection missed gaps. Run `braid status` to check drift score."

**Resolution**: The drift score provides a cross-check. If drift_score > 0 but
candidates = 0, the detector is missing gaps. Over time, FP/FN tracking
(INV-HARVEST-004, Stage 1) calibrates the detection thresholds.

---

## Operationalizing for Stage 0

### The Minimal Viable Harvest

At Stage 0, the harvest pipeline is:

1. **DETECT (LLM-assisted)**: Present the session's transaction log and tool call
   history to the agent. Ask: "What knowledge exists in this session that is not
   yet in the store?"

2. **PROPOSE**: The agent generates harvest candidates with category, confidence,
   and datom specifications.

3. **REVIEW**: Self-review (single agent reviews own harvest proposals).

4. **COMMIT**: Accepted candidates are transacted via `Store::transact`.

5. **RECORD**: Harvest session entity created with metadata.

### What "LLM-Assisted Detection" Means Concretely

The `braid harvest` command at Stage 0:

```
1. Read session context (agent ID, task description, start TX)
2. Query store for all transactions since session start
3. Query tool call log for all tool calls since session start
4. For each tool call result:
   a. Check if a corresponding observation datom exists in store
   b. If not: generate a harvest candidate with category=Observation
5. Present candidates to the agent for categorization and refinement
6. Agent adds reasoning-derived candidates (decisions, uncertainties)
7. Agent reviews and accepts/rejects each candidate
8. Commit accepted candidates
```

Steps 1-4 are mechanical. Steps 5-7 are LLM-assisted. This is the
semi-automated model from ADR-HARVEST-001.

### The Session Context as Ground Truth

The `SessionContext` struct provides the observational substrate:

```rust
pub struct SessionContext {
    pub agent:              AgentId,
    pub session_start_tx:   TxId,
    pub recent_transactions: Vec<TxId>,
    pub task_description:   String,
}
```

The session start TX marks the "before" state. Everything since then that
is in K_agent but not in K_store is a harvest candidate. The transaction
list provides the "already captured" set. The difference is the gap.

---

## Assessment

### Strengths of the Formal Model

- The epistemic gap definition is mathematically sound
- The quality metrics (FP/FN rates) enable learning over time
- The progressive refinement path (manual -> semi-auto -> auto) is realistic
- The bounded conversation lifecycle (INV-HARVEST-007) forces regular harvests

### Weaknesses / Risks

- **K_agent is fundamentally unobservable** — the model requires the agent to
  self-report its knowledge, which is subject to the same attention degradation
  that harvest is designed to counteract
- **Category 5 knowledge (reasoning conclusions) may be systematically missed**
  at Stage 0, leading to high false negative rates
- **The "LLM-assisted" detection at Stage 0 is really "LLM-is-the-detector"** —
  the mechanical heuristics (tool call matching) catch only low-hanging fruit

### Recommendations

1. **Accept the LLM-assisted model for Stage 0.** There is no alternative that
   avoids asking the agent what it knows.
2. **Invest in the tool call log matcher** (steps 1-4 above) as the mechanical
   baseline. This catches the highest-value gaps (file observations, test results,
   errors encountered).
3. **Track FP/FN rates from day one** (INV-HARVEST-004 is Stage 1, but the data
   collection should start at Stage 0).
4. **The drift score is the key diagnostic.** If drift_score is consistently high
   but candidate count is low, the detector is broken.
5. **Consider a "harvest prompt template"** that asks the agent structured questions:
   - "What files did you read that aren't recorded?"
   - "What decisions did you make that aren't ADRs?"
   - "What uncertainties did you discover?"
   - "What dependencies exist that aren't linked?"
   This structures the LLM-assisted detection and reduces false negatives.
