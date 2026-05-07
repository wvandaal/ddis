---
module: cmp-constitution
domain: cmp
tier: 1
ddis_version: "3.0"
namespace: CMP
---

# CMP Namespace Constitution

## CMP Invariant Registry

**CMP-INV-010: Component Isomorphism**

*Every component renders identically in both React and mdast contexts.*

Violation scenario: a component diverges between renderers.

Validation: render both and diff.

---

## CMP ADR Registry

### CMP-ADR-005: Renderer Architecture

#### Problem

Components must render in two contexts.

#### Decision

**Option A: dual renderer.** Maintain parity across both.

#### Consequences

Both renderers must stay in sync.
