---
module: main
domain: main
maintains: [APP-INV-001]
interfaces: [CMP-INV-010]
implements: [APP-ADR-001]
adjacent: [CMP-ADR-005]
negative_specs: []
---

# Main Module

This module references the CMP namespace at CMP-INV-010 and CMP-ADR-005.

## Invariants

**APP-INV-001: App Invariant Referencing CMP**

*The app-side invariant aligns with CMP-INV-010 from the CMP namespace.*

```
FOR ALL s IN app_states:
  app_predicate(s) AND consistent_with(CMP-INV-010)
```

Violation scenario: app state contradicts CMP-INV-010 expectations.

Validation: cross-check against CMP-INV-010 oracle.

---

## ADRs

### APP-ADR-001: App Decision

#### Problem

App design must align with CMP-ADR-005.

#### Decision

**Option A: align.** Adopt the CMP-ADR-005 architecture (also satisfies APP-INV-001).

#### Consequences

CMP-INV-010 is required by APP-INV-001 transitively.
