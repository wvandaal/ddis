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

**APP-INV-001: Italic-Spans-Multiple-Lines**

*The invariant statement intentionally spans multiple lines to exercise
parser tolerance for prose that does not fit on a single trimmed line —
authors writing fuller normative statements naturally break across lines.*

```
FOR ALL s: predicate(s)
```

Violation scenario: parser fails to recognize the italic statement when it
spans more than one line.

Validation: assert APP-INV-001 lands with statement substring "spans multiple lines".

---
