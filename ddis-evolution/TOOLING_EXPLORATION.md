# DDIS Tooling Ecosystem — Design Exploration

## Context

DDIS v3.0 is an 88/100 specification standard with a working recursive improvement loop (RALPH), modular decomposition, assembly tooling, and validation checks. The spec itself is excellent — but the *tooling around it* is primitive. This exploration honestly articulates what an LLM needs to work with DDIS at its full potential, across both human-driven authoring and autonomous improvement workflows.

**Design decisions from discussion:**
- **Workflow**: Both human-driven (conversational authoring) and LLM-autonomous (RALPH loops) equally
- **Form factor**: CLI-first (integrates with RALPH, CI, any agent), with MCP server layered on later
- **Composition model**: Three-tier modularization IS the composition model. "Cross-spec" is really "cross-domain within one system spec." The gap is tooling support for three-tier, not a new abstraction.

---

## The Honest Problem

I've spent multiple sessions reading, improving, evaluating, and modularizing this spec. Here is my genuine, unvarnished assessment of what I struggle with.

### What I Actually Experience Working With This Spec

**Context starvation.** The monolithic spec is 3,101 lines (~25% of my context window). If I also need the improvement methodology, previous judgments, and room to produce output — I'm working in a cramped space. Modularization helps (bundles are 967-1,471 lines) but isn't enough alone, because I still need cross-cutting knowledge from other modules.

**Reference amnesia.** When I encounter "See INV-006" in §4.2, I need to know what INV-006 says. If it's not in my current context, I either skip the check (silent failure) or reconstruct what I think it says (hallucination). Even with INV-018's restatement requirement, I lose track of the full invariant system across 3,000 lines.

**Inability to self-validate.** After writing a spec section, I cannot mechanically verify I've satisfied INV-017 (enough DO NOTs?), INV-006 (orphan section?), or INV-003 (every invariant has a violation scenario?). I'm doing pattern-matching on my own output — the same brain checking its own homework. The v17 evaluation found issues the RALPH judge missed.

**Destructive rewrites.** When improving a 3,000-line spec, I rewrite the ENTIRE document. Every iteration risks dropping content, introducing inconsistencies, or regressing on previously-satisfied invariants. The v1→v2 modularization attempt lost 53% of content. Full rewrites are my most error-prone operation.

**Impact blindness.** When a user says "add a new invariant," I don't systematically trace what else changes: the invariant table, namespace note, quality gates, module manifest, invariant registry, referencing chapters, master TODO. I'll miss at least one every time.

**No memory between turns.** In a conversational session, each message resets my working state. If we're iterating on §4.2 across 5 exchanges, I'm re-reading and re-analyzing the spec fragment each time.

---

## The Six Tool Categories

### 1. Spec Intelligence Layer — Parse, Index, Query

**The foundation everything else builds on.** A DDIS-aware parser that transforms raw markdown into a queryable structure.

What it produces:
- **Section tree**: Every heading → path (§0.2.2), level, line range, parent/children
- **Entity index**: Every INV-NNN, ADR-NNN, Gate-N with: ID, one-line summary, full definition location, all reference sites
- **Cross-reference graph**: Directed edges from every reference to its target
- **Glossary index**: Every defined term → definition location, all usage sites
- **Negative spec index**: Every DO NOT constraint → location, serving invariant
- **Verification prompt index**: Every verification block → location, checked invariants
- **Meta-instruction index**: Every META-INSTRUCTION → location, ordering constraint

Why this matters: Every other tool needs to answer "where is INV-006 defined?" or "what references §3.2?" Without this layer, each tool re-parses from scratch.

**CLI**: `ddis parse spec.md` → `spec_index.json`

### 2. Surgical Retrieval — Context-Optimized Fragment Assembly

**The single most impactful tool for LLM effectiveness.** Instead of loading 3,000 lines, load exactly what's needed.

Core operations:
- **Section + dependencies**: "Give me §3.2 + all invariants it references + glossary terms it uses + its verification prompt" → 200-400 lines instead of 3,000
- **Invariant bundle**: "Give me INV-006 full definition + every section that references it" → understand one invariant completely
- **Implementation context**: "Give me everything I need to implement Chapter 5" → the chapter + prerequisites + adjacent interfaces + relevant ADRs + glossary
- **Backlinks**: "What references §3.2?" → all pointing sections, invariants, ADRs
- **Dependency chain**: "Trace §4.1 back to first principles" → §4.1 → INV-004 → §0.2 formal model → design goals
- **Domain bundle** (three-tier): "Give me the full context for working on the auth domain" → system constitution + domain constitution + all domain modules

Fragments must be **self-contained** — an LLM receiving just the fragment should be able to work without the rest. This means resolving references inline, including glossary terms, and adding orientation preambles.

**CLI**: `ddis query §3.2 --resolve-refs --include-glossary` / `ddis query INV-006 --backlinks` / `ddis bundle auth-domain`

### 3. Mechanical Validation — Invariant Checking Without LLM Reasoning

**Things that should never require an LLM to verify.** Pure structural/textual checks:

Fully automatable:
- **Cross-reference integrity**: All §X.Y, INV-NNN, ADR-NNN targets exist. No broken links.
- **INV-017 compliance**: Count DO NOT / MUST NOT per implementation chapter. Flag below threshold.
- **INV-006 compliance**: Orphan section detection. Cross-reference density computation.
- **INV-003 compliance**: Every invariant has: statement, semi-formal, violation scenario, validation method.
- **INV-009 compliance**: Every domain term has a glossary entry.
- **INV-013 compliance**: Each invariant maintained by exactly one module (modular specs).
- **INV-014 compliance**: Bundle sizes within hard ceiling.
- **INV-015 compliance**: Constitution declarations match module definitions.
- **INV-016 compliance**: Manifest reflects current file state.
- **Structural conformance (Gate 1)**: All required elements present per §0.3 template.
- **Proportional weight**: Chapter sizes within ±20% guidance.
- **Namespace consistency**: Counts in namespace notes match actual counts.
- **Glossary completeness**: Terms used but undefined; terms defined but unused.

Hybrid (need LLM assistance):
- **INV-002 (Decision Completeness)**: Design choices without ADRs? (Needs judgment)
- **INV-008 (Self-Containment)**: Spec sufficient for correct v1? (Needs simulation)
- **Gate 6 (Implementation Readiness)**: Zero clarifying questions? (Needs LLM role-play)
- **Gate 7 (LLM Implementation Readiness)**: LLM produces correct output from chapter? (Needs test)

**CLI**: `ddis validate spec.md` / `ddis validate spec.md --checks 3,6,17 --json` / `ddis validate --modular manifest.yaml`

### 4. Impact Analysis & Change Tracking

**When something changes, show the ripple effects before they become bugs.**

Core operations:
- **Forward impact**: "I'm changing INV-006" → 14 sections reference it → 3 modules interface → 2 gates check it
- **Backward trace**: "Why does §4.2 say X?" → ADR-003 → INV-001 → design goal #2
- **Change set generation**: Targeted edits instead of full rewrites: "In §0.13 line 847, change 'INV-019' to 'INV-020'. In §A.1 add glossary entry..."
- **Regression detection**: "Your proposed change satisfies the new requirement but creates an orphan section (INV-006 violation)"
- **Cascade protocol**: When constitutional content changes, identify all modules needing revalidation

Why this matters: The #1 source of regressions in the RALPH loop is unintended side effects. v17 scored 84→82 partly because of a stale namespace note. Automated impact analysis would have flagged "you added INV-020 but didn't update the namespace note."

**CLI**: `ddis impact "add INV-021" --spec spec.md` / `ddis cascade INV-006 --manifest manifest.yaml` / `ddis diff v0.md v1.md --trace`

### 5. Conversational Spec Authoring — Interactive Iteration Support

**Tools that transform the user↔LLM spec development workflow.**

Current workflow: "Add X" → rewrite entire spec → review 3,000 lines to find what changed.

Desired workflow: "Add X" → query relevant fragment → propose targeted edits with impact → review focused diff → apply approved changes → validate automatically.

Specific tools:
- **Skeleton generator**: `ddis skeleton --domain "database-migration"` → DDIS-conforming template with all required sections, placeholder invariants, 16-step authoring TODO
- **Coverage dashboard**: `ddis coverage spec.md` → "8/10 invariants, 2/6 verification prompts, 0 negative specs in Ch.3, 3 undefined glossary terms"
- **Checkpoint runner**: `ddis checkpoint spec.md --gate 1,5` → run mechanically-checkable gates at any authoring stage
- **Proposal mode**: Generate change sets with impact analysis, not full rewrites
- **Session state**: `ddis state spec.md` → persistent record of decisions made, sections completed, open items

**CLI**: `ddis skeleton`, `ddis coverage`, `ddis checkpoint`, `ddis state`

### 6. Spec-to-Implementation Bridge

**Tools for going from "understanding the spec" to "writing code."**

- **Implementation checklist**: Parse verification prompts → hierarchical checklist of everything that must be true
- **Test scaffold generator**: From worked examples + invariants → test stubs with spec-derived assertions
- **Implementation ordering**: Flatten INV-019 dependency DAG → recommended build sequence
- **Progress tracker**: "Chapters 2-3 done. Chapter 4 next. Prerequisites satisfied. 3 applicable invariants."
- **Spec-code traceability**: Map code locations → spec sections. "`validate_event()` at line 142 → §4.2 (Event Validation), satisfies INV-004"

**CLI**: `ddis impl-order spec.md` / `ddis checklist spec.md` / `ddis progress spec.md --done ch2,ch3`

---

## Architecture

### The `ddis` CLI

A single unified command with subcommands, replacing the existing bash scripts:

```
ddis
├── parse <spec.md>                          → build spec index (JSON)
├── query <target> [--resolve-refs] [...]    → surgical fragment retrieval
├── validate <spec.md> [--checks N] [--json] → mechanical validation
├── impact <change> [--spec spec.md]         → forward/backward impact analysis
├── coverage <spec.md>                       → completeness dashboard
├── checkpoint <spec.md> --gate N            → run specific quality gate
├── skeleton [--domain X]                    → generate conforming template
├── cascade <inv-id> [--manifest ...]        → module cascade analysis
├── assemble [module] [--manifest ...]       → bundle assembly (replaces ddis_assemble.sh)
├── diff <v1.md> <v2.md> [--trace]           → structural diff with traceability
├── impl-order <spec.md>                     → implementation sequence from DAG
├── checklist <spec.md>                      → verification prompt → checklist
├── progress <spec.md> --done ch1,ch2        → what's next, what's ready
├── state <spec.md>                          → session state management
└── bundle <domain> [--manifest ...]         → three-tier domain bundle assembly
```

### Composition Through Three-Tier Modularization

The key insight from our discussion: "cross-spec composition" is really "cross-domain composition within one system." Three-tier modularization already provides the model:

```
System Constitution (shared truth)
├── Domain: Backend
│   ├── Domain Constitution (backend-specific invariants, ADRs)
│   ├── Module: API Gateway
│   ├── Module: Auth Service
│   └── Module: Data Layer
├── Domain: Frontend
│   ├── Domain Constitution (frontend-specific invariants)
│   ├── Module: Component Library
│   └── Module: State Management
└── Domain: Infrastructure
    ├── Domain Constitution (infra invariants)
    ├── Module: Deployment
    └── Module: Monitoring
```

What the tooling needs to support:
- **Cross-domain queries**: "Show me all invariants that the Frontend domain interfaces with from Backend"
- **Domain-scoped bundles**: "Assemble everything needed to work on the Auth Service" → system constitution + backend domain constitution + auth module + deep context from adjacent domains
- **Cross-domain cascade**: "INV-003 changed in Backend → which Frontend modules interface with it?"
- **Domain-level coverage**: "Backend domain: 8/12 modules have verification prompts. Frontend: 3/5."

The manifest.yaml already supports this structure (the `domain` field on modules, the three-tier `constitution.domains` field). The gap is purely in tooling — no existing tool performs cross-domain operations.

### Implementation Language

**Python.** Reasons:
- YAML/JSON parsing native
- Markdown parsing libraries mature (mistune, markdown-it-py)
- Graph algorithms (networkx) for dependency/impact analysis
- MCP Python SDK for future server layer
- Matches the embedded Python already in ddis_assemble.sh and ddis_validate.sh
- Can absorb and replace the existing bash+Python scripts cleanly

### The Spec Index (Core Data Structure)

The parsed index is the heart of the system:

```yaml
spec:
  path: "ddis_final.md"
  version: "3.0"
  total_lines: 3101

sections:
  - id: "§0.2.2"
    title: "LLM Consumption Model"
    level: 3
    lines: [142, 198]
    parent: "§0.2"
    children: []
    refs_out: ["INV-017", "INV-018", "§3.8", "§5.6"]
    refs_in: ["§0.1", "§5.7", "ADR-008"]
    glossary_terms: ["hallucination", "context window"]
    negative_spec_count: 0
    has_verification_prompt: false

invariants:
  INV-006:
    title: "Cross-Reference Density"
    definition: {section: "§0.5", lines: [287, 302]}
    components: [statement, semi_formal, violation_scenario, validation_method]
    referenced_by: ["§2.1", "§3.2", "§4.1", "Gate-5"]
    demonstrated_at: ["§0.2.2", "§1.4"]

# ... ADRs, glossary, cross_references similarly structured
```

---

## How Each Workflow Transforms

### A: Writing a New DDIS-Conforming Spec

**Before**: Read 3,000-line standard, hold template in memory, write from scratch, miss elements.
**After**:
1. `ddis skeleton --domain "database-migration"` → conforming template
2. Fill sections following 16-step authoring sequence
3. `ddis checkpoint spec.md --gate 1` after each section → immediate feedback
4. `ddis coverage spec.md` → "6/10 invariants, 0 negative specs in Ch.3"
5. `ddis validate spec.md` → catch all mechanical violations
6. Context consumed: ~400 lines (skeleton + fragments) instead of 3,000

### B: Implementing Code From a DDIS Spec

**Before**: Load entire spec, lose track of applicable invariants, hallucinate.
**After**:
1. `ddis impl-order spec.md` → "Auth (no deps) → Session (needs Auth) → API (needs both)"
2. `ddis query "Chapter 3: Auth" --resolve-refs` → 300 lines, not 2,000
3. Implement, then run verification checklist from fragment
4. `ddis progress spec.md --done ch3` → "Ch.4 unblocked. 2 invariants carry forward."

### C: Iterating on a Spec With User in Conversation

**Before**: "Add invariant" → rewrite 2,000 lines. Repeat 5x = 10,000 lines reviewed.
**After**:
1. `ddis query "invariants"` → load just invariant section
2. Propose INV-011 with full definition
3. `ddis impact "add INV-011"` → "Affects §4.2, §5.1. Needs: violation scenario, glossary entry. Namespace update required."
4. Generate ~50 lines of targeted edits, not 2,000-line rewrite
5. `ddis validate spec.md` → confirm no regressions

### D: RALPH Loop Integration

**Before**: Each iteration reads + rewrites entire spec. Massive token burn.
**After**:
1. `ddis validate spec.md --json` → mechanical violations as priority list (no LLM needed)
2. Improve: "Fix these 5 violations" with surgical fragments, not "rewrite everything"
3. Judge: `ddis diff v0.md v1.md --trace` → structural diff. Judge evaluates *substance*, not typos.
4. `ddis coverage v1.md` → progress dashboard
5. Modularize: `ddis parse spec.md` → dependency graph informs decomposition

### E: Three-Tier Domain Composition

**Before**: Manual decomposition, no cross-domain visibility.
**After**:
1. `ddis bundle backend --manifest manifest.yaml` → full backend working context
2. `ddis cascade INV-003 --manifest manifest.yaml` → "Frontend modules X, Y interface with this"
3. `ddis coverage --domain backend` → per-domain completeness
4. `ddis validate --modular manifest.yaml` → all 9 checks + cross-domain consistency

---

## Build Prioritization

### Phase 1: Foundation (enables everything else)
1. **`ddis parse`** — spec parser + index builder. THE prerequisite.
2. **`ddis query`** — surgical fragment retrieval. Single biggest impact.
3. **`ddis validate`** — mechanical invariant checks. Subsumes ddis_validate.sh.

### Phase 2: Authoring Support
4. **`ddis coverage`** — completeness dashboard
5. **`ddis skeleton`** — template generator for new specs
6. **`ddis assemble`** — bundle assembly. Subsumes ddis_assemble.sh.

### Phase 3: Change Intelligence
7. **`ddis impact`** — forward/backward ripple analysis
8. **`ddis diff`** — structural diff with traceability
9. **`ddis cascade`** — module cascade for modular specs

### Phase 4: Workflow Integration
10. **`ddis checkpoint`** — gate runner for authoring sessions
11. **`ddis impl-order`** — implementation DAG flattening
12. **`ddis bundle`** — three-tier domain bundle assembly

### Phase 5: Advanced
13. **`ddis checklist`** — verification prompt extraction
14. **`ddis progress`** — implementation tracker
15. **`ddis state`** — session state persistence
16. **MCP server layer** — expose all tools as MCP tools/resources

---

## Open Design Questions

1. **Parser granularity**: Should the parser understand DDIS-specific markdown conventions (invariant blocks, ADR blocks, verification prompts) or just generic markdown? DDIS-specific is more powerful but less reusable. *Recommendation: DDIS-specific. This is a DDIS tool, not a generic markdown tool.*

2. **Index freshness**: Rebuild on every query (accurate, slower) or cache with manual refresh (faster, staleness risk)? *Recommendation: Cache + auto-invalidate on file mtime change.*

3. **Fragment depth**: When resolving "§3.2 + dependencies," how deep? One level? Transitive? *Recommendation: Configurable with sensible default (1 level for refs, transitive for invariant chains). Budget-aware: stop when fragment exceeds N lines.*

4. **Change set format**: What format for targeted edits? Unified diff? DDIS-specific? *Recommendation: Standard unified diff for human review, JSON patch for programmatic application.*

5. **Absorb vs wrap existing scripts**: Should `ddis validate` rewrite ddis_validate.sh in Python or wrap it? *Recommendation: Rewrite. The existing scripts already have embedded Python. Unified Python codebase is cleaner.*

6. **Spec self-description**: Should the `ddis` tool itself be specified as a DDIS-conforming spec? *The ultimate self-bootstrapping flex: use DDIS to spec the tool that validates DDIS specs.*
