---
module: code-bridge
domain: bridge
maintains: [APP-INV-017, APP-INV-018, APP-INV-019, APP-INV-020, APP-INV-021, APP-INV-048, APP-INV-054, APP-INV-055, APP-INV-111]
interfaces: [APP-INV-001, APP-INV-002, APP-INV-003, APP-INV-008, APP-INV-009, APP-INV-015, APP-INV-016]
implements: [APP-ADR-012, APP-ADR-014, APP-ADR-015, APP-ADR-034, APP-ADR-038, APP-ADR-040, APP-ADR-042, APP-ADR-051]
adjacent: [parse-pipeline, query-validation, lifecycle-ops, auto-prompting]
negative_specs:
  - "Must NOT require language-specific AST parsers for annotation extraction"
  - "Must NOT report false positive contradictions — reported conflicts must be real logical conflicts"
  - "Must NOT modify or delete existing event stream records"
  - "Must NOT silently drop annotations that use valid grammar but target non-existent spec elements"
---

# Code Bridge Module

The code bridge module closes the gap between specification and implementation by providing three complementary subsystems: a cross-language annotation scanner that extracts spec traceability from source code comments, a contradiction detector that identifies logical conflicts between spec elements, and a multi-stream event sourcing system that records specification evolution over time.

This module answers the question: **does the code match the spec, does the spec contradict itself, and how did it get here?** Together, the three subsystems extend drift detection from spec-internal consistency (already handled by the drift-management module in the parent spec) to spec-code correspondence and temporal evolution.

The annotation system uses a universal `ddis:` comment prefix --- no language-specific AST parsing required. Annotations travel with the code, making traceability a property of the source file, not an external manifest. The contradiction detector operates in four tiers: structural validation (Tier 1), graph-based predicate analysis (Tier 2), SAT solving via gophersat (Tier 3), and heuristic NLP + LSI (Tier 4). Event sourcing records every parse, validation, and drift measurement as an append-only JSONL stream.

**Invariants interfaced from other modules (INV-018 compliance):**

- APP-INV-001: Round-Trip Fidelity --- parse then render produces byte-identical output (maintained by parse-pipeline). *Annotation scanning depends on stable file content; round-trip fidelity ensures the scanned source matches the indexed spec.*
- APP-INV-002: Validation Determinism --- results independent of clock, RNG, execution order (maintained by query-validation). *Contradiction detection results must be deterministic; non-deterministic validation would produce flickering contradiction reports.*
- APP-INV-003: Cross-Reference Integrity --- every resolved reference points to an existing element (maintained by query-validation). *Annotation verification cross-references targets against the spec index; broken references in the index produce false orphan reports.*
- APP-INV-008: RRF Fusion Correctness --- score equals correctly computed weighted sum (maintained by search-intelligence). *Intent coverage scoring (Phase 11A) reuses the RRF scoring infrastructure; incorrect fusion affects coverage assessment.*
- APP-INV-015: Deterministic Hashing --- SHA-256 with no salt (maintained by parse-pipeline). *Event stream records include spec hashes for correlation; non-deterministic hashing makes temporal comparisons meaningless.*
- APP-INV-016: Implementation Traceability --- valid Source/Tests/Validates-via paths (maintained by lifecycle-ops). *The annotation system provides the source-side evidence for APP-INV-016's traceability claims; annotations are the bridge between implementation traces and code reality.*

---

## Background and Design Rationale

DDIS specifications exist in two worlds: structured markdown (the specification) and executable code (the implementation). Without a bridge, these worlds drift apart silently. The specification describes invariants, ADRs, and quality gates; the implementation claims to satisfy them. But claims without evidence are wishes. The gap between "the spec says X" and "the code does X" is where correctness goes to die.

The code bridge closes this gap through three mechanisms that correspond to three questions every engineering team asks:

1. **Annotations** --- "Which code implements which spec element?" The annotation system gives code a voice in the spec conversation. A `// ddis:maintains INV-006` comment is not documentation; it is a testable claim that the surrounding code satisfies an invariant. The claim travels with the code through refactors, merges, and repository reorganizations.

2. **Contradiction detection** --- "Does the spec contradict itself?" A specification large enough to be useful is large enough to contain contradictions. Two invariants written months apart by different authors may make logically incompatible claims. Tier 1 (graph-based predicate analysis) catches structural contradictions --- negation pairs, quantifier conflicts, circular implications. Tier 2 (Z3 SMT solving) catches semantic contradictions --- conflicting performance budgets, arithmetic impossibilities.

3. **Event sourcing** --- "How did the spec get here?" Three JSONL streams record the temporal dimension of the specification lifecycle: discovery events (ideas becoming spec), specification events (parse/validate/drift), and implementation events (issues/code/feedback). The streams are append-only --- no rewriting history.

**Category theory framing.** For those who find it clarifying: annotations are morphisms from code objects to spec objects. The scan operation is a functor from the category of annotated source files to the category of spec-code correspondences. Contradiction detection verifies that this functor preserves logical structure --- if the spec's predicate graph is consistent, the implementation's claims about that graph must also be consistent. Event sourcing records the natural transformations between successive versions of these functors over time.

---

## Invariants Maintained by This Module

This module maintains five invariants. Each invariant is fully specified with all six components: plain-language statement, semi-formal expression, violation scenario, validation method, WHY THIS MATTERS annotation, and confidence level.

---

**APP-INV-017: Annotation Portability**

*The annotation grammar is parseable in any programming language that supports single-line comments. The scanner extracts annotations from 14+ languages --- Go, Rust, TypeScript, JavaScript, Python, Ruby, Shell, YAML, SQL, Lua, C, Java, Markdown, and HTML --- without requiring language-specific parsers.*

```
FOR ALL file IN annotated_files:
  FOR ALL annotation IN file.annotations:
    annotation.parsed_correctly = true
  WHERE:
    parsed_correctly = (verb IN valid_verbs) AND (target MATCHES target_regex)
    valid_verbs = {maintains, implements, interfaces, tests, validates-via,
                   postcondition, relates-to, satisfies}
    target_regex = /(INV|ADR|APP-INV|APP-ADR|Gate|S\d)-?\d+|@[\w-]+/
    comment_families = {
      "//":  [Go, Rust, TypeScript, JavaScript, C, Java],
      "#":   [Python, Ruby, Shell, YAML],
      "--":  [SQL, Lua],
      "<!--": [Markdown, HTML]
    }
```

Violation scenario: A Python developer writes `# ddis:maintains INV-006` in a module. The scanner's regex is tuned only for `//`-style comments and misses the `#`-style comment. The annotation silently disappears from the scan results. The developer believes the code is traced to INV-006 but `ddis scan --verify` reports INV-006 as unimplemented.

Validation: Create test files in at least 8 languages (Go, Python, Rust, TypeScript, Shell, YAML, SQL, Markdown) each containing `ddis:maintains INV-001`. Run `ddis scan` on all files. Every annotation must be extracted with correct verb and target.

// WHY THIS MATTERS: The annotation system's value proposition is language-agnostic traceability. A scanner that only works for Go defeats the purpose --- the spec-code bridge must reach every source file in the project.

**Confidence:** property-derived

---

**APP-INV-018: Scan-Spec Correspondence**

*When `ddis scan --verify` is run with a spec database, every annotation target must resolve to an existing spec element. Annotations targeting non-existent spec elements are reported as orphaned. Spec elements with zero code annotations are reported as unimplemented.*

```
FOR ALL annotations IN scan_result WHERE verify_mode = true:
  annotation.target IN spec_index.elements
  OR annotation IN orphaned_report

FOR ALL elements IN spec_index WHERE element.type IN {invariant, adr, gate}:
  EXISTS annotation IN scan_result: annotation.target = element.id
  OR element IN unimplemented_report
```

Violation scenario: A developer writes `// ddis:maintains INV-XYZ` but no such invariant exists in the spec. The scanner silently stores this annotation without validation. The developer believes they've established traceability to a real invariant, but the link is orphaned. Meanwhile, the actual invariant they intended (APP-INV-009) remains marked as unimplemented.

Validation: Create a test spec with 5 invariants. Annotate code files targeting 3 of them correctly, 1 with a non-existent target (`INV-XYZ`), and leave 1 with no annotations. Run `ddis scan --verify`. Verify: 3 annotations resolve, the non-existent target reported as orphaned, the unannotated invariant reported as unimplemented.

// WHY THIS MATTERS: Unverified annotations give a false sense of traceability. The annotation system must be bidirectional: code claims must match spec reality, and spec claims must have code evidence.

**Confidence:** property-derived

---

**APP-INV-019: Contradiction Graph Soundness**

*Every contradiction reported by the detection system represents a genuine logical conflict between spec elements. The system may miss contradictions (false negatives are acceptable at Tier 1), but must never report contradictions that do not exist (false positives are not acceptable at any tier).*

```
FOR ALL reported_contradictions (a, b) IN contradiction_graph:
  logically_inconsistent(a.predicate, b.predicate) = true
WHERE:
  logically_inconsistent = negation_pair OR quantifier_conflict
                        OR circular_implication OR negative_spec_violation
  negation_pair = (a says "must X") AND (b says "must NOT X") for same X
  quantifier_conflict = (a says "for all") AND (b says "there exists NOT") for same domain
  circular_implication = path A implies B implies ... implies NOT A
  negative_spec_violation = (a is negative_spec "must NOT X") AND (b implies X)
```

Violation scenario: INV-007 states "every section earns its place" and INV-018 states "invariants restated at point of use." The Tier 1 detector sees "every section" (universal) and "restated" (repetition) and flags this as a contradiction --- but it's not: restating invariants IS earning a place because it enables module self-containment. The false positive erodes trust in the contradiction system. After three such false alarms, a developer configures their editor to ignore contradiction reports entirely. When a genuine contradiction between performance budget constraints is detected two weeks later, the developer dismisses it without reading. The system has poisoned its own signal channel.

Validation: Create a spec with 3 genuine contradictions and 5 non-contradictions that share terminology. Run contradiction detection. All 3 genuine contradictions must be reported. Zero false positives from the 5 non-contradictions. Precision = 100%. Additionally, verify the graph analysis: extract predicates from both INV-007 and INV-018, build the conflict graph, and confirm no edge connects them (they share no subject-predicate pair where one negates the other).

// WHY THIS MATTERS: A contradiction detector with false positives trains users to ignore its output. The system is worthless the moment developers start dismissing real contradictions because of past false alarms.

**Confidence:** property-derived

---

**APP-INV-020: Event Stream Append-Only**

*The JSONL event stream is strictly append-only with monotonically increasing timestamps. No existing record may be modified, deleted, or reordered after write. The stream survives database recreation. Event equality is defined as content_hash equality: two events are identical if and only if SHA-256 of their JSON payload produces the same digest.*

```
FOR ALL events e_i, e_j IN event_stream WHERE i < j:
  e_i.timestamp <= e_j.timestamp      (monotonic)
  e_i.content_at_write = e_i.content  (immutable after write)
  len(event_stream) is monotonically non-decreasing over time

FOR ALL events e_a, e_b:
  equal(e_a, e_b) = (sha256(json(e_a.payload)) = sha256(json(e_b.payload)))

FOR ALL parse_operations:
  event_stream receives exactly one event: {type: "spec_parsed", spec_hash, drift_score, validation_summary}

FOR ALL validate_operations:
  event_stream receives exactly one event: {type: "validation_run", check_results, spec_hash}
```

Violation scenario: Suppose event streams were stored inside the SQLite database rather than in append-only JSONL files. A `ddis parse --force` recreates the database. All historical parse and validation events would be lost. The developer could no longer track how the spec evolved --- the entire temporal record gone. This motivates the JSONL design: stream files at `.ddis/events/stream-{1,2,3}.jsonl` survive database recreation by existing outside the DB.

Validation: Write 5 events to the stream. Run `ddis parse --force` (which recreates the DB). Verify all 5 events are still present and in original order. Write 1 more event. Verify it appears after the original 5 with a monotonically increasing timestamp. Additionally, write two events with identical payloads; verify their content hashes match. Write two events with different payloads; verify their content hashes differ.

// WHY THIS MATTERS: The event stream is the temporal backbone of the specification lifecycle. It records drift trends, validation trajectories, and spec evolution. An event stream that can be corrupted or lost makes temporal analysis impossible.

**Confidence:** property-derived

---

**APP-INV-021: Consistency Encoding Fidelity**

*Propositional encodings generated from invariant `semi_formal` fields faithfully represent the logical content of those fields. If the SAT solver reports UNSAT (unsatisfiable) for a set of encoded clauses, the corresponding invariants are genuinely logically inconsistent. (Updated per APP-ADR-034: Z3 CGo dependency replaced by pure-Go gophersat.)*

```
FOR ALL invariant_sets S WHERE sat_check(encode(S)) = UNSAT:
  logically_inconsistent(S) = true
WHERE:
  encode: semi_formal -> propositional clause set (CNF)
  sat_check: clause set -> {SAT, UNSAT}
  logically_inconsistent: manually verifiable that no model satisfies all constraints in S

Encoding rules:
  "FOR ALL x: P(x)":  conjunction of P(x_i) for all known domain elements x_i
  "EXISTS x: P(x)":   disjunction of P(x_i) for all known domain elements x_i
  "A AND B":          clause_A AND clause_B
  "A OR B":           clause_A OR clause_B
  "A IMPLIES B":      NOT clause_A OR clause_B
  "NOT A":            negation of clause_A
  Numeric constraints: encoded as propositional bounds (e.g., x <= 5 becomes NOT x_6 AND NOT x_7 ...)
  Named variables:    GLOBAL propositional variable per unique identifier (NOT per-invariant)

Tier 5 SMT extension (Z3 subprocess via APP-ADR-038):
  sat_check extended: clause set -> {SAT, UNSAT} via Z3 SMT-LIB2 when propositional encoding is insufficient
  Arithmetic:         x > 5 encoded as (assert (> x 5)) in QF_LIA
  Quantifiers:        FOR ALL x: P(x) encoded as (assert (forall ((x Int)) (P x))) in LIA
  Uninterpreted:      f(x) = y encoded via (declare-fun f (Int) Int) (assert (= (f x) y)) in QF_UF
  Graceful:           When Z3 not in PATH, Tier 5 silently skipped — no false negatives from absence
```

Violation scenario: INV-005 has `semi_formal: "bundle_size(b) <= hard_ceiling"` and INV-007 has `semi_formal: "signal_density(s) > 0.5"`. The encoder maps `bundle_size` and `signal_density` to the same propositional variable set. The solver reports UNSAT for a satisfiable pair. The system reports a false contradiction between two unrelated invariants.

Validation: Create 5 invariant pairs with known satisfiability status (3 satisfiable, 2 unsatisfiable). Encode and check via gophersat. Verify results match ground truth for all 5 pairs. Verify MUS extraction identifies the correct minimal conflicting subset for UNSAT pairs.

// WHY THIS MATTERS: The SAT solver is the strongest pure-Go contradiction detection mechanism, but its value depends on faithful encoding. A mis-encoding turns a sound solver into a random oracle.

**Confidence:** property-derived

---

## Algorithm Specifications

---

### Algorithm: Tier 1 Predicate Extraction and Contradiction Graph

```
Algorithm: Tier1PredicateExtraction
Input: spec_index (all invariants and ADRs)
Output: ContradictionGraph (edges = confirmed conflicts)

1. For each invariant i in spec_index:
   a. Extract predicates from i.semi_formal:
      - Universals: "FOR ALL x: P(x)" -> Predicate{type: universal, var: x, prop: P}
      - Existentials: "EXISTS x: P(x)" -> Predicate{type: existential, var: x, prop: P}
      - Negations: "NOT P" -> Predicate{type: negation, prop: P}
      - Implications: "A IMPLIES B" -> Predicate{type: implication, antecedent: A, consequent: B}
   b. Extract predicates from i.negative_specs:
      - "Must NOT X" -> Predicate{type: prohibition, prop: X}
2. Build predicate graph G:
   - Nodes = predicates
   - Edges where predicates share a variable or property domain
3. Detect contradiction patterns:
   a. Negation pair: P(x) AND NOT P(x) for same domain
   b. Quantifier conflict: FOR ALL x: P(x) AND EXISTS x: NOT P(x)
   c. Circular implication: A IMPLIES B AND B IMPLIES NOT A
   d. Negative spec violation: prohibition(X) AND exists predicate implying X
4. For each detected pattern:
   - Verify: is the conflict genuine (same domain, same scope)?
   - If genuine: add edge to ContradictionGraph with evidence
   - If uncertain: discard (no false positives -- APP-INV-019)
5. Return ContradictionGraph
```

// WHY THIS MATTERS: The algorithm explicitly prioritizes precision over recall. Step 4's discard-if-uncertain rule is the mechanism that preserves APP-INV-019 (zero false positives). An algorithm that reports uncertain findings as confirmed contradictions poisons the signal channel.

---

### Algorithm: Annotation Grammar

The formal grammar defining the syntax of DDIS annotations in source code:

```
Grammar: DDIS Annotation
  annotation     = comment_marker WS "ddis:" verb WS target [WS qualifier]
  comment_marker = "//" | "#" | "--" | ";" | "%" | "'" | "REM" | "<!--"
  verb           = "maintains" | "implements" | "interfaces" | "tests"
                 | "validates-via" | "postcondition" | "relates-to" | "satisfies"
  target         = inv_ref | adr_ref | gate_ref | section_ref | named_ref
  inv_ref        = ("INV" | "APP-INV") "-" DIGITS
  adr_ref        = ("ADR" | "APP-ADR") "-" DIGITS
  gate_ref       = "Gate-" DIGITS
  section_ref    = "S" DIGITS ("." DIGITS)*
  named_ref      = "@" IDENT
  qualifier      = "(" TEXT ")"
  WS             = " "+
  DIGITS         = [0-9]+
  IDENT          = [a-zA-Z0-9_-]+
  TEXT           = [^)]+
```

The regex implementation that captures this grammar:

```
AnnotationRe = `ddis:(maintains|implements|interfaces|tests|validates-via|postcondition|relates-to|satisfies)\s+((?:APP-)?(?:INV|ADR)-\d{3}|Gate-\d+|S\d+(?:\.\d+)*|@[\w-]+)(.*)$`
```

### Comment-Family Map

The scanner uses a comment-family map to associate file extensions with their comment prefix. This map is the sole source of language-specific knowledge in the annotation system --- no AST parsing, no language-specific intelligence beyond "what does a comment look like."

| Language Family | Comment Marker(s) | Languages |
|---|---|---|
| C-style | `//` | Go, Rust, TypeScript, JavaScript, Java, C, C++, C#, Kotlin, Swift |
| Hash | `#` | Python, Ruby, Shell, YAML, TOML, Perl |
| SQL | `--` | SQL, Lua, Haskell |
| Semicolon | `;` | Lisp, Clojure, Assembly |
| Percent | `%` | LaTeX, Erlang, MATLAB |
| HTML | `<!--` ... `-->` | HTML, XML, Markdown |

// WHY THIS MATTERS: Adding support for a new language requires exactly one operation: add its file extension and comment marker to this map. No new parser, no new test infrastructure, no new dependency. The comment-family map is the mechanism by which APP-INV-017 scales from 14 languages today to any language tomorrow.

---

**APP-INV-111: Cascade Relationship Completeness**

*When analyzing cascade impact for element E, the result must include every module M such that a relationship R exists where R.target = E, regardless of R.rel_type. All four relationship types (maintains, interfaces, implements, adjacent) must contribute to the affected module set. The output must categorize each affected module by its relationship role.*

```
forall element E, module M, relationship R in module_relationships:
  R.target = E AND R.module_id = M.id =>
    M in cascade(E).AffectedModules
  role(M) = "owner" if R.rel_type = "maintains"
  role(M) = "consumer" if R.rel_type in {"interfaces", "implements"}
  role(M) = "peer" if R.rel_type = "adjacent"
```

Violation scenario: Module M maintains APP-INV-001. Run `ddis cascade APP-INV-001 db.db`. Module M is missing from the affected modules list because `maintains` relationships are excluded at line 100. The developer modifying APP-INV-001 has no signal that M (the owning module) needs updating.

Validation: Create a test database with module M maintaining INV-X, module N interfacing INV-X, module P adjacent to INV-X. Run cascade analysis on INV-X. Assert all three modules appear in the result with correct roles (owner, consumer, peer respectively).

// WHY THIS MATTERS: Cascade analysis is how developers discover the blast radius of spec changes. Excluding the owning module — the most affected party — means the most critical update is missed. This creates a false sense of containment: the cascade report says "only consumers are affected" when the owner itself needs revision. The asymmetry with `progress` and `implorder` (which DO include maintains) makes the behavior inconsistent across commands. Related: APP-INV-016 (Implementation Traceability), APP-INV-020 (Spec-Impl Correspondence).

---

## Architecture Decision Records

---

### APP-ADR-012: Annotations over Code Manifest

#### Problem
The spec-code bridge requires a mechanism to declare which code implements which spec elements. Two approaches: a centralized manifest file (`code_manifest.yaml`) or distributed annotations embedded in source code comments.

#### Options
A) **Code Manifest** --- a single YAML file declaring `{spec_element: [file:line, ...]}` mappings.
- Pros: single file to review, easy to parse
- Cons: manifest drifts from code (separate file = separate maintenance), not portable across languages, doesn't travel with the code during refactoring

B) **Inline Annotations** --- `// ddis:maintains INV-006` comments embedded in source code.
- Pros: contracts travel with the code (zero drift by construction for annotated files), portable across all languages via comment syntax, extensible to non-code artifacts (markdown, YAML, Dockerfiles)
- Cons: scattered across files, requires scanner to aggregate

#### Decision
**Option B: Inline Annotations.** Annotations are the bridge, not a manifest. The universal grammar `<comment-marker> ddis:<verb> <target> [qualifier]` works in any language with comments. No language-specific AST parser required --- a single regex scanner extracts all annotations.

// WHY NOT Code Manifest? A manifest is a declaration of intent, not a proof of implementation. Annotations embed the declaration at the point of implementation, making false claims immediately visible during code review. A manifest that says "file.go:42 implements INV-006" can lie; a comment on line 42 saying `// ddis:maintains INV-006` is inspectable in context.

#### Consequences
- New package `internal/annotate/` with scanner, grammar, scope resolution, and report modules
- New CLI command `ddis scan <code-root> [--spec <db>] [--verify] [--store]`
- New storage table `code_annotations` in spec DB
- `ddis drift --code <root>` integrates scan results into drift measurement
- `ddis coverage` extended with implementation coverage metric

#### Tests
- Scan a Go project with 10 annotations across 5 files; verify all 10 extracted correctly
- Run `ddis scan --verify` against a spec DB; verify orphaned and unimplemented reports are accurate

---

### APP-ADR-013: Z3 as Required Dependency — **SUPERSEDED by APP-ADR-034**

#### Problem
Contradiction detection Tier 2 requires an SMT solver. Z3 (via `mitchellh/go-z3` CGO bindings) is the industry standard. Should it be optional (build tags) or required?

#### Options
A) **Z3 required (CGo)** — Link Z3 via CGo bindings.
B) **Z3 optional (build tags)** — Conditional compilation for Z3.
C) **Pure-Go replacement** — See APP-ADR-034.

#### Decision
**Option C: Pure-Go replacement (SUPERSEDED).** This ADR is superseded by APP-ADR-034 (Pure-Go Tiered Consistency over Z3). All Go Z3 bindings require CGo, which violates single-binary distribution. The most mature binding (mitchellh/go-z3) was archived in October 2023. See APP-ADR-034 for the replacement architecture.

#### Consequences
Superseded. No consequences apply — see APP-ADR-034 for the replacement decision and its consequences.

#### Tests
Superseded. Verification is covered by APP-ADR-034 tests.

---

### APP-ADR-034: Pure-Go Tiered Consistency over Z3 — SUPERSEDED BY APP-ADR-038

#### Problem
Contradiction detection requires formal reasoning about invariant consistency. APP-ADR-013 prescribed Z3 via CGo bindings, but all Go Z3 bindings require CGo, which violates single-binary distribution (APP-ADR-024: `curl | bash` installable). The most mature binding (mitchellh/go-z3) was archived in October 2023. How should the CLI detect contradictions without CGo?

#### Options
A) **Z3 subprocess** --- pipe SMT-LIB2 to `z3` binary via `os/exec`.
- Pros: full Z3 power, no CGo
- Cons: requires Z3 on PATH, breaks single-binary promise, ~50-200ms startup per query

B) **Pure-Go SAT (gophersat)** --- encode semi-formal expressions as propositional logic, use gophersat for SAT checking with UNSAT core (MUS) extraction.
- Pros: pure Go (MIT, v1.4), single binary, MUS extraction for conflict localization
- Cons: propositional only (no quantified theories), ~80% of semi-formal expressions parseable

C) **Tiered pure-Go** --- Layer multiple techniques: graph analysis (existing BFS/PageRank), SAT (gophersat), heuristic NLP, LSI (existing gonum). Each tier catches a different class of contradiction.
- Pros: best coverage, all pure Go, reuses existing infrastructure extensively
- Cons: more code surface than single technique

#### Decision
**Option C: Tiered pure-Go.** Six detection tiers, each targeting a different contradiction class:

- **Tier 1 (structural)**: Existing 14 validator checks already catch structural inconsistencies (orphan references, broken declarations, missing components). Zero additional code.
- **Tier 2 (graph)**: Typed cross-reference edges (supports/requires/qualifies/conflicts/supersedes), contradictory cycle detection (DFS + sign tracking), governance overlap analysis, unsupported invariant detection. Reuses `internal/impact/impact.go` BFS and `internal/search/authority.go` PageRank. ~200 LOC.
- **Tier 3 (SAT)**: Parse semi-formal expressions into AST (~80% of 46 expressions). Encode invariant pairs as gophersat clauses. UNSAT = contradiction. MUS extraction identifies minimal conflicting subset. Dependency: `github.com/crillab/gophersat` (pure Go, MIT, v1.4, maintained by CRIL research lab). ~300 LOC.
- **Tier 4 (heuristic + semantic)**: Polarity inversion detection (must vs must-not on same subject), quantifier conflict (forall vs exists-not on same domain), numeric bound conflicts. LSI projection of invariant + negative-spec statements for cross-boundary tension detection. Reuses `internal/search/lsi.go`. ~250 LOC.

// WHY NOT Z3 subprocess? Breaks single-binary promise. Optional external tools contradict the CLI's design principle of offline, self-contained operation.

// WHY NOT SAT-only? Semi-formal parsing achieves ~80% coverage. The remaining 20% of expressions are best handled by Tier 4 heuristics. Graph analysis (Tier 2) catches structural contradictions that SAT encoding would miss.

#### Consequences
- `github.com/crillab/gophersat` added to `go.mod` (pure Go, ~500KB)
- New package `internal/consistency/` with graph.go, sat.go, heuristic.go, semantic.go
- New CLI command `ddis contradict manifest.ddis.db [--tier N] [--json]`
- APP-INV-021 updated: "Z3 Translation Fidelity" → "Consistency Encoding Fidelity"
- `drift.ImplDrift.Contradictions` populated from real analysis (previously hardcoded to 0)

#### Tests
- Create spec with 3 structural contradictions and 2 numeric contradictions; verify Tier 2 catches structural, Tier 3/4 catches numeric
- Verify zero false positives on the DDIS CLI's own spec (self-bootstrap)
- Verify gophersat MUS extraction identifies the correct minimal conflict set

---

### APP-ADR-014: Tiered Contradiction Detection

#### Problem
Spec elements can contradict each other in ways that structural validation misses. How should the system detect logical contradictions?

#### Options
A) **Graph-only** --- predicate extraction + contradiction graph from invariant statements and cross-references. Pattern-matching on quantifiers and negation.
- Pros: pure Go, no external dependencies, fast
- Cons: misses semantic contradictions (conflicting performance budgets, arithmetic impossibilities)

B) **SAT-only** --- encode all constraints as propositional logic, check satisfiability via gophersat.
- Pros: pure Go, formal soundness, UNSAT core extraction
- Cons: semi-formal parsing covers ~80% of expressions, misses nuanced natural-language tension

C) **Four-tier layered** --- structural validation (existing), graph analysis, SAT solving, heuristic NLP + LSI. Each tier catches a different class of contradiction.
- Pros: best coverage, all pure Go, reuses existing infrastructure
- Cons: four subsystems to maintain

#### Decision
**Option C: Six-tier layered.** (Updated per APP-ADR-034 — Z3 replaced with pure-Go tiers.)

- Tier 1 (structural): Existing 14 validator checks catch structural inconsistencies.
- Tier 2 (graph): Typed cross-ref edges, cycle detection, governance overlap. Reuses existing BFS + PageRank.
- Tier 3 (SAT): Semi-formal → gophersat propositional encoding. UNSAT = contradiction. MUS = minimal conflict.
- Tier 4 (heuristic + semantic): Polarity/quantifier/numeric rules + LSI tension detection.

All tiers run; results merged with deduplication by element pair. Tier 2 is designed for speed; Tier 3 for logical soundness; Tier 4 for natural-language coverage.

// WHY NOT Graph-only? Misses arithmetic contradictions (e.g., conflicting latency budgets: 50ms max vs 3×20ms sequential calls = 60ms).

// WHY NOT SAT-only? Semi-formal parsing covers ~80% of expressions. The remaining 20% with complex natural-language predicates fall to Tier 4 heuristics.

#### Consequences
- New package `internal/consistency/` with `graph.go` (Tier 2), `sat.go` (Tier 3), `heuristic.go` (Tier 4), `semantic.go` (Tier 4)
- `drift.ImplDrift.Contradictions` populated from real analysis (currently hardcoded to 0)
- New CLI command `ddis contradict` for standalone contradiction analysis
- Dependency: `github.com/crillab/gophersat` (pure Go, MIT, v1.4)

#### Tests
- Create spec with 3 structural contradictions and 2 arithmetic contradictions; verify Tier 2 catches structural and Tier 3/4 catches arithmetic
- Verify zero false positives on a known-consistent spec (the DDIS CLI spec itself)

---

### APP-ADR-015: Three-Stream Event Sourcing

#### Problem
The specification lifecycle produces events across three domains --- discovery (idea-to-spec), specification (parse/validate/drift), and implementation (issues/PRs/code). How should these events be recorded?

#### Options
A) **Single stream** --- all events in one JSONL file. Simple but hard to filter and reason about.
B) **Three streams** --- one JSONL per lifecycle domain. Clear separation of concerns. Cross-stream references via shared identifiers (INV-NNN, ADR-NNN, br-NNN).

#### Decision
**Option B: Three streams.** Each stream owns its concern: Stream 1 (discovery) records questions, decisions, and artifact crystallization. Stream 2 (spec) records parse, validate, drift, and contradiction events. Stream 3 (implementation) records issue creation, status changes, and implementation findings that feed back to Stream 1. Streams reference each other via shared artifact IDs but never write to each other.

// WHY NOT Single stream? Event types from different domains have different schemas, different consumers, and different lifecycle semantics. Mixing them creates a stream that's hard to filter, hard to reason about, and hard to evolve independently.

#### Consequences
- `ddis parse` writes to Stream 2 on every invocation
- `ddis validate` writes to Stream 2 on every invocation
- `ddis drift` writes to Stream 2 on every invocation
- `ddis history` reads Stream 2 (and optionally joins all 3 streams with `--all-streams`)
- Cross-stream identifiers: INV-NNN / ADR-NNN (spec <-> discovery), br-NNN (implementation <-> spec)

#### Tests
- Run `ddis parse` 3 times on evolving spec; verify 3 events in Stream 2 with monotonic timestamps
- Run `ddis history --all-streams` with test data in all 3 streams; verify unified temporal view

---

## Implementation Chapters

---

### Chapter: Annotation Scanner

**Preserves:** APP-INV-017 (Annotation Portability --- grammar works across 14+ languages), APP-INV-018 (Scan-Spec Correspondence --- every annotation resolves or is reported orphaned).

**Interfaces:** APP-INV-015 (Deterministic Hashing --- annotation content hashes use sha256Hex), APP-INV-016 (Implementation Traceability --- annotations provide the source-side evidence for traceability claims).

The annotation scanner is the primary subsystem for establishing spec-code correspondence. It walks a directory tree, detects programming languages by file extension, extracts comment lines, parses them against the annotation grammar, resolves the scope of each annotation, and produces a structured report. No AST parsing, no language-specific intelligence beyond the comment-family map.

#### Scan Algorithm

```
Algorithm: WalkAndScan
Input: root directory, ScanOptions (exclude globs, verify flag, store flag)
Output: ScanResult (annotations, summary, verify report if applicable)

1. Initialize empty annotation list, file counter, skip counter
2. filepath.WalkDir(root):
   a. For each directory entry:
      - Check against exclude globs (.git, node_modules, vendor by default)
      - If excluded: return filepath.SkipDir
      - If symlink: skip (do not follow -- NEG-BRIDGE-008)
   b. For each file:
      - Look up extension in comment-family map
      - If extension unknown: increment skip counter, continue
      - Read file line by line via bufio.Scanner
      - For each line:
        i.   Find comment_prefix in line
        ii.  Strip prefix (and suffix for HTML-style)
        iii. Match stripped content against AnnotationRe
        iv.  If match: extract verb (group 1), target (group 2), qualifier (group 3)
        v.   Resolve scope via backward line scan (see Scope Resolution)
        vi.  Compute content_hash = sha256Hex(verb + ":" + target + qualifier)
        vii. Append Annotation{FilePath, Line, Verb, Target, Qualifier, Scope, Language, ContentHash}
3. Build ScanResult with annotation list, summary by verb, summary by target
4. If verify flag: cross-validate against spec index (see Verify Path)
5. If store flag: persist to code_annotations table
6. Return ScanResult
```

#### Grammar Specification

```
annotation     := comment_prefix SPACE "ddis:" verb SPACE target (SPACE qualifier)?
comment_prefix := "//" | "#" | "--" | ";" | "%" | "<!--"
verb           := "maintains" | "implements" | "interfaces" | "tests"
                | "validates-via" | "postcondition" | "relates-to" | "satisfies"
target         := inv_ref | adr_ref | gate_ref | named_ref
inv_ref        := ("APP-")? "INV-" DIGIT{3}
adr_ref        := ("APP-")? "ADR-" DIGIT{3}
gate_ref       := "Gate" SPACE DIGIT+
named_ref      := "@" WORD ("-" WORD)*
qualifier      := any text after target until end of line
```

The regex implementation:

```
AnnotationRe = `ddis:(maintains|implements|interfaces|tests|validates-via|postcondition|relates-to|satisfies)\s+((?:APP-)?(?:INV|ADR)-\d{3}|Gate\s+\d+|@[\w-]+)(.*)$`
```

#### Comment-Family Map (`grammar.go`)

The comment-family map associates file extensions with comment prefixes:

| Family | Prefix | Suffix | Extensions |
|---|---|---|---|
| C-style | `//` | (none) | `.go`, `.rs`, `.ts`, `.tsx`, `.js`, `.jsx`, `.c`, `.h`, `.java`, `.kt`, `.swift` |
| Hash | `#` | (none) | `.py`, `.rb`, `.sh`, `.bash`, `.yaml`, `.yml`, `.toml`, `.pl` |
| SQL | `--` | (none) | `.sql`, `.lua`, `.hs` |
| Semicolon | `;` | (none) | `.lisp`, `.clj`, `.asm` |
| Percent | `%` | (none) | `.tex`, `.erl`, `.m` |
| HTML | `<!--` | `-->` | `.md`, `.html`, `.htm`, `.xml` |

For HTML-style comments, the scanner matches `<!-- ddis:<verb> <target> -->` and strips the closing `-->` before extracting the annotation.

#### Scope Resolution (`scope.go`)

Scope resolution determines what code the annotation refers to:

- **file**: the entire file (annotation at file top, outside any function)
- **declaration**: the nearest function/type/method declaration above the annotation
- **block**: the nearest code block (e.g., `if`, `for`, `match`) containing the annotation

Scope is resolved by heuristic: walk lines backward from the annotation until a declaration pattern is found. Declaration patterns are language-family-specific but not language-AST-specific (e.g., `func ` for Go, `def ` for Python, `fn ` for Rust).

#### Verify Path

When `--verify` is set, the scanner cross-validates annotation targets against the spec index:

```
Algorithm: VerifyAnnotations
Input: ScanResult, spec_index (database)
Output: VerifyReport (resolved, orphaned, unimplemented)

1. For each annotation in ScanResult:
   a. Query spec_index for annotation.target
   b. If found: add to resolved list with file, line, verb
   c. If not found: add to orphaned list with reason "does not exist in spec index"
2. For each element in spec_index where type IN {invariant, adr, gate}:
   a. If no annotation in ScanResult targets this element: add to unimplemented list
3. Compute summary: total, resolved count, orphaned count, spec elements, implemented, unimplemented
4. Return VerifyReport
```

// WHY THIS MATTERS: The verify path is the enforcement mechanism for bidirectional traceability. Without it, annotations are claims; with it, they are evidence.

#### Worked Example: Scanning a Go File with 3 Annotations

Given a file `internal/parser/document.go`:

```go
// ddis:maintains APP-INV-001 (round-trip fidelity)
func ParseDocument(specPath string, db storage.DB) error {
    // Parse the specification...
    return nil
}

// ddis:implements APP-ADR-009
// ddis:validates-via APP-INV-015
func extractElementsFromLines(lines []string, db storage.DB, specID int64) error {
    // 4-pass pipeline, uses sha256Hex...
    return nil
}
```

**Scanner execution:**

1. File extension `.go` -> C-style family, prefix `//`
2. Line 1: strip `// ` -> `ddis:maintains APP-INV-001 (round-trip fidelity)` -> match: verb=maintains, target=APP-INV-001, qualifier=(round-trip fidelity)
3. Scope: next declaration on line 2 (`func ParseDocument`) -> scope=declaration, scope_name=ParseDocument
4. Line 7: strip `// ` -> `ddis:implements APP-ADR-009` -> match: verb=implements, target=APP-ADR-009
5. Scope: next declaration on line 9 (`func extractElementsFromLines`) -> scope=declaration, scope_name=extractElementsFromLines
6. Line 8: strip `// ` -> `ddis:validates-via APP-INV-015` -> match: verb=validates-via, target=APP-INV-015
7. Same scope as line 7

Result: 3 annotations, 3 unique targets, 1 file with annotations.

**Implementation Trace:**
- Source: `internal/annotate/scan.go::WalkAndScan`
- Source: `internal/annotate/scan.go::scanFile`
- Source: `internal/annotate/grammar.go::AnnotationRe`
- Source: `internal/annotate/grammar.go::CommentFamilyMap`
- Source: `internal/annotate/scope.go::ResolveScope`
- Source: `internal/annotate/report.go::FormatJSON`
- Source: `internal/annotate/report.go::FormatHuman`
- Source: `internal/annotate/models.go::Annotation`
- Source: `internal/annotate/models.go::ScanResult`

---

### Chapter: Contradiction Detector

**Preserves:** APP-INV-019 (Contradiction Graph Soundness --- zero false positives), APP-INV-021 (Z3 Translation Fidelity --- SMT encoding preserves semantics).

**Interfaces:** APP-INV-002 (Validation Determinism --- contradiction results are deterministic for the same spec).

Contradiction detection operates in five tiers (Tiers 2--6). Tier 2 (graph-based) runs in pure Go with no external dependencies. Tier 3 (SAT) encodes semi-formal expressions as propositional logic via gophersat. Tier 4 (heuristic + semantic) catches natural-language contradictions through polarity and tension analysis. Tier 5 (SMT/Z3) handles arithmetic constraints via Z3 subprocess. Tier 6 (LLM-as-judge) detects semantic contradictions that formal methods miss. All tiers run on every invocation; results are merged with deduplication by element pair. Tier 1 (structural) operates through the existing 16 validator checks, not the contradiction pipeline.

#### Tier 2: Predicate Extraction (`graph.go`)

`ExtractPredicates(db storage.DB, specID int64) ([]PredicateTuple, error)` queries all invariants, ADRs, and negative specs for the spec, then parses each element's statement and semi_formal fields:

1. Tokenize the text into sentences
2. For each sentence, match modal patterns:
   - `must` / `shall` / `always` -> modal = MUST
   - `must not` / `shall not` / `never` -> modal = MUST_NOT
   - `for all` / `every` / `each` -> modal = FOR_ALL
   - `exists` / `there exists` / `some` -> modal = EXISTS
3. Extract the subject (noun phrase before modal) and predicate (verb phrase after modal)
4. Normalize: lowercase, strip articles, collapse whitespace
5. Return `PredicateTuple{ElementID, Subject, Modal, Predicate, SourceText, ContentHash}`

#### Tier 2: Conflict Graph (`graph.go`)

`BuildConflictGraph(predicates []PredicateTuple) *ConflictGraph` constructs the graph:

1. Create a node for each predicate tuple
2. For each pair of nodes (i, j) where i < j:
   a. **Negation pair check**: same normalized subject AND (modal_i negates modal_j) AND same normalized predicate. Negation pairs: (MUST, MUST_NOT), (ALWAYS, NEVER).
   b. **Quantifier conflict check**: (FOR_ALL, EXISTS) over same subject domain with one predicate negating the other.
   c. **Negative spec violation check**: if node_i comes from a negative_spec and node_j from an invariant, and node_i's predicate (stripped of "not") matches node_j's predicate.
3. For implies edges: if predicate_i subsumes predicate_j (predicate_j is a substring of predicate_i or they share >60% tokens), add an implies edge.
4. Run DFS cycle detection on the implies subgraph. If a cycle contains node A and a node whose predicate negates A's predicate, add a circular implication conflict.

`DetectContradictions(graph *ConflictGraph) []Contradiction` returns all conflicts with their confidence scores.

#### Tier 5: Z3 Translation (`smt.go`)

`TranslateToZ3(predicates []PredicateTuple) ([]z3.AST, error)` converts semi_formal fields to Z3 assertions:

**Z3 Translation Rules:**

| Semi-formal Pattern | Z3 S-Expression |
|---|---|
| `FOR ALL x: P(x)` | `(assert (forall ((x T)) (P x)))` |
| `EXISTS x: P(x)` | `(assert (exists ((x T)) (P x)))` |
| `bundle_size(b) <= ceiling` | `(assert (<= bundle_size ceiling))` |
| `A AND B` | `(assert (and A B))` |
| `A IMPLIES B` | `(assert (=> A B))` |
| `NOT A` | `(assert (not A))` |
| Negation pairs | `(assert P) (assert (not P)) (check-sat)` -> UNSAT = contradiction |

Critical rule: each unique identifier in a semi_formal field maps to exactly one Z3 variable. Variables from different invariants that share a name are unified (they refer to the same entity). Variables with different names are kept distinct, even if they describe related concepts.

`CheckSatisfiability(assertions []z3.AST) (z3.Result, z3.Model, error)` runs the solver. If UNSAT, the model is nil and the conflicting assertion core is extracted for the report.

#### Tier 3: SAT Encoding (`sat.go`)

`analyzeSAT` translates semi-formal predicates into conjunctive normal form (CNF) and feeds the combined clause set to a CDCL SAT solver (gophersat). The key design decision preserving APP-INV-021 (detection completeness) is a **shared variable namespace**: identifiers appearing in different invariants' semi-formals are unified into the same boolean variable via `VarMap`. This ensures cross-invariant conflicts are detectable.

**CNF Translation Rules:**

| Semi-formal Pattern | CNF Translation |
|---|---|
| `FOR ALL x: P(x)` | Extract variables from quantifier body |
| `A IMPLIES B` | `(NOT A OR B)` — single clause |
| `A AND B` | Split into separate unit clauses |
| `A OR B` | Single clause with both literals |
| `NOT P` | Negated literal of `P` |

Each invariant's semi-formal becomes one or more CNF clauses. The combined clause set across all invariants is fed to `solver.Solve()`. UNSAT means the invariant set is mutually contradictory (conflict type: `SATUnsatisfiable`). Confidence: 0.85 (propositional encoding loses arithmetic and quantifier semantics that Tier 5 captures).

#### Tier 4: Heuristic + Semantic Analysis (`heuristic.go`, `semantic.go`)

Two complementary analyzers run at Tier 4, catching patterns that formal solvers miss:

**Heuristic analysis** (`analyzeHeuristic`) applies three lightweight checks against invariant semi-formals and statement text:

1. **Polarity inversion** (confidence 0.6): detects "must X" vs "must not X" on overlapping subjects (overlap threshold >0.5). Conflict type: `PolarityInversion`.
2. **Quantifier conflict** (confidence 0.5): universal ("for all") vs existential negation ("no X may") with subject overlap >0.4. Conflict type: `QuantifierConflict`.
3. **Numeric bound conflict** (confidence 0.7): incompatible quantitative constraints (`at_most` vs `at_least`, `exactly` vs different-`exactly`) on the same subject. Conflict type: `NumericBoundConflict`.

**Semantic analysis** (`analyzeSemantic`) builds TF-IDF vectors from invariant statement text and computes pairwise cosine similarity:

- Similarity >0.85 between invariants with opposing polarity produces `SemanticTension` (confidence: `sim * 0.7`)
- Invariant-vs-negative_spec similarity >0.7 produces suspicious alignment (confidence: `sim * 0.5`)

Both analyzers require no external dependencies and run in ~1ms per pair.

#### Tier 6: LLM-as-Judge (`llm.go`)

`analyzeLLM` targets invariant pairs whose semi-formals could not be fully parsed by Tiers 3--5 (natural language or mixed-formality statements). Each pair is classified as contradictory/compatible/independent via the Anthropic API (preserving APP-ADR-042).

**Majority vote protocol** (preserving APP-INV-055): 3 independent completions per pair. 3/3 agreement yields confidence 0.95; 2/3 agreement yields confidence 0.80; less than 2/3 produces no verdict (confidence 0.0). Response normalization extracts the first word of each completion and maps "contradictory"/"contradiction" to contradictory, "consistent"/"compatible" to compatible.

**Graceful degradation** (preserving APP-INV-054): `LLMAvailable()` checks whether a provider is configured. If `ANTHROPIC_API_KEY` is absent, Tier 6 is silently skipped with zero contradictions reported. For testability, `SetLLMProvider()` injects mock providers.

#### Worked Example: Detecting a Genuine Contradiction

*Example INV-XYZ:* All API responses must complete within 50 milliseconds.
```
semi_formal: "FOR ALL request r: response_time(r) <= 50ms"
```

*Example INV-ABC:* Every request must pass through authentication, rate limiting, and logging middleware.
```
semi_formal: "FOR ALL request r: sequential_middleware(r, [auth, rate_limit, logging]) AND
              min_latency(auth) = 25ms AND min_latency(rate_limit) = 20ms AND min_latency(logging) = 8ms"
```

**Tier 2 (graph):** Extracts (api_response, must, complete_within_50ms) and (request, must, pass_through_sequential_middleware). No negation pair, no quantifier conflict. Result: no contradiction detected.

**Tier 5 (SMT/Z3):** Z3 translation:

```smt2
(declare-const response_time_r Int)
(assert (<= response_time_r 50))
(declare-const auth_latency Int)
(declare-const rate_limit_latency Int)
(declare-const logging_latency Int)
(assert (= auth_latency 25))
(assert (= rate_limit_latency 20))
(assert (= logging_latency 8))
(assert (>= response_time_r (+ auth_latency rate_limit_latency logging_latency)))
(check-sat)  ; -> UNSAT (25+20+8=53 > 50)
```

Z3 result: **UNSAT** --- the class of arithmetic contradiction that Tier 2 (graph) cannot detect and that motivated APP-ADR-038.

**Implementation Trace:**
- Source: `internal/consistency/graph.go::BuildConflictGraph`
- Source: `internal/consistency/graph.go::DetectContradictions`
- Source: `internal/consistency/graph.go::detectCycles`
- Source: `internal/consistency/sat.go::CheckSAT`
- Source: `internal/consistency/heuristic.go::CheckHeuristic`
- Source: `internal/consistency/semantic.go::CheckSemantic`
- Source: `internal/consistency/smt.go::CheckSMT`
- Source: `internal/consistency/smt.go::translateToSMTLIB2`
- Source: `internal/consistency/llm.go::CheckLLM`
- Source: `internal/consistency/models.go::Contradiction`

---

### Chapter: Event Stream Management

**Preserves:** APP-INV-020 (Event Stream Append-Only --- JSONL with O_APPEND, no modifications).

**Interfaces:** APP-INV-010 (Oplog Append-Only --- identical append pattern from lifecycle-ops), APP-INV-015 (Deterministic Hashing --- spec_hash field uses sha256Hex).

The event sourcing subsystem records the specification lifecycle across three independent JSONL streams. Each stream is a strictly append-only file. Cross-stream references use shared artifact identifiers (INV-NNN, ADR-NNN, br-NNN) but streams never write to each other.

#### Three-Stream Architecture

| Stream | File | Domain | Event Types |
|---|---|---|---|
| 1 | `.ddis/events/stream-1.jsonl` | Discovery | question_opened, answer_recorded, confidence_changed, decision_crystallized, artifact_written, implementation_feedback, thread_branched, thread_merged, thread_parked |
| 2 | `.ddis/events/stream-2.jsonl` | Specification | spec_parsed, validation_run, drift_measured, contradiction_detected, amendment_applied |
| 3 | `.ddis/events/stream-3.jsonl` | Implementation | issue_created, status_changed, dependency_resolved, implementation_finding |

#### Envelope Schema

Every event conforms to this structure:

```json
{
  "id": "evt-{YYYYMMDD}-{seq}",
  "type": "<event_type>",
  "timestamp": "<RFC3339 UTC>",
  "spec_hash": "sha256:<hex>",
  "stream": 1 | 2 | 3,
  "payload": { ... }
}
```

Event equality: `equal(e_a, e_b) = sha256(canonical_json(e_a.payload)) = sha256(canonical_json(e_b.payload))` (APP-INV-020).

#### Write Path

`AppendEvent(streamPath string, event *Event) error`:

1. Open file with `os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)`
2. Compute `event.ID` from stream sequence (atomic counter per stream)
3. Set `event.Timestamp` from `time.Now().UTC().Format(time.RFC3339)`
4. Marshal event to JSON (single line, no pretty-printing)
5. Write via `json.Encoder` (appends newline automatically)
6. Close file handle (no handle reuse across calls)

// WHY THIS MATTERS: The file flags `O_APPEND|O_CREATE|O_WRONLY` are identical to the oplog pattern in APP-INV-010. No read-write handles are ever created by the write path. A function that cannot seek cannot overwrite.

#### Read Path

`ReadStream(streamPath string, filters EventFilters) ([]*Event, error)`:

1. Open file with `os.Open` (read-only)
2. `bufio.Scanner` with 10MB max line buffer
3. For each line: unmarshal JSON, apply filters (type, since, limit, artifact_ref)
4. Return matching events in chronological order

#### Cross-Stream Correlation

`CorrelateStreams(stream1, stream2, stream3 string, artifactID string) ([]*Event, error)`:

1. Read all three streams filtered by `artifact_ref` containing `artifactID`
2. Merge by timestamp
3. Return unified timeline showing how a single artifact evolved across discovery, specification, and implementation

#### JSONL Examples

**Stream 1 (Discovery) --- decision_crystallized:**
```jsonl
{"id":"evt-20260224-001","type":"decision_crystallized","timestamp":"2026-02-24T09:15:00Z","spec_hash":"sha256:7f3a...","stream":1,"payload":{"thread_id":"discovery-auth-flow","question":"Should the auth middleware use JWT or session cookies?","decision":"JWT with short-lived tokens and refresh rotation","confidence":4,"artifact_refs":["ADR-XYZ","INV-XYZ"],"rationale":"Stateless auth aligns with horizontal scaling requirement"}}
```

**Stream 2 (Specification) --- spec_parsed:**
```jsonl
{"id":"evt-20260224-042","type":"spec_parsed","timestamp":"2026-02-24T10:30:00Z","spec_hash":"sha256:abc123def456...","stream":2,"payload":{"sections":349,"invariants":41,"adrs":29,"cross_refs_resolved":704,"cross_refs_unresolved":0,"drift_score":0.0}}
```

**Stream 2 (Specification) --- contradiction_detected:**
```jsonl
{"id":"evt-20260224-043","type":"contradiction_detected","timestamp":"2026-02-24T10:30:05Z","spec_hash":"sha256:abc123def456...","stream":2,"payload":{"tier":1,"confidence":"high","element_a":"INV-XYZ","element_b":"INV-ABC","pattern":"negation_pair","description":"INV-XYZ requires all requests to be authenticated; INV-ABC exempts health check endpoints"}}
```

**Stream 3 (Implementation) --- implementation_finding:**
```jsonl
{"id":"evt-20260224-100","type":"implementation_finding","timestamp":"2026-02-24T14:00:00Z","spec_hash":"sha256:abc123def456...","stream":3,"payload":{"source":"br-42","finding_type":"spec_gap","description":"Implementation requires connection pooling; no spec section covers pool lifecycle","affected_elements":["INV-010"],"suggested_action":"Add connection pool management section to lifecycle-ops module"}}
```

Cross-stream correlation: the `artifact_refs` in Stream 1's event reference `INV-XYZ`, which also appears in Stream 2's `contradiction_detected` event. Querying `CorrelateStreams` with `artifactID="INV-XYZ"` returns both events in timestamp order.

**Implementation Trace:**
- Source: `internal/events/stream.go::AppendEvent`
- Source: `internal/events/stream.go::ReadStream`
- Source: `internal/events/stream.go::CorrelateStreams`
- Source: `internal/events/models.go::Event`
- Source: `internal/events/models.go::EventFilters`
- Source: `internal/events/schema.go::EventType`
- Source: `internal/events/schema.go::ValidatePayload`

---

## Negative Specifications (Detailed)

These constraints prevent the most likely implementation errors and LLM hallucination patterns for the code bridge subsystem. Each addresses a failure mode that an LLM, given only the positive specification, would plausibly introduce.

**DO NOT** require language-specific AST parsers for annotation extraction (Validates APP-INV-017). The annotation scanner uses regex-based comment extraction with a per-language comment-family map. Adding a new language requires only adding its comment marker(s) to the map, not writing a parser. The 14+ supported languages all use the same regex engine with different comment prefix patterns. An LLM tempted to add "better" annotation extraction by importing a Go AST parser or a tree-sitter binding violates this constraint --- the simplicity of regex extraction IS the portability guarantee.

**DO NOT** report false positive contradictions --- reported conflicts must be real logical conflicts (Validates APP-INV-019). If the contradiction detector is uncertain, it must classify the finding as "potential" with an explanation, not as a confirmed contradiction. Certainty is required for the default report; uncertainty is surfaced only with `--verbose`. The conflict graph must contain no edges between predicates that do not form a genuine logical contradiction as defined by the four patterns (negation pair, quantifier conflict, circular implication, negative spec violation).

**DO NOT** modify or delete existing event stream records (Validates APP-INV-020). The event stream is append-only. Even schema migrations must be handled by adding new event types, not by modifying the structure of existing records. A `version` field on each event enables forward-compatible evolution. No function in the event stream package may open a JSONL file with `O_RDWR`, `O_TRUNC`, or call `Seek`, `Truncate`, or `WriteAt`.

**DO NOT** silently drop annotations that use valid grammar but target non-existent spec elements (Validates APP-INV-018). Every annotation is either successfully resolved against the spec index or explicitly reported as orphaned. The scanner must never discard a structurally valid annotation without reporting it. An annotation targeting `INV-XYZ` is valid grammar (it matches the regex) but invalid semantics (no such invariant exists); it must appear in the orphaned report, not be silently excluded from the scan results.

**DO NOT** unify Z3 variables across invariants that use the same identifier for different concepts (Validates APP-INV-021). When two invariants both use a variable named `x` in independent `FOR ALL` scopes, the translator must map them to distinct Z3 variables. Only variables that share explicit cross-references (e.g., both reference the same INV or ADR) should be unified. Unifying independent variables creates false contradictions between unrelated invariants --- the exact failure mode described in APP-INV-021's violation scenario.

**DO NOT** open event stream files with O_RDWR or allow handle reuse across function calls (Validates APP-INV-020). The `AppendEvent` function opens the file, writes, and closes it within a single function scope. No file handle is returned to callers. No public API in the `events` package accepts a writable file handle. The read path uses `os.Open` (read-only). These two constraints together make in-place modification structurally impossible through the API.

**DO NOT** report Low-confidence contradiction findings in default (non-verbose) output (Validates APP-INV-019). Low-confidence findings are transitive inferences through multi-hop implies edges --- they are the most likely to be false positives. Surfacing them by default trains users to ignore the contradiction report entirely. The `--verbose` flag is the explicit opt-in for speculative findings, and even then, each finding must include the full inference chain so the user can evaluate it.

**DO NOT** allow the annotation scanner to traverse symlinks or follow paths outside the specified code root (Validates APP-INV-017, APP-INV-018). A symlink pointing outside the code root could cause the scanner to read files from unrelated projects, producing annotations that appear local but reference external code. The file walker must use `filepath.WalkDir` with symlink-aware logic: `entry.Type()&os.ModeSymlink != 0` causes a skip, not a follow.

**DO NOT** use `ddis patch` for multi-file, multi-occurrence renames. Rename operations MUST use `ddis rename` which provides totality checking, cross-file consistency, and oplog recording. (Validates APP-ADR-051)

---

## New CLI Commands

### `ddis scan`

**Interface**: `ddis scan <code-root> [--spec <db>] [--json] [--exclude <glob>] [--verify] [--store]`

**Behavior**:
1. Walk the file tree under `<code-root>`, applying `--exclude` globs
2. For each file, detect language from extension, select comment-family
3. Extract all `ddis:<verb> <target> [qualifier]` annotations with file, line, scope context
4. Default: print summary (annotation count by verb, by target, by file)
5. `--verify`: cross-validate targets against spec index in `<db>`. Report orphaned annotations and unimplemented spec elements
6. `--store`: persist annotations to `code_annotations` table in `<db>`

**State transitions**:
```
T_scan:     CodeRoot * Opts -> ScanResult
T_scan_verify: CodeRoot * Index * Opts -> ScanResult * VerifyReport
T_scan_store:  CodeRoot * Index * Opts -> ScanResult * Index'
```

### `ddis history`

**Interface**: `ddis history [--all-streams] [--since <date>] [--json]`

**Behavior**:
1. Read Stream 2 (spec events) from JSONL
2. If `--all-streams`: join all 3 streams by timestamp, correlate by shared artifact IDs
3. Display temporal evolution: drift trend, validation trajectory, spec hash changes
4. `--since <date>`: filter events after the given date

**State transitions**:
```
T_history: EventStream * Filters -> FormattedTimeline
T_history_all: EventStream[3] * Filters -> UnifiedTimeline
```

---

## New Storage

### `code_annotations` Table

```sql
CREATE TABLE code_annotations (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    file_path TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    verb TEXT NOT NULL,
    target TEXT NOT NULL,
    qualifier TEXT,
    scope_level TEXT NOT NULL DEFAULT 'file',  -- file, declaration, block
    scope_name TEXT,
    language TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    scanned_at TEXT NOT NULL,
    UNIQUE(spec_id, file_path, line_number, verb, target)
);
```

### New Drift Types

When `ddis drift --code <root>` is used:
- **code_unspecified**: annotation targets a non-existent spec element (orphaned annotation)
- **code_unimplemented**: spec element has zero code annotations (missing implementation evidence)
- **code_untested**: element has `implements` annotation but no `tests` annotation (implementation without test evidence)

---

## Verification Prompt

Use this self-check after implementing or modifying the code bridge subsystem.

**Positive checks (DOES the implementation...):**

1. DOES `ddis scan` extract annotations from files in at least 8 languages (Go, Python, Rust, TypeScript, Shell, YAML, SQL, Markdown)? (APP-INV-017)
2. DOES the comment-family map cover all 14+ supported languages with correct prefix mappings? (APP-INV-017)
3. DOES `ddis scan --verify` report orphaned annotations that target non-existent spec elements? (APP-INV-018)
4. DOES `ddis scan --verify` report spec elements with zero code annotations as unimplemented? (APP-INV-018)
5. DOES the Tier 1 contradiction detector achieve 100% precision (zero false positives) on the test corpus? (APP-INV-019)
6. DOES the conflict graph correctly identify all four patterns: negation pairs, quantifier conflicts, circular implications, and negative spec violations? (APP-INV-019)
7. DOES the event stream survive `ddis parse --force` database recreation? (APP-INV-020)
8. DOES event equality use SHA-256 of JSON payload (content_hash equality)? (APP-INV-020)
9. DOES Z3 translation produce faithful assertions for the 5-pair test corpus? (APP-INV-021)
10. DOES Z3 correctly report UNSAT for arithmetic contradictions like conflicting latency budgets? (APP-INV-021)
11. DOES the event stream write path use exactly `os.O_APPEND|os.O_CREATE|os.O_WRONLY`? (APP-INV-020)
12. DOES `CorrelateStreams` merge events from all three streams by timestamp for a given artifact ID? (APP-ADR-015)
13. DOES the scanner resolve annotation scope (file, declaration, block) via backward line scan? (APP-INV-017)
14. DOES the `code_annotations` table enforce uniqueness on (spec_id, file_path, line_number, verb, target)? (APP-INV-018)

**Negative checks (does NOT the implementation...):**

1. Does NOT the scanner require language-specific AST parsers for any supported language? (NEG-BRIDGE-001, APP-INV-017)
2. Does NOT the scanner silently drop valid annotations targeting non-existent spec elements? (NEG-BRIDGE-002, APP-INV-018)
3. Does NOT the Tier 1 detector report contradictions between elements that share terminology but are not logically inconsistent? (NEG-BRIDGE-003, APP-INV-019)
4. Does NOT any function in the event stream package open JSONL files with O_RDWR, O_TRUNC, or call Seek/Truncate/WriteAt? (NEG-BRIDGE-004, APP-INV-020)
5. Does NOT the Z3 translator map independent quantifier-scoped variables to the same Z3 variable? (NEG-BRIDGE-005, APP-INV-021)
6. Does NOT the event stream allow modification or deletion of existing records, even via schema migration? (NEG-BRIDGE-006, APP-INV-020)
7. Does NOT the contradiction detector report Low-confidence findings in default (non-verbose) output? (NEG-BRIDGE-007, APP-INV-019)
8. Does NOT the scanner traverse symlinks or follow paths outside the specified code root? (NEG-BRIDGE-008, APP-INV-017)

---

## Package Structure

```
internal/annotate/
+-- scan.go       -- file walker + annotation extractor (regex-based)
+-- grammar.go    -- verb vocabulary, language->comment-family map, target regex
+-- scope.go      -- scope resolution (file/declaration/block) via per-language heuristics
+-- models.go     -- Annotation, AnnotationScope, ScanResult, ScanOptions types
+-- report.go     -- human-readable + JSON output

internal/contradiction/
+-- predicate.go  -- extract logical predicates from invariant statement + semi_formal fields
+-- graph.go      -- build contradiction graph, detect quantifier/negation/circular conflicts
+-- z3.go         -- Z3 SMT solver wrapper, constraint translation from semi_formal
+-- models.go     -- Contradiction, PredicateTuple, ConflictEdge types

internal/events/
+-- stream.go     -- AppendEvent, ReadStream, CorrelateStreams
+-- models.go     -- Event, EventFilters, stream-specific payload types
+-- schema.go     -- Event type constants, payload validation
```

---

## Referenced Invariants from Other Modules

Per the cross-module reference completeness convention, this section lists invariants
owned by other modules that this module depends on or interfaces with:

| Invariant   | Owner               | Relationship | Usage in This Module                                                         |
|-------------|---------------------|--------------|------------------------------------------------------------------------------|
| APP-INV-001 | parse-pipeline      | interfaces   | Round-trip fidelity ensures scanned source matches indexed spec              |
| APP-INV-002 | query-validation    | interfaces   | Validation determinism ensures contradiction results are reproducible        |
| APP-INV-003 | query-validation    | interfaces   | Cross-ref integrity ensures annotation targets resolve against clean index   |
| APP-INV-008 | search-intelligence | interfaces   | RRF fusion correctness for intent coverage scoring                           |
| APP-INV-010 | lifecycle-ops       | interfaces   | Append-only pattern reused for event stream write path                       |
| APP-INV-015 | parse-pipeline      | interfaces   | Deterministic hashing for event stream spec_hash and annotation content_hash |
| APP-INV-016 | lifecycle-ops       | interfaces   | Implementation traceability provides source-side evidence for annotations    |

**APP-INV-048: Event Stream VCS Primacy**

*Event stream JSONL files are primary data artifacts, tracked in version control. They must never be gitignored, and init must create them with spec-conformant names (stream-N.jsonl).*

```
forall ws in Workspaces: .ddis/events/stream-{1,2,3}.jsonl in VCS(ws) AND NOT in .gitignore(ws)
```

Violation scenario: Init adds events/*.jsonl to .gitignore, or creates files with non-conformant names (e.g. discovery.jsonl instead of stream-1.jsonl), silently discarding the provenance chain.

Validation: ddis validate checks .gitignore does not exclude .ddis/events/*.jsonl and verifies stream filenames match stream-N.jsonl pattern

// WHY THIS MATTERS: JSONL event streams capture the complete discovery-to-implementation provenance chain. If gitignored, the bilateral lifecycle loses its audit trail across VCS boundaries.

---

### APP-ADR-036: Tagged Bottom Types for Explicit Non-Resolution

#### Problem

When a design decision is deferred or a spec element is known to be needed but not yet defined, there is no formal mechanism to mark it as explicitly uninhabited. TODOs and comments lack type-theoretic grounding and are invisible to validation.

#### Options

A. TODO comments (informal, invisible to tooling). B. Dedicated sentinel values in schema (breaks type safety). C. Tagged bottom type system where bottom-N carries a named tag identifying the unresolved area, enabling validation to track non-resolution as first-class data.

#### Decision

**Option C: Tagged bottom type system.** A bottom element is written as a formal placeholder with a named tag (e.g. bottom[event-wiring]) that is tracked by the validation system. Like Scala Nothing with named tags, this provides compile-time-equivalent tracking of unresolved design areas. Tagged bottoms make non-resolution visible to formal verification.

#### Consequences

The validator can count, track, and report on unresolved areas. Bottom tags participate in the type lattice (bottom is subtype of every type) so they compose safely with existing invariants and ADRs. They survive parse-render round-trips and are first-class citizens in the coverage model.

#### Tests

1. ddis validate reports tagged bottom elements as warnings (not errors). 2. ddis coverage distinguishes bottom-tagged vs fully-defined elements. 3. Bottom tags survive parse-render round-trips.

---

### APP-ADR-038: Z3 Subprocess as Tier 5 SMT Consistency

#### Problem

Approximately 19% of semi-formal expressions fall through to heuristic fallback because they contain arithmetic constraints (x > 5, latency <= 200), higher-order functions (f(g(x))), complex quantifiers (nested FOR ALL/EXISTS), or unstructured English. Propositional logic (Tier 3 gophersat) cannot express these theories. APP-ADR-034 rejected Z3 citing CGo/single-binary concerns, but subprocess invocation preserves the single-binary property.

#### Options

A) Z3 subprocess via SMT-LIB2 stdin/stdout — preserves single binary, graceful degradation. B) Z3 CGo binding — requires CGo toolchain, breaks single binary. C) Pure-Go SMT solver — no mature Go SMT library exists.

#### Decision

**Option A: Z3 subprocess via SMT-LIB2.** Z3 invoked via exec.CommandContext with SMT-LIB2 on stdin. 30-second timeout per check. Graceful degradation via exec.LookPath — when Z3 is not installed, Tier 5 is silently skipped. Supersedes APP-ADR-034 (pure-Go tiered consistency).

#### Consequences

New Tier 5 (SMT) in consistency checker alongside existing Tier 2 (graph), Tier 3 (SAT/gophersat), Tier 4 (heuristic). ddis contradict --tier 5 activates SMT analysis. Arithmetic, quantifier, and uninterpreted function contradictions now detectable. APP-ADR-034 superseded — gophersat retained for fast propositional path. Z3 is an optional runtime dependency, not a build dependency.

#### Tests

TestTranslateSMTLIB2_Arithmetic, TestRunZ3_Sat, TestRunZ3_Unsat, TestAnalyzeSMT_PairwiseContradiction, TestAPPINV021_EncodingFidelity (updated for SMT extension)

---

### APP-ADR-040: LLM-as-Judge Semantic Contradictions via Anthropic SDK

#### Problem

Tiers 2-5 detect structural, propositional, arithmetic, and heuristic contradictions but miss semantic conflicts: synonym collisions, implicit assumption mismatches, scope ambiguities, and temporal ordering violations. Estimated 25-40% of real contradictions are semantic.

#### Options

A: Anthropic SDK subprocess via Provider interface with majority vote. B: OpenAI SDK with function calling. C: Local LLM (ollama) for offline analysis. D: No LLM — expand heuristic patterns.

#### Decision

**Option A: Anthropic SDK via Provider interface.** Pairwise invariant comparison across domains with majority vote (3 runs, 2/3 agreement) boosting precision from 85% to 94%. Graceful degradation when ANTHROPIC_API_KEY absent. Tier 6 in consistency checker.

#### Consequences

New Tier 6 in consistency checker. Processes only pairs that Tiers 3-5 could not parse. Cost: ~$0.01 per invariant pair. Provider abstraction (APP-INV-054) enables graceful degradation. Majority vote protocol (APP-INV-055) ensures statistical soundness. Supersedes APP-ADR-040 general design with specific detection mechanics.

#### Tests

TestTier6_SemanticConflictDetected, TestTier6_NoFalsePositive, TestTier6_GracefulDegradation, TestTier6_MajorityVote

---

**APP-INV-054: LLM Provider Graceful Degradation**

*The LLM provider MUST degrade gracefully when no API key is configured: all LLM-dependent features skip silently, no command fails due to missing LLM configuration.*

```
FOR ALL commands c that use LLM: NOT provider.Available() IMPLIES c succeeds with degraded_output AND c.exit_code = 0; provider.Available() IMPLIES c uses LLM for enhanced_output
```

Violation scenario: A user installs ddis without configuring ANTHROPIC_API_KEY. They run ddis contradict --tier 6 which attempts LLM-as-judge evaluation. The command crashes with "API key not found" error, breaking the offline-first contract.

Validation: Unset ANTHROPIC_API_KEY. Run ddis contradict --tier 6. Verify exit code 0, output mentions "Tier 6 skipped (no LLM provider)". Set ANTHROPIC_API_KEY. Re-run. Verify Tier 6 executes and produces results.

// WHY THIS MATTERS: DDIS is a single-binary CLI that must work without external services. The Z3 graceful degradation pattern (Z3Available()) proves this works. LLM features must follow the same pattern: enhance when available, degrade when not.

---

**APP-INV-055: Eval Evidence Statistical Soundness**

*The eval witness evidence type MUST use majority vote (3 independent LLM runs, 2/3 agreement) to produce statistically sound confidence scores, recording prompt template, model ID, vote distribution, and raw responses for auditability.*

```
FOR ALL eval witnesses w: w.runs >= 3 AND w.agreement >= 2/3; w.confidence = IF agreement = 3/3 THEN 0.95 ELSE IF agreement = 2/3 THEN 0.75 ELSE REJECT; w.audit_trail INCLUDES {prompt_template, model_id, vote_distribution, raw_responses}
```

Violation scenario: An agent records an eval witness for APP-INV-035 using a single LLM call. The LLM returns "holds" with confidence 0.85. But the single evaluation was a false positive — the invariant actually has a subtle violation. The single-run confidence score enters the evidence accumulation pipeline and contributes to a false confirmed verdict.

Validation: Attempt to record an eval witness with runs < 3. Verify rejection. Record an eval witness with 3 runs, 2 agreeing. Verify confidence = 0.75. Record with 3/3 agreement. Verify confidence = 0.95. Verify audit trail contains prompt_template, model_id, vote_distribution, raw_responses fields.

// WHY THIS MATTERS: A single LLM judgment has ~85% precision, meaning ~15% of single-run evaluations are false positives. Majority vote with 3 independent runs and 2/3 agreement raises precision to ~94%. Without this protocol, eval witnesses introduce systematic overconfidence that Goodharts the confirmation threshold.

---

### APP-ADR-042: Tier 6 LLM-as-Judge Semantic Contradiction Detection

#### Problem

Tiers 2-5 detect structural (graph), propositional (SAT), heuristic (keyword), and arithmetic/quantifier (SMT) contradictions. But approximately 19% of semi-formal expressions contain semantic content that no formal method can evaluate: intent conflicts, domain assumption clashes, temporal ordering violations expressed in natural language.

#### Options

A) LLM-as-judge with majority vote on invariant pairs. B) Embedding similarity with cosine threshold. C) NLI model fine-tuned on spec language. D) Accept the semantic gap.

#### Decision

**Option A: LLM-as-judge using Anthropic Claude API.** For each pair of invariants whose semi-formals failed Tiers 3-5 parsing, prompt the LLM to classify the relationship as compatible, contradictory, or independent. Majority vote (3 runs, 2/3 agreement). Contradictory with 2/3 agreement yields confidence 0.80, with 3/3 agreement yields 0.95. Requires Provider.Available() for graceful degradation.

#### Consequences

New Tier 6 in consistency checker. Only processes pairs that Tiers 3-5 could not parse. Requires ANTHROPIC_API_KEY environment variable. Graceful degradation via Provider.Available(). Cost approximately 0.01 USD per invariant pair evaluation. APP-ADR-040 general design refined into specific detection mechanics.

#### Tests

TestTier6_SemanticConflictDetected, TestTier6_NoFalsePositive, TestTier6_GracefulDegradation, TestTier6_MajorityVote

---

### APP-ADR-051: Dedicated Rename over Patch --replace-all

#### Problem

patch enforces single-occurrence by design. No command for multi-file, multi-occurrence renames.

#### Options

A) ddis rename command. B) ddis patch --replace-all. C) External sed/rg.

#### Decision

**Option A: ddis rename.** Searches all spec source files and manifest. Optional --code-root for annotations. --dry-run. WHY NOT Option B? Patch single-occurrence is a safety feature. Rename is a different operation. WHY NOT Option C? Bypasses oplog, skips cross-reference validation.

#### Consequences

New command: ddis rename --old --new. New state transition: T_rename. Scans all source files + manifest + optionally code annotations.

#### Tests

Rename title: verify changes in module, constitution, manifest. --dry-run: no files modified. No occurrences: error.

---

---
