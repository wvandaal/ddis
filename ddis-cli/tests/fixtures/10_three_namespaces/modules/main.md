---
module: main
domain: main
maintains: [APP-INV-001]
interfaces: [CMP-INV-010, DOM-INV-100]
implements: []
adjacent: []
negative_specs: []
---

# Main Module

References both CMP-INV-010 and DOM-INV-100.

## Invariants

**APP-INV-001: Three-way Coordinator**

*The APP invariant coordinates CMP-INV-010 and DOM-INV-100 via a single contract.*

```
predicate(s) AND consistent_with(CMP-INV-010) AND consistent_with(DOM-INV-100)
```

Violation scenario: parser drops one or both of the cross-namespace references.

Validation: assert all three of APP-INV-001, CMP-INV-010, DOM-INV-100 land as invariant rows.

---
