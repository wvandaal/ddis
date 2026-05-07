---
module: cmp-constitution
domain: cmp
tier: 1
ddis_version: "3.0"
namespace: CMP
---

# CMP Namespace

## CMP Invariants

**CMP-INV-001: Component-side Invariant 001**

*Distinct invariant from APP-INV-001; same number, different namespace.*

Violation scenario: cross-ref to CMP-INV-001 silently resolves against APP-INV-001.

Validation: assert both APP-INV-001 and CMP-INV-001 exist as separate rows.

---
