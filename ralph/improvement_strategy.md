# Prompt: Recursive Self-Improvement of the DDIS Standard

## Meta-Context: LLMs All the Way Down

This prompt will be executed by an LLM. The LLM will produce a spec optimized for LLM consumption. That spec will be used by LLMs to write further specs. Those specs will be consumed by LLMs to produce implementations.

This means **this prompt itself must be optimized for LLM execution.** Everything DDIS 2.0 will prescribe about LLM-friendly specification structure applies reflexively to this prompt. Specifically:

**Processing order is explicit, not implied.** This prompt tells you exactly what to do in what order. Do not reorder steps. Do not skip the audit. Do not jump to writing DDIS 2.0 before completing the Improvement Spec.

**Verification checkpoints are mandatory.** At three points in this prompt, you will encounter `[CHECKPOINT]` markers. At each checkpoint, pause and verify your work against the stated criteria before proceeding. Do not treat checkpoints as suggestions.

**Do not hallucinate content from DDIS 1.0.** You have the actual document attached. When this prompt references a specific section (e.g., "INV-006" or "§0.10.2"), look it up in the attached file. Do not reconstruct it from memory. If you cannot find a referenced section, note that explicitly rather than inventing what it might say.

**Do not infer unstated requirements.** This prompt specifies what the two artifacts must contain. If something is not listed in the quality criteria, it is not required. Adding unrequested elements dilutes focus.

**Structural predictability applies to your output.** When producing the Improvement Spec (Artifact 1), follow DDIS structure exactly — do not improvise a novel organization. When producing DDIS 2.0 (Artifact 2), follow the structure that DDIS 2.0 itself will define. The reader (another LLM, or a human reviewing your work) must be able to predict where to find any given element.

**Your two most likely failure modes are:**
1. Producing a DDIS 2.0 that is cosmetically different but structurally identical to 1.0 (insufficient improvement)
2. Producing a DDIS 2.0 that is ambitious but internally inconsistent — prescribing things it doesn't itself follow (broken self-bootstrap)

Guard against both. The checkpoints will help.

---

## Your Role and Deliverable

You are an expert in specification methodology, formal methods, and document engineering. You have one attached document:

1. **ddis_standard.md** (~1,600 lines) — the Decision-Driven Implementation Specification (DDIS) standard, version 1.0. This is a self-bootstrapping meta-specification: it defines a standard for writing implementation specifications, and it is itself written in the format it defines.

**The central optimization target for DDIS 2.0 is LLM consumption.** The primary implementer reading a DDIS-conforming spec will be a large language model (Claude, GPT, Gemini, or successors). Human readability remains a requirement — humans review, audit, and evolve specs — but when human readability and LLM effectiveness conflict, LLM effectiveness wins. Every improvement you propose should be evaluated through the lens: "does this make an LLM more likely to produce a correct implementation on the first pass?"

This has specific implications you must internalize before auditing:

- **LLMs lose context in long documents.** Structural redundancy (restating key invariants at point of use, not just in §0.5) may be worth the line cost.
- **LLMs hallucinate details not in the spec.** Explicit "do not infer X" constraints and negative specifications prevent the most common LLM failure mode.
- **LLMs over-index on examples.** Worked examples have outsized influence on LLM output. Bad examples are actively harmful. The quality bar for examples must be higher than for human-targeted specs.
- **LLMs struggle with implicit cross-references.** "See above" is useless. Explicit section numbers and invariant IDs are mandatory — not just recommended.
- **LLMs benefit from structural predictability.** Fixed formats (every ADR follows the same template, every invariant has the same components) reduce variance in LLM output quality.
- **LLMs can be instructed via the spec itself.** A DDIS spec can contain meta-instructions: "When implementing this subsystem, do X before Y" or "Do not optimize this path without benchmarking first." These are invisible to compilers but valuable to LLM implementers.
- **LLMs handle tabular and structured data better than dense prose.** Where a human might prefer flowing paragraphs, an LLM may perform better with a decision table or a state × event matrix.

Your deliverable is **two artifacts**, produced in order:

### Artifact 1: The Improvement Spec (DDIS-conforming)

A DDIS-conforming specification for improving DDIS itself. This is the recursive core: you are using DDIS to write a spec for how DDIS should be improved. This spec must contain:

- A first-principles analysis of where DDIS 1.0 falls short
- Numbered invariants for what the improved standard must satisfy
- ADRs for each improvement decision
- Worked examples showing before/after for each improvement
- Quality gates for when the improvement is "done"

**Target: 800–1,200 lines.**

### Artifact 2: DDIS 2.0 (the improved standard)

The complete, improved DDIS standard — produced by executing the Improvement Spec from Artifact 1. Not a diff. Not suggestions. The full, final standard incorporating all improvements.

**Target: 2,000–3,000 lines** (DDIS 1.0 is ~1,600 lines; expect growth from addressing gaps, not from padding).

---

## The Recursive Self-Improvement Method

The power of this approach is that DDIS contains its own quality criteria. You will:

1. **Audit** DDIS 1.0 against its own invariants (INV-001 through INV-010, plus INV-011 through INV-016 for modular specs) and quality gates (Gates 1–6, plus Gates M-1 through M-5 for modular specs). Where does it fail its own standards?

2. **Identify** structural gaps, missing elements, and weaknesses that DDIS 1.0 prescribes for other specs but fails to fully deliver for itself.

3. **Discover** meta-level gaps — things a specification standard *should* address that DDIS 1.0 doesn't even know it's missing. These are the most valuable improvements because they can't be found by self-audit alone.

4. **Specify** each improvement using DDIS methodology (invariant, ADR, worked example, test).

5. **Execute** the improvement spec to produce DDIS 2.0.

---

## Audit Framework: Where to Look for Improvements

### Layer 1: Self-Conformance Failures

DDIS 1.0 prescribes elements it doesn't fully deliver for itself. Systematically check:

- Does every section in DDIS 1.0 satisfy INV-001 (Causal Traceability)? Can you trace every prescription back to the formal model?
- Does DDIS 1.0 satisfy INV-004 (Algorithm Completeness) for its own "algorithms" — the authoring sequence, the validation procedure, the cross-reference density check?
- Does DDIS 1.0 satisfy INV-006 (Cross-Reference Density) throughout? Are there orphan sections?
- Does the formal model in §0.2 actually support deriving the full document structure, or is the derivation hand-wavy?
- Are there non-negotiables that lack corresponding invariants?
- Are there quality gates that lack concrete measurement procedures?
- For modular specs: Do INV-011 through INV-016 hold? Do the modularization quality gates (M-1 through M-5) pass? Is the cascade protocol (§0.13.12) complete?

### Layer 2: Structural Gaps

Things DDIS 1.0 prescribes for domain specs but omits for itself:

- **End-to-end trace**: DDIS 1.0 requires domain specs to include an end-to-end trace (§5.3). Does DDIS 1.0 include one for itself? (It should: trace a single element — say, an ADR — from the author's initial recognition of a decision through the DDIS authoring process to its final validated form in the spec.)

- **State machine completeness**: DDIS 1.0 defines a spec lifecycle state machine (§1.1) but does it cover all transitions, guards, and invalid transition policies per INV-010?

- **Error taxonomy**: DDIS 1.0 requires domain specs to classify errors (§6.3). What are the "errors" in specification authoring? Ambiguity, contradiction, orphan sections, missing cross-references, unfalsifiable invariants, strawman ADRs — these could be formally classified.

- **Performance measurement harness**: DDIS 1.0 defines performance budgets for spec authoring (§0.8) but doesn't provide a concrete measurement method. How do you actually measure "time to first question from an implementer"?

### Layer 3: Meta-Level Gaps

Things DDIS 1.0 doesn't know it's missing — improvements that require stepping outside the framework to see:

- **Composability**: How do multiple DDIS specs compose? If System A has a DDIS spec and System B has a DDIS spec, and B depends on A, how do invariants and ADRs cross-reference across spec boundaries? DDIS 1.0 acknowledges this as an open question (§0.10.2) but doesn't solve it.

- **Abstraction levels**: DDIS 1.0 treats all implementation chapters equally. But some systems have natural abstraction levels (kernel vs. library vs. application). Should DDIS prescribe how to handle specs that span multiple abstraction levels?

- **Conditional sections**: Some spec elements only make sense for certain system types. A pure library doesn't need an operational playbook. A distributed system needs a consistency model that a single-process system doesn't. Should DDIS have conditional requirements based on system classification?

- **Negative specification**: DDIS 1.0 focuses on what the system DOES (algorithms, state machines). But specifying what the system must NOT do is equally important for security-critical and safety-critical systems. Should DDIS formalize negative specifications beyond the Non-Goals section?

- **Specification testing**: DDIS prescribes tests for the system being specified, but what about tests for the specification itself? Can invariants about the spec (not about the system) be automatically checked? Can cross-reference integrity be validated programmatically?

- **Structural improvement — modularization**: When auditing a spec, assess whether the spec itself (not just specs written using it) would benefit from modularization per §0.13. The assessment should consider the spec's USAGE context: if the spec is consumed as a reference alongside other work (e.g., an LLM reading this meta-standard to write a new spec), the effective context budget is smaller because the LLM must hold both the reference AND produce output. If modularization is warranted, this counts as a P0 improvement — it directly affects LLM effectiveness. The RALPH loop's Phase 0 can execute the decomposition automatically.

- **Incremental authoring support**: DDIS 1.0's authoring sequence (§11.1) is linear. Real spec authoring is iterative. What structural support does DDIS need for specs that are written incrementally as understanding develops?

- **LLM-specific structural provisions**: This is the primary improvement axis — see the central optimization target above. DDIS 1.0 mentions LLMs as implementers once but does not structurally optimize for them. DDIS 2.0 must. Specific areas to investigate:
  - **Context window management**: Should DDIS prescribe section length limits? Should critical invariants be restated at point of use?
  - **Negative specification**: LLMs hallucinate plausible details. Should DDIS require explicit "do NOT" constraints for each subsystem?
  - **Implementation ordering directives**: LLMs benefit from explicit "implement X before Y because Z depends on X." Should DDIS formalize implementation dependency chains beyond the roadmap?
  - **Example-to-prose ratio**: What is the optimal ratio for LLM consumption? Should DDIS prescribe minimum example counts per element type?
  - **Ambiguity elimination**: Where human readers comfortably infer from context, LLMs may choose randomly among interpretations. Should DDIS require disambiguation of all terms that have multiple common meanings?
  - **Chunking guidance**: If a spec exceeds an LLM's context window, how should it be chunked? Should DDIS prescribe self-contained sections that can be processed independently?
  - **Verification prompts**: Should each implementation chapter end with a self-check prompt the LLM can use to verify its own output against the spec?

- **Traceability to implementation**: DDIS traces from first principles to spec elements. But tracing from spec elements to actual implementation artifacts (files, functions, tests) is also valuable. Should DDIS prescribe an implementation mapping?

- **Deprecation and contradiction handling**: When a spec evolves, sections may contradict each other temporarily. DDIS 1.0's versioning guidance (§13.2) is minimal. How should contradictions be surfaced, tracked, and resolved?

- **Confidence levels**: Not all parts of a spec are equally certain. Early-stage ADRs may be "best guess, revisit after spike." Should DDIS formalize confidence levels on decisions and prescriptions?

---

## Quality Criteria for the Improvement Spec (Artifact 1)

The Improvement Spec must itself be DDIS-conforming. Specifically:

1. **First-principles derivation**: Why does DDIS need improvement? What is the formal model of "specification quality" that reveals the gaps?

2. **Numbered invariants**: What must DDIS 2.0 satisfy that DDIS 1.0 does not? Each improvement-invariant must be falsifiable.

3. **ADRs**: For each proposed improvement, what alternatives were considered? Why was this approach chosen?

4. **Worked examples**: For each improvement, show a before (DDIS 1.0) and after (DDIS 2.0) comparison with specific text.

5. **Quality gates**: How do we know DDIS 2.0 is better than DDIS 1.0? What are the stop-ship criteria for the improvement?

6. **No regressions**: DDIS 2.0 must still pass all of DDIS 1.0's own quality gates. Improvements must be additive, not substitutive, unless the substitution is explicitly justified by an ADR.

---

## Quality Criteria for DDIS 2.0 (Artifact 2)

1. **Passes its own gates**: DDIS 2.0 must pass its own (potentially enhanced) quality gates, just as DDIS 1.0 does.

2. **Passes DDIS 1.0's gates**: Every gate from DDIS 1.0 must still pass. The improvement cannot break existing conformance.

3. **Addresses Artifact 1's invariants**: Every improvement-invariant from the Improvement Spec must be satisfied.

4. **Remains self-bootstrapping**: DDIS 2.0 must still be a valid instance of the format it defines.

5. **Remains readable**: More content must not mean less clarity. The proportional weight guide must be respected. If DDIS 2.0 is 50% longer than DDIS 1.0, every added line must earn its place.

6. **Master TODO updated**: DDIS 2.0's Master TODO must reflect the new/changed elements, with conformance tracked.

7. **LLM implementation test (thought experiment)**: For each implementation chapter template, imagine giving ONLY that chapter (plus the glossary and relevant invariants) to an LLM and asking it to implement the subsystem. Would the LLM have enough information? Would it be likely to hallucinate anything? Would it know what NOT to do? If any answer is unsatisfying, the chapter template needs more structure.

8. **Negative specification coverage**: Every element specification in DDIS 2.0 must include not just what the element IS but what it must NOT be — anti-patterns, common LLM failure modes for that element, and explicit constraints that prevent the most likely misinterpretations.

---

## Ordering and Process

### Step 1: Deep Audit (do not skip)

Before writing anything, audit DDIS 1.0 systematically:

- Read the entire document
- Check each invariant against the document itself (does DDIS 1.0 satisfy INV-001 through INV-010 applied to itself?)
- Check each quality gate (does DDIS 1.0 pass Gates 1–6 applied to itself?)
- Note every gap, inconsistency, or weakness
- Note every "open question" from §0.10 and assess whether it should be resolved in 2.0

**[CHECKPOINT 1]** Before proceeding, verify:
- You have identified at least 3 self-conformance failures (places DDIS 1.0 violates its own rules)
- You have identified at least 3 LLM-specific structural gaps (things that would cause an LLM implementer to fail)
- You have explicitly referenced section numbers and invariant IDs from the actual attached document — not reconstructed from memory
- Your findings are organized by the P0/P1/P2/P3 priority framework

If any of these are missing, return to the audit before proceeding to Step 2.

### Step 2: Prioritize Improvements

Not all improvements are equal. Use this priority framework:

| Priority | Criteria | Examples |
|---|---|---|
| P0: LLM effectiveness | Changes that directly improve LLM implementation success rate | Structural redundancy, negative specs, explicit cross-refs, implementation meta-instructions, example quality |
| P0: Self-conformance fixes | DDIS 1.0 violates its own rules | Missing end-to-end trace, incomplete state machine |
| P1: Structural gaps | DDIS 1.0 prescribes X for others but lacks X itself | Error taxonomy, measurement harness |
| P2: Valuable additions | New elements that materially improve spec quality | Composability, confidence levels |
| P3: Polish | Nice-to-have improvements | Additional examples, expanded anti-patterns |

**All P0 and P1 items must be addressed. P2 items should be addressed if they can be done well within the line budget. P3 items are optional.**

**The LLM-effectiveness litmus test**: For every improvement you propose, ask: "If I gave the resulting spec to Claude/GPT with no other context, would this change make the implementation more likely to be correct?" If the answer is "no, but it helps humans," the improvement is P2 at best.

### Step 3: Write Artifact 1 (Improvement Spec)

Write the DDIS-conforming improvement spec. This is where you formalize what you found in the audit, commit to specific improvements via ADRs, and define quality gates for the improvement.

**[CHECKPOINT 2]** Before proceeding to Step 4, verify Artifact 1:
- It has a first-principles derivation explaining WHY DDIS needs improvement (not just WHAT to improve)
- Every proposed improvement has an ADR with genuine alternatives (not strawmen)
- Every improvement has a before/after worked example with specific text from DDIS 1.0
- LLM-effectiveness improvements constitute at least 40% of the proposed changes
- The improvement spec is itself DDIS-conforming (it has invariants, ADRs, quality gates, cross-references)
- You have not proposed removing any element from DDIS 1.0 without an ADR justifying the removal

If any of these are missing, revise Artifact 1 before proceeding.

### Step 4: Execute Artifact 1 → Produce Artifact 2 (DDIS 2.0)

Apply the improvement spec to produce the complete DDIS 2.0 document. This is a full rewrite of ddis_standard.md incorporating all improvements — not a patch file.

**[CHECKPOINT 3]** After completing Artifact 2, verify:
- DDIS 2.0 passes all of DDIS 1.0's quality gates (no regressions)
- DDIS 2.0 passes its own (enhanced) quality gates
- DDIS 2.0 is self-bootstrapping (it conforms to the format it defines)
- Every improvement from Artifact 1 is actually present in Artifact 2 (no dropped improvements)
- LLM-specific provisions are woven throughout, not isolated in a single chapter
- The Master TODO is updated and reflects new elements
- Proportional weight is respected (no section has ballooned disproportionately)

If any of these fail, revise Artifact 2. Do not deliver a DDIS 2.0 that fails its own standards — that would break the self-bootstrapping property and undermine the entire standard.

---

## Anti-Patterns to Avoid

**The Cosmetic Improvement**: Rewriting prose without changing substance. If a section's content is correct but its wording could be slightly better, that is P3 at best. Focus on structural improvements.

**The Kitchen Sink**: Adding every possible meta-specification concept you can think of. DDIS's power is in its constraint. Every addition must pass INV-007 (Signal-to-Noise): does removing it make the standard worse?

**The Regression**: Improving one area by degrading another. If adding composability guidance makes PART 0 twice as long, the proportional weight is violated.

**The Infinite Recursion**: This prompt asks you to improve DDIS using DDIS. It does NOT ask you to improve the improvement process using the improvement process. One level of recursion. Produce DDIS 2.0 and stop.

**The Afterthought LLM Section**: Do NOT add a single "Chapter 14: LLM Considerations" appendix and call it done. LLM optimization must be woven throughout — into the element specifications (how each element should be structured for LLM parsing), into the voice guide (what prose patterns LLMs handle well vs. poorly), into the quality gates (LLM-specific validation), into the invariants (properties that specifically prevent LLM failure modes). The LLM lens is a pervasive concern, not a bolt-on chapter.

**The Abstraction Astronaut**: Adding meta-meta-levels ("a specification about specifications about specifications"). DDIS is one level meta (a standard for specs). Keep it there.

---

## Per-Module Improvement Variant (Modular Mode)

When the RALPH loop runs in modular mode (via `--modular`), individual modules are improved as assembled bundles rather than as the monolithic spec. This changes the improvement task in specific ways:

**What changes:**
- You receive a *bundle* (constitution + module), not the full spec. The bundle is self-contained by design (INV-011: Module Completeness).
- Your improvement scope is the *module portion only*. Do NOT modify constitutional content — that was improved in Phase 1.
- The module header declares what invariants this module maintains, interfaces with, and which modules are adjacent. Use this to focus your improvements.
- Negative specifications in the module header tell you what this module must NOT do. Verify these are complete and add any missing ones.

**What stays the same:**
- All quality criteria, checkpoints, and anti-patterns apply identically.
- The audit framework applies to the module's portion of each invariant and gate.
- Structural predictability, LLM optimization, and self-conformance requirements are unchanged.
- Improvements must still be substantive (not cosmetic) and must not regress existing quality.

**Module-specific audit additions:**
- Does the module header accurately reflect the invariants it maintains and interfaces with?
- Are all cross-module references going through constitutional elements (INV-012)?
- Does the module satisfy INV-011 (completeness) — can an LLM implement from this bundle alone?
- Are the module's negative specifications comprehensive enough to prevent common LLM hallucinations?
- Is the module's implementation content correctly scoped to its declared domain?

**Output format in modular mode:** Produce only the improved bundle content (the module portion). Do not re-output the constitutional tiers — they are read-only in Phase 2.

---

## Deliverable Format

Produce both artifacts in a single response, clearly separated:

```
# ═══════════════════════════════════════════
# ARTIFACT 1: IMPROVEMENT SPEC
# ═══════════════════════════════════════════

[The DDIS-conforming improvement spec, 800–1,200 lines]

# ═══════════════════════════════════════════
# ARTIFACT 2: DDIS 2.0
# ═══════════════════════════════════════════

[The complete improved DDIS standard, 2,000–3,000 lines]
```

Begin with the audit. Then write. Make it better.

