# Session 047: Sub-1s Performance — Cleanroom Rebuild

> **Identity**: You are executing the PERF-ZERO epic — the performance
> endgame for braid. Every CLI command completes in <1s, including
> `status --deep`. This is not optimization — it is a formal correctness
> property: INV-PERF-001.

---

## The Invariant

```
INV-PERF-001: Sub-Second Universal Latency
  Formal: ∀ cmd ∈ {status, status --deep, observe, task list,
          task ready, harvest --commit, seed --inject, query}.
          wall_clock(cmd, store) < 1000ms
          where |store.datoms| ≤ 500K
  Falsification: Any command exceeding 1000ms on a ≤500K datom store.
  Oracle: scripts/e2e_sub1s_guard.sh → PASS/FAIL per command
```

Current baselines (108K datoms):

| Command | Current | Target | Gap |
|---------|---------|--------|-----|
| `status` | 5.2s | <1s | 5.2× |
| `status --deep` | HANGS | <1s | ∞ |
| `observe` | 3.3s | <1s | 3.3× |
| `task list` | 3.4s | <1s | 3.4× |
| `seed --inject` | 5.8s | <1s | 5.8× |
| `query` | 4.2s | <1s | 4.2× |

---

## The Three Bottleneck Layers

A complete audit (Session 046, 3 subagents, 126K LOC) identified three
independent layers of cost. Fixing any one layer alone is insufficient.
All three must be addressed simultaneously.

### Layer 1: Store Loading (3-9s per command)

`main.rs` opens `LiveStore` 2-3 times per command invocation:
1. Session auto-detect (lines 108-132): open, write session tx, **drop** (serializes 104MB)
2. Command dispatch: open again (deserializes 104MB)
3. Post-command hooks (lines 178-188): open a THIRD time

Each `LiveStore::open()` deserializes 104MB bincode = ~3s. `braid observe`
pays 9s in deserialization alone.

**Fix**: Two changes, both mandatory:
- **SINGLE-LIVESTORE**: One `LiveStore` per process, threaded through all phases
- **DAEMON-MANDATORY**: Auto-start daemon on first command. Subsequent commands
  route through Unix socket to warm in-memory store. 0ms deserialization.

### Layer 2: Redundant O(N) Computation (1-2s)

MaterializedViews maintains F(S) at O(1) but 12 other derived values are
batch-recomputed on every invocation:

| Function | Calls per status | Scans | Materialized? |
|----------|-----------------|-------|---------------|
| `compute_fitness_from_policy()` | 1 | **~20** (entities_matching_pattern per boundary) | No |
| `telemetry_from_store()` | 1 | **7** | No |
| `all_tasks()` | **4** | 4 | Partial (views.task_status_live exists, unused) |
| `live_projections()` | **5** (in --deep) | 5 | Done (views.isp_* wired in Session 046) |
| `compute_beta_1()` | 1 | 1 | Partial (views.ref_graph_stats exists, unused) |
| `formality_level()` | per entity | N/entity | No (uses datoms() instead of entity_datoms()) |

**Fix**: Wire ALL MaterializedViews consumers. Every derived value that is
a commutative monoid homomorphism from (P(D), ∪) → Aᵢ belongs in
`observe_datom`. The key algebraic invariant:

```
∀ sequence d₁...dₙ.
  fold(observe_datom, d₁...dₙ) == batch_compute(∪{d₁...dₙ})
```

### Layer 3: Spectral O(n³) (status --deep hangs)

`status --deep --spectral` runs eigendecomposition on a 10K-node entity
graph. O(n³) = 10¹² operations. Even `--deep` without `--spectral` runs
the bilateral cycle with 5+ O(N) scans.

**Fix**: Disk-cached computation results, keyed by `txn_fingerprint`.
First run computes and caches. Subsequent runs read from cache until the
store changes. The bilateral cycle result and spectral certificate are
pure functions of the store state — deterministic and cacheable.

---

## Execution Protocol

### Phase 1: FORMALIZE (this prompt)

Tear down the existing PERF-REGRESSION task tree (stale targets, missing
layers, wrong dependencies). Rebuild from the complete audit.

Before writing any code:
1. Read `braid status --verbose` output to understand current state
2. Read all existing PERF-REGRESSION tasks: `braid task search "PERF"`
3. Close all stale/superseded tasks with documented reasons
4. Crystallize INV-PERF-001 as a spec element in the store
5. Create the definitive task tree with full dependency DAG

The task tree must cover ALL three layers with no gaps:

```
EPIC: PERF-ZERO — Sub-1s Universal Latency (INV-PERF-001)
│
├─ LAYER-1: Eliminate Store Loading
│  ├─ L1-SINGLE: Single LiveStore per process (main.rs refactor)
│  ├─ L1-SLIM: Slim cache format (datoms + views only, ~12MB vs 104MB)
│  ├─ L1-DAEMON: Mandatory daemon auto-start (lazy fork, never blocks)
│  └─ L1-TEST: Verify single-open + daemon routing + cold-start < 1s
│
├─ LAYER-2: Materialize All Derived State
│  ├─ L2-POLICY-FITNESS: Eliminate 20 scans in compute_fitness_from_policy
│  ├─ L2-TELEMETRY: Fold 7 telemetry scans into observe_datom
│  ├─ L2-TASKS: Wire task_counts to views.task_counts_live()
│  ├─ L2-BETA1: Wire compute_beta_1 to views.ref_graph_stats
│  ├─ L2-FORMALITY: Fix formality_level() O(N) → entity_datoms() O(1)
│  ├─ L2-QUERY: Fix query to use attribute_index when --attribute given
│  └─ L2-TEST: Isomorphism proof (incremental == batch for ALL domains)
│
├─ LAYER-3: Cache Expensive Computations
│  ├─ L3-BILATERAL: Disk-cache bilateral cycle result per txn_fingerprint
│  ├─ L3-SPECTRAL: Disk-cache spectral certificate per txn_fingerprint
│  ├─ L3-SEED-TF-IDF: Cache inverted index for seed TF-IDF association
│  └─ L3-TEST: Verify cache hit/miss/invalidation correctness
│
└─ CAPSTONE
   ├─ E2E-SUB1S: scripts/e2e_sub1s_guard.sh (all commands, 3 runs, median)
   └─ INV-PERF-001: Spec element crystallized in store
```

### Phase 2: IMPLEMENT

Execute the task tree. For each task:
1. Mark in-progress in braid
2. Implement with zero-defect discipline
3. Run `cargo test --all-targets` — 0 failures
4. Measure latency improvement
5. Close task in braid with measured results
6. If implementation reveals new issues: observe → crystallize → task

### Phase 3: VERIFY

After all tasks complete:
1. Run `scripts/e2e_sub1s_guard.sh` — ALL commands < 1s
2. Run full test suite — 2040+ tests, 0 failures
3. Measure with daemon running and without (embedded mode)
4. Commit, push, harvest, seed

---

## Hard Constraints

- **C1**: Append-only store. NEVER edit .braid/txns/ files.
- **C8**: Substrate independence. No domain-specific logic in kernel.
- **C9**: Parameter substrate independence. Config overrides for all thresholds.
- **INV-PERF-001**: All commands < 1s at ≤500K datoms.
- **NEG-001**: No aspirational stubs. Implement fully or don't create.
- No `unwrap()` in production code. `Result` everywhere.

---

## Quality Bar

Here is what done looks like — the existing `index_datom` from Session 046:

```rust
fn index_datom(&mut self, d: &Datom) {
    self.views.observe_datom(d);
    self.entity_index.entry(d.entity).or_default().push(d.clone());
    self.attribute_index.entry(d.attribute.clone()).or_default().push(d.clone());
    if let Value::Ref(target) = &d.value {
        self.vaet_index.entry(*target).or_default().push(d.clone());
    }
    if d.op == Op::Assert {
        self.avet_index
            .entry((d.attribute.clone(), d.value.clone()))
            .or_default()
            .push(d.clone());
        let key = (d.entity, d.attribute.clone());
        self.live_view
            .entry(key)
            .and_modify(|(v, tx)| { if d.tx > *tx { *v = d.value.clone(); *tx = d.tx; } })
            .or_insert((d.value.clone(), d.tx));
    }
    if d.op == Op::Retract {
        let key = (d.entity, d.attribute.clone());
        if let Some((existing_val, existing_tx)) = self.live_view.get(&key) {
            if *existing_val == d.value && d.tx >= *existing_tx {
                self.live_view.remove(&key);
            }
        }
    }
}
```

Single function. Single responsibility. Every index maintained atomically.
No duplication. No unwrap. Pure computation. This is the bar.

---

## What Success Looks Like

```
$ time braid status
store: .braid (500K datoms, 50K entities)
F(S)=0.75 | coherence: Coherent | M(t)=0.60
tasks: 50 open | harvest: 3 tx since last
→ braid observe "..." | braid harvest --commit

real    0m0.450s

$ time braid status --deep
bilateral: F(S)=0.75 [cached]
spectral: entropy=2.31 [cached, txn_fingerprint=abc123]
trajectory: converging (5 sessions)

real    0m0.780s

$ time braid observe "the merge module has a race condition"
observed: :observation/the-merge-module-has-a-race-condition
[merge-patterns] expected: {cascade, pipeline}. You found: {race, condition}.
NEW TERRITORY: beyond merge-patterns (surprise=0.62)

real    0m0.120s
```

Every command under 1 second. The system feels instant. The agent never
waits. Knowledge flows at the speed of thought.
