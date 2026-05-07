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

This module's prose mentions external paths inside backticks. Those
mentions are NOT cross-references; they are example strings.

## Invariants

**APP-INV-001: Inline-code Tolerance**

*Cross-reference extraction must skip text inside inline code spans —
references like `docs/external.md §99.99` or `XYZ-INV-555` are prose
about external systems, not resolvable refs in this spec.*

```
predicate(s)
```

Violation scenario: parser greedily extracts every regex match across
all backticked strings, producing false-positive unresolved references
that confuse downstream tooling.

Validation: assert that this fixture parses with zero unresolved
cross-references despite containing `§99.99` and `XYZ-INV-555` inside
inline code spans on this very line.

---
