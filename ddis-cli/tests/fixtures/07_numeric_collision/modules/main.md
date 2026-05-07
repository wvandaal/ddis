---
module: main
domain: main
maintains: [APP-INV-001]
interfaces: [CMP-INV-001]
implements: []
adjacent: []
negative_specs: []
---

# Main Module

References both APP-INV-001 (own) and CMP-INV-001 (cross-namespace).

## Invariants

**APP-INV-001: App-side Invariant 001**

*The app-side invariant is distinct from CMP-INV-001 even though they share the numeric suffix.*

```
predicate_app(s)
```

Violation scenario: parser conflates APP-INV-001 and CMP-INV-001 into a single row.

Validation: assert two distinct invariants_id rows: APP-INV-001 and CMP-INV-001.

---
