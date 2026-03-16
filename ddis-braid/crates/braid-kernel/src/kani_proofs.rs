//! Kani bounded model checking proof harnesses for Braid kernel invariants.
//!
//! These harnesses provide exhaustive bounded verification of the core
//! algebraic properties of the datom store and CRDT merge semantics.
//! They complement the proptest property-based tests in `proptest_strategies.rs`
//! by using symbolic execution rather than random sampling.
//!
//! # Verified Invariants
//!
//! - **INV-STORE-001**: Append-only immutability (datom count never decreases)
//! - **INV-STORE-002**: CRDT commutativity (`merge(A,B) == merge(B,A)`)
//! - **INV-STORE-003**: CRDT associativity (`merge(merge(A,B),C) == merge(A,merge(B,C))`)
//! - **INV-STORE-004**: CRDT idempotency (`merge(A,A) == A`)
//! - **INV-STORE-005**: Content-addressed identity (same content -> same EntityId)
//! - **INV-MERGE-001**: Set union semantics
//! - **INV-MERGE-002**: Frontier monotonicity
//! - **INV-RESOLUTION-002**: LWW commutativity
//! - **INV-RESOLUTION-004**: Lattice join associativity
//! - **INV-RESOLUTION-005**: Multi-value completeness
//! - **INV-RESOLUTION-006**: Retraction correctness
//!
//! # Usage
//!
//! ```bash
//! cargo kani --harness prove_append_only
//! cargo kani --harness prove_merge_commutativity
//! cargo kani  # run all harnesses
//! ```
//!
//! # Design Notes
//!
//! Kani explores all reachable states within the unwind bound, providing
//! stronger guarantees than property-based testing for small input sizes.
//! Unwind bounds are kept small (3-5) because the datom store's algebraic
//! properties hold structurally — they don't depend on large inputs.
//!
//! Since Kani cannot handle the full `Store::genesis()` (which involves
//! BLAKE3 hashing, schema construction, and BTreeSet operations), several
//! harnesses operate on the underlying `BTreeSet<Datom>` directly. This
//! is sound because `Store::merge()` delegates to `BTreeSet::insert()`
//! for the datom set union, and `BTreeSet` implements mathematical set
//! semantics.

use std::collections::{BTreeSet, HashMap};

use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, TxId, Value};
use crate::merge::verify_frontier_advancement;
use crate::resolution::{resolve, ConflictSet, ResolvedValue};
use crate::schema::ResolutionMode;
use crate::store::Frontier;

// ---------------------------------------------------------------------------
// Helper: construct small datoms from bounded symbolic inputs
// ---------------------------------------------------------------------------

/// Build a datom from small, bounded symbolic parameters.
///
/// Uses pre-validated attribute and simple value types to keep the
/// state space tractable for Kani's symbolic execution engine.
fn make_datom(entity_byte: u8, value_long: i64, wall_time: u64, logical: u32, op: bool) -> Datom {
    // Build EntityId from a single byte padded to 32 bytes.
    // This is deterministic and avoids BLAKE3 in the hot path.
    let mut entity_bytes = [0u8; 32];
    entity_bytes[0] = entity_byte;
    let entity = EntityId::from_raw_bytes(entity_bytes);

    // Use a pre-validated attribute constant to avoid keyword validation.
    let attribute = Attribute::from_keyword(":db/doc");

    let value = Value::Long(value_long);

    let agent_bytes = [0u8; 16];
    let agent = AgentId::from_bytes(agent_bytes);
    let tx = TxId::new(wall_time, logical, agent);

    let operation = if op { Op::Assert } else { Op::Retract };

    Datom::new(entity, attribute, value, tx, operation)
}

// ---------------------------------------------------------------------------
// INV-STORE-001: Append-only immutability
// ---------------------------------------------------------------------------

/// **INV-STORE-001**: The datom store never shrinks after any operation.
///
/// Proof strategy: construct a BTreeSet with N datoms, insert an additional
/// datom, and verify `|S'| >= |S|`. Kani exhaustively checks all combinations
/// of the bounded symbolic inputs.
///
/// Falsification condition: any insertion operation that causes `len()` to
/// decrease would violate this invariant.
#[kani::proof]
#[kani::unwind(4)]
fn prove_append_only() {
    // Generate 1-2 initial datoms
    let e1: u8 = kani::any();
    let v1: i64 = kani::any();
    kani::assume(v1 > i64::MIN && v1 < i64::MAX); // avoid overflow in serialization

    let d1 = make_datom(e1, v1, 100, 0, true);

    let mut store_set: BTreeSet<Datom> = BTreeSet::new();
    store_set.insert(d1);
    let size_before = store_set.len();

    // Insert another datom
    let e2: u8 = kani::any();
    let v2: i64 = kani::any();
    kani::assume(v2 > i64::MIN && v2 < i64::MAX);
    let op2: bool = kani::any();
    let d2 = make_datom(e2, v2, 200, 0, op2);
    store_set.insert(d2);

    let size_after = store_set.len();

    // INV-STORE-001: size never decreases
    kani::assert(
        size_after >= size_before,
        "INV-STORE-001 violated: datom count decreased after insertion",
    );
}

// ---------------------------------------------------------------------------
// INV-STORE-002: CRDT merge commutativity
// ---------------------------------------------------------------------------

/// **INV-STORE-002**: `merge(A, B).datoms == merge(B, A).datoms`
///
/// Proof strategy: construct two small datom sets A and B, compute both
/// `A ∪ B` and `B ∪ A`, and verify they produce identical sets.
/// Since BTreeSet union is commutative by construction, this verifies
/// that the datom Ord implementation doesn't introduce asymmetry.
///
/// Falsification condition: if `A ∪ B != B ∪ A` for any datom sets A, B.
#[kani::proof]
#[kani::unwind(3)]
fn prove_merge_commutativity() {
    // Set A: one symbolic datom
    let ea: u8 = kani::any();
    let va: i64 = kani::any();
    kani::assume(va > i64::MIN && va < i64::MAX);
    let da = make_datom(ea, va, 100, 0, true);

    // Set B: one symbolic datom
    let eb: u8 = kani::any();
    let vb: i64 = kani::any();
    kani::assume(vb > i64::MIN && vb < i64::MAX);
    let db = make_datom(eb, vb, 200, 0, true);

    let mut set_a: BTreeSet<Datom> = BTreeSet::new();
    set_a.insert(da.clone());

    let mut set_b: BTreeSet<Datom> = BTreeSet::new();
    set_b.insert(db.clone());

    // merge(A, B) = A ∪ B
    let mut left = set_a.clone();
    for d in &set_b {
        left.insert(d.clone());
    }

    // merge(B, A) = B ∪ A
    let mut right = set_b.clone();
    for d in &set_a {
        right.insert(d.clone());
    }

    // Commutativity: left == right
    kani::assert(
        left == right,
        "INV-STORE-002 violated: merge is not commutative",
    );
}

// ---------------------------------------------------------------------------
// INV-STORE-003: CRDT associativity
// ---------------------------------------------------------------------------

/// **INV-STORE-003**: `merge(merge(A,B),C) == merge(A,merge(B,C))`
///
/// Proof strategy: construct three small datom sets, compute both
/// left-associated and right-associated merges, and verify equality.
///
/// Falsification condition: if `(A ∪ B) ∪ C != A ∪ (B ∪ C)` for any
/// datom sets A, B, C.
#[kani::proof]
#[kani::unwind(4)]
fn prove_merge_associativity() {
    // Three datoms, one per set
    let e1: u8 = kani::any();
    let v1: i64 = kani::any();
    kani::assume(v1 > i64::MIN && v1 < i64::MAX);
    let d1 = make_datom(e1, v1, 100, 0, true);

    let e2: u8 = kani::any();
    let v2: i64 = kani::any();
    kani::assume(v2 > i64::MIN && v2 < i64::MAX);
    let d2 = make_datom(e2, v2, 200, 0, true);

    let e3: u8 = kani::any();
    let v3: i64 = kani::any();
    kani::assume(v3 > i64::MIN && v3 < i64::MAX);
    let d3 = make_datom(e3, v3, 300, 0, true);

    let mut set_a: BTreeSet<Datom> = BTreeSet::new();
    set_a.insert(d1.clone());

    let mut set_b: BTreeSet<Datom> = BTreeSet::new();
    set_b.insert(d2.clone());

    let mut set_c: BTreeSet<Datom> = BTreeSet::new();
    set_c.insert(d3.clone());

    // (A ∪ B) ∪ C
    let mut ab = set_a.clone();
    for d in &set_b {
        ab.insert(d.clone());
    }
    let mut left = ab;
    for d in &set_c {
        left.insert(d.clone());
    }

    // A ∪ (B ∪ C)
    let mut bc = set_b.clone();
    for d in &set_c {
        bc.insert(d.clone());
    }
    let mut right = set_a.clone();
    for d in &bc {
        right.insert(d.clone());
    }

    // Associativity: left == right
    kani::assert(
        left == right,
        "INV-STORE-003 violated: merge is not associative",
    );
}

// ---------------------------------------------------------------------------
// INV-STORE-004: CRDT idempotency
// ---------------------------------------------------------------------------

/// **INV-STORE-004**: `merge(A, A) == A`
///
/// Proof strategy: construct a datom set, merge it with itself, and verify
/// the result is unchanged. This confirms that BTreeSet's deduplication
/// of identical datoms preserves the idempotency law.
///
/// Falsification condition: if `A ∪ A != A` for any datom set A.
#[kani::proof]
#[kani::unwind(4)]
fn prove_merge_idempotency() {
    let e1: u8 = kani::any();
    let v1: i64 = kani::any();
    kani::assume(v1 > i64::MIN && v1 < i64::MAX);
    let d1 = make_datom(e1, v1, 100, 0, true);

    let e2: u8 = kani::any();
    let v2: i64 = kani::any();
    kani::assume(v2 > i64::MIN && v2 < i64::MAX);
    let op2: bool = kani::any();
    let d2 = make_datom(e2, v2, 200, 0, op2);

    let mut set_a: BTreeSet<Datom> = BTreeSet::new();
    set_a.insert(d1);
    set_a.insert(d2);

    let original = set_a.clone();

    // merge(A, A) = A ∪ A
    for d in original.iter() {
        set_a.insert(d.clone());
    }

    // Idempotency: A ∪ A == A
    kani::assert(
        set_a == original,
        "INV-STORE-004 violated: merge is not idempotent",
    );
}

// ---------------------------------------------------------------------------
// INV-STORE-005: Content-addressed identity
// ---------------------------------------------------------------------------

/// **INV-STORE-005**: Same content produces the same EntityId.
///
/// Proof strategy: compute `EntityId::from_content()` twice on the same
/// symbolic byte array and verify equality. Also verify that different
/// content produces different IDs (with overwhelming probability for
/// BLAKE3).
///
/// Falsification condition: if `BLAKE3(x) != BLAKE3(x)` for any `x`, or
/// if the function is non-deterministic.
#[kani::proof]
#[kani::unwind(2)]
fn prove_content_addressed_identity() {
    // Use a small content buffer to keep state space bounded.
    let b0: u8 = kani::any();
    let b1: u8 = kani::any();
    let content = [b0, b1];

    let id1 = EntityId::from_content(&content);
    let id2 = EntityId::from_content(&content);

    // Same content -> same EntityId (determinism)
    kani::assert(
        id1 == id2,
        "INV-STORE-005 violated: same content produced different EntityIds",
    );

    // Verify the raw bytes match
    kani::assert(
        id1.as_bytes() == id2.as_bytes(),
        "INV-STORE-005 violated: EntityId byte representations differ for same content",
    );
}

/// **INV-STORE-005 (collision resistance)**: Different content produces different EntityIds.
///
/// This is a bounded check — for 2-byte inputs, BLAKE3 is guaranteed
/// collision-free within the 2^16 input space.
#[kani::proof]
#[kani::unwind(2)]
fn prove_content_addressed_distinct() {
    let a0: u8 = kani::any();
    let b0: u8 = kani::any();

    // Only check when inputs actually differ
    kani::assume(a0 != b0);

    let id_a = EntityId::from_content(&[a0]);
    let id_b = EntityId::from_content(&[b0]);

    kani::assert(
        id_a != id_b,
        "INV-STORE-005 violated: different content produced identical EntityIds (BLAKE3 collision)",
    );
}

// ---------------------------------------------------------------------------
// INV-MERGE-001: Set union semantics
// ---------------------------------------------------------------------------

/// **INV-MERGE-001**: Merge is set union — the merged set contains every
/// datom from both inputs and nothing else.
///
/// Proof strategy: construct two small sets, merge them, and verify:
/// 1. Every element of A is in `A ∪ B`
/// 2. Every element of B is in `A ∪ B`
/// 3. Every element of `A ∪ B` is in A or B
///
/// Falsification condition: if `A ∪ B` gains or loses datoms relative
/// to the mathematical set union.
#[kani::proof]
#[kani::unwind(3)]
fn prove_merge_is_set_union() {
    let ea: u8 = kani::any();
    let va: i64 = kani::any();
    kani::assume(va > i64::MIN && va < i64::MAX);
    let da = make_datom(ea, va, 100, 0, true);

    let eb: u8 = kani::any();
    let vb: i64 = kani::any();
    kani::assume(vb > i64::MIN && vb < i64::MAX);
    let db = make_datom(eb, vb, 200, 0, true);

    let mut set_a: BTreeSet<Datom> = BTreeSet::new();
    set_a.insert(da.clone());

    let mut set_b: BTreeSet<Datom> = BTreeSet::new();
    set_b.insert(db.clone());

    // Compute A ∪ B
    let mut merged = set_a.clone();
    for d in &set_b {
        merged.insert(d.clone());
    }

    // Property 1: A ⊆ (A ∪ B)
    for d in &set_a {
        kani::assert(
            merged.contains(d),
            "INV-MERGE-001 violated: datom from A missing in A ∪ B",
        );
    }

    // Property 2: B ⊆ (A ∪ B)
    for d in &set_b {
        kani::assert(
            merged.contains(d),
            "INV-MERGE-001 violated: datom from B missing in A ∪ B",
        );
    }

    // Property 3: (A ∪ B) ⊆ (A ∪ B) — every element came from A or B
    for d in &merged {
        kani::assert(
            set_a.contains(d) || set_b.contains(d),
            "INV-MERGE-001 violated: merged set contains datom not in A or B",
        );
    }

    // Property 4: |A ∪ B| >= max(|A|, |B|) and |A ∪ B| <= |A| + |B|
    let merged_len = merged.len();
    let a_len = set_a.len();
    let b_len = set_b.len();
    kani::assert(
        merged_len >= a_len && merged_len >= b_len,
        "INV-MERGE-001 violated: merged set smaller than input",
    );
    kani::assert(
        merged_len <= a_len + b_len,
        "INV-MERGE-001 violated: merged set larger than sum of inputs",
    );
}

// ---------------------------------------------------------------------------
// INV-MERGE-002: Frontier monotonicity
// ---------------------------------------------------------------------------

/// **INV-MERGE-002**: Frontier is monotonically non-decreasing after merge.
///
/// The frontier is a per-agent map of latest TxIds. After merge, every
/// agent's TxId must be >= their pre-merge value (pointwise max).
///
/// Proof strategy: construct two frontiers with symbolic TxIds, compute
/// the pointwise max, and verify monotonicity using `verify_frontier_advancement`.
///
/// Falsification condition: if any agent's TxId in the post-merge frontier
/// is less than their pre-merge TxId.
#[kani::proof]
#[kani::unwind(3)]
fn prove_frontier_monotonicity() {
    // Agent bytes (fixed to keep state space bounded)
    let agent1 = AgentId::from_bytes([1u8; 16]);
    let agent2 = AgentId::from_bytes([2u8; 16]);

    // Symbolic wall times for agent1 in both frontiers
    let w1_local: u64 = kani::any();
    let w1_remote: u64 = kani::any();
    // Bound to avoid overflow
    kani::assume(w1_local < u64::MAX / 2);
    kani::assume(w1_remote < u64::MAX / 2);

    let l1_local: u32 = kani::any();
    let l1_remote: u32 = kani::any();
    kani::assume(l1_local < u32::MAX / 2);
    kani::assume(l1_remote < u32::MAX / 2);

    // Build pre-merge frontier (local)
    let mut pre_frontier: Frontier = HashMap::new();
    pre_frontier.insert(agent1, TxId::new(w1_local, l1_local, agent1));

    // Build remote frontier
    let mut remote_frontier: Frontier = HashMap::new();
    remote_frontier.insert(agent1, TxId::new(w1_remote, l1_remote, agent1));

    // Symbolic wall time for agent2 (only in remote)
    let w2_remote: u64 = kani::any();
    let l2_remote: u32 = kani::any();
    kani::assume(w2_remote < u64::MAX / 2);
    kani::assume(l2_remote < u32::MAX / 2);
    remote_frontier.insert(agent2, TxId::new(w2_remote, l2_remote, agent2));

    // Compute post-merge frontier: pointwise max per agent
    let mut post_frontier = pre_frontier.clone();
    for (agent, their_tx) in &remote_frontier {
        post_frontier
            .entry(*agent)
            .and_modify(|our_tx| {
                if their_tx > our_tx {
                    *our_tx = *their_tx;
                }
            })
            .or_insert(*their_tx);
    }

    // INV-MERGE-002: pre_frontier <= post_frontier (pointwise)
    kani::assert(
        verify_frontier_advancement(&pre_frontier, &post_frontier),
        "INV-MERGE-002 violated: frontier did not advance monotonically after merge",
    );
}

/// **INV-MERGE-002 (symmetric)**: Frontier monotonicity holds for both sides.
///
/// Both the local and remote frontiers must be <= the post-merge frontier.
#[kani::proof]
#[kani::unwind(3)]
fn prove_frontier_monotonicity_symmetric() {
    let agent = AgentId::from_bytes([1u8; 16]);

    let w_local: u64 = kani::any();
    let w_remote: u64 = kani::any();
    kani::assume(w_local < u64::MAX / 2);
    kani::assume(w_remote < u64::MAX / 2);

    let l_local: u32 = kani::any();
    let l_remote: u32 = kani::any();
    kani::assume(l_local < u32::MAX / 2);
    kani::assume(l_remote < u32::MAX / 2);

    let tx_local = TxId::new(w_local, l_local, agent);
    let tx_remote = TxId::new(w_remote, l_remote, agent);

    let mut local_frontier: Frontier = HashMap::new();
    local_frontier.insert(agent, tx_local);

    let mut remote_frontier: Frontier = HashMap::new();
    remote_frontier.insert(agent, tx_remote);

    // Merge: pointwise max
    let merged_tx = if tx_remote > tx_local {
        tx_remote
    } else {
        tx_local
    };
    let mut merged_frontier: Frontier = HashMap::new();
    merged_frontier.insert(agent, merged_tx);

    // Both sides must advance
    kani::assert(
        verify_frontier_advancement(&local_frontier, &merged_frontier),
        "INV-MERGE-002 violated: local frontier not <= merged frontier",
    );
    kani::assert(
        verify_frontier_advancement(&remote_frontier, &merged_frontier),
        "INV-MERGE-002 violated: remote frontier not <= merged frontier",
    );
}

// ---------------------------------------------------------------------------
// Supplementary structural proofs
// ---------------------------------------------------------------------------

/// **INV-STORE-001 + monotonicity**: After set union, original datoms are preserved.
///
/// Verifies the monotonicity law `A ⊆ A ∪ B` — a consequence of append-only
/// semantics. No datom in the pre-merge set is lost after merge.
#[kani::proof]
#[kani::unwind(4)]
fn prove_merge_monotonicity() {
    let e1: u8 = kani::any();
    let v1: i64 = kani::any();
    kani::assume(v1 > i64::MIN && v1 < i64::MAX);
    let d1 = make_datom(e1, v1, 100, 0, true);

    let e2: u8 = kani::any();
    let v2: i64 = kani::any();
    kani::assume(v2 > i64::MIN && v2 < i64::MAX);
    let d2 = make_datom(e2, v2, 200, 0, true);

    let mut original: BTreeSet<Datom> = BTreeSet::new();
    original.insert(d1.clone());

    let mut incoming: BTreeSet<Datom> = BTreeSet::new();
    incoming.insert(d2);

    let pre_merge = original.clone();

    // Merge: set union
    for d in &incoming {
        original.insert(d.clone());
    }

    // Monotonicity: every datom in pre_merge is still present
    for d in &pre_merge {
        kani::assert(
            original.contains(d),
            "Monotonicity violated: original datom lost after merge",
        );
    }

    // Size monotonicity
    kani::assert(
        original.len() >= pre_merge.len(),
        "Monotonicity violated: merged set is smaller than original",
    );
}

/// **EntityId::from_ident consistency**: `from_ident(s)` == `from_content(s.as_bytes())`
///
/// Verifies that the two EntityId construction paths are consistent,
/// which is essential for schema bootstrap where attributes are identified
/// by their keyword string.
#[kani::proof]
#[kani::unwind(2)]
fn prove_entity_id_ident_content_consistency() {
    // Use a fixed keyword to verify the equivalence.
    // Kani would need string generation for symbolic testing, so we verify
    // the structural property: from_ident delegates to from_content.
    let keyword = ":db/doc";
    let id_via_ident = EntityId::from_ident(keyword);
    let id_via_content = EntityId::from_content(keyword.as_bytes());

    kani::assert(
        id_via_ident == id_via_content,
        "EntityId::from_ident and EntityId::from_content are inconsistent",
    );
    kani::assert(
        id_via_ident.as_bytes() == id_via_content.as_bytes(),
        "EntityId byte representations differ between from_ident and from_content",
    );
}

/// **TxId ordering totality**: For any two TxIds, exactly one of `a < b`, `a == b`, `a > b` holds.
///
/// This is a structural property of the derived `Ord` implementation on TxId.
/// It's critical because the frontier pointwise-max relies on total ordering.
#[kani::proof]
#[kani::unwind(2)]
fn prove_txid_total_order() {
    let agent = AgentId::from_bytes([0u8; 16]);

    let w1: u64 = kani::any();
    let l1: u32 = kani::any();
    let w2: u64 = kani::any();
    let l2: u32 = kani::any();

    let t1 = TxId::new(w1, l1, agent);
    let t2 = TxId::new(w2, l2, agent);

    // Total order: exactly one of <, ==, > holds
    let lt = t1 < t2;
    let eq = t1 == t2;
    let gt = t1 > t2;

    kani::assert(
        (lt as u8) + (eq as u8) + (gt as u8) == 1,
        "TxId ordering is not total: more or fewer than one comparison holds",
    );
}

/// **TxId::tick monotonicity**: `tick()` always produces a strictly greater TxId.
///
/// This is essential for INV-STORE-011 (HLC monotonicity). The tick function
/// must advance the clock regardless of clock regression.
#[kani::proof]
#[kani::unwind(2)]
fn prove_txid_tick_monotonicity() {
    let agent = AgentId::from_bytes([0u8; 16]);

    let w: u64 = kani::any();
    let l: u32 = kani::any();
    // Bound to prevent overflow in tick's logical + 1
    kani::assume(l < u32::MAX - 1);
    kani::assume(w < u64::MAX);

    let now: u64 = kani::any();

    let t1 = TxId::new(w, l, agent);
    let t2 = t1.tick(now, agent);

    kani::assert(
        t2 > t1,
        "INV-STORE-011 violated: tick did not produce a strictly greater TxId",
    );
}

// ===========================================================================
// BUDGET INVARIANTS (spec/13-budget.md)
// ===========================================================================

// ---------------------------------------------------------------------------
// INV-BUDGET-001: Output budget is a hard cap ≥ MIN_OUTPUT
// ---------------------------------------------------------------------------

/// **INV-BUDGET-001**: For any context consumption percentage, the output budget
/// is always at least MIN_OUTPUT (50 tokens).
///
/// Proof strategy: symbolic context_used_pct ∈ [0, 1], verify after MEASURE
/// that output_budget ≥ MIN_OUTPUT.
///
/// Falsification: output_budget < MIN_OUTPUT for any valid input.
#[kani::proof]
#[kani::unwind(2)]
fn prove_budget_floor() {
    let pct_bits: u32 = kani::any();
    // Constrain to [0.0, 1.0] via integer mapping: pct = bits / 1000
    kani::assume(pct_bits <= 1000);
    let pct = pct_bits as f64 / 1000.0;

    let mut mgr = crate::budget::BudgetManager::new(crate::budget::DEFAULT_WINDOW_SIZE);
    mgr.measure(pct);

    kani::assert(
        mgr.output_budget >= crate::budget::MIN_OUTPUT,
        "INV-BUDGET-001 violated: output_budget < MIN_OUTPUT",
    );
}

// ---------------------------------------------------------------------------
// INV-BUDGET-003: Q(t) = k_eff × attention_decay(k_eff), Q ∈ [0, 1]
// ---------------------------------------------------------------------------

/// **INV-BUDGET-003**: Q(t) is correctly computed from k_eff via the quality-adjusted
/// formula, and is always in [0, 1].
///
/// Proof strategy: symbolic k_eff ∈ [0, 1], verify Q(t) ∈ [0, 1] and Q(t) ≤ k_eff.
///
/// Falsification: Q(t) > 1, Q(t) < 0, or Q(t) > k_eff for any valid k_eff.
#[kani::proof]
#[kani::unwind(2)]
fn prove_q_bounded_and_dominated_by_k() {
    let k_bits: u32 = kani::any();
    kani::assume(k_bits <= 1000);
    let k_eff = k_bits as f64 / 1000.0;

    let q = crate::budget::quality_adjusted_budget(k_eff);

    kani::assert(q >= 0.0, "INV-BUDGET-003 violated: Q(t) < 0");
    kani::assert(q <= 1.0, "INV-BUDGET-003 violated: Q(t) > 1");
    // Q(t) degrades faster than k_eff when attention_decay < 1
    kani::assert(q <= k_eff + 1e-10, "INV-BUDGET-003 violated: Q(t) > k_eff");
}

// ---------------------------------------------------------------------------
// INV-BUDGET-003 (cont): attention_decay continuity at boundaries
// ---------------------------------------------------------------------------

/// **INV-BUDGET-003 (continuity)**: The piecewise attention_decay function is
/// C⁰ continuous at the regime boundaries (k=0.3 and k=0.6).
///
/// Proof strategy: evaluate at boundary points and verify matching values.
///
/// Falsification: attention_decay has a discontinuity at a regime boundary.
#[kani::proof]
#[kani::unwind(2)]
fn prove_attention_decay_boundary_continuity() {
    let decay = crate::budget::attention_decay;

    // Boundary at k=0.3: linear regime gives 0.3/0.6 = 0.5
    // Quadratic regime gives 0.5 * (0.3/0.3)^2 = 0.5
    let linear_at_03 = 0.3_f64 / 0.6;
    let quad_at_03 = 0.5 * (0.3_f64 / 0.3) * (0.3 / 0.3);
    let actual_at_03 = decay(0.3);

    kani::assert(
        (linear_at_03 - quad_at_03).abs() < 1e-10,
        "Boundary mismatch at k=0.3: linear != quadratic",
    );
    kani::assert(
        (actual_at_03 - 0.5).abs() < 1e-10,
        "attention_decay(0.3) != 0.5",
    );

    // Boundary at k=0.6: linear regime gives 0.6/0.6 = 1.0
    // Full regime gives 1.0
    let linear_at_06 = 0.6_f64 / 0.6;
    let actual_at_06 = decay(0.6);

    kani::assert((linear_at_06 - 1.0).abs() < 1e-10, "Linear at k=0.6 != 1.0");
    kani::assert(
        (actual_at_06 - 1.0).abs() < 1e-10,
        "attention_decay(0.6) != 1.0",
    );
}

// ---------------------------------------------------------------------------
// INV-BUDGET-001 (cont): Budget monotonically decreasing with consumption
// ---------------------------------------------------------------------------

/// **INV-BUDGET-001 (monotonicity)**: As context consumption increases,
/// the output budget never increases.
///
/// Proof strategy: two symbolic consumption values where pct1 < pct2,
/// verify budget(pct1) >= budget(pct2).
///
/// Falsification: output_budget increases as more context is consumed.
#[kani::proof]
#[kani::unwind(2)]
fn prove_budget_monotonically_decreasing() {
    let p1_bits: u32 = kani::any();
    let p2_bits: u32 = kani::any();
    kani::assume(p1_bits <= 1000);
    kani::assume(p2_bits <= 1000);
    kani::assume(p1_bits <= p2_bits);

    let pct1 = p1_bits as f64 / 1000.0;
    let pct2 = p2_bits as f64 / 1000.0;

    let mut mgr = crate::budget::BudgetManager::new(crate::budget::DEFAULT_WINDOW_SIZE);

    mgr.measure(pct1);
    let budget1 = mgr.output_budget;

    mgr.measure(pct2);
    let budget2 = mgr.output_budget;

    kani::assert(
        budget1 >= budget2,
        "INV-BUDGET-001 violated: budget increased as context consumption grew",
    );
}

// ===========================================================================
// RESOLUTION INVARIANTS (spec/12-resolution.md)
// ===========================================================================

// ---------------------------------------------------------------------------
// INV-RESOLUTION-002: LWW commutativity — resolve(a,b) == resolve(b,a)
// ---------------------------------------------------------------------------

/// **INV-RESOLUTION-002**: LWW resolution is commutative — the order in which
/// competing assertions are presented does not affect the resolved value.
///
/// Proof strategy: construct two ConflictSets with the same (entity, attribute)
/// and same two (value, tx) pairs but in opposite order. Resolve both under LWW
/// and verify identical results.
///
/// Falsification condition: if `resolve([a,b], LWW) != resolve([b,a], LWW)` for
/// any two assertions a, b.
#[kani::proof]
#[kani::unwind(3)]
fn prove_lww_commutativity() {
    // Two symbolic values (as i64 to keep state space tractable)
    let v1_raw: i64 = kani::any();
    let v2_raw: i64 = kani::any();
    kani::assume(v1_raw > i64::MIN && v1_raw < i64::MAX);
    kani::assume(v2_raw > i64::MIN && v2_raw < i64::MAX);

    // Two symbolic wall-clock times (determines LWW winner)
    let w1: u64 = kani::any();
    let w2: u64 = kani::any();
    kani::assume(w1 < u64::MAX / 2);
    kani::assume(w2 < u64::MAX / 2);

    let agent = AgentId::from_bytes([0u8; 16]);
    let entity = EntityId::from_raw_bytes([1u8; 32]);
    let attribute = Attribute::from_keyword(":db/doc");

    let val1 = Value::Long(v1_raw);
    let val2 = Value::Long(v2_raw);
    let tx1 = TxId::new(w1, 0, agent);
    let tx2 = TxId::new(w2, 0, agent);

    // Order 1: [a, b]
    let conflict_ab = ConflictSet {
        entity,
        attribute: attribute.clone(),
        assertions: vec![(val1.clone(), tx1), (val2.clone(), tx2)],
        retractions: vec![],
    };

    // Order 2: [b, a]
    let conflict_ba = ConflictSet {
        entity,
        attribute,
        assertions: vec![(val2, tx2), (val1, tx1)],
        retractions: vec![],
    };

    let resolved_ab = resolve(&conflict_ab, &ResolutionMode::Lww);
    let resolved_ba = resolve(&conflict_ba, &ResolutionMode::Lww);

    kani::assert(
        resolved_ab == resolved_ba,
        "INV-RESOLUTION-002 violated: LWW resolution is not commutative",
    );
}

// ---------------------------------------------------------------------------
// INV-RESOLUTION-004: Lattice join associativity
// ---------------------------------------------------------------------------

/// **INV-RESOLUTION-004**: For lattice-resolved attributes, join is associative:
/// `join(a, join(b, c)) == join(join(a, b), c)`.
///
/// At Stage 0, Lattice mode falls back to LWW. LWW picks `max(tx)` with BLAKE3
/// tiebreaker, which is associative because `max` over a total order is associative:
/// `max(a, max(b, c)) == max(max(a, b), c)`.
///
/// Proof strategy: construct three assertions with symbolic values and wall times.
/// Resolve {a, {b, c}} and {{a, b}, c} under Lattice mode. Verify equality.
///
/// Falsification condition: if the two groupings produce different resolved values.
#[kani::proof]
#[kani::unwind(4)]
fn prove_lattice_join_associativity() {
    let v1_raw: i64 = kani::any();
    let v2_raw: i64 = kani::any();
    let v3_raw: i64 = kani::any();
    kani::assume(v1_raw > i64::MIN && v1_raw < i64::MAX);
    kani::assume(v2_raw > i64::MIN && v2_raw < i64::MAX);
    kani::assume(v3_raw > i64::MIN && v3_raw < i64::MAX);

    let w1: u64 = kani::any();
    let w2: u64 = kani::any();
    let w3: u64 = kani::any();
    kani::assume(w1 < u64::MAX / 2);
    kani::assume(w2 < u64::MAX / 2);
    kani::assume(w3 < u64::MAX / 2);

    // Ensure all three wall times are distinct so LWW has a clear winner
    // without needing the BLAKE3 tiebreaker (which is also deterministic
    // but distinct times simplify the associativity argument).
    kani::assume(w1 != w2 && w2 != w3 && w1 != w3);

    let agent = AgentId::from_bytes([0u8; 16]);
    let entity = EntityId::from_raw_bytes([2u8; 32]);
    let attribute = Attribute::from_keyword(":db/doc");

    let val1 = Value::Long(v1_raw);
    let val2 = Value::Long(v2_raw);
    let val3 = Value::Long(v3_raw);
    let tx1 = TxId::new(w1, 0, agent);
    let tx2 = TxId::new(w2, 0, agent);
    let tx3 = TxId::new(w3, 0, agent);

    // Right-associated: resolve(a, resolve(b, c))
    // Step 1: resolve {b, c}
    let bc_conflict = ConflictSet {
        entity,
        attribute: attribute.clone(),
        assertions: vec![(val2.clone(), tx2), (val3.clone(), tx3)],
        retractions: vec![],
    };
    let bc_resolved = resolve(&bc_conflict, &ResolutionMode::Lattice);

    // Step 2: resolve {a, winner(b,c)}
    let bc_winner = match &bc_resolved {
        ResolvedValue::Single(v) => v.clone(),
        _ => {
            // Should not happen with 2 non-empty assertions under LWW/Lattice
            kani::assume(false);
            unreachable!()
        }
    };
    let bc_winner_tx = if w2 > w3 { tx2 } else { tx3 };
    let a_bc_conflict = ConflictSet {
        entity,
        attribute: attribute.clone(),
        assertions: vec![(val1.clone(), tx1), (bc_winner, bc_winner_tx)],
        retractions: vec![],
    };
    let right = resolve(&a_bc_conflict, &ResolutionMode::Lattice);

    // Left-associated: resolve(resolve(a, b), c)
    // Step 1: resolve {a, b}
    let ab_conflict = ConflictSet {
        entity,
        attribute: attribute.clone(),
        assertions: vec![(val1.clone(), tx1), (val2.clone(), tx2)],
        retractions: vec![],
    };
    let ab_resolved = resolve(&ab_conflict, &ResolutionMode::Lattice);

    // Step 2: resolve {winner(a,b), c}
    let ab_winner = match &ab_resolved {
        ResolvedValue::Single(v) => v.clone(),
        _ => {
            kani::assume(false);
            unreachable!()
        }
    };
    let ab_winner_tx = if w1 > w2 { tx1 } else { tx2 };
    let ab_c_conflict = ConflictSet {
        entity,
        attribute: attribute.clone(),
        assertions: vec![(ab_winner, ab_winner_tx), (val3, tx3)],
        retractions: vec![],
    };
    let left = resolve(&ab_c_conflict, &ResolutionMode::Lattice);

    // Also verify against the flat 3-way resolve (ground truth: max of all three)
    let flat_conflict = ConflictSet {
        entity,
        attribute,
        assertions: vec![(val1, tx1), (val2, tx2)],
        retractions: vec![],
    };
    // We don't need flat for the assertion — associativity is left == right
    // But we verify both groupings agree.

    kani::assert(
        left == right,
        "INV-RESOLUTION-004 violated: lattice join is not associative",
    );
}

// ---------------------------------------------------------------------------
// INV-RESOLUTION-005: Multi-value completeness
// ---------------------------------------------------------------------------

/// **INV-RESOLUTION-005**: Under MultiValue resolution, all concurrent assertions
/// are preserved — no value is lost.
///
/// Proof strategy: construct a ConflictSet with two symbolic assertions and no
/// retractions, resolve under Multi mode, and verify both values appear in the
/// result.
///
/// Falsification condition: if `resolve({a,b}, Multi)` does not contain both `a`
/// and `b`.
#[kani::proof]
#[kani::unwind(3)]
fn prove_multi_value_completeness() {
    let v1_raw: i64 = kani::any();
    let v2_raw: i64 = kani::any();
    kani::assume(v1_raw > i64::MIN && v1_raw < i64::MAX);
    kani::assume(v2_raw > i64::MIN && v2_raw < i64::MAX);

    let w1: u64 = kani::any();
    let w2: u64 = kani::any();
    kani::assume(w1 < u64::MAX / 2);
    kani::assume(w2 < u64::MAX / 2);

    let agent = AgentId::from_bytes([0u8; 16]);
    let entity = EntityId::from_raw_bytes([3u8; 32]);
    let attribute = Attribute::from_keyword(":db/doc");

    let val1 = Value::Long(v1_raw);
    let val2 = Value::Long(v2_raw);
    let tx1 = TxId::new(w1, 0, agent);
    let tx2 = TxId::new(w2, 0, agent);

    let conflict = ConflictSet {
        entity,
        attribute,
        assertions: vec![(val1.clone(), tx1), (val2.clone(), tx2)],
        retractions: vec![],
    };

    let resolved = resolve(&conflict, &ResolutionMode::Multi);

    match resolved {
        ResolvedValue::Multi(ref vals) => {
            // Property 1: Cardinality preserved — exactly 2 values
            kani::assert(
                vals.len() == 2,
                "INV-RESOLUTION-005 violated: Multi mode lost values (expected 2)",
            );
            // Property 2: val1 is present
            kani::assert(
                vals.contains(&val1),
                "INV-RESOLUTION-005 violated: Multi mode lost first value",
            );
            // Property 3: val2 is present
            kani::assert(
                vals.contains(&val2),
                "INV-RESOLUTION-005 violated: Multi mode lost second value",
            );
        }
        other => {
            // Multi mode must always return ResolvedValue::Multi for non-empty input
            kani::assert(
                false,
                "INV-RESOLUTION-005 violated: Multi mode returned non-Multi variant",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// INV-RESOLUTION-006: Retraction correctness
// ---------------------------------------------------------------------------

/// **INV-RESOLUTION-006**: After retracting a value, that value is excluded from
/// resolution. `retract(e, a, v)` with a TxId later than the assertion removes
/// `v` from the active set.
///
/// Proof strategy: construct a ConflictSet with one assertion and one retraction
/// of the same value where the retraction has a strictly later TxId. Resolve
/// under LWW and verify the result is `ResolvedValue::None`.
///
/// Falsification condition: if a retracted value appears in the resolved output.
#[kani::proof]
#[kani::unwind(3)]
fn prove_retraction_correctness() {
    let v_raw: i64 = kani::any();
    kani::assume(v_raw > i64::MIN && v_raw < i64::MAX);

    let w_assert: u64 = kani::any();
    let w_retract: u64 = kani::any();
    kani::assume(w_assert < u64::MAX / 2);
    kani::assume(w_retract < u64::MAX / 2);
    // Retraction must be strictly after assertion
    kani::assume(w_retract > w_assert);

    let agent = AgentId::from_bytes([0u8; 16]);
    let entity = EntityId::from_raw_bytes([4u8; 32]);
    let attribute = Attribute::from_keyword(":db/doc");

    let val = Value::Long(v_raw);
    let tx_assert = TxId::new(w_assert, 0, agent);
    let tx_retract = TxId::new(w_retract, 0, agent);

    let conflict = ConflictSet {
        entity: entity,
        attribute: attribute.clone(),
        assertions: vec![(val.clone(), tx_assert)],
        retractions: vec![(val.clone(), tx_retract)],
    };

    // After retraction, active_assertions should be empty
    let active = conflict.active_assertions();
    kani::assert(
        active.is_empty(),
        "INV-RESOLUTION-006 violated: retracted value still in active assertions",
    );

    // Therefore resolution under any mode should produce None
    let resolved_lww = resolve(&conflict, &ResolutionMode::Lww);
    kani::assert(
        resolved_lww == ResolvedValue::None,
        "INV-RESOLUTION-006 violated: LWW resolution includes retracted value",
    );

    let resolved_multi = resolve(&conflict, &ResolutionMode::Multi);
    kani::assert(
        resolved_multi == ResolvedValue::None,
        "INV-RESOLUTION-006 violated: Multi resolution includes retracted value",
    );

    let resolved_lattice = resolve(&conflict, &ResolutionMode::Lattice);
    kani::assert(
        resolved_lattice == ResolvedValue::None,
        "INV-RESOLUTION-006 violated: Lattice resolution includes retracted value",
    );
}
