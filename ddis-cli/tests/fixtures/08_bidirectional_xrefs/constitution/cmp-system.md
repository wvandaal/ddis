---
module: cmp-constitution
domain: cmp
tier: 1
ddis_version: "3.0"
namespace: CMP
---

# CMP Namespace

## CMP Invariants

**CMP-INV-010: CMP↔APP Bidirectional**

*The CMP invariant references APP-INV-001 from the APP namespace.*

Violation scenario: CMP→APP back-reference fails to resolve.

Validation: assert APP-INV-001 cross-ref from CMP-INV-010 is resolved.

---
