---
module: main
domain: main
maintains: [APP-INV-001]
interfaces: [CMP-INV-010]
implements: []
adjacent: []
negative_specs: []
---

# Main Module

## Invariants

**APP-INV-001: APP↔CMP Bidirectional**

*Forward reference to CMP-INV-010; backward reference established by CMP-INV-010.*

```
predicate_app(s) AND consistent_with(CMP-INV-010)
```

Violation scenario: only one direction of the bidirectional ref resolves.

Validation: both APP-INV-001 → CMP-INV-010 and CMP-INV-010 → APP-INV-001 must resolve.

---
