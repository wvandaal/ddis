# Phantom Type Audit — Stage 0

> **Beads**: R4.1a, R4.2a, R4.3a
> **Date**: 2026-03-03
> **Method**: Systematic cross-reference of docs/guide/00-architecture.md Type Catalog (section 0.2),
> all docs/guide/ implementation plans (01-store through 09-interface, 10-12), and all spec/
> Level 2 sections (01-store through 14-interface, 15-17).

---

## Section 1: Phantom Types (in Type Catalog but not in spec/)

These types appear in the docs/guide/00-architecture.md section 0.2 Type Catalog but have no
formal Level 2 definition in any spec/ file. For each: namespace, stage needed, file
locations, and recommendation.

### 1.1 TxReport

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Type Catalog location** | docs/guide/00-architecture.md section 0.2 (Cross-Namespace Types) |
| **Guide location** | docs/guide/00-architecture.md lines 409-413, docs/guide/01-store.md (Transaction::receipt) |
| **Spec location** | None. spec/01-store.md defines `Transaction<Applied>` with `receipt()` method (line 360) but no `TxReport` struct. |
| **Recommendation** | **Formalize in spec.** TxReport is the return value of `Transaction<Applied>::receipt()` and is essential for every transact operation. Add to spec/01-store.md Level 2 section. |

### 1.2 TxValidationError

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Type Catalog location** | docs/guide/00-architecture.md section 0.2 (Cross-Namespace Types) |
| **Guide location** | docs/guide/00-architecture.md lines 415-419, docs/guide/01-store.md, docs/guide/02-schema.md |
| **Spec location** | Referenced in spec/01-store.md as return type of `commit()` (line 350) and spec/02-schema.md (line 313), but the enum variants are never formally defined in any Level 2 section. |
| **Recommendation** | **Formalize in spec.** This error type gates transaction correctness. The spec uses it as a return type but never defines its variants. Add full enum definition to spec/01-store.md Level 2. |

### 1.3 SchemaError

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Type Catalog location** | docs/guide/00-architecture.md section 0.2 (Cross-Namespace Types) |
| **Guide location** | docs/guide/00-architecture.md line 423, docs/guide/02-schema.md (SchemaValidationError) |
| **Spec location** | Referenced as return type of `Schema::validate_value()` in spec/02-schema.md line 170, but the enum variants are never formally defined. |
| **Recommendation** | **Formalize in spec.** Schema validation errors are part of the correctness contract (INV-SCHEMA-005). Define variants in spec/02-schema.md Level 2. |

### 1.4 QueryStats

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Type Catalog location** | docs/guide/00-architecture.md section 0.2 (Cross-Namespace Types) |
| **Guide location** | docs/guide/00-architecture.md line 432 |
| **Spec location** | None. spec/03-query.md defines QueryResult (line 183) but not QueryStats. |
| **Recommendation** | **Keep guide-only.** QueryStats (`datoms_scanned`, `bindings_produced`) is an observability aid, not a correctness contract. No invariant depends on it. |

### 1.5 BindingSet

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Type Catalog location** | docs/guide/00-architecture.md section 0.2 (Cross-Namespace Types) |
| **Guide location** | docs/guide/00-architecture.md line 433, docs/guide/03-query.md |
| **Spec location** | Referenced implicitly in spec/03-query.md QueryResult (line 183: `pub bindings: Vec<HashMap<Variable, Value>>`) but not as a named type alias. |
| **Recommendation** | **Keep guide-only.** This is a convenience type alias (`HashMap<Variable, Value>`). The spec uses the expanded form inline, which is equivalent. No formalization needed. |

### 1.6 FrontierRef

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Type Catalog location** | docs/guide/00-architecture.md section 0.2 (Cross-Namespace Types, line 435) |
| **Guide location** | docs/guide/00-architecture.md line 435 |
| **Spec location** | None. spec/03-query.md defines Clause variants (lines 164-175) that include frontier scoping but does not name a FrontierRef type. |
| **Recommendation** | **Keep guide-only.** FrontierRef is an implementation detail for clause operands. The spec covers frontier-scoped queries via QueryMode::Stratified(Frontier) which is sufficient. |

### 1.7 GraphError

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Type Catalog location** | docs/guide/00-architecture.md section 0.2 (Graph Engine Types, lines 356-360) |
| **Guide location** | docs/guide/00-architecture.md lines 356-360, docs/guide/03-query.md |
| **Spec location** | None. spec/03-query.md defines graph algorithm functions (topo_sort, tarjan_scc, pagerank, etc.) but each uses `Result<_, QueryError>` or custom error conditions, never a unified GraphError type. |
| **Recommendation** | **Formalize in spec.** Graph algorithms need a unified error type for CycleDetected, EmptyGraph, NonConvergence. These are invariant-relevant (INV-QUERY-013 cycle detection). Add to spec/03-query.md Level 2 as a dedicated error enum. |

### 1.8 TxApplyError

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Type Catalog location** | Not explicitly in catalog but referenced by spec/01-store.md line 355 and 381 |
| **Guide location** | docs/guide/01-store.md (Transaction::apply return type) |
| **Spec location** | Referenced as return type in spec/01-store.md (lines 355, 381) but never defined. |
| **Recommendation** | **Formalize in spec.** This error type is used in the store's core `transact()` and `apply()` signatures. The variants need formal definition. Add to spec/01-store.md Level 2. |

### 1.9 Frontier (as named type)

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Type Catalog location** | Used throughout as `HashMap<AgentId, TxId>` but never formally named in Type Catalog |
| **Guide location** | docs/guide/01-store.md (Store::frontier field), docs/guide/00-architecture.md |
| **Spec location** | spec/01-store.md uses `Frontier` as parameter type in `as_of()` (line 390) and QueryMode::Stratified(Frontier) in spec/03-query.md (line 179) but never defines it as a named type. |
| **Recommendation** | **Formalize in spec.** The Frontier type is used as a parameter in multiple spec-level signatures but is never formally defined. Add `pub type Frontier = HashMap<AgentId, TxId>;` or newtype to spec/01-store.md Level 2. |

### 1.10 Indexes

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Type Catalog location** | Referenced in Store struct (docs/guide/00-architecture.md line 293) |
| **Guide location** | docs/guide/01-store.md (Store fields) |
| **Spec location** | spec/01-store.md mentions EAVT/AEVT/VAET/AVET indexes in INV-STORE-012 (Level 2, lines 816-823) but Indexes is not a named struct. |
| **Recommendation** | **Keep guide-only.** Index structure is an implementation detail. The spec defines index correctness via INV-STORE-012 without requiring a specific struct layout. |

### 1.11 EntityView

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Type Catalog location** | Referenced by Store::current() in guide catalog |
| **Guide location** | docs/guide/01-store.md |
| **Spec location** | spec/01-store.md line 387: `pub fn current(&self, entity: EntityId) -> EntityView;` — used as return type but never defined. |
| **Recommendation** | **Formalize in spec.** EntityView is the return type of a spec-level function. The struct needs formal definition in spec/01-store.md. |

### 1.12 SnapshotView

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 0 |
| **Type Catalog location** | Referenced by Store::as_of() in guide catalog |
| **Guide location** | docs/guide/01-store.md |
| **Spec location** | spec/01-store.md line 390: `pub fn as_of(&self, frontier: &Frontier) -> SnapshotView;` — used as return type but never defined. |
| **Recommendation** | **Formalize in spec.** SnapshotView is the return type of a spec-level function. Define in spec/01-store.md Level 2. |

### 1.13 HarvestQuality

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Type Catalog location** | docs/guide/00-architecture.md line 450 (HarvestResult contains drift_score + quality) |
| **Guide location** | docs/guide/05-harvest.md lines 68-73 |
| **Spec location** | None. spec/05-harvest.md defines HarvestSession (line 188) but not HarvestQuality. |
| **Recommendation** | **Keep guide-only.** HarvestQuality is a diagnostic summary (count by confidence tier). No invariant depends on its structure; it serves the LLM-facing output format. |

### 1.14 SessionContext

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Type Catalog location** | Not in Type Catalog but referenced across guide/ |
| **Guide location** | docs/guide/05-harvest.md lines 89-94 |
| **Spec location** | None. spec/05-harvest.md uses agent: AgentId parameter directly, not SessionContext. |
| **Recommendation** | **Keep guide-only.** SessionContext bundles arguments for the harvest pipeline. The spec's functional signatures accept individual parameters. This is a convenience aggregation for the implementation. |

### 1.15 ReconciliationType

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Type Catalog location** | Not in Type Catalog but used in guide/ |
| **Guide location** | docs/guide/05-harvest.md lines 50-54 |
| **Spec location** | None. spec/05-harvest.md does not define ReconciliationType. |
| **Recommendation** | **Keep guide-only.** ReconciliationType (Epistemic/Structural/Consequential) traces to the reconciliation taxonomy in CLAUDE.md but is not referenced by any spec invariant. It is metadata for LLM-facing output. |

### 1.16 CandidateStatus

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Type Catalog location** | docs/guide/00-architecture.md line 451 |
| **Guide location** | docs/guide/05-harvest.md lines 36-41 |
| **Spec location** | spec/05-harvest.md line 176 uses `status: CandidateStatus` but calls it a "lattice: :proposed < :under-review < :committed < :rejected". The enum is described in prose but not formally defined as Rust code. |
| **Recommendation** | **Formalize in spec.** The spec describes CandidateStatus as a lattice but only gives it as a comment, not a formal Level 2 enum. Add the Rust enum definition to spec/05-harvest.md Level 2 to match the guide. |

### 1.17 AssembledContext

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Type Catalog location** | docs/guide/00-architecture.md line 455 |
| **Guide location** | docs/guide/00-architecture.md line 455, docs/guide/06-seed.md (uses SeedOutput as the agent-facing formatted template) |
| **Spec location** | spec/06-seed.md lines 149-160 — **formally defined** with fields: sections, total_tokens, budget_remaining. |
| **Recommendation** | **No action needed.** AssembledContext IS defined in spec/06-seed.md Level 2. This is NOT a phantom type. The Type Catalog correctly references it and the spec defines it. |

### 1.18 ContextSection

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Type Catalog location** | Not explicitly in Type Catalog but is a field type of AssembledContext |
| **Guide location** | Not directly in docs/guide/ build plans |
| **Spec location** | spec/06-seed.md lines 156-161 — **formally defined** with fields: name, content, token_count, relevance. |
| **Recommendation** | **No action needed.** ContextSection is defined in spec/06-seed.md. Not a phantom type. |

### 1.19 ProjectionLevel

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Type Catalog location** | Not in Type Catalog |
| **Guide location** | Not in docs/guide/ build plans |
| **Spec location** | spec/06-seed.md lines 163-168 — **formally defined**: Pi0 through Pi4. |
| **Recommendation** | **No action needed.** Defined in spec. Deferred to later section. |

---

### Summary: Phantom Type Recommendations

| Type | Namespace | Recommendation |
|------|-----------|---------------|
| TxReport | STORE | **Formalize in spec** |
| TxValidationError | STORE | **Formalize in spec** |
| TxApplyError | STORE | **Formalize in spec** |
| EntityView | STORE | **Formalize in spec** |
| SnapshotView | STORE | **Formalize in spec** |
| Frontier (named type) | STORE | **Formalize in spec** |
| SchemaError | SCHEMA | **Formalize in spec** |
| GraphError | QUERY | **Formalize in spec** |
| CandidateStatus | HARVEST | **Formalize in spec** |
| QueryStats | QUERY | Keep guide-only |
| BindingSet | QUERY | Keep guide-only |
| FrontierRef | QUERY | Keep guide-only |
| Indexes | STORE | Keep guide-only |
| HarvestQuality | HARVEST | Keep guide-only |
| SessionContext | HARVEST | Keep guide-only |
| ReconciliationType | HARVEST | Keep guide-only |

**Formalize: 9 types.** Keep guide-only: 7 types. Already in spec: 2 types (AssembledContext, ContextSection -- false positives, not actually phantom).

---

## Section 2: Guide-Only Types (in docs/guide/ but not in spec/)

These types are defined in docs/guide/ implementation plans (docs/guide/01-store through docs/guide/09-interface)
but are not mentioned in any spec/ file. For each: determine whether to add to spec as a
Level 2 contract or keep as implementation detail.

### 2.1 SchemaLayer (docs/guide/02-schema.md)

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Guide location** | docs/guide/02-schema.md (enum: Layer0_Axiomatic through Layer5_Extension) |
| **Spec location** | spec/02-schema.md describes the six-layer architecture in prose (INV-SCHEMA-006) but does not define a Rust enum. |
| **Recommendation** | **Formalize in spec.** SchemaLayer is central to INV-SCHEMA-006 (progressive validation). The spec describes the layers but lacks the Rust enum. Add to spec/02-schema.md Level 2. |

### 2.2 LayerViolation (docs/guide/02-schema.md)

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Guide location** | docs/guide/02-schema.md (error type for layer validation) |
| **Spec location** | None. |
| **Recommendation** | **Keep guide-only.** This is an error detail type for layer violation reporting. The spec covers layer correctness via INV-SCHEMA-006 without needing this specific error struct. |

### 2.3 LatticeDefError (docs/guide/02-schema.md)

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Guide location** | docs/guide/02-schema.md |
| **Spec location** | None. |
| **Recommendation** | **Keep guide-only.** Error detail for lattice definition validation. The spec covers lattice correctness via INV-RESOLUTION-006 without this specific type. |

### 2.4 SchemaValidationError (docs/guide/02-schema.md)

| Field | Value |
|-------|-------|
| **Namespace** | SCHEMA |
| **Stage** | 0 |
| **Guide location** | docs/guide/02-schema.md |
| **Spec location** | None (related to SchemaError which is also missing). |
| **Recommendation** | **Keep guide-only.** Overlaps with SchemaError (section 1.3 above). The guide uses more specific error types; the spec needs only the top-level SchemaError. |

### 2.5 Stratum (docs/guide/03-query.md)

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Guide location** | docs/guide/03-query.md (enum: S0_Primitive, S1_MonotonicJoin, ..., S5_Temporal) |
| **Spec location** | spec/03-query.md defines six strata in prose (INV-QUERY-002) and uses `Stratum` in QueryResult (line 186) but does not define the Rust enum. |
| **Recommendation** | **Formalize in spec.** Stratum classification is core to CALM compliance (INV-QUERY-002). The spec references it as a type but never defines the enum variants. Add to spec/03-query.md Level 2. |

### 2.6 FindSpec (docs/guide/03-query.md)

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Guide location** | docs/guide/03-query.md |
| **Spec location** | Not defined. spec/03-query.md uses QueryExpr which contains a find_spec field but FindSpec itself is not a named type in spec. |
| **Recommendation** | **Keep guide-only.** The spec's QueryExpr enum already captures the query structure. FindSpec is a sub-structure for the implementation. |

### 2.7 ParsedQuery (docs/guide/03-query.md)

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Guide location** | docs/guide/03-query.md |
| **Spec location** | Not defined. spec/03-query.md has a parser section (INV-QUERY-005 Level 2, line 248) that returns `QueryAst`. |
| **Recommendation** | **Keep guide-only.** The spec uses `QueryAst` as the parsed representation. ParsedQuery appears to be an alias or earlier name. |

### 2.8 ParseError / QueryError (docs/guide/03-query.md)

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Guide location** | docs/guide/03-query.md |
| **Spec location** | `QueryError` is used as a return type in spec/03-query.md (line 191) but never formally defined as a Rust enum. |
| **Recommendation** | **Formalize in spec.** QueryError is used in spec-level signatures. Define variants in spec/03-query.md Level 2. |

### 2.9 DirectedGraph (docs/guide/03-query.md)

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Guide location** | docs/guide/03-query.md |
| **Spec location** | None. spec/03-query.md graph algorithms take `(&Store, EntityId, ...)` parameters directly, not a DirectedGraph type. |
| **Recommendation** | **Keep guide-only.** The graph engine in spec/ works on the store directly. DirectedGraph is an implementation-level intermediate representation. |

### 2.10 PullPattern / EntityRef (docs/guide/03-query.md)

| Field | Value |
|-------|-------|
| **Namespace** | QUERY |
| **Stage** | 0 |
| **Guide location** | docs/guide/03-query.md |
| **Spec location** | Not defined. spec/03-query.md mentions pull expressions in QueryExpr (line 157) but does not define PullPattern or EntityRef. |
| **Recommendation** | **Keep guide-only.** Pull patterns are an implementation concern of the query engine's entity retrieval mode. |

### 2.11 CascadeReceipt (docs/guide/07-merge-basic.md)

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 0 |
| **Guide location** | docs/guide/07-merge-basic.md lines 103-110 |
| **Spec location** | None. spec/07-merge.md defines MergeReceipt (line 176) but not CascadeReceipt. INV-MERGE-002 describes the 5-step cascade but without a receipt type. |
| **Recommendation** | **Keep guide-only.** CascadeReceipt is an implementation detail for tracking which cascade steps fired. The spec's INV-MERGE-002 defines the cascade requirement; the receipt is how the guide satisfies it. |

### 2.12 DriftSignals (docs/guide/08-guidance.md)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Guide location** | docs/guide/08-guidance.md lines 84-92 |
| **Spec location** | None. spec/12-guidance.md defines the guidance topology and footer but not DriftSignals as a struct. |
| **Recommendation** | **Keep guide-only.** DriftSignals bundles intermediate drift detection state. The spec defines drift detection at the invariant level (INV-GUIDANCE-004); the internal signal structure is implementation. |

### 2.13 GuidanceMechanism (docs/guide/08-guidance.md)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Guide location** | docs/guide/08-guidance.md |
| **Spec location** | None. |
| **Recommendation** | **Keep guide-only.** Enum for selecting which anti-drift mechanism to apply. This is guidance implementation routing, not a spec-level contract. |

### 2.14 DriftPriority (docs/guide/08-guidance.md)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Guide location** | docs/guide/08-guidance.md |
| **Spec location** | None. |
| **Recommendation** | **Keep guide-only.** Priority ordering for drift signal handling. Implementation detail of footer selection. |

### 2.15 GuidanceOutput (docs/guide/08-guidance.md)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Guide location** | docs/guide/08-guidance.md lines 94-102 |
| **Spec location** | None. spec/12-guidance.md defines GuidanceFooter and GuidanceTopology but not GuidanceOutput. |
| **Recommendation** | **Keep guide-only.** GuidanceOutput is the full response struct for the `braid guidance` command. It aggregates existing spec types (GuidanceFooter, MethodologyScore, RoutingDecision). |

### 2.16 ValueTemplate / PriorityFn (docs/guide/08-guidance.md)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Guide location** | docs/guide/08-guidance.md lines 141-142, line 135 |
| **Spec location** | spec/12-guidance.md defines DerivationRule (line 474) and TaskTemplate (line 482) but not ValueTemplate or PriorityFn. |
| **Recommendation** | **Keep guide-only.** These are sub-types of TaskTemplate and DerivationRule. The spec defines the parent types; these are implementation details. |

### 2.17 ClaudeMdConfig / AmbientSection / ActiveSection / Demonstration / DriftCorrection (docs/guide/08-guidance.md)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Guide location** | docs/guide/08-guidance.md (CLAUDE.md generation pipeline types) |
| **Spec location** | spec/12-guidance.md defines ClaudeMdConfig (line 306), AmbientSection (line 311), ActiveSection (line 317) at Level 2. |
| **Recommendation** | **No action needed for ClaudeMdConfig, AmbientSection, ActiveSection** -- these ARE in spec. **Keep guide-only for Demonstration and DriftCorrection** -- these are sub-types used within the CLAUDE.md pipeline, not needed at spec level. |

### 2.18 CommandRecord / SessionState (docs/guide/08-guidance.md)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE / INTERFACE |
| **Stage** | 0 |
| **Guide location** | docs/guide/08-guidance.md (detect_drift parameter), docs/guide/09-interface.md |
| **Spec location** | spec/14-interface.md defines SessionState (line 107) with fields. CommandRecord is not in spec. |
| **Recommendation** | **No action needed for SessionState** -- it IS in spec. **Keep guide-only for CommandRecord** -- it is an input type for drift detection, not a spec-level contract. |

### 2.19 MCPServer (docs/guide/09-interface.md)

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Guide location** | docs/guide/09-interface.md lines 105-126 |
| **Spec location** | spec/14-interface.md line 90 defines MCPServer with fields. |
| **Recommendation** | **No action needed.** MCPServer IS in spec. Not a guide-only type. |

### 2.20 ToolDescription / TypedParam (docs/guide/09-interface.md)

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Guide location** | docs/guide/09-interface.md lines 136-142 |
| **Spec location** | spec/14-interface.md lines 327-333 defines ToolDescription. TypedParam is not explicitly in spec. |
| **Recommendation** | **No action needed for ToolDescription** -- it IS in spec. **Keep guide-only for TypedParam** -- it is a sub-type of ToolDescription for parameter metadata. |

### 2.21 RecoveryHint / RecoveryAction / KernelError (docs/guide/09-interface.md)

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Guide location** | docs/guide/09-interface.md lines 202-218 |
| **Spec location** | spec/14-interface.md lines 372-386 defines RecoveryHint, RecoveryAction, and KernelError.recovery(). |
| **Recommendation** | **No action needed.** These ARE in spec. Not guide-only types. |

### 2.22 PersistenceError (docs/guide/09-interface.md)

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Guide location** | docs/guide/09-interface.md (load_store/save_store return type) |
| **Spec location** | None. |
| **Recommendation** | **Keep guide-only.** PersistenceError is a binary-crate concern (IO boundary), not a kernel invariant. The spec explicitly excludes IO from the kernel (docs/guide/00-architecture.md: braid-kernel has no IO). |

### 2.23 SeedOutput (docs/guide/06-seed.md)

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Guide location** | docs/guide/06-seed.md lines 31-37 |
| **Spec location** | Not defined as such. spec/06-seed.md defines AssembledContext (line 149) as the core type. SeedOutput is the guide's agent-facing formatted template produced FROM AssembledContext. |
| **Recommendation** | **Keep guide-only.** SeedOutput is the presentation format of AssembledContext. The spec defines the data model (AssembledContext); the guide defines the presentation (SeedOutput). This is a correct separation of concerns. |

---

### Summary: Guide-Only Type Recommendations

| Type | Namespace | Recommendation |
|------|-----------|---------------|
| SchemaLayer | SCHEMA | **Formalize in spec** |
| Stratum | QUERY | **Formalize in spec** |
| QueryError | QUERY | **Formalize in spec** |
| LayerViolation | SCHEMA | Keep guide-only |
| LatticeDefError | SCHEMA | Keep guide-only |
| SchemaValidationError | SCHEMA | Keep guide-only |
| FindSpec | QUERY | Keep guide-only |
| ParsedQuery | QUERY | Keep guide-only |
| DirectedGraph | QUERY | Keep guide-only |
| PullPattern / EntityRef | QUERY | Keep guide-only |
| CascadeReceipt | MERGE | Keep guide-only |
| DriftSignals | GUIDANCE | Keep guide-only |
| GuidanceMechanism | GUIDANCE | Keep guide-only |
| DriftPriority | GUIDANCE | Keep guide-only |
| GuidanceOutput | GUIDANCE | Keep guide-only |
| ValueTemplate / PriorityFn | GUIDANCE | Keep guide-only |
| Demonstration / DriftCorrection | GUIDANCE | Keep guide-only |
| CommandRecord | GUIDANCE | Keep guide-only |
| SeedOutput | SEED | Keep guide-only |
| PersistenceError | INTERFACE | Keep guide-only |
| TypedParam | INTERFACE | Keep guide-only |

**Formalize: 3 types.** Keep guide-only: 18 types. False positives (already in spec): 5 types (ClaudeMdConfig, AmbientSection, ActiveSection, SessionState, MCPServer, ToolDescription, RecoveryHint, RecoveryAction).

---

## Section 3: Spec-Only Types (in spec/ but not in guide/)

These types are defined in spec/ Level 2 sections but are not mentioned in any docs/guide/
implementation plan. For each: determine if this is a gap in the guide (should be added)
or correctly omitted (Stage 2+ type not needed yet).

### 3.1 WorkingSet (spec/01-store.md)

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 2 |
| **Spec location** | spec/01-store.md lines 866-876 (INV-STORE-013 Level 2) |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** INV-STORE-013 is Stage 2 (branching). docs/guide/01-store.md correctly defers this. |

### 3.2 MergedView (spec/01-store.md)

| Field | Value |
|-------|-------|
| **Namespace** | STORE |
| **Stage** | 2 |
| **Spec location** | spec/01-store.md line 873 (return type of WorkingSet::query_view) |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Companion type to WorkingSet, Stage 2. |

### 3.3 Branch (spec/07-merge.md)

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 2 |
| **Spec location** | spec/07-merge.md lines 142-152 |
| **Guide location** | None. docs/guide/07-merge-basic.md explicitly states "Branching (INV-MERGE-003-007) deferred to Stage 2." |
| **Recommendation** | **Correctly omitted.** Stage 2 type. docs/guide/07-merge-basic.md correctly covers only Stage 0 merge (set union). |

### 3.4 CombineStrategy (spec/07-merge.md)

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 2 |
| **Spec location** | spec/07-merge.md lines 153-158 (SetUnion, PickWinner, Interleave) |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 2 branching type. Stage 0 always uses SetUnion. |

### 3.5 ComparisonCriterion (spec/07-merge.md)

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 2 |
| **Spec location** | spec/07-merge.md lines 159-165 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 2 branch comparison type. |

### 3.6 BranchComparison (spec/07-merge.md)

| Field | Value |
|-------|-------|
| **Namespace** | MERGE |
| **Stage** | 2 |
| **Spec location** | spec/07-merge.md lines 167-175 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 2 type for comparing competing branches. |

### 3.7 Barrier (spec/08-sync.md)

| Field | Value |
|-------|-------|
| **Namespace** | SYNC |
| **Stage** | 3 |
| **Spec location** | spec/08-sync.md lines 107-115 |
| **Guide location** | None (no guide file for SYNC). |
| **Recommendation** | **Correctly omitted.** SYNC is entirely Stage 3. No guide file needed at Stage 0. |

### 3.8 BarrierResult / BarrierStatus (spec/08-sync.md)

| Field | Value |
|-------|-------|
| **Namespace** | SYNC |
| **Stage** | 3 |
| **Spec location** | spec/08-sync.md lines 116-122 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 3 type. |

### 3.9 SignalType (spec/09-signal.md)

| Field | Value |
|-------|-------|
| **Namespace** | SIGNAL |
| **Stage** | 1-3 (progressive) |
| **Spec location** | spec/09-signal.md lines 90-101 (eight signal types) |
| **Guide location** | None (no guide file for SIGNAL). |
| **Recommendation** | **Correctly omitted.** SIGNAL namespace is Stage 1+ (INV-SIGNAL-002 confusion signal is Stage 1; full taxonomy is Stage 3). No guide file needed at Stage 0. |

### 3.10 ConfusionKind (spec/09-signal.md)

| Field | Value |
|-------|-------|
| **Namespace** | SIGNAL |
| **Stage** | 1 |
| **Spec location** | spec/09-signal.md lines 102-108 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 1 type. |

### 3.11 Signal (spec/09-signal.md)

| Field | Value |
|-------|-------|
| **Namespace** | SIGNAL |
| **Stage** | 1-3 |
| **Spec location** | spec/09-signal.md lines 109-117 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 1+ type. |

### 3.12 Subscription (spec/09-signal.md)

| Field | Value |
|-------|-------|
| **Namespace** | SIGNAL |
| **Stage** | 3 |
| **Spec location** | spec/09-signal.md lines 118-125 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 3 type. |

### 3.13 BilateralLoop (spec/10-bilateral.md)

| Field | Value |
|-------|-------|
| **Namespace** | BILATERAL |
| **Stage** | 1 |
| **Spec location** | spec/10-bilateral.md lines 106-113 |
| **Guide location** | None (no guide file for BILATERAL). |
| **Recommendation** | **Correctly omitted.** BILATERAL is Stage 1+. No guide file needed at Stage 0. |

### 3.14 Boundary (spec/10-bilateral.md)

| Field | Value |
|-------|-------|
| **Namespace** | BILATERAL |
| **Stage** | 1 |
| **Spec location** | spec/10-bilateral.md lines 114-120 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 1 type. |

### 3.15 Gap (spec/10-bilateral.md)

| Field | Value |
|-------|-------|
| **Namespace** | BILATERAL |
| **Stage** | 1 |
| **Spec location** | spec/10-bilateral.md lines 121-131 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 1 type. |

### 3.16 Deliberation (spec/11-deliberation.md)

| Field | Value |
|-------|-------|
| **Namespace** | DELIBERATION |
| **Stage** | 2 |
| **Spec location** | spec/11-deliberation.md lines 81-89 |
| **Guide location** | None (no guide file for DELIBERATION). |
| **Recommendation** | **Correctly omitted.** DELIBERATION is Stage 2. No guide file needed at Stage 0. |

### 3.17 DeliberationStatus (spec/11-deliberation.md)

| Field | Value |
|-------|-------|
| **Namespace** | DELIBERATION |
| **Stage** | 2 |
| **Spec location** | spec/11-deliberation.md lines 90-97 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 2 type. |

### 3.18 Position (spec/11-deliberation.md)

| Field | Value |
|-------|-------|
| **Namespace** | DELIBERATION |
| **Stage** | 2 |
| **Spec location** | spec/11-deliberation.md lines 98-106 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 2 type. |

### 3.19 Decision (spec/11-deliberation.md)

| Field | Value |
|-------|-------|
| **Namespace** | DELIBERATION |
| **Stage** | 2 |
| **Spec location** | spec/11-deliberation.md lines 107-117 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 2 type. |

### 3.20 GuidanceTopology (spec/12-guidance.md)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec location** | spec/12-guidance.md lines 97-100 |
| **Guide location** | docs/guide/08-guidance.md lines 25-28 |
| **Recommendation** | **No action needed.** This IS in the guide. Not a spec-only type. |

### 3.21 GuidanceNode (spec/12-guidance.md)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec location** | spec/12-guidance.md lines 102-108 |
| **Guide location** | docs/guide/08-guidance.md lines 30-36 |
| **Recommendation** | **No action needed.** This IS in the guide. Not a spec-only type. |

### 3.22 GuidanceAction (spec/12-guidance.md)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 0 |
| **Spec location** | spec/12-guidance.md lines 110-115 |
| **Guide location** | docs/guide/08-guidance.md lines 38-43 |
| **Recommendation** | **No action needed.** This IS in the guide. Not a spec-only type. |

### 3.23 LwwClock (spec/04-resolution.md)

| Field | Value |
|-------|-------|
| **Namespace** | RESOLUTION |
| **Stage** | 0 |
| **Spec location** | spec/04-resolution.md lines 139-144 (enum: HlcTimestamp, Blake3Hash) |
| **Guide location** | None. docs/guide/04-resolution.md describes BLAKE3 tie-breaking in prose (line 89) but does not define LwwClock as a type. |
| **Recommendation** | **Gap -- add to guide.** LwwClock is a Stage 0 type needed for INV-RESOLUTION-005 (LWW semilattice). The guide describes the behavior but should include this enum in the API surface. |

### 3.24 Conflict (spec/04-resolution.md)

| Field | Value |
|-------|-------|
| **Namespace** | RESOLUTION |
| **Stage** | 0 |
| **Spec location** | spec/04-resolution.md lines 146-153 |
| **Guide location** | docs/guide/04-resolution.md uses ConflictSet (similar but different name/structure). |
| **Recommendation** | **Alignment issue.** Spec uses `Conflict` (entity, attribute, values, tier). Guide uses `ConflictSet` (entity, attribute, assertions, retractions). These should be reconciled. The guide's ConflictSet is richer (separates assertions/retractions). Recommend aligning spec to use ConflictSet or documenting the mapping. |

### 3.25 ConflictTier (spec/04-resolution.md)

| Field | Value |
|-------|-------|
| **Namespace** | RESOLUTION |
| **Stage** | 0 |
| **Spec location** | spec/04-resolution.md lines 155-161 (Auto, Notify, Escalate) |
| **Guide location** | docs/guide/04-resolution.md uses RoutingTier (Automatic, AgentNotification, HumanRequired). |
| **Recommendation** | **Alignment issue.** Spec's ConflictTier and guide's RoutingTier are equivalent but differently named. Reconcile naming: both represent INV-RESOLUTION-007's three-tier routing. |

### 3.26 Resolution (spec/04-resolution.md)

| Field | Value |
|-------|-------|
| **Namespace** | RESOLUTION |
| **Stage** | 0 |
| **Spec location** | spec/04-resolution.md lines 162-170 (entity with method, value, explanation) |
| **Guide location** | docs/guide/04-resolution.md uses ResolvedValue (enum: Single, Multi, Conflict). |
| **Recommendation** | **Alignment issue.** Spec's Resolution is an entity recording the resolution decision. Guide's ResolvedValue is the resolved value itself. These are different types serving different purposes (one for provenance, one for query result). Both are needed. Guide should add Resolution as the provenance record type per NEG-RESOLUTION-003. |

### 3.27 BudgetManager (spec/13-budget.md)

| Field | Value |
|-------|-------|
| **Namespace** | BUDGET |
| **Stage** | 1 |
| **Spec location** | spec/13-budget.md lines 92-98 |
| **Guide location** | None (no guide file for BUDGET). |
| **Recommendation** | **Correctly omitted.** BUDGET is Stage 1. No guide file needed at Stage 0. |

### 3.28 OutputPrecedence (spec/13-budget.md)

| Field | Value |
|-------|-------|
| **Namespace** | BUDGET |
| **Stage** | 1 |
| **Spec location** | spec/13-budget.md lines 99-108 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 1 type (budget-aware output ordering). |

### 3.29 TokenEfficiency (spec/13-budget.md)

| Field | Value |
|-------|-------|
| **Namespace** | BUDGET |
| **Stage** | 1 |
| **Spec location** | spec/13-budget.md lines 284-291 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** Stage 1 type for INV-BUDGET-006. |

### 3.30 HarvestSession (spec/05-harvest.md)

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 |
| **Spec location** | spec/05-harvest.md lines 188-195 |
| **Guide location** | Not defined as a struct. docs/guide/05-harvest.md uses `harvest_session_entity()` function that creates a session entity but does not define the struct. |
| **Recommendation** | **Gap -- add to guide.** HarvestSession is a Stage 0 type (INV-HARVEST-002 provenance trail). The guide should define the struct to match spec. Currently the guide creates session entities procedurally but lacks the type definition. |

### 3.31 ReviewTopology (spec/05-harvest.md)

| Field | Value |
|-------|-------|
| **Namespace** | HARVEST |
| **Stage** | 0 (SelfReview), 2 (other variants) |
| **Spec location** | spec/05-harvest.md lines 197-203 |
| **Guide location** | Not defined. docs/guide/05-harvest.md mentions "Stage 0 = single-agent self-review" but does not define the ReviewTopology enum. |
| **Recommendation** | **Gap -- add to guide.** At minimum, define the Stage 0 variant (SelfReview). The full enum can note deferred variants. |

### 3.32 ClaudeMdGenerator (spec/06-seed.md)

| Field | Value |
|-------|-------|
| **Namespace** | SEED |
| **Stage** | 0 |
| **Spec location** | spec/06-seed.md lines 171-198 |
| **Guide location** | docs/guide/06-seed.md defines `generate_claude_md()` as a free function (line 43) but not as a struct with methods. |
| **Recommendation** | **Minor gap.** The spec uses a stateful struct (ClaudeMdGenerator) while the guide uses a free function. The guide approach is simpler and adequate for Stage 0. Note the divergence but no action required -- the guide's free function satisfies the spec's functional requirements. |

### 3.33 MCPTool (spec/14-interface.md)

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 0 |
| **Spec location** | spec/14-interface.md lines 96-105 (enum with 6 variants) |
| **Guide location** | docs/guide/09-interface.md uses `const MCP_TOOLS: [&str; 6]` (line 129) as a string array, not an enum. |
| **Recommendation** | **Minor gap.** The spec defines MCPTool as an enum; the guide uses a string array. Both enforce INV-INTERFACE-003 (exactly 6 tools). The enum is stronger (compile-time exhaustiveness). The guide should align to use the enum pattern. |

### 3.34 TUIState (spec/14-interface.md)

| Field | Value |
|-------|-------|
| **Namespace** | INTERFACE |
| **Stage** | 4 |
| **Spec location** | spec/14-interface.md lines 119-127 |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** TUI is Stage 4. No guide coverage needed at Stage 0. |

### 3.35 Topology / TopologyRecommendation / ProjectPhase (spec/12-guidance.md)

| Field | Value |
|-------|-------|
| **Namespace** | GUIDANCE |
| **Stage** | 2 |
| **Spec location** | spec/12-guidance.md lines 650-664 (INV-GUIDANCE-011) |
| **Guide location** | None. |
| **Recommendation** | **Correctly omitted.** INV-GUIDANCE-011 (topology fitness T(t)) is Stage 2. |

---

### Summary: Spec-Only Type Recommendations

| Type | Namespace | Stage | Recommendation |
|------|-----------|-------|---------------|
| LwwClock | RESOLUTION | 0 | **Gap -- add to guide** |
| Conflict / ConflictSet | RESOLUTION | 0 | **Alignment issue** -- reconcile names |
| ConflictTier / RoutingTier | RESOLUTION | 0 | **Alignment issue** -- reconcile names |
| Resolution (provenance entity) | RESOLUTION | 0 | **Gap -- add to guide** |
| HarvestSession | HARVEST | 0 | **Gap -- add to guide** |
| ReviewTopology | HARVEST | 0 | **Gap -- add to guide** |
| MCPTool (enum vs string array) | INTERFACE | 0 | **Minor gap** -- align to enum |
| ClaudeMdGenerator (struct vs fn) | SEED | 0 | **Minor gap** -- note divergence |
| WorkingSet | STORE | 2 | Correctly omitted |
| MergedView | STORE | 2 | Correctly omitted |
| Branch | MERGE | 2 | Correctly omitted |
| CombineStrategy | MERGE | 2 | Correctly omitted |
| ComparisonCriterion | MERGE | 2 | Correctly omitted |
| BranchComparison | MERGE | 2 | Correctly omitted |
| Barrier | SYNC | 3 | Correctly omitted |
| BarrierResult / BarrierStatus | SYNC | 3 | Correctly omitted |
| SignalType | SIGNAL | 1-3 | Correctly omitted |
| ConfusionKind | SIGNAL | 1 | Correctly omitted |
| Signal | SIGNAL | 1-3 | Correctly omitted |
| Subscription | SIGNAL | 3 | Correctly omitted |
| BilateralLoop | BILATERAL | 1 | Correctly omitted |
| Boundary | BILATERAL | 1 | Correctly omitted |
| Gap | BILATERAL | 1 | Correctly omitted |
| Deliberation | DELIBERATION | 2 | Correctly omitted |
| DeliberationStatus | DELIBERATION | 2 | Correctly omitted |
| Position | DELIBERATION | 2 | Correctly omitted |
| Decision | DELIBERATION | 2 | Correctly omitted |
| BudgetManager | BUDGET | 1 | Correctly omitted |
| OutputPrecedence | BUDGET | 1 | Correctly omitted |
| TokenEfficiency | BUDGET | 1 | Correctly omitted |
| TUIState | INTERFACE | 4 | Correctly omitted |
| Topology / TopologyRecommendation / ProjectPhase | GUIDANCE | 2 | Correctly omitted |

**Gaps to fill: 4 types.** Alignment issues: 3 type-pairs. Minor gaps: 2. Correctly omitted: 24 types (all Stage 1+).

---

## Section 4: Consolidated Action Items

### Priority 1: Formalize in spec/ (9 types from Section 1 + 3 from Section 2 = 12 total)

These types are used in spec-level function signatures or enforce spec invariants but
lack formal Level 2 definitions.

| Type | Target File | Rationale |
|------|-------------|-----------|
| TxReport | spec/01-store.md | Return type of Transaction<Applied>::receipt() |
| TxValidationError | spec/01-store.md | Return type of commit(); gates correctness |
| TxApplyError | spec/01-store.md | Return type of transact()/apply() |
| EntityView | spec/01-store.md | Return type of Store::current() |
| SnapshotView | spec/01-store.md | Return type of Store::as_of() |
| Frontier (named type) | spec/01-store.md | Used as parameter type in multiple signatures |
| SchemaError | spec/02-schema.md | Return type of validate_value() |
| GraphError | spec/03-query.md | Needed for graph algorithm error handling |
| CandidateStatus | spec/05-harvest.md | Described in prose but not as Rust enum |
| SchemaLayer | spec/02-schema.md | Central to INV-SCHEMA-006 progressive validation |
| Stratum | spec/03-query.md | Central to INV-QUERY-002 CALM compliance |
| QueryError | spec/03-query.md | Return type of query() in spec signatures |

### Priority 2: Add to docs/guide/ (4 types from Section 3)

These types are defined in spec/ for Stage 0 but are missing from docs/guide/ build plans.

| Type | Target File | Rationale |
|------|-------------|-----------|
| LwwClock | docs/guide/04-resolution.md | Stage 0 type for INV-RESOLUTION-005 |
| Resolution (provenance entity) | docs/guide/04-resolution.md | NEG-RESOLUTION-003 requires resolution provenance |
| HarvestSession | docs/guide/05-harvest.md | INV-HARVEST-002 provenance trail |
| ReviewTopology | docs/guide/05-harvest.md | At minimum SelfReview variant for Stage 0 |

### Priority 3: Reconcile naming (3 type-pairs from Section 3)

Spec and guide use different names for equivalent Stage 0 types.

| Spec Name | Guide Name | File | Recommendation |
|-----------|-----------|------|---------------|
| Conflict | ConflictSet | spec/04-resolution.md, docs/guide/04-resolution.md | Adopt ConflictSet (richer, separates assertions/retractions) |
| ConflictTier | RoutingTier | spec/04-resolution.md, docs/guide/04-resolution.md | Adopt RoutingTier (more descriptive variant names) |
| MCPTool (enum) | MCP_TOOLS (string array) | spec/14-interface.md, docs/guide/09-interface.md | Adopt enum pattern from spec (stronger guarantee) |

### Priority 4: Minor gaps (2 from Section 3, informational only)

| Type | Issue | Action |
|------|-------|--------|
| ClaudeMdGenerator | Spec: struct. Guide: free function. | No action. Guide approach is simpler, functionally equivalent. |
| MCPTool enum vs string array | Spec defines enum, guide uses const array. | Consider aligning guide to enum (Priority 3 above). |

---

## Appendix: Type Coverage Matrix

Total unique types across spec/ and guide/:

| Category | Count | Formalize | Guide-only | Correctly omitted |
|----------|-------|-----------|-----------|-------------------|
| Phantom (catalog, no spec) | 16 real + 2 false positive | 9 | 7 | -- |
| Guide-only (guide, no spec) | 23 real + 8 false positive | 3 | 18 | -- |
| Spec-only (spec, no guide) | 35 total | 4 gaps + 3 alignment | -- | 24 |
| **Totals** | **74 unique types audited** | **16 + 3 alignment** | **25** | **24** |

The 24 correctly omitted spec-only types are all from Stage 1+ namespaces:
- SYNC (3 types, Stage 3)
- SIGNAL (4 types, Stage 1-3)
- BILATERAL (3 types, Stage 1)
- DELIBERATION (4 types, Stage 2)
- MERGE branching (4 types, Stage 2)
- BUDGET (3 types, Stage 1)
- STORE WorkingSet (2 types, Stage 2)
- INTERFACE TUI (1 type, Stage 4)
- GUIDANCE topology (3 types, Stage 2)

This confirms the staged design is working correctly: Stage 0 guide files do not
prematurely include Stage 1+ types.

---
