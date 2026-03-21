> **DEPRECATED**: This file is bootstrap scaffolding. The canonical source of truth is the braid datom store. Use `braid spec show` and `braid query` to access spec elements. See ADR-STORE-019.

---

# §22 Reflexive Divergence

> **Purpose**: Formalize the ninth divergence type — the gap between the system's model
> of itself and its actual state. A self-bootstrapping system (C7) that manages its own
> specification is itself a changing entity. Reflexive coherence requires
> `M(S) ≅ S`: the store's model of the system must be isomorphic (up to relevant
> properties) to the system's actual state.
>
> **Traces to**: SEED.md §6 (Reconciliation Framework), §7 (Self-Improvement Loop),
> §8 (Interface Principles), C7 (Self-Bootstrap)

---

## §22.1 Motivation

The reconciliation taxonomy (SEED.md §6) defines eight divergence types covering
agent-store, spec-impl, agent-agent, and other boundaries. None covers **system ↔ self**.

Five empirically observed friction points (Session 025 dog-food assessment) share a
common root: the system lacks a formal model of its own observation apparatus:

| Friction | What the system can't observe about itself |
|----------|-------------------------------------------|
| Capability detection false negatives | Its own compiled binary capabilities |
| k_eff estimation inaccuracy | The agent's context consumption state |
| Crystallization false positives | The precise human ↔ machine identifier bijection |
| Tag collision in self-referential content | The distinction between using and mentioning its own syntax |
| Disconnected dynamic sections | The reader's cognitive model of the document |

Each is an instance of reflexive divergence: the store contains a model `M(S)` that
diverges from the system's actual state `S`.

**Ninth divergence type**:

| Property | Value |
|----------|-------|
| **Type** | Reflexive |
| **Boundary** | System's model of itself ↔ System's actual state |
| **Detection** | Capability census, structural self-test, naming consistency check |
| **Resolution** | Self-assertion datoms, formal quoting, declared document architecture |

---

## §22.2 Invariants

### INV-REFLEXIVE-001: Capability Census Completeness

**Traces to**: C7 (Self-Bootstrap), C3 (Schema-as-Data)
**Type**: Invariant
**Stage**: 1
**Statement**: At system initialization (session start or `braid init`), for every
subsystem `S_i` compiled into the binary, the store contains a datom:

```
[:capability/S_i :capability/status V_i :capability/version version_string]
where V_i ∈ {:implemented, :spec-only, :partial}
```

The census function `census: C_binary → Vec<CensusResult>` is total — it covers
every subsystem. `capability_scan(store)` queries these datoms instead of probing
for attribute prefixes that may not exist.

**Algebraic property**: Let `C_binary` = the set of capabilities compiled into the
binary. The census function is a faithful functor: `|census(C_binary)| = |C_binary|`
and the status assignment preserves the implementation state.

**Falsification**: A subsystem is compiled into the binary and functional (its code
executes correctly) but the store contains no `:capability/*` datom for it, OR
contains `:spec-only` when the subsystem's diagnostic probe returns success.
**Verification**: V:PROP

---

### INV-REFLEXIVE-002: k_eff Estimation Fidelity

**Traces to**: SEED.md §8 (Interface Principles), INV-BUDGET-004
**Type**: Invariant
**Stage**: 1
**Statement**: The system's estimate of k_eff at time t satisfies:

```
|k̂_eff(t) - k_true(t)| ≤ 0.15 with probability ≥ 0.9
```

where `k_true` is the actual context budget remaining (measured via `--context-used`).

The estimator uses multiple observable signals:

```
evidence = (tx_count_since_session, wall_elapsed, tx_velocity, output_estimate, observe_count)
k̂_eff = 1.0 - Σ(wᵢ × sigmoid(eᵢ / τᵢ))
```

Weights `wᵢ` and thresholds `τᵢ` are calibrated from the store's own session history
when `--context-used` calibration data is available.

**Algebraic property**: The estimator is a contraction mapping — successive
calibration cycles reduce estimation error monotonically until convergence.

**Falsification**: The system estimates k_eff = 0.8 when the actual remaining
context is 0.3 (or vice versa), and no calibration data is recorded.
**Verification**: V:PROP

---

### INV-REFLEXIVE-003: Spec ID Bijection

**Traces to**: C5 (Traceability), INV-STORE-003 (Content-Addressable Identity)
**Type**: Invariant
**Stage**: 1
**Statement**: There exists a total, injective function pair:

```
normalize : HumanSpecId → StoreIdent
denormalize : StoreIdent → HumanSpecId

such that:
  normalize("INV-GUIDANCE-022") = ":spec/inv-guidance-022"
  denormalize(":spec/inv-guidance-022") = "INV-GUIDANCE-022"

Left-inverse law: denormalize(normalize(h)) = h for all valid h
Uniqueness: normalize(h₁) = normalize(h₂) → h₁ = h₂ (injectivity)
```

Both functions are implemented as a single `SpecId` type used at EVERY boundary
where spec IDs cross between human-readable and machine-readable form. No call
site performs its own ad-hoc normalization.

**Algebraic property**: `normalize` is a homomorphism from the free monoid of
human-readable spec IDs (under concatenation) to the store ident space (under
keyword construction). The quotient `HumanSpecId / ~` where `h₁ ~ h₂ iff
normalize(h₁) = normalize(h₂)` is in bijection with `StoreIdent`.

**Falsification**: Two different human-readable spec IDs normalize to the same
store ident, OR a valid store ident fails to denormalize, OR any code path
performs spec ID normalization without using the `SpecId` type.
**Verification**: V:PROP (proptest over generated spec IDs)

---

### INV-REFLEXIVE-004: Injection Tag Containment Safety

**Traces to**: C7 (Self-Bootstrap), SEED.md §8 (Interface Principles)
**Type**: Invariant
**Stage**: 1
**Statement**: For any injection tag type `T ∈ {seed, methodology, witness, ...}`:

```
Content INSIDE a <braid-T> section is in "mention mode."
Tag-like strings within are data, not structure.

Formally: Let R(T) = byte range of <braid-T>...</braid-T> in the document.
For any tag type U:
  find_tagged_section(text, U) searches ONLY in:
    text \ ∪{R(T) : T ≠ U} \ code_blocks

The exclusion is SYMMETRIC and UNIVERSAL:
  ∀ T, U where T ≠ U: content(T) ∩ structure(U) = ∅
```

Adding a new tag type `<braid-X>` requires calling `find_tagged_section(text, "X")`
with ZERO changes to the injection engine.

**Algebraic property**: The set of braid tag names forms a free monoid under
string concatenation. Each tag name defines a quoted context. The containment
rule is a universal property: it holds for all tag types, present and future,
without per-type special cases.

**Falsification**: Adding a new tag type `<braid-witness>` requires modifying
existing `find_*_point()` functions (violates open-closed principle), OR content
inside a `<braid-seed>` section containing the literal string `<braid-methodology>`
is matched as a real methodology tag.
**Verification**: V:PROP

---

### INV-REFLEXIVE-005: Dynamic Section Self-Documentation

**Traces to**: SEED.md §8 (Interface Principles), INV-INTERFACE-008
**Type**: Invariant
**Stage**: 1
**Statement**: Every auto-generated section in AGENTS.md is preceded by a
contextual header that explains:
  (a) what the section contains
  (b) that it is auto-generated
  (c) how it relates to other dynamic sections

```
## Live Methodology (auto-generated: HOW to work)
<braid-methodology>
...
</braid-methodology>

## Dynamic Store Context (auto-generated: WHAT to work on)
<braid-seed>
...
</braid-seed>
```

The injection engine generates these headers from the document architecture,
not hardcoded strings. The relationship description (HOW vs WHAT) is part of
the rendered output.

**Falsification**: After `braid seed --inject AGENTS.md`, a dynamic section
exists without a preceding contextual header, OR the header does not explain
the section's purpose and relationship to other sections.
**Verification**: V:PROP

---

## §22.3 Architectural Decision Records

### ADR-REFLEXIVE-001: Reflexive Divergence as Ninth Type

**Traces to**: SEED.md §6 (Reconciliation Framework), C7 (Self-Bootstrap)
**Type**: ADR
**Stage**: 1

**Problem**: The reconciliation taxonomy has 8 divergence types but none covers
the system's relationship to itself. A self-bootstrapping system manages its own
specification, making itself a changing entity. When the system's model of itself
diverges from its actual state, errors are silent and systematic.

**Options considered**:
1. **Extend existing types**: Map reflexive issues to existing categories (e.g.,
   capability detection = structural divergence, k_eff = epistemic divergence).
2. **New divergence type**: Create a dedicated ninth type with its own detection
   and resolution mechanisms.
3. **Ignore**: Treat self-knowledge as a non-concern (the system is a fixed point).

**Decision**: Option 2 — Reflexive Divergence as a first-class ninth type.

**Rationale**:
- Option 1 fails because the BOUNDARY is different. Structural divergence is
  spec ↔ impl. Reflexive divergence is system ↔ system's-model-of-self. The
  detection and resolution mechanisms are fundamentally different (capability
  census vs bilateral scan; self-assertion datoms vs guided reimplementation).
- Option 3 is falsified by empirical evidence: 5 dog-food friction points in
  Session 025 all traced to reflexive divergence.
- Option 2 follows the taxonomy's own principle: each divergence type has a
  characteristic boundary, detection, and resolution. Reflexive divergence
  has all three, distinct from existing types.

**Consequences**: The reconciliation taxonomy expands from 8 to 9 types. The
reflexive type becomes a design consideration for all future self-referential
features (dynamic AGENTS.md generation, capability reporting, attention estimation).

---

## §22.4 Negative Cases

### NEG-REFLEXIVE-001: No Silent Capability False Negatives

**Traces to**: INV-REFLEXIVE-001
**Type**: Negative Case
**Stage**: 1
**Statement**: The system MUST NOT report a compiled, functional subsystem as
"NOT YET IMPLEMENTED" or `:spec-only`. If a subsystem's diagnostic probe returns
success, the capability status MUST be `:implemented`.

**Violation scenario**: `.cache/` persistence exists and works (DiskLayout writes
and reads datoms.bin + meta.json), but `capability_scan` reports it as unimplemented
because no `:cache/*` attribute prefix exists in the store.

---

### NEG-REFLEXIVE-002: No Ad-Hoc Spec ID Normalization

**Traces to**: INV-REFLEXIVE-003
**Type**: Negative Case
**Stage**: 1
**Statement**: No code path MUST perform spec ID normalization (converting between
"INV-GUIDANCE-022" and ":spec/inv-guidance-022") outside the `SpecId` type. Ad-hoc
normalization in individual functions is a bug, even if it produces correct results,
because it creates multiple points of failure for the bijection.

**Violation scenario**: `crystallization_candidates` uses `id.to_uppercase()` while
`resolve_spec_refs` uses `id.to_lowercase()` — both ad-hoc, both correct in isolation,
but they define different equivalence relations that may diverge on edge cases.

---

### NEG-REFLEXIVE-003: No Per-Tag-Type Exclusion Functions

**Traces to**: INV-REFLEXIVE-004
**Type**: Negative Case
**Stage**: 1
**Statement**: The injection engine MUST NOT have separate find/inject functions
for each tag type that maintain independent exclusion logic. All tag types MUST
use the universal `find_tagged_section(text, tag_name)` function.

**Violation scenario**: Adding `<braid-witness>` requires creating a
`find_witness_point()` function with its own exclusion zones for seed and
methodology regions — O(n²) growth in tag count.
