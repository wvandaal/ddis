---
module: cmp-constitution
domain: cmp
tier: 1
ddis_version: "3.0"
namespace: CMP
---

# CMP Namespace

## CMP Invariants

**CMP-INV-010: Component Invariant**

*Component-layer invariant referenced by APP and DOM.*

Violation scenario: CMP-INV-010 fails to resolve from APP module.

Validation: assert APP→CMP cross-ref resolves.

---
