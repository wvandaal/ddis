---
module: main
domain: main
maintains: [APP-INV-001]
interfaces: []
implements: [APP-ADR-001]
adjacent: []
negative_specs: []
---

# Main Module

This module references APP-INV-001 and APP-ADR-001.

## Invariants

**APP-INV-001: Baseline Invariant**

*The baseline invariant must hold for every state.*

```
FOR ALL s IN states: predicate(s)
```

Violation scenario: predicate fails for some state.

Validation: enumerate states and check predicate.

---

## ADRs

### APP-ADR-001: Baseline Decision

#### Problem

We need a baseline.

#### Options

A) **Option A**
- Pros: simple
- Cons: limited

B) **Option B**
- Pros: flexible
- Cons: complex

#### Decision

**Option A: simple wins.** Chosen for clarity (APP-INV-001).

#### Consequences

Simple system.

#### Tests

Run conformance suite.
