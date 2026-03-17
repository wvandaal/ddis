# Schema + Resolution — Stage 0/1 Audit
> Wave 1 Domain Audit | Date: 2026-03-17 | Agent: Opus 4.6 | Methodology: Fagan Inspection + IEEE Walkthrough

## Domain Inventory

### SCHEMA Spec Elements (spec/02-schema.md)

**Invariants (9):**
- INV-SCHEMA-001 through INV-SCHEMA-009

**ADRs (8):**
- ADR-SCHEMA-001 through ADR-SCHEMA-008

**Negative Cases (3):**
- NEG-SCHEMA-001, NEG-SCHEMA-002, NEG-SCHEMA-003

### RESOLUTION Spec Elements (spec/04-resolution.md)

**Invariants (8):**
- INV-RESOLUTION-001 through INV-RESOLUTION-008

**ADRs (13):**
- ADR-RESOLUTION-001 through ADR-RESOLUTION-013

**Negative Cases (3):**
- NEG-RESOLUTION-001, NEG-RESOLUTION-002, NEG-RESOLUTION-003

---

## Findings

### FINDING-001: Genesis attribute count diverges between spec (17/18) and code (19)

Severity: HIGH
Type: DIVERGENCE
Sources: spec/02-schema.md line 30 ("18 axiomatic meta-schema attributes") vs schema.rs:488 (`GENESIS_ATTR_COUNT: usize = 19`) vs spec/02-schema.md INV-SCHEMA-002 Level 1 ("exactly the 18 axiomatic attribute definitions") vs guide 02-schema.md line 85 ("17 axiomatic attributes")
Evidence: The spec algebraic definition says "Let A_0 = {a_1, ..., a_18} be the 18 axiomatic meta-schema attributes" at the L0 level. INV-SCHEMA-002 L1 states "exactly the 18 axiomatic attribute definitions". The guide says "17 axiomatic attributes" at line 85 and "exactly 17 attributes" in the genesis assertion at line 437. The code defines `GENESIS_ATTR_COUNT = 19` and lists 19 attributes (9 `:db/*` + 5 `:lattice/*` + 5 `:tx/*` including `:tx/rationale` and `:tx/coherence-override`). Three numbers (17, 18, 19) exist across four documents for the same concept.
Impact: The spec, guide, and code all disagree on the foundational count. This undermines INV-SCHEMA-002 (Genesis Completeness). Any test asserting "exactly 17" or "exactly 18" would fail against the actual code, which has 19. The inconsistency itself violates C5 (Traceability) -- there is no single source of truth for the genesis attribute count.

---

### FINDING-002: ResolutionMode::Lattice variant lacks lattice_id field

Severity: HIGH
Type: DIVERGENCE
Sources: spec/04-resolution.md L2 interface at line 22 (`Lattice { lattice_id: EntityId }`) and guide/04-resolution.md line 21 (`Lattice { lattice_id: EntityId }`) vs schema.rs:183 (`Lattice`)
Evidence: Both the spec and the guide define `ResolutionMode::Lattice` as a struct variant carrying a `lattice_id: EntityId` reference to the lattice definition entity. The implementation defines `Lattice` as a unit variant with no associated data: `pub enum ResolutionMode { Lww, Lattice, Multi }`. The resolution code at resolution.rs:158-162 confirms the fallback: `ResolutionMode::Lattice => { // Stage 0: lattice resolution falls back to LWW resolve_lww(&active) }`.
Impact: Without the `lattice_id` field, there is no way to look up the lattice definition entity from the resolution mode. The entire lattice resolution pathway described in the spec (lookup lattice entity, compute join, detect incomparability) is structurally impossible with this type. This is not merely a deferred feature -- the type itself is wrong relative to the spec.

---

### FINDING-003: Lattice resolution is entirely unimplemented -- falls back to LWW

Severity: HIGH
Type: UNIMPLEMENTED
Sources: spec/04-resolution.md INV-RESOLUTION-006 ("lattice resolution produces the least upper bound") vs resolution.rs:158-162
Evidence: The code at resolution.rs:158-162 reads: `ResolutionMode::Lattice => { // Stage 0: lattice resolution falls back to LWW // Full lattice join requires lattice definitions from store resolve_lww(&active) }`. INV-RESOLUTION-006 requires: "lattice resolution produces the least upper bound; diamond lattices produce error signal element for incomparable values." INV-SCHEMA-008 (Stage 2) requires diamond lattice signal generation. Neither exists.
Impact: Any attribute declared with `:resolution/lattice` mode silently behaves as LWW. This means the three resolution modes are effectively two. Lattice-resolved attributes declared in the schema (e.g., `:task/status` described as "lattice-resolved" at schema.rs:1528) will exhibit LWW semantics, which may produce incorrect resolution for attributes with non-total orderings.

---

### FINDING-004: Schema::from_datoms filters only "db" namespace, excluding lattice datoms

Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/02-schema.md INV-SCHEMA-007 ("Every lattice-resolved attribute has a complete lattice definition") and guide 02-schema.md lines 196-218 (lattice_def extraction during from_store) vs schema.rs:275
Evidence: At schema.rs:275, `Schema::from_datoms` filters: `if datom.op == Op::Assert && datom.attribute.namespace() == "db"`. This excludes `:lattice/*` namespace datoms. The guide at lines 196-218 prescribes that `Schema::from_store()` should extract lattice definitions from datoms and store them internally, with a `lattice_def(id)` method. The code has no `LatticeDef` struct, no `lattice_def()` method, and no lattice extraction logic.
Impact: Even if lattice definition datoms exist in the store, they are invisible to the schema reconstruction. INV-SCHEMA-007 (Lattice Definition Completeness) cannot be enforced because the validation infrastructure to check lattice completeness does not exist. The `validate_lattice_completeness` method prescribed by the guide is absent from the code.

---

### FINDING-005: No LwwClock type or per-attribute clock selection

Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: spec/02-schema.md L2 interface lines 300-310 (LwwClock enum with Hlc/Wall/AgentRank) and guide/04-resolution.md lines 53-59 vs resolution.rs and schema.rs (no occurrence)
Evidence: The spec defines `LwwClock { Hlc, Wall, AgentRank }` as a per-attribute clock selection mechanism. The guide specifies `pub fn lww_clock(&self, attr: &Attribute) -> LwwClock`. Search for "LwwClock", "lww_clock", or "lww.*clock" across all Rust files returns zero matches. The `:db/lwwClock` attribute is defined in genesis (schema.rs:608-612), but its value is never read or used by the resolution engine.
Impact: The genesis schema allocates storage for `:db/lwwClock` but the resolution engine ignores it. All LWW resolution uses TxId ordering (wall_time, logical, agent fields). The advertised capability of per-attribute clock selection is dead code at the schema level and dead air at the resolution level.

---

### FINDING-006: Schema constructor named `from_datoms` vs spec/guide `from_store`

Severity: LOW
Type: MISALIGNMENT
Sources: spec/02-schema.md L2 at line 279 (`Schema::from_store(datoms: &BTreeSet<Datom>)`) and guide/02-schema.md line 25 (`Schema::from_store`) vs schema.rs:270 (`Schema::from_datoms`)
Evidence: Both the spec and guide consistently name the constructor `from_store`. The implementation names it `from_datoms`. The spec states "the only constructor -- enforces C3" referring to `from_store`. The guide API surface at line 25 reads `pub fn from_store(datoms: &BTreeSet<Datom>) -> Schema`.
Impact: Minor naming inconsistency. Does not affect correctness but violates C5 (Traceability) -- searching for `from_store` in code finds nothing, making spec-to-code mapping harder.

---

### FINDING-007: `new_attribute` method does not exist

Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: guide/02-schema.md lines 31 and 246-259 (`Schema::new_attribute(&self, spec: AttributeSpec) -> Vec<Datom>`) vs schema.rs
Evidence: The guide prescribes a `new_attribute` method on Schema that takes an `AttributeSpec` and produces datoms for a new attribute definition. Searching the codebase for "new_attribute" returns zero results. The equivalent functionality exists as the free function `schema_datoms_from_specs` at schema.rs:1740, but it is private (`fn`, not `pub fn`) and operates on a slice of specs, not through the Schema struct.
Impact: The public API for schema evolution does not match the guide's prescription. Callers cannot use the Schema struct to produce schema evolution datoms as documented. The existing mechanism works (Layer 1-4 functions use `schema_datoms_from_specs`), but the advertised API surface is absent.

---

### FINDING-008: `validate_layer_ordering` method does not exist

Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: guide/02-schema.md lines 171-174 (`pub fn validate_layer_ordering(&self) -> Vec<LayerViolation>`) and spec INV-SCHEMA-006 ("layer ordering must be respected") vs schema.rs
Evidence: The guide prescribes `Schema::validate_layer_ordering()` returning `Vec<LayerViolation>`. The spec has verification tag `V:PROP` with a proptest that calls this method. There is no `SchemaLayer` enum, no `LayerViolation` type, and no `validate_layer_ordering` method in the code. Layer ordering is verified only in tests via value type assertions (e.g., "layer 3 only references layer 0 types" at schema.rs:2583), not through a programmatic API.
Impact: INV-SCHEMA-006 has partial enforcement -- the tests verify layer properties, but there is no runtime mechanism to detect layer violations during schema evolution. A user-defined attribute that violates layer ordering would not be caught at transact time.

---

### FINDING-009: `has_conflict` does not check causal independence (condition 6 of INV-RESOLUTION-004)

Severity: HIGH
Type: DIVERGENCE
Sources: spec/04-resolution.md INV-RESOLUTION-004 L0 (six conditions including "causally independent") and guide/04-resolution.md lines 121-152 (`is_causal_ancestor` walk) vs resolution.rs:216-229
Evidence: INV-RESOLUTION-004 requires six conditions for a conflict, the sixth being: "Causally independent (different agents or no causal ordering)." The guide provides a full `is_causal_ancestor` implementation using causal_predecessors walk. The actual code at resolution.rs:216-229 is: `let active = conflict.active_assertions(); if active.len() <= 1 { return false; } let first_val = &active[0].0; active.iter().any(|(v, _)| v != first_val)`. This checks only conditions 1-4 (same entity, same attribute, different values, both assertions) implicitly and condition 5 (multi never conflicts). It does NOT check causal independence. Two causally ordered assertions with different values (a legitimate update) would be falsely flagged as a conflict.
Impact: This creates false positive conflicts. Every sequential update to a cardinality-one attribute produces a false conflict report because the function does not distinguish causal updates from truly concurrent assertions. This directly violates the formal predicate in INV-RESOLUTION-004 and undermines INV-RESOLUTION-003 (conservative detection is fine for false positives, but the spec's formal predicate explicitly requires causal independence as a condition, making this a semantic divergence from the specified behavior).

---

### FINDING-010: `detect_conflicts` has different signature than guide

Severity: MEDIUM
Type: DIVERGENCE
Sources: guide/04-resolution.md lines 173-187 (`pub fn detect_conflicts(store: &Store, frontier: &HashMap<AgentId, TxId>) -> Vec<ConflictSet>`) vs resolution.rs:327-369 (`pub fn detect_conflicts(store: &Store, entity: EntityId, attribute: &Attribute) -> Option<ConflictEntity>`)
Evidence: The guide prescribes a store-wide conflict detection function that takes a frontier and returns all conflicts. The implementation is per-(entity, attribute) pair, returning a single optional conflict. There is no store-wide scan function. INV-RESOLUTION-003 specifically requires frontier-relative detection: "For any local frontier F_local <= F_global: conflicts(F_local) >= conflicts(F_global)". The implementation ignores frontiers entirely.
Impact: Frontier-based conservative conflict detection (the core of INV-RESOLUTION-003) is not implemented. The code detects conflicts over the full store, not a frontier subset. This means the "conservative detection" property is vacuously satisfied (full store = global frontier), but the frontier-relative guarantees that are critical for multi-agent scenarios are absent.

---

### FINDING-011: `conflict_to_datoms` uses unregistered `:resolution/*` attributes

Severity: HIGH
Type: DIVERGENCE
Sources: resolution.rs:437-489 (uses `:resolution/entity`, `:resolution/attribute`, `:resolution/mode`, `:resolution/winner`, `:resolution/conflict-count`) vs schema.rs (no `:resolution/*` attributes in any layer)
Evidence: The function `conflict_to_datoms` produces datoms with five `:resolution/*` namespace attributes. None of these attributes are defined in any schema layer (Layer 0 through Layer 4). They are not in `genesis_datoms`, `layer_1_attributes`, `layer_2_attributes`, `layer_3_attributes`, or `layer_4_attributes`. The spec (spec/04-resolution.md NEG-RESOLUTION-003) and guide (guide/04-resolution.md lines 62-68) both expect resolution provenance as datoms in the store.
Impact: If `conflict_to_datoms` output is transacted into a store with schema validation enabled (INV-SCHEMA-004), every datom will be rejected as `UnknownAttribute`. The audit trail mechanism for resolutions is structurally incompatible with schema validation. Either schema validation silently allows unknown attributes in some path, or resolution provenance datoms are never actually transacted.

---

### FINDING-012: Three-tier routing (INV-RESOLUTION-007) is unimplemented

Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: spec/04-resolution.md INV-RESOLUTION-007 ("every detected conflict is routed to exactly one of {Automatic, AgentNotification, HumanRequired}") and guide/04-resolution.md lines 232-248 vs codebase
Evidence: Search for `RoutingTier`, `route_conflict`, `AgentNotification`, and `HumanRequired` across all Rust files returns zero results. The guide specifies a `RoutingTier` enum and `route_conflict` function. The spec has proptest verification at guide/04-resolution.md lines 386-396. None of this exists.
Impact: INV-RESOLUTION-007 is entirely unimplemented. There is no mechanism to classify conflicts by severity or route them to different resolution tiers. All conflicts are implicitly "automatic" (LWW resolves them), which is correct for Stage 0 single-agent use but does not satisfy the invariant's totality requirement.

---

### FINDING-013: INV-RESOLUTION-008 conflict lifecycle pipeline is mostly unimplemented

Severity: MEDIUM
Type: UNIMPLEMENTED
Sources: spec/04-resolution.md INV-RESOLUTION-008 ("full conflict lifecycle: assert, severity, route, fire TUI, update uncertainty, invalidate caches all produce datoms") vs resolution.rs
Evidence: ADR-RESOLUTION-013 explicitly acknowledges this gap and prescribes stub datoms for steps 4-6. However, even steps 1-3 lack the full pipeline behavior described in the spec. There is no severity computation, no routing step, and the stub datoms for steps 4-6 do not exist. The `ConflictEntity` struct exists but is only used in `detect_conflicts` and `resolve_with_trail`, not in a pipeline producing 6 datoms per lifecycle step.
Impact: Per ADR-RESOLUTION-013 (which is Stage 0), steps 1-3 should be fully implemented and steps 4-6 should produce stub datoms. Neither is true. The audit trail is partial at best.

---

### FINDING-014: Spec has 18 axiomatic attributes, guide has 17, code has 19 -- three-way disagreement

Severity: HIGH
Type: CONTRADICTION
Sources: spec/02-schema.md L0 ("18 axiomatic meta-schema attributes" at line 30), spec/02-schema.md ADR-SCHEMA-002 ("17 hardcoded meta-schema attributes" at line 773), guide/02-schema.md line 85 ("17 axiomatic attributes"), schema.rs:488 (`GENESIS_ATTR_COUNT: usize = 19`)
Evidence: Within the spec itself, there is a contradiction: the L0 algebraic definition says 18, but ADR-SCHEMA-002 Decision says "Exactly 17 attributes are hardcoded in the engine." The guide says 17. The code has 19. The difference between 17 and 19 is `:tx/rationale` and `:tx/coherence-override`, which were added in later sessions. The spec was not updated.
Impact: This is a contradiction within the spec (violating C6 -- the falsification condition for INV-SCHEMA-002 references "17" in one place and "18" in another). The code is the ground truth at 19, but no spec element records this evolution.

---

### FINDING-015: No `SchemaValidationError` type exists as specified

Severity: LOW
Type: MISALIGNMENT
Sources: spec/02-schema.md L2 lines 330-341 (5-variant SchemaValidationError enum) vs schema.rs:395-410 (uses `StoreError`)
Evidence: The spec defines a dedicated `SchemaValidationError` enum with 5 variants: `UnknownAttribute`, `TypeMismatch`, `CardinalityViolation`, `InvalidLatticeValue`, `InvalidRetraction`. The code uses `StoreError` variants (`StoreError::UnknownAttribute`, `StoreError::SchemaViolation`). Of the five spec variants, only `UnknownAttribute` and type mismatch (as `SchemaViolation`) are implemented. `CardinalityViolation`, `InvalidLatticeValue`, and `InvalidRetraction` do not exist.
Impact: Schema validation is less precise than specified. Cardinality violations, invalid lattice values, and invalid retractions are not checked. This weakens INV-SCHEMA-004 ("no datom with an undefined attribute or mistyped value enters the store"). The retraction validation ("can only retract what was asserted") is not implemented.

---

### FINDING-016: `validate_datom` does not check cardinality or retraction validity

Severity: MEDIUM
Type: GAP
Sources: spec/02-schema.md INV-SCHEMA-004 L0 ("cardinality check" and "retraction requires prior assertion") vs schema.rs:395-410
Evidence: INV-SCHEMA-004 L0 formally requires: (1) attribute must exist, (2) value type must match, (3) retraction requires prior assertion of the same entity-attribute pair. The code at schema.rs:395-410 only checks conditions (1) and (2). There is no cardinality enforcement at validation time and no retraction-requires-prior-assertion check. The `validate_datom` method receives a single datom with no access to existing store state, making condition (3) structurally impossible to check at this layer.
Impact: Retractions of never-asserted entity-attribute pairs are silently accepted. This violates the formal predicate of INV-SCHEMA-004.

---

### FINDING-017: NEG-RESOLUTION-001 is well-enforced at the type level

Severity: INFO
Type: (positive finding)
Sources: spec/04-resolution.md NEG-RESOLUTION-001 ("merge has no Schema parameter") vs store.rs:651 (`pub fn merge(&mut self, other: &Store) -> MergeReceipt`)
Evidence: The `merge` function signature is `pub fn merge(&mut self, other: &Store) -> MergeReceipt`. It has no Schema parameter and performs pure set union (BTreeSet insertion). The function body at store.rs:652-656 inserts datoms without any resolution logic. The guide's claim "fn merge(&mut self, other: &Store) -- no schema access possible at the type level" is verified.
Impact: None (positive finding). This is the strongest enforced negative case in the domain.

---

### FINDING-018: ADR-RESOLUTION-009 (BLAKE3 tie-breaking) is correctly implemented

Severity: INFO
Type: (positive finding)
Sources: spec/04-resolution.md ADR-RESOLUTION-009 vs resolution.rs:178-192
Evidence: The `resolve_lww` function at resolution.rs:178-192 correctly implements BLAKE3 tie-breaking: `tx1.cmp(tx2).then_with(|| { let h1 = blake3::hash(&serde_json::to_vec(v1)...); let h2 = blake3::hash(&serde_json::to_vec(v2)...); h1.as_bytes().cmp(h2.as_bytes()) })`. This matches the spec's prescription. Tests at resolution.rs:537-557 verify determinism.
Impact: None (positive finding). LWW resolution with BLAKE3 tie-breaking is correctly implemented and tested.

---

### FINDING-019: SEED.md Axiom 3 is about snapshots, not schema-as-data

Severity: LOW
Type: MISALIGNMENT
Sources: CLAUDE.md claims "SEED.md S4 Axioms 3 and 5: schema-as-data, per-attribute resolution" vs SEED.md:142-146
Evidence: SEED.md S4 Axiom 3 reads: "**Snapshots.** Default query mode is the local frontier..." Axiom 5 reads: "**Resolution.** Each attribute declares a resolution mode..." Schema-as-data is described in the prose at SEED.md:122 ("The schema emerges from usage, defined as data in the store itself") and in constraint C3, but it is not Axiom 3. The spec correctly traces INV-SCHEMA-001 to "SEED S4, C3" -- the audit instructions in the task contained the error.
Impact: No implementation impact; just an incorrect cross-reference in the task instructions.

---

### FINDING-020: Spec INV-SCHEMA-009 (Spec Dependency Graph Completeness) added but not in guide

Severity: LOW
Type: GAP
Sources: spec/02-schema.md INV-SCHEMA-009 (lines 699-734) vs guide/02-schema.md (absent)
Evidence: INV-SCHEMA-009 requires that every spec element with a prose dependency has a corresponding `:spec/depends-on` ref datom. It was added to the spec but is not mentioned in the guide. The guide's implementation checklist at lines 374-384 does not include dependency graph validation. The schema module does not implement dependency completeness checking.
Impact: This invariant is Stage 0 but has no implementation path described in the guide. It is essentially orphaned between spec and guide.

---

### FINDING-021: Spec says `:impl/implements` is Cardinality::Many with multi resolution; code says Cardinality::One with LWW

Severity: MEDIUM
Type: DIVERGENCE
Sources: spec/02-schema.md L1 attribute table (line 206: `:impl/implements Ref :many multi`) vs schema.rs:860-864
Evidence: The spec's Layer 1 attribute table declares `:impl/implements` as `Ref, :many, multi`. The code at schema.rs:860 uses the `attr()` helper which sets `Cardinality::One` and `ResolutionMode::Lww`. The attribute definition is: `attr(":impl/implements", ValueType::Ref, Cardinality::One, ...)`.
Impact: An implementation entity can only link to a single spec element in the code, whereas the spec allows multiple spec elements per implementation entity. This restricts the expressiveness of the trilateral coherence model.

---

## Quantitative Summary

### SCHEMA Domain

| Metric | Count |
|--------|-------|
| Total INVs | 9 (INV-SCHEMA-001 through 009) |
| Implemented | 5 (001, 002 [count differs], 003, 004 [partial], 005) |
| Partially Implemented | 2 (004 lacks cardinality/retraction check; 006 tested but no runtime API) |
| Unimplemented | 2 (007 lattice completeness, 008 diamond signal [Stage 2], 009 dependency graph) |
| Divergent | 1 (002: count 18/17 in spec vs 19 in code) |
| Total ADRs | 8 |
| Reflected in code | 5 (001, 002 [partially], 003, 005, 006) |
| Drifted | 2 (002 count mismatch, 004 lattice definitions not stored) |
| Stage 2+ | 1 (008) |
| Total NEGs | 3 |
| Enforced | 2 (001 from_datoms only constructor, 002 no remove method) |
| Partially enforced | 1 (003 no circular deps verified by tests, not runtime) |

### RESOLUTION Domain

| Metric | Count |
|--------|-------|
| Total INVs | 8 (INV-RESOLUTION-001 through 008) |
| Implemented | 3 (001 per-attribute from schema, 002 commutativity, 005 LWW semilattice) |
| Partially Implemented | 2 (003 conservative detection [no frontier], 004 [missing causal check]) |
| Unimplemented | 3 (006 lattice join, 007 three-tier routing, 008 conflict lifecycle) |
| Divergent | 1 (004: missing causal independence check) |
| Total ADRs | 13 |
| Reflected in code | 3 (001, 002 query-time resolution, 009 BLAKE3 tie-breaking) |
| Stage 2+ | 7 (005-008, 010-012) |
| Stage 0 deferred | 1 (013 progressive activation -- not implemented even as stubs) |
| Drifted | 2 (003 no frontier, 004 no routing) |
| Total NEGs | 3 |
| Enforced | 1 (001 merge has no schema parameter -- type-level enforcement) |
| Partially enforced | 1 (003 provenance: conflict_to_datoms exists but uses unregistered attrs) |
| Unverifiable | 1 (002 no false negatives -- requires Stateright model) |

---

## Domain Health Assessment

**Strongest aspect:** The foundational schema-as-data architecture (INV-SCHEMA-001) is genuinely implemented -- schema IS derived from datoms, there is no external DDL, the `from_datoms` constructor is the sole pathway, and merge correctly rebuilds the schema. The append-only monotonicity property (INV-SCHEMA-003) is well-tested with proptest including full semilattice witness (closure, commutativity, associativity, idempotency, monotonicity). NEG-RESOLUTION-001 (no merge-time resolution) is enforced at the type level. These are real structural guarantees, not process obligations.

**Most concerning gap:** The lattice resolution pathway is structurally broken across all layers. The `ResolutionMode::Lattice` variant lacks the `lattice_id` field required by spec. `Schema::from_datoms` cannot extract lattice definitions. There is no `LatticeDef` type, no `validate_lattice_completeness`, no `verify_semilattice`, no `LwwClock`, no `RoutingTier`, and no `route_conflict`. The combined effect is that the entire non-LWW, non-multi resolution pathway is a dead branch that silently degrades to LWW. This affects INV-SCHEMA-007, INV-SCHEMA-008, INV-RESOLUTION-006, INV-RESOLUTION-007, and the downstream delegation mechanisms (ADR-RESOLUTION-006 through 008). This is not merely "Stage 2 deferred" -- the spec marks INV-SCHEMA-007 and INV-RESOLUTION-006 as Stage 0, yet the type-level prerequisites for their implementation are absent.

The second most concerning gap is FINDING-009: the `has_conflict` function does not check causal independence, causing it to report false conflicts on sequential updates. This undermines the formal correctness of INV-RESOLUTION-004, which is the foundation of the entire conflict detection subsystem.
