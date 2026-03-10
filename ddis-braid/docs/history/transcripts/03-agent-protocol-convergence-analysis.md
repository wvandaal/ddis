# Chapter 3: Agent Protocol Convergence Analysis

> Synthesis of formal agentic system architecture with multi-agent CRDT coordination protocol. Analyzes convergence between dual-process cognitive architecture (System 1/2, EAV fact stores, context assembly) and multi-agent coordination mechanisms (uncertainty tensors, spectral authority, bilateral loops). Identifies protocol gaps and proposes unified agent operations.

---

## Claude

This is the operational core. Every coordination decision in the system reduces to a query. I'll build from primitive patterns up to the full coordination logic, flagging monotonicity at each level because that determines whether the query is safe to run uncoordinated.

---

## The Query Language

First, the dialect. I'm using Datomic-style Datalog syntax because it's well-documented and maps directly to our schema, but with three extensions needed for the CRDT-aware context:

```
STANDARD DATOMIC CLAUSES:

[:find ?vars ...                    — projection (what to return)
 :in $ ?input-vars ...              — parameterization ($ = current db)
 :where                             — conjunction of pattern clauses
 [?e :attr ?v]                      — entity-attribute-value pattern
 [?e :attr ?v ?tx]                  — with transaction binding
 [?e :attr ?v ?tx ?op]              — with operation binding (assert/retract)
 [(pred ?x ?y)]                     — predicate expression
 [(fn ?x) ?result]                  — function expression
 (rule ?x ?y)                       — rule invocation
 (not [?e :attr ?v])                — negation (NON-MONOTONIC)
 (or [?e :attr ?v] [?e :attr2 ?v]) — disjunction
]

EXTENSIONS FOR CRDT CONTEXT:

[:frontier ?frontier-ref            — bind query to a specific frontier 
                                      (default: local frontier)
 :stability-min ?threshold          — only use facts with commitment ≥ threshold
 :barrier :required                 — assert that a sync barrier must be held
]

:frontier restricts the query to datoms visible at a given frontier 
(the down-set in the causal partial order). Without it, the query 
runs against the agent's full local knowledge.

:stability-min filters out low-commitment facts before evaluation. 
This is a pre-filter, not a post-filter — it affects which datoms 
participate in joins, not just which results are returned.

:barrier declares that this query requires a consistent cut. The 
engine MUST refuse to evaluate if no sync barrier is held. This is 
the static enforcement mechanism for the CALM theorem boundary.
```

### Evaluation Strategy

**Bottom-up semi-naive evaluation** (following datafrog's approach). Rules are evaluated iteratively: each round computes new tuples from the previous round's delta until fixpoint. This is well-suited to Rust because the core loop is tight and cache-friendly.

**Stratification for negation.** Queries containing `not` or aggregation are evaluated in strata. Within a stratum, all rules are monotonic. Between strata, the negation/aggregation is applied to the completed result of the previous stratum. This is standard and well-understood.

**The critical interaction with CRDTs:** Within a single stratum of monotonic rules, the query result is a monotonic function of the input datom set. This means:

1. Adding more datoms can only add to the result (never remove)
2. Two agents with overlapping but different datom sets get results that are both subsets of the result over the union
3. After merging, the result over the merged set is a superset of both pre-merge results

This is CALM compliance for the monotonic stratum. Non-monotonic strata break this property — which is why they're gated by `:stability-min` or `:barrier`.

---

## Stratum 0: Primitive Patterns (Monotonic)

These are the base-level queries that everything else builds on. All are monotonic and safe to run uncoordinated at any frontier.

### Current Value Resolution

The most fundamental query: "what is the current value of attribute `a` for entity `e`?" This must respect the resolution mode (A5).

```datalog
;; RULE: current-value
;; Resolves the "current" value of a cardinality-one attribute
;; according to its resolution mode.
;;
;; For :lww — latest assertion by HLC wins
;; For :lattice — lattice join of all concurrent assertions
;; For :multi — returns all values (caller must handle set)
;;
;; Retractions cancel specific prior assertions (A2: Op semantics)

;; Step 1: Compute the "live" assertions — those not retracted
;; MONOTONIC (filtering by set membership)

[:find ?e ?a ?v ?tx
 :where
 [?e ?a ?v ?tx :assert]
 (not-join [?tx-r]                    ;; NON-MONOTONIC — see note below
   [?e ?a ?v ?tx-r :retract]
   [(tx-after? ?tx-r ?tx)])]          ;; retraction is causally after assertion
```

**Monotonicity note:** This query contains `not-join`, which is non-monotonic. However, it's a *stratified* non-monotonicity: the retraction set is computed in a lower stratum (just the raw retract datoms), and the negation is applied in the next stratum. This is safe under stratified evaluation *if* the set of retract datoms is stable at the agent's frontier.

This is the fundamental tension in the system: even the most basic "current value" query involves negation (checking that a value hasn't been retracted). We have three options, and I want to present them honestly because this is a foundational choice:

**Option A: Accept stratified negation as safe for local-frontier queries.** The reasoning is that an agent's local frontier is fixed at query time. The set of retract datoms it knows about won't change during evaluation. So stratified negation over a fixed frontier is deterministic and consistent, even if another agent with a different frontier would get a different answer.

**Option B: Separate the "live set" computation into a materialized view.** Maintain a separate index of `[e a v]` triples that represent the current live state, updated incrementally as new datoms arrive. Queries over the live view are monotonic (just lookups). The non-monotonicity is pushed into the materialization step.

**Option C: Don't use negation at all. Use the assertion/retraction history directly.** Define "current value" as a fold over the datom history for `[e a]`, applying assert/retract operations in causal order. This is a function, not a query — it's evaluated imperatively rather than declaratively.

**My recommendation: Option B.** It's the cleanest separation of concerns. The materialized live-set is updated by a deterministic incremental process (analogous to DDIS's `materialize` fold), and all Datalog queries run over the live set without negation. This makes the entire query stratum monotonic, which means full CALM compliance for all coordination queries.

The materialized live-set becomes a fifth index alongside EAVT, AEVT, VAET, AVET:

```
EAVT  — all datoms, sorted by entity-attribute-value-tx
AEVT  — all datoms, sorted by attribute-entity-value-tx
VAET  — ref datoms, sorted by value-attribute-entity-tx
AVET  — unique/indexed datoms, sorted by attribute-value-entity-tx
LIVE  — current live state: [e a v] after applying all assert/retract ops
        Resolution-mode-aware: for :lww, one value per [e a];
        for :lattice, the lattice join; for :multi, the full set
```

With this, the "current value" query becomes:

```datalog
;; RULE: current-value (over LIVE index — fully monotonic)
;; No negation required. The LIVE index is the materialized result
;; of applying all assert/retract operations.

[:find ?e ?a ?v
 :in $ ?target-entity ?target-attr
 :where
 [?target-entity ?target-attr ?v]]    ;; implicit: queries LIVE index
```

I'll proceed with Option B for all subsequent queries.

**Invariant:**

```
INV-LIVE-001: Live Index Consistency
The LIVE index MUST be the deterministic result of applying all 
assert and retract datoms in causal order with the declared 
resolution mode per attribute.

Formally: LIVE(S) = fold(causal-sort(S), apply-resolution)

Where apply-resolution for each [e a]:
  :lww    → value from the assertion with the greatest HLC
  :lattice → ⊔ over all unretracted assertions
  :multi  → set of all unretracted values

Violation: The LIVE index shows [bob :status :active] but the 
datom history contains a retraction of this value with no 
subsequent re-assertion.

Verification: Full recomputation of LIVE from raw datoms; 
compare to incremental result.
```

---

## Stratum 1: Graph Traversal Rules (Monotonic)

These rules compute transitive relationships over the datom graph. They're recursive Datalog — the core of what makes Datalog more expressive than SQL for graph queries.

### Causal Ancestry

```datalog
;; RULE: causal-ancestor
;; Transitive closure of the causal-parents relation on transactions.
;; ?ancestor causally precedes ?descendant.

(causal-ancestor ?ancestor ?descendant) :-
  [?descendant :tx/causal-parents ?ancestor].

(causal-ancestor ?ancestor ?descendant) :-
  [?descendant :tx/causal-parents ?mid],
  (causal-ancestor ?ancestor ?mid).

;; MONOTONIC: adding new transactions can only add new ancestry paths.
;; Semi-naive evaluation: each round discovers ancestors one hop further.
;; Terminates because the causal graph is a DAG (no cycles by construction).
```

### Forward Causal Cone (for commitment function w)

```datalog
;; RULE: causal-descendant
;; The inverse of causal-ancestor. ?descendant depends on ?ancestor.

(causal-descendant ?descendant ?ancestor) :-
  [?descendant :tx/causal-parents ?ancestor].

(causal-descendant ?descendant ?ancestor) :-
  [?descendant :tx/causal-parents ?mid],
  (causal-descendant ?mid ?ancestor).

;; RULE: datom-dependents
;; All datoms that transitively depend on datom ?d
;; (i.e., were asserted in transactions that causally follow ?d's tx)

[:find (count ?dep-datom)
 :in $ ?target-datom
 :where
 [?target-datom _ _ ?target-tx]       ;; get the tx of the target datom
 (causal-descendant ?dep-tx ?target-tx)
 [?dep-datom _ _ ?dep-tx]]            ;; all datoms in descendant txs

;; This count IS the commitment function w(d).
;; MONOTONIC in the datom count (adding datoms can only increase w).
;; NON-MONOTONIC in the count aggregation — but the count can only 
;; increase, so it's monotonically non-decreasing as a function of 
;; store growth. This makes it safe for the stability computation.
```

### Entity Dependency Graph

```datalog
;; RULE: entity-depends-on
;; Entity ?a depends on entity ?b if ?a has a ref attribute pointing to ?b,
;; OR if ?a was asserted in a transaction that causally follows a 
;; transaction asserting ?b.

(entity-depends-on ?a ?b) :-
  [?a ?attr ?b],
  [?attr :db/valueType :db.type/ref].

(entity-depends-on ?a ?b) :-
  [?a _ _ ?tx-a],
  [?b _ _ ?tx-b],
  (causal-ancestor ?tx-b ?tx-a),
  [(!= ?a ?b)].

;; Transitive closure
(entity-depends-on-transitive ?a ?b) :-
  (entity-depends-on ?a ?b).

(entity-depends-on-transitive ?a ?b) :-
  (entity-depends-on ?a ?mid),
  (entity-depends-on-transitive ?mid ?b).
```

### Cross-Reference Reachability (for DDIS contradiction detection)

```datalog
;; RULE: reach-set
;; The set of all entities reachable from ?source via cross-references.
;; Used for Jaccard similarity in Tier 2 contradiction detection.

(reachable ?source ?target) :-
  [?xref :xref/source ?source],
  [?xref :xref/target ?target],
  [?xref :xref/resolved true].

(reachable ?source ?target) :-
  [?xref :xref/source ?source],
  [?xref :xref/target ?mid],
  [?xref :xref/resolved true],
  (reachable ?mid ?target).
```

---

## Stratum 2: Uncertainty Computation (Mixed Monotonicity)

This is the core coordination logic. Each dimension of the uncertainty tensor is a separate computation with different monotonicity properties.

### Epistemic Uncertainty (σ_e)

"How much do we know about this entity?"

```datalog
;; RULE: schema-expected-attributes
;; For a given entity type, what attributes are expected?
;; This is derived from the schema: all attributes whose :db/doc
;; mentions this entity type, or that appear on exemplar entities
;; of this type.

;; For now, we use a simpler heuristic: count attributes that 
;; have been observed on ANY entity of the same type, and compare
;; to how many this specific entity has.

;; Step 1: Determine entity type (by convention: :entity/type attr)
;; Step 2: Count distinct attributes observed on peers of same type  
;; Step 3: Count distinct attributes on target entity
;; Step 4: Ratio = observed / expected

;; QUERY: epistemic-uncertainty
[:find ?entity (- 1.0 (/ ?observed ?expected))
 :in $ ?entity
 :where
 [?entity :entity/type ?type]
 
 ;; Count attributes on this entity
 [(ground ?entity) ?e]
 [?e ?any-attr _]
 [(count-distinct ?any-attr) ?observed]
 
 ;; Count attributes observed across all entities of this type
 [?peer :entity/type ?type]
 [?peer ?peer-attr _]
 [(count-distinct ?peer-attr) ?expected]]
```

**Monotonicity:** Adding more datoms about the entity *decreases* σ_e (more observed attributes → ratio increases → uncertainty decreases). Adding more datoms about peer entities *increases* σ_e (more expected attributes → ratio decreases). In practice, the expected set stabilizes quickly (schema is defined early), so σ_e is dominated by the monotonically decreasing numerator term. This is *eventually* monotonically non-increasing but not strictly so.

**Assessment:** Safe to run uncoordinated for practical purposes. The worst case is that an agent overestimates epistemic uncertainty (hasn't seen schema datoms from other agents yet), which causes it to *under*-delegate. This is the conservative failure mode.

### Aleatory Uncertainty (σ_a)

"How much do agents disagree about this entity?"

This is the entropy computation over concurrent values for cardinality-one attributes. This is the most complex query because it requires grouping concurrent assertions and computing Shannon entropy.

```datalog
;; RULE: concurrent-values
;; For a given entity-attribute pair, find all values that are 
;; "concurrent" — asserted in causally independent transactions.
;;
;; Two assertions are concurrent if neither's tx is a causal 
;; ancestor of the other's.

;; Step 1: Get all live assertions for [e a]
;; Step 2: For each pair, check causal independence
;; Step 3: Group concurrent values

;; QUERY: concurrent-assertions
;; Returns all live values for a given [e a] with their asserting agents
[:find ?entity ?attr ?value ?agent
 :in $ ?entity ?attr
 :where
 [?entity ?attr ?value ?tx :assert]
 [?tx :tx/agent ?agent]
 ;; Exclude retracted values (LIVE index handles this)
 ]

;; QUERY: aleatory-uncertainty
;; Computes entropy over the value distribution for each attr of an entity
;; 
;; This CANNOT be expressed in pure Datalog because entropy requires:
;; 1. Grouping (group by value, count agents per value)
;; 2. Division (proportion = count / total)
;; 3. Logarithm (entropy = -Σ p log p)
;;
;; These are aggregate functions that break the relational algebra.
;; 
;; DECISION: σ_a is computed as a DERIVED FUNCTION, not a pure query.
;; The Datalog engine provides the input (concurrent-assertions), 
;; and a Rust function computes the entropy.

fn aleatory_uncertainty(entity: EntityId, store: &Store) -> f64 {
    let mut total_entropy = 0.0;
    let mut attr_count = 0;
    
    for attr in store.attributes_of(entity) {
        if store.cardinality(attr) != Cardinality::One { continue; }
        
        // Get all live values with agent counts
        let values: HashMap<Value, usize> = store.query(
            "[:find ?value (count ?agent)
              :in $ ?entity ?attr
              :where
              [?entity ?attr ?value ?tx]
              [?tx :tx/agent ?agent]]",
            entity, attr
        );
        
        if values.len() <= 1 { continue; } // no disagreement
        
        let total: f64 = values.values().sum::<usize>() as f64;
        let entropy: f64 = values.values()
            .map(|&c| {
                let p = c as f64 / total;
                -p * p.ln()
            })
            .sum();
        
        // Normalize to [0,1] by dividing by max possible entropy
        let max_entropy = (values.len() as f64).ln();
        total_entropy += entropy / max_entropy;
        attr_count += 1;
    }
    
    if attr_count == 0 { 0.0 } else { total_entropy / attr_count as f64 }
}
```

**Monotonicity:** σ_a is NOT monotonically non-increasing. Adding a new concurrent assertion can *increase* entropy (more disagreement). This is correct — aleatory uncertainty genuinely increases when agents disagree more. It's the one dimension that can grow, and that growth is the signal to the coordination layer that deliberation is needed.

**Assessment:** Safe to run uncoordinated because overestimation (seeing disagreement that might be resolved elsewhere) causes conservative behavior (escalation), and underestimation (not yet seeing a conflicting assertion) is self-correcting on the next merge.

### Consequential Uncertainty (σ_c)

"How much would it hurt to get this entity wrong?"

```datalog
;; RULE: consequential-uncertainty
;; σ_c(e) = w(e) × Σ_{d ∈ dependents(e)} uncertainty_scalar(d)
;;
;; This is recursive: consequential uncertainty depends on the 
;; uncertainty of dependents, which may themselves have consequential
;; uncertainty depending on their dependents.
;;
;; To avoid infinite recursion, we compute this bottom-up:
;; leaf entities (no dependents) have σ_c = 0.
;; Inner entities accumulate from leaves upward.
;;
;; This is a MONOTONIC DATALOG computation if we treat it as a 
;; fixed-point over the dependency DAG.

;; Step 1: Leaf entities (no outgoing entity-depends-on edges)
(consequential-base ?entity 0.0) :-
  [?entity :entity/type _],
  (not (entity-depends-on _ ?entity)).  ;; NON-MONOTONIC: negation

;; Step 2: Recursive accumulation
;; NOTE: This requires stratified negation (for the leaf base case)
;; followed by monotonic fixed-point computation.

;; DECISION: Like σ_a, this is a DERIVED FUNCTION, not pure Datalog.
;; The dependency graph is extracted by Datalog; the accumulation 
;; is computed by a Rust function doing bottom-up traversal.

fn consequential_uncertainty(
    entity: EntityId, 
    store: &Store,
    cache: &mut HashMap<EntityId, f64>,  // memoization
) -> f64 {
    if let Some(&cached) = cache.get(&entity) {
        return cached;
    }
    
    let commitment = store.commitment_weight(entity);
    
    let dependents: Vec<EntityId> = store.query(
        "[:find ?dep
          :in $ ?entity
          :where
          (entity-depends-on ?dep ?entity)]",
        entity
    );
    
    if dependents.is_empty() {
        cache.insert(entity, 0.0);
        return 0.0;
    }
    
    // CRITICAL: exclude uncertainty measurements of THIS entity
    // from the dependent set (INV-MEASURE-001 exclusion clause)
    let dependents: Vec<EntityId> = dependents.into_iter()
        .filter(|d| !is_uncertainty_measurement_of(store, *d, entity))
        .collect();
    
    let dep_uncertainty_sum: f64 = dependents.iter()
        .map(|d| scalar_uncertainty(store, *d, cache))
        .sum();
    
    let sigma_c = (commitment as f64) * dep_uncertainty_sum 
                  / (dependents.len() as f64 + 1.0);  // +1 smoothing
    let sigma_c = sigma_c.min(1.0);  // clamp to [0,1]
    
    cache.insert(entity, sigma_c);
    sigma_c
}
```

**The INV-MEASURE-001 exclusion is enforced here.** Line 19 of the function filters out uncertainty measurement entities that target the entity being measured. Without this filter, the function would diverge in the presence of self-referential measurements.

**Monotonicity:** σ_c is monotonically non-decreasing with respect to the dependency graph size (more dependents → higher σ_c) and monotonically non-increasing with respect to dependent resolution (resolved dependents have lower uncertainty → lower σ_c). The net direction depends on whether the system is growing (adding entities) or converging (resolving uncertainties). In the bilateral loop, the system first grows (forward flow) then converges (backward flow), so σ_c first increases then decreases. This is the expected behavior.

### Combined Uncertainty Scalar

```rust
fn scalar_uncertainty(
    store: &Store, 
    entity: EntityId,
    cache: &mut HashMap<EntityId, f64>,
) -> f64 {
    let sigma_e = epistemic_uncertainty(entity, store);
    let sigma_a = aleatory_uncertainty(entity, store);
    let sigma_c = consequential_uncertainty(entity, store, cache);
    
    // Weighted L2 norm
    // α, β, γ are system parameters stored as datoms in the meta-schema
    let alpha = store.system_param(":uncertainty/alpha"); // default 0.4
    let beta  = store.system_param(":uncertainty/beta");  // default 0.4
    let gamma = store.system_param(":uncertainty/gamma"); // default 0.2
    
    (alpha * sigma_e.powi(2) 
     + beta * sigma_a.powi(2) 
     + gamma * sigma_c.powi(2))
    .sqrt()
}
```

The default weights emphasize epistemic and aleatory uncertainty equally (0.4 each) with consequential uncertainty weighted lower (0.2). Rationale: epistemic and aleatory uncertainty are *actionable* — you can reduce them by exploring more or deliberating more. Consequential uncertainty is *structural* — it depends on the graph topology, which changes slowly. Overweighting it would cause the system to be overly cautious about well-understood entities that happen to be heavily depended upon.

These weights are themselves facts in the store and can be adjusted per deployment.

---

## Stratum 3: Authority Computation

The spectral authority derivation requires building the agent-entity contribution graph and computing its decomposition. This is not expressible in Datalog — it's linear algebra. But the *inputs* are Datalog queries, and the *outputs* are datoms in the store.

### Building the Contribution Graph

```datalog
;; QUERY: contribution-edges
;; All (agent, entity, weight) triples where the agent has contributed
;; to the entity through verified assertions.

[:find ?agent ?entity (sum ?weight)
 :where
 [?datom _ _ ?tx :assert]
 [?tx :tx/agent ?agent]
 [?datom :entity/type _]              ;; ?datom is about an entity
 
 ;; Weight = 1 for unverified, 2 for witnessed, 3 for challenged-confirmed
 ;; Computed via a helper rule:
 (contribution-weight ?datom ?weight)]

;; RULE: contribution-weight
;; Weight of a contribution based on its verification status

(contribution-weight ?datom 3) :-
  [?datom _ _ ?tx],
  [?witness :witness/invariant ?inv],
  [?challenge :challenge/witness ?witness],
  [?challenge :challenge/verdict :confirmed],
  ;; the datom is about an entity maintained by this invariant
  [?inv :inv/owner-module ?mod],
  [?datom :entity/type ?type].         ;; simplified; real version checks 
                                        ;; module-entity relationship

(contribution-weight ?datom 2) :-
  [?datom _ _ ?tx],
  [?witness :witness/invariant ?inv],
  [?witness :witness/status :valid],
  (not [?challenge :challenge/witness ?witness]
       [?challenge :challenge/verdict :confirmed]).

(contribution-weight ?datom 1) :-
  [?datom _ _ ?tx],
  (not [?witness :witness/invariant _]  ;; no witness covers this
       (contribution-weight ?datom 2)),
  (not (contribution-weight ?datom 3)).
```

### Spectral Decomposition

```rust
/// Build the bipartite adjacency matrix and compute its SVD.
/// Returns authority scores per (agent, entity) pair.
fn compute_spectral_authority(store: &Store) -> Vec<AuthorityScore> {
    // Step 1: Get contribution edges from Datalog
    let edges: Vec<(AgentId, EntityId, f64)> = store.query(
        "[:find ?agent ?entity ?weight
          :where (contribution-edge ?agent ?entity ?weight)]"
    );
    
    // Step 2: Build bipartite adjacency matrix
    // Rows = agents, Columns = entities, Values = contribution weights
    let (agent_idx, entity_idx, matrix) = build_adjacency_matrix(&edges);
    
    // Step 3: SVD decomposition
    // A ≈ U_k × Σ_k × V_k^T
    // k = min(50, agent_count, entity_count)
    let k = 50.min(agent_idx.len()).min(entity_idx.len());
    let svd = truncated_svd(&matrix, k);
    
    // Step 4: Authority scores
    // For agent i on entity j:
    //   authority(i,j) = Σ_d (U[i,d] × Σ[d] × V[j,d])
    // This is the reconstruction of the original matrix through the 
    // low-rank approximation — it captures the "structural" contribution
    // pattern, not just the raw edge weight.
    let mut scores = Vec::new();
    for (agent, i) in &agent_idx {
        for (entity, j) in &entity_idx {
            let score: f64 = (0..k)
                .map(|d| svd.u[(*i, d)] * svd.sigma[d] * svd.vt[(d, *j)])
                .sum();
            
            if score > 0.0 {
                scores.push(AuthorityScore {
                    agent: *agent,
                    entity: *entity,
                    score,
                    contribution_count: direct_contribution_count(
                        &edges, *agent, *entity
                    ),
                });
            }
        }
    }
    
    scores
}
```

### Why Spectral and Not Just Raw Contribution Count

The spectral decomposition captures something that raw contribution counting misses: **transitive authority.** If agent α has contributed heavily to entities A, B, and C, and entities A, B, and C are all closely related to entity D (through cross-references or shared attributes), then agent α has authority over entity D *even if it has never directly touched D*. The SVD captures this by projecting agents and entities into a shared latent space where proximity reflects structural similarity.

This is the same mathematical machinery DDIS already uses for LSI search (SVD over the term-document matrix). Applied to the agent-entity matrix instead, it gives you authority instead of relevance. The analogy is precise: LSI finds "this document is relevant to this query because it's similar to other documents that match" — spectral authority finds "this agent has authority on this entity because it has contributed to structurally similar entities."

### Delegation Threshold Computation

```datalog
;; QUERY: delegation-threshold
;; The minimum authority required to work on a given entity.
;; Derived from graph metrics and consequential uncertainty.

[:find ?entity ?threshold
 :in $ ?entity
 :where
 [?metrics :metrics/entity ?entity]
 [?metrics :metrics/betweenness ?betweenness]
 [?metrics :metrics/in-degree ?in-degree]
 [?unc :uncertainty/entity ?entity]
 [?unc :uncertainty/consequential ?sigma-c]
 
 ;; Schema cardinality factor: entities with more cardinality-one
 ;; attributes are more conflict-prone
 [(entity-conflict-surface ?entity) ?surface]
 
 ;; Threshold formula:
 ;; threshold = normalize(
 ;;   w1 * betweenness + w2 * in-degree + w3 * σ_c + w4 * surface
 ;; )
 [(* 0.3 ?betweenness) ?t1]
 [(* 0.2 ?in-degree) ?t2]
 [(* 0.3 ?sigma-c) ?t3]
 [(* 0.2 ?surface) ?t4]
 [(+ ?t1 ?t2 ?t3 ?t4) ?threshold]]

;; RULE: entity-conflict-surface
;; Fraction of an entity's attributes that are cardinality-one
;; (and therefore susceptible to conflict)

[(entity-conflict-surface ?entity) ?surface] :-
  [?entity _ _ ?tx],
  [(count-attrs ?entity :one) ?one-count],
  [(count-attrs ?entity :all) ?all-count],
  [(/ ?one-count ?all-count) ?surface].
```

### The Delegation Decision

```datalog
;; QUERY: can-resolve
;; Can agent ?agent resolve conflicts on entity ?entity?

[:find ?agent ?entity
 :in $ ?agent ?entity
 :where
 [?auth :authority/agent ?agent]
 [?auth :authority/entity ?entity]
 [?auth :authority/score ?score]
 
 [?deleg :delegation/entity ?entity]
 [?deleg :delegation/threshold ?threshold]
 
 [(>= ?score ?threshold)]]

;; QUERY: delegation-decision
;; What is the delegation status of entity ?entity?

[:find ?entity ?decision
 :in $ ?entity
 :where
 [?unc :uncertainty/entity ?entity]
 [?unc :uncertainty/scalar ?u-scalar]
 
 ;; Count agents that can resolve
 [(resolver-count ?entity) ?rc]
 
 ;; Decision logic:
 ;; rc > 0 AND u_scalar < 0.2  → :delegatable
 ;; rc > 0 AND u_scalar >= 0.2 → :contested  
 ;; rc = 0 AND u_scalar < 0.5  → :escalated
 ;; rc = 0 AND u_scalar >= 0.5 → :human-required
 [(delegation-classify ?rc ?u-scalar) ?decision]]
```

The threshold values (0.2, 0.5) are configurable system parameters stored as datoms.

---

## Stratum 4: Conflict Detection and Routing

### Structural Conflict Detection

```datalog
;; QUERY: detect-conflicts
;; Find all entity-attribute pairs with concurrent conflicting values.
;; This is the core conflict detection query.

;; Two datoms conflict iff:
;; 1. Same entity and attribute
;; 2. Different values
;; 3. Both currently live (in LIVE index)
;; 4. Attribute has cardinality :one
;; 5. Neither tx causally precedes the other (concurrent)

[:find ?entity ?attr ?val-a ?tx-a ?val-b ?tx-b
 :where
 [?entity ?attr ?val-a ?tx-a :assert]
 [?entity ?attr ?val-b ?tx-b :assert]
 [(!= ?val-a ?val-b)]
 [(!= ?tx-a ?tx-b)]
 
 ;; Cardinality check
 [?attr :db/cardinality :db.cardinality/one]
 
 ;; Concurrency check: neither is ancestor of the other
 (not (causal-ancestor ?tx-a ?tx-b))
 (not (causal-ancestor ?tx-b ?tx-a))
 
 ;; Both are still live (not retracted)
 ;; (Handled by querying against LIVE index)
 ]

;; MONOTONICITY: This query contains negation (not causal-ancestor).
;; However, causal-ancestor is MONOTONICALLY GROWING — adding new 
;; causal links can only ADD ancestry paths, which means the negation 
;; can only REMOVE pairs from the conflict set (a previously-concurrent 
;; pair might become causally ordered after learning about intermediate 
;; transactions). This means the conflict set is MONOTONICALLY 
;; NON-INCREASING with respect to causal information.
;;
;; In practice: an agent might detect a "false conflict" because it 
;; hasn't yet learned about the causal path between two transactions. 
;; After merging, the false conflict disappears. This is the 
;; conservative failure mode: over-detection of conflicts, never 
;; under-detection.
```

This monotonicity property is important enough to state as an invariant:

```
INV-CONFLICT-CONSERVATIVE-001: Conservative Conflict Detection
The set of detected conflicts at any local frontier MUST be a 
superset of the conflicts that would be detected at the global 
frontier (union of all agents' knowledge).

Formally: conflicts(frontier_local) ⊇ conflicts(frontier_global)

This holds because learning about new causal paths can only 
resolve apparent concurrency (turning concurrent pairs into 
causally-ordered pairs), never create new concurrency.

Consequence: An agent may waste effort resolving a conflict that 
doesn't actually exist (another agent's assertion was causally 
prior but the causal link hasn't propagated yet). This is safe — 
the resolution produces a redundant datom that is harmlessly 
deduplicated on merge. The system never MISSES a real conflict.
```

### Conflict Severity and Routing

```rust
/// Route a detected conflict to the appropriate resolution mechanism.
fn route_conflict(
    store: &Store,
    entity: EntityId,
    attr: AttrId,
    datom_a: DatomId,
    datom_b: DatomId,
) -> ResolutionRoute {
    // Step 1: Check if the attribute has a lattice
    if let Some(lattice) = store.lattice_for(attr) {
        let val_a = store.value_of(datom_a);
        let val_b = store.value_of(datom_b);
        
        if let Some(joined) = lattice.join(val_a, val_b) {
            // Lattice resolves it deterministically
            return ResolutionRoute::Automatic {
                method: ResolutionMethod::LatticeJoin,
                winner: joined,
                confidence: 1.0,
            };
        }
        // Values are incomparable in the lattice — fall through
    }
    
    // Step 2: Check if LWW resolves it
    let hlc_a = store.hlc_of(datom_a);
    let hlc_b = store.hlc_of(datom_b);
    if hlc_a != hlc_b {
        let winner = if hlc_a > hlc_b { datom_a } else { datom_b };
        // LWW resolves it, but with lower confidence
        return ResolutionRoute::Automatic {
            method: ResolutionMethod::LWW,
            winner: store.value_of(winner),
            confidence: 0.7, // LWW is deterministic but arbitrary
        };
    }
    
    // Step 3: Compute severity for escalation
    let w_a = store.commitment_weight(datom_a);
    let w_b = store.commitment_weight(datom_b);
    let severity = w_a.max(w_b) as f64;
    
    // Step 4: Find agents with authority to resolve
    let resolvers: Vec<AgentId> = store.query(
        "[:find ?agent
          :in $ ?entity
          :where (can-resolve ?agent ?entity)]",
        entity
    );
    
    if resolvers.is_empty() {
        return ResolutionRoute::Human {
            severity,
            reason: "No agent has sufficient authority".into(),
        };
    }
    
    // Step 5: Route based on severity thresholds
    let threshold_auto = store.system_param(":conflict/threshold-auto");
    let threshold_agent = store.system_param(":conflict/threshold-agent");
    
    if severity < threshold_auto {
        // Low severity: best-authority agent resolves automatically
        let best = resolvers.into_iter()
            .max_by_key(|a| store.authority(*a, entity))
            .unwrap();
        ResolutionRoute::AgentAutomatic {
            agent: best,
            severity,
        }
    } else if severity < threshold_agent {
        // Medium severity: agent resolves with human notification
        let best = resolvers.into_iter()
            .max_by_key(|a| store.authority(*a, entity))
            .unwrap();
        ResolutionRoute::AgentWithNotification {
            agent: best,
            severity,
            notify: store.human_agent(),
        }
    } else {
        // High severity: human decides
        ResolutionRoute::Human {
            severity,
            reason: format!(
                "Severity {} exceeds agent threshold {}", 
                severity, threshold_agent
            ),
        }
    }
}
```

---

## Stratum 5: DDIS Bilateral Loop Queries

These implement the specific DDIS operations over the datom store. They replace the current JSONL-fold-based implementations.

### Fitness Function

```datalog
;; QUERY: spec-fitness
;; Computes F(S) for a given spec entity.
;; Each component is a separate sub-query.

;; V: Validation score
[:find (/ (count ?passing) (count ?check))
 :in $ ?spec
 :where
 [?check :validation-check/spec ?spec]
 [?check :validation-check/status ?status]
 [(= ?status :passing) ?passing-bool]
 [(true? ?passing-bool)]
 ;; bind ?passing from ?check where status = :passing
 ]

;; C: Coverage score  
[:find (/ (count ?complete) (count ?element))
 :in $ ?spec
 :where
 [?element :inv/spec ?spec]
 ;; An invariant is "complete" if it has all 6 components
 [?element :inv/title _]
 [?element :inv/statement _]
 [?element :inv/semi-formal _]
 [?element :inv/violation _]
 [?element :inv/validation _]
 [?element :inv/why-this-matters _]]

;; D: Drift score
[:find (avg ?drift-score)
 :in $ ?spec
 :where
 [?drift :drift/spec-element ?elem]
 [?elem :inv/spec ?spec]
 [?drift :drift/score ?drift-score]]

;; H: Challenge health
[:find (/ (count ?confirmed) (count ?witness))
 :in $ ?spec
 :where
 [?witness :witness/invariant ?inv]
 [?inv :inv/spec ?spec]
 [?witness :witness/status :valid]
 [?challenge :challenge/witness ?witness]
 [?challenge :challenge/verdict :confirmed]]

;; K: Contradiction density
[:find (count ?contradiction)
 :in $ ?spec
 :where
 [?contradiction :contradiction/element-a ?a]
 [?a :inv/spec ?spec]
 [?contradiction :contradiction/status ?status]
 [(!= ?status :resolved)]]

;; U: Mean uncertainty (NEW — extends the fitness function)
[:find (avg ?u-scalar)
 :in $ ?spec
 :where
 [?entity _ _ _]
 [?entity :inv/spec ?spec]  ;; or more broadly, any entity in this spec
 [?unc :uncertainty/entity ?entity]
 [?unc :uncertainty/scalar ?u-scalar]]
```

The combined fitness function, now with seven terms:

```
F(S) = 0.18×V + 0.18×C + 0.18×(1-D) + 0.13×H + 0.13×(1-K) + 0.08×(1-I) + 0.12×(1-U)

where U = mean uncertainty across all spec entities
```

The weights are redistributed from the original 6-signal function to accommodate the uncertainty term. The uncertainty signal gets weight 0.12, reflecting its importance as a coordination-layer metric. The other weights are slightly reduced proportionally.

### Crystallization Readiness

This is the query that answers "is this discovery finding ready to become a spec element?"

```datalog
;; QUERY: crystallization-candidates
;; Findings whose stability exceeds the crystallization threshold
;; AND whose parent thread has sufficient confidence.

[:find ?finding ?thread ?stability
 :where
 [?finding :finding/status :refined]           ;; must be refined, not raw
 [?finding :finding/thread ?thread]
 [?thread :thread/status :active]
 
 ;; Stability check
 [?stab :stability/entity ?finding]
 [?stab :stability/score ?stability]
 [(>= ?stability 0.7)]                         ;; configurable threshold
 
 ;; Confidence check on parent thread
 [?thread :thread/confidence ?conf]
 [?conf :confidence/depth ?depth]
 [?conf :confidence/coherence ?coherence]
 [(>= ?depth 0.6)]
 [(>= ?coherence 0.6)]
 
 ;; No unresolved conflicts on this finding
 (not [?conflict :conflict/entity ?finding]
      [?conflict :conflict/status ?cs]
      [(!= ?cs :resolved)])]
```

This query has a `not` clause (no unresolved conflicts), making it non-monotonic. An agent that hasn't yet learned about a conflict on this finding might incorrectly conclude it's ready to crystallize. This is the *dangerous* failure mode — premature crystallization.

The defense is the `:stability-min` guard:

```datalog
;; SAFE VARIANT: only consider findings whose contributing facts 
;; all have commitment weight ≥ threshold

[:find ?finding ?thread ?stability
 :stability-min 3                              ;; at least 3 dependents
 :where
 ... same as above ...]
```

The `:stability-min 3` filter ensures that only findings built on well-established facts are considered. A finding whose supporting facts have few dependents is too provisional to crystallize even if it appears conflict-free.

### Delegation Routing for Implementation Tasks

```datalog
;; QUERY: ready-tasks
;; Tasks that are ready for delegation: all dependencies satisfied,
;; target spec element is delegatable, and an agent with authority exists.

[:find ?task ?spec-element ?agent ?authority-score
 :where
 [?task :task/status :pending]
 [?task :task/spec-element ?spec-element]
 
 ;; All dependencies are done
 (not [?task :task/depends-on ?dep]
      [?dep :task/status ?dep-status]
      [(!= ?dep-status :done)])
 
 ;; Spec element is delegatable
 [?deleg :delegation/entity ?spec-element]
 [?deleg :delegation/decision :delegatable]
 
 ;; Find the best agent
 [?auth :authority/entity ?spec-element]
 [?auth :authority/agent ?agent]
 [?auth :authority/score ?authority-score]
 [?agent :agent/status :active]]

;; ORDER BY ?authority-score DESC — best agent first
```

### The Backward Flow: Absorption Detection

```datalog
;; QUERY: drift-candidates
;; Spec elements whose implementation annotations suggest divergence.
;; This is the trigger for the backward flow.

[:find ?inv ?annotation ?verb
 :where
 [?annotation :annotation/target ?inv]
 [?annotation :annotation/verb ?verb]
 [?annotation :annotation/orphaned false]
 
 ;; The annotation's source file has been modified more recently
 ;; than the invariant's last update
 [?annotation :annotation/file ?file]
 [?annotation _ _ ?ann-tx]
 [?inv :inv/content-hash _ ?inv-tx]
 
 ;; ann-tx is causally after inv-tx
 (causal-ancestor ?inv-tx ?ann-tx)
 
 ;; No absorption has been recorded since the annotation
 (not [?abs :absorption/target-element ?inv]
      [?abs _ _ ?abs-tx]
      (causal-ancestor ?ann-tx ?abs-tx))]
```

---

## Query Pattern Index

For reference, here's the complete set of query patterns we've defined, classified by stratum and monotonicity:

| Stratum | Query | Monotonic | Sync Barrier | Purpose |
|---|---|---|---|---|
| 0 | current-value | ✅ (via LIVE index) | No | Resolve current attribute value |
| 1 | causal-ancestor | ✅ | No | Transitive causal ordering |
| 1 | causal-descendant | ✅ | No | Forward causal cone |
| 1 | entity-depends-on | ✅ | No | Entity dependency graph |
| 1 | reachable | ✅ | No | Cross-reference reachability |
| 2 | epistemic-uncertainty | ⚠️ eventually | No | σ_e computation |
| 2 | aleatory-uncertainty | ❌ | No (conservative) | σ_a computation |
| 2 | consequential-uncertainty | ⚠️ mixed | No (conservative) | σ_c computation |
| 3 | contribution-edges | ✅ | No | Agent-entity contribution graph |
| 3 | spectral-authority | N/A (linear algebra) | No | Authority scores |
| 3 | delegation-threshold | ✅ | No | Per-entity threshold |
| 3 | can-resolve | ✅ | No | Authority ≥ threshold check |
| 3 | delegation-decision | ⚠️ aggregation | No | Delegation classification |
| 4 | detect-conflicts | ⚠️ conservative | No | Find concurrent contradictions |
| 4 | route-conflict | N/A (imperative) | No | Severity → resolution path |
| 5 | spec-fitness | ❌ aggregation | Optional | F(S) computation |
| 5 | crystallization-candidates | ❌ negation | Recommended | Ready-to-crystallize findings |
| 5 | ready-tasks | ❌ negation | Recommended | Delegatable implementation tasks |
| 5 | drift-candidates | ❌ negation | Recommended | Backward-flow triggers |

The pattern is clear: Strata 0–3 are fully or conservatively monotonic, safe to run at any frontier without coordination. Strata 4–5 contain non-monotonic queries that benefit from sync barriers for correctness-critical decisions (crystallization, delegation, fitness measurement) but can run uncoordinated for advisory purposes (showing the human what *might* be ready).

This maps to exactly the 4B/4C hybrid you chose in Question 4: monotonic queries run freely, non-monotonic queries are stability-gated for critical decisions.

---

## One Structural Observation

Looking at the full query set, a pattern emerges that I didn't anticipate. The queries naturally partition into two categories that correspond to the two directions of the bilateral loop:

**Forward-flow queries** (human → spec → implementation): epistemic uncertainty, crystallization candidates, delegation decisions, ready tasks. These are *planning* queries — "what should we do next?"

**Backward-flow queries** (implementation → spec → human): conflict detection, drift candidates, aleatory uncertainty, absorption triggers. These are *assessment* queries — "what went wrong or changed?"

The commitment function w(d) and consequential uncertainty σ_c sit at the intersection — they're used by both flows. The spectral authority computation is the bridge: it's updated by backward-flow observations (agent contributions) and consumed by forward-flow decisions (delegation).

This bilateral structure in the query layer mirrors the bilateral structure in the DDIS workflow. It's not something I designed in — it fell out of the formalization. That's usually a sign the model is capturing real structure.

---

## Next Step

With the schema (Layer 0–5) and query patterns (Strata 0–5) defined, we have the full formal core of the system. The next step is to either:

**A. Draft the DDIS manifest and constitution for this spec.** Map everything we've defined into DDIS-conforming sections with invariants, ADRs, and quality gates. This would be the actual spec document that a DDIS tool would validate.

**B. Define the storage engine interface.** The Rust trait boundaries for the datom store, index layer, and query engine — the API surface that the implementation would code against.

**C. Define the agent protocol.** How agents actually interact with the store — the transaction format, the merge protocol, the sync barrier mechanism, and the frontier exchange format.

My instinct is C — the agent protocol is the missing piece between "the store exists and has queries" and "agents actually use it to coordinate." But your call.

---

## Willem

Yes I'd like to define C. With that in mind, I've attached a related conversation I had with another agent about a formal analysis of agentic systems which touches on this while also suggesting more broadly generalizable formalizations for agentic systems, coding or otherwise. Read it in its ENTIRETY and then tell me what you think about our plan given this new context.

---

## Claude

I've read the entire document. This is a remarkably convergent piece of work — you've been building toward the same architecture from two independent directions, and the places where the two lines of reasoning *independently agree* are the strongest signal that we're on the right track. But there are also places where this document reveals gaps in our plan, and places where our plan addresses gaps the document identifies but doesn't resolve. Let me lay it out.

---

## Where the Two Converge (Independent Validation)

The document arrives at **datom-native fact stores replacing JSONL** from a completely different starting point than ours. We got there by asking "what's the right CRDT for multi-agent coordination?" The other conversation got there by asking "what's the optimal epistemic substrate for a single agent?" Both arrive at the same answer: append-only EAV with Datalog queries and schema-on-read. When two independent derivations converge on the same structure, it's usually because the structure is capturing something real.

The document's tripartite decomposition **(E, R, A)** — event log, runtime, agent — maps exactly onto our architecture:

- **E** = the datom store (our G-Set CRDT over D = E × A × V × T × Op)
- **R** = the runtime environment (filesystem, shell, tools)  
- **A** = the LLM agent with tool-calling (the policy function π)

And the document's formal specification `AgentSystem = (E, Op, Exo, Obs, S, π, δ, φ)` is *precisely* the single-agent specialization of our multi-agent system. Our protocol (part C) is what you get when you generalize this to N agents sharing the same E, each with their own local frontier, and needing to coordinate.

The `assert` mechanism the document proposes — agents writing their own edges into the knowledge graph — is exactly what our transaction protocol needs to support. And the `associate` tool — schema discovery before precise query — maps to a specific query pattern we should formalize in the protocol.

---

## Where the Document Reveals Gaps in Our Plan

### Gap 1: The Dual-Process Architecture Should Be Protocol-Level, Not Application-Level

The document's Kahneman mapping (System 1 = lightweight associative retrieval, System 2 = heavyweight LLM reasoning) is not just an analogy — it's an architectural requirement. The critical insight:

> System 2 doesn't know what System 1 didn't surface. A brilliant reasoner with a bad retrieval policy is a brilliant reasoner that's always solving the wrong problem.

Our protocol currently defines how agents *transact* with the store (assert, retract, merge) and how they *query* it (the Datalog patterns we defined). But we haven't formalized the **context assembly** step — how an agent goes from "I have access to a store with millions of datoms" to "I have a context window with the right 8,000 tokens loaded." The document shows this is *the* critical operation. If we leave it to each agent implementation to figure out, we'll get the same flat-buffer pathology the document identifies.

**What this means for the protocol:** We need a first-class `assemble` operation — or more precisely, the two-phase pattern the document describes:

```
Phase 1: associate(semantic_context, depth, breadth) → schema neighborhood
Phase 2: query(datalog_expression) → concrete datoms
```

This is the functor between the semantic category and the structural category that the document formalizes. It should be a protocol-level operation, not something each agent reinvents.

### Gap 2: The Confusion Feedback Channel

The document identifies a critical missing primitive: System 2 signaling confusion back to System 1 for re-retrieval. In our architecture, this maps directly to the uncertainty tensor — but the mapping is more operational than I initially framed it.

When an agent detects high epistemic uncertainty (σ_e) during its reasoning, that's a confusion signal. The document's `Confusion` type:

```
data Confusion
  = NeedMore Cue         -- σ_e is high: need more information
  | Conflict [Datom]     -- σ_a is high: found contradictions
  | GoalUnclear          -- lost the thread: need invariants back in context
```

This is our uncertainty tensor expressed as an actionable protocol message rather than a stored measurement. The protocol should support agents emitting confusion signals that trigger re-assembly of their context from the store.

### Gap 3: The Exogenous Event / Frame Problem Is Our Merge Problem

The document formalizes exogenous events — state changes to R that don't appear in E — as the source of belief-state divergence. In a multi-agent system, **every other agent's assertions are exogenous events from the perspective of any given agent.** Agent α doesn't know about agent β's transactions until they merge.

This means our merge protocol isn't just a CRDT operation (set union) — it's also an **epistemic update.** When agent α merges agent β's datoms, α's belief state about the world changes. Some of α's cached query results become stale. Some conflicts that α thought were resolved might be reopened by β's assertions.

The protocol needs to handle the post-merge invalidation cascade:

```
merge(local_frontier, remote_datoms) → {
  new_datoms: [Datom],           -- the actual new facts
  invalidated_queries: [QueryId], -- cached results that are now stale
  new_conflicts: [Conflict],      -- freshly detected conflicts
  uncertainty_deltas: [(Entity, ΔU)]  -- uncertainty changes
}
```

### Gap 4: The Compaction / Context Budget Problem

The document's most important practical insight is that context assembly is a **rate-distortion problem** — given bounded context window w, maximize decision quality by selecting the optimal subset of available information. This is formalized as:

```
σ* = argmax_σ E[Quality(π(σ(I)))] subject to |σ(I)| ≤ w
```

Our protocol defines queries but doesn't address the **budget constraint.** A query might return 50,000 datoms when the agent can only process 8,000 tokens worth. The protocol needs to support budget-aware assembly — which is exactly what the DDIS shard/bead architecture was designed for, now generalized to datoms instead of markdown sections.

---

## Where Our Plan Addresses Gaps the Document Identifies But Doesn't Resolve

This is equally important. The document identifies several open problems that our formal core *already solves.*

### Solved Gap: Multi-Agent Coordination

The document is entirely single-agent. It asks "how does one agent manage its epistemic state optimally?" but never asks "what happens when two agents share a fact store and disagree?" Our uncertainty tensor (σ_e, σ_a, σ_c), spectral authority, delegation thresholds, and conflict resolution protocol are the answer to the multi-agent generalization that the document doesn't attempt.

### Solved Gap: Who Resolves Contradictions?

The document mentions that the agent might surface conflicting facts but doesn't formalize how conflicts are detected or resolved. Our conflict detection query (Stratum 4) with the conservative detection invariant (INV-CONFLICT-CONSERVATIVE-001), severity-based routing, and the lattice resolution modes are exactly the machinery this requires.

### Solved Gap: The Self-Correction Problem

The document proposes self-authored associations (agents writing their own edges) but flags the quality control problem — what stops an agent from writing bad associations that degrade the graph? Our answer: the challenge/witness system plus spectral authority. An agent's assertions are weighted by its authority score, which is earned through verified contributions. Bad associations get challenged and retracted. The graph self-corrects through the bilateral loop.

### Solved Gap: When to Assert vs. When to Stay Quiet

The document poses this as an open design question — eager assertion, confusion-triggered assertion, or periodic reflection. Our framework gives a principled answer: **assert when the reversibility test passes AND the commitment function suggests stability.** If a fact has low w(d) (few dependents), it's cheap to assert and retract later. If a fact would have high w(d) immediately (many things depend on it), require higher confidence before assertion. The commitment function provides the natural graduated assertion policy.

---

## The Synthesis: What This Means for the Agent Protocol

Given both lines of reasoning, the agent protocol (part C) should be designed with these principles:

**Principle 1: The protocol is the interface between A and E in the universal agent signature.** Any system conforming to the (E, R, A) decomposition should be able to participate. This means the protocol is more general than DDIS — it's the coordination layer for *any* agent swarm sharing a datom store.

**Principle 2: Context assembly is a protocol-level concern, not an application concern.** The two-phase retrieval pattern (associate → query) should be a first-class protocol operation. Budget-aware assembly (the rate-distortion optimization) should be parameterizable.

**Principle 3: The protocol must support both the forward and backward channels of the dual-process loop.** Forward: store → agent (query, assemble). Backward: agent → store (assert, retract, signal-confusion). The confusion channel is not optional — it's how agents request better context.

**Principle 4: Merge is an epistemic event, not just a set operation.** The protocol must propagate the consequences of merge (invalidated caches, new conflicts, uncertainty changes) as first-class signals.

**Principle 5: The protocol should be topology-agnostic.** The document's single-agent loop, our bilateral loop, a flat swarm, a hierarchy — all are valid topologies. The protocol defines the operations; the topology emerges from how agents use them.

---

## Revised Protocol Outline

Given all of this, here's what I think the protocol needs to cover:

```
PROTOCOL OPERATIONS:

1. TRANSACT — assert/retract datoms
   Input: [Datom], agent_id, causal_parents, rationale
   Output: tx_entity, new_frontier
   Constraint: append-only, monotonic growth

2. QUERY — read datoms via Datalog
   Input: datalog_expression, frontier (optional), stability_min (optional)
   Output: result_set, stability_score
   Modes: monotonic (uncoordinated), non-monotonic (barrier-gated)

3. ASSOCIATE — schema discovery (System 1 Phase 1)
   Input: semantic_cue OR explicit_seeds, depth, breadth
   Output: schema_neighborhood (entities, attributes, types, not values)
   Cost: bounded by depth × breadth, always cheap

4. ASSEMBLE — budget-aware context construction (System 1 Phase 2)
   Input: query_results, budget_tokens, priority_weights
   Output: assembled_context ≤ budget
   Strategy: rate-distortion optimal selection

5. MERGE — receive remote datoms
   Input: remote_datom_set, remote_frontier
   Output: new_datoms, invalidated_queries, new_conflicts, uncertainty_deltas
   Semantics: G-Set union with post-merge cascade

6. SYNC-BARRIER — request consistent cut
   Input: set of agent_ids to synchronize with
   Output: consistent_frontier across all participants
   Constraint: blocks until all agents have exchanged frontiers

7. SIGNAL — emit coordination signal
   Types: Confusion(cue), Conflict(datoms), UncertaintySpike(entity, ΔU),
          ResolutionProposal(conflict, proposed_winner, reasoning),
          DelegationRequest(entity, task)
   Routing: to coordinator, to specific agent, or broadcast

8. FRONTIER-EXCHANGE — share causal state
   Input: local_frontier (set of latest-known tx refs per peer)
   Output: remote_frontier
   Protocol: bidirectional, pairwise or gossip
```

The document's formalization tells us that operations 3 and 4 (ASSOCIATE and ASSEMBLE) are just as fundamental as 1 and 2 (TRANSACT and QUERY). Current agent systems get operations 1 and 2 right (they can write to and read from their logs) but operations 3 and 4 are vestigial or absent — which is why agents plateau at moderate task complexity regardless of LLM capability. The memory architecture, not the reasoning capability, is the bottleneck.

And the document's insight about self-authored associations tells us that operation 1 (TRANSACT) isn't just for recording observations — it's the mechanism by which agents build expertise. Every high-quality assertion an agent makes enriches the graph that ASSOCIATE traverses, creating the flywheel the document describes: more experience → richer graph → better retrieval → better performance → richer experience.

---

## My Overall Assessment

The two bodies of work are not just compatible — they're complementary in a way that's almost too clean. The document provides the *single-agent cognitive architecture* (the dual-process loop, the context assembly problem, the rate-distortion formulation). Our work provides the *multi-agent coordination layer* (the uncertainty tensor, spectral authority, CRDT merge semantics, bilateral loop). Neither is complete without the other.

The agent protocol should be the bridge. It should be general enough to support a single agent using the fact store as its personal episodic memory (the document's use case), AND a swarm of agents using it as a shared coordination substrate (our use case). The operations above cover both.

Shall I proceed to formalize the protocol operations with full type signatures, invariants, and the wire format for inter-agent communication?

---
