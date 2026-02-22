# Module: Element Specifications
<!-- domain: element-reference -->
<!-- maintains: (none) -->
<!-- interfaces_with: INV-001, INV-002, INV-003, INV-004, INV-005, INV-006, INV-007, INV-008, INV-009, INV-010 -->
<!-- adjacent: core-framework, guidance-and-practice -->
<!-- budget: 600 lines -->

## Negative Specifications
- This module MUST NOT redefine invariants or ADRs — it only references them as quality criteria for elements
- This module MUST NOT contain high-level architectural framing (executive summary, formal model derivation) — that belongs to core-framework
- This module MUST NOT contain authoring workflow advice (ordering, common mistakes) — that belongs to guidance-and-practice
- This module MUST NOT contain modularization procedures — that belongs to modularization-protocol

---

## PART II: Core Standard — Element Specifications

Each section specifies one structural element: what it must contain, quality criteria, how it relates to other elements, and what good vs. bad looks like.

### Chapter 2: Preamble Elements

#### 2.1 Design Goal

**What it is**: A single sentence (≤ 30 words) stating the system's reason for existing.

**Required properties**: States core value proposition (not implementation). Uses bold for 3–5 key properties. Readable by non-technical stakeholder.

**Quality criteria**: A reader seeing only the design goal can decide relevance.

**Anti-pattern**: "Build a distributed task coordination system using event sourcing and advisory reservations." ← Describes implementation, not value.

**Good example**: "Design goal: **scrollback-native, zero-flicker, agent-ergonomic, and high-performance** Rust terminal apps."

**Cross-references**: Each bolded property should correspond to at least one invariant and one quality gate.

#### 2.2 Core Promise

**What it is**: A single sentence (≤ 40 words) describing capabilities from the user's perspective.

**Required properties**: User's viewpoint. Concrete capabilities. Uses "without" clauses to highlight what isn't sacrificed.

**Anti-pattern**: "The system provides robust, scalable, enterprise-grade coordination." ← Meaningless buzzwords.

#### 2.3 Document Note

**What it is**: 2–4 sentence disclaimer about code blocks and where correctness lives.

**Template**:
> Code blocks are **design sketches**. The correctness contract lives in the invariants, tests, and ADRs — not in pseudo-code syntax.

#### 2.4 How to Use This Plan

**What it is**: 4–6 item numbered list with practical reading/execution guidance. Must start with "Read PART 0 end-to-end," identify churn-magnets, point to Master TODO, identify at least one non-negotiable process requirement.

### Chapter 3: PART 0 Elements

#### 3.1 Non-Negotiables (Engineering Contract)

5–10 properties defining what the system IS. Stronger than invariants — philosophical commitments that must never be compromised.

**Format**: `- **[Property name]** [One concrete sentence]`

**Quality criteria**: An implementer could imagine a tempting violation scenario; the non-negotiable clearly says no. Not a restatement of a technical invariant — it's the "why" that justifies groups of invariants.

#### 3.2 Non-Goals

5–10 explicit exclusions. The immune system against scope creep.

**Quality criteria**: Someone has actually asked for this (not absurd exclusions). Brief explanation of why excluded.

**Anti-pattern**: "Non-goal: Building a quantum computer." ← Nobody asked.

#### 3.3 First-Principles Derivation

The formal model making the architecture feel *inevitable* rather than *asserted*.

**Required**: (1) Mathematical system definition as state machine or function. (2) 3–5 consequence bullets. (3) Fundamental operations table.

**Quality criteria**: After reading, an implementer could derive the architecture independently.

#### 3.4 Invariants

Numbered properties that must hold at all times.

**Required format**:
```
**INV-NNN: [Name]**
*[Plain-language statement]*
  [Semi-formal expression]
Violation scenario: [Concrete description]
Validation: [Named test strategy]
// WHY THIS MATTERS: [Consequences of violation]
```

**Quality criteria**: Falsifiable (constructible counterexample). Consequential (violation causes bad behavior). Non-trivial (not a type constraint). Testable.

**Quantity**: 10–25 for medium-complexity systems.

**Anti-patterns**:
- "The system shall be performant." ← Not falsifiable.
- "TaskId values are unique." ← Trivially enforced by type system.

#### 3.5 Architecture Decision Records

**Required format**: Problem → Options (≥2, ≤4, genuine alternatives) → Decision with WHY NOT → Consequences → Tests.

**Quality criteria**: Genuine alternatives (competent engineer would choose each in some context). Concrete tradeoffs (specific, measurable). Consequential decision (> 1 day refactoring to change).

**Anti-pattern**: Options where Option B is a strawman nobody would choose.

**Churn-magnets**: After all ADRs, identify which decisions cause the most downstream rework if changed.

#### 3.6 Quality Gates

4–8 stop-ship predicates, ordered by priority. Each references specific invariants/tests. Failing Gate N makes Gate N+1 irrelevant.

#### 3.7 Performance Budgets and Design Point

**Required**: (1) Design point (hardware, workload, scale). (2) Budget table: operation → target → measurement. (3) Measurement harness description. (4) Adjustment guidance.

**Anti-pattern**: "The system should be fast enough for real-time use." ← No number, no design point, no method.

### Chapter 4: PART I Elements

#### 4.1 Full Formal Model
Expanded first-principles derivation: complete state, input/event taxonomy, output/effect taxonomy, transition semantics, composition rules.

#### 4.2 State Machines
Every stateful component gets: state diagram, state × event table (no empty cells), guard conditions, invalid transition policy, entry/exit actions.

#### 4.3 Complexity Analysis
Big-O bounds with constants where they matter for the design point.

### Chapter 5: PART II Elements

#### 5.1 Implementation Chapters

One chapter per major subsystem. **Required components** (10 items):
1. Purpose statement (2–3 sentences, references formal model)
2. Formal types with memory layout analysis
3. Algorithm pseudocode with inline complexity
4. State machine (if stateful)
5. Invariants preserved (INV-NNN list)
6. Worked example(s) with specific values
7. Edge cases and error handling
8. Test strategy (unit, property, integration, replay, stress)
9. Performance budget (subsystem's share)
10. Cross-references (ADRs, invariants, other subsystems, formal model)

**Quality criteria**: Implementer could build subsystem from this chapter alone.

#### 5.2 Worked Examples

Concrete scenarios with specific values (not variables). Shows state before, operation, state after. Includes at least one non-trivial aspect.

**Anti-pattern**: "When a task is completed, the scheduler updates the DAG." ← No concrete values, no before/after, no edge case.

#### 5.3 End-to-End Trace

Single scenario traversing ALL subsystems. Shows exact data at each boundary. Identifies invariants exercised at each step. Includes at least one cross-subsystem interaction that could go wrong. (Validated by INV-001.)

#### 5.4 WHY NOT Annotations

Inline comments explaining the road not taken. Use when an implementer might think "I can improve this by doing X" and X was considered and rejected.

Format: `// WHY NOT [alternative]? [Brief tradeoff. Reference ADR-NNN if exists.]`

If annotation grows beyond 3 lines, promote to ADR.

#### 5.5 Comparison Blocks

Side-by-side ❌ SUBOPTIMAL vs ✅ CHOSEN with quantified reasoning. For data structure, algorithm, or API choices where quantitative difference is the justification.

### Chapter 6: PART IV Elements

#### 6.1 Operational Playbook

**6.1.1 Phase -1: Decision Spikes** — Tiny experiments that de-risk unknowns. Each produces an ADR. Max time budget per spike.

**6.1.2 Exit Criteria per Phase** — Specific, testable conditions per phase. Not "scheduler works" but "property test demonstrates fair scheduling across 50 agents with no starvation for > 1000 ticks."

**6.1.3 Merge Discipline** — What every PR touching invariants or critical paths must include.

**6.1.4 Minimal Deliverables Order** — Build order maximizing "working subset" at each stage.

**6.1.5 Immediate Next Steps** — First 5–6 things to implement in dependency order.

#### 6.2 Testing Strategy

| Test Type | What It Validates | Example |
|---|---|---|
| Unit | Individual function correctness | Reservation conflict detection |
| Property | Invariant preservation under random inputs | Replay determinism |
| Integration | Subsystem composition | Task completion triggers scheduling |
| Stress | Behavior at design point limits | 300 agents, 10K tasks, 60s |
| Replay | Determinism | Process N events, snapshot, replay, byte-compare |
| Adversarial | Robustness against malicious input | Forged task_id |

#### 6.3 Error Taxonomy

Each error class has: severity (fatal/degraded/recoverable/ignorable), handling strategy (crash/retry/degrade/log), cross-references to threatened invariants.

### Chapter 7: Appendix Elements

#### 7.1 Glossary
Alphabetized domain-specific terms, 1–3 sentences each, with cross-reference to formal definition. Distinguish common vs. domain-specific meanings.

#### 7.2 Risk Register
Top 5–10 risks with: description, impact, mitigation, detection method.

#### 7.3 Master TODO Inventory
Checkboxable tasks organized by subsystem (not phase), each small enough for a single PR, cross-referenced to ADRs/invariants.
