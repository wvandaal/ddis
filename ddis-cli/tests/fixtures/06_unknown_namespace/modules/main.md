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

References XYZ-INV-001 from an unregistered namespace.

## Invariants

**APP-INV-001: References Unknown Namespace**

*This invariant references XYZ-INV-001, where namespace XYZ is not declared anywhere.*

```
FOR ALL s: predicate(s) AND consistent_with(XYZ-INV-001)
```

Violation scenario: parser silently strips the XYZ- prefix and resolves against APP-INV-001.

Validation: assert that the validation report contains an error mentioning the XYZ namespace explicitly.

---
