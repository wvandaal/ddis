# DDIS Project — Agent Guidelines

## Drift Management

Before modifying DDIS specs, run `ddis drift` to understand the current state.

**The one rule: drift must not increase after any edit.**

```bash
# Measure current drift
ddis drift index.db

# After every spec edit
ddis parse manifest.yaml -o index.db && ddis drift index.db

# Full report
ddis drift index.db --report --json
```

For the full workflow: `ms load ddis-drift-workflow -m`

## Spec Locations

- **Meta-spec (DDIS standard)**: `ddis-modular/manifest.yaml`
- **CLI spec**: `ddis-cli-spec/manifest.yaml` (parent_spec: ddis-modular)
- **CLI binary**: `ddis-cli/bin/ddis`

## Quality Gates

Before committing spec changes:

```bash
ddis validate index.db --json    # All checks should pass
ddis drift index.db --report     # Drift should be 0 or decreasing
ddis coverage index.db           # Component completeness
```
