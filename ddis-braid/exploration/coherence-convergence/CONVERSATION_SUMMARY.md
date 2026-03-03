# Conversation Compaction Summary

## Session: Braid Coherence Engine Architecture (2026-03-03)

### Comprehensive Seed Document
All decisions, architecture, analysis, schema, invariants, open questions, and next actions are captured in:
**`/mnt/user-data/outputs/SEED-coherence-engine.md`** (v2, 17 sections)

Read this document FIRST — it is the authoritative state of the conversation.

### Source Materials Available
- **Original Braid architecture transcript**: `/mnt/user-data/uploads/Datomic_implementation_in_Rust.md` (10,055 lines) — the "Datomic Implementation in Rust" session that defined Braid's core architecture (five axioms, 31 entity types, ~195 attributes, 12 lattice definitions, CRDT semantics, query strata)
- **Earlier portion of this conversation's transcript**: `/mnt/transcripts/2026-03-03-19-20-21-braid-datalog-prolog-architecture.txt` (1,358 lines) — covers the initial Datalog vs Prolog analysis and first round of clarifying questions
- **SEED.md (Braid spec)**: `https://raw.githubusercontent.com/wvandaal/ddis/refs/heads/main/ddis-braid/SEED.md` — the master specification seed document
- **Seven DDIS Primitives document**: Provided inline during session (INV, ADR, NEG, UNC, Contradiction Detection 5-tier, F(S) 7 dimensions, Bilateral Loop)

### Arc of the Conversation (Detailed)

**Phase 1 — Datalog vs Prolog Analysis:**
Willem asked whether Prolog makes sense for Braid alongside Datalog, aiming for "zero-defect, formal methods, cleanroom" implementation. Analysis established Datalog must remain primary query engine (CALM, termination, CRDT properties). Prolog adds unification, goal-directed search, meta-programming. Willem chose: both — termination for queries, Turing-complete for verification. Supplement Datalog, don't replace.

**Phase 2 — Architecture Proposals:**
Two architectures compared: stratified separation (two engines) vs unified tabled engine (one engine, two modes). FFI integration via existing SQ-010 mechanism identified as the natural seam. Cleanroom build chosen over Scryer Prolog integration. Rules-as-datoms decided (agents can write rules freely). Fuel-bounded evaluation chosen over unbounded-with-timeout.

**Phase 3 — Critical Assessment:**
Claude pushed back hard on the plan. Key criticisms: (1) system over-specified top-down from theory when empirical evidence favors bottom-up from usage; (2) Prolog verification engine solving a problem that doesn't exist yet; (3) "zero-defect via Prolog" is a category error (Prolog search ≠ formal proof in CompCert/seL4 sense); (4) formalism outrunning usability. Recommended stripping back radically to prove harvest/seed transforms workflows first.

**Phase 4 — Willem's Context Correction:**
Willem provided critical context: the Go CLI's 0% bilateral lifecycle adoption was because it was never built as first-class, not because it's not valuable. Braid explicitly fixes this. Willem's actual use case for Prolog: divergence detection and diagnosis in evolving specifications, not zero-defect via formal proof. This reframed the entire analysis.

**Phase 5 — The Breakthrough ("Type System for Specifications"):**
The single most innovative addition identified: live coherence checking during spec authoring, in natural language. LLM translates prose invariants to Horn clauses at transact time; coherence engine checks consistency in subsecond; contradictions surfaced immediately with diagnosis and resolution options. Willem confirmed: "This is literally exactly what I want." The coherence engine moved from Stage 2+ to Stage 0.

**Phase 6 — Expansion to All Spec Element Types:**
Analysis showed ADRs contribute three kinds of logical content: commitments, assumptions, and exclusions. Each enables different coherence checks. Negative cases already formal (temporal logic). Goals need satisfaction conditions. The translation prompt becomes structured slot-filling, not arbitrary logic extraction.

**Phase 7 — The Seven Primitives Analysis (Final Deepening):**
Willem provided the Seven DDIS Primitives document. This revealed the coherence engine isn't a checker for one primitive — it's **the computational substrate for all seven primitives.** Every relationship arrow in the primitive interaction web becomes a computable predicate. Key new insights:

1. **Ten meta-rules** in three families (Contradiction, Drift, Coverage) cover the entire primitive interaction web
2. **Meta-rules form a fixed-point system** — cascading to convergence like Datalog semi-naive evaluation
3. **Three cascade patterns**: goal change cascade, uncertainty-resolution-reveals-contradiction, constructive goal entailment
4. **Five contradiction tiers** map precisely to engine capabilities (Tiers 1-3: Datalog+LLM, Tier 4: Prolog's core strength, Tier 5: goal entailment)
5. **F(S) fitness function becomes partially computable** — 5 of 7 dimensions automated
6. **Four-layer predicate ontology**: spec facts → logical forms → meta-rules → metrics + resolution
7. **Self-referential coherence**: meta-rules are spec elements, engine verifies itself
8. **Spec-level catches what implementation-level tools cannot**: contradictions in the spec itself

### Current State
The conversation has completed a full arc from "should we use Prolog?" through to "this is the computational core of DDIS." The concept is fully developed. Willem was exploring "something deeper" — the seven-primitive analysis was that deeper exploration. Both the seed document and this compaction summary are now updated to v2 reflecting all insights.

### Recommended Next Action
The expanded feasibility experiment: test LLM translation of all seven primitive types into logical forms, then test cross-element coherence checking. This is the highest-risk, lowest-cost validation of the entire concept.

### Key Context for Continuation
- Willem works in franchise business development with deep technical interests in formal methods
- DDIS is his meta-specification standard; Braid is the embedded temporal database substrate
- Go CLI (62,500 lines) being replaced by Braid (Rust, datom-based)
- No time pressure — priority is getting the spec right
- Values honest, direct critical assessment over validation
- "Type system for specifications" is the resonant framing
- Hardest unsolved design problems: `incompatible/2` and `entails/2` (the property vocabulary)

