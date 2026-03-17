# Continuation Guide — Stage 0/1 Remediation

> Written: 2026-03-17 | For: Future agents and sessions
> This document is the handoff from the audit session to the implementation sessions.

## What Was Done (2026-03-17 Audit Session)

### Phase 0: Correctness Fixes (COMPLETE)
5 fixes applied, all tests green (1,012 pass, 0 fail):
1. `EntityId::from_raw_bytes` restricted to `pub(crate)` — datom.rs:66
2. `serialize_tx` now sorts datoms and causal_predecessors — layout.rs:225-237
3. False SYNC witness annotations removed — merge.rs:99-110
4. `generated_coherence_tests.rs` replaced with compilable placeholder
5. Genesis attribute count reconciled to 19 across 16 files

### Phase 1: Task Creation (COMPLETE)
228 tasks in braid datom store with 160+ dependency edges:
- 12 epics, 189 implementation, 30 test, 30 docs
- Store: 11,680 datoms, 651 transactions

### Audit Documents (COMPLETE)
13 files in `docs/audits/stage-0-1/` (252 KB, 4,242 lines):
- 00: Executive summary with formal methods assessment
- 01-07: Domain audit findings (124 total)
- 08-11: Cross-cutting synthesis reports
- 12: Execution plan (1,464 lines, 8 waves)

### Commits (PUSHED)
- `c5fe6a8` Phase 0 correctness fixes
- `50d0fbe` Genesis count reconciliation (39 edits, 16 files)
- `d611d60` Audit documents (13 files)
- `509db12` Braid store with 228 tasks (491 files)

---

## What To Do Next (Priority Order)

### Session N+1: "The Bootstrap Session"

**Goal**: Reach the inflection point where `braid next` directs work instead of
a static execution plan.

**Step 1**: Fix `budget.rs:86` Unicode boundary panic (5 min)
```
File: crates/braid-kernel/src/budget.rs:86
Bug: String truncation at byte boundary inside multi-byte UTF-8 char (checkmark/cross)
Fix: Use char_indices() to find the nearest char boundary, or truncate before
     inserting multi-byte chars
Impact: Every braid command panics when output contains Unicode symbols at truncation point
```

**Step 2**: Wire R(t) routing to real store tasks — task t-f2f3 (M effort)
```
File: crates/braid-kernel/src/guidance.rs
Current: compute_routing() takes synthetic TaskNode inputs
Needed: compute_routing_from_store(store: &Store) that:
  1. Calls all_tasks(store) to get real tasks
  2. Builds TaskNode graph from :task/depends-on edges
  3. Computes composite impact scores
  4. Returns ranked TaskRouting results
Integration: Wire into derive_actions() so guidance footer shows R(t)-ranked next task
Test: Verify braid next returns highest-impact ready task, not just highest priority
```

**Step 3**: Wire intention anchoring in seed — task t-acf0 (M effort)
```
File: crates/braid-kernel/src/seed.rs
Current: Directive section contains generic guidance
Needed: Query active intentions (tasks with status=in-progress) and pin them at pi_0
Sub-tasks: t-5910 (intention querying), t-5b3a (pi_0 pinning), t-4a9e (budget signal)
```

**After Steps 2-3**: The operational loop becomes:
```bash
braid seed --task "$(braid next --json | jq -r .title)"  # context
braid go <id>                                              # claim
# ... work ...
braid done <id>                                            # close
braid harvest --commit                                     # persist
```

### Session N+2: Wave A — Critical Correctness (P0)

**Goal**: Close all P0 tasks (merge cascade, conflict predicate, lattice_id).

Start with: `braid task update t-bcee --status in-progress` (Merge Cascade epic)

Key tasks in dependency order:
1. t-2cc1: Fix MergeReceipt fields (duplicate_datoms, frontier_delta)
2. t-f869: Implement CascadeReceipt struct
3. t-b70c: Cascade step 1 — conflict detection after merge
4. t-2dbb: Cascade steps 2-5 stub datoms (ADR-MERGE-007)
5. t-6094: Wire cascade into Store::merge return type
6. t-0246: Integration — merge cascade end-to-end
7. t-8ef5: Fix Kani proof for INV-MERGE-002

Parallel track (no dependency on cascade):
- t-eb7f: Implement is_causal_ancestor() BFS walk
- t-152b: Fix has_conflict causal independence check
- t-70b0: Add lattice_id to ResolutionMode::Lattice
- t-7e8a: Register :resolution/* attributes in genesis schema

### Session N+3+: Wave B — Stage 0 Close (P1)

See `12-execution-plan.md` Wave B for full detail. Key items:
- t-0d9c: Deliberation stability guard (6 conditions)
- t-77a9: Fix DeliberationStatus Ord derivation
- t-acf0: Intention anchoring (if not done in N+1)
- t-ee9a + t-7625: Fix seed budget tautology
- t-a581: Fix causal predecessor error variant
- t-ca63: Align harvest warning thresholds
- t-f684 + t-f671: Layout hash verification on read

---

## How To Start Any Session

```bash
# 1. Orient
braid status                    # Dashboard, F(S), M(t), task counts
braid task ready                # What's unblocked

# 2. Pick work
braid next                      # Highest-impact ready task (after R(t) wiring)
# OR: braid task ready | head -5  # Manual selection (before R(t) wiring)

# 3. Claim
braid go <task-id>              # Mark in-progress

# 4. Work
# Read the task title for the spec INV it addresses
# Read the corresponding spec file and guide file
# Implement, test, verify with cargo check/test/clippy

# 5. Close
braid done <task-id> --reason "Implemented in <commit>"

# 6. Harvest
braid harvest --commit          # Persist session knowledge
braid seed --inject AGENTS.md   # Refresh seed for next session
```

---

## Key Files Reference

| Purpose | File |
|---------|------|
| Execution plan | docs/audits/stage-0-1/12-execution-plan.md |
| Executive summary | docs/audits/stage-0-1/00-executive-summary.md |
| Domain findings | docs/audits/stage-0-1/01-07*.md |
| Synthesis reports | docs/audits/stage-0-1/08-11*.md |
| Spec elements | spec/*.md (22 namespaces) |
| Implementation guides | docs/guide/*.md |
| Design decisions | docs/design/ADRS.md |
| Failure modes | docs/design/FAILURE_MODES.md |
| SEED document | SEED.md |

---

## Success Metrics

| Metric | Current | Stage 0 Target | Stage 1 Target |
|--------|---------|----------------|----------------|
| Stage 0 INVs implemented | 62/83 (75%) | 83/83 (100%) | — |
| Stage 1 INVs implemented | 12/26 (46%) | — | 26/26 (100%) |
| F(S) | ~0.65 | >= 0.85 | >= 0.95 |
| M(t) | 0.61 | >= 0.70 | >= 0.80 |
| Tests passing | 1,012 | 1,200+ | 1,500+ |
| False witnesses | 0 (was 83) | 0 | 0 |
| Open P0 tasks | 11 | 0 | 0 |
| Open P1 tasks | 69 | 0 | 0 |

---

## The Self-Bootstrap Principle

This remediation plan is itself an instance of what it fixes. The execution plan
(12-execution-plan.md) is a hand-compiled coordination topology — T = (G, Phi, Sigma, Pi)
from spec/19-topology.md expressed in prose. As we implement the tasks in the plan,
the system gains the capability to generate this document automatically:

- t-f2f3 (R(t) wiring) → `braid next` replaces "which task next?"
- t-acf0 (intention anchoring) → `braid seed` replaces "what context?"
- Stage 3 topology compilation → `braid compile --topology` replaces the entire plan

The plan's terminal state is its own obsolescence. Every session that follows this
plan and harvests its outcomes teaches the system what good coordination looks like.
The plan is not documentation — it is training data for its own replacement.
