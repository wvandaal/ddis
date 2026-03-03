# §7. MERGE (Basic) — Build Plan

> **Spec reference**: [spec/07-merge.md](../spec/07-merge.md) — read FIRST
> **Stage 0 elements**: INV-MERGE-001–002, 008–009 (4 INV), ADR-MERGE-001, NEG-MERGE-001, NEG-MERGE-003
> **Dependencies**: STORE (§1), SCHEMA (§2), QUERY (§3), RESOLUTION (§4)
> **Cognitive mode**: Set-theoretic — union, deduplication, monotonicity

---

## §7.1 Scope

Stage 0 requires the core merge subset:

- **INV-MERGE-001**: Merge preserves all datoms — `S ⊆ merge(S, S')` for both inputs.
- **INV-MERGE-002**: Merge Cascade Completeness — all 5 cascade steps execute: (1) conflict
  detection, (2) cache invalidation, (3) projection staleness, (4) uncertainty update,
  (5) subscription notification. Each step produces datoms recording its effects.
- **INV-MERGE-008**: At-least-once idempotent delivery — duplicate merges are harmless.
- **INV-MERGE-009**: Merge receipt records the operation — count of new datoms, frontier delta.

Branching (INV-MERGE-003–007), W_α working sets are **deferred to Stage 2**.
Stage 0 merge is pure set union of two flat stores with full cascade.

---

## §7.2 Module Structure

```
braid-kernel/src/
└── merge.rs    ← merge(), MergeReceipt
```

### Public API Surface

```rust
/// Merge two stores: mathematical set union of datom sets.
/// Returns receipt with merge statistics.
pub fn merge(target: &mut Store, source: &Store) -> MergeReceipt;

pub struct MergeReceipt {
    pub new_datoms:      usize,   // datoms in source not in target
    pub duplicate_datoms: usize,  // datoms already present (content-identity dedup)
    pub frontier_delta:  HashMap<AgentId, (Option<TxId>, TxId)>,  // (old, new) per agent
}
```

---

## §7.3 Three-Box Decomposition

### Merge (set union)

**Black box** (contract):
- INV-MERGE-001: `∀ d ∈ source: d ∈ merge(target, source)` and
  `∀ d ∈ target: d ∈ merge(target, source)`. No datom is lost.
- Commutativity, associativity, idempotency inherited from STORE CRDT laws (INV-STORE-004–006).
- NEG-MERGE-001: Merge never discards datoms from either input.

**State box** (internal design):
- Iterate `source.datoms()` → insert each into `target.datoms` BTreeSet.
- BTreeSet handles deduplication automatically (content-identity, INV-STORE-003).
- Update indexes incrementally for new datoms.
- Merge frontiers: for each agent, take max TxId.

**Clear box** (implementation):
```rust
pub fn merge(target: &mut Store, source: &Store) -> MergeReceipt {
    let pre_len = target.len();
    for datom in source.datoms() {
        target.insert_datom(datom.clone());  // BTreeSet::insert handles dedup
    }
    // Merge frontiers (pointwise max per agent)
    for (agent, tx) in source.frontier() {
        let entry = target.frontier.entry(*agent).or_insert(*tx);
        if tx > entry { *entry = *tx; }
    }
    // Rebuild affected indexes
    target.rebuild_indexes_incremental(pre_len);
    MergeReceipt {
        new_datoms: target.len() - pre_len,
        duplicate_datoms: source.len() - (target.len() - pre_len),
        frontier_delta: /* computed from pre/post frontier comparison */,
    }
}
```

### Merge Cascade (INV-MERGE-002)

**Black box** (contract):
- INV-MERGE-002: Every merge executes all 5 cascade steps, each producing datoms:
  (1) conflict detection, (2) cache invalidation, (3) projection staleness,
  (4) uncertainty update, (5) subscription notification.
  The cascade is atomic — either all 5 steps complete or the merge fails.

**State box** (internal design):
- After set union (INV-MERGE-001), the cascade runs sequentially on newly-inserted datoms.
- Each step queries the newly-merged state and produces metadata datoms.
- Stage 0 cascade is lightweight: steps 2–5 produce stub datoms recording that the step ran.
  Full cascade behavior expands in later stages.

**Clear box** (implementation):
```rust
pub struct CascadeReceipt {
    pub conflicts_detected: usize,
    pub caches_invalidated: usize,
    pub projections_staled: usize,
    pub uncertainties_updated: usize,
    pub notifications_sent: usize,
    pub cascade_datoms: Vec<Datom>,  // datoms recording cascade effects
}

fn merge_cascade(
    store: &mut Store,
    new_datoms: &[Datom],
    agent: AgentId,
) -> CascadeReceipt {
    let mut receipt = CascadeReceipt::default();
    // (1) Conflict detection — find new conflicts from merged datoms
    let conflicts = detect_new_conflicts(store, new_datoms);
    receipt.conflicts_detected = conflicts.len();
    for c in &conflicts {
        store.transact_cascade_datom(cascade_conflict_datom(c, agent));
    }
    // (2) Cache invalidation — mark LIVE cache entries stale for affected entities
    let affected = affected_entities(new_datoms);
    receipt.caches_invalidated = affected.len();
    for e in &affected {
        store.transact_cascade_datom(cascade_invalidation_datom(*e, agent));
    }
    // (3) Projection staleness — mark projections for affected entities as stale
    receipt.projections_staled = affected.len();
    for e in &affected {
        store.transact_cascade_datom(cascade_projection_datom(*e, agent));
    }
    // (4) Uncertainty update — recompute uncertainty for entities with new data
    let uncertainties = recompute_uncertainties(store, &affected);
    receipt.uncertainties_updated = uncertainties.len();
    for u in &uncertainties {
        store.transact_cascade_datom(cascade_uncertainty_datom(u, agent));
    }
    // (5) Subscription notification — notify subscribers of affected entities
    let notifications = notify_subscribers(store, &affected);
    receipt.notifications_sent = notifications.len();
    for n in &notifications {
        store.transact_cascade_datom(cascade_notification_datom(n, agent));
    }
    receipt
}
```

**proptest strategy**: Merge two arbitrary stores. Verify `CascadeReceipt` has entries for all
5 steps, and that each step produced at least one datom in the store when new datoms exist.

---

## §7.4 LLM-Facing Outputs

### Agent-Mode Output — `braid merge`

```
[MERGE] Merged {N} new datoms ({M} duplicates deduplicated).
Store: {total} datoms. Frontier updated for {agents}.
---
↳ Merge is pure set union (INV-MERGE-001). No datoms were lost.
  Check LIVE view for resolution changes: `braid entity {affected_entity}`
```

---

## §7.5 Verification

### Key Properties

```rust
proptest! {
    // INV-MERGE-001: No datom loss
    fn inv_merge_001(s1 in arb_store(5), s2 in arb_store(5)) {
        let mut target = s1.clone();
        let receipt = merge(&mut target, &s2);
        for d in s1.datoms() { prop_assert!(target.contains(d)); }
        for d in s2.datoms() { prop_assert!(target.contains(d)); }
    }

    // INV-MERGE-002: Cascade Completeness — all 5 steps produce datoms
    fn inv_merge_002(s1 in arb_store(3), s2 in arb_store(3)) {
        let mut target = s1.clone();
        let pre_len = target.len();
        let receipt = merge(&mut target, &s2);
        if receipt.new_datoms > 0 {
            // Cascade should have run and produced datoms for each step
            let cascade_datoms: Vec<_> = target.datoms()
                .filter(|d| d.attribute.name().starts_with(":cascade/"))
                .collect();
            // At least 5 cascade datoms (one per step)
            prop_assert!(cascade_datoms.len() >= 5,
                "Expected ≥5 cascade datoms, got {}", cascade_datoms.len());
        }
    }

    // INV-MERGE-008: Idempotent delivery — re-merging same store is no-op
    fn inv_merge_008(s1 in arb_store(3), s2 in arb_store(3)) {
        let mut once = s1.clone();
        let _r1 = merge(&mut once, &s2);
        let mut twice = once.clone();
        let r2 = merge(&mut twice, &s2);
        prop_assert_eq!(once.datoms().collect::<BTreeSet<_>>(),
                        twice.datoms().collect::<BTreeSet<_>>());
        prop_assert_eq!(r2.new_datoms, 0);  // No new datoms on re-merge
    }

    // INV-MERGE-009: Receipt completeness — receipt matches actual store delta
    fn inv_merge_009(s1 in arb_store(5), s2 in arb_store(5)) {
        let pre_len = s1.len();
        let mut target = s1.clone();
        let receipt = merge(&mut target, &s2);
        let post_len = target.len();
        prop_assert_eq!(receipt.new_datoms, post_len - pre_len,
            "new_datoms mismatch: receipt={}, actual={}", receipt.new_datoms, post_len - pre_len);
        prop_assert_eq!(receipt.duplicate_datoms, s2.len() - receipt.new_datoms,
            "duplicate_datoms mismatch");
    }
}
```

### Kani Harnesses

INV-MERGE-001 has V:KANI tag.

---

## §7.6 Stage 2 Extension Points

When Stage 2 adds branching:
- `merge()` gains a `MergeStrategy` parameter (currently: always `SetUnion`).
- Branch merges use the same underlying set union but track branch ancestry.
- W_α working sets use branch-scoped views.
- The `MergeReceipt` gains `conflicts: Vec<ConflictSet>` for branch-level conflict reporting.

No premature implementation needed. The `merge()` function signature supports extension
by adding parameters with defaults.

---

## §7.7 Implementation Checklist

- [ ] `merge()` function implements set union
- [ ] `MergeReceipt` records statistics correctly (INV-MERGE-009)
- [ ] Content-identity deduplication works (BTreeSet)
- [ ] Frontier merged (pointwise max per agent)
- [ ] Indexes updated after merge
- [ ] No datom loss verified (proptest + Kani)
- [ ] Commutativity/associativity/idempotency hold (from STORE tests)
- [ ] Integration: two independent stores merge cleanly

---
