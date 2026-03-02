# §7. MERGE (Basic) — Build Plan

> **Spec reference**: [spec/07-merge.md](../spec/07-merge.md) — read FIRST
> **Stage 0 elements**: INV-MERGE-001, INV-MERGE-008 only (2 INV), ADR-MERGE-001, NEG-MERGE-001, NEG-MERGE-003
> **Dependencies**: STORE (§1), SCHEMA (§2), QUERY (§3), RESOLUTION (§4)
> **Cognitive mode**: Set-theoretic — union, deduplication, monotonicity

---

## §7.1 Scope

Stage 0 requires only the minimal merge subset:

- **INV-MERGE-001**: Merge preserves all datoms — `S ⊆ merge(S, S')` for both inputs.
- **INV-MERGE-008**: Merge receipt records the operation — count of new datoms, frontier delta.

Branching (INV-MERGE-002–007), W_α working sets, and merge cascade are **deferred to Stage 2**.
Stage 0 merge is pure set union of two flat stores.

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

    // INV-MERGE-008: Receipt correctness
    fn inv_merge_008(s1 in arb_store(3), s2 in arb_store(3)) {
        let pre_len = s1.len();
        let mut target = s1.clone();
        let receipt = merge(&mut target, &s2);
        prop_assert_eq!(receipt.new_datoms + pre_len, target.len());
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
- [ ] `MergeReceipt` records statistics correctly
- [ ] Content-identity deduplication works (BTreeSet)
- [ ] Frontier merged (pointwise max per agent)
- [ ] Indexes updated after merge
- [ ] No datom loss verified (proptest + Kani)
- [ ] Commutativity/associativity/idempotency hold (from STORE tests)
- [ ] Integration: two independent stores merge cleanly

---
