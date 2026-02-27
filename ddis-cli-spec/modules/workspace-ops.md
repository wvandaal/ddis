---
module: workspace-ops
domain: workspace
maintains: [APP-INV-037, APP-INV-038, APP-INV-039, APP-INV-040, APP-INV-057, APP-INV-060]
interfaces: [APP-INV-001, APP-INV-002, APP-INV-003, APP-INV-006, APP-INV-009, APP-INV-010, APP-INV-015, APP-INV-016, APP-INV-017, APP-INV-018, APP-INV-020, APP-INV-025]
implements: [APP-ADR-026, APP-ADR-027, APP-ADR-028, APP-ADR-029, APP-ADR-044, APP-ADR-047]
adjacent: [parse-pipeline, query-validation, lifecycle-ops, code-bridge, auto-prompting]
negative_specs:
  - "Must NOT modify files outside the workspace root during init"
  - "Must NOT require all peer specs to be present for local validation"
  - "Must NOT block Level 1 validation on missing Level 3 content"
  - "Must NOT couple workspace specs — each spec must be independently parseable"
  - "Must NOT generate tasks without mechanical derivation from the artifact map"
  - "Must NOT treat validation failure as binary — report maturity level and next steps"
  - "Must NOT require external tools (br, bv) for task generation core — only for output format"
---

# Workspace Operations Module

This module owns workspace initialization (`ddis init`), multi-domain composition (`ddis spec add/list`, cross-spec drift), mechanical task generation from discovery artifacts (`ddis tasks`), and progressive validation. It answers: **how do you start a project, how do multiple specs coexist, and how do discovery artifacts become implementation work?**

The architectural principle: **workspace operations are isolated, deterministic, and progressive.** Init confines all writes to the workspace root. Each spec parses independently --- removing one never prevents parsing of others. Task derivation is a pure function from artifact type to task template. Validation guides rather than gatekeeps, grouping checks into maturity levels with actionable next-steps.

```
Level 2 (Growing) requirements --- specific components:

Invariant components at Level 2 (minimum 2 of 4):
  Required: statement (plain language) + semi_formal (pseudo-code predicate)
  Optional at Level 2: violation_scenario, validation_method

  WHY these two? Statement defines WHAT the invariant claims. Semi-formal defines
  HOW to test it. Together they are sufficient for discovery and drift measurement.
  Violation scenarios and validation methods add depth but are not needed for the
  spec to be functional.

ADR subsections at Level 2 (minimum 3 of 5):
  Required: Problem + Options + Decision
  Optional at Level 2: Consequences, Tests

  WHY these three? Problem defines the design question. Options show alternatives
  were considered. Decision records the choice with rationale. Consequences and
  tests add implementation guidance but are not needed for the ADR to be useful
  as a design record.
```

**Gestalt connection:** Progressive validation IS Gestalt theory applied to spec authoring. Level 1 operates at very high Degrees of Freedom --- just get the shape right. Level 2 narrows DoF --- add structure. Level 3 is low DoF --- every detail specified. Imposing Level 3 on a seed spec is the spec-authoring equivalent of overprompting past the k* threshold.

**Invariants interfaced from other modules (INV-018 compliance):**

- APP-INV-001: Round-Trip Fidelity (maintained by parse-pipeline). *Workspace init generates template files that must round-trip through parse-render without corruption.*
- APP-INV-002: Validation Determinism (maintained by query-validation). *Progressive validation regroups existing checks; the determinism guarantee must hold across all maturity levels.*
- APP-INV-003: Cross-Reference Integrity (maintained by query-validation). *Cross-spec references must resolve against the correct spec in a multi-spec workspace.*
- APP-INV-006: Transaction State Machine (maintained by lifecycle-ops). *Task generation events may be recorded within transactions.*
- APP-INV-009: Monolith-Modular Equivalence (maintained by parse-pipeline). *Workspace init generates modular specs; equivalence must hold.*
- APP-INV-010: Oplog Append-Only (maintained by lifecycle-ops). *Task generation events are recorded in the event stream.*
- APP-INV-015: Deterministic Hashing (maintained by parse-pipeline). *Cross-spec drift detection compares content hashes.*
- APP-INV-016: Implementation Traceability (maintained by lifecycle-ops). *Workspace commands that produce spec content must generate valid Implementation Trace annotations.*
- APP-INV-017: Annotation Portability (maintained by code-bridge). *`ddis init` generates template files with annotation examples using the portable grammar.*
- APP-INV-018: Scan-Spec Correspondence (maintained by code-bridge). *Cross-spec references generalize scan-spec correspondence across spec boundaries.*
- APP-INV-020: Event Stream Append-Only (maintained by code-bridge). *`ddis init` creates the event stream files; monotonicity and immutability guarantees hold from creation.*
- APP-INV-025: Discovery Provenance Chain (maintained by auto-prompting). *`ddis tasks` reads the provenance chain; broken provenance produces orphaned tasks.*

---

## Invariants

---

**APP-INV-037: Workspace Isolation**

*Each spec in a multi-spec workspace is independently parseable. Removing one spec must not prevent parsing of others. Cross-spec references that become unresolvable are warnings, not errors. `ddis init` confines all writes to the workspace root.*

```
FOR ALL specs s IN workspace:
  ddis_parse(s).success = true  (independently of other specs)

FOR ALL spec_pairs (a, b) IN workspace WHERE a references b:
  IF b is removed:
    ddis_parse(a).success = true
    ddis_parse(a).warnings INCLUDES "unresolved cross-spec reference to b"

FOR ALL paths p WRITTEN BY ddis_init(root):
  p STARTS_WITH root  (no writes outside workspace root)
```

Violation scenario: A project has two specs: `api-spec` and `data-spec`. `api-spec` references `DATA-INV-003` from `data-spec`. A developer removes `data-spec` from the workspace. `ddis parse api-spec/manifest.yaml` fails with "cannot resolve DATA-INV-003" --- cross-spec resolution is not fault-tolerant, blocking the developer until `data-spec` is restored.

Validation: Create a workspace with 2 specs, each referencing the other. Remove one spec. Verify the remaining spec parses with unresolved cross-spec references reported as warnings. Re-add the spec. Verify all references resolve. Run `ddis init` and verify no files are created outside the specified root directory.

// WHY THIS MATTERS: Workspace isolation prevents cascading failures. In a multi-team project, one team's spec being temporarily broken must not block other teams. Init isolation prevents accidental modification of files outside the project.

**Confidence:** structurally-verified

---

**APP-INV-038: Cross-Spec Reference Integrity**

*In a multi-spec workspace, cross-spec references resolve correctly. A reference to `[OtherSpec]:INV-NNN` resolves to the target invariant in OtherSpec's index. Broken references appear in validation reports. Cross-spec drift is measurable: if OtherSpec's element changes, referencing specs are flagged.*

```
FOR ALL cross_spec_refs r IN workspace:
  r.source_spec != r.target_spec
  IF r.target EXISTS in target_spec.index:
    r.resolved = true
    r.target_content_hash = target.content_hash  (stored at resolution time)
  ELSE:
    r IN validation_report.warnings

FOR ALL elements e IN workspace WHERE e.has_external_dependents:
  IF e.content_hash changes:
    e.dependents.flagged_for_review = true
```

Violation scenario: `api-spec` references `[ddis-modular]:INV-006`. The meta-spec team amends INV-006, changing its validation method. `api-spec`'s reference still resolves (same ID) but the semantics have changed. No warning is raised. The API team implements against an outdated definition.

Validation: Create a workspace with 2 specs. Add a cross-spec reference from spec A to spec B's INV-001. Modify INV-001 in spec B. Run `ddis drift --workspace`. Verify the stale reference is flagged with both stored and current hashes.

// WHY THIS MATTERS: Cross-spec references are contracts between teams. A reference that silently points to changed content is a broken contract.

**Confidence:** structurally-verified

---

**APP-INV-039: Task Derivation Completeness**

*Every artifact in a discovery artifact map generates the appropriate tasks per the derivation rules. No artifact is silently skipped. The rules are mechanical and deterministic --- a pure function from artifact type and action to task template.*

```
FOR ALL entries e IN artifact_map:
  LET expected_tasks = derive_tasks(e.type, e.action)
  THEN:
    |actual_tasks GENERATED FROM e| = |expected_tasks|
    AND FOR ALL t IN expected_tasks: t IN actual_tasks
```

Violation scenario: A discovery session crystallizes 3 invariants and 2 ADRs. `ddis tasks` processes the artifact map but generates only the constraint task, not the property test task. 3 invariants produce 3 tasks instead of 6. Missing property tests mean invariants are implemented but never verified.

Validation: Create a test artifact map with 2 ADRs (2 tasks each), 2 invariants (2 tasks each), 1 negative spec (2 tasks), 1 amendment (2 tasks), 1 glossary entry (1 task), 1 performance budget (2 tasks), 1 worked example (1 task). Run `ddis tasks`. Verify exactly 14 tasks with correct types. Verify no artifact in the input is absent from the output.

// WHY THIS MATTERS: Task generation is the bridge between discovery and implementation. A bridge that silently drops artifacts means some decisions are never implemented and some invariants are never tested.

**Confidence:** structurally-verified

---

**APP-INV-040: Progressive Validation Monotonicity**

*Validation maturity levels are strictly ordered: Level 1 (Seed) subset Level 2 (Growing) subset Level 3 (Complete). A spec that passes Level N also passes all levels below N.*

```
FOR ALL specs s:
  IF validate(s, level=3).pass = true:
    validate(s, level=2).pass = true AND validate(s, level=1).pass = true
  IF validate(s, level=2).pass = true:
    validate(s, level=1).pass = true

FOR ALL levels L1 < L2:
  checks(L1) SUBSET_OF checks(L2)
  AND thresholds(L1) <= thresholds(L2)
```

Violation scenario: A spec passes Level 2 (invariants have statement + violation scenario) but fails Level 1 (no overview section). This violates monotonicity --- Level 1 is supposed to be easier than Level 2. Reporting "Level 2 passed, Level 1 failed" is incoherent.

Validation: Create 3 test specs: one passing only Level 1, one passing Levels 1-2, one passing all 3. Run `ddis validate --level N` for each. Verify monotonicity holds: no spec passes a higher level without passing all lower levels.

// WHY THIS MATTERS: Monotonicity is what makes levels meaningful --- they represent genuine progression, not arbitrary groupings. Without it, levels mislead the user about spec maturity.

**Confidence:** structurally-verified

---

## Architecture Decision Records

---

### APP-ADR-026: Full Workspace Init at Phase 12

#### Problem

When should `ddis init` be introduced, and how comprehensive should it be?

#### Options

A) **Partial init at Phase 10** --- create manifest and DB only.
- Pros: Lower implementation cost.
- Cons: Incomplete scaffolding requiring retroactive extension. Users need to manually add event streams, discovery directories, and workspace infrastructure later.

B) **Full workspace init at Phase 12** --- comprehensive scaffolding including manifest, DB, event streams, discovery directory, `.gitignore`, workspace infrastructure.
- Pros: One command creates everything. No "upgrade" step. First-run experience is complete.
- Cons: Larger implementation scope.

#### Decision

**Option B: Full workspace init at Phase 12.** `ddis init` creates the complete project scaffolding in one command. The generated manifest template passes Level 1 validation immediately: constitution template includes a placeholder overview (>50 words), one template invariant with title and statement, and one template ADR with title, problem, and decision. `ddis init && ddis parse manifest.yaml -o index.db && ddis validate index.db` succeeds on the first try.

// WHY NOT Partial init? A partial init that requires follow-up commands signals "this tool is not ready." First impressions determine adoption.

#### Consequences

- `ddis init [--workspace] [--skeleton <level>]` creates complete project scaffolding
- `ddis spec add <manifest-path>` loads additional specs into workspace
- `ddis spec list` shows loaded specs with status, parent relationships, drift scores
- Generated template passes Level 1 validation without modification

#### Tests

- Run `ddis init` in an empty directory; verify all scaffolding files exist
- Run `ddis init --workspace`; verify workspace infrastructure created
- Run `ddis parse` on the generated manifest; verify it validates at Level 1
- Run `ddis init` twice; verify idempotent (no error, no file clobbering)

---

### APP-ADR-027: Peer Spec Relationships

#### Problem

The current `parent_spec` field supports only parent-child hierarchies. Real projects have peer relationships (e.g., API spec and data spec at the same level).

#### Options

A) **Parent-child only** --- all relationships are hierarchical.
- Pros: Simpler resolution logic.
- Cons: Semantically incorrect for independent specifications that reference each other.

B) **Peer relationships** --- extend manifest with `related_specs` array. Support diamond dependencies.
- Pros: Accurate modeling. Diamond dependencies supported.
- Cons: Ambiguity when multiple related specs define the same ID.

#### Decision

**Option B: Peer relationships.** Manifest gains a `related_specs` array alongside `parent_spec`. Cross-spec references resolve local -> parent -> related (declaration order). Diamond dependencies supported.

```yaml
parent_spec: "../ddis-modular/manifest.yaml"
related_specs:
  - "../api-spec/manifest.yaml"
  - "../data-spec/manifest.yaml"
```

// WHY NOT Parent-child only? The CLI spec and DDIS meta-spec are already peer domains that need composition. Forcing one to be "parent" is semantically incorrect.

#### Consequences

- Manifest schema extended: `related_specs: ["../other-spec/manifest.yaml"]`
- Cross-ref resolution order: local -> parent -> related (first match wins)
- `ddis drift --workspace` measures drift across all spec relationships
- Fully-qualified references (`[spec-name]:ELEMENT-ID`) bypass ambiguity resolution

#### Tests

- Create 2 peer specs with cross-references; verify correct resolution
- Create a diamond dependency; verify no ambiguity
- Remove a peer spec; verify workspace isolation (APP-INV-037) holds
- Create 2 specs with identically-named elements; verify ambiguity warning emitted
- Verify fully-qualified reference resolves when ambiguous ID exists in multiple specs

---

### APP-ADR-028: Progressive Validation over Binary Pass/Fail

#### Problem

`ddis validate` reports pass/fail per check. A freshly-initialized spec fails most checks, which is discouraging and uninformative.

#### Options

A) **Binary** --- pass or fail per check.
- Pros: Simple. Clear boundary.
- Cons: A freshly-initialized spec "failing" 11 of 12 checks conveys no useful information.

B) **Progressive** --- group checks into maturity levels (Seed/Growing/Complete). Report current level and next steps.
- Pros: Encouraging first-run experience. Actionable guidance. CI can gate on chosen level.
- Cons: Must ensure monotonicity.

#### Decision

**Option B: Progressive validation.** Existing checks regrouped into three maturity tiers. Validation never says "FAIL" --- it says "here's where you are and what's next." `ddis validate --level 2` exits 0 if Level 2 passes.

// WHY NOT Binary? A freshly-initialized spec "failing" 11 of 12 checks is discouraging. The spec author needs to know what to do next, not that they haven't done everything yet.

#### Consequences

- `ddis validate` output restructured with level grouping and guidance
- `--level <N>` flag for CI gating at chosen maturity
- No new checks --- same 12-13 checks reorganized into progressive tiers

#### Tests

- Freshly-initialized spec: verify Level 1 passes
- Spec with overview + 3 invariants (statement + validation_method only): verify Level 2 passes, Level 3 does not
- Full CLI spec: verify all 3 levels pass
- `ddis validate --level 2 spec.db`: verify exit code 0 when Level 2 passes

---

### APP-ADR-029: Beads-Compatible Task Output

#### Problem

`ddis tasks` generates tasks from discovery artifact maps. What format should the output use?

#### Options

A) **JSON only** --- generic, no external tool dependency.
B) **Beads-compatible JSONL** --- default matches `br import` format. Also supports JSON and markdown.
C) **Markdown only** --- human-readable but not machine-importable.

#### Decision

**Option B: Beads-compatible JSONL as default.** `ddis tasks` defaults to beads-compatible JSONL for direct import via `br import`. Also supports `--format json` and `--format markdown`. Task dependencies derived from `implementation_map.phases`.

// WHY NOT JSON only? Beads is the project's issue tracking system. Beads-compatible output eliminates a manual conversion step.

#### Consequences

- Default: `ddis tasks --format beads` outputs JSONL compatible with `br import`
- Each JSONL line includes: `{id, title, type, priority, labels, acceptance, depends_on, source_artifact}`
- `br` is not required --- beads format is just JSONL with specific fields

#### Tests

- Generate tasks from test artifact map; verify JSONL importable via `br import`
- Generate tasks with `--format markdown`; verify human-readable output
- Generate tasks with dependencies; verify edges match `implementation_map.phases`
- Generate tasks from all 8 artifact types; verify task count matches derivation rules

---

## Implementation Chapters

### Chapter: Workspace Initialization

**Preserves:** APP-INV-037 (Workspace Isolation --- init confines all writes to workspace root), APP-INV-001 (Round-Trip Fidelity --- generated templates must round-trip through parse-render).

**Interfaces:** APP-INV-020 (Event Stream Append-Only --- init creates the stream files that lifecycle-ops and code-bridge manage).

The initialization subsystem creates a complete DDIS workspace from an empty directory. Every file needed for parsing, validation, search, discovery, and event sourcing is created in one command.

#### Init Scaffolding Layout

```
ddis init creates:                         ddis init --workspace adds:
  <root>/                                    <root>/.ddis/workspace.yaml
  |- manifest.yaml                             (loaded_specs, relationships)
  |- constitution/system.md  (Level 1)
  |- modules/                (empty)
  |- .ddis/
  |  |- index.db             (schema init)
  |  |- oplog.jsonl          (empty)
  |  |- events/
  |  |  |- stream-1.jsonl    (discovery — empty)
  |  |  |- stream-2.jsonl    (specification — empty)
  |  |  +-- stream-3.jsonl   (implementation — empty)
  |  +-- discoveries/
  +-- .gitignore             (ignores .ddis/index.db)
```

#### File Creation Algorithm

```
Algorithm: WorkspaceInit
Input: root (directory path), options {workspace: bool, skeleton_level: int}
Output: list of created files

1. Resolve root to absolute path: root = filepath.Abs(root)
2. FOR EACH file in creation_order:
     abs_path = filepath.Clean(filepath.Join(root, relative_path))
     IF NOT strings.HasPrefix(abs_path, root):
       RETURN error("path escapes workspace root")  // path traversal guard
     IF file_exists(abs_path):
       SKIP (idempotency --- never overwrite existing files)
     ELSE:
       create_file(abs_path, template_content)
3. IF options.workspace:
     create_file(root/.ddis/workspace.yaml, workspace_template)
4. Initialize SQLite schema in root/.ddis/index.db
5. Append .gitignore patterns (if not already present)
6. RETURN list of created files
```

// WHY THIS MATTERS: Idempotency means running `ddis init` twice does not corrupt existing data. Path confinement prevents a crafted `relativePath` from writing outside the workspace root.

#### Constitution Skeleton (Level 1)

The Level 1 skeleton provides the minimum content to pass Level 1 validation: YAML frontmatter, an overview section (>50 words placeholder), one invariant (title + statement), and one ADR (title + problem + decision). With `--skeleton 3`, the template includes all invariant components and all ADR subsections.

```markdown
---
module: system-constitution
domain: system
tier: 1
---

# [Spec Name]

## Overview

[This specification defines... Provide at least 50 words.]

## Invariants

INV-NNN: [Title]

[Statement describing the invariant in plain language.]

## Architecture Decision Records

ADR-NNN: [Title]

#### Problem
[What problem does this decision address?]

#### Decision
[What was decided and why?]
```

**Implementation Trace:**
- Source: `internal/workspace/init.go::Init`
- Source: `internal/workspace/init.go::createManifest`
- Source: `internal/workspace/init.go::createConstitution`
- Source: `internal/workspace/init.go::initializeDB`
- Source: `internal/workspace/init.go::updateGitignore`
- Tests: `tests/workspace_init_test.go::TestInitEmptyDir`
- Tests: `tests/workspace_init_test.go::TestInitIdempotent`
- Tests: `tests/workspace_init_test.go::TestInitPathConfinement`

---

### Chapter: Multi-Spec Management

**Preserves:** APP-INV-037 (Workspace Isolation --- each spec parses independently), APP-INV-038 (Cross-Spec Reference Integrity --- references resolve across spec boundaries with stale detection).

**Interfaces:** APP-INV-015 (Deterministic Hashing --- content hashes for cross-spec drift must be reproducible).

The multi-spec subsystem manages peer relationships between specifications, resolves cross-spec references, and detects stale references via content hash tracking.

#### Cross-Spec Resolution Algorithm

```
Algorithm: Cross-Spec Reference Resolution
Input: reference (string), local_index (DB), parent_index (DB?), related_indices ([]DB)
Output: ResolvedRef | UnresolvedWarning

Resolution order (first match wins):
1. Local index: query reference against local spec_index
2. Parent index: if parent_spec configured, query parent's index
3. Related indices: iterate related_specs in manifest order, query each
4. Unresolved: return warning (not error --- APP-INV-037)

For each resolution attempt:
  - Match by canonical ID (e.g., "INV-006" -> spec_index WHERE element_id = "INV-006")
  - Match by qualified ID (e.g., "[ddis-modular]:INV-006" -> specific spec's index)
  - Record: {source_spec, source_element, target_spec, target_element, resolution_method}

Ambiguity resolution:
  IF multiple related specs define element with same ID:
    WARNING: "Ambiguous reference: {id} found in {spec_a} and {spec_b}"
    Resolution: first match by declaration order in manifest.related_specs array
    User override: fully-qualified reference [spec-name]:ELEMENT-ID always unambiguous

Content hash tracking for stale detection:
  - On first resolution: store target.content_hash in cross_spec_refs table
  - On subsequent resolution: compare current hash with stored hash
  - If hash changed: flag as stale in drift report (APP-INV-038)
```

#### Cross-Spec Drift Detection

On `ddis drift --workspace`, each cross-spec reference's stored `target_content_hash` (recorded at resolution time) is compared against the current hash in the target spec's index. Mismatches produce `StaleReference` warnings identifying source, target, both hashes, and the resolution action (re-parse the referencing spec to update stored hashes).

#### Worked Example: Adding a Peer Spec and Detecting Drift

```
$ ddis spec add ../api-spec/manifest.yaml
Loaded spec: api-spec (42 invariants, 15 ADRs)
Cross-spec references: 10 total, all resolved.

# Another team modifies API-INV-003 in api-spec

$ ddis drift --workspace
Stale references (1):
  current-spec:S3.2 -> [api-spec]:API-INV-003
    Stored hash: a1b2c3d4...  Current hash: e5f6a7b8...
    Action: re-parse current spec to update stored hashes
Healthy references (9): all current.
```

**Implementation Trace:**
- Source: `internal/workspace/spec.go::AddSpec`
- Source: `internal/workspace/spec.go::ListSpecs`
- Source: `internal/workspace/crossref.go::ResolveCrossSpecRefs`
- Source: `internal/workspace/crossref.go::DetectCrossSpecDrift`
- Tests: `tests/workspace_spec_test.go::TestAddPeerSpec`
- Tests: `tests/workspace_spec_test.go::TestCrossSpecDrift`
- Tests: `tests/workspace_spec_test.go::TestRemovePeerSpecIsolation`

---

### Chapter: Task Generation

**Preserves:** APP-INV-039 (Task Derivation Completeness --- every artifact produces the correct tasks), APP-INV-025 (Discovery Provenance Chain --- tasks trace back to discovery decisions).

**Interfaces:** APP-INV-010 (Oplog Append-Only --- task generation events are recorded in event streams).

The task derivation engine converts a discovery artifact map into implementation tasks. The engine is a pure function: given the same artifact map, it produces the same tasks.

#### Artifact Map Reduction Algorithm

```
Algorithm: Reduce Discovery JSONL to Current State
Input: discovery_jsonl_path (string)
Output: DiscoveryState {artifact_map, confidence_vector, open_questions, threads}

1. Read all events from JSONL, sorted by timestamp
2. Initialize empty state
3. For each event e:
   Switch on e.type:
     "finding_recorded"      -> state.findings[e.data.id] = e.data
     "question_opened"       -> state.open_questions[e.data.id] = e.data
     "question_resolved"     -> delete(state.open_questions[e.data.id])
     "decision_crystallized" -> state.artifact_map[e.data.artifact_id] = e.data
     "artifact_amended"      -> state.artifact_map[e.data.artifact_id].amendments.append(e.data)
     "artifact_deleted"      -> state.artifact_map[e.data.artifact_id].status = "deleted"
     "thread_created"        -> state.threads[e.thread_id] = {status: "active"}
     "thread_parked"         -> state.threads[e.thread_id].status = "parked"
     "thread_merged"         -> state.threads[e.thread_id].status = "merged"
4. Compute confidence_vector from artifact completeness
5. Return state

Correctness property: reduction is idempotent --- reducing the same JSONL twice
produces the same artifact map. Follows from chronological ordering and last-write-wins.
```

#### Task Derivation Rules (8 rules)

```
Algorithm: Task Derivation from Artifact Map
Input: artifact_map ([]ArtifactMapEntry), implementation_map (optional)
Output: tasks ([]Task)

RULE 1 --- ADR created:
  -> Task: "Implement {adr.title}"
  -> Acceptance: adr.tests (if present) OR "ADR consequences realized"
  -> Priority: P2 | Labels: ["implementation", adr.domain]

RULE 2 --- Invariant created:
  -> Task A: "Implement constraint: {inv.title}"
  -> Task B: "Property test: {inv.title}"
  -> Acceptance A: "inv.validation_method passes"
  -> Acceptance B: "Test exists that triggers violation scenario"
  -> Priority: P1 | Labels: ["constraint", inv.domain]

RULE 3 --- Negative spec created:
  -> Task A: "Guard: {neg_spec.text}"
  -> Task B: "Regression test: {neg_spec.text}"
  -> Priority: P2 | Labels: ["guard", neg_spec.domain]

RULE 4 --- Glossary entry created:
  -> Task: "Add to glossary: {term}" (single task, no test)
  -> Priority: P3 | Labels: ["documentation"]

RULE 5 --- Gate created:
  -> Task: "Gate integration: {gate_name}"
  -> Priority: P2 | Labels: ["gate"]

RULE 6 --- Amendment to existing:
  -> Task A: "Update implementation of {id}: {change}"
  -> Task B: "Update tests for {id}"
  -> Priority: P1 | Labels: ["amendment", artifact.domain]

RULE 7 --- Deletion of existing:
  -> Task A: "Remove implementation of {id}"
  -> Task B: "Remove tests for {id}"
  -> Task C: "Verify no orphan references to {id}"
  -> Priority: P2 | Labels: ["removal"]

RULE 8 --- Cross-spec reference created:
  -> Task: "Verify cross-spec contract: {source} -> {target}"
  -> Priority: P2 | Labels: ["cross-spec"]

Each task includes:
  - title: derived from rule template
  - type: "task" | "test"
  - priority: P1-P3 from rule
  - labels: [artifact.type, artifact.module]
  - dependencies: from implementation_map.phases if provided
  - acceptance_criteria: from artifact's tests/validation_method field
  - provenance: {artifact_id, derivation_rule, discovery_session}

Dependencies:
  - Tasks from same phase: no inter-dependencies (parallel execution)
  - Tasks from phase N+1: depend on ALL tasks from phase N
  - Within a phase: invariant tasks depend on their ADR's implementation task
```

#### implementation_map.phases Format

```yaml
phases:
  - id: "phase-1"
    title: "Core Infrastructure"
    artifacts:
      - { id: "APP-ADR-001", type: "adr", action: "implement" }
      - { id: "APP-INV-001", type: "invariant", action: "implement" }
    depends_on: []

  - id: "phase-2"
    title: "Search & Context"
    artifacts:
      - { id: "APP-ADR-003", type: "adr", action: "implement" }
      - { id: "APP-INV-005", type: "invariant", action: "implement" }
    depends_on: ["phase-1"]

Task dependencies derive from phase dependencies:
  phase-2 depends on phase-1 -> all tasks from phase-2 blocked by phase-1 tasks
```

#### Worked Example: Task Derivation

Given 2 ADRs and 3 invariants across 2 phases, `ddis tasks --from-discovery session-001.jsonl --format beads` produces 8 JSONL lines (showing first 3):

```jsonl
{"title":"Implement: Go over Rust","type":"task","priority":2,"labels":["implementation"],"acceptance":"ADR consequences realized","depends_on":[],"metadata":{"source_artifact":"APP-ADR-001","phase":"phase-1"}}
{"title":"Implement constraint: Round-Trip Fidelity","type":"task","priority":1,"labels":["constraint"],"acceptance":"inv.validation_method passes","depends_on":["TASK-APP-ADR-001-impl"],"metadata":{"source_artifact":"APP-INV-001","phase":"phase-1"}}
{"title":"Property test: Round-Trip Fidelity","type":"test","priority":1,"labels":["constraint"],"acceptance":"Test triggers violation scenario","depends_on":["TASK-APP-ADR-001-impl"],"metadata":{"source_artifact":"APP-INV-001","phase":"phase-1"}}
```

**Derivation trace:** APP-ADR-001 (Rule 1, 1 task) + APP-INV-001 (Rule 2, 2 tasks) + APP-ADR-003 (Rule 1, 1 task) + APP-INV-005 (Rule 2, 2 tasks) + APP-INV-008 (Rule 2, 2 tasks) = 8 tasks. Phase-2 tasks inherit `depends_on` from phase-1 completion.

**Implementation Trace:**
- Source: `internal/discovery/events.go::ReduceToState`
- Source: `internal/discovery/artifacts.go::ExtractArtifactMap`
- Source: `internal/discovery/tasks.go::DeriveTasks`
- Source: `internal/discovery/tasks.go::applyRule`
- Source: `internal/discovery/tasks.go::FormatBeads`
- Source: `internal/discovery/tasks.go::FormatMarkdown`
- Tests: `tests/discovery_tasks_test.go::TestDeriveTasksAllRules`
- Tests: `tests/discovery_tasks_test.go::TestDeriveTasksDependencies`
- Tests: `tests/discovery_tasks_test.go::TestReduceIdempotent`

---

### Chapter: Onboarding Flow

**Preserves:** APP-INV-042 (Guidance Emission --- ddis next always recommends a concrete action).

**Interfaces:** APP-INV-037 (Workspace Isolation --- init creates an isolated workspace).

The onboarding flow ensures that `ddis next` provides correct guidance at every stage of a cold start, from an empty directory to a fully parsed spec.

#### Cold-Start Detection

When `ddis next` runs, it checks three conditions in order:

1. **No manifest.yaml AND no .ddis/ directory** --- the user has never initialized. Suggest `ddis init`.
2. **manifest.yaml exists but no .ddis/\*.db** --- the user has a spec but hasn't parsed. Suggest `ddis parse manifest.yaml`.
3. **Database exists** --- delegate to the standard priority pyramid (validation > coverage > drift > challenges).

This 3-stage detection replaces the previous behavior where a missing database always suggested `ddis parse`, even when no manifest existed.

**Implementation Trace:**
- Source: `internal/cli/next.go::runNext`

---

### Chapter: Progressive Validation

**Preserves:** APP-INV-040 (Progressive Validation Monotonicity --- levels are strictly ordered subsets), APP-INV-002 (Validation Determinism --- level assignment is deterministic).

**Interfaces:** APP-INV-011 (Check Composability --- progressive validation groups compose existing checks without modifying their behavior).

The progressive validation engine wraps the existing 12+ validation checks with a maturity-level overlay. It does not introduce new checks --- it regroups existing checks into levels and transforms output from binary pass/fail to maturity guidance.

#### Check-to-Level Mapping

| Check | Name | Level 1 | Level 2 | Level 3 |
|-------|------|---------|---------|---------|
| G1 | Structural conformance | required | required | required |
| NS | Namespace consistency | required | required | required |
| C1 | Cross-reference resolution | skip | required | required |
| C2 | Validation determinism | skip | required | required |
| C3 | Gate coverage | skip | skip | required |
| C4 | Glossary completeness (>=80%) | skip | required | required |
| C5 | Verification prompt coverage | skip | required | required |
| C6 | Negative spec coverage | skip | skip | required |
| C7 | Diff completeness | skip | required | required |
| C8 | Manifest-module sync | skip | required | required |
| C9 | Content quality (full) | skip | skip | required |
| C10 | Overview quality | relaxed (>50w) | relaxed | required (full) |
| C11 | Proportional weight | skip | skip | required |
| C12 | Worked example coverage | skip | required | required |
| C13 | Implementation traceability | skip | skip | required (if --code-root) |
| INV | Invariant completeness | title+statement | stmt+semi_formal | all 4 components |
| ADR | ADR completeness | title+problem+decision | prob+opts+decision | all 5 subsections |

#### Level Evaluation Algorithm

```
Algorithm: EvaluateMaturityLevel
Input: check_results (from validator.Validate), spec_stats
Output: {achieved_level, next_steps}

1. passed_l1 = level1_checks ALL pass
2. passed_l2 = passed_l1 AND level2_checks ALL pass
3. passed_l3 = passed_l2 AND level3_checks ALL pass

4. IF passed_l3: achieved_level = 3
   ELIF passed_l2: achieved_level = 2
   ELIF passed_l1: achieved_level = 1
   ELSE: achieved_level = 0

5. IF achieved_level < 3:
     next_level = achieved_level + 1
     failing_checks = checks_at(next_level) WHERE NOT pass
     next_steps = failing_checks.map(c => guidance_text(c))
   ELSE:
     next_steps = ["All checks pass at full strictness. Ready for production."]

6. RETURN {achieved_level, next_steps}

Monotonicity proof:
  L1 checks SUBSET L2 checks SUBSET L3 checks (by definition)
  L1 thresholds <= L2 thresholds <= L3 thresholds (each level raises the bar)
  Therefore: pass(L3) IMPLIES pass(L2) IMPLIES pass(L1)  QED
```

#### Worked Example: Level 1 Spec with Guidance

```
$ ddis validate index.db
Validation Report: Level 1 (Seed) achieved
========================================

Level 1 --- Seed: PASS
  [x] Overview exists (62 words)
  [x] Invariant count >= 1 (found: 1)
  [x] ADR count >= 1 (found: 1)
  [x] Namespace consistent

Level 2 --- Growing: NOT YET
  [ ] Cross-reference resolution (Check 1): 0 references (nothing to resolve yet)
  [ ] Glossary completeness (Check 4): 0 of 0 terms (no glossary section)
  [ ] Invariant components: 0 of 1 have >= 2 components (INV-001 has statement only)
  [ ] ADR subsections: 0 of 1 have >= 3 subsections (ADR-001 has problem + decision only)

To reach Level 2:
  1. Add cross-references between invariants and ADRs
  2. Create a Glossary section with definitions for bold terms
  3. Add a semi-formal predicate to INV-001
  4. Add options to ADR-001
```

With `--level 2`: exit code 1 (Level 2 not achieved). With `--level 1`: exit code 0.

**Implementation Trace:**
- Source: `internal/validator/levels.go::EvaluateLevel`
- Source: `internal/validator/levels.go::checkToLevelMap`
- Source: `internal/validator/guidance.go::GenerateGuidance`
- Source: `internal/validator/guidance.go::guidanceText`
- Tests: `tests/progressive_validate_test.go::TestMonotonicity`
- Tests: `tests/progressive_validate_test.go::TestLevel1FreshInit`
- Tests: `tests/progressive_validate_test.go::TestLevel2Guidance`
- Tests: `tests/progressive_validate_test.go::TestLevelFlag`

---

## Negative Specifications

These constraints prevent the most likely failure modes in the workspace operations subsystem. Each addresses a specific failure mode that an LLM, given only the positive specification, would plausibly introduce.

**NEG-WORKSPACE-001: DO NOT** modify files outside the workspace root during init. `ddis init /path/to/project` must confine all file creation to `/path/to/project/` and its subdirectories. No parent directory traversal, no home directory configuration files, no global state mutation. Verify by checking that every `os.Create`, `os.MkdirAll`, and `os.WriteFile` call receives a path starting with the resolved workspace root. (Validates APP-INV-037)

**NEG-WORKSPACE-002: DO NOT** couple workspace specs --- each spec must be independently parseable. Workspace operations that load multiple specs must use separate parse passes. A failure in one spec's parse must not affect another's. The parse-pipeline (APP-INV-001) operates per-spec; cross-spec resolution is a separate, fault-tolerant pass. `ddis spec add` must not modify the added spec's files (read-only access). (Validates APP-INV-037)

**NEG-WORKSPACE-003: DO NOT** generate tasks without mechanical derivation from the artifact map. The task derivation rules are deterministic: a function from artifact type to task template. No task may be generated from inference, heuristics, or LLM analysis. No artifact in the input map may be silently skipped. (Validates APP-INV-039)

**NEG-WORKSPACE-004: DO NOT** treat validation failure as binary --- report maturity level and next steps. The `ddis validate` output for a Level 1 spec must not say "FAIL: 10 checks failed." It must say "Level 1 achieved. To reach Level 2: ..." Guidance is specific, actionable, and encouraging. (Validates APP-INV-040)

**NEG-WORKSPACE-005: DO NOT** require external tools (br, bv) for task generation core --- only for output format. The task derivation logic is self-contained. `br` is only needed for beads-compatible output format. Without it, `ddis tasks --format json` produces complete, correct tasks. (Validates APP-INV-039)

**NEG-WORKSPACE-006: DO NOT** require all peer specs to be present for local validation. A spec with `related_specs` entries pointing to missing manifests must still validate locally. Missing peer specs generate warnings, not errors. This ensures `ddis validate` works in partial checkouts and sparse clones. (Validates APP-INV-037, APP-INV-038)

**NEG-WORKSPACE-007: DO NOT** block Level 1 validation on missing Level 3 content. A spec with an overview and one invariant (statement only) must pass Level 1. Checks that evaluate invariant completeness (semi-formal, violation, validation) are Level 2+ checks. Level 1 only verifies existence, not completeness. (Validates APP-INV-040)

**NEG-WORKSPACE-008: DO NOT** allow `ddis init` to overwrite existing files. Running init in a directory that already has a manifest must preserve existing files and only create missing ones. This prevents accidental data loss when using init as a "repair" command. (Validates APP-INV-037, APP-ADR-026)

**NEG-WORKSPACE-009: DO NOT** require manual creation of module stub files after declaring modules in the manifest. The `ddis manifest scaffold` command MUST generate corresponding files with correct frontmatter. (Validates APP-INV-060)

---

## CLI Commands

### `ddis init`

**Interface**: `ddis init [--workspace] [--skeleton <level>]`

**Behavior**:
1. Create `manifest.yaml` with spec metadata template
2. Create `constitution/system.md` skeleton (Level 1 by default)
3. Create `modules/` directory
4. Create `.ddis/` directory with `index.db`, `oplog.jsonl`, `events/` (3 streams), `discoveries/`
5. Create `.gitignore` entries for derived artifacts
6. If `--workspace`: create `.ddis/workspace.yaml` for multi-spec management
7. If `--skeleton <level>`: generate template at specified maturity level (default: 1)

**Idempotency:** Running `ddis init` in a directory with an existing manifest does not overwrite existing files. Only missing files are created.

### `ddis spec`

**Interface**: `ddis spec <add|list> [args]`

- `ddis spec add <manifest-path>` --- load additional spec into workspace DB
- `ddis spec list [--json]` --- show loaded specs with status, parent/peer relationships, drift scores

### `ddis tasks`

**Interface**: `ddis tasks --from-discovery <path> [--spec <db>] [--format beads|json|markdown]`

**Behavior**:
1. Parse discovery JSONL, reduce to current state via `ReduceDiscoveryToArtifactMap`
2. Extract `artifact_map` from reduced state
3. If `--spec <db>`: cross-validate artifact IDs exist in spec DB (flag orphaned)
4. Apply derivation rules per artifact type (8 rules, see APP-INV-039)
5. Derive task dependencies from `implementation_map.phases`
6. Output in requested format (default: beads JSONL)

### `ddis validate` (progressive mode)

- Default: run all checks, group by Level 1/2/3, report highest achieved level + next steps
- `--level <N>`: exit 0 if Level N passes (CI gating)
- No new checks --- same 12-13 checks, reorganized into maturity tiers

---

## Package Structure

```
internal/workspace/
  init.go          --- ddis init scaffolding (manifest, DB, streams, gitignore)
  spec.go          --- ddis spec add/list (workspace management)
  crossref.go      --- cross-spec reference resolution and drift detection
  models.go        --- Workspace, SpecEntry, CrossSpecRef, StaleReference types

internal/discovery/
  events.go        --- JSONL event types, parsing, reduction to state
  artifacts.go     --- artifact map extraction from reduced state
  tasks.go         --- task derivation rules, dependency mapping, format output
  models.go        --- DiscoveryEvent, DiscoveryState, ArtifactMapEntry, DerivedTask types

internal/validator/
  levels.go        --- maturity level definitions, check-to-level mapping
  guidance.go      --- next-steps generation per level transition, guidance text templates
```

---

## Verification Prompt for Workspace Operations Module

After implementing or modifying the workspace operations subsystem, execute the following checks:

**Positive checks (DOES):**

1. DOES `ddis init` create a complete workspace in one command? (APP-INV-037)
2. DOES the generated manifest template parse successfully via `ddis parse`? (APP-INV-001)
3. DOES the generated spec pass Level 1 validation without modification? (APP-INV-040, APP-ADR-028)
4. DOES `ddis init` run idempotently (second run does not overwrite existing files)? (APP-ADR-026)
5. DOES `ddis spec add` correctly load a peer spec and resolve cross-references? (APP-INV-038)
6. DOES `ddis drift --workspace` detect stale cross-spec references by comparing stored and current content hashes? (APP-INV-038)
7. DOES cross-spec resolution follow the precedence order: local -> parent -> related? (APP-ADR-027)
8. DOES `ddis tasks` generate the correct number of tasks for each artifact type per the 8 derivation rules? (APP-INV-039)
9. DOES `ddis tasks` include task dependencies derived from `implementation_map.phases`? (APP-INV-039)
10. DOES `ddis validate` report maturity level and guidance text (not binary pass/fail)? (APP-INV-040)
11. DOES `ddis validate --level 2` exit 0 when Level 2 passes? (APP-ADR-028)
12. DOES progressive validation maintain monotonicity (pass(L3) implies pass(L2) implies pass(L1))? (APP-INV-040)
13. DOES beads JSONL output from `ddis tasks --format beads` import cleanly via `br import`? (APP-ADR-029)
14. DOES the artifact map reduction algorithm produce deterministic output for the same JSONL input? (APP-INV-039)

**Negative checks (does NOT):**

1. Does NOT `ddis init` create or modify files outside the workspace root directory? (NEG-WORKSPACE-001, APP-INV-037)
2. Does NOT removing a peer spec from the workspace prevent parsing of remaining specs? (NEG-WORKSPACE-002, APP-INV-037)
3. Does NOT `ddis tasks` generate any task not mechanically derived from the artifact map rules? (NEG-WORKSPACE-003, APP-INV-039)
4. Does NOT `ddis validate` output "FAIL" for a Level 1 spec that has not yet reached Level 2? (NEG-WORKSPACE-004, APP-INV-040)
5. Does NOT `ddis tasks --format json` require `br` or `bv` to be installed? (NEG-WORKSPACE-005, APP-INV-039)
6. Does NOT `ddis validate` error when `related_specs` entries point to missing manifests? (NEG-WORKSPACE-006, APP-INV-037)
7. Does NOT Level 1 validation check for semi-formal predicates or violation scenarios? (NEG-WORKSPACE-007, APP-INV-040)
8. Does NOT `ddis spec add` modify the added spec's files (read-only access)? (NEG-WORKSPACE-002, APP-INV-037)
9. Does NOT `ddis tasks` silently skip any artifact in the input map? (NEG-WORKSPACE-003, APP-INV-039)
10. Does NOT `ddis init` overwrite existing files when run a second time? (NEG-WORKSPACE-008, APP-INV-037)

---

## Referenced Invariants from Other Modules

Per the cross-module reference completeness convention, this section lists invariants owned by other modules that this module depends on or interfaces with:

| Invariant | Owner | Relationship | Usage in This Module |
|---|---|---|---|
| APP-INV-001 | parse-pipeline | interfaces | Generated templates must round-trip through parse-render |
| APP-INV-002 | query-validation | interfaces | Progressive validation level assignment must be deterministic |
| APP-INV-003 | query-validation | interfaces | Cross-spec references resolve against correct spec in workspace |
| APP-INV-006 | lifecycle-ops | interfaces | Task generation events may be recorded within transactions |
| APP-INV-009 | parse-pipeline | interfaces | Modular templates must parse equivalently to monolith form |
| APP-INV-010 | lifecycle-ops | interfaces | Event stream writes during task generation are append-only |
| APP-INV-015 | parse-pipeline | interfaces | Content hashes in cross-spec drift detection must be deterministic |
| APP-INV-016 | lifecycle-ops | interfaces | Generated spec content must produce valid Implementation Trace |
| APP-INV-017 | code-bridge | interfaces | Template annotation examples use portable grammar |
| APP-INV-018 | code-bridge | interfaces | Cross-spec references generalize scan-spec correspondence |
| APP-INV-020 | code-bridge | interfaces | Event stream files created by init honor append-only guarantee |
| APP-INV-025 | auto-prompting | interfaces | Task derivation reads provenance chain from discovery artifacts |

### APP-ADR-044: External Issue Tracker Integration via gh CLI

#### Problem

Users need to report bugs and request features for DDIS without leaving the CLI. Manually navigating to GitHub breaks flow and loses context that the CLI already has (spec version, drift state, validation status).

#### Options

A. Direct GitHub API calls via net/http (heavy, auth management). B. Thin wrapper around gh CLI (minimal code, delegates auth to gh). C. Built-in issue tracker (scope creep, duplicates GitHub).

#### Decision

**Option B: Thin wrapper around gh CLI.** Delegates auth to gh, minimal code (~50 LOC), gh already detects repo from git remote.

Wrap gh CLI. The ddis issue command shells out to gh issue create with title, body, and label flags. gh handles authentication, rate limits, and repo detection. If gh is not installed or not authenticated, ddis issue fails with a clear recovery hint rather than silently degrading.

#### Consequences

Minimal code surface (~50 LOC). Auth delegation to gh avoids storing tokens. gh detects the repo from git remote. Aligns with workspace-ops domain (external artifact integration). Does not violate APP-INV-015 since issue filing is not core task generation.

#### Tests

1. ddis issue --help prints usage without requiring gh. 2. ddis issue with missing gh binary returns error with recovery hint. 3. ddis issue with valid gh creates issue and prints URL.

---

**APP-INV-057: External Tool Graceful Degradation**

*When a ddis command depends on an external tool (e.g., gh), absence or failure of that tool must produce a clear, actionable error with recovery guidance — never a panic, silent failure, or cryptic exec error.*

```
forall cmd in ExternalToolCommands: missing(tool(cmd)) => (exit_code > 0 AND stderr contains recovery_hint(tool(cmd)))
```

Violation scenario: User runs ddis issue without gh installed. Instead of exec: gh: not found, the CLI prints: Error: gh CLI not found. Install from https://cli.github.com/ and run gh auth login.

Validation: Unit test: mock exec.LookPath returning error, verify error message contains install URL.

// WHY THIS MATTERS: External tool wrappers are only valuable if they fail gracefully. A raw exec error is worse than no wrapper at all.

---

**APP-INV-060: Manifest-Module Bijection**

*The manifest scaffold operation is the left adjoint to manifest sync. Given a manifest declaring N modules, manifest scaffold MUST generate exactly N module stub files with correct frontmatter. The composition sync . scaffold is isomorphic to the identity on Manifest.*

```
FOR ALL m IN ValidManifests: LET files = manifest_scaffold(m); THEN |files| = |m.modules| AND FOR ALL (name, decl) IN m.modules: EXISTS f IN files: f.frontmatter.module = name AND f.frontmatter.domain = decl.domain AND f.frontmatter.maintains = decl.maintains. FOR ALL m IN ValidManifests: manifest_sync(manifest_scaffold(m)) = m
```

Violation scenario: User adds modules to manifest but has no tool to generate corresponding files. Manual creation produces frontmatter mismatches that fail Check 15.

Validation: Create a manifest with 3 modules. Run manifest scaffold. Verify 3 files with correct frontmatter. Run manifest sync on scaffolded files. Verify round-trip recovers manifest structure.

// WHY THIS MATTERS: Without the left adjoint, the bilateral lifecycle has a fixed-point initialization problem. manifest scaffold breaks this circularity.

---

### APP-ADR-047: Manifest Scaffold as Bilateral Dual

#### Problem

No inverse to manifest sync. Fixed-point initialization problem: need files to validate manifest, need manifest to create files.

#### Options

A) ddis manifest scaffold -- left adjoint to manifest sync. B) Extend ddis init. C) External template tool.

#### Decision

**Option A: ddis manifest scaffold.** Left adjoint completing the Manifest-Files adjunction. Idempotent. WHY NOT Option B? init creates workspaces from scratch; scaffold adds modules to existing workspaces. WHY NOT Option C? External tools cannot enforce frontmatter-manifest bijection.

#### Consequences

New command: ddis manifest scaffold. New state transition: T_manifest_scaffold. Composition T_manifest_sync . T_manifest_scaffold verifiable as adjunction unit.

#### Tests

Scaffold 3-module manifest: verify 3 files with correct frontmatter. Re-run scaffold: all 3 skipped (idempotent). manifest sync on scaffolded files: round-trip matches.

---
