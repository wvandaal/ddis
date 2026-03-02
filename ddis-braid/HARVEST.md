# HARVEST.md — Session Log

> This file is the manual harvest/seed mechanism. Every session appends an entry.
> Read the latest entry at session start (your "seed"). Write a new entry at session end (your "harvest").
> When the datom store exists, this file becomes unnecessary — the harvest/seed cycle is automated.

---

## Session 001 — 2026-03-01/02 (Pre-Braid: Design Foundation)

**Platform**: Claude.ai (multi-session conversation)
**Duration**: ~7 design sessions across several hours

### What Was Accomplished

- Produced `SEED.md` — the 11-section foundational design document covering:
  - Divergence as the fundamental problem (not just AI memory)
  - Specification formalism (invariants, ADRs, negative cases, uncertainty markers)
  - Datom abstraction with 5 algebraic axioms
  - Harvest/seed lifecycle
  - Reconciliation taxonomy (8 divergence types mapped to detection/resolution mechanisms)
  - Self-improvement loop (graph densification, adaptive instructions, retrieval sharpening)
  - Interface principles (budget-aware output, guidance injection, five layers)
  - Staged roadmap (Stage 0–4)
  - Design rationale (7 "why" entries including self-bootstrap)

- Produced `CLAUDE.md` — LLM-optimized operating instructions for all braid sessions

- Produced `onboarding.md` — comprehensive guide to the existing DDIS Go CLI

- Established the self-bootstrap commitment: DDIS specifies itself using DDIS methodology

### Decisions Made

| Decision | Rationale |
|---|---|
| Braid is a new implementation, not a patch of ddis-cli | The specification has diverged enough from the existing Go implementation that adaptation would be more costly than rebuild on clean foundations |
| DDIS specifies itself | Integrity (can't spec coherence system incoherently), bootstrapping (spec elements are first data), validation (if DDIS can't spec DDIS, it can't spec anything) |
| Manual harvest/seed before tools exist | Methodology precedes tooling; tools automate established practice |
| Reconciliation mechanisms are a unified taxonomy | All protocol operations are instances of: detect divergence → classify → resolve to coherence |
| Uncertainty markers are first-class | Prevents aspirational prose from being implemented as axioms |

### Open Questions

1. **Implementation language**: SEED.md says "existing Rust implementation" but the current CLI is Go. Decision needed: Rust (as originally designed) or Go (for continuity)?
2. **Section 9 of SEED.md is incomplete**: Needs the codebase description filled in by Willem
3. **Datom serialization format**: Not yet specified. JSONL? Protobuf? Custom binary?
4. **SQLite vs. custom storage**: The existing CLI uses SQLite extensively. Does braid?
5. **Temporal decay of facts**: Discussed but not formalized. λ parameter per attribute namespace.

### Recommended Next Action

**Produce SPEC.md** — the DDIS-structured specification. Work through SEED.md section by section,
extracting every implicit claim into formal invariants, ADRs, and negative cases. This is Step 2
in the concrete roadmap (SEED.md §10). Estimated: 2–4 hours across multiple Claude Code sessions.

