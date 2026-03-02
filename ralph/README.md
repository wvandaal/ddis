# DDIS RALPH Loop

**Recursive Autonomous LLM-driven Progressive Honing** of the DDIS specification standard.

## Setup

All RALPH files live in the `ralph/` directory:

```
ralph/
├── ddis_ralph_loop.sh              # Main RALPH loop script
├── ddis_assemble.sh                # Module assembler
├── ddis_validate.sh                # Validation checks
├── improvement_strategy.md         # The improvement methodology
├── kickoff_prompt.md               # Short kickoff for iteration 1
├── README.md                       # This file
└── judgments/                      # RALPH convergence evaluations
    ├── judgment_v1.json
    └── judgment_v2.json
```

The seed spec lives at `ddis-evolution/versions/ddis_v0.md` (the original DDIS 1.0).

Requirements: `claude` CLI (Claude Code), `jq`, and `python3` with PyYAML.

## Usage

```bash
# Basic run — monolith mode (defaults: 5 iterations, opus improver, sonnet judge)
./ddis_ralph_loop.sh

# Customize via environment variables
DDIS_MAX_ITERATIONS=3 DDIS_IMPROVER_MODEL=sonnet ./ddis_ralph_loop.sh

# Budget-conscious run
DDIS_IMPROVER_MODEL=sonnet DDIS_JUDGE_MODEL=haiku DDIS_MAX_ITERATIONS=3 ./ddis_ralph_loop.sh

# Modular mode — for specs decomposed via §0.13 Modularization Protocol
./ddis_ralph_loop.sh --modular --manifest path/to/manifest.yaml

# Modular mode — constitution only (Phase 1)
./ddis_ralph_loop.sh --manifest path/to/manifest.yaml --phase 1

# Modular mode — modules only (Phase 2), e.g. after a manual constitution edit
./ddis_ralph_loop.sh --manifest path/to/manifest.yaml --phase 2
```

## Auto-Modularization (Phase 0)

Before the improvement cycle begins, the RALPH loop automatically assesses whether a monolith spec should be decomposed into modules. This runs when:
- The spec is in monolith mode (no `--modular` or `--manifest` flag)
- `DDIS_AUTO_MODULARIZE` is not set to `false`

**How it works:**
1. **Structural Assessment**: An LLM evaluates the monolith against the §0.13.7 decision framework, considering both the spec's size AND its usage context (standalone vs. consumed as a reference alongside other work).
2. **Decomposition** (if warranted): The LLM executes the §0.13.14 Monolith-to-Module Migration Procedure — producing a manifest.yaml, constitution files, and module files.
3. **Validation**: `ddis_validate.sh` verifies the decomposition. If validation fails, one correction pass is attempted.
4. **Seamless Transition**: If decomposition succeeds, the loop automatically switches to modular mode. The original monolith is preserved as `ddis_v0.md`.

**Key insight**: The threshold for modularization isn't "is the spec too big to read?" — it's "does the spec leave enough room for the LLM to do useful work alongside it?" A spec consumed as a reference (like DDIS itself) needs modularization at a lower line count than a standalone spec.

```bash
# Auto-modularization is on by default. To disable:
DDIS_AUTO_MODULARIZE=false ./ddis_ralph_loop.sh

# To skip Phase 0 and use a pre-decomposed structure:
./ddis_ralph_loop.sh --manifest path/to/manifest.yaml
```

## How It Works

Each iteration runs two separate LLM calls:

1. **IMPROVE** (opus): Reads current spec + improvement prompt + previous judgment → produces next version
2. **JUDGE** (sonnet): Independently compares versions → structured quality score + remaining gaps

The judge's assessment feeds forward into the next improvement iteration, creating a tight feedback loop.

## Modular Mode

For specs that have been decomposed using the DDIS Modularization Protocol (§0.13), the RALPH loop operates in two phases:

**Phase 1: Constitution Improvement**
- Concatenates all constitution files (system + domain + deep context tiers) into a single document
- Runs the standard IMPROVE → JUDGE → DECIDE cycle on the combined constitution
- Splits the improved constitution back into individual source files
- Re-assembles all bundles using `ddis_assemble.sh`

**Phase 2: Per-Module Improvement**
- Determines module order using dependency analysis (fewer dependencies first, cross-cutting last)
- For each module: assembles its bundle, runs the RALPH cycle on the bundle, extracts the improved module back
- After each module improvement, runs cascade detection to flag downstream modules that reference changed invariants
- Ends with a full `ddis_validate.sh` pass to verify consistency

**Why two phases?** The constitution constrains all modules. Improving it first means every module benefits from the improved shared substrate. If modules were improved first, constitutional improvements would invalidate the module work.

**Cascade detection:** When a module is improved, the script checks which invariants it maintains and uses `ddis_validate.sh --check-cascade` to identify downstream modules. Flagged modules receive extra attention during their improvement iteration.

```bash
# Required for modular mode
ddis_assemble.sh   # In the same directory as ddis_ralph_loop.sh
ddis_validate.sh   # In the same directory as ddis_ralph_loop.sh
python3 + PyYAML   # For manifest parsing
```

## Beads Integration (Optional)

If [br](https://github.com/Dicklesworthstone/beads_rust) and [bv](https://github.com/Dicklesworthstone/beads_viewer) are installed, the script adds a gap tracking layer:

**What it does:**
- **Pre-loop**: Initializes a beads workspace in `ddis-evolution/`. If a workspace already exists from a previous run, persistent unresolved gaps are fed into iteration 1 as priority items.
- **Per-iteration**: After each judge assessment, remaining gaps are synced as br issues. Gaps resolved between iterations are automatically closed. Each gap is labeled with its origin iteration (`iter-1`, `iter-2`, etc.) for tracking persistence.
- **Post-loop**: Final remaining gaps persist as open br issues. If bv is available, `--robot-triage` provides dependency-aware prioritization of what to fix next (manually or in a future RALPH run).
- **Cross-run memory**: Gaps that survive multiple RALPH runs accumulate history — you can see which problems are genuinely hard vs. which were just missed.

**What it doesn't do:** Beads does NOT influence the stopping condition. The judge still drives convergence detection. Beads adds persistence and triage, not control flow. The loop is 3-5 iterations — not enough nodes for graph analysis to matter on the iteration DAG itself. The value is in treating *gaps* as a task backlog.

```bash
# Install (optional — script works fine without them)
curl -fsSL https://raw.githubusercontent.com/Dicklesworthstone/beads_rust/main/install.sh | bash
brew install dicklesworthstone/tap/bv

# After a RALPH run, explore remaining gaps:
cd ddis-evolution && bv                      # Interactive TUI
cd ddis-evolution && bv --robot-triage       # Structured triage for agents
cd ddis-evolution && br ready                # What's actionable
cd ddis-evolution && br list --label iter-1  # Gaps from iteration 1

# Control beads behavior:
DDIS_USE_BEADS=no ./ddis_ralph_loop.sh      # Disable entirely
DDIS_USE_BEADS=yes ./ddis_ralph_loop.sh     # Require (fail if not installed)
# Default (auto): use if found, skip gracefully if not
```

## Stopping Condition

Three independent signals, any of which triggers stop:

| Signal | What It Detects | Threshold |
|--------|----------------|-----------|
| Diminishing returns | Fewer than N substantive improvements | `DDIS_MIN_IMPROVEMENTS` (default: 2) |
| Quality plateau | Score delta below threshold | `DDIS_MIN_DELTA` (default: 3 points) |
| Regression | Version N is worse than N-1 | Any net regression → keep N-1 |

Plus: `stop_excellent` if score ≥ 95 with no critical gaps, and a safety cap at `DDIS_MAX_ITERATIONS`.

## Output

```
ddis-evolution/versions/
├── ddis_v0.md          # Seed (DDIS 1.0)
├── ddis_v1.md          # First improvement
├── ddis_v2.md          # Second improvement
└── ddis_final.md       # Best version, polished

ralph/judgments/
├── judgment_v1.json    # Judge comparison: v0 vs v1
└── judgment_v2.json    # Judge comparison: v1 vs v2
```

The final spec is written to `ddis-evolution/versions/ddis_final.md`.

In modular mode, the output structure also includes:

```
ddis-evolution/
├── constitution_versions/
│   ├── constitution_v0.md     # Concatenated constitution seed
│   ├── constitution_v1.md     # First improvement
│   └── ...
├── module_versions/
│   ├── event_store/
│   │   ├── bundle_v0.md       # Assembled bundle seed
│   │   ├── bundle_v1.md       # First improvement
│   │   └── ...
│   └── scheduler/
│       └── ...
├── judgments/
│   ├── constitution_judgment_v1.json
│   ├── module_event_store_judgment_v1.json
│   └── ...
└── ...
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `DDIS_MAX_ITERATIONS` | 5 | Safety cap on iterations |
| `DDIS_MIN_IMPROVEMENTS` | 2 | Minimum substantive improvements to continue |
| `DDIS_MIN_DELTA` | 3 | Minimum quality score increase to continue |
| `DDIS_IMPROVER_MODEL` | opus | Model for improvement (expensive, needs deep reasoning) |
| `DDIS_JUDGE_MODEL` | sonnet | Model for judging (structured eval, lower cost) |
| `DDIS_POLISH` | true | Run a consolidation pass on the final version |
| `DDIS_USE_BEADS` | auto | Beads gap tracking: "auto" (use if found), "yes" (require), "no" (skip) |
| `DDIS_AUTO_MODULARIZE` | true | Phase 0 auto-modularization: assess monoliths and decompose if warranted |

**CLI flags (modular mode):**

| Flag | Description |
|------|-------------|
| `--modular` | Enable modular mode (uses ddis_assemble.sh + ddis_validate.sh) |
| `--manifest PATH` | Path to manifest.yaml (implies `--modular`) |
| `--phase 1\|2\|both` | Run Phase 1 only (constitution), Phase 2 only (modules), or both (default) |

## Cost Estimate

**Monolith mode:** Each iteration ≈ 1 improver call (large context, opus) + 1 judge call (large context, sonnet).
Rough estimate: $2–5 per iteration depending on spec size. A full 5-iteration run: $10–25.

**Modular mode:** Phase 1 is similar to monolith mode. Phase 2 costs ≈ $2–5 per module per iteration (each module gets its own RALPH cycle). Total for N modules: Phase 1 ($10–25) + Phase 2 ($10–25 × N).

## End-to-End Modular Example

```bash
# 1. Start with a modular spec project:
cd my-spec-project/
ls manifest.yaml constitution/ modules/

# 2. Validate the manifest first:
ddis_validate.sh -m manifest.yaml -v

# 3. Assemble bundles to verify structure:
ddis_assemble.sh -m manifest.yaml -v

# 4. Run the full modular RALPH loop:
./ddis_ralph_loop.sh --manifest manifest.yaml

# 5. Or run phases independently:
./ddis_ralph_loop.sh --manifest manifest.yaml --phase 1   # Improve constitution
ddis_validate.sh -m manifest.yaml                          # Verify consistency
./ddis_ralph_loop.sh --manifest manifest.yaml --phase 2   # Improve modules

# 6. After the run, check remaining gaps:
cd ddis-evolution && bv --robot-triage
```

