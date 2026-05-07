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

**APP-INV-001: Synonym Field Prefixes**

*Field prefixes Violation and Validation strategy must be accepted as synonyms for the canonical Violation scenario and Validation forms.*

```
FOR ALL s: predicate(s)
```

Violation: parser ignores the field because the canonical prefix is "Violation scenario:".

Validation strategy: assert that violation_scenario and validation_method columns are populated.

---
