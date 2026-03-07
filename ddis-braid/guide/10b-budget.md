# §10b. BUDGET — Build Plan (Stage 1)

> **Spec reference**: [spec/13-budget.md](../spec/13-budget.md)
> **Stage 1 elements**: INV-BUDGET-001-006, ADR-BUDGET-001-004, NEG-BUDGET-001-002
> **Dependencies**: STORE, SCHEMA, QUERY, GUIDANCE
> **Cognitive mode**: Information-theoretic — rate-distortion, capacity constraints
> **Stage 0 prep**: TokenCounter trait + ApproxTokenCounter (guide/00-architecture.md §0.6)

---

## §10b.1 Stage 0 Foundation

The `TokenCounter` trait and `ApproxTokenCounter` implementation are defined in
guide/00-architecture.md §0.6 and built at Stage 0. These provide the measurement
substrate that the full BUDGET namespace consumes at Stage 1.

ADR-BUDGET-004 (Tokenization via Chars/4 Approximation) is the only Stage 0 ADR in
this namespace. It specifies the `TokenCounter` trait contract and the zero-dependency
`ApproxTokenCounter` that Stage 1 will eventually replace with tiktoken-rs cl100k_base.

## §10b.2 Stage 1 Scope

Full build plan to be expanded before Stage 1 implementation. Will cover:

- `BudgetManager` struct with `measure`, `allocate`, `project` transitions
- `OutputPrecedence` five-level enum (Ambient < Speculative < UserRequested < Methodology < System)
- Precedence-ordered truncation pipeline (INV-BUDGET-002)
- Quality-adjusted degradation via piecewise `attention_decay` (INV-BUDGET-003)
- Guidance compression by budget level (INV-BUDGET-004)
- Command attention profiles: CHEAP/MODERATE/EXPENSIVE/META (INV-BUDGET-005)
- Token efficiency density monotonicity (INV-BUDGET-006)
- Projection pyramid: pi_0 (full) through pi_3 (single-line) level selection

---
