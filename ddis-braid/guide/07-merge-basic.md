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
/// Merge two stores: mathematical set union of datom sets + deterministic cascade.
/// Returns (MergeReceipt, CascadeReceipt) — merge statistics and cascade effects.
///
/// Spec note (spec/07-merge.md §7.3 L2):
///   The algebraic formulation `MERGE : Store × Store → Store` is expressed in Rust as
///   `Store::merge(&mut self, other) -> (MergeReceipt, CascadeReceipt)` — the target is
///   mutated in place (avoiding a full-store copy) and the return value captures the
///   delta, not the resulting store. The spec's `merge(A,B) -> C` maps to `A.merge(B)`
///   with A mutated to become C. The receipts provide the audit trail required by
///   INV-MERGE-009 (merge receipt) and INV-MERGE-002 (cascade receipt).
pub fn merge(target: &mut Store, source: &Store) -> (MergeReceipt, CascadeReceipt);

pub struct MergeReceipt {
    pub new_datoms:      usize,   // datoms in source not in target
    pub duplicate_datoms: usize,  // datoms already present (content-identity dedup)
    pub frontier_delta:  HashMap<AgentId, (Option<TxId>, TxId)>,  // (old, new) per agent
}
```

---

## §7.3 Three-Box Decomposition

### Merge (set union)

**Precondition** (infrastructure, not a cascade step — see spec/07-merge.md §7.2):
- `Pre: target.schema().is_superset_of(source.schema())` — the target store's
  schema must be able to validate all incoming datoms. If the source introduces
  schema datoms (attributes with `:db/ident`, `:db/valueType`, `:db/cardinality`,
  `:db/resolutionMode`, or `:db/doc`), the schema is rebuilt from the merged datom
  set via `Schema::from_store()` before cascade steps execute. Schema rebuild is
  structural (ADR-SCHEMA-005), owned by Store construction, not a cascade step.

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
pub fn merge(target: &mut Store, source: &Store) -> (MergeReceipt, CascadeReceipt) {
    let pre_len = target.len();
    // Precondition: schema superset check (see spec/07-merge.md §7.2)
    // If source introduces schema datoms, Schema::from_store() rebuilds after union.
    for datom in source.datoms() {
        target.insert_datom(datom.clone());  // BTreeSet::insert handles dedup
    }
    // Merge frontiers (pointwise max per agent)
    for (agent, tx) in source.frontier() {
        let entry = target.frontier.entry(*agent).or_insert(*tx);
        if tx > entry { *entry = *tx; }
    }
    // Rebuild schema if merge introduced schema datoms (ADR-SCHEMA-005)
    target.rebuild_schema_if_needed();
    // Rebuild affected indexes
    target.rebuild_indexes_incremental(pre_len);
    let new_datoms_slice = /* datoms added in this merge */;
    let merge_receipt = MergeReceipt {
        new_datoms: target.len() - pre_len,
        duplicate_datoms: source.len() - (target.len() - pre_len),
        frontier_delta: target.frontier().iter()
            .filter(|(agent, tx)| source.frontier().get(agent) != Some(tx))
            .count(),
    };
    // Run cascade (INV-MERGE-002, INV-MERGE-010)
    let cascade_receipt = run_cascade(target, new_datoms_slice);
    (merge_receipt, cascade_receipt)
}
```

### Merge Cascade (INV-MERGE-002, INV-MERGE-010)

**Black box** (contract):
- INV-MERGE-002: Every merge executes all 5 cascade steps, each producing datoms.
  Step ordering follows spec/07-merge.md §7.2 CASCADE and INV-MERGE-002 L0 (authoritative):
  1. **Conflict detection** — find new conflicts from merged datoms
  2. **Cache invalidation** — mark query results as stale for affected entities
  3. **Projection staleness** — mark existing projections touching affected entities for refresh
  4. **Uncertainty update** — recompute σ(e) for entities with new assertions or conflicts
  5. **Subscription notification** — notify subscribers whose patterns match new datoms
  Note: ADR-MERGE-007 stub attribute names diverge (e.g., step 3 = `:cascade/secondary-conflicts`,
  step 5 = `:cascade/projection-staleness`). This guide follows the L0 definition, which is
  authoritative. Stub `:cascade/*` attribute names in the implementation should match these
  step names, not the ADR-MERGE-007 draft names.
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
    // Full implementations are Stage 1+ deliverables per ADR-MERGE-007.
    // Step names match spec/07-merge.md §7.2 CASCADE definition:
    //   Step 2: cache-invalidation     (mark query results stale)
    //   Step 3: projection-staleness   (mark projections for refresh)
    //   Step 4: uncertainty-update     (recompute σ for affected entities)
    //   Step 5: subscription-notification (notify pattern subscribers)
    for step in &["cache-invalidation", "projection-staleness", "uncertainty-update", "subscription-notification"] {
        let stub = cascade_stub(step, store);
        match *step {
            "cache-invalidation" => receipt.caches_invalidated = 0,
            "projection-staleness" => receipt.projections_staled = 0,
            "uncertainty-update" => receipt.uncertainties_updated = 0,
            "subscription-notification" => receipt.notifications_sent = 0,
            _ => {}
        }
        receipt.cascade_datoms.extend(stub);
    }

    receipt
}

fn cascade_stub(step: &str, merged: &Store) -> Vec<Datom> {
    // Steps 2-5 produce stub datoms at Stage 0.
    // Full implementations are Stage 1+ deliverables per ADR-MERGE-007.
    // Attribute namespace is :cascade/* per spec/07-merge.md §7.2 CASCADE:
    //   Step 2: :cascade/cache-invalidation        (query result staleness)
    //   Step 3: :cascade/projection-staleness       (projection refresh marking)
    //   Step 4: :cascade/uncertainty-update          (σ recomputation)
    //   Step 5: :cascade/subscription-notification   (subscriber pattern matching)
    let attr_name = format!(":cascade/{}", step);
    vec![Datom::new(
        EntityId::from_content(format!("cascade:{}", step).as_bytes()),
        Attribute::new(&attr_name).unwrap(),
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
        let (_merge_receipt, _cascade_receipt) = merge(&mut target, &s2);
        for d in s1.datoms() { prop_assert!(target.contains(d)); }
        for d in s2.datoms() { prop_assert!(target.contains(d)); }
    }

    // INV-MERGE-002: Cascade Completeness — all 5 steps produce datoms
    fn inv_merge_002(s1 in arb_store(3), s2 in arb_store(3)) {
        let mut target = s1.clone();
        let pre_len = target.len();
        let (merge_receipt, cascade_receipt) = merge(&mut target, &s2);
        if merge_receipt.new_datoms > 0 {
            // Cascade should have run and produced datoms for each step
            let cascade_datoms: Vec<_> = target.datoms()
                .filter(|d| d.attribute.name().starts_with(":cascade/"))
                .collect();
            // At least 5 cascade datoms (one per step)
            prop_assert!(cascade_datoms.len() >= 5,
                "Expected ≥5 cascade datoms, got {}", cascade_datoms.len());
            // CascadeReceipt must also reflect all 5 steps
            prop_assert_eq!(cascade_receipt.cascade_datoms.len(), cascade_datoms.len());
        }
    }

    // INV-MERGE-008: Idempotent delivery — re-merging same store is no-op
    fn inv_merge_008(s1 in arb_store(3), s2 in arb_store(3)) {
        let mut once = s1.clone();
        let _r1 = merge(&mut once, &s2);
        let mut twice = once.clone();
        let (r2, _) = merge(&mut twice, &s2);
        prop_assert_eq!(once.datoms().collect::<BTreeSet<_>>(),
                        twice.datoms().collect::<BTreeSet<_>>());
        prop_assert_eq!(r2.new_datoms, 0);  // No new datoms on re-merge
    }

    // INV-MERGE-009: Receipt completeness — receipt matches actual store delta
    fn inv_merge_009(s1 in arb_store(5), s2 in arb_store(5)) {
        let pre_len = s1.len();
        let mut target = s1.clone();
        let (receipt, _cascade) = merge(&mut target, &s2);
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

## §7.6 Cascade Step Reference

The merge cascade is the sequence of deterministic post-merge operations that maintain
derived-state consistency. All 5 steps are mandatory per INV-MERGE-002; at Stage 0, only
step 1 is fully implemented (steps 2-5 produce stub datoms per ADR-MERGE-007).

### Step 1: Conflict Detection (Stage 0 — FULL)

For each new datom `d = [e, a, v, tx, Assert]` entering from the merge:
- If `cardinality(a) = :one` and an existing datom `d' = [e, a, v', tx', Assert]` exists
  where `v != v'` and `d` and `d'` are causally independent, assert a Conflict entity.
- Conflict entity is content-addressable from `(e, a, v, v')` — same conflict always
  produces the same entity ID (INV-MERGE-010 determinism).
- Conflicts feed into the RESOLUTION namespace (guide/04-resolution.md) for routing.

### Step 2: Cache Invalidation (Stage 0 — STUB)

Mark cached query results as stale for entities affected by new datoms.
- At Stage 0, there is no query cache layer — queries are direct store reads.
- Stub datom: `:cascade/cache-invalidation` with count=0.
- Activates at Stage 1 when query result caching is implemented.

### Step 3: Projection Staleness (Stage 0 — STUB)

Mark existing seed/query projections touching affected entities for refresh.
- At Stage 0, projections are not yet implemented.
- Stub datom: `:cascade/projection-staleness` with count=0.
- Activates at Stage 1+ when the projection management system is built.
- When activated, step 3 must activate simultaneously with step 5 (projection refresh
  depends on knowing which projections exist).

### Step 4: Uncertainty Update (Stage 0 — STUB)

Recompute `sigma(e)` (uncertainty tensor) for entities that received new assertions or
have newly detected conflicts.
- At Stage 0, the uncertainty tensor is not used for delegation (ADR-RESOLUTION-006)
  or budget allocation (BUDGET, Stage 1), so stale sigma values have no effect.
- Stub datom: `:cascade/uncertainty-update` with count=0.
- Activates at Stage 1 when the BUDGET namespace is implemented.

### Step 5: Subscription Notification (Stage 0 — STUB)

Notify subscribers whose query patterns match the new datoms introduced by the merge.
- At Stage 0, there is no subscription system — agents poll rather than subscribe.
- Stub datom: `:cascade/subscription-notification` with count=0.
- Activates at Stage 3 when multi-agent coordination requires push notifications.

### Progressive Activation Schedule

| Stage | Step 1 (Conflict) | Step 2 (Cache) | Step 3 (Projection) | Step 4 (Uncertainty) | Step 5 (Subscription) |
|-------|-------------------|----------------|---------------------|----------------------|-----------------------|
| 0 | Full | Stub | Stub | Stub | Stub |
| 1 | Full | Full | Full | Full | Stub |
| 2+ | Full | Full | Full | Full | Full |

---

## §7.7 Stage 2 Extension Points

When Stage 2 adds branching:
- `merge()` gains a `MergeStrategy` parameter (currently: always `SetUnion`).
- Branch merges use the same underlying set union but track branch ancestry.
- W_α working sets use branch-scoped views.
- The `MergeReceipt` gains `conflicts: Vec<ConflictSet>` for branch-level conflict reporting.

No premature implementation needed. The `merge()` function signature supports extension
by adding parameters with defaults.

---

## §7.8 Implementation Checklist

- [ ] `merge()` function implements set union
- [ ] `MergeReceipt` records statistics correctly (INV-MERGE-009)
- [ ] Content-identity deduplication works (BTreeSet)
- [ ] Frontier merged (pointwise max per agent)
- [ ] Indexes updated after merge
- [ ] No datom loss verified (proptest + Kani)
- [ ] Commutativity/associativity/idempotency hold (from STORE tests)
- [ ] `run_cascade()` takes only `&Store` + `&[Datom]` — no AgentId, no clock, no RNG (INV-MERGE-010)
- [ ] Cascade step 1 (conflict detection) fully implemented
- [ ] Cascade steps 2-5 produce stub datoms with `:cascade/*` attributes (ADR-MERGE-007)
- [ ] Cascade datom identity is content-addressable from conflict/change content (INV-MERGE-010)
- [ ] Cascade determinism proptest passes: merge(A,B) and merge(B,A) produce identical cascade datoms
- [ ] All 5 cascade steps produce at least one datom when new datoms exist (INV-MERGE-002)
- [ ] Integration: two independent stores merge cleanly

---
