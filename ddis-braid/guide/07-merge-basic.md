# §7. MERGE (Basic) — Build Plan

> **Spec reference**: [spec/07-merge.md](../spec/07-merge.md) — read FIRST
> **Stage 0 elements**: INV-MERGE-001–002, 008–010 (5 INV), ADR-MERGE-001, ADR-MERGE-005, NEG-MERGE-001, NEG-MERGE-003
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
- **INV-MERGE-010**: Cascade Determinism — cascade is a pure function of the merged datom set.
  No agent ID, timestamps, or external state may influence cascade output. This is what
  restores L1/L2 at the full post-cascade store level (ADR-MERGE-005).

Branching (INV-MERGE-003–007), W_α working sets are **deferred to Stage 2**.
Stage 0 merge is pure set union of two flat stores with deterministic cascade.

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

### Merge Cascade (INV-MERGE-002, INV-MERGE-010)

**Black box** (contract):
- INV-MERGE-002: Every merge executes all 5 cascade steps, each producing datoms:
  (1) conflict detection, (2) cache invalidation, (3) projection staleness,
  (4) uncertainty update, (5) subscription notification.
  The cascade is atomic — either all 5 steps complete or the merge fails.
- INV-MERGE-010: Cascade is a **deterministic function of the merged datom set**.
  The function signature enforces this — it takes `&Store` and `&[Datom]` only.
  No `AgentId`, no `SystemTime`, no RNG. Two agents independently merging the
  same two stores produce identical cascade datom sets.

**State box** (internal design):
- After set union (INV-MERGE-001), the cascade runs sequentially on newly-inserted datoms.
- Each step queries the newly-merged state and produces metadata datoms.
- Cascade datom identity is derived from the conflict/change content itself
  (content-addressable, INV-STORE-003), not from who detected it or when.
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

/// INV-MERGE-010: Cascade takes ONLY &Store and &[Datom].
/// No AgentId, no SystemTime, no RNG — determinism by construction.
/// Two agents merging the same stores produce identical cascade output.
fn run_cascade(
    store: &Store,
    new_datoms: &[Datom],
) -> CascadeReceipt {
    let mut receipt = CascadeReceipt::default();

    // (1) Conflict detection — find new conflicts from merged datoms
    //     detect_new_conflicts reads only store state (schema, existing datoms)
    let conflicts = detect_new_conflicts(store, new_datoms);
    receipt.conflicts_detected = conflicts.len();
    for c in &conflicts {
        receipt.cascade_datoms.push(cascade_conflict_datom(c));
    }

    // (2)–(5) Stub datoms per ADR-MERGE-007.
    // Steps 2-5 produce stub datoms at Stage 0.
    // Full implementations are Stage 2+ deliverables per ADR-MERGE-007.
    for step in &["cache-invalidation", "projection-staleness", "uncertainty-delta", "subscription-notification"] {
        let stub = cascade_stub(step, store);
        match *step {
            "cache-invalidation" => receipt.caches_invalidated = 0,
            "projection-staleness" => receipt.projections_staled = 0,
            "uncertainty-delta" => receipt.uncertainties_updated = 0,
            "subscription-notification" => receipt.notifications_sent = 0,
            _ => {}
        }
        receipt.cascade_datoms.extend(stub);
    }

    receipt
}

fn cascade_stub(step: &str, merged: &Store) -> Vec<Datom> {
    // Steps 2-5 produce stub datoms at Stage 0.
    // Full implementations are Stage 2+ deliverables per ADR-MERGE-007.
    vec![Datom::new(
        EntityId::from_content(format!("cascade:{}", step).as_bytes()),
        Attribute::new(":merge/cascade-step").unwrap(),
        Value::String(format!("{}: 0 items processed (stub)", step)),
        merged.frontier().values().max().copied().unwrap_or_default(),
        Op::Assert,
    )]
}

/// Cascade datom identity derived from conflict content, not from who detected it.
/// Same conflict always produces same datom (content-addressable, INV-STORE-003).
fn cascade_conflict_datom(conflict: &Conflict) -> Datom {
    let entity_id = EntityId::from_content(&[
        conflict.entity.as_bytes(),
        conflict.attribute.as_bytes(),
        conflict.value_a.as_bytes(),
        conflict.value_b.as_bytes(),
    ]);
    Datom::new(entity_id, Attribute::cascade_conflict(), /* ... */)
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

    // INV-MERGE-010: Cascade determinism — same merged state, same cascade output
    fn inv_merge_010(s1 in arb_store(5), s2 in arb_store(5)) {
        // Merge in order A∪B
        let mut target_ab = s1.clone();
        merge(&mut target_ab, &s2);
        let cascade_ab = run_cascade(&target_ab, /* new datoms from merge */);

        // Merge in order B∪A
        let mut target_ba = s2.clone();
        merge(&mut target_ba, &s1);
        let cascade_ba = run_cascade(&target_ba, /* new datoms from merge */);

        // Cascade outputs must be identical (determinism + commutativity)
        let datoms_ab: BTreeSet<Datom> = cascade_ab.cascade_datoms.into_iter().collect();
        let datoms_ba: BTreeSet<Datom> = cascade_ba.cascade_datoms.into_iter().collect();
        prop_assert_eq!(datoms_ab, datoms_ba,
            "Cascade determinism violation: different cascade datoms for same merged state");
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
- [ ] `run_cascade()` takes only `&Store` + `&[Datom]` — no AgentId, no clock, no RNG (INV-MERGE-010)
- [ ] Cascade datom identity is content-addressable from conflict/change content (INV-MERGE-010)
- [ ] Cascade determinism proptest passes: merge(A,B) and merge(B,A) produce identical cascade datoms
- [ ] Integration: two independent stores merge cleanly

---
