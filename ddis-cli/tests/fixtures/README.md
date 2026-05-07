# Conformance Fixture Suite

Self-contained fixtures pinning expected parser behavior across the cross-reference + namespace + format-tolerance dimensions.

Each fixture is a directory containing a complete modular spec:

```
NN_short_name/
  manifest.yaml               # required — entry point
  constitution/system.md      # required — registry + ADRs
  modules/main.md             # required — invariant/ADR definitions
  modules/<more>.md           # optional — additional modules
  baseline.yaml               # required — expected outcomes (see schema below)
```

## Baseline schema

```yaml
description: "Human-readable purpose"
expect:
  parse_error_substring: null   # or string: parsing must fail and error must contain this
  invariants:
    count: 1
    ids: [APP-INV-001]          # exact set, unordered
  adrs:
    count: 1
    ids: [APP-ADR-001]
  cross_refs:
    resolved: 2
    unresolved: 0
    targets: [APP-INV-001, APP-ADR-001]   # set comparison over RefTarget column
  validation:
    errors: 0                   # max errors allowed
    error_must_contain: []      # list of substrings each appearing in some Finding.Message
```

Any field omitted is not asserted. The harness in `tests/namespace_conformance_test.go` enforces these contracts.

## Adding a fixture

1. Create `tests/fixtures/NN_descriptive_name/` with the four required files.
2. Run `go test ./tests -run TestNamespaceConformance` — the new fixture is auto-discovered.
3. If the fixture must currently FAIL (e.g., a behavior not yet implemented), add `// FIXME: pending phase X` to the description so the regression baseline is intentional.

## Current fixtures

| # | Name | Pins behavior |
|---|------|---------------|
| 01 | baseline_app_only | Pure APP namespace, single invariant + ADR — control case |
| 02 | multi_namespace_basic | APP + CMP, both resolve, no collisions — the core motivating case |
| 03 | header_owner_suffix | `**APP-INV-NNN: Title** (Owner: x)` on module definitions parses |
| 04 | multiline_italic | `*statement spanning ... multiple lines*` parses as one statement |
| 05 | field_synonyms | `Violation:` ≡ `Violation scenario:`; `Validation strategy:` ≡ `Validation:` |
| 06 | unknown_namespace | `XYZ-INV-001` cross-ref fails with namespace-aware error |
| 07 | numeric_collision | APP-INV-001 and CMP-INV-001 coexist; refs route to correct namespace |
| 08 | bidirectional_xrefs | APP module references CMP, CMP module references APP |
| 09 | back_compat_no_namespace | Bare `INV-001` defaults to APP for back-compat |
| 10 | three_namespaces | APP + CMP + DOM; three-way registry coordination |
