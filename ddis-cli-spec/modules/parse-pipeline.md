---
module: parse-pipeline
domain: parsing
maintains: [APP-INV-001, APP-INV-009, APP-INV-015]
interfaces: [APP-INV-002, APP-INV-003, APP-INV-005, APP-INV-007, APP-INV-008, APP-INV-016]
implements: [APP-ADR-001, APP-ADR-002, APP-ADR-005, APP-ADR-009, APP-ADR-010]
adjacent: [search-intelligence, query-validation, lifecycle-ops]
negative_specs:
  - "Must NOT silently drop markdown content during parsing"
  - "Must NOT assume heading hierarchy is always well-formed"
  - "Must NOT use non-deterministic operations in hash computation"
---

# Parse Pipeline Module

The parse pipeline is the foundation of the DDIS CLI. It transforms markdown specifications into a normalized, queryable SQLite index and reconstructs them back with byte-level fidelity. Every other module --- search, validation, lifecycle --- depends on the index this module produces.

This module owns three invariants that together guarantee the core contract: what goes in must come out identically (APP-INV-001), the two input formats must produce equivalent results (APP-INV-009), and content identity must be computed deterministically (APP-INV-015).

**Invariants referenced from other modules (INV-018 compliance):**

- APP-INV-002: Validation Determinism --- results independent of clock, RNG, execution order (maintained by query-validation module)
- APP-INV-003: Cross-Reference Integrity --- every resolved reference points to an existing element (maintained by query-validation module)
- APP-INV-005: Context Self-Containment --- bundles include all 9 intelligence signals for LLM consumption (maintained by search-intelligence module)
- APP-INV-007: Diff Completeness --- structural diff reports every add/remove/modify, no silent drops (maintained by query-validation module)
- APP-INV-008: RRF Fusion Correctness --- score equals weighted sum across all ranking signals (maintained by search-intelligence module)
- APP-INV-016: Implementation Traceability --- every invariant with implementation claims has valid Source/Tests/Validates-via paths (maintained by lifecycle-ops module)

---

## Invariants Maintained by This Module

---

**APP-INV-001: Round-Trip Fidelity**

*For every valid specification, parsing it into the index and rendering it back produces byte-identical output.*

```
FOR ALL spec IN ValidSpecs:
  render(parse(spec)) = spec     (byte-level identity)
  WHERE:
    parse  = ParseDocument (monolith) OR ParseModularSpec (modular)
    render = RenderMonolith (monolith) OR RenderModular (modular)
    =      denotes byte-for-byte equality including whitespace, blank lines, and trailing newlines
```

Violation scenario: The renderer omits a blank line that separates two invariant blocks. The rendered output is 3,416 bytes; the original was 3,417 bytes. A subsequent parse of the rendered output merges the two invariant blocks into one because the blank-line separator is missing, silently corrupting the index.

Validation: Parse any valid DDIS specification (monolith or modular). Render the result back to the filesystem. Byte-compare the original input with the rendered output. Any difference --- including trailing whitespace, blank lines between sections, or horizontal rule formatting --- constitutes a violation. Run on at least 3 distinct specs (small, medium, and the DDIS standard itself).

// WHY THIS MATTERS: The parse-render cycle is the foundation of transactional editing. If round-trip fidelity fails, every commit or rollback silently corrupts the specification. Downstream validation and search operate on a different document than the author wrote.

**Confidence:** property-checked

**Implementation Trace:**
- Source: `internal/parser/document.go::ParseDocument`
- Source: `internal/renderer/monolith.go::RenderMonolith`
- Source: `internal/renderer/modular.go::RenderModular`
- Source: `internal/parser/document.go::extractFormattingHints`
- Tests: `tests/roundtrip_test.go::TestRoundTripMonolith`
- Tests: `tests/roundtrip_test.go::TestRoundTripModular`

---

**APP-INV-009: Monolith-Modular Equivalence**

*Parsing a monolith specification and parsing its corresponding modular form (manifest + modules) produces semantically equivalent index content: identical element counts, identical content hashes per element, and identical cross-reference graphs.*

```
FOR ALL spec HAVING modular_form:
  LET mono  = elements(ParseDocument(monolith_path))
  LET mod   = elements(ParseModularSpec(manifest_path))
  THEN:
    |mono.invariants| = |mod.invariants|
    AND |mono.adrs| = |mod.adrs|
    AND |mono.gates| = |mod.gates|
    AND FOR ALL inv IN mono.invariants:
      EXISTS inv' IN mod.invariants:
        inv.invariant_id = inv'.invariant_id
        AND inv.content_hash = inv'.content_hash
    AND |mono.cross_references| APPROX= |mod.cross_references|  (within 5%)
```

Violation scenario: An invariant definition spans a module boundary in the modular form --- the header is in `core-standard.md` but the violation scenario continues in `element-specifications.md`. The modular parser, processing files independently, captures only the header and statement, producing an invariant with a NULL violation scenario. The monolith parser captures the full block. The two indices disagree on invariant completeness.

Validation: Parse both the monolith (`ddis_final.md`) and modular (`ddis-modular/manifest.yaml`) forms of the same specification. Compare element counts per type (invariants, ADRs, gates, negative specs, glossary entries). Compare content hashes for invariants and ADRs by their IDs. Cross-reference counts may differ slightly because module frontmatter introduces additional section boundaries, but the difference must be within 5%.

// WHY THIS MATTERS: Users must be able to switch between monolith and modular forms freely. If parsing produces different indices, validation results and search rankings diverge depending on which form was parsed --- an invisible, silent correctness failure.

**Confidence:** property-checked

**Implementation Trace:**
- Source: `internal/parser/document.go::ParseDocument`
- Source: `internal/parser/manifest.go::ParseModularSpec`
- Source: `internal/parser/manifest.go::parseAndInsertFile`
- Source: `internal/parser/document.go::extractElementsFromFile`
- Tests: `tests/diff_test.go::TestDiffMonolithVsModular`

---

**APP-INV-015: Deterministic Hashing**

*The SHA-256 hash function used for content identity is pure: given identical input bytes, it produces identical output across all invocations, processes, and time. No salt, timestamp, nonce, or random seed is included in the hash computation.*

```
FOR ALL content IN Strings, FOR ALL t1, t2 IN Time:
  sha256Hex(content, t1) = sha256Hex(content, t2)
  WHERE sha256Hex(s) = fmt.Sprintf("%x", sha256.Sum256([]byte(s)))
  AND the function takes NO additional parameters
```

Violation scenario: A developer adds a timestamp to the hash computation for "debugging purposes": `sha256Hex(content + time.Now().String())`. Every parse of the same spec now produces different content hashes. The diff command reports every element as modified. The transaction system cannot detect no-op edits. Change intelligence is useless.

Validation: Hash the same string content twice in the same process, then twice in a separate process. All four hashes must be identical. Inspect the `sha256Hex` function source to confirm it takes only a single string parameter and calls `sha256.Sum256` with no additional input.

// WHY THIS MATTERS: Content hashes are the identity mechanism for change detection, diff computation, and transaction integrity. Non-deterministic hashing makes every operation unreliable --- changes appear where none exist, and real changes may be masked by hash collisions with stale cached values.

**Confidence:** property-checked

**Implementation Trace:**
- Source: `internal/parser/sections.go::sha256Hex`
- Tests: `tests/parser_test.go::TestParsePopulatesIndex` (verifies content_hash fields are populated)
- Validates-via: `internal/parser/sections.go::InsertSectionsDB` (uses sha256Hex for section hashes)
- Validates-via: `internal/parser/invariants.go::ExtractInvariants` (uses sha256Hex for invariant hashes)
- Validates-via: `internal/parser/adrs.go::ExtractADRs` (uses sha256Hex for ADR hashes)

---

## Architecture Decision Records

---

### APP-ADR-001: Go over Rust

#### Problem

What implementation language should the DDIS CLI use?

#### Options

A) **Go** --- Fast compilation, pure-Go SQLite via modernc.org/sqlite, excellent CLI ecosystem (Cobra), single-binary deployment, straightforward concurrency model.
- Pros: Sub-second compile times enable tight RALPH iteration loops. Pure-Go SQLite avoids CGo complexity and cross-compilation issues. Cobra provides battle-tested CLI scaffolding with subcommands, flags, and shell completion.
- Cons: No algebraic types; error handling is verbose; no borrow checker for memory safety.

B) **Rust** --- Maximum runtime performance, strong type system with algebraic data types, ownership model prevents data races.
- Pros: Pattern matching would simplify parser state machines. Zero-cost abstractions for performance-critical code paths.
- Cons: Compilation times 10-50x slower than Go (critical for RALPH loops that rebuild frequently). SQLite bindings require either unsafe FFI or CGo-style linking. Steeper learning curve for contributors.

C) **TypeScript** --- Rapid prototyping, large ecosystem, familiar to most developers.
- Pros: Fastest prototyping speed. npm ecosystem for parsing and CLI utilities.
- Cons: No mature pure-JS SQLite with the reliability of modernc.org/sqlite. Numerical computations for LSI (SVD, matrix operations) lack a gonum-equivalent library with precision guarantees. Runtime dependency (Node.js or Bun).

#### Decision

**Option A: Go.** The DDIS CLI is I/O-bound (SQLite queries, file reads, markdown parsing), not CPU-bound. Go's compilation speed directly impacts RALPH loop iteration time --- the CLI is rebuilt on every improvement cycle. Pure-Go SQLite (modernc.org/sqlite) eliminates CGo complexity entirely.

// WHY NOT Rust? Compilation speed matters more than runtime speed for this tool. A RALPH iteration rebuilds the CLI, parses a spec, runs validation, and computes search indices. Go builds in < 2 seconds; Rust builds take 30-60 seconds. Over 50 RALPH iterations, that is 25-50 minutes of waiting for compilation alone.

// WHY NOT TypeScript? Numerical accuracy is non-negotiable for LSI (Singular Value Decomposition) and PageRank (eigenvector iteration). gonum provides IEEE 754 double-precision guarantees. No TypeScript library offers equivalent precision control for matrix operations.

#### Consequences

- All numerical code uses gonum (matrix operations, SVD for LSI, eigenvector iteration for PageRank)
- SQLite via modernc.org/sqlite (pure Go, no CGo, no cross-compilation issues)
- CLI framework: Cobra (APP-ADR-004, maintained by query-validation module)
- Fast edit-compile-test cycle supports RALPH iteration (< 2s from source change to running binary)

#### Tests

- Build succeeds on linux/amd64 without CGo: `CGO_ENABLED=0 go build ./...`
- No `import "C"` statements in the codebase
- `go test ./...` passes without CGo

---

### APP-ADR-002: SQLite as Sole Storage Backend

#### Problem

What storage backend should the spec index use?

#### Options

A) **SQLite (pure Go via modernc.org/sqlite)** --- Single-file embedded database, zero configuration, ACID transactions, full SQL query capability.
- Pros: No server process. Database is a single file that can be committed to version control, copied, or diffed. Supports complex joins needed for cross-reference resolution. FTS5 extension for full-text search. Transactional safety for the operation log.
- Cons: Single-writer concurrency model. No built-in replication.

B) **JSON files** --- One JSON file per element type (invariants.json, adrs.json, etc.).
- Pros: Human-readable. Easy to diff with standard tools. No database dependency.
- Cons: No query capability --- cross-reference joins require loading all files into memory. No transactional guarantees. O(n) search without indexing. Cross-element queries (e.g., "all invariants referenced by this ADR") require application-level joins.

C) **PostgreSQL** --- Full relational database with advanced features.
- Pros: Concurrent writers. Advanced query optimizer. Extensions ecosystem.
- Cons: Requires a running server. Deployment complexity contradicts single-binary goal. Overkill for a tool that operates on one spec at a time.

#### Decision

**Option A: SQLite.** The 30-table normalized schema (APP-ADR-005) requires efficient joins --- finding all cross-references from a section, or all sections referencing an invariant, are multi-table operations. SQLite handles these in microseconds. The single-file nature means the index is portable: copy one `.db` file to have a complete, queryable spec index.

// WHY NOT JSON? Cross-reference resolution requires joining invariants, ADRs, gates, and sections. With JSON files, every cross-reference query loads 4+ files and performs O(n x m) matching in application code. With SQLite, it is a single indexed JOIN.

// WHY NOT PostgreSQL? The CLI parses one spec at a time in a single process. There are no concurrent writers. PostgreSQL's deployment overhead (server, authentication, connection strings) contradicts the single-binary, zero-configuration design.

#### Consequences

- Database file is `.ddis.db` alongside the spec
- Schema creation is automatic on first parse (CREATE TABLE IF NOT EXISTS)
- All queries use standard SQL; no ORM layer
- Transactions protect multi-table inserts during parsing (one parse = one atomic operation)
- FTS5 virtual table for full-text search (search-intelligence module)

#### Tests

- `storage.Open()` creates a valid database with all 30 tables
- Parse + render round-trip succeeds with the database as intermediary
- Concurrent read queries do not block (WAL mode)

---

### APP-ADR-005: 30-Table Normalized Schema

#### Problem

How should the parsed specification be stored in SQLite? A single denormalized table with JSON blobs, a handful of wide tables, or a fully normalized relational schema?

#### Options

A) **Single table with JSON columns** --- One `elements` table with `type`, `id`, `data JSON`.
- Pros: Simple schema. Easy to add new element types without migration.
- Cons: No referential integrity. Cross-reference queries require JSON extraction functions (slow, not indexed). Cannot enforce UNIQUE constraints on element IDs per type.

B) **Wide tables per category** --- 5-6 tables (invariants, adrs, sections, etc.) with many nullable columns.
- Pros: Moderate normalization. Decent query performance.
- Cons: Nullable columns mask missing data. No separate table for ADR options, budget entries, or state machine cells. Related data is flattened into the parent row.

C) **Fully normalized (30 tables)** --- One table per entity type, with foreign keys, CHECK constraints, and UNIQUE indexes.
- Pros: Referential integrity enforced by the database. Cross-reference queries are natural JOINs on indexed columns. Each table's columns are non-nullable where the spec requires data. Separate tables for sub-entities (adr_options, budget_entries, state_machine_cells, verification_checks) preserve structure.
- Cons: More complex inserts (must respect foreign key ordering). Schema migration required for new element types.

#### Decision

**Option C: Fully normalized, 30 tables.** The CLI's core value is cross-element queries: "Which sections reference INV-003?", "Which invariants lack violation scenarios?", "What is the authority score of ADR-005?" These queries are natural SELECTs with JOINs on a normalized schema. On a denormalized schema, they require parsing JSON blobs at query time.

// WHY NOT single-table JSON? The cross_references table has foreign keys to both source_section_id and needs to join against invariants, adrs, and gates tables for resolution. With JSON blobs, `ResolveCrossReferences` would need to parse every element's JSON to find matching IDs --- O(refs x elements) instead of O(refs x log(elements)) with indexed lookups.

// WHY NOT wide tables? ADR options, state machine cells, and verification checks are one-to-many relationships. Flattening them into parent rows loses structure (how many options? which one is chosen?) or requires parsing delimited strings.

#### Consequences

- 30 tables organized into 6 groups:
  - **Core**: spec_index, source_files, sections, formatting_hints
  - **Elements**: invariants, adrs, adr_options, quality_gates, negative_specs, verification_prompts, verification_checks, meta_instructions, worked_examples, why_not_annotations, comparison_blocks, performance_budgets, budget_entries, state_machines, state_machine_cells, glossary_entries
  - **Cross-references**: cross_references (with resolution status)
  - **Modular structure**: modules, module_relationships, module_negative_specs, manifest, invariant_registry
  - **Transactions**: transactions, tx_operations
  - **Search**: fts_index (FTS5), search_vectors, search_model, search_authority
- Foreign keys enforce consistency: a cross-reference cannot reference a nonexistent section
- CHECK constraints enforce valid enums (source_type, ref_type, check_type, rel_type)
- UNIQUE indexes prevent duplicate element IDs per spec

#### Tests

- `TestParsePopulatesIndex` verifies minimum row counts across element tables after parsing a real spec
- Schema creation is idempotent (CREATE TABLE IF NOT EXISTS)
- Foreign key violations cause insert failures (tested by attempting orphan inserts)

---

### APP-ADR-009: 4-Pass Parse Pipeline

#### Problem

In what order should parsing proceed? A single pass that extracts everything simultaneously, or multiple ordered passes where later passes depend on earlier results?

#### Options

A) **Single pass** --- Walk lines once, extracting sections, elements, and cross-references simultaneously.
- Pros: One iteration over the lines. Simplest loop structure.
- Cons: Cross-references encountered before their targets are parsed cannot be resolved. Section tree must be built incrementally, complicating parent assignment. Element extractors need the section tree to associate elements with their containing section.

B) **4-pass pipeline** --- (1) Build section tree, (2) extract elements, (3) extract cross-references, (4) resolve cross-references.
- Pros: Each pass has clear inputs and outputs. Pass 2 uses the complete section tree from pass 1 to correctly assign elements to sections. Pass 3 uses section IDs from the database. Pass 4 can resolve references against the fully populated index. Easier to test each pass independently.
- Cons: Four iterations over the lines. Slightly more total work.

C) **2-pass (build + resolve)** --- First pass builds everything; second pass resolves references.
- Pros: Fewer passes.
- Cons: Building sections and extracting elements in the same pass creates ordering dependencies (an element extractor may run before its containing section exists in the tree).

#### Decision

**Option B: 4-pass pipeline.** The dependency chain is strict: element extraction needs the section tree (pass 1 output), cross-reference extraction needs database IDs for elements (pass 2 output), and resolution needs all elements in the database (pass 3 output). Collapsing these into fewer passes creates temporal coupling and race conditions on database state.

// WHY NOT single pass? `ExtractInvariants` calls `FindSectionForLine(sections, i)` to associate each invariant with its containing section. If the section tree is incomplete (still being built), the invariant may be assigned to the wrong section or have a NULL section_id.

// WHY NOT 2-pass? Extracting 12 element types (invariants, ADRs, gates, negative specs, verification prompts, meta-instructions, worked examples, WHY NOT annotations, comparison blocks, performance budgets, state machines, glossary entries) alongside section tree construction creates a 1,000-line single function with complex interleaved state machines. Separating element extraction into pass 2 keeps each recognizer independent and testable.

#### Consequences

- `ParseDocument` orchestrates: `BuildSectionTree` -> `extractElementsFromLines` -> `ExtractCrossReferences` -> `ResolveCrossReferences`
- Each pass writes to the database; subsequent passes query it
- Adding a new element type means adding one function to pass 2 (no changes to other passes)
- Performance cost is negligible: 4 passes over a 3,000-line spec takes < 50ms total

#### Tests

- `TestParsePopulatesIndex` verifies all element types are extracted (pass 2)
- `TestCrossReferenceResolution` verifies >= 90% resolution rate (pass 4)
- `TestSectionTree` verifies known section paths exist in the tree (pass 1)

---

### APP-ADR-010: Monolith/Modular Polymorphism by Filename Detection

#### Problem

The CLI must support both monolith (single `.md` file) and modular (manifest.yaml + module files) specifications. How should it detect and dispatch between the two formats?

#### Options

A) **Explicit flag** --- `ddis parse --format=monolith` or `--format=modular`.
- Pros: Unambiguous. User controls behavior.
- Cons: Extra flag to remember. Errors if the user specifies the wrong format.

B) **Filename detection** --- If the input path ends in `.yaml` or `.yml`, treat as modular (manifest). If `.md`, treat as monolith.
- Pros: Zero-configuration. The manifest file is always YAML; monolith specs are always Markdown. No ambiguity.
- Cons: Cannot parse a YAML file as monolith or a Markdown file as modular. (Neither case makes sense.)

C) **Content sniffing** --- Read the first few lines and detect YAML frontmatter vs manifest structure.
- Pros: Works regardless of file extension.
- Cons: Fragile. Module files have YAML frontmatter too, creating ambiguity. A markdown file starting with `---` could be either.

#### Decision

**Option B: Filename detection.** The mapping is injective: `.yaml`/`.yml` files are always manifests; `.md` files are always monolith specs or module files (but module files are only parsed via their manifest, never directly). There is no ambiguous case.

// WHY NOT explicit flag? It adds cognitive overhead for no benefit. The file extension already encodes the format. A `--format` flag would be redundant and a source of user error.

// WHY NOT content sniffing? Module markdown files (e.g., `modules/core-standard.md`) begin with `---` (YAML frontmatter). A content sniffer would need to distinguish between module frontmatter and a manifest file --- both are valid YAML. Filename detection avoids this ambiguity entirely.

#### Consequences

- `ParseDocument` handles monolith (`.md`) inputs
- `ParseModularSpec` handles modular (`.yaml`/`.yml`) inputs
- Dispatch logic is a simple extension check at the CLI command level
- Both parsers produce the same schema; downstream commands (query, validate, search, diff) are format-agnostic
- The `spec_index.source_type` column records `"monolith"` or `"modular"` for provenance

#### Tests

- `TestRoundTripMonolith` parses `.md`, verifies monolith path
- `TestRoundTripModular` parses `manifest.yaml`, verifies modular path
- `TestDiffMonolithVsModular` proves both formats produce comparable indices

---

## Implementation Chapters

### Chapter 1: The 4-Pass Parse Pipeline

**Preserves:** APP-INV-001 (Round-Trip Fidelity --- parse-render produces byte-identical output), APP-INV-015 (Deterministic Hashing --- SHA-256 with no salt).

The parse pipeline transforms a markdown specification into a normalized SQLite index through four strictly ordered passes. Each pass depends on the output of the previous one.

#### Pass 1: Section Tree Construction

`BuildSectionTree` walks every line, matching against `HeadingRe` (`^(#{1,6})\s+(.+)$`). For each heading found, it creates a `SectionNode` with:

- `SectionPath`: normalized via `normalizeSectionPath` (PART headings become `PART-N`, numbered sections become `section-N.M`, chapters become `Chapter-N`, appendices become `Appendix-X`, everything else is slugified)
- `HeadingLevel`: 1-6 derived from the count of `#` characters
- `LineStart`: 0-indexed line of the heading
- `LineEnd`: 0-indexed line of the next same-or-higher-level heading (or EOF)
- `ParentIdx`: index into the flat list, resolved by walking a stack of most-recent headings at each level

A stack of depth 7 (indices 0-6, index 0 unused) tracks the most recent heading at each level. When a new heading at level L is encountered, all stack entries at levels > L are cleared. Parent resolution walks the stack downward from level L-1 to find the nearest ancestor.

`InsertSectionsDB` then inserts each node into the `sections` table with:
- 1-indexed line numbers (database convention)
- `raw_text` extracted by joining lines[start:end]
- `content_hash` from `sha256Hex(raw_text)`

#### Pass 2: Element Extraction

`extractElementsFromLines` runs 12 independent recognizers sequentially. Each recognizer is a state machine that walks the lines array, identifies blocks matching its pattern, and inserts rows into the corresponding table. The recognizers are:

1. `ExtractInvariants` --- state machine: idle -> headerSeen -> statementSeen -> inCodeBlock -> codeDone -> afterCode. Terminates on `---` or next invariant header.
2. `ExtractADRs` --- state machine: idle -> headerSeen -> inProblem -> inOptions -> inDecision -> inConsequences -> inTests. Tracks option labels and chosen option.
3. `ExtractGates` --- matches `GateRe` pattern for quality gate blocks.
4. `ExtractNegativeSpecs` --- matches `NegSpecRe` for **DO NOT** constraints.
5. `ExtractVerificationPrompts` --- matches `VerifPromptRe` for structured self-checks.
6. `ExtractMetaInstructions` --- matches `MetaInstrRe` for META-INSTRUCTION blocks.
7. `ExtractWorkedExamples` --- matches `WorkedExampleRe` for worked example blocks.
8. `ExtractWhyNots` --- matches `WhyNotRe` for WHY NOT annotations.
9. `ExtractComparisonBlocks` --- matches comparison markers for suboptimal/chosen approach pairs.
10. `ExtractPerformanceBudgets` --- matches `PerfBudgetHeaderRe` for budget tables.
11. `ExtractStateMachines` --- matches `StateMachineHeaderRe` for state-event tables.
12. `ExtractGlossaryEntries` --- matches `GlossaryRowRe` for glossary table rows.

Every recognizer calls `FindSectionForLine` to associate extracted elements with their containing section (the deepest section whose `[LineStart, LineEnd)` range includes the element's line).

#### Pass 3: Cross-Reference Extraction

`ExtractCrossReferences` walks every line (skipping code blocks) and matches four regex patterns:

- `XRefSectionRe`: `§(\d+(?:\.\d+)*)` --- captures section references like `§0.5`, `§3.8`
- `XRefInvRe`: `((?:APP-)?INV-\d{3})` --- captures invariant references like `INV-001`, `APP-INV-003`
- `XRefADRRe`: `((?:APP-)?ADR-\d{3})` --- captures ADR references like `ADR-002`, `APP-ADR-005`
- `XRefGateRe`: `Gate\s+((?:M-)?[1-9]\d*)` --- captures gate references like `Gate 3`, `Gate M-1`

Each match is inserted into the `cross_references` table with `resolved = 0`. Definition lines (invariant headers, ADR headers, gate definitions) are excluded to prevent self-references.

#### Pass 4: Cross-Reference Resolution

`ResolveCrossReferences` queries all unresolved cross-references for the spec, then for each one checks whether the target exists:

- `section` type: queries `sections` for matching `section_path`
- `invariant`/`app_invariant` type: queries `invariants` for matching `invariant_id`
- `adr`/`app_adr` type: queries `adrs` for matching `adr_id`
- `gate` type: queries `quality_gates` for matching `gate_id`

Resolved references are updated with `resolved = 1`.

#### Worked Example: Parsing a Small Fragment

Consider this 12-line markdown fragment:

```markdown
## 0.5 Invariants

**INV-001: Causal Traceability**

*Every implementation section traces to at least one ADR.*

Violation scenario: An implementation chapter has no ADR reference.

Validation: Pick 5 random sections, trace references backward.

// WHY THIS MATTERS: Without traceability, sections accumulate without justification.

---
```

**Pass 1 (Section Tree):** `BuildSectionTree` finds one heading at line 0 (`## 0.5 Invariants`), creating a `SectionNode` with `SectionPath = "§0.5"`, `HeadingLevel = 2`, `LineStart = 0`, `LineEnd = 12` (EOF).

**Pass 2 (Element Extraction):** `ExtractInvariants` identifies the invariant block:
- Line 2: `InvHeaderRe` matches -> state = `headerSeen`, `invariant_id = "INV-001"`, `title = "Causal Traceability"`
- Line 4: `InvStatementRe` matches -> state = `statementSeen`, `statement = "Every implementation section traces to at least one ADR."`
- Line 6: `ViolationRe` matches -> state = `afterCode`, `violation_scenario = "An implementation chapter has no ADR reference."`
- Line 8: `ValidationRe` matches -> `validation_method = "Pick 5 random sections, trace references backward."`
- Line 10: `WhyMattersRe` matches -> `why_this_matters = "Without traceability, sections accumulate without justification."`
- Line 12: `---` -> flush. `content_hash = sha256Hex(raw_text)`. Insert into `invariants` table.

**Pass 3 (Cross-Reference Extraction):** `ExtractCrossReferences` scans lines:
- Line 4: `XRefADRRe` does not match "ADR" (the word "ADR" without the dash-number suffix is not a reference pattern)
- No other reference patterns found in this fragment

**Pass 4 (Cross-Reference Resolution):** No unresolved references to process.

---

### Chapter 2: Schema Design

**Preserves:** APP-INV-001 (Round-Trip Fidelity --- raw_text stored verbatim enables byte-identical rendering), APP-INV-015 (Deterministic Hashing --- content_hash columns use sha256Hex).

The 30-table schema is organized into six groups. Each table has a specific role in the pipeline.

#### Core Tables (4 tables)

| Table | Purpose | Key Columns |
|---|---|---|
| `spec_index` | One row per parsed spec | `spec_path`, `content_hash`, `source_type` (monolith/modular) |
| `source_files` | One row per input file (1 for monolith, N for modular) | `file_role` (monolith/manifest/system_constitution/module), `raw_text` |
| `sections` | Heading-delimited tree structure | `section_path`, `heading_level`, `parent_id`, `raw_text`, `content_hash` |
| `formatting_hints` | Blank lines and horizontal rules for round-trip fidelity | `line_number`, `hint_type` (blank_line/hr) |

The `source_files.raw_text` column is the single source of truth for rendering. `RenderMonolith` queries this column directly; `RenderModular` queries all source files and writes each back to its original path.

#### Element Tables (16 tables)

| Table | Element Type | Foreign Keys |
|---|---|---|
| `invariants` | Invariant blocks | `spec_id`, `source_file_id`, `section_id` |
| `adrs` | Architecture Decision Records | `spec_id`, `source_file_id`, `section_id` |
| `adr_options` | Per-ADR options (1:N from `adrs`) | `adr_id` |
| `quality_gates` | Quality gate definitions | `spec_id`, `section_id` |
| `negative_specs` | DO NOT constraints | `spec_id`, `source_file_id`, `section_id` |
| `verification_prompts` | Verification prompt blocks | `spec_id`, `section_id` |
| `verification_checks` | Individual checks (1:N from prompts) | `prompt_id` |
| `meta_instructions` | META-INSTRUCTION directives | `spec_id`, `section_id` |
| `worked_examples` | Worked example blocks | `spec_id`, `section_id` |
| `why_not_annotations` | WHY NOT annotations | `spec_id`, `section_id` |
| `comparison_blocks` | Suboptimal/chosen comparison pairs | `spec_id`, `section_id` |
| `performance_budgets` | Performance budget headers | `spec_id`, `section_id` |
| `budget_entries` | Individual budget rows (1:N from budgets) | `budget_id` |
| `state_machines` | State machine blocks | `spec_id`, `section_id` |
| `state_machine_cells` | State x event cells (1:N from machines) | `machine_id` |
| `glossary_entries` | Term-definition pairs | `spec_id`, `section_id` |

Every element table has `section_id` as a foreign key, establishing the containment relationship: which section does this element live in? This enables queries like "all invariants in section §0.5" via a single indexed join.

#### Cross-Reference Table (1 table)

The `cross_references` table records every explicit reference found in the spec:

- `source_section_id` -> `sections(id)`: which section contains this reference
- `ref_type`: one of `section`, `invariant`, `adr`, `gate`, `app_invariant`, `app_adr`, `glossary_term`
- `ref_target`: the identifier being referenced (e.g., `"§0.5"`, `"INV-001"`, `"ADR-002"`)
- `resolved`: 0 or 1, set by pass 4

The `ref_type` + `ref_target` pair forms the cross-reference graph that the search-intelligence module uses for PageRank computation (APP-INV-004, maintained by search-intelligence) and that the query-validation module uses for integrity checking (APP-INV-003, maintained by query-validation).

#### Modular Structure Tables (5 tables)

| Table | Purpose |
|---|---|
| `modules` | One row per module, links to source_file |
| `module_relationships` | Maintains/interfaces/implements/adjacent relationships |
| `module_negative_specs` | Per-module negative specifications from manifest |
| `manifest` | Parsed manifest metadata (budget, tier mode) |
| `invariant_registry` | Owner/domain/description per invariant from manifest |

#### Transaction Tables (2 tables)

| Table | Purpose |
|---|---|
| `transactions` | Transaction state (pending/committed/rolled_back) |
| `tx_operations` | Ordered operations within a transaction |

#### Search Tables (3 tables)

| Table | Purpose |
|---|---|
| `fts_index` | FTS5 virtual table for BM25 full-text search |
| `search_vectors` | LSI document vectors (BLOB of k floats) |
| `search_model` | Serialized LSI model metadata |
| `search_authority` | PageRank authority scores per element |

---

### Chapter 3: Monolith/Modular Polymorphism

**Preserves:** APP-INV-009 (Monolith-Modular Equivalence --- both forms produce same index).

The CLI supports two input formats that produce the same normalized schema. Downstream commands (query, validate, search, diff, context) are format-agnostic --- they operate on the schema, never on raw files.

#### Monolith Path: `ParseDocument`

1. Read the single `.md` file into memory
2. Compute `content_hash = sha256Hex(fullText)`
3. Insert one `spec_index` row with `source_type = "monolith"`
4. Insert one `source_files` row with `file_role = "monolith"` and the full text in `raw_text`
5. Execute the 4-pass pipeline on the full line array

#### Modular Path: `ParseModularSpec`

1. Read and parse `manifest.yaml` via `ParseManifestFile` (YAML -> `ManifestData` struct)
2. Insert `spec_index` row with `source_type = "modular"`
3. Insert `manifest` row with budget/tier metadata and `raw_yaml`
4. Insert `invariant_registry` entries from the manifest
5. Insert manifest as a `source_files` row with `file_role = "manifest"`
6. Parse constitution file: `parseAndInsertFile` reads the file, inserts as `source_files` with `file_role = "system_constitution"`, builds section tree, inserts sections, then `extractElementsFromFile` runs passes 2-3
7. Parse each module file: same as constitution but with `file_role = "module"` and additional `modules`, `module_relationships`, and `module_negative_specs` rows
8. After all files: `ResolveCrossReferences` runs once across the entire spec (pass 4 operates on the unified index)

The key difference: the monolith parser runs all 4 passes on one line array. The modular parser runs passes 1-3 per file, then pass 4 once across all files. This ensures cross-references between modules (e.g., an invariant in `core-standard.md` referenced from `element-specifications.md`) are resolved correctly.

#### `parseAndInsertFile` --- The Shared Building Block

Both paths use the same underlying functions. `parseAndInsertFile` reads a file, inserts it as a `source_files` row, calls `BuildSectionTree` and `InsertSectionsDB`. Then `extractElementsFromFile` calls `loadSectionDBIDs` (queries section IDs back from the database) before running element extractors and cross-reference extraction.

---

### Chapter 4: Round-Trip Render Mechanism

**Preserves:** APP-INV-001 (Round-Trip Fidelity --- parse-render produces byte-identical output).

The render mechanism is intentionally simple: it stores the raw text verbatim during parsing and writes it back unchanged during rendering. No reconstruction from parsed fields. No reformatting.

#### `RenderMonolith`

Queries `source_files` for the row with `file_role = 'monolith'` (or `'system_constitution'` as fallback). Writes `raw_text` to a temporary file, then atomically renames it to the output path. The temporary file + rename pattern prevents partial writes from corrupting the output.

#### `RenderModular`

Queries all `source_files` rows (excluding the manifest), writes each to its original relative path under the output directory (creating subdirectories as needed). Then queries the `manifest` table for `raw_yaml` and writes it as `manifest.yaml`.

#### Formatting Hints

The `formatting_hints` table records blank lines and horizontal rules with their exact line numbers. While the current renderer uses `raw_text` directly (making these hints redundant for rendering), they serve two purposes:

1. Validation: the query-validation module can verify that formatting is consistent (e.g., every invariant block ends with `---`)
2. Future: if the renderer is ever changed to reconstruct from parsed fields instead of raw text, formatting hints ensure blank lines and horizontal rules are preserved

The `extractFormattingHints` function records:
- `hint_type = "blank_line"` for empty lines (trimmed to empty)
- `hint_type = "hr"` with `hint_value = "---"` for horizontal rules (excluding frontmatter delimiters)

---

## Negative Specifications

The following constraints prevent the most likely failure modes in the parse pipeline.

**DO NOT** silently drop markdown content during parsing. Every line in the input file MUST map to exactly one section's `raw_text`. If a line falls between the end of one section and the start of the next (e.g., blank lines before the first heading), it must still be captured in `source_files.raw_text` for round-trip fidelity. Violation of this constraint breaks APP-INV-001.

**DO NOT** assume heading hierarchy is always well-formed. A spec may jump from `##` directly to `####` (skipping `###`). The `BuildSectionTree` stack must handle level gaps: when a heading at level 4 is encountered with no preceding level 3, parent resolution walks the stack from level 3 down to level 1, finding the nearest ancestor. Do not insert a synthetic level-3 heading or reject the input.

**DO NOT** use non-deterministic operations in hash computation. The `sha256Hex` function must take exactly one parameter (the content string) and call `sha256.Sum256` with no additional input. No timestamps, process IDs, random nonces, or environment variables may be included. Violation breaks APP-INV-015 and cascades to break diff, transactions, and change detection.

**DO NOT** produce empty sections. A section's `raw_text` is derived from `lines[LineStart:LineEnd]`. Even if a section contains only its heading and blank lines, it must have non-empty `raw_text` (at minimum, the heading line itself). The section tree construction guarantees this because `LineStart` is the heading line and `LineEnd` is at least `LineStart + 1`.

**DO NOT** resolve cross-references to template placeholders. Patterns like `§X.Y`, `INV-NNN`, or `ADR-NNN` where the number portion is literally `X`, `Y`, or `NNN` (template variables in guidance text) must not be inserted into the `cross_references` table. The regex patterns `XRefSectionRe`, `XRefInvRe`, and `XRefADRRe` use `\d{3}` (three digits), which correctly excludes alphabetic placeholders.

**DO NOT** process cross-references inside code blocks. Fenced code blocks (delimited by triple backticks or more) contain example text, pseudocode, and SQL queries that use identifiers like `INV-001` illustratively. `ExtractCrossReferences` tracks code fence state and skips all lines inside fenced blocks.

**DO NOT** overwrite section database IDs during the modular parse. `loadSectionDBIDs` queries section IDs from the database to populate in-memory `SectionNode.DBID` fields. It must match by both `section_path` AND `line_start` (adjusted for 0-indexed vs 1-indexed) to avoid assigning the wrong ID when two files have sections with the same path (e.g., both have a "Glossary" section).

---

## Verification Prompt for Parse Pipeline Module

After implementing or modifying the parse pipeline, execute the following checks:

**Positive checks (DOES):**

1. DOES parsing a monolith spec and rendering it back produce byte-identical output? (APP-INV-001)
2. DOES parsing a modular spec and rendering it back produce byte-identical files for every module, the constitution, and the manifest? (APP-INV-001)
3. DOES parsing a monolith and its corresponding modular form produce the same invariant count, ADR count, and gate count? (APP-INV-009)
4. DOES `sha256Hex("hello")` return the same value when called 100 times? (APP-INV-015)
5. DOES the parser correctly handle heading level gaps (e.g., `##` followed by `####` with no `###`)? (Negative spec 2)
6. DOES `ExtractCrossReferences` skip references inside fenced code blocks? (Negative spec 6)
7. DOES `ResolveCrossReferences` achieve >= 90% resolution rate on a real DDIS specification? (Pass 4 correctness)

**Negative checks (does NOT):**

1. Does NOT the renderer modify whitespace, blank lines, or horizontal rules compared to the original input. (APP-INV-001)
2. Does NOT `sha256Hex` accept any parameter other than the content string --- no timestamps, seeds, or salts. (APP-INV-015)
3. Does NOT the parser silently drop lines that fall outside any section boundary. (Negative spec 1)
4. Does NOT the modular parser fail when the manifest references a module file that contains cross-references to elements in another module file. (APP-INV-009, pass 4 runs after all files)
5. Does NOT `ExtractInvariants` insert an invariant with an empty statement field. (Data integrity)
