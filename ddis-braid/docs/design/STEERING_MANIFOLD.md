# Braid as Latent Space Navigation System

> **The Complete Theory**: How braid steers LLMs through their own knowledge manifold.
> This document maps the entire idea space — from the foundational reframing through
> the Piagetian framework, Y-combinator structure, and acquisition function, to the
> deep connection with prompt optimization theory. Every claim traces to a primary
> source (session ID, line number, ADR, or memory file).

---

## 1. THE FOUNDATIONAL REFRAMING

**Primary source**: Session 042 (id: `56ece7bd`), lines 2416-2424

> **Braid is not a knowledge store. It is a navigation system for a pre-existing knowledge manifold.**

The LLM already contains the knowledge — structured, deeply associated representations of every domain in its training data. The datom store contains three things:

1. **Where the agent has been** (observations, harvests — the trajectory)
2. **Where it should go next** (R(t) routing, acquisition function — the steering)
3. **Calibration data for steering accuracy** (hypothesis ledger — the feedback)

The implication is total: **every token that crosses the Markov blanket — in either direction — steers the LLM through its knowledge manifold.** Not just outputs. Commands, flags, help text, error messages, schema attribute names, output structure, even silences. The command name `braid observe` steers differently than `braid record` would. The flag `--confidence` steers differently than `--certainty`. The error `"attribute not found in schema"` steers differently than `"unknown attribute"`. The API design IS the epistemology. The quality of braid IS the quality of its steering across every surface the LLM touches.

A `braid status` that returns metadata about an empty container produces a journal-keeping agent. A `braid status` that returns a cognitively structured view produces a schema-building agent.

**Verbatim from session (line 2420)**:
> "The CLI is a Markov blanket — the boundary across which the store's state (the persistent model) and the LLM's state (the transient model) exchange information. Every `braid status`, every `braid observe` response, every `braid seed` output is a **steering vector** injected into the LLM's context, shaping what the LLM attends to, what it explores next, what categories it thinks in."

---

## 2. THE STEERING MECHANISM (How It Works)

**Primary source**: Session 042 (id: `56ece7bd`), line 2424

Every braid CLI response has three components, each a steering vector:

| Component | What it does | Activation effect |
|---|---|---|
| **Concept membership** | "You're building knowledge in area X, here's its size" | Activates the LLM's schema for that area, primes assimilation |
| **Structural gap** | "Here's unexplored territory connected to what you just found" | The acquisition function rendered as natural language |
| **The question** | Specific, falsifiable question from concept topology | Steers toward the highest-surprise boundary between concepts |

**The current failure**: `braid observe` returns `"recorded +9 datoms. No connections yet."` — a receipt, not a steering event. It wastes the single most valuable moment: right after the agent has articulated a finding, when its attention is maximally focused on that topic.

**The fix**: After the agent observes "materialize engine implements deterministic fold over events":
```
observed: +9 datoms -> concept:event-processing (3 members)
  this concept spans: events/, materialize/ -- but not cascade/ (imports materialize, unexplored)
  related concept: error-handling (2 members) shares no packages -- potential cross-cutting gap
-> what happens when the fold encounters a malformed event?
```

Three lines. Each computed from the concept graph without any LLM call. The question targets the boundary between two concepts — where the most surprising finding is likely to be.

**Verbatim from session**:
> "This is the whole thing. Everything else — embeddings, concept entities, innate schemas, developmental stages — exists to make THIS response possible. The response is the product. The infrastructure is invisible."

And:
> "Braid stops being a recording system that occasionally suggests things. It becomes a **cognitive partner** that listens to what you found, places it in the structure of what's known, identifies what's missing, and asks the question that would be most valuable to answer next."
>
> "The Markov blanket becomes bidirectional. The agent shapes braid's knowledge. Braid shapes the agent's attention. The co-evolution is the convergence."

---

## 3. THE PIAGETIAN FOUNDATION

**Primary source**: Session 042 (id: `56ece7bd`), line 2416; crystallized as ADR-FOUNDATION-034

Piaget's genetic epistemology maps precisely to braid:

| Piaget | Braid |
|---|---|
| **Schema** | Concept entity in the store |
| **Assimilation** | Observation fits existing concept (high cosine similarity) |
| **Disequilibrium** | Observation fits no concept (low similarity to all) — a signal, not a failure |
| **Accommodation** | Concept splits, merges, or restructures |
| **Sensorimotor stage** | Fresh store with innate schemas + raw observations |
| **Concrete operational** | ~20 obs: domain-specific concepts emerge |
| **Formal operational** | ~50 obs: cross-cutting abstractions form |

**Five innate schemas** (Piagetian reflexes for structured artifacts, C8-compliant):
1. **Components** — parts and boundaries
2. **Dependencies** — how parts relate
3. **Invariants** — what should be true
4. **Patterns** — recurring structures
5. **Anomalies** — deviations from patterns

These are substrate-independent. They work for Go CLIs, React apps, research labs, compliance systems. They come from the policy manifest (so different domains can have different innate schemas).

**The bootstrapping insight**: Braid on its own codebase has 42+ sessions of self-knowledge. Braid on a new project starts from zero. The LLM is full; the store is empty. Innate schemas bridge this gap — they give the store enough structure to begin assimilating observations productively from the very first `braid observe`.

**Verbatim**:
> "The infant doesn't need to be taught what a schema is. The infant grasps, encounters resistance, accommodates. The machinery is innate. The schemas emerge from interaction with the world. Braid doesn't need to be taught what a concept is. The agent observes, braid detects clustering, concepts crystallize, status surfaces them, the agent's next observation is shaped by the surfaced concepts."

**Concept crystallization as ontology discovery** (OBSERVER-6):

1. **Innate schemas** — Piagetian scaffolding at `braid init`
2. **Embedding-based membership** — cosine similarity determines assimilation
3. **Concept evolution** — split when variance exceeds threshold, merge when centroids converge
4. **Developmental trajectory** — sensorimotor -> concrete -> formal operational
5. **Memetic transmission** — harvest captures concept graph, seed transmits it
6. **Cross-project transfer** — CRDT merge on concept schemas (cultural evolution)

**Verbatim**:
> "Braid's self-knowledge becomes the genome for its offspring sessions."

---

## 4. THE Y-COMBINATOR FOR LLMs

**Primary source**: Session 038 (id: `d9ac2e6b`); memory: `session-038-y-combinator.md`, `bedrock-vision.md`

**Braid is the fixed-point combinator that gives stateless LLMs self-reference.**

```
Y = lambda f.(lambda x.f(x x))(lambda x.f(x x))
```

Y takes a function that cannot refer to itself and gives it self-reference through a fixed point: `f(Y(f)) = Y(f)`.

An LLM is `context -> continuation` — stateless, memoryless, unable to self-refer across invocations. Braid wraps it:

```
seed -> agent session -> harvest -> updated store -> new seed -> ...
```

When this converges, the seed contains exactly the information to produce the session that produces the harvest that produces the seed. **The function finds itself in its own output.**

Each capability is Y applied at a different level:

| Level | Y applied to | Mechanism |
|---|---|---|
| L0 | Memory | Store + seed/harvest |
| L1 | Self-observation | Daemon (introspective observer) |
| L2 | Learning | Calibration (hypothesis ledger) |
| L3 | Inquiry | FEGH bridge hypotheses |
| L4 | Ontology | Ontological surprise detection |
| L5 | Meta-learning | Load-bearing failure escalation |

**The hierarchy closes into a loop. The loop IS Y. F(S) -> 1.0 IS the fixed point. Free energy -> 0 IS convergence.**

**The Hegelian connection**:
- Thesis = observation (what reality looks like)
- Antithesis = comparison to model (how the model disagrees)
- Synthesis = reconciliation (updated model that subsumes both)
- Aufhebung = the harvest/seed cycle (preserves, negates, transcends)

The Hegelian dialectic (observe/compare/reconcile) IS Y iterating toward the fixed point.

---

## 5. THE OBSERVATION-PROJECTION DUALITY

**Primary source**: Session 034; ADR-FOUNDATION-016; memory: `session-034-formalism.md`

Observation (Reality -> Store) and Projection (Store -> Reality) are **adjoint functors**.

| Projection | Source | Target |
|---|---|---|
| Code | Spec | Source files |
| Tests | Invariants | Test suite |
| Seeds | Store through relevance filter | Agent context |
| AGENTS.md | Methodology through behavioral filter | Operating instructions |
| Status | Fitness + tasks + boundaries | CLI output |

**Every boundary has two morphisms**: observation (in) and projection (out). The boundary is "closed" when the round-trip is coherent:

```
observe(project(store)) ~ store
```

**F(S) = 1.0 when observation and projection are inverse functors — when knowing IS doing.**

**Connection to steering**: Every projection IS a steering vector. The seed is a projection that steers the agent. Status output is a projection that steers attention. Guidance is a projection that steers methodology. The observation-projection duality means braid's steering (projections) and the agent's contributions (observations) form a coupled dynamical system that converges toward coherence.

The Universal Action Queue (ADR-FOUNDATION-019) scores both directions:
- When F(S) is low from observation gaps -> observe more (discovery)
- When F(S) is low from projection gaps -> project more (execution)
- **The system shifts between discovery and execution automatically.**

---

## 6. THE ACQUISITION FUNCTION + FREE ENERGY GRADIENT

**Primary source**: Session 036 (id: `92160d00`); ADR-FOUNDATION-025/028/030

**The universal scoring function**:
```
alpha(action | store) = E[delta_F(S)] / cost(action)
```

This collapses five ranking mechanisms into one: impact x relevance x novelty x confidence / cost. But alpha alone is **pure exploitation** — it has no exploration term.

**The extended form** (from the free energy gradient insight):
```
alpha(action) = E[delta_F(S)] + beta(phase) * IG(region(action))
```

Where:
- `E[delta_F(S)]` = expected fitness improvement (exploitation)
- `beta(phase)` = exploration coefficient (high early, decreases through lifecycle, spikes before deployment)
- `IG(region)` = information gain in the region of the action (exploration)

**The deepest insight about exploration** (verbatim, line 1403):
> "The most valuable observation is not the one that covers new territory. It's the one that CONNECTS previously disconnected understanding."

This is FEGH — the Free Energy Gradient Hypothesis (ADR-FOUNDATION-030):
- Generate candidate bridging observations from graph structure
- Score them with `project_delta` (hypothetical delta_F(S))
- Rank in the same UAQ queue as tasks and other actions
- **"The fog is a signal, not an absence."** Coverage gaps are as informative as knowledge.

**Verbatim (line 1386)**:
> "The agent's trajectory through a session is a path on an **epistemological manifold**. But unlike standard Bayesian optimization where the manifold is given (a continuous d-dimensional space), **braid must discover the manifold it's exploring.** The topology of the knowledge space isn't known in advance — it emerges through exploration."

**Creativity** (line 1398):
> "Creativity is the heavy tail of the exploration distribution — the system's willingness to look where it doesn't expect to find anything — and it is calibrated by the same falsification pipeline that calibrates everything else: propose, predict, observe, measure, adjust."

**Connection to steering**: The acquisition function IS the steering algorithm. When braid returns a question like "what happens when the fold encounters a malformed event?", that question IS the maximum-alpha action — the highest expected information gain per unit cost — expressed as natural language.

---

## 7. THE HYPOTHETICO-DEDUCTIVE LOOP

**Primary source**: Session 034; ADR-FOUNDATION-017

The complete architecture reduces to one cycle:

```
OBSERVE -> RECONCILE -> HYPOTHESIZE -> ACT -> INTEGRATE -> CALIBRATE -> repeat
```

| Phase | Operation | Formal analog |
|---|---|---|
| Observe | Extractors/agents produce datoms from reality | Bayesian prior update |
| Reconcile | 8-type divergence taxonomy classifies where model != reality | Likelihood computation |
| Hypothesize | Acquisition function selects optimal action to reduce divergence | Experimental design |
| Act | Execute the hypothesis (observe or project) | Experiment |
| Integrate | store U new datoms (set union, C4) | Posterior update |
| Calibrate | predicted vs actual delta_F(S) -> adjust acquisition function | Meta-learning |

**Phase 6 is the recursive gain**: the system improves not just the model but its ability to choose what to do next. **The system learns how to learn.**

Convergent by construction: Bayesian inference with active learning.

---

## 8. THE EPISTEMOLOGICAL TRIANGLE

**Primary source**: Session 034; ADR-FOUNDATION-020/021

Three irreducible operations underlie ALL knowledge:

```
ASSERTION (monadic)       -- Reality -> Store(Datom)     -- how knowledge enters
FALSIFICATION (comonadic) -- Store(H) -> (H, {not_H_i}) -- how knowledge is tested
WITNESS (constructive)    -- not_not_H -> H              -- how survival becomes knowledge
```

Every datom exists in triple context: how it entered, how it could be destroyed, why it survived.

```
OPINION (0.0) -> HYPOTHESIS (0.15) -> TESTED (0.4) -> SURVIVED (0.7) -> KNOWLEDGE (1.0)
```

**Anti-Goodhart by construction**: F(S) rewards survived falsification, not accumulated confirmation. You CANNOT inflate F(S) by adding claims — only by deepening the dialectic.

**The SAW-TOOTH INVARIANT**: Healthy F(S) oscillates between surprise (dips from falsification) and consolidation (recovery to higher peaks). Monotonic increase = echo chamber = overfitting. The system HUNGERS for surprise because falsification is the only path to higher depth.

**The comonadic `duplicate` IS the dialectic**: H -> not_H -> not_not_H -> ... converges to fixed point. The challenge protocol (6 levels) = first 6 levels of comonadic duplicate. `braid challenge` = comonadic duplicate made executable.

---

## 9. THE MARKOV BLANKET

**Primary source**: Sessions 038, 042

The CLI is the Markov blanket separating two coupled systems:

```
Q(s) = the store         -- internal model, accumulated epistemology
P(o,s) = the project     -- external reality generating observations
```

| Direction | States | Examples |
|---|---|---|
| **Sensory** (in) | Observations from reality | observe, harvest, trace |
| **Active** (out) | Steering vectors to agent | go, seed, guidance, status |

**The DOGFOOD finding** (Session 042): The blanket was unidirectional — agent -> braid only. Braid's steering (active states) didn't flow back meaningfully. Scores: 6.75/10. The bidirectional flow is the whole product.

**Connection to active inference** (Friston): F(S) IS 1 - normalized_free_energy. The acquisition function alpha = E[delta_F(S)]/cost IS transformer attention at knowledge scale. The daemon IS the introspective observer — it observes the blanket itself (Type 9 reflexive divergence detection).

---

## 10. THE THREE PRIMITIVES

**Primary source**: Session 034; memory: `bedrock-vision.md`

The entire architecture reduces to three primitives:

| Primitive | What it does | Instances |
|---|---|---|
| **Morphisms** | Move knowledge between reality and store | Extractors (in), projectors (out), harvest, seed, merge, init |
| **Reconciliation** | Compare model to reality | 8-type taxonomy, boundary evaluation, F(S), bilateral scan |
| **Acquisition function** | Select optimal next morphism | R(t), observer selection, seed assembly, guidance, FEGH |

Everything else — extractors, projectors, boundaries, fitness, calibration, guidance, R(t), topology, witnesses, challenge protocol, three learning loops — is a specific instance of one of these three.

---

## 11. THE THREE LEARNING LOOPS

| Loop | What improves | Mechanism | Scientific analogy |
|---|---|---|---|
| **OBSERVER-4** (calibration) | Better hypotheses | predicted vs actual delta_F(S) -> weight adjustment | Instrument reliability |
| **OBSERVER-5** (structure) | Better reconciliation | temporal coupling -> proposed boundaries | Discovering new phenomena |
| **OBSERVER-6** (ontology) | Better morphisms | observation clustering -> concept entities | Paradigm formation |

Together: the system learns **what coherence means** (weights), **what should be aligned** (structure), and **what kinds of knowledge exist** (ontology).

---

## 12. THE UNIFYING PICTURE

```
                    THE LLM'S KNOWLEDGE MANIFOLD
                    (vast, pre-existing, latent)
                              |
                    +---------+---------+
                    |   MARKOV BLANKET   |
                    |   (braid CLI)      |
                    |                    |
              IN <--+  observe/harvest   +---> OUT
           (sensory)|                    |(active = steering)
                    |  seed/status/      |
                    |  guidance/go       |
                    +---------+---------+
                              |
                    +---------+---------+
                    |    DATOM STORE     |
                    |   (trajectory +   |
                    |    calibration)    |
                    |                    |
                    |  a = E[dF(S)]/cost |  <-- acquisition function
                    |  F(S) = alignment  |  <-- fitness (Lyapunov)
                    |  8 divergence types|  <-- reconciliation
                    +-------------------+
                              |
                         Y-COMBINATOR
                    seed -> session -> harvest
                         -> store -> seed
                    (fixed point = convergence)
```

**In one sentence**: Braid is a fixed-point combinator that gives stateless LLMs self-reference by maintaining a trajectory through the LLM's own knowledge manifold, steering it toward coherence via an acquisition function that minimizes free energy across 8 divergence types, with three learning loops that improve the steering process itself.

---

## 13. THE DEEP CONNECTION TO PROMPT OPTIMIZATION

The prompt-optimization skill (v5.2.0) says: *"A prompt is a field configuration over the model's activation manifold."*

The steering insight says: *"Every string braid emits is a steering event on the LLM's latent space manifold."*

**These are the same claim.** Braid IS a prompt optimization engine at organizational knowledge scale. Every `braid status`, every `braid observe` response, every `braid seed` is a prompt — a field configuration that activates specific regions of the LLM's knowledge, suppresses others, and creates attentional gradients toward productive exploration.

| Prompt optimization concept | Braid analog |
|---|---|
| **k\*** (overprompting threshold) | Seed budget system (INV-SEED-002) |
| **Demonstrations over constraints** | Concept-structured status vs. flat metadata |
| **Trajectory management** (seed turns) | `braid seed` establishing cognitive basin at session start |
| **Mid-DoF saddle** | Unidirectional Markov blanket (DOGFOOD failure) |
| **Field configuration** | Every CLI response as a steering vector |
| **Basin trapping** | Agent stuck in journal-mode because status gave flat metadata |
| **Constraint removal test** | The acquisition function: does this action increase F(S)? |

The entire prompt-optimization theory — field configurations, activation manifolds, DoF calibration, trajectory dynamics — is what braid formalizes and automates. The prompt-optimization skill is the manual, human-driven version of what braid does computationally.

---

## 14. THE API AS FORMAL LANGUAGE (The Complete Scope)

The steering principle extends far beyond CLI outputs. **Every token that crosses the
Markov blanket — in either direction — is a steering event.** This includes:

| Surface | Steering effect |
|---|---|
| CLAUDE.md | First steering event. Sets the session-start trajectory basin. |
| Command names | Encode the ontology. `observe` activates different knowledge than `record`. |
| Flag names | `--confidence` primes probabilistic reasoning. `--sure` would not. |
| Command grammar | `braid task list` vs `braid observe --list` teaches inconsistent patterns. |
| Help text | Teaches the grammar by example. Inconsistent examples teach inconsistent grammar. |
| Schema attributes | `:task/title`, `:observation/content` — naming IS the conceptual vocabulary. |
| Error messages | Most powerful steering moments — the LLM's model is wrong and maximally receptive. |
| Output structure | Teaches what information the system considers important and in what order. |
| Output absence | Silence teaches "nothing interesting happened" — which may be false. |
| Spec element IDs | `INV-STORE-001` teaches a naming pattern that enables prediction of unseen IDs. |
| Task descriptions | Vocabulary in task text becomes the agent's working vocabulary for the session. |
| Cross-command patterns | Consistency enables generalization. Inconsistency forces memorization. |

### Braid is Not a Tool. Braid is a Language.

A tool has a fixed interface. A language has grammar, vocabulary, semantics, and pragmatics.
The quality of thought is bounded by the quality of the language.

When the CLI grammar is consistent, the LLM can **think in braid** — reason using braid's
concepts as native vocabulary. When the grammar is inconsistent, the LLM must **translate
to braid** — reason in its own terms and then translate into commands, losing information
at the boundary.

"Thinking in braid" is the convergence state. "Translating to braid" is the divergent state.
The gap between them is epistemic divergence (Type 1 in our own taxonomy). The CLI's job is
to minimize that gap — to be so consistent that the LLM's in-context model converges to
braid's actual structure within the first 3-5 interactions.

### The API is an In-Context Curriculum

The LLM is doing in-context learning every time it interacts with braid. The API responses
are the training examples. The command grammar is the underlying distribution.

- **Clean distribution** (consistent grammar, coherent vocabulary, predictable structure)
  -> the in-context model converges quickly and accurately
- **Noisy distribution** (inconsistent patterns, mixed vocabularies, unpredictable structure)
  -> the model converges slowly or not at all

By the 10th command in a session, the LLM's model of braid is primarily determined by the
previous 9 interactions, not by CLAUDE.md. This is the trajectory dynamic from prompt
optimization: prior outputs reshape the field more than the current prompt. If those 9
interactions were coherent, the LLM has a sharp model and its 10th command is well-chosen.
If incoherent, the model is fuzzy and the 10th command is a guess.

### The Cardinality Equation for CLI Design

From formal Rust engineering: `|YourType| = |ValidStates|`. Applied to CLI grammar:
`|GrammarSpace| = |ValidCommands|`.

Every command the agent predicts should exist (because the grammar implies it) but doesn't
is the equivalent of an invalid state — a prediction failure that wastes a turn and erodes
the in-context model. If `braid task list` and `braid task show <id>` work, the LLM will
predict `braid observation list` and `braid observation show <id>`. If these don't exist
because it's actually `braid observe --list`, that's excess cardinality in the LLM's
grammar model — parasitic constraints that degrade reasoning on every subsequent turn.

### CLI Coherence Invariants

These are falsifiable — each can be verified by test:

**INV-CLI-GRAMMAR**: All commands follow one structural pattern. The LLM can predict any
command from the pattern without consulting help.

**INV-CLI-VOCABULARY**: The same concept uses the same word everywhere — output, help text,
error messages, schema attributes, task descriptions. Never "observation" in one place and
"finding" in another.

**INV-CLI-OUTPUT-SHAPE**: All list commands return the same structure. All detail commands
return the same structure. The LLM learns the shape once, applies it everywhere.

**INV-CLI-ERROR-STEERING**: Every error message teaches the correct action. Not just what
went wrong — what right looks like. Errors are the system's most powerful teaching moments
because they occur exactly when the LLM's internal model is wrong and most receptive to
correction.

**INV-CLI-RESPONSE-STEERING**: Every response includes at minimum: (1) what happened,
(2) what changed in the model, (3) what to do next. A bare acknowledgment violates this.

**INV-CLI-PROGRESSIVE**: Complexity is revealed gradually. Default output is optimized for
the most common next action. Verbose/detailed output is opt-in. This matches the Piagetian
developmental trajectory: simple schemas first, refinements through accommodation.

### Why This Matters: The DOGFOOD Evidence

DOGFOOD scored 6.75/10. The agents used braid as a write-only journal. Not because braid
lacked features — because its surfaces didn't speak a learnable language. The Markov blanket
was unidirectional. The agents couldn't learn to think in braid because braid's grammar was
inconsistent, its responses were receipts not steering vectors, and its vocabulary didn't
match across commands.

The fix is not better features. It is **grammatical, semantic, structural, and pedagogical
coherence across every surface** — so that an LLM agent's in-context model of braid
converges to braid's actual structure within the first few interactions, and every
subsequent interaction steers the agent toward the highest-value next action.

---

## SPEC ELEMENTS REFERENCED

| ID | Name |
|---|---|
| ADR-FOUNDATION-012 | Braid as Epistemology Runtime |
| ADR-FOUNDATION-013 | Declarative Policy Manifest |
| ADR-FOUNDATION-014 | The Convergence Thesis |
| ADR-FOUNDATION-015 | The Observer Formalism |
| ADR-FOUNDATION-016 | The Observation-Projection Duality |
| ADR-FOUNDATION-017 | The Hypothetico-Deductive Loop |
| ADR-FOUNDATION-018 | The Hypothesis Ledger |
| ADR-FOUNDATION-019 | The Universal Action Queue |
| ADR-FOUNDATION-020 | The Falsification-First Principle + Dialectical Comonad |
| ADR-FOUNDATION-021 | The Anti-Goodhart Architecture |
| ADR-FOUNDATION-025/028 | The Universal Acquisition Function |
| ADR-FOUNDATION-029 | Connections as Datoms |
| ADR-FOUNDATION-030 | The Free Energy Gradient (FEGH) |
| ADR-FOUNDATION-031 | The Introspective Observer (daemon) |
| ADR-FOUNDATION-032 | Ontological Surprise (unknown unknowns) |
| ADR-FOUNDATION-033 | Load-Bearing Failure Detection |
| ADR-FOUNDATION-034 | Concept Crystallization (ontology discovery) |
| INV-FOUNDATION-006 | Kernel Methodology Agnosticism |
| INV-FOUNDATION-007-015 | Policy, Versioning, Vision Coherence, Composition, Alpha, Universality, Bridges, Self-Calibration, Type 9 |
| NEG-FOUNDATION-003/004/005 | No DDIS assumption, No software-tool framing, No open loops |

## PRIMARY SESSIONS

| Session | ID | Key contribution |
|---|---|---|
| 033 | `c01bb082` | Convergence thesis, 8 divergence types, bedrock vision |
| 034 | `499cba26` | Complete formalism: observer, duality, loop, triangle, 3 primitives |
| 036 | `92160d00` | UAQ, acquisition function, FEGH, free energy gradient |
| 038 | `d9ac2e6b` | Y-combinator, 7-level reflexive hierarchy, Type 9 |
| 042 | `56ece7bd` | Piaget, steering vectors, concept crystallization, Markov blanket, innate schemas |

---

*This document is a distillation of the project's theoretical foundations, traced to primary
sources in session logs and memory files. It should be read alongside `bedrock-vision.md`
(the operational version) and `SEED.md` (the specification). Together they form the complete
intellectual foundation of the braid project.*
