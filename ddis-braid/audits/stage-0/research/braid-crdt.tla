---------------------------- MODULE BraidCRDT ----------------------------
(*
 * TLA+ Specification: Braid CRDT Merge Algebra
 *
 * Captures the formal properties of Braid's datom store as a G-Set CvRDT,
 * the MERGE operation as set union, the three resolution modes (LWW, Lattice,
 * Multi) applied in the LIVE index, and the key convergence properties.
 *
 * Source spec references:
 *   spec/01-store.md  -- Store axioms L1-L4, LIVE definition
 *   spec/07-merge.md  -- MERGE operation, branching extension
 *   spec/04-resolution.md -- Resolution modes, composition proof (section 4.3.1)
 *
 * Invariant coverage:
 *   INV-STORE-004  (Merge Commutativity)
 *   INV-STORE-005  (Merge Associativity)
 *   INV-STORE-006  (Merge Idempotency)
 *   INV-STORE-007  (Merge Monotonicity)
 *   INV-MERGE-001  (Merge Is Set Union)
 *   INV-MERGE-008  (At-Least-Once Idempotent Delivery)
 *   INV-RESOLUTION-002 (Resolution Commutativity)
 *   INV-RESOLUTION-005 (LWW Semilattice Properties)
 *   INV-RESOLUTION-006 (Lattice Join Correctness)
 *)
EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS
    EntityIds,       \* Finite set of entity identifiers (e.g., {"e1","e2"})
    Attributes,      \* Finite set of attribute names (e.g., {"name","status"})
    Values,          \* Finite set of possible values (e.g., {"v1","v2","v3"})
    TxIds,           \* Finite set of transaction identifiers (modeled as integers for ordering)
    Agents,          \* Finite set of agent identifiers (e.g., {"alpha","beta","gamma"})
    ResolutionModes, \* Function: Attributes -> {"lww", "lattice", "multi"}
    LatticeOrder     \* Partial order for lattice-mode attributes: set of <<v1, v2>> pairs
                     \* meaning v1 <= v2 in the user-defined lattice

(* ---- Operation type ---- *)
Ops == {"assert", "retract"}

(* ---- Datom: the atomic fact [entity, attribute, value, tx, op] ---- *)
Datom == [
    entity    : EntityIds,
    attribute : Attributes,
    value     : Values,
    tx        : TxIds,
    op        : Ops
]

(* ---- A Store is a set of datoms (G-Set: grow-only set) ---- *)
Store == SUBSET Datom

(*
 * ================================================================
 * MERGE: Pure set union (INV-MERGE-001, C4)
 *
 * MERGE(S1, S2) = S1 \union S2
 *
 * No heuristics, no resolution, no filtering.
 * Conflict detection and resolution are post-merge concerns.
 * ================================================================
 *)
Merge(S1, S2) == S1 \union S2

(*
 * ================================================================
 * RETRACTION CHECK
 *
 * A datom d is retracted in store S if there exists a retraction
 * datom r with the same (entity, attribute, value) and r.tx > d.tx.
 * ================================================================
 *)
IsRetracted(S, d) ==
    \E r \in S :
        /\ r.entity    = d.entity
        /\ r.attribute = d.attribute
        /\ r.value     = d.value
        /\ r.op        = "retract"
        /\ r.tx        > d.tx

(*
 * ================================================================
 * CANDIDATE SET
 *
 * For a given entity e and attribute a, the candidate set is all
 * unretracted assert datoms in the store.
 * ================================================================
 *)
Candidates(S, e, a) ==
    { d \in S :
        /\ d.entity    = e
        /\ d.attribute = a
        /\ d.op        = "assert"
        /\ ~IsRetracted(S, d) }

CandidateValues(S, e, a) ==
    { d.value : d \in Candidates(S, e, a) }

(*
 * ================================================================
 * RESOLUTION MODES
 *
 * Three modes, each forming a join-semilattice:
 *
 * LWW (Last-Writer-Wins):
 *   Pick the value with the highest tx among unretracted asserts.
 *   Ties broken deterministically (in Braid: BLAKE3 hash; here: max value).
 *
 * Lattice:
 *   Compute the join (least upper bound) over the user-defined lattice.
 *
 * Multi (Multi-Value):
 *   Return the full set of unretracted values.
 * ================================================================
 *)

(*
 * LWW resolution: pick the value from the datom with the maximum tx.
 * If multiple datoms share the same max tx, deterministic tiebreak
 * by choosing the maximum value (models BLAKE3 hash comparison from
 * ADR-RESOLUTION-009).
 *)
LwwResolve(S, e, a) ==
    LET cands == Candidates(S, e, a)
    IN IF cands = {} THEN {}
       ELSE LET maxTx == CHOOSE t \in { d.tx : d \in cands } :
                            \A t2 \in { d.tx : d \in cands } : t >= t2
                maxTxCands == { d \in cands : d.tx = maxTx }
                winner == CHOOSE v \in { d.value : d \in maxTxCands } :
                            \A v2 \in { d.value : d \in maxTxCands } : v >= v2
            IN {winner}

(*
 * Lattice resolution: compute the join (LUB) of all candidate values.
 *
 * LatticeOrder is a set of <<v1, v2>> pairs meaning v1 <= v2.
 * The join of a set V is the least element u such that for all v in V,
 * v <= u (i.e., <<v, u>> \in LatticeOrder or v = u).
 *
 * For model checking with small state spaces, we compute this directly.
 *)
LatticeLeq(v1, v2) ==
    v1 = v2 \/ <<v1, v2>> \in LatticeOrder

LatticeJoin(V) ==
    IF V = {} THEN {}
    ELSE LET upperBounds == { u \in Values :
                \A v \in V : LatticeLeq(v, u) }
         IN IF upperBounds = {} THEN V  \* No join exists -> return all (error signal)
            ELSE LET lub == CHOOSE u \in upperBounds :
                               \A u2 \in upperBounds : LatticeLeq(u, u2)
                 IN {lub}

LatticeResolve(S, e, a) ==
    LatticeJoin(CandidateValues(S, e, a))

(*
 * Multi resolution: return all unretracted values (trivial semilattice
 * under set union).
 *)
MultiResolve(S, e, a) ==
    CandidateValues(S, e, a)

(*
 * ================================================================
 * RESOLVE: dispatch to the appropriate mode
 * ================================================================
 *)
Resolve(S, e, a) ==
    LET mode == ResolutionModes[a]
    IN CASE mode = "lww"     -> LwwResolve(S, e, a)
         [] mode = "lattice" -> LatticeResolve(S, e, a)
         [] mode = "multi"   -> MultiResolve(S, e, a)

(*
 * ================================================================
 * LIVE FUNCTION
 *
 * LIVE(S) computes the current resolved state from the store.
 * It produces a mapping from (entity, attribute) pairs to resolved values.
 *
 * LIVE(S) = { <<e, a>> -> Resolve(S, e, a) : e in entities(S), a in attrs(S) }
 *
 * This is a derived CRDT: if S forms a G-Set CvRDT and Resolve is a
 * monotonic function from S to a per-attribute semilattice, then LIVE
 * inherits strong eventual consistency from S.
 * (spec/04-resolution.md section 4.3.1 Corollary)
 * ================================================================
 *)
EntitiesIn(S) == { d.entity : d \in S }
AttrsIn(S)    == { d.attribute : d \in S }

Live(S) ==
    [ pair \in (EntitiesIn(S) \X AttrsIn(S)) |->
        Resolve(S, pair[1], pair[2]) ]

(*
 * For comparing LIVE states across stores with potentially different
 * entity/attribute domains, we need a normalized comparison.
 * Two LIVE states are equivalent if for every (e,a) pair present in
 * either store, the resolved value sets are equal.
 *)
LiveEquivalent(S1, S2) ==
    LET allEntities == EntitiesIn(S1) \union EntitiesIn(S2)
        allAttrs    == AttrsIn(S1) \union AttrsIn(S2)
    IN \A e \in allEntities :
       \A a \in allAttrs :
           Resolve(S1, e, a) = Resolve(S2, e, a)

(*
 * ================================================================
 * VARIABLES: Multi-agent store replication
 *
 * Each agent maintains a local store (replica). Agents can:
 *   1. TRANSACT: add new datoms to their local store
 *   2. MERGE: merge another agent's store into their own
 *
 * We model N agents, each with a local store, communicating
 * via pairwise merge operations.
 * ================================================================
 *)
VARIABLES
    stores,      \* Function: Agents -> Store (each agent's local replica)
    delivered    \* Set of <<sender, receiver>> pairs that have merged
                 \* (tracks which merges have happened for liveness)

vars == <<stores, delivered>>

(*
 * ================================================================
 * INITIAL STATE
 *
 * All agents start with the empty store (or genesis store).
 * For model checking purposes, we start empty and let agents transact.
 * ================================================================
 *)
Init ==
    /\ stores    = [a \in Agents |-> {}]
    /\ delivered = {}

(*
 * ================================================================
 * ACTIONS
 * ================================================================
 *)

(*
 * Agent 'agent' transacts a new datom into its local store.
 * This models the TRANSACT operation.
 * The datom is added to the agent's local store only.
 *)
Transact(agent, d) ==
    /\ d \in Datom
    /\ d \notin stores[agent]          \* Only add new datoms
    /\ stores'    = [stores EXCEPT ![agent] = stores[agent] \union {d}]
    /\ delivered' = delivered           \* No change to delivery state

(*
 * Agent 'receiver' merges the store of agent 'sender'.
 * MERGE is pure set union (INV-MERGE-001).
 *)
MergeStores(sender, receiver) ==
    /\ sender /= receiver
    /\ stores'    = [stores EXCEPT ![receiver] = Merge(stores[receiver], stores[sender])]
    /\ delivered' = delivered \union {<<sender, receiver>>}

(*
 * Next-state relation: either some agent transacts or some pair merges.
 *)
Next ==
    \/ \E agent \in Agents, d \in Datom :
        Transact(agent, d)
    \/ \E sender, receiver \in Agents :
        MergeStores(sender, receiver)

(*
 * Fairness: eventually every agent's data reaches every other agent.
 * This is needed for the liveness property (strong eventual consistency).
 *)
Fairness ==
    \A s, r \in Agents :
        s /= r => WF_vars(MergeStores(s, r))

Spec == Init /\ [][Next]_vars /\ Fairness

(*
 * ================================================================
 * PROPERTIES TO CHECK
 * ================================================================
 *)

(*
 * PROPERTY 1: Merge Commutativity (INV-STORE-004, L1)
 *
 * For all pairs of agent stores, merging in either order produces
 * the same datom set.
 *)
MergeCommutativity ==
    \A a1, a2 \in Agents :
        Merge(stores[a1], stores[a2]) = Merge(stores[a2], stores[a1])

(*
 * PROPERTY 2: Merge Associativity (INV-STORE-005, L2)
 *
 * For all triples of agent stores, regrouping merge produces the
 * same datom set.
 *)
MergeAssociativity ==
    \A a1, a2, a3 \in Agents :
        Merge(Merge(stores[a1], stores[a2]), stores[a3]) =
        Merge(stores[a1], Merge(stores[a2], stores[a3]))

(*
 * PROPERTY 3: Merge Idempotency (INV-STORE-006, L3)
 *
 * Merging a store with itself produces no change.
 *)
MergeIdempotency ==
    \A a \in Agents :
        Merge(stores[a], stores[a]) = stores[a]

(*
 * PROPERTY 4: Merge Monotonicity (INV-STORE-007, L4)
 *
 * Both inputs are subsets of the merge result.
 *)
MergeMonotonicity ==
    \A a1, a2 \in Agents :
        /\ stores[a1] \subseteq Merge(stores[a1], stores[a2])
        /\ stores[a2] \subseteq Merge(stores[a1], stores[a2])

(*
 * PROPERTY 5: LIVE Determinism
 *
 * The same store always produces the same LIVE view.
 * (This is trivially true since Resolve is a pure function,
 * but we verify it holds across all reachable states.)
 *)
LiveDeterminism ==
    \A a1, a2 \in Agents :
        stores[a1] = stores[a2] => LiveEquivalent(stores[a1], stores[a2])

(*
 * PROPERTY 6: Strong Eventual Consistency (SEC)
 *
 * If two replicas have received the same set of updates (i.e.,
 * their stores contain the same datom set), they have the same
 * LIVE state.
 *
 * This is the key CRDT convergence guarantee:
 *   same datoms => same resolved view
 *
 * Combined with the liveness property (fairness ensures eventual
 * delivery), this gives Strong Eventual Consistency.
 *)
StrongEventualConsistency ==
    \A a1, a2 \in Agents :
        stores[a1] = stores[a2] =>
            LiveEquivalent(stores[a1], stores[a2])

(*
 * PROPERTY 7: Idempotent Delivery (INV-MERGE-008)
 *
 * Duplicate merge operations are harmless.
 * MERGE(MERGE(S, R), R) = MERGE(S, R)
 *)
IdempotentDelivery ==
    \A a1, a2 \in Agents :
        Merge(Merge(stores[a1], stores[a2]), stores[a2]) =
        Merge(stores[a1], stores[a2])

(*
 * PROPERTY 8: Resolution-Merge Composition (spec/04-resolution.md section 4.3.1)
 *
 * LIVE(MERGE(S1, S2)) = LIVE(MERGE(S2, S1))   -- commutativity
 *
 * Resolution in the LIVE layer commutes with set-union merge in the
 * store layer. This is the "derived CRDT" property.
 *)
ResolutionMergeCommutativity ==
    \A a1, a2 \in Agents :
        LiveEquivalent(
            Merge(stores[a1], stores[a2]),
            Merge(stores[a2], stores[a1])
        )

(*
 * PROPERTY 9: Resolution-Merge Associativity
 *
 * LIVE(MERGE(MERGE(S1,S2),S3)) = LIVE(MERGE(S1,MERGE(S2,S3)))
 *)
ResolutionMergeAssociativity ==
    \A a1, a2, a3 \in Agents :
        LiveEquivalent(
            Merge(Merge(stores[a1], stores[a2]), stores[a3]),
            Merge(stores[a1], Merge(stores[a2], stores[a3]))
        )

(*
 * PROPERTY 10: No Merge Data Loss (NEG-MERGE-001)
 *
 * Safety property: no datom from either input is lost during merge.
 * []~(E d in S1 \union S2 : d \notin MERGE(S1, S2))
 *)
NoMergeDataLoss ==
    \A a1, a2 \in Agents :
        \A d \in stores[a1] \union stores[a2] :
            d \in Merge(stores[a1], stores[a2])

(*
 * PROPERTY 11: Append-Only Monotonicity (INV-STORE-001)
 *
 * Once a datom is in an agent's store, it remains there forever.
 * This is checked as a temporal property on individual stores.
 *)
AppendOnlyMonotonicity ==
    \A a \in Agents :
        \A d \in stores[a] :
            [][d \in stores'[a]]_vars

(*
 * LIVENESS PROPERTY: Eventual Convergence
 *
 * If all agents eventually merge with all others (ensured by Fairness),
 * then eventually all agents have the same LIVE state.
 *)
EventualConvergence ==
    <>(\A a1, a2 \in Agents : LiveEquivalent(stores[a1], stores[a2]))

(*
 * ================================================================
 * TYPE INVARIANT
 *
 * Structural well-formedness of the state.
 * ================================================================
 *)
TypeInvariant ==
    /\ \A a \in Agents : stores[a] \subseteq Datom
    /\ delivered \subseteq (Agents \X Agents)

(*
 * ================================================================
 * COMPOSITE INVARIANTS
 *
 * All safety properties that must hold in every reachable state.
 * ================================================================
 *)
SafetyProperties ==
    /\ TypeInvariant
    /\ MergeCommutativity
    /\ MergeAssociativity
    /\ MergeIdempotency
    /\ MergeMonotonicity
    /\ LiveDeterminism
    /\ StrongEventualConsistency
    /\ IdempotentDelivery
    /\ ResolutionMergeCommutativity
    /\ ResolutionMergeAssociativity
    /\ NoMergeDataLoss

(*
 * ================================================================
 * THEOREMS (checked by TLC model checker)
 * ================================================================
 *)
THEOREM Spec => []SafetyProperties
THEOREM Spec => EventualConvergence

=============================================================================
