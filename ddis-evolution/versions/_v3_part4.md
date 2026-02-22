
---

## Chapter 6: PART IV Elements

### 6.1 Operational Playbook

**What it is**: Prevents infinite refinement without shipping.

#### 6.1.1 Phase -1: Decision Spikes

Before building anything, run experiments that de-risk unknowns. Each spike produces an ADR (with Confidence field — spikes often produce Provisional ADRs). Required per spike: question it answers, time budget (1–3 days), exit criterion (one ADR).

#### 6.1.2 Exit Criteria per Phase

Every phase has a specific, testable exit criterion.

**Anti-pattern**: "Phase 2: Implement the scheduler. Exit: Scheduler works."
**Good example**: "Phase 2 exit: Property test demonstrates fair scheduling across 50 agents with no starvation for > 1000 ticks. Benchmark shows dispatch < 1ms at design point."

#### 6.1.3 Merge Discipline

What every PR touching invariants or critical paths must include: tests, a note on which invariants it preserves (with substance restated per [[INV-018|substance restated at point of use]]), benchmark comparison if touching a hot path.

#### 6.1.4 Minimal Deliverables Order

The order to build subsystems, chosen to maximize the "working subset" at each stage. Must include dependency rationale.

#### 6.1.5 Immediate Next Steps (First PRs)

The literal first 5–6 things to implement, in dependency order. Tactical, not strategic.

### 6.2 Testing Strategy

| Test Type | What It Validates | Example |
|---|---|---|
| Unit | Individual function correctness | Conflict detection returns correct overlaps |
| Property | Invariant preservation under random inputs | ∀ events: replay(snapshot, events) = direct_state |
| Integration | Subsystem composition | Completed task triggers scheduling cascade |
| Stress | Behavior at design point limits | 300 agents, 10K tasks, sustained 60s |
| Replay | Determinism | Process N events, snapshot, replay, byte-compare |
| Adversarial | Robustness against malformed input | Agent sends event with forged task_id |
| LLM Output | Implementation matches spec | Verification prompt (§5.6) run against generated code |
| Spec Structural | Spec itself is well-formed | Automated cross-ref validation (§12.3, Gate 8) |

### 6.3 Error Taxonomy

**What it is**: Classification of errors with handling strategy per class.

**Required**: Each error class has severity (fatal/degraded/recoverable/ignorable), handling strategy (crash/retry/degrade/log), and cross-references to threatened invariants.

See also Appendix D for the specification authoring error taxonomy (meta-level).

---

## Chapter 7: Appendix Elements

### 7.1 Glossary

**What it is**: Every domain-specific term, defined in 1–3 sentences with a cross-reference.

**Required**: Alphabetized. Each entry includes `(see §X.Y)`. Terms with both common and domain-specific meanings clearly distinguish the two.

**LLM note**: The glossary is the primary disambiguation mechanism ([[INV-009|glossary coverage]]). Domain terms with common-English meanings are the #1 source of LLM misinterpretation.

### 7.2 Risk Register

**What it is**: Top 5–10 risks, each with description, impact, mitigation, and detection.

### 7.3 Master TODO Inventory

**What it is**: Comprehensive, checkboxable task list organized by subsystem.

**Required**: Organized by subsystem. Each item is PR-sized. Cross-references to ADRs/invariants. Includes tasks for writing negative specifications and verification prompts per subsystem. Includes a "Spec Testing" section for setting up automated validation.

---

# PART III: GUIDANCE (RECOMMENDED)

## Chapter 8: Voice and Style

### 8.1 The DDIS Voice

**Technically precise but human.** The voice of a senior engineer explaining their system to a peer they respect.

**Properties**:
- Uses concrete examples, not abstract descriptions
- Admits uncertainty where it exists (Provisional ADRs make this explicit)
- Is direct about tradeoffs
- Does not hedge every statement
- Never uses marketing language ("enterprise-grade," "cutting-edge")
- Never uses bureaucratic language ("it is recommended that," "the system shall")

**LLM-specific voice guidance**: LLMs trained on corporate documentation tend to produce hedging, passive voice, and vague claims. The DDIS voice actively counteracts this. When reviewing LLM-generated spec sections, check for: passive voice ("it was decided" → "we chose"), hedge words ("arguably" → delete), abstract claims ("provides robust handling" → "retries 3 times with exponential backoff, then returns error E-004").

**Calibration examples**:
```
✅ GOOD: "The kernel loop is single-threaded by design — not because concurrency
is hard, but because serialization through the event log is the mechanism that
gives us deterministic replay for free."

❌ BAD (academic): "The kernel loop utilizes a single-threaded architecture paradigm
to facilitate deterministic replay capabilities."

❌ BAD (casual): "We made the kernel single-threaded and it's awesome!"

❌ BAD (bureaucratic): "It is recommended that the kernel loop shall be implemented
in a single-threaded manner to support the deterministic replay requirement."
```

### 8.2 Formatting Conventions

- **Bold** for terms being defined, non-negotiable properties, critical warnings
- `Code` for types, function names, file names, anything in source code
- `// Comments` for inline justifications and WHY NOT annotations
- `[[ID|substance]]` for machine-readable cross-references ([[INV-022|parseable cross-refs]])
- Tables for structured data (prefer tables over equivalent prose for LLM consumption)
- ASCII diagrams preferred over external images
- `Must NOT` in negative specifications always bold and capitalized

### 8.3 Anti-Pattern Catalog

**The Hedge Cascade**:
```
❌ "It might be worth considering the possibility of potentially using..."
✅ "The kernel loop is single-threaded. This gives us deterministic replay.
See [[ADR-003|single-threaded for deterministic replay]]."
```

**The Orphan Section**: A section that references nothing and is referenced by nothing. Either connect it or remove it. (Violates [[INV-006|cross-reference density]].)

**The Trivial Invariant**: "INV-042: The system uses UTF-8 encoding." Either enforced by the platform or belongs in Non-Negotiables.

**The Strawman ADR**: Options where only one is viable. Every option must have a genuine advocate.

**The Percentage-Free Performance Budget**: "The system should respond quickly." Without a number, a design point, and a measurement method, this is a wish.

**The Spec That Requires Oral Tradition**: If an implementer must ask a question the spec should have answered, the spec has a gap. (Violates [[INV-008|self-containment]].)

**The Implicit Context Reference**: "As discussed above, we use event sourcing." An LLM may not have "above" in context. Cite explicitly: "Per [[ADR-003|event sourcing chosen for audit trail]]." (Violates [[INV-018|substance restated]].)

**The Positive-Only Specification**: A chapter that says what to build but never says what NOT to build. LLMs will fill the gap with plausible but incorrect behavior. (Violates [[INV-017|negative spec per chapter]].)

**The Provisional-Forever ADR**: An ADR marked Provisional with no review trigger, or with a trigger that can never be evaluated. Every Provisional ADR must have a concrete, observable trigger. (Violates [[ADR-012|confidence levels]].)

**The Unparseable Reference Web**: Cross-references using inconsistent formats ("see INV-003," "per the determinism invariant," "(INV-003)") that prevent automated validation. Use `[[ID|substance]]` consistently. (Violates [[INV-022|parseable cross-refs]].)

---

## Chapter 9: Proportional Weight Deep Dive

### 9.1 Identifying the Heart

Every system has a "heart" — the 2–3 subsystems where most complexity and bugs live. These receive 40–50% of PART II's line budget.

**How to identify**: Which subsystems have the most invariants? The most ADRs? The most cross-references? If you cut the spec in half, which would you keep?

### 9.2 Signals of Imbalanced Weight

- A subsystem with 5 invariants and 50 lines of spec is **starved**
- A subsystem with 1 invariant and 500 lines of spec is **bloated**
- PART 0 longer than PART II means the spec is top-heavy
- Appendices longer than PART II means reference material displaces implementation
- A chapter exceeding 500 lines should be split for LLM context management
- (Validated by [[INV-021|proportional weight compliance]]; automated by Gate 8)

---

## Chapter 10: Cross-Reference Patterns

### 10.1 Reference Syntax

DDIS prescribes consistent conventions with restated substance (per [[INV-018|substance restated at point of use]]) using machine-readable syntax (per [[INV-022|parseable cross-refs]]):

```
[[§3.2|non-goals]]                                          — section reference
[[INV-004|every algorithm has pseudocode + examples]]        — invariant with substance
[[ADR-003|single-threaded for deterministic replay]]         — ADR with substance
(measured by Benchmark B-001)                                — performance reference
(defined in Glossary: "task")                                — glossary reference
```

### 10.2 Reference Density Targets

| Section Type | Minimum Outbound References |
|---|---|
| Implementation chapter | 3 (at least: one ADR, one invariant, one other chapter) |
| ADR | 2 (at least: one invariant, one implementation chapter) |
| Invariant | 1 (at least: one test or validation method) |
| Performance budget | 2 (at least: one benchmark, one design point) |
| Test strategy | 2 (at least: one invariant, one implementation chapter) |
| Negative specification | 1 (at least: one invariant or misinterpretation source) |

### 10.3 Machine-Readable Cross-Reference Syntax

**Format**: `[[TARGET-ID|substance summary]]`

**TARGET-ID patterns**:
- Invariants: `INV-NNN` or `APP-INV-NNN` (domain specs)
- ADRs: `ADR-NNN` or `APP-ADR-NNN` (domain specs)
- Sections: `§N.N` or `§N.N.N`
- External specs: `EXT:SpecName:vN.N:INV-NNN` (per composability protocol §0.2.5)

**Parsing**: The regex `\[\[([^\]|]+)\|([^\]]+)\]\]` extracts `(target_id, substance)` pairs. Tools can then:
1. Build a directed graph: source_section → target_id
2. Verify all target_ids resolve to existing elements
3. Compare substance text to source definitions for staleness
4. Compute density per section (INV-006 validation)
5. Flag proportional weight deviations ([[INV-021|weight compliance]])

**Backward compatibility**: Specs authored before 3.0 may use freeform references. A one-time migration to `[[ID|substance]]` syntax is recommended when adopting automated testing (§12.3). The migration is mechanical: search for patterns like "INV-NNN", "ADR-NNN", "(see §N.N)" and convert to the machine-readable form.

---

# PART IV: OPERATIONS

## Chapter 11: Applying DDIS to a New Project

### 11.1 The Authoring Sequence

Write sections in this order (not document order) to minimize rework:

1. **Design goal + Core promise** (articulate the value)
2. **First-principles formal model** (understand the domain)
3. **Non-negotiables** (commit to what matters)
4. **Invariants** (formalize the commitments)
5. **ADRs** (lock controversial decisions — mark uncertain ones as Provisional with review triggers)
6. **Implementation chapters** — heaviest subsystems first (the "heart")
7. **Negative specifications** per chapter (think adversarially: what would an LLM get wrong?)
8. **End-to-end trace** (reveals gaps in subsystem interfaces)
9. **Performance budgets** (anchor to measurable targets)
10. **Test strategies** (turn invariants into verification)
11. **Verification prompts** per chapter (convert spec into self-checks)
12. **Meta-instructions** per chapter (make implementation ordering explicit)
13. **Cross-references with substance** (weave the web using `[[ID|substance]]` syntax)
14. **Glossary** (extract terms from the complete spec)
15. **Master TODO** (convert spec into execution plan)
16. **Operational playbook** (how to start building)
17. **Automated spec tests** (run Gate 8 checks — see §12.3)

### 11.2 Common Mistakes in First DDIS Specs

1. **Writing implementation chapters before ADRs.** You'll rewrite them when ADRs imply different choices.
2. **Writing the glossary first.** You don't know your terminology yet.
3. **Treating the end-to-end trace as optional.** It's the single most effective quality check.
4. **Under-investing in WHY NOT annotations.** Every non-obvious choice needs one.
5. **Skipping negative specifications.** "The LLM will figure it out" is exactly the failure mode negative specs prevent.
6. **Writing ID-only cross-references.** "See INV-003" is useless without context. Always use `[[INV-003|substance]]`.
7. **Generic verification prompts.** "Check your work" is not a verification prompt. Reference specific invariants and negative specs.
8. **Skipping the anti-patterns.** Show what bad output looks like. LLMs benefit significantly from negative examples.
9. **Omitting confidence levels on uncertain ADRs.** Marking spike-derived decisions as Decided creates false certainty. Use Provisional with a concrete review trigger.
10. **Deferring automated testing to "later."** Set up `[[ID|substance]]` syntax from the start. Retrofitting is harder than starting right.

### 11.3 Incremental Authoring Workflow

The authoring sequence (§11.1) is linear — write everything, then validate. Real spec development is iterative. This section provides checkpoints for incremental authoring.

**Phase A: Skeleton (steps 1–3)**
Write the design goal, formal model, and non-negotiables. At this point, you can validate:
- Is the formal model precise enough to derive invariants? (If not, iterate on the model.)
- Do the non-negotiables feel load-bearing? (If they're platitudes, dig deeper.)

**Phase B: Contracts (steps 4–5)**
Write invariants and ADRs. Validate:
- Can you construct a violation scenario for every invariant? ([[INV-003|falsifiability]])
- Does every ADR have a genuine alternative? (Gate 3)
- Are uncertain decisions marked Provisional? ([[ADR-012|confidence levels]])

**Phase C: Heart (steps 6–7)**
Write the 2-3 heaviest implementation chapters with negative specs. Validate:
- Does the end-to-end trace (step 8) work with the chapters drafted so far?
- Are negative specs plausible, not trivial? ([[INV-017|negative spec coverage]])
- Run an LLM smoke test: give one chapter to an LLM. Does it hallucinate? (Informal Gate 7.)

**Phase D: Completion (steps 8–17)**
Fill remaining chapters, weave cross-references, add verification prompts and meta-instructions, build the glossary, and run automated tests.

**Key principle**: Each phase produces a spec that is incomplete but internally consistent for the parts that exist. Never proceed to Phase C with Phase B violations — contracts must hold before building on them.

---

## Chapter 12: Validating a DDIS Specification

### 12.1 Self-Validation Checklist

1. Pick 5 random implementation sections. Trace each backward to the formal model. Any broken chains? (Gate 2)
2. Read each ADR's alternatives. Would a competent engineer genuinely choose any rejected option? (Gate 3)
3. For each invariant, spend 60 seconds constructing a violation scenario. Can't? Too vague. (Gate 4)
4. Build the cross-reference graph. Any orphans? Do references include substance? (Gate 5)
5. Read the spec as a first-time implementer. Where did you guess? (Gate 6)
6. Pick one implementation chapter. Give it (with glossary and restated invariants) to an LLM. Does the LLM correctly identify constraints and produce a valid skeleton? (Gate 7)
7. Check each negative specification: is the prohibited behavior plausible? ([[INV-017|negative spec coverage]])
8. Check each verification prompt: does it reference specific invariants and negative specs? ([[INV-019|verification prompt coverage]])
9. Run automated cross-reference validation. Do all `[[ID|substance]]` refs resolve? (Gate 8)
10. Check proportional weight. Any section starved or bloated? ([[INV-021|weight compliance]])

### 12.2 External Validation

Give the spec to an implementer (or LLM) and track:
- Questions they ask that the spec should have answered (→ gaps, [[INV-008|self-containment]])
- Incorrect implementations the spec didn't prevent (→ missing negative specs, [[INV-017|negative spec coverage]])
- Hallucinated features not in the spec (→ missing negative specs or non-goals)
- Sections skipped because unreadable (→ voice/clarity issues)

### 12.3 Automated Specification Testing

Automated tests validate structural properties that manual review misses or tires of checking. These tests correspond to Gate 8 (Specification Testability).

**Test 1: Cross-Reference Resolution**
Parse all `[[TARGET-ID|substance]]` references. Verify every TARGET-ID maps to an existing invariant, ADR, or section heading.
```
Input: spec text
Output: list of unresolved references (should be empty)
Complexity: O(references × elements), typically < 1 second
```

**Test 2: Reference Graph Density**
Build a directed graph from cross-references. For each non-trivial section, verify at least one inbound and one outbound edge. (Validates [[INV-006|cross-reference density]].)
```
Output: list of orphan sections (should be empty)
```

**Test 3: Substance Staleness Detection**
For each `[[INV-NNN|substance]]` reference, extract the substance text. Compare against the source definition of INV-NNN. Flag references where the substance has diverged.
```
Output: list of potentially stale restatements with diff
Note: requires fuzzy matching (substance is a summary, not a copy)
```

**Test 4: Proportional Weight**
Count lines per PART. Compare against §0.8.2 targets. Flag deviations exceeding the tolerance band that lack WHY NOT annotations. (Validates [[INV-021|proportional weight]].)

**Test 5: Invariant Completeness**
For each invariant, check for the required components: plain-language statement, semi-formal expression, violation scenario, validation method. (Validates [[INV-003|falsifiability]] structurally, not semantically.)

**Test 6: ADR Completeness**
For each ADR, check for: Problem, Options (≥ 2), Decision, Consequences, Tests, Confidence field. Flag Provisional ADRs without review triggers.

**Test 7: Negative Spec and Verification Prompt Coverage**
For each implementation chapter, verify at least one negative specification and one verification prompt exist. (Validates [[INV-017|negative spec coverage]], [[INV-019|verification prompt coverage]].)

**Implementation note**: These tests can be implemented as a ~200-line script in any language. The machine-readable cross-reference syntax ([[INV-022|parseable refs]]) makes parsing trivial. A reference implementation is not part of the DDIS standard, but the test specifications above are precise enough to implement from.

---

## Chapter 13: Evolving a DDIS Specification

### 13.1 The Living Spec

Once implementation begins, the spec enters the Living state (§1.1). In this state:

- **Gaps** are patched back into the spec. Track each gap's category using Appendix D.
- **ADRs may be superseded.** Mark old ADRs as "Superseded by ADR-NNN" and update all cross-references and substance restatements.
- **Provisional ADRs are reviewed** when their triggers fire. Either promote to Decided or supersede with a new ADR.
- **New invariants may be added** with full format.
- **Negative specifications grow.** Implementation reveals plausible misinterpretations the author didn't anticipate.
- **Performance budgets may be revised** with documented rationale.
- **Automated spec tests** (§12.3) should run after every spec edit to catch structural regressions.

### 13.2 Spec Versioning

`Major.Minor` where:
- **Major** increments when the formal model or a non-negotiable changes
- **Minor** increments when ADRs, invariants, negative specs, or implementation chapters are added or revised

When external specs (§0.2.5) pin to a version, major increments require downstream specs to re-evaluate their external dependency declarations.

---

# APPENDICES

## Appendix A: Glossary

| Term | Definition |
|---|---|
| **ADR** | Architecture Decision Record. A structured record of a design choice with alternatives and rationale. (See §3.5) |
| **Bundle** | The assembled document for LLM consumption: constitution + module. (See §0.13.2) |
| **Cascade protocol** | Procedure for re-validating modules after constitutional changes. (See §0.13.12) |
| **Causal chain** | Traceable path from first principle through invariant/ADR to implementation detail. (See §0.2.2, [[INV-001|causal traceability]]) |
| **Churn-magnet** | A decision causing the most downstream rework if left open. (See §3.5) |
| **Comparison block** | Side-by-side ❌/✅ comparison with quantified reasoning. (See §5.5) |
| **Composability protocol** | Rules for how DDIS specs reference each other across system boundaries. (See §0.2.5) |
| **Confidence level** | ADR annotation: Decided (load-bearing) or Provisional (review trigger required). (See [[ADR-012|confidence levels on ADRs]]) |
| **Constitution** | Cross-cutting material constraining all modules. Organized in tiers. (See §0.13.3) |
| **Cross-reference** | Explicit link between sections forming the reference web. Uses `[[ID\|substance]]` syntax. (See Chapter 10, [[INV-006|density]], [[INV-018|substance restated]], [[INV-022|parseable refs]]) |
| **DDIS** | Decision-Driven Implementation Specification. This standard. |
| **Decision spike** | A time-boxed experiment producing an ADR. (See §6.1.1) |
| **Declaration** | Compact 1-line summary of an invariant/ADR in the system constitution. (See §0.13.4) |
| **Deep context** | Tier 3: cross-domain invariant definitions and interface contracts. (See §0.13.3) |
| **Definition** | Full specification of an invariant/ADR in the domain constitution. (See §0.13.4) |
| **Design point** | Specific hardware/workload/scale scenario for performance validation. (See §3.7) |
| **Domain** | Architectural grouping of related modules. (See §0.13.2) |
| **Element composition trace** | Worked example showing all DDIS element types composing within one chapter. (See §5.8) |
| **End-to-end trace** | Worked scenario traversing all subsystems. (See §5.3, §1.4) |
| **Exit criterion** | Testable condition for phase completion. (See §6.1.2) |
| **External dependency** | Cross-spec reference pinned to a version. (See §0.2.5) |
| **Falsifiable** | Can be violated by concrete scenario and detected by test. (See [[INV-003|invariant falsifiability]], ADR-002) |
| **First principles** | The formal model from which architecture derives. (See §3.3) |
| **Formal model** | Mathematical definition of the system as a state machine or function. (See §0.2.1) |
| **Gate** | A quality gate: stop-ship predicate. (See §3.6) |
| **Hallucination gap** | The space between what a spec specifies and what an LLM might implement. Closed by negative specifications. (See §0.2.3) |
| **Incremental authoring** | Phased spec development with validation checkpoints per phase. (See §11.3) |
| **Invariant** | A numbered, falsifiable property that must always hold. (See §3.4) |
| **Living spec** | Specification in active use, updated as implementation reveals gaps. (See §13.1) |
| **LLM Consumption Model** | Formal model of how LLMs process specifications, justifying structural provisions. (See §0.2.3) |
| **Machine-readable cross-reference** | `[[ID\|substance]]` syntax enabling automated validation. (See §10.3, [[INV-022|parseable refs]]) |
| **Manifest** | YAML file declaring modules, ownership, interfaces, assembly rules. (See §0.13.9) |
| **Master TODO** | Checkboxable task inventory cross-referenced to subsystems and ADRs. (See §7.3) |
| **Meta-instruction** | Explicit directive about implementation strategy and ordering. (See §5.7, [[INV-020|meta-instruction explicitness]]) |
| **Module** | Self-contained spec unit covering one subsystem. (See §0.13.2) |
| **Module header** | YAML block declaring domain, invariants, interfaces, negative specs. (See §0.13.5) |
| **Monolith** | A DDIS spec as a single document. (See §0.13.2) |
| **Negative specification** | Explicit "must NOT" constraint preventing plausible misinterpretation. (See §3.8, [[INV-017|negative spec coverage]]) |
| **Non-goal** | Something the system explicitly does not attempt. (See §3.2) |
| **Non-negotiable** | Philosophical commitment defining what the system IS. (See §3.1) |
| **Operational playbook** | How the spec gets converted into shipped software. (See §6.1) |
| **Proportional weight** | Line budget guidance preventing bloat and starvation. (See §0.8.2, [[INV-021|weight compliance]]) |
| **Provisional ADR** | An ADR with confidence level Provisional, requiring re-evaluation at a defined trigger. (See [[ADR-012|confidence levels]]) |
| **Self-bootstrapping** | Property of this standard: written in the format it defines. (See ADR-004) |
| **Spec testing** | Automated validation of structural spec properties (cross-refs, density, weight, completeness). (See §12.3, Gate 8) |
| **Structural redundancy** | Restating constraint substance at point of use. Trades DRY for LLM self-sufficiency. (See [[INV-018|substance restated]], ADR-009) |
| **Verification prompt** | Structured self-check at the end of each implementation chapter. (See §5.6, [[INV-019|verification prompt coverage]]) |
| **Voice** | Writing style: technically precise but human. (See §8.1) |
| **WHY NOT annotation** | Inline comment explaining why a non-obvious alternative was rejected. (See §5.4) |
| **Worked example** | Concrete scenario with specific values showing a subsystem in action. (See §5.2) |

---

## Appendix B: Risk Register

| # | Risk | Impact | Mitigation | Detection |
|---|---|---|---|---|
| 1 | Standard is too prescriptive | Low adoption | Non-goals and [Optional] elements provide flexibility | Author feedback |
| 2 | Specs become shelfware | Implementers don't read | Proportional weight limits bloat; voice guide keeps prose readable | Track questions spec should have answered |
| 3 | Cross-reference + restatement burden | Authors skip or restate incorrectly | Authoring sequence defers to step 13; stale restatements detected by automated testing (§12.3) | Spec test 3 (staleness detection) |
| 4 | Self-bootstrapping creates confusion | Readers can't distinguish meta/object level | Document note and consistent "this standard" vs "a conforming specification" language | Reader feedback |
| 5 | Negative specs become trivial boilerplate | Authors write "must NOT format the hard drive" | Quality criteria require plausible misinterpretations; Gate 7 validates | Review for plausibility |
| 6 | LLM provisions increase length without value | Longer specs, diminishing returns | Each provision prevents a specific failure mode (§0.2.2 table); proportional weight enforced | Measure LLM accuracy with/without provisions |
| 7 | Verification prompts become generic | Self-checks don't catch errors | Quality criteria require specific invariant and negative spec references | Gate 7 tests error detection |
| 8 | Machine-readable syntax adoption friction | Authors reject `[[ID\|substance]]` | Syntax is lightweight and familiar (wiki-links); migration is mechanical | Track adoption rate in conforming specs |
| 9 | Automated tests create false confidence | Pass structural checks but miss semantic gaps | Gate 8 is structural only; Gates 6-7 remain semantic | Compare automated vs manual findings |

---

## Appendix C: Quick-Reference Card

```
PREAMBLE: Design goal → Core promise → Document note → How to use
PART 0:   Summary → First principles + LLM consumption model + Composability →
          Architecture → Layout → Invariants → ADRs (with Confidence) →
          Gates (1-8) → Budgets → API → Non-negotiables → Non-goals
PART I:   Formal model → State machines → Complexity → End-to-end trace
PART II:  [Per subsystem: types → algorithm → state machine → invariants (restated) →
          negative specs → example → WHY NOT → tests → budget →
          cross-refs ([[ID|substance]]) → meta-instructions → verification prompt]
          Element composition trace → End-to-end trace (crosses all subsystems)
PART III: Protocol schemas → Adapters → UI contracts
PART IV:  Test taxonomy → Error taxonomy → Operational playbook
          (spikes → exit criteria → merge discipline → deliverable order → first PRs)
          Incremental authoring (Phases A-D) → Automated spec testing (7 tests)
APPENDICES: Glossary → Risks → Quick-Ref → Error Taxonomy → Formats → Benchmarks
PART X:   Master TODO (checkboxable, by subsystem)

Every invariant: ID + statement + formal + violation + test + WHY THIS MATTERS
Every ADR: Confidence + problem + options (genuine) + decision + WHY NOT +
           consequences + tests
Every algorithm: pseudocode + complexity + example + edge cases
Every chapter: negative specs (≥1) + verification prompt + meta-instructions
Cross-refs: [[ID|substance]] syntax. Web, not list. No orphans.
Voice: senior engineer to respected peer. No hedging. No marketing. No bureaucracy.
LLM: Each chapter self-contained. Negative specs prevent hallucination.
     Verification prompts enable self-check. Meta-instructions order implementation.
Composability: External deps pinned to version. Reference by invariant, not section.
Testing: Gate 8 = automated structural validation. Gates 6-7 = semantic validation.
```

---

## Appendix D: Specification Error Taxonomy

Classification of errors during specification authoring — the meta-level analog of §6.3.

| Error Class | Severity | Symptom | Detection | Resolution |
|---|---|---|---|---|
| **Broken causal chain** | Critical | Section with no path to formal model | Gate 2 audit | Add cross-references or remove unjustified section |
| **Strawman ADR** | Critical | ADR with no genuine alternative | Gate 3 review | Research real alternatives or demote to WHY NOT |
| **Unfalsifiable invariant** | Critical | Invariant with no constructible counterexample | Gate 4 check | Sharpen or remove |
| **Orphan section** | Major | No inbound or outbound references | Gate 5 graph; Spec test 2 | Add references or remove section |
| **Missing negative spec** | Major | Implementation chapter with no "must NOT" | Gate 7 LLM test; Spec test 7 | Add plausible negative specs |
| **Stale restatement** | Major | Restated substance no longer matches source | Spec test 3 (automated) | Update restatement |
| **ID-only reference** | Major | Cross-reference with no substance | [[INV-018|substance restated]] audit | Convert to `[[ID\|substance]]` format |
| **Unparseable reference** | Moderate | Reference not in `[[ID\|substance]]` format | Spec test 1 | Convert to machine-readable syntax |
| **Generic verification prompt** | Moderate | Prompt says "check your work" | [[INV-019|verification prompt]] audit | Reference specific invariants and negative specs |
| **Implicit context dependency** | Moderate | Uses "as discussed above" | LLM isolation test | Replace with `[[ID\|substance]]` reference |
| **Missing meta-instruction** | Minor | Ordering dependencies without guidance | [[INV-020|meta-instruction]] check | Add meta-instructions with rationale |
| **Trivial negative spec** | Minor | "Must NOT format the hard drive" | Plausibility review | Replace with plausible constraint or remove |
| **Provisional-forever ADR** | Minor | Provisional ADR with no review trigger | Spec test 6 (automated) | Add concrete review trigger |
| **Weight imbalance** | Minor | Section exceeds proportional weight tolerance | Spec test 4 (automated) | Rebalance or add WHY NOT annotation |

---

# PART X: MASTER TODO INVENTORY

## A) Meta-Standard Validation
- [x] Self-bootstrapping: this document uses the format it defines
- [x] Preamble elements: design goal, core promise, document note, how to use — updated for 3.0
- [x] Non-negotiables defined (§0.1.2) — includes specification testability non-negotiable (new in 3.0)
- [x] Non-goals defined (§0.1.3) — includes tooling non-prescription (new in 3.0)
- [x] First-principles derivation (§0.2) with LLM Consumption Model (§0.2.3)
- [x] Composability Model (§0.2.5) — new in 3.0, resolves Open Question #1 from 2.0
- [x] LLM Consumption Model justifies INV-017 through INV-020 and ADR-008 through ADR-011
- [x] Document structure prescribed (§0.3) — includes external dependencies section
- [x] Invariants numbered and falsifiable: INV-001–010 (base), INV-011–016 (modularization), INV-017–022 (LLM + testability)
- [x] INV-021 (Proportional Weight Compliance): new in 3.0, full format
- [x] INV-022 (Machine-Readable Cross-References): new in 3.0, full format
- [x] INV-011–016 (modularization): full format with violation scenarios and validation
- [x] ADRs with genuine alternatives: ADR-001–007 (base + modularization), ADR-008–011 (LLM), ADR-012–013 (testability)
- [x] ADR-012 (Confidence Levels): new in 3.0, 3 genuine options
- [x] ADR-013 (Machine-Readable Ref Syntax): new in 3.0, 3 genuine options
- [x] Quality gates defined (§0.7) — Gates 1–8 including Gate 8 (Specification Testability), new in 3.0
- [x] Gate 7 operational with concrete test procedure
- [x] Gate 8 operational with concrete test procedure — new in 3.0
- [x] Performance budgets (§0.8) — includes machine-readable ref conversion time
- [x] Proportional weight guide (§0.8.2) with meta-standard deviation note

## B) Element Specifications
- [x] Preamble elements specified (Chapter 2)
- [x] PART 0 elements specified (Chapter 3) — includes §3.8 Negative Specifications
- [x] §3.5 ADR format updated with Confidence field (new in 3.0)
- [x] Negative Specifications element spec (§3.8) with format, quality criteria, worked example, anti-patterns
- [x] PART I elements specified (Chapter 4)
- [x] PART II elements specified (Chapter 5) — includes §5.6, §5.7, §5.8
- [x] §5.8 Element Composition Trace (new in 3.0) — worked example showing all elements composing
- [x] Verification Prompts element spec (§5.6) with format, quality criteria, worked example
- [x] Meta-Instructions element spec (§5.7) with format, quality criteria, worked example
- [x] PART IV elements specified (Chapter 6) — includes spec structural testing in test taxonomy
- [x] Appendix elements specified (Chapter 7)
- [x] Anti-pattern catalog (§8.3) — includes Provisional-Forever and Unparseable Reference anti-patterns (new in 3.0)
- [x] Cross-reference patterns (Chapter 10) — includes §10.3 machine-readable syntax (new in 3.0)

## C) Guidance
- [x] Voice and style guide (Chapter 8) — includes `[[ID|substance]]` in formatting conventions
- [x] Proportional weight deep dive (Chapter 9) — references [[INV-021|weight compliance]]
- [x] Authoring sequence (§11.1) — 17 steps including automated spec tests
- [x] Common mistakes (§11.2) — 10 items including confidence levels and machine-readable refs
- [x] Incremental authoring workflow (§11.3) — new in 3.0, Phases A-D
- [x] Validation procedure (Chapter 12) — includes Gate 8 and automated testing
- [x] Automated specification testing (§12.3) — new in 3.0, 7 concrete tests
- [x] Evolution guidance (Chapter 13) — includes Provisional ADR review and automated regression testing

## D) Reference Material
- [x] Glossary (Appendix A) — expanded with new terms (composability, confidence level, spec testing, etc.)
- [x] Risk register (Appendix B) — expanded with machine-readable syntax and false confidence risks
- [x] Quick-reference card (Appendix C) — updated for DDIS 3.0
- [x] Specification Error Taxonomy (Appendix D) — expanded with unparseable reference, provisional-forever, weight imbalance

## E) Self-Conformance Fixes
- [x] §0.13.8–0.13.14 restored with full content (was elided in 2.0, violating [[INV-008|self-containment]])
- [x] State machine (§1.1) expanded with Tested state (new in 3.0, supports Gate 8 workflow)
- [x] End-to-end trace (§1.4) updated with automated testing step
- [x] Failure mode table (§0.2.2) includes spec testability and composability failure modes
- [x] All new invariants have complete format: statement, formal expression, violation scenario, validation, WHY THIS MATTERS
- [x] All new ADRs have genuine alternatives with concrete tradeoffs and Confidence field
- [x] Machine-readable cross-reference syntax used throughout (validating [[INV-022|parseable refs]] by self-demonstration)
- [x] Modularization protocol complete (INV-011–016, ADR-006–007, Gates M-1–M-5, §0.13.8–0.13.13)

## F) Validation
- [x] INV-001 (Causal Traceability): Every element spec traces to the formal model via failure mode table (§0.2.2)
- [x] INV-003 (Falsifiability): Each invariant has violation scenario and validation method
- [x] INV-006 (Cross-Reference Density): Sections reference each other throughout using `[[ID|substance]]`
- [x] INV-007 (Signal-to-Noise): Each section serves a named purpose in the failure mode table
- [x] INV-017 (Negative Spec Coverage): Demonstrated in §3.8 worked example and throughout element specs
- [x] INV-018 (Structural Redundancy): Demonstrated in §1.4 trace and reference syntax (§10.1)
- [x] INV-019 (Verification Prompt Coverage): Demonstrated in §5.6 worked example
- [x] INV-021 (Proportional Weight): Deviation annotated in §0.8.2; tolerance mechanism in INV-021
- [x] INV-022 (Machine-Readable Refs): `[[ID|substance]]` syntax used throughout this document
- [ ] INV-008 (Self-Containment): Requires external validation — give this standard to a first-time author
- [ ] Gate 6 (Implementation Readiness): Requires a non-trivial spec written conforming to DDIS 3.0
- [ ] Gate 7 (LLM Implementation Readiness): Requires an LLM to attempt implementing from a conforming spec chapter
- [ ] Gate 8 (Specification Testability): Requires running automated tests against a conforming spec

---

## Conclusion

DDIS 3.0 extends the standard along two new axes: **specification testability** and **composability**, while preserving all capabilities from previous versions.

**Retained from 1.0**: Architecture Decision Records (Nygard), Design by Contract (Meyer), temporal invariants (Lamport), performance budgeting, test-driven specification, the causal chain, the cross-reference web, WHY NOT annotations, comparison blocks, voice guidance, the modularization protocol, and self-bootstrapping validation.

**Retained from 2.0**: The LLM Consumption Model (§0.2.3), negative specifications ([[INV-017|negative spec per chapter]], [[ADR-008|required per chapter]], §3.8), structural redundancy ([[INV-018|substance restated]], [[ADR-009|over DRY]]), verification prompts ([[INV-019|self-check per chapter]], [[ADR-010|required per chapter]], §5.6), meta-instructions ([[INV-020|explicit sequencing]], §5.7), and Gate 7 (LLM Implementation Readiness).

**Added in 3.0**:

- **Machine-readable cross-references** ([[INV-022|parseable refs]], [[ADR-013|wiki-link syntax]], §10.3) enable automated graph construction, staleness detection, and density validation — turning spec quality from a review concern into a CI concern.
- **Automated specification testing** (§12.3, Gate 8) provides seven concrete structural tests that catch cross-reference errors, orphan sections, stale restatements, and weight imbalances programmatically.
- **ADR confidence levels** ([[ADR-012|Decided vs Provisional]]) surface technical debt and prevent "provisional forever" decisions from calcifying.
- **Composability protocol** (§0.2.5) defines how DDIS specs reference each other: by stable contract, with version pinning, and with explicit external dependency declarations.
- **Incremental authoring workflow** (§11.3) supports iterative spec development with validation checkpoints per phase, replacing the linear "write everything then validate" approach.
- **Element composition trace** (§5.8) demonstrates how all DDIS element types compose within a single implementation chapter.
- **Proportional weight invariant** ([[INV-021|weight compliance]]) formalizes the weight guide as a testable property with tolerance bands.
- **Restored modularization protocol** (§0.13.8–0.13.13) — full content where 2.0 had only stubs, fixing the self-containment gap.

The result is a specification standard that is:

- **Decision-driven**: Architecture emerges from locked decisions with explicit confidence levels
- **Invariant-anchored**: Correctness is defined before implementation
- **Falsifiable throughout**: Every claim can be tested
- **LLM-optimized**: Every structural element prevents a specific LLM failure mode
- **Automatically testable**: Structural quality is validated by tooling, not just review
- **Composable**: Specs reference each other through stable, versioned contracts
- **Self-validating**: Quality gates — including LLM readiness and automated testing — provide conformance checking
- **Self-bootstrapping**: This document is both the standard and its first conforming instance

*DDIS 3.0: Where rigor meets readability — and specifications become implementations, whether the implementer is human or machine.*
