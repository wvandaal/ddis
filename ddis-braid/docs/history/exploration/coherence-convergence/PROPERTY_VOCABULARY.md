# Coherence Engine Property Vocabulary v2 — Expanded (109 Properties)

> Closed ontology for LLM-mediated extraction of logical forms from Braid spec elements.
> Properties are the atomic predicates. Incompatibilities and entailments are the inference rules.
> Together they enable automated coherence checking without Prolog.

---

## Properties by Category

### STORAGE (8)
| Property | Meaning |
|----------|---------|
| `append_only` | Store never deletes or mutates existing datoms |
| `grow_only` | Datom count monotonically non-decreasing |
| `content_addressable` | Identity by [e,a,v,tx,op] content, not sequential ID |
| `immutable_datoms` | Five-tuple frozen once asserted |
| `retraction_as_assertion` | Retractions are new datoms with op=retract |
| `set_union_merge` | Merge is mathematical set union (C4) |
| `datom_five_tuple` | Atomic fact is [entity, attribute, value, tx, op] |
| `monotonic_store_growth` | \|S(t+1)\| >= \|S(t)\| always |

### CONCURRENCY (7)
| Property | Meaning |
|----------|---------|
| `coordination_free` | Monotonic operations need no synchronization |
| `frontier_relative` | Queries relative to agent's known frontier |
| `crdt_mergeable` | Store forms join-semilattice under set union |
| `monotonic_computation` | CALM-safe operations that need no barrier |
| `causal_ordering` | Happens-before relation via tx predecessors |
| `at_least_once_delivery` | No message loss; dedup at receiver |
| `local_first` | Default to local frontier, no remote required |

### QUERY (8)
| Property | Meaning |
|----------|---------|
| `bounded_query_time` | Queries terminate in bounded time |
| `deterministic_query` | Same inputs produce same results |
| `terminating_evaluation` | Datalog evaluation always terminates |
| `calm_compliant` | Monotonic queries need no coordination (CALM theorem) |
| `stratified_negation` | Negation only in higher strata |
| `pure_function` | FFI extensions are pure (no side effects) |
| `datalog_primary` | Datalog is the query language |
| `stratum_classified` | Queries classified by safety stratum (S0-S3) |

### SCHEMA (6)
| Property | Meaning |
|----------|---------|
| `schema_as_data` | Schema defined as datoms in store |
| `schema_evolution_as_transaction` | Schema changes are transactions |
| `self_describing` | Schema entities describe themselves |
| `attribute_typed` | Every attribute has a declared value type |
| `cardinality_enforced` | :one vs :many enforced at transact time |
| `unique_identity_attrs` | :db.unique/identity creates lookup refs |

### RESOLUTION (7)
| Property | Meaning |
|----------|---------|
| `per_attribute_resolution` | Resolution mode declared per attribute |
| `lattice_resolved` | Values form join-semilattice |
| `last_writer_wins` | HLC timestamp ordering for conflicts |
| `multi_value` | Power set union — all values retained |
| `resolution_at_query_time` | Conflicts resolved on read, not during merge |
| `severity_tiered_routing` | Three-tier conflict routing (auto/agent/human) |
| `conflict_requires_causal_independence` | Only concurrent assertions create conflicts |

### MERGE (8)
| Property | Meaning |
|----------|---------|
| `merge_commutative` | MERGE(S1, S2) = MERGE(S2, S1) |
| `merge_associative` | MERGE(MERGE(S1, S2), S3) = MERGE(S1, MERGE(S2, S3)) |
| `merge_idempotent` | MERGE(S, S) = S |
| `merge_monotonic` | S ⊆ MERGE(S, S') |
| `branch_isolation` | Branches invisible to each other |
| `working_set_private` | W_alpha invisible to other agents |
| `competing_branch_lock` | Competing branches require comparison before commit |
| `merge_cascade` | Merge triggers conflict/cache/uncertainty/subscription cascade |

### HARVEST (6)
| Property | Meaning |
|----------|---------|
| `harvest_monotonic` | Harvest only adds, never removes |
| `harvest_captures_untransacted` | Extracts knowledge not yet in store |
| `harvest_agent_mediated` | Agent selects what to crystallize |
| `harvest_preserves_provenance` | Provenance typed: observed/derived/inferred |
| `harvest_boundary_detection` | Detects session ending conditions |
| `harvest_idempotent` | Re-harvest produces no new datoms if nothing changed |

### SEED (6)
| Property | Meaning |
|----------|---------|
| `seed_is_projection` | Seed is a query over existing store |
| `seed_query_assembled` | Assembled via Datalog query |
| `seed_relevance_ranked` | Entities ranked by relevance to task |
| `seed_budget_constrained` | Size limited by attention budget |
| `seed_methodology_included` | Always includes methodology pointers |
| `seed_no_fabrication` | Never includes facts not in store |

### SYNC (5)
| Property | Meaning |
|----------|---------|
| `consistent_cut` | Barrier establishes shared reference point |
| `sync_barrier_blocking` | Participants block until all report |
| `post_barrier_deterministic` | Non-monotonic queries deterministic after barrier |
| `sync_topology_independent` | Result independent of network topology |
| `sync_timeout_safe` | Timeout produces partial result, not error |

### SIGNAL (7)
| Property | Meaning |
|----------|---------|
| `signal_as_datom` | Every signal recorded as datom in store |
| `dispatch_total` | Every signal type has a defined dispatch target |
| `severity_monotonic_cost` | Higher severity routes to more expensive resolution |
| `signal_subscription_persistent` | Subscriptions persist until explicitly removed |
| `signal_auditable` | Signal history queryable via store |
| `signal_typed` | Eight signal types covering all divergence classes |
| `signal_triggers_resolution` | Signals automatically route to resolution mechanisms |

### BILATERAL (6)
| Property | Meaning |
|----------|---------|
| `bilateral_symmetry` | Every forward operation has an inverse |
| `five_point_coherence` | Five-check convergence criterion |
| `fitness_monotonic` | F(S) >= F(S_prev) over operations |
| `bilateral_lifecycle_stages` | Lifecycle: discovered -> refined -> specified -> ... |
| `bilateral_inverse_operations` | discover<->absorb, refine<->drift |
| `fitness_seven_dimensions` | F(S) measures 7 dimensions of spec quality |

### DELIBERATION (6)
| Property | Meaning |
|----------|---------|
| `deliberation_converges` | Deliberation always terminates |
| `stability_guard_required` | Proposal must be stable before commit |
| `precedent_queryable` | Past decisions queryable for consistency |
| `no_backward_lifecycle` | Lifecycle status only moves forward |
| `deliberation_fuel_bounded` | Evaluation limited by fuel counter |
| `position_as_datom` | Each position is a datom with provenance |

### GUIDANCE (7)
| Property | Meaning |
|----------|---------|
| `guidance_per_response` | Every tool response includes guidance footer |
| `guidance_budget_aware` | Footer size scales with k*_eff |
| `dynamic_claude_md` | CLAUDE.md generated from store state |
| `guidance_methodology_first` | Guidance uses spec language, not checklists |
| `guidance_anti_drift` | Maintains Basin A probability above threshold |
| `guidance_learned` | Learned patterns are effectiveness-tracked |
| `guidance_six_mechanisms` | Six anti-drift mechanisms (preemption, injection, detection, gate, alarm, harvest) |

### BUDGET (6)
| Property | Meaning |
|----------|---------|
| `budget_monotonic_decreasing` | k*_eff never increases within session |
| `precedence_five_level` | System > Methodology > UserRequested > Speculative > Ambient |
| `graceful_degradation` | Output quality degrades smoothly with budget |
| `projection_pyramid_four_level` | Four output projection tiers (pi0/pi1/pi2/pi3) |
| `minimum_output_guaranteed` | Even at zero budget, harvest signal emitted |
| `budget_hard_cap` | Output never exceeds 5% of remaining capacity |

### INTERFACE (6)
| Property | Meaning |
|----------|---------|
| `five_layer_graded` | Five interface layers (ambient/CLI/MCP/guidance/TUI) |
| `store_sole_truth` | All layers read/write same store, no layer-local state |
| `cli_primary_agent_interface` | CLI is the primary agent interaction layer |
| `mcp_six_tools` | MCP exposes exactly six tools |
| `tui_human_only` | TUI not constrained by attention budget |
| `statusline_zero_cost` | Statusline has zero agent context cost |

### SAFETY (7)
| Property | Meaning |
|----------|---------|
| `no_data_loss` | No datoms ever lost |
| `no_fabrication` | No facts asserted that weren't observed/derived |
| `no_budget_overflow` | Tool responses respect budget cap |
| `no_premature_crystallization` | Uncertain claims stay uncertain |
| `no_branch_leak` | Branch datoms invisible to non-branch agents |
| `no_silent_failure` | All errors recorded as datoms |
| `no_orphan_entities` | Every entity traces to spec element or transaction |

### SELF_BOOTSTRAP (3)
| Property | Meaning |
|----------|---------|
| `self_specifying` | DDIS specifies itself |
| `spec_elements_as_datoms` | Spec elements are first data managed |
| `self_referential_coherence` | System checks its own spec for contradictions |

---

## Incompatibilities (12)

These property pairs are mutually exclusive. An element committing to both is a contradiction.

| # | Property A | Property B | Reason |
|---|-----------|-----------|--------|
| I1 | `append_only` | `mutable_datoms` | Append-only means no mutation |
| I2 | `coordination_free` | `total_ordering` | Total ordering requires coordination |
| I3 | `grow_only` | `compaction` | Compaction removes datoms |
| I4 | `content_addressable` | `sequential_ids` | Content-addressed means no sequential IDs |
| I5 | `no_backward_lifecycle` | `lifecycle_rollback` | Forward-only lifecycle |
| I6 | `resolution_at_query_time` | `resolution_at_merge_time` | Resolution happens in one place |
| I7 | `local_first` | `global_consistency_required` | Local-first means no global requirement |
| I8 | `calm_compliant` | `requires_barrier_for_reads` | CALM means no barrier for monotonic reads |
| I9 | `sync_barrier_blocking` | `always_nonblocking` | Barriers block by definition |
| I10 | `working_set_private` | `working_set_shared` | W_alpha is private or not |
| I11 | `branch_isolation` | `branch_visibility` | Branches are isolated or visible |
| I12 | `harvest_monotonic` | `harvest_deletes` | Harvest only adds |

## Entailments (16)

If an element commits to property A, it logically entails property B.

| # | If (A) | Then (B) | Reason |
|---|--------|----------|--------|
| E1 | `append_only` | `grow_only` | Append-only implies growth |
| E2 | `append_only` | `immutable_datoms` | Append-only implies immutability |
| E3 | `append_only` | `retraction_as_assertion` | No delete means retract-as-assert |
| E4 | `content_addressable` | `deterministic_query` | Content addressing ensures determinism |
| E5 | `calm_compliant` | `coordination_free` | CALM theorem |
| E6 | `schema_as_data` | `schema_evolution_as_transaction` | Schema-as-data means schema changes are txs |
| E7 | `seed_is_projection` | `seed_no_fabrication` | Projection cannot fabricate |
| E8 | `stability_guard_required` | `no_premature_crystallization` | Stability guard prevents premature crystallization |
| E9 | `set_union_merge` | `merge_commutative` | Set union is commutative |
| E10 | `set_union_merge` | `merge_associative` | Set union is associative |
| E11 | `set_union_merge` | `merge_idempotent` | Set union is idempotent |
| E12 | `set_union_merge` | `merge_monotonic` | Set union is monotonic |
| E13 | `signal_as_datom` | `signal_auditable` | Datoms are queryable |
| E14 | `crdt_mergeable` | `merge_commutative` | CRDTs are commutative |
| E15 | `budget_monotonic_decreasing` | `graceful_degradation` | Monotonic decrease implies degradation |
| E16 | `self_specifying` | `spec_elements_as_datoms` | Self-specifying requires spec-as-data |

---

## Extraction Schema

### For INVs (Invariants):
```json
{
  "id": "INV-NAMESPACE-NNN",
  "type": "invariant",
  "properties_committed": ["property1", "property2"],
  "properties_assumed": ["property3"],
  "violation_condition": "This is violated if...",
  "dependencies": ["INV-OTHER-NNN"],
  "assumptions": ["description of assumption"],
  "confidence": 1.0,
  "stage": 0
}
```

### For ADRs (Architecture Decision Records):
```json
{
  "id": "ADR-NAMESPACE-NNN",
  "type": "adr",
  "commitments": ["property1"],
  "assumptions": ["property2"],
  "exclusions": ["anti_property"],
  "dependencies": ["INV-OTHER-NNN"],
  "confidence": 1.0,
  "stage": 0
}
```

### For NEGs (Negative Cases):
```json
{
  "id": "NEG-NAMESPACE-NNN",
  "type": "negative_case",
  "prohibited_properties": ["anti_property"],
  "safety_properties": ["property1"],
  "related_invariants": ["INV-OTHER-NNN"],
  "confidence": 1.0,
  "stage": 0
}
```

### For UNCs (Uncertainty Markers):
```json
{
  "id": "UNC-NAMESPACE-NNN",
  "type": "uncertainty",
  "provisional_claim": "description",
  "confidence": 0.5,
  "impact_if_wrong": "description",
  "properties_affected": ["property1"],
  "dependencies_affected": ["INV-OTHER-NNN"]
}
```
