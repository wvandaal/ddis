---
module: main
domain: main
maintains: [INV-001]
interfaces: []
implements: []
adjacent: []
negative_specs: []
---

# Main Module

## Invariants

**INV-001: Back-Compat No Prefix**

*Bare INV-001 with no namespace prefix.*

```
predicate(s)
```

Violation scenario: parser rejects bare INV-001 because it expects APP- prefix.

Validation: assert INV-001 lands in invariants table (legacy back-compat).

---
