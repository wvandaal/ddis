---
module: main
domain: main
maintains: [APP-INV-001]
interfaces: []
implements: []
adjacent: []
negative_specs: []
---

# Main Module

## Invariants

**APP-INV-001: Suffix-Tolerant Invariant** (Owner: main)

*The module-side invariant definition carries an owner suffix, mirroring the registry convention.*

```
FOR ALL s: predicate(s)
```

Violation scenario: parser drops the invariant because of the trailing `(Owner: main)` suffix.

Validation: assert that APP-INV-001 lands in the invariants table.

---
