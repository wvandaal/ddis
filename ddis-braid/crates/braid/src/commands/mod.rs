//! CLI command definitions and dispatch.

use std::path::{Path, PathBuf};

use braid_kernel::budget::{self, BudgetManager};
use clap::Subcommand;

pub(crate) mod analyze;
mod bilateral;
mod config;
mod harvest;
mod init;
mod log;
pub(crate) mod observe;
pub mod orientation;
mod query;
mod schema;
mod seed;
pub(crate) mod session;
pub(crate) mod shell;
mod spec;
mod status;
mod task;
mod trace;
mod wrap;
pub(crate) mod write;

// Re-export mcp serve as a special case (runs an event loop, not a single command).
pub use crate::mcp;

// ---------------------------------------------------------------------------
// Budget context (INV-BUDGET-001, spec/13-budget.md §13.2)
// ---------------------------------------------------------------------------

/// Budget context resolved from CLI flags.
///
/// Budget source precedence (IB-004):
/// 1. `--budget` flag (explicit token budget)
/// 2. `--context-used` flag (fraction consumed → k*_eff computation)
/// 3. Conservative default: full budget (k*_eff = 1.0)
#[derive(Clone, Debug)]
pub struct BudgetCtx {
    pub manager: BudgetManager,
}

impl BudgetCtx {
    /// Resolve budget from CLI flags following IB-004 precedence.
    pub fn from_flags(budget: Option<u32>, context_used: Option<f64>) -> Self {
        let mut mgr = BudgetManager::default();

        if let Some(pct) = context_used {
            // --context-used: direct measurement → full MEASURE transition
            mgr.measure(pct);
        } else if let Some(budget_tokens) = budget {
            // --budget: explicit token budget → back-compute k_eff
            // Invert: output_budget = max(MIN, Q(t) × W × 0.05)
            // Approximate k_eff from budget (monotonic, so search is valid)
            let target = budget_tokens as f64;
            let max_budget = mgr.window_size as f64 * budget::BUDGET_FRACTION;
            // Linear approximation: k_eff ≈ budget / max_budget (exact in full-quality regime)
            let approx_k = (target / max_budget).clamp(0.0, 1.0);
            mgr.measure(1.0 - approx_k);
            // Override with exact requested budget (the math above is approximate)
            mgr.output_budget = budget_tokens.max(budget::MIN_OUTPUT);
        }
        // else: default = full budget (k_eff=1.0, output_budget=10000)

        BudgetCtx { manager: mgr }
    }

    /// k*_eff for guidance footer compression.
    pub fn k_eff(&self) -> f64 {
        self.manager.k_eff
    }
}

/// All subcommands available in the `braid` CLI.
///
/// Commands are organized by workflow phase (ADR-INTERFACE-008: agent cycle):
///   SETUP:     init
///   CAPTURE:   observe, write
///   QUERY:     query, status, log
///   LIFECYCLE: harvest, seed
///   ADMIN:     shell, mcp, merge
#[derive(Subcommand)]
pub enum Command {
    // ── SETUP ──────────────────────────────────────────────────────────
    /// Create store, detect environment, record config as datoms.
    ///
    /// `braid init` → .braid/ + AGENTS.md + seed injected.
    /// Auto-detects git, language, tools. Bootstraps spec/ if present.
    #[command(after_long_help = "\
Examples:
  braid init
  braid init --path /tmp/mystore/.braid
  braid init --spec-dir spec    # auto-bootstrap spec elements

After init:
  braid observe \"project started\" --confidence 1.0     # first observation
  braid status                                          # verify store
  braid seed --inject AGENTS.md                         # configure agent context")]
    Init {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Spec directory to auto-bootstrap (if it exists).
        #[arg(long, default_value = "spec")]
        spec_dir: PathBuf,
    },

    // ── CAPTURE ────────────────────────────────────────────────────────
    /// Capture knowledge as content-addressed entity.
    ///
    /// `braid observe "CRDT merge commutes" -c 0.9` → entity :exploration/crdt-merge-commutes.
    /// Creates :exploration/* datoms. Use for knowledge capture during work.
    #[command(after_long_help = "\
Examples:
  braid observe \"merge is a bottleneck\" --confidence 0.8 --tag bottleneck
  braid observe \"CRDT merge is commutative\" --category theorem --relates-to :spec/inv-store-004
  braid observe \"query returns wrong results\" --confidence 0.3 --category conjecture

Categories: observation, conjecture, theorem, definition, algorithm, design-decision, open-question
Workflow: observe \u{2192} status (check) \u{2192} observe more \u{2192} harvest (commit)")]
    Observe {
        /// The observation text.
        text: String,

        /// Epistemic confidence (0.0=uncertain, 1.0=certain).
        #[arg(long, short = 'c', default_value = "0.7")]
        confidence: f64,

        /// Tags for filtering (repeatable).
        #[arg(long, short = 't', action = clap::ArgAction::Append)]
        tag: Vec<String>,

        /// Category: observation|conjecture|theorem|definition|algorithm|design-decision|open-question.
        #[arg(long)]
        category: Option<String>,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,

        /// Cross-reference to a spec element (e.g., ":spec/inv-store-001").
        #[arg(long)]
        relates_to: Option<String>,

        /// Rationale for a design decision (why this choice was made).
        #[arg(long)]
        rationale: Option<String>,

        /// Alternatives considered (for decisions — what else was evaluated).
        #[arg(long)]
        alternatives: Option<String>,
    },

    /// Write datoms: assert, retract, promote, or export.
    ///
    /// Subcommands for all structured store mutations.
    /// All modes are append-only (INV-STORE-001).
    #[command(
        subcommand_required = true,
        after_long_help = "\
Examples:
  braid write assert --rationale \"add spec\" --datom :spec/inv-001 :db/doc \"Append-only\"
  braid write retract --entity :spec/inv-001 --attribute :db/doc
  braid write promote --entity :obs/merge --target-id INV-STORE-042 --namespace STORE --type invariant
  braid write export --output spec"
    )]
    Write {
        #[command(subcommand)]
        action: WriteAction,
    },

    // ── QUERY ──────────────────────────────────────────────────────────
    /// Datalog or entity/attribute filter.
    ///
    /// `braid query '[:find ?e :where [?e :spec/type "invariant"]]'` → matching entities.
    /// Three modes: Datalog (positional or --datalog), entity filter (--entity), attribute filter (--attribute).
    #[command(after_long_help = "\
Examples:
  braid query '[:find ?e ?v :where [?e :db/doc ?v]]'          # Datalog (positional)
  braid query --entity :spec/inv-store-001                     # all datoms for entity
  braid query --attribute :db/doc                              # all values of attribute
  braid query --datalog '[:find ?e :where [?e :exploration/body _]]'  # Datalog (explicit)

Result format: [entity attribute value tx op] \u{2014} one line per datom
Empty results? Try: braid query --attribute :db/ident  # list known entities")]
    Query {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Filter by entity ident (keyword).
        #[arg(long, short = 'e')]
        entity: Option<String>,

        /// Filter by attribute (keyword).
        #[arg(long, short = 'a')]
        attribute: Option<String>,

        /// Datalog expression: [:find ?vars :where [clauses]].
        #[arg(long)]
        datalog: Option<String>,

        /// Positional Datalog expression (auto-detected from "[:find" prefix).
        #[arg(value_name = "DATALOG_EXPR")]
        positional_datalog: Option<String>,

        /// Frontier scope: "current" (latest per-agent), "tx:N" (up to tx wall-time N),
        /// or omitted (all datoms visible).
        ///
        /// Restricts query results to datoms visible within the specified frontier.
        /// Useful for time-travel queries and agent-scoped views.
        #[arg(long)]
        frontier: Option<String>,

        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Where you are: F(S), M(t), tasks, next action.
    ///
    /// `braid status` → store: 9k datoms, F(S)=0.77, next: trace 3 gaps.
    /// Progressive: bare → --verbose → --deep (bilateral F(S) + analytics).
    #[command(after_long_help = "\
Examples:
  braid status                         # terse dashboard with next action
  braid status --verbose               # full methodology + all actions
  braid status --deep                  # bilateral F(S) + graph analytics
  braid status --deep --spectral       # include spectral certificate
  braid status --verify                # integrity check
  braid status --json                  # structured output
  braid status --json | jq '.coherence'  # extract specific fields")]
    Status {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// Full output with all metrics and actions.
        #[arg(long)]
        verbose: bool,

        /// Run bilateral F(S) + graph analytics + convergence.
        #[arg(long)]
        deep: bool,

        /// Include spectral certificate (with --deep). Implied by --deep.
        #[arg(long, hide = true)]
        spectral: bool,

        /// Full 14-algorithm dashboard (with --deep). Implied by --deep.
        #[arg(long, hide = true)]
        full: bool,

        /// Verify on-disk store integrity (content hashes).
        #[arg(long)]
        verify: bool,

        /// Agent identity (for deep mode).
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,

        /// Persist bilateral cycle results (with --deep).
        #[arg(long)]
        commit: bool,
    },

    // ── COHERENCE ──────────────────────────────────────────────────────
    /// Coherence: F(S) + CC-1..5 + convergence.
    ///
    /// `braid bilateral` → F(S)=0.77, CC=4/5, next: trace 3 gaps.
    /// Focused coherence view. Use --commit to persist cycle results.
    #[command(after_long_help = "\
Examples:
  braid bilateral                     # F(S) + CC pass/fail + next steps
  braid bilateral --full              # full breakdown + spectral certificate
  braid bilateral --history           # convergence trajectory over time
  braid bilateral --json              # machine-readable output
  braid bilateral --commit            # persist cycle results to store

Workflow: braid bilateral → fix issues → braid bilateral --commit → track convergence")]
    Bilateral {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Full F(S) breakdown with spectral certificate.
        #[arg(long)]
        full: bool,

        /// Include spectral certificate (Phi, beta_1, entropy).
        #[arg(long)]
        spectral: bool,

        /// Show convergence trajectory over time.
        #[arg(long)]
        history: bool,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// Persist bilateral cycle results to the store.
        #[arg(long)]
        commit: bool,

        /// Agent identity (for commit provenance).
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    /// Link code to spec: scan source for INV/ADR/NEG references.
    ///
    /// `braid trace --commit` → 869 :impl/implements datoms, 259 :spec/witnessed.
    /// Scans Rust comments for spec element IDs, creates traceability datoms.
    #[command(after_long_help = "\
Examples:
  braid trace                          # dry-run: show what would be linked
  braid trace --commit                 # write traceability datoms to store
  braid trace --source crates/         # custom source directory

Workflow: braid trace → review → braid trace --commit → braid bilateral")]
    Trace {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Source directory to scan for Rust files.
        #[arg(long, short = 's', default_value = "crates")]
        source: PathBuf,

        /// Write traceability datoms to the store.
        #[arg(long)]
        commit: bool,

        /// Agent identity (for commit provenance).
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    /// Browse transactions, newest first.
    ///
    /// `braid log --limit 5` → 5 txns with agent, rationale, datom count.
    #[command(after_long_help = "\
Examples:
  braid log --limit 5                 # last 5 transactions (terse)
  braid log --verbose                  # verbose with rationale/provenance
  braid log --agent braid:user         # only this agent's transactions
  braid log --datoms                   # show individual datoms

Workflow: braid log --limit 3 \u{2192} see recent \u{2192} braid status \u{2192} decide next")]
    Log {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Max transactions to show.
        #[arg(long, short = 'n', default_value = "20")]
        limit: usize,

        /// Filter by agent name.
        #[arg(long, short = 'a')]
        agent: Option<String>,

        /// Include individual datoms per transaction.
        #[arg(long)]
        datoms: bool,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// Full multi-line output per transaction.
        #[arg(long)]
        verbose: bool,
    },

    /// Attribute discovery: types, cardinality, resolution modes.
    ///
    /// `braid schema --pattern ':spec/*'` → 11 spec attributes with types.
    /// Provides the info needed to write correct queries and transactions.
    #[command(after_long_help = "\
Examples:
  braid schema                                # list all attributes
  braid schema --pattern ':db/*'              # filter by namespace glob
  braid schema --pattern ':spec/*' --verbose  # full details with usage counts
  braid schema --json                         # structured JSON output
  braid schema --pattern harvest              # substring match

Workflow: braid schema \u{2192} pick attributes \u{2192} braid query --attribute :attr/name")]
    Schema {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Filter attributes by pattern (glob with * or substring match).
        #[arg(long)]
        pattern: Option<String>,

        /// Show full details per attribute (type, cardinality, resolution, usage count).
        #[arg(long)]
        verbose: bool,

        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },

    // ── LIFECYCLE ──────────────────────────────────────────────────────
    /// End-of-session: observations → datoms.
    ///
    /// `braid harvest --commit` → 5 candidates crystallized, 12 datoms.
    /// Scores by novelty/specificity/relevance. Task auto-detected from session context.
    #[command(after_long_help = "\
Examples:
  braid harvest                                          # auto-detect, show candidates
  braid harvest --task \"implemented query engine\"         # explicit task override
  braid harvest --commit                                  # persist candidates
  braid harvest --commit --guard                          # commit with crystallization guard
  braid harvest --commit --force                          # bypass crystallization guard (legacy)
  braid harvest --knowledge gap \"missing join optimization\" --commit

Workflow:
  braid observe \"found auth bug\" --confidence 0.8        # capture during session
  braid observe \"decided JWT\" --category design-decision  # record decisions
  braid harvest                                          # review candidates at session end
  braid harvest --commit                                  # persist to store
  braid seed --task \"continue\"                            # next session picks up")]
    Harvest {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,

        /// Task description override (auto-detected if omitted).
        #[arg(long, short = 't')]
        task: Option<String>,

        /// Knowledge items: key value (repeatable).
        #[arg(long = "knowledge", short = 'k', num_args = 2, action = clap::ArgAction::Append)]
        knowledge: Vec<String>,

        /// Persist approved candidates to the store.
        /// At Stage 0, --commit bypasses the crystallization guard by default
        /// (no external validators exist to benefit from staged crystallization).
        /// Use --guard to explicitly enable the crystallization guard.
        #[arg(long)]
        commit: bool,

        /// Bypass crystallization guard (commit all candidates regardless of stability).
        /// At Stage 0, --commit implies --force. This flag is retained for explicitness.
        #[arg(long, short = 'f')]
        force: bool,

        /// Explicitly enable the crystallization guard (overrides Stage 0 default).
        /// Useful when multiple agents contribute to knowledge maturation.
        #[arg(long)]
        guard: bool,
    },

    /// Start-of-session: store → agent context.
    ///
    /// `braid seed --task "my work"` → 5-section briefing under token budget.
    /// Assembles relevant entities, recent txns, methodology guidance for the task.
    #[command(after_long_help = "\
Examples:
  braid seed --task \"fix query engine joins\" --seed-budget 3000
  braid seed                                    # continue last session's task
  braid seed --task \"implement harvest\" --for-human
  braid seed --task \"implement harvest\" --agent-md
  braid seed --compact                          # <200 tokens, orientation + directive")]
    Seed {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Task description (auto-detected from last session if omitted).
        #[arg(long, short = 't')]
        task: Option<String>,

        /// Token budget for seed output (distinct from global --budget).
        #[arg(long, default_value = "2000")]
        seed_budget: usize,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,

        /// Emit a natural-language briefing (< 200 words) instead of structured sections.
        #[arg(long)]
        for_human: bool,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// Also generate dynamic AGENTS.md from store state (ADR-SEED-006).
        #[arg(long)]
        agent_md: bool,

        /// Compact mode: <200 tokens, orientation + top entities + directive only.
        #[arg(long)]
        compact: bool,

        /// Inject seed context into a file's <braid-seed> tags (C7: self-bootstrap).
        ///
        /// Reads the file, finds <braid-seed>...</braid-seed> tags, replaces
        /// content between them with dynamically generated context from the store,
        /// and writes the file back. Content outside tags is never modified.
        #[arg(long)]
        inject: Option<PathBuf>,
    },

    // ── SESSION LIFECYCLE ──────────────────────────────────────────────
    /// Session lifecycle: start and end sessions with one command.
    ///
    /// `braid session start` — inject seed + show actionable summary.
    /// `braid session end`   — harvest + re-inject + show git guidance.
    ///
    /// Replaces the multi-step start/end protocol with two commands.
    #[command(
        subcommand_required = true,
        after_long_help = "\
Examples:
  braid session start                                 # auto-continue from last harvest
  braid session start --task \"implement budget output\" # explicit task
  braid session end                                   # harvest + inject + guide
  braid session end --task \"completed budget pipeline\" # override task

Workflow: session start → observe → work → observe → session end"
    )]
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    // ── CAPTURE (continued) ────────────────────────────────────────────
    /// Run a command and auto-observe failures/warnings.
    ///
    /// Proxies subprocess output to the terminal in real time. On failure or
    /// warnings, creates an observation automatically. Clean success = no
    /// observation (INV-WRAP-001).
    #[command(after_long_help = "\
Examples:
  braid wrap cargo test                    # auto-observe test failures
  braid wrap cargo clippy -- -D warnings   # auto-observe warnings
  braid wrap cargo fmt --check             # auto-observe format issues
  braid wrap ./scripts/e2e.sh              # any command

Workflow: braid wrap cargo test → fix failures → braid wrap cargo test")]
    Wrap {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,

        /// Timeout in seconds (0 = no timeout).
        #[arg(long, default_value = "0")]
        timeout: u64,

        /// The command and its arguments.
        #[arg(trailing_var_arg = true, required = true)]
        cmd: Vec<String>,
    },

    // ── TASK MANAGEMENT ─────────────────────────────────────────────────
    /// Issue tracking as datoms — create, list, close, depend.
    ///
    /// Tasks are first-class store entities with lattice-resolved status
    /// (INV-TASK-001) and DAG dependencies (INV-TASK-002).
    #[command(
        subcommand_required = true,
        after_long_help = "\
Examples:
  braid task create \"Fix harvest noise\" --priority 1 --type bug
  braid task list                        # open tasks
  braid task ready                       # unblocked, sorted by priority
  braid task show <id>                   # full detail
  braid task close <id> --reason done
  braid task update <id> --status in-progress
  braid task dep <from-id> <to-id>       # add dependency edge
  braid task import --beads .beads/issues.jsonl

Workflow: braid task ready → pick top → braid task update <id> --status in-progress → work → braid task close <id>"
    )]
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },

    // ── CONFIGURATION ──────────────────────────────────────────────────
    /// Read, write, or list configuration (stored as datoms, not files).
    ///
    /// Config lives in the store (ADR-INTERFACE-005). No .braid/config.toml.
    /// Unset keys fall back to built-in defaults.
    #[command(after_long_help = "\
Examples:
  braid config                           # list all config
  braid config output.default-mode       # get a value
  braid config output.default-mode json  # set a value
  braid config --reset output.default-mode  # revert to default

Built-in keys: output.default-mode, output.token-budget, harvest.auto-commit,
  harvest.confidence-floor, session.auto-start, trace.source-dirs, git.enabled")]
    Config {
        /// Config key.
        #[arg(value_name = "KEY")]
        key: Option<String>,

        /// Config value (set mode).
        #[arg(value_name = "VALUE")]
        value: Option<String>,

        /// Reset key to default.
        #[arg(long)]
        reset: bool,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    // ── ADMIN ──────────────────────────────────────────────────────────
    /// Interactive exploration shell (zero external deps).
    ///
    /// Starts a readline loop for quick exploration. Type 'help' for commands.
    /// Exit with Ctrl-D or 'quit'.
    #[command(after_long_help = "\
Examples:
  braid shell                          # start with default .braid store
  braid shell --path /tmp/store        # start with custom store path")]
    Shell {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,
    },

    /// Merge another store into this one (CRDT set union, no conflicts).
    Merge {
        /// Target store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Source store to merge from.
        #[arg(long, short = 's')]
        source: PathBuf,
    },

    /// Start MCP server (JSON-RPC over stdio).
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },

    // ── SHORTCUTS (WP6) ──────────────────────────────────────────────
    /// Top unblocked task + claim command.
    ///
    /// `braid next` → T-42: implement merge → `braid go T-42`.
    /// Use --skip to skip a specific task.
    #[command(after_long_help = "\
Examples:
  braid next                  # show top ready task
  braid next --skip t-aB3c   # skip a task, show the next one

Workflow: braid next → braid go <id> → work → braid done <id>")]
    Next {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Skip this task ID (show the next one).
        #[arg(long)]
        skip: Option<String>,
    },

    /// Close a task (shortcut for `braid task close`).
    ///
    /// Closes the current or specified task with reason "completed".
    #[command(after_long_help = "\
Examples:
  braid done t-aB3c            # close specific task
  braid done t-aB3c t-xY2z     # close multiple tasks

Workflow: braid go <id> → work → braid done <id>")]
    Done {
        /// Task ID(s) to close.
        ids: Vec<String>,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    /// Quick observation (shortcut for `braid observe` with defaults).
    ///
    /// Creates an observation with confidence 0.7 and category "observation".
    /// For decisions, use `braid observe --category design-decision` instead.
    #[command(after_long_help = "\
Examples:
  braid note \"The merge path needs optimization\"
  braid note \"Found a bug in query joins\" --confidence 0.9

Workflow: braid note \"...\" → braid status → braid note \"...\" → braid harvest")]
    Note {
        /// The observation text.
        text: String,

        /// Epistemic confidence (default: 0.7).
        #[arg(long, short = 'c', default_value = "0.7")]
        confidence: f64,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    /// Claim a task (shortcut for `braid task update --status in-progress`).
    #[command(after_long_help = "\
Examples:
  braid go t-aB3c             # start working on task

Workflow: braid next → braid go <id> → work → braid done <id>")]
    Go {
        /// Task ID to claim.
        id: String,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    // ── SPEC MANAGEMENT (WP5, W4B.3) ────────────────────────────────
    /// Create, review, accept, reject, and inspect spec proposals.
    ///
    /// Auto-detects type from ID prefix (INV-, ADR-, NEG-).
    /// Proposal review lifecycle: review → accept/reject → history.
    #[command(after_long_help = "\
Examples:
  braid spec create INV-OUTPUT-001 \"Mode Resolution Determinism\" \\
    --statement \"Given identical inputs, resolve() returns the same Mode\" \\
    --falsification \"Two calls with identical inputs return different modes\"
  braid spec review                           # list pending proposals
  braid spec accept INV-STORE-017             # accept by suggested ID
  braid spec reject a1b2c3d4 --reason \"Duplicate of INV-STORE-003\"
  braid spec history                          # full proposal lifecycle")]
    Spec {
        #[command(subcommand)]
        action: SpecAction,
    },
}

/// Spec subcommands (WP5, W4B.3).
#[derive(Subcommand)]
pub enum SpecAction {
    /// Create a new spec element (INV, ADR, or NEG).
    Create {
        /// Spec element ID (e.g., INV-OUTPUT-001, ADR-INTERFACE-010, NEG-MUTATION-001).
        /// Type is auto-detected from prefix.
        id: String,

        /// Title / short description.
        title: String,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Formal statement (invariants and negative cases).
        #[arg(long)]
        statement: Option<String>,

        /// Falsification condition (invariants and negative cases).
        #[arg(long)]
        falsification: Option<String>,

        /// Problem statement (ADRs).
        #[arg(long)]
        problem: Option<String>,

        /// Decision text (ADRs).
        #[arg(long)]
        decision: Option<String>,

        /// Traces to (SEED.md section or spec reference).
        #[arg(long)]
        traces_to: Option<String>,

        /// Epistemic confidence (0.0-1.0).
        #[arg(long, short = 'c')]
        confidence: Option<f64>,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    /// List pending proposals awaiting human review (confidence < 0.9).
    ///
    /// Shows proposals generated by harvest that need explicit accept/reject.
    Review {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,
    },

    /// Accept a pending proposal, promoting it to a first-class spec element.
    ///
    /// The proposal ID can be a suggested spec ID (e.g., INV-STORE-017) or
    /// an entity hex prefix from `braid spec review`.
    Accept {
        /// Proposal identifier (suggested ID or entity hex prefix).
        id: String,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    /// Reject a pending proposal with a rationale note.
    ///
    /// The proposal ID can be a suggested spec ID (e.g., INV-STORE-017) or
    /// an entity hex prefix from `braid spec review`.
    Reject {
        /// Proposal identifier (suggested ID or entity hex prefix).
        id: String,

        /// Rationale for rejection.
        #[arg(long)]
        reason: String,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    /// Show all proposals with their lifecycle status (accepted/rejected/pending).
    History {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,
    },
}

/// Task subcommands.
#[derive(Subcommand)]
pub enum TaskAction {
    /// Create a new task.
    Create {
        /// Task title.
        title: String,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Priority: 0=critical, 1=high, 2=medium, 3=low, 4=backlog.
        #[arg(long, default_value = "2")]
        priority: i64,

        /// Type: task, bug, feature, epic, question, docs.
        #[arg(long = "type", default_value = "task")]
        task_type: String,

        /// Description (longer detail).
        #[arg(long)]
        description: Option<String>,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,

        /// Spec element references (repeatable).
        #[arg(long = "traces-to", action = clap::ArgAction::Append)]
        traces_to: Vec<String>,

        /// Labels (repeatable).
        #[arg(long = "label", action = clap::ArgAction::Append)]
        labels: Vec<String>,
    },

    /// List tasks (open by default, --all for everything).
    List {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Include closed tasks.
        #[arg(long)]
        all: bool,
    },

    /// Show ready (unblocked) tasks sorted by priority.
    Ready {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,
    },

    /// Show detailed info about a specific task.
    Show {
        /// Task ID (e.g., t-aB3c).
        id: String,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,
    },

    /// Close one or more tasks.
    Close {
        /// Task ID(s) to close.
        ids: Vec<String>,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Reason for closing.
        #[arg(long, default_value = "completed")]
        reason: String,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    /// Update a task's status.
    Update {
        /// Task ID.
        id: String,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// New status: open, in-progress, closed.
        #[arg(long)]
        status: String,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    /// Add a dependency edge between tasks.
    Dep {
        /// Source task (the one that depends).
        from: String,

        /// Target task (the one depended upon).
        to: String,

        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    /// Import tasks from a beads JSONL file.
    Import {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Path to beads JSONL file.
        #[arg(long)]
        beads: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },
}

/// Write subcommands — each mode has exactly the flags it needs.
#[derive(Subcommand)]
pub enum WriteAction {
    /// Assert datoms into the store.
    ///
    /// For structured data. Each --datom flag takes 3 args: entity attribute value.
    /// Prefer `braid observe` for knowledge capture; assert for schema/metadata.
    #[command(after_long_help = "\
Examples:
  braid write assert --rationale \"add spec\" --datom :spec/inv-001 :db/doc \"Append-only\"
  braid write assert -r \"link entities\" -d :spec/inv-001 :spec/traces-to \"SEED.md s4\"")]
    Assert {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,

        /// Why this transaction exists.
        #[arg(long, short = 'r')]
        rationale: String,

        /// Datom triples: entity attribute value (repeatable).
        #[arg(long = "datom", short = 'd', num_args = 3, action = clap::ArgAction::Append)]
        datoms: Vec<String>,
    },

    /// Retract assertions (append-only: creates retraction datoms, never deletes).
    #[command(after_long_help = "\
Examples:
  braid write retract --entity :spec/inv-store-001 --attribute :db/doc
  braid write retract --entity :spec/inv-001 --attribute :db/doc --value \"old text\"")]
    Retract {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,

        /// Entity ident (e.g., ":spec/inv-store-001").
        #[arg(long, short = 'e')]
        entity: String,

        /// Attribute to retract (e.g., ":db/doc").
        #[arg(long)]
        attribute: String,

        /// Only retract if value matches this.
        #[arg(long, short = 'v')]
        value: Option<String>,
    },

    /// Promote an exploration entity to a formal spec element.
    #[command(after_long_help = "\
Examples:
  braid write promote --entity :observation/merge-bottleneck --target-id INV-STORE-042 \\
    --namespace STORE --type invariant --statement \"Merge is O(n)\" \\
    --falsification \"Merge > O(n log n)\"")]
    Promote {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Entity ident to promote.
        #[arg(long, short = 'e')]
        entity: String,

        /// Target spec element ID (e.g., "INV-STORE-042").
        #[arg(long)]
        target_id: String,

        /// Target namespace (e.g., "STORE").
        #[arg(long, short = 'n')]
        namespace: String,

        /// Target type: invariant, adr, negative-case.
        #[arg(long = "type")]
        target_type: String,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,

        /// Formal statement (invariants).
        #[arg(long)]
        statement: Option<String>,

        /// Falsification condition (invariants, negative cases).
        #[arg(long)]
        falsification: Option<String>,

        /// Verification method.
        #[arg(long)]
        verification: Option<String>,

        /// Problem statement (ADRs).
        #[arg(long)]
        problem: Option<String>,

        /// Decision text (ADRs).
        #[arg(long)]
        decision: Option<String>,
    },

    /// Export store entities to spec/*.md (inverse of bootstrap).
    #[command(after_long_help = "\
Examples:
  braid write export
  braid write export --output spec --namespace STORE")]
    Export {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Output directory for spec files.
        #[arg(long, short = 'o', default_value = "spec")]
        output: PathBuf,

        /// Filter to one namespace (e.g., "STORE"). Omit for all.
        #[arg(long, short = 'n')]
        namespace: Option<String>,
    },
}

/// MCP server subcommands.
#[derive(Subcommand)]
pub enum McpAction {
    /// Start the MCP server (reads JSON-RPC from stdin, writes to stdout).
    Serve {
        /// Path to the .braid directory.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,
    },
}

/// Session lifecycle subcommands.
#[derive(Subcommand)]
pub enum SessionAction {
    /// Start a session: inject seed into AGENTS.md, show actionable summary.
    ///
    /// Task is auto-detected from the last harvest's synthesis directive.
    /// Override with --task.
    Start {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Task description. If omitted, reads from last harvest's synthesis directive.
        #[arg(long, short = 't')]
        task: Option<String>,

        /// File to inject seed into.
        #[arg(long, default_value = "AGENTS.md")]
        inject: PathBuf,

        /// Token budget for seed injection (distinct from global --budget).
        #[arg(long, default_value = "2000")]
        seed_budget: usize,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },

    /// End a session: harvest, re-inject seed, show git guidance.
    ///
    /// Does NOT run git commands — shows guidance only.
    End {
        /// Store directory path.
        #[arg(long, short = 'p', default_value = ".braid")]
        path: PathBuf,

        /// Task description override.
        #[arg(long, short = 't')]
        task: Option<String>,

        /// File to inject seed into after harvest.
        #[arg(long, default_value = "AGENTS.md")]
        inject: PathBuf,

        /// Token budget for seed injection (distinct from global --budget).
        #[arg(long, default_value = "2000")]
        seed_budget: usize,

        /// Agent identity.
        #[arg(long, short = 'a', default_value = "braid:user")]
        agent: String,
    },
}

/// Extract the store path from a command variant (if the command uses a store).
fn store_path(cmd: &Command) -> Option<&Path> {
    match cmd {
        Command::Mcp { .. } => None,
        Command::Init { path, .. }
        | Command::Status { path, .. }
        | Command::Bilateral { path, .. }
        | Command::Trace { path, .. }
        | Command::Query { path, .. }
        | Command::Schema { path, .. }
        | Command::Harvest { path, .. }
        | Command::Seed { path, .. }
        | Command::Merge { path, .. }
        | Command::Log { path, .. }
        | Command::Observe { path, .. }
        | Command::Shell { path, .. }
        | Command::Wrap { path, .. }
        | Command::Config { path, .. }
        | Command::Next { path, .. }
        | Command::Done { path, .. }
        | Command::Note { path, .. }
        | Command::Go { path, .. } => Some(path),
        Command::Session { action } => match action {
            SessionAction::Start { path, .. } | SessionAction::End { path, .. } => Some(path),
        },
        Command::Task { action } => match action {
            TaskAction::Create { path, .. }
            | TaskAction::List { path, .. }
            | TaskAction::Ready { path, .. }
            | TaskAction::Show { path, .. }
            | TaskAction::Close { path, .. }
            | TaskAction::Update { path, .. }
            | TaskAction::Dep { path, .. }
            | TaskAction::Import { path, .. } => Some(path),
        },
        Command::Write { action } => match action {
            WriteAction::Assert { path, .. }
            | WriteAction::Retract { path, .. }
            | WriteAction::Promote { path, .. }
            | WriteAction::Export { path, .. } => Some(path),
        },
        Command::Spec { action } => match action {
            SpecAction::Create { path, .. }
            | SpecAction::Review { path, .. }
            | SpecAction::Accept { path, .. }
            | SpecAction::Reject { path, .. }
            | SpecAction::History { path, .. } => Some(path),
        },
    }
}

/// Whether a command produces JSON output (footers must not corrupt JSON).
fn is_json_output(cmd: &Command, mode: crate::output::OutputMode) -> bool {
    // Global --format json overrides per-command --json flags
    if mode == crate::output::OutputMode::Json {
        return true;
    }
    matches!(
        cmd,
        Command::Query { json: true, .. }
            | Command::Status { json: true, .. }
            | Command::Schema { json: true, .. }
            | Command::Log { json: true, .. }
            | Command::Seed { json: true, .. }
            | Command::Bilateral { json: true, .. }
    )
}

/// Whether a command already includes guidance output (avoid duplication).
fn is_guidance_command(cmd: &Command) -> bool {
    matches!(cmd, Command::Shell { .. })
}

/// Whether the command output may be piped to files (footers would corrupt).
fn is_generative_output(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Seed { .. }
            | Command::Session { .. }
            | Command::Wrap { .. }
            | Command::Write {
                action: WriteAction::Export { .. }
            }
    )
}

/// Inject a guidance footer into all three representations of a CommandOutput.
fn inject_footer(
    mut cmd_output: crate::output::CommandOutput,
    footer_text: &str,
) -> crate::output::CommandOutput {
    // Human mode: append as suffix
    cmd_output.human.push('\n');
    cmd_output.human.push_str(footer_text);
    // Agent mode: set the designated footer field
    cmd_output.agent.footer = footer_text.to_string();
    // JSON mode: add as _guidance field
    if let serde_json::Value::Object(ref mut map) = cmd_output.json {
        map.insert("_guidance".to_string(), serde_json::json!(footer_text));
    }
    cmd_output
}

/// Apply guidance footer to a CommandOutput if applicable.
///
/// INV-GUIDANCE-001: Every non-JSON, non-generative tool response includes guidance.
/// This is the single injection point — all commands route through here.
fn maybe_inject_footer(
    cmd_output: crate::output::CommandOutput,
    skip_footer: bool,
    path: Option<&Path>,
    budget_ctx: &BudgetCtx,
) -> crate::output::CommandOutput {
    if skip_footer {
        return cmd_output;
    }
    match path.and_then(|p| try_build_footer(p, budget_ctx)) {
        Some(footer_text) => inject_footer(cmd_output, &footer_text),
        None => cmd_output,
    }
}

/// Apply the budget gate to a `CommandOutput` (INV-BUDGET-001).
///
/// Enforces `budget_ctx.manager.output_budget` as a hard token ceiling on
/// the human and agent text representations. JSON output is **never**
/// truncated — agents consuming structured data need every field intact.
///
/// Intended to be called in `main()` after `commands::run()` returns and
/// before `CommandOutput::render()`. This is the last gate in the pipeline.
pub fn apply_budget_gate(
    mut output: crate::output::CommandOutput,
    mode: crate::output::OutputMode,
    budget_ctx: &BudgetCtx,
) -> crate::output::CommandOutput {
    // JSON mode: never truncate — agents need complete structured data.
    if mode == crate::output::OutputMode::Json {
        return output;
    }

    let ceiling = budget_ctx.manager.output_budget as usize;

    // Enforce ceiling on the human-readable representation.
    output.human = budget::enforce_ceiling(&output.human, ceiling);

    // Enforce ceiling on the agent-mode rendered text.
    let agent_rendered = output.agent.render();
    let gated = budget::enforce_ceiling(&agent_rendered, ceiling);
    if gated != agent_rendered {
        // The agent output was truncated — replace content with the gated text
        // while preserving the three-part structure as best we can.
        // Context and footer stay; content absorbs the truncation.
        output.agent.content = gated;
        output.agent.context = String::new();
        output.agent.footer = String::new();
    }

    output
}

/// Build a guidance footer string (INV-GUIDANCE-001).
///
/// Best-effort: if the store can't be loaded, returns None.
///
/// INV-BUDGET-004: Guidance footer compresses by k*_eff level from `budget_ctx`.
fn try_build_footer(path: &Path, budget_ctx: &BudgetCtx) -> Option<String> {
    let layout = crate::layout::DiskLayout::open(path).ok()?;
    let store = layout.load_store().ok()?;
    let footer = braid_kernel::guidance::build_command_footer(&store, Some(budget_ctx.k_eff()));
    if footer.is_empty() {
        None
    } else {
        Some(footer)
    }
}

/// Resolve the store path: if the default `.braid` doesn't exist in CWD,
/// walk up the directory tree to find it (like git finds `.git/`).
fn resolve_store_path(path: PathBuf) -> PathBuf {
    if path.is_dir() {
        return path;
    }
    // Only auto-detect when using the default ".braid" path
    if path == Path::new(".braid") {
        if let Ok(cwd) = std::env::current_dir() {
            if let Some(found) = crate::layout::find_braid_root(&cwd) {
                return found;
            }
        }
    }
    path // Return original (will error at open time with a clear message)
}

/// Rewrite all `path` fields in a Command to resolved paths.
fn resolve_command_paths(mut cmd: Command) -> Command {
    match &mut cmd {
        Command::Mcp { .. } => {}
        Command::Init { path, .. }
        | Command::Status { path, .. }
        | Command::Bilateral { path, .. }
        | Command::Trace { path, .. }
        | Command::Query { path, .. }
        | Command::Schema { path, .. }
        | Command::Harvest { path, .. }
        | Command::Seed { path, .. }
        | Command::Merge { path, .. }
        | Command::Log { path, .. }
        | Command::Observe { path, .. }
        | Command::Shell { path, .. }
        | Command::Wrap { path, .. }
        | Command::Config { path, .. }
        | Command::Next { path, .. }
        | Command::Done { path, .. }
        | Command::Note { path, .. }
        | Command::Go { path, .. } => {
            *path = resolve_store_path(path.clone());
        }
        Command::Session { action } => match action {
            SessionAction::Start { path, .. } | SessionAction::End { path, .. } => {
                *path = resolve_store_path(path.clone());
            }
        },
        Command::Task { action } => match action {
            TaskAction::Create { path, .. }
            | TaskAction::List { path, .. }
            | TaskAction::Ready { path, .. }
            | TaskAction::Show { path, .. }
            | TaskAction::Close { path, .. }
            | TaskAction::Update { path, .. }
            | TaskAction::Dep { path, .. }
            | TaskAction::Import { path, .. } => {
                *path = resolve_store_path(path.clone());
            }
        },
        Command::Write { action } => match action {
            WriteAction::Assert { path, .. }
            | WriteAction::Retract { path, .. }
            | WriteAction::Promote { path, .. }
            | WriteAction::Export { path, .. } => {
                *path = resolve_store_path(path.clone());
            }
        },
        Command::Spec { action } => match action {
            SpecAction::Create { path, .. }
            | SpecAction::Review { path, .. }
            | SpecAction::Accept { path, .. }
            | SpecAction::Reject { path, .. }
            | SpecAction::History { path, .. } => {
                *path = resolve_store_path(path.clone());
            }
        },
    }
    cmd
}

/// Execute a CLI command and return structured output.
///
/// INV-BUDGET-001: Output respects `budget_ctx` for guidance footer compression
/// and command attention profiles. Commands that exceed their attention profile
/// ceiling are truncated.
///
/// All commands return `CommandOutput` via the `from_human()` bridge until
/// individually converted to native tri-mode output (Phase C).
pub fn run(
    cmd: Command,
    budget_ctx: &BudgetCtx,
    mode: crate::output::OutputMode,
) -> Result<crate::output::CommandOutput, crate::error::BraidError> {
    use crate::output::CommandOutput;

    // Resolve .braid path by walking up directory tree (like git finds .git/).
    let cmd = resolve_command_paths(cmd);

    // Pre-extract metadata needed for footer injection (before cmd is consumed).
    let path_for_footer = store_path(&cmd).map(|p| p.to_path_buf());
    let skip_footer =
        is_json_output(&cmd, mode) || is_guidance_command(&cmd) || is_generative_output(&cmd);

    let result: Result<String, crate::error::BraidError> = match cmd {
        Command::Init { path, spec_dir } => {
            let cmd_output = init::run(&path, &spec_dir)?;
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Status {
            path,
            json,
            verbose,
            deep,
            spectral,
            full,
            verify,
            agent,
            commit,
        } => {
            // --deep implies --full and --spectral (progressive disclosure).
            let full = full || deep;
            let spectral = spectral || deep;
            // Status returns CommandOutput natively — need special handling.
            let cmd_output = status::run(
                &path, &agent, json, verbose, deep, spectral, full, verify, commit,
            )?;
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Bilateral {
            path,
            full,
            spectral,
            history,
            json,
            commit,
            agent,
        } => {
            let cmd_output = bilateral::run(&path, &agent, full, spectral, history, json, commit)?;
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Trace {
            path,
            source,
            commit,
            agent,
        } => trace::run(&path, &source, &agent, commit),
        Command::Write { action } => match action {
            WriteAction::Assert {
                path,
                agent,
                rationale,
                datoms,
            } => write::run_assert(&path, &agent, &rationale, &datoms),
            WriteAction::Retract {
                path,
                agent,
                entity,
                attribute,
                value,
            } => write::run_retract(&path, &agent, &entity, &attribute, value.as_deref()),
            WriteAction::Promote {
                path,
                entity,
                target_id,
                namespace,
                target_type,
                agent,
                statement,
                falsification,
                verification,
                problem,
                decision,
            } => write::run_promote(write::PromoteArgs {
                path: &path,
                entity_ident: &entity,
                target_id: &target_id,
                namespace: &namespace,
                target_type: &target_type,
                agent_name: &agent,
                statement: statement.as_deref(),
                falsification: falsification.as_deref(),
                verification: verification.as_deref(),
                problem: problem.as_deref(),
                decision: decision.as_deref(),
            }),
            WriteAction::Export {
                path,
                output,
                namespace,
            } => write::run_export(&path, &output, namespace.as_deref()),
        },
        Command::Query {
            path,
            entity,
            attribute,
            datalog,
            positional_datalog,
            frontier,
            json,
        } => {
            let dq = datalog.or(positional_datalog);
            let cmd_output = if let Some(ref dq) = dq {
                query::run_datalog(&path, dq, frontier.as_deref(), json)?
            } else {
                query::run(
                    &path,
                    entity.as_deref(),
                    attribute.as_deref(),
                    frontier.as_deref(),
                    json,
                )?
            };
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Schema {
            path,
            pattern,
            verbose,
            json,
        } => {
            let cmd_output = schema::run(&path, pattern.as_deref(), verbose, json)?;
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Harvest {
            path,
            agent,
            task,
            knowledge,
            commit,
            force,
            guard,
        } => {
            // Stage 0: --commit bypasses crystallization guard by default.
            // --guard re-enables it. --force always bypasses (legacy compat).
            let effective_force = force || (commit && !guard);
            let cmd_output = harvest::run(
                &path,
                &agent,
                task.as_deref(),
                &knowledge,
                commit,
                effective_force,
            )?;
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Seed {
            path,
            task,
            seed_budget,
            agent,
            for_human,
            json,
            agent_md,
            compact,
            inject,
        } => {
            let mut effective_budget = if compact { 200 } else { seed_budget };
            // INV-BUDGET-001: Global budget acts as ceiling for seed output.
            // Seed's own --budget controls content assembly; global --budget
            // is a hard cap from the caller's remaining context window.
            effective_budget = effective_budget.min(budget_ctx.manager.output_budget as usize);
            let effective_task = task.as_deref().unwrap_or("continue");

            // --inject mode: update file in place with seed content (SB.3.3)
            if let Some(ref inject_path) = inject {
                let cmd_output =
                    seed::run_inject(&path, inject_path, effective_task, effective_budget)?;
                return Ok(maybe_inject_footer(
                    cmd_output,
                    skip_footer,
                    path_for_footer.as_deref(),
                    budget_ctx,
                ));
            }

            let cmd_output = seed::run(
                &path,
                effective_task,
                effective_budget,
                &agent,
                for_human,
                json,
                agent_md,
            )?;
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Merge { path, source } => merge::run(&path, &source),
        Command::Log {
            path,
            limit,
            agent,
            datoms,
            json,
            verbose,
        } => {
            let cmd_output = log::run(&path, limit, agent.as_deref(), datoms, json, verbose)?;
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Observe {
            path,
            text,
            confidence,
            tag,
            category,
            agent,
            relates_to,
            rationale,
            alternatives,
        } => {
            let cmd_output = observe::run(observe::ObserveArgs {
                path: &path,
                text: &text,
                confidence,
                tags: &tag,
                category: category.as_deref(),
                agent: &agent,
                relates_to: relates_to.as_deref(),
                rationale: rationale.as_deref(),
                alternatives: alternatives.as_deref(),
            })?;
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Session { action } => match action {
            SessionAction::Start {
                path,
                task,
                inject,
                seed_budget,
                agent,
            } => {
                let eff = seed_budget.min(budget_ctx.manager.output_budget as usize);
                session::run_start(&path, &inject, task.as_deref(), eff, &agent)
            }
            SessionAction::End {
                path,
                task,
                inject,
                seed_budget,
                agent,
            } => {
                let eff = seed_budget.min(budget_ctx.manager.output_budget as usize);
                session::run_end(&path, &inject, task.as_deref(), eff, &agent)
            }
        },
        Command::Wrap {
            path,
            agent,
            timeout,
            cmd,
        } => {
            let timeout_opt = if timeout == 0 { None } else { Some(timeout) };
            wrap::run(&path, &agent, &cmd, timeout_opt)
        }
        Command::Task { action } => match action {
            TaskAction::Create {
                title,
                path,
                priority,
                task_type,
                description,
                agent,
                traces_to,
                labels,
            } => task::create(task::CreateArgs {
                path: &path,
                title: &title,
                description: description.as_deref(),
                priority,
                task_type: &task_type,
                agent: &agent,
                traces_to: &traces_to,
                labels: &labels,
            }),
            TaskAction::List { path, all } => task::list(&path, all),
            TaskAction::Ready { path } => {
                let cmd_output = task::ready(&path)?;
                return Ok(maybe_inject_footer(
                    cmd_output,
                    skip_footer,
                    path_for_footer.as_deref(),
                    budget_ctx,
                ));
            }
            TaskAction::Show { id, path } => task::show(&path, &id),
            TaskAction::Close {
                ids,
                path,
                reason,
                agent,
            } => task::close(&path, &ids, &reason, &agent),
            TaskAction::Update {
                id,
                path,
                status,
                agent,
            } => task::update(&path, &id, &status, &agent),
            TaskAction::Dep {
                from,
                to,
                path,
                agent,
            } => task::dep_add(&path, &from, &to, &agent),
            TaskAction::Import { path, beads, agent } => task::import_beads(&path, &beads, &agent),
        },
        Command::Config {
            key,
            value,
            reset,
            path,
            agent,
        } => {
            let cmd_output = config::run(&path, key.as_deref(), value.as_deref(), reset, &agent)?;
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Shell { path } => shell::run(&path),
        Command::Mcp { action } => match action {
            McpAction::Serve { path } => {
                mcp::serve(&path)?;
                Ok(String::new())
            }
        },

        // ── Shortcuts (WP6: delegates to existing handlers) ──────────
        Command::Next { path, skip } => {
            let cmd_output = task::next(&path, skip.as_deref())?;
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Done { ids, path, agent } => task::close(&path, &ids, "completed", &agent),
        Command::Note {
            text,
            confidence,
            path,
            agent,
        } => {
            let cmd_output = observe::run(observe::ObserveArgs {
                path: &path,
                text: &text,
                confidence,
                tags: &[],
                category: Some("observation"),
                agent: &agent,
                relates_to: None,
                rationale: None,
                alternatives: None,
            })?;
            return Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ));
        }
        Command::Go { id, path, agent } => task::update(&path, &id, "in-progress", &agent),
        Command::Spec { action } => match action {
            SpecAction::Create {
                id,
                title,
                path,
                statement,
                falsification,
                problem,
                decision,
                traces_to,
                confidence,
                agent,
            } => spec::run_create(spec::CreateArgs {
                path: &path,
                id: &id,
                title: &title,
                statement: statement.as_deref(),
                falsification: falsification.as_deref(),
                problem: problem.as_deref(),
                decision: decision.as_deref(),
                traces_to: traces_to.as_deref(),
                confidence,
                agent: &agent,
            }),
            SpecAction::Review { path } => {
                let cmd_output = spec::run_review(&path)?;
                return Ok(maybe_inject_footer(
                    cmd_output,
                    skip_footer,
                    path_for_footer.as_deref(),
                    budget_ctx,
                ));
            }
            SpecAction::Accept { id, path, agent } => {
                let cmd_output = spec::run_accept(&path, &id, &agent)?;
                return Ok(maybe_inject_footer(
                    cmd_output,
                    skip_footer,
                    path_for_footer.as_deref(),
                    budget_ctx,
                ));
            }
            SpecAction::Reject {
                id,
                reason,
                path,
                agent,
            } => {
                let cmd_output = spec::run_reject(&path, &id, &reason, &agent)?;
                return Ok(maybe_inject_footer(
                    cmd_output,
                    skip_footer,
                    path_for_footer.as_deref(),
                    budget_ctx,
                ));
            }
            SpecAction::History { path } => {
                let cmd_output = spec::run_history(&path)?;
                return Ok(maybe_inject_footer(
                    cmd_output,
                    skip_footer,
                    path_for_footer.as_deref(),
                    budget_ctx,
                ));
            }
        },
    };

    // INV-GUIDANCE-001: Inject guidance footer into applicable command outputs.
    // INV-BUDGET-004: Footer compressed by k*_eff from budget_ctx.
    match result {
        Ok(output) => {
            let cmd_output = CommandOutput::from_human(output);
            Ok(maybe_inject_footer(
                cmd_output,
                skip_footer,
                path_for_footer.as_deref(),
                budget_ctx,
            ))
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::OutputMode;

    #[test]
    fn try_build_footer_on_missing_store_returns_none() {
        let ctx = BudgetCtx::from_flags(None, None);
        let result = try_build_footer(Path::new("/nonexistent/.braid"), &ctx);
        assert!(result.is_none(), "Missing store should return None");
    }

    #[test]
    fn is_json_output_detects_all_json_variants() {
        let h = OutputMode::Human; // default mode for testing per-command flags
        let query_json = Command::Query {
            path: PathBuf::from(".braid"),
            entity: None,
            attribute: None,
            datalog: None,
            positional_datalog: None,
            frontier: None,
            json: true,
        };
        assert!(is_json_output(&query_json, h));

        let query_no_json = Command::Query {
            path: PathBuf::from(".braid"),
            entity: None,
            attribute: None,
            datalog: None,
            positional_datalog: None,
            frontier: None,
            json: false,
        };
        assert!(!is_json_output(&query_no_json, h));

        let status_json = Command::Status {
            path: PathBuf::from(".braid"),
            json: true,
            verbose: false,
            deep: false,
            spectral: false,
            full: false,
            verify: false,
            agent: "test".into(),
            commit: false,
        };
        assert!(is_json_output(&status_json, h));

        let log_json = Command::Log {
            path: PathBuf::from(".braid"),
            limit: 20,
            agent: None,
            datoms: false,
            json: true,
            verbose: false,
        };
        assert!(is_json_output(&log_json, h));

        let seed_json = Command::Seed {
            path: PathBuf::from(".braid"),
            task: None,
            seed_budget: 2000,
            agent: "test".into(),
            for_human: false,
            json: true,
            agent_md: false,
            compact: false,
            inject: None,
        };
        assert!(is_json_output(&seed_json, h));

        // Phase 0A fix: Bilateral --json must be detected (was missing).
        let bilateral_json = Command::Bilateral {
            path: PathBuf::from(".braid"),
            full: false,
            spectral: false,
            history: false,
            json: true,
            commit: false,
            agent: "test".into(),
        };
        assert!(is_json_output(&bilateral_json, h));
    }

    #[test]
    fn is_json_output_false_for_non_json_commands() {
        let h = OutputMode::Human;
        let status_no_json = Command::Status {
            path: PathBuf::from(".braid"),
            json: false,
            verbose: false,
            deep: false,
            spectral: false,
            full: false,
            verify: false,
            agent: "test".into(),
            commit: false,
        };
        assert!(!is_json_output(&status_no_json, h));

        let harvest = Command::Harvest {
            path: PathBuf::from(".braid"),
            agent: "test".into(),
            task: None,
            knowledge: vec![],
            commit: false,
            force: false,
            guard: false,
        };
        assert!(!is_json_output(&harvest, h));

        let init = Command::Init {
            path: PathBuf::from(".braid"),
            spec_dir: PathBuf::from("spec"),
        };
        assert!(!is_json_output(&init, h));
    }

    #[test]
    fn is_json_output_global_mode_overrides() {
        // Even if per-command --json is false, global OutputMode::Json wins
        let status_no_json = Command::Status {
            path: PathBuf::from(".braid"),
            json: false,
            verbose: false,
            deep: false,
            spectral: false,
            full: false,
            verify: false,
            agent: "test".into(),
            commit: false,
        };
        assert!(is_json_output(&status_no_json, OutputMode::Json));
    }

    #[test]
    fn is_generative_output_detects_seed() {
        let seed = Command::Seed {
            path: PathBuf::from(".braid"),
            task: None,
            seed_budget: 2000,
            agent: "test".into(),
            for_human: false,
            json: false,
            agent_md: false,
            compact: false,
            inject: None,
        };
        assert!(is_generative_output(&seed));
    }

    #[test]
    fn is_generative_output_detects_export() {
        let export = Command::Write {
            action: WriteAction::Export {
                path: PathBuf::from(".braid"),
                output: PathBuf::from("spec"),
                namespace: None,
            },
        };
        assert!(is_generative_output(&export));
    }

    #[test]
    fn is_generative_output_false_for_non_generative() {
        let query = Command::Query {
            path: PathBuf::from(".braid"),
            entity: None,
            attribute: None,
            datalog: None,
            positional_datalog: None,
            frontier: None,
            json: false,
        };
        assert!(!is_generative_output(&query));

        let observe = Command::Observe {
            text: "test".into(),
            confidence: 0.7,
            tag: vec![],
            category: None,
            path: PathBuf::from(".braid"),
            agent: "test".into(),
            relates_to: None,
            rationale: None,
            alternatives: None,
        };
        assert!(!is_generative_output(&observe));
    }

    #[test]
    fn shell_is_guidance_command() {
        let shell = Command::Shell {
            path: PathBuf::from(".braid"),
        };
        assert!(is_guidance_command(&shell));
    }

    #[test]
    fn non_shell_is_not_guidance_command() {
        let status = Command::Status {
            path: PathBuf::from(".braid"),
            json: false,
            verbose: false,
            deep: false,
            spectral: false,
            full: false,
            verify: false,
            agent: "test".into(),
            commit: false,
        };
        assert!(!is_guidance_command(&status));
    }

    #[test]
    fn store_path_extracts_correctly() {
        let status = Command::Status {
            path: PathBuf::from("/tmp/test/.braid"),
            json: false,
            verbose: false,
            deep: false,
            spectral: false,
            full: false,
            verify: false,
            agent: "test".into(),
            commit: false,
        };
        assert_eq!(store_path(&status), Some(Path::new("/tmp/test/.braid")));

        let query = Command::Query {
            path: PathBuf::from("/data/.braid"),
            entity: None,
            attribute: None,
            datalog: None,
            positional_datalog: None,
            frontier: None,
            json: false,
        };
        assert_eq!(store_path(&query), Some(Path::new("/data/.braid")));

        let write_assert = Command::Write {
            action: WriteAction::Assert {
                path: PathBuf::from("/w/.braid"),
                agent: "test".into(),
                rationale: "test".into(),
                datoms: vec![],
            },
        };
        assert_eq!(store_path(&write_assert), Some(Path::new("/w/.braid")));
    }

    #[test]
    fn store_path_returns_none_for_mcp() {
        let mcp = Command::Mcp {
            action: McpAction::Serve {
                path: PathBuf::from(".braid"),
            },
        };
        assert_eq!(store_path(&mcp), None);
    }

    // ── INV-INTERFACE-011 audit: CLI surface as optimized prompt ─────────
    // Every command must be verified for LLM-friendly output:
    //   1. Guidance footer coverage (INV-GUIDANCE-001)
    //   2. Help text with demonstrations (ADR-INTERFACE-002)
    //   3. Error messages with four-part protocol (INV-INTERFACE-009)
    //   4. Terse by default, verbose opt-in

    #[test]
    fn audit_all_commands_have_help_text() {
        // INV-INTERFACE-011: every subcommand must have discoverable help text
        // so LLM agents can self-orient from --help output alone.
        use clap::CommandFactory;
        let app = crate::Cli::command();

        for subcmd in app.get_subcommands() {
            let name = subcmd.get_name().to_string();
            let has_help = subcmd.get_after_long_help().is_some()
                || subcmd.get_long_about().is_some()
                || subcmd.get_about().is_some();
            assert!(
                has_help,
                "Command '{name}' must have help text (INV-INTERFACE-011)"
            );
        }
    }

    #[test]
    fn audit_top_level_commands_have_examples() {
        // ADR-INTERFACE-002: commands should have demonstrations in help.
        // Top-level commands that agents use directly must include Examples
        // in after_long_help. Structural subcommands (mcp) are exempt.
        use clap::CommandFactory;
        let app = crate::Cli::command();

        let exempt = ["mcp", "merge"]; // structural/admin, not agent-facing
        for subcmd in app.get_subcommands() {
            let name = subcmd.get_name().to_string();
            if exempt.contains(&name.as_str()) {
                continue;
            }
            let has_examples = subcmd
                .get_after_long_help()
                .map(|h| h.to_string().contains("Examples"))
                .unwrap_or(false);
            assert!(
                has_examples,
                "Command '{name}' must have Examples in help (ADR-INTERFACE-002)"
            );
        }
    }

    #[test]
    fn audit_footer_injection_coverage() {
        // INV-GUIDANCE-001: all non-JSON, non-generative, non-guidance commands
        // must receive a guidance footer. Verify the skip predicates are correct.

        // Commands that MUST skip footers:
        let json_query = Command::Query {
            path: PathBuf::from(".braid"),
            entity: None,
            attribute: None,
            datalog: None,
            positional_datalog: None,
            frontier: None,
            json: true,
        };
        let h = OutputMode::Human;
        assert!(
            is_json_output(&json_query, h),
            "JSON query must skip footer"
        );

        let seed = Command::Seed {
            path: PathBuf::from(".braid"),
            task: None,
            seed_budget: 2000,
            agent: "test".into(),
            for_human: false,
            json: false,
            agent_md: false,
            compact: false,
            inject: None,
        };
        assert!(
            is_generative_output(&seed),
            "Seed must skip footer (generative)"
        );

        // Commands that MUST get footers:
        let observe = Command::Observe {
            path: PathBuf::from(".braid"),
            text: "test".into(),
            confidence: 0.7,
            tag: vec![],
            category: None,
            agent: "test".into(),
            relates_to: None,
            rationale: None,
            alternatives: None,
        };
        assert!(!is_json_output(&observe, h), "Observe must get footer");
        assert!(!is_generative_output(&observe), "Observe must get footer");
        assert!(!is_guidance_command(&observe), "Observe must get footer");

        let harvest = Command::Harvest {
            path: PathBuf::from(".braid"),
            agent: "test".into(),
            task: None,
            knowledge: vec![],
            commit: false,
            force: false,
            guard: false,
        };
        assert!(!is_json_output(&harvest, h), "Harvest must get footer");
        assert!(!is_generative_output(&harvest), "Harvest must get footer");
    }

    #[test]
    fn audit_write_subcommands_have_examples() {
        // ADR-INTERFACE-002: write subcommands are the primary structured
        // mutation surface; each must have Examples for agent discoverability.
        use clap::CommandFactory;
        let app = crate::Cli::command();

        let write_cmd = app
            .get_subcommands()
            .find(|c| c.get_name() == "write")
            .expect("write command must exist");

        for subcmd in write_cmd.get_subcommands() {
            let name = subcmd.get_name().to_string();
            let has_examples = subcmd
                .get_after_long_help()
                .map(|h| h.to_string().contains("Examples"))
                .unwrap_or(false);
            assert!(
                has_examples,
                "Write subcommand '{name}' must have Examples (ADR-INTERFACE-002)"
            );
        }
    }

    #[test]
    fn audit_terse_default_verbose_opt_in() {
        // INV-INTERFACE-011: output must be terse by default, verbose opt-in.
        // Commands with verbosity controls must default to terse (verbose=false).
        use clap::CommandFactory;
        let app = crate::Cli::command();

        // Status has --verbose and --deep flags (both default false).
        let status_cmd = app
            .get_subcommands()
            .find(|c| c.get_name() == "status")
            .expect("status command must exist");

        let verbose_arg = status_cmd.get_arguments().find(|a| a.get_id() == "verbose");
        assert!(
            verbose_arg.is_some(),
            "Status must have --verbose flag for opt-in verbosity"
        );

        // Log has --verbose flag.
        let log_cmd = app
            .get_subcommands()
            .find(|c| c.get_name() == "log")
            .expect("log command must exist");

        let log_verbose = log_cmd.get_arguments().find(|a| a.get_id() == "verbose");
        assert!(
            log_verbose.is_some(),
            "Log must have --verbose flag for opt-in verbosity"
        );
    }

    #[test]
    fn audit_all_store_commands_have_path_flag() {
        // INV-INTERFACE-011: every store-using command must accept --path
        // so agents can specify non-default store locations.
        // Verify via the store_path() extractor: every command except Mcp
        // must return Some.

        let commands_with_paths = [
            Command::Init {
                path: PathBuf::from(".braid"),
                spec_dir: PathBuf::from("spec"),
            },
            Command::Status {
                path: PathBuf::from(".braid"),
                json: false,
                verbose: false,
                deep: false,
                spectral: false,
                full: false,
                verify: false,
                agent: "test".into(),
                commit: false,
            },
            Command::Query {
                path: PathBuf::from(".braid"),
                entity: None,
                attribute: None,
                datalog: None,
                positional_datalog: None,
                frontier: None,
                json: false,
            },
            Command::Harvest {
                path: PathBuf::from(".braid"),
                agent: "test".into(),
                task: None,
                knowledge: vec![],
                commit: false,
                force: false,
                guard: false,
            },
            Command::Seed {
                path: PathBuf::from(".braid"),
                task: None,
                seed_budget: 2000,
                agent: "test".into(),
                for_human: false,
                json: false,
                agent_md: false,
                compact: false,
                inject: None,
            },
            Command::Log {
                path: PathBuf::from(".braid"),
                limit: 20,
                agent: None,
                datoms: false,
                json: false,
                verbose: false,
            },
            Command::Observe {
                path: PathBuf::from(".braid"),
                text: "test".into(),
                confidence: 0.7,
                tag: vec![],
                category: None,
                agent: "test".into(),
                relates_to: None,
                rationale: None,
                alternatives: None,
            },
            Command::Schema {
                path: PathBuf::from(".braid"),
                pattern: None,
                verbose: false,
                json: false,
            },
            Command::Shell {
                path: PathBuf::from(".braid"),
            },
            Command::Merge {
                path: PathBuf::from(".braid"),
                source: PathBuf::from("/tmp"),
            },
        ];

        for cmd in &commands_with_paths {
            assert!(
                store_path(cmd).is_some(),
                "Store-using command must have extractable path (INV-INTERFACE-011)"
            );
        }
    }

    #[test]
    fn footer_skip_logic_is_consistent() {
        let h = OutputMode::Human;
        // JSON output should skip footer
        let json_cmd = Command::Query {
            path: PathBuf::from(".braid"),
            entity: None,
            attribute: None,
            datalog: None,
            positional_datalog: None,
            frontier: None,
            json: true,
        };
        assert!(
            is_json_output(&json_cmd, h),
            "JSON commands must skip footer to avoid corrupting output"
        );

        // Guidance commands should skip footer (avoid double injection)
        let guidance_cmd = Command::Shell {
            path: PathBuf::from(".braid"),
        };
        assert!(
            is_guidance_command(&guidance_cmd),
            "Guidance commands must skip footer to avoid duplication"
        );

        // Generative output should skip footer (avoid corrupting piped output)
        let gen_cmd = Command::Seed {
            path: PathBuf::from(".braid"),
            task: None,
            seed_budget: 2000,
            agent: "test".into(),
            for_human: false,
            json: false,
            agent_md: false,
            compact: false,
            inject: None,
        };
        assert!(
            is_generative_output(&gen_cmd),
            "Generative commands must skip footer to avoid corrupting files"
        );
    }

    // ---- BudgetCtx tests (INV-BUDGET-001, INV-BUDGET-004) ----

    #[test]
    fn budget_ctx_default_full_quality() {
        let ctx = BudgetCtx::from_flags(None, None);
        assert!(
            (ctx.k_eff() - 1.0).abs() < 1e-10,
            "Default budget should be full quality"
        );
        assert_eq!(ctx.manager.output_budget, 10000);
    }

    #[test]
    fn budget_ctx_from_context_used() {
        let ctx = BudgetCtx::from_flags(None, Some(0.5));
        assert!(
            (ctx.k_eff() - 0.5).abs() < 1e-10,
            "k_eff should be 1.0 - 0.5 = 0.5"
        );
        // k=0.5 → linear decay = 0.5/0.6 ≈ 0.833
        // Q = 0.5 * 0.833 ≈ 0.417 → budget ≈ 4166
        assert!(ctx.manager.output_budget > 4000);
        assert!(ctx.manager.output_budget < 4500);
    }

    #[test]
    fn budget_ctx_from_explicit_budget() {
        let ctx = BudgetCtx::from_flags(Some(500), None);
        assert_eq!(
            ctx.manager.output_budget, 500,
            "Explicit budget should be respected"
        );
    }

    #[test]
    fn budget_ctx_explicit_budget_floors_at_min() {
        let ctx = BudgetCtx::from_flags(Some(10), None);
        assert_eq!(
            ctx.manager.output_budget,
            braid_kernel::budget::MIN_OUTPUT,
            "Budget below MIN_OUTPUT should floor at MIN_OUTPUT"
        );
    }

    #[test]
    fn budget_ctx_context_used_clamps() {
        // Over 100% consumed → k_eff = 0
        let ctx = BudgetCtx::from_flags(None, Some(1.5));
        assert!(
            (ctx.k_eff() - 0.0).abs() < 1e-10,
            "Over-consumed context should clamp to k_eff=0"
        );
        assert_eq!(ctx.manager.output_budget, braid_kernel::budget::MIN_OUTPUT);
    }

    #[test]
    fn budget_ctx_context_used_produces_correct_guidance_level() {
        use braid_kernel::budget::GuidanceLevel;

        // Full quality
        let ctx_full = BudgetCtx::from_flags(None, Some(0.1)); // k=0.9
        assert_eq!(ctx_full.manager.guidance_level(), GuidanceLevel::Full);

        // Compressed
        let ctx_comp = BudgetCtx::from_flags(None, Some(0.5)); // k=0.5
        assert_eq!(ctx_comp.manager.guidance_level(), GuidanceLevel::Compressed);

        // Minimal
        let ctx_min = BudgetCtx::from_flags(None, Some(0.75)); // k=0.25
        assert_eq!(ctx_min.manager.guidance_level(), GuidanceLevel::Minimal);

        // Harvest only
        let ctx_harv = BudgetCtx::from_flags(None, Some(0.9)); // k=0.1
        assert_eq!(
            ctx_harv.manager.guidance_level(),
            GuidanceLevel::HarvestOnly
        );
    }

    // ---- Budget gate tests (W2C.2 + W2C.3: enforce_ceiling wiring) ----

    // Verifies: INV-BUDGET-001 — Budget gate truncates long output
    #[test]
    fn budget_gate_truncates_long_output() {
        use crate::output::{AgentOutput, CommandOutput};

        let long_text = "word ".repeat(400); // ~2000 chars → ~500 tokens
        let co = CommandOutput {
            json: serde_json::json!({"data": &long_text}),
            agent: AgentOutput {
                context: String::new(),
                content: long_text.clone(),
                footer: String::new(),
            },
            human: long_text.clone(),
        };

        // Explicit budget of 50 tokens — forces truncation.
        let ctx = BudgetCtx::from_flags(Some(50), None);
        let gated = apply_budget_gate(co, OutputMode::Human, &ctx);

        assert!(
            gated.human.len() < long_text.len(),
            "human output should be truncated (got {} vs original {})",
            gated.human.len(),
            long_text.len()
        );
        assert!(
            gated.human.contains("[...truncated:"),
            "truncated output must contain truncation notice"
        );
    }

    // Verifies: INV-BUDGET-001 — Short output passes through unchanged
    #[test]
    fn budget_gate_passthrough_for_short_output() {
        use crate::output::{AgentOutput, CommandOutput};

        let short_text = "ok: 3 datoms".to_string(); // ~3 tokens
        let co = CommandOutput {
            json: serde_json::json!({"status": "ok"}),
            agent: AgentOutput {
                context: String::new(),
                content: short_text.clone(),
                footer: String::new(),
            },
            human: short_text.clone(),
        };

        // Even a tight budget (50 tokens) is plenty for 3 tokens of output.
        let ctx = BudgetCtx::from_flags(Some(50), None);
        let gated = apply_budget_gate(co, OutputMode::Human, &ctx);

        assert_eq!(
            gated.human, short_text,
            "short output must pass through unchanged"
        );
        assert_eq!(
            gated.agent.content, short_text,
            "short agent content must pass through unchanged"
        );
    }

    // Verifies: INV-BUDGET-001 — JSON output is NEVER truncated
    #[test]
    fn budget_gate_json_never_truncated() {
        use crate::output::{AgentOutput, CommandOutput};

        let long_text = "word ".repeat(400); // ~500 tokens
        let json_data = serde_json::json!({"results": [1, 2, 3], "detail": &long_text});
        let co = CommandOutput {
            json: json_data.clone(),
            agent: AgentOutput {
                context: String::new(),
                content: long_text.clone(),
                footer: String::new(),
            },
            human: long_text.clone(),
        };

        // Very tight budget (50 tokens) — JSON must still be untouched.
        let ctx = BudgetCtx::from_flags(Some(50), None);
        let gated = apply_budget_gate(co, OutputMode::Json, &ctx);

        assert_eq!(gated.json, json_data, "JSON output must never be truncated");
        // When mode is JSON, human/agent should also be untouched (no processing).
        assert_eq!(
            gated.human, long_text,
            "in JSON mode, human field should be untouched"
        );
    }

    // Verifies: INV-BUDGET-001 — Agent mode truncation works
    #[test]
    fn budget_gate_truncates_agent_mode() {
        use crate::output::{AgentOutput, CommandOutput};

        let long_content = "entity ".repeat(300); // ~525 tokens
        let co = CommandOutput {
            json: serde_json::json!({"data": "irrelevant"}),
            agent: AgentOutput {
                context: "store: 9k datoms".into(),
                content: long_content.clone(),
                footer: "next: braid status".into(),
            },
            human: "short human".into(),
        };

        let ctx = BudgetCtx::from_flags(Some(50), None);
        let gated = apply_budget_gate(co, OutputMode::Agent, &ctx);

        // Agent output was truncated — the content field now holds the gated text.
        assert!(
            gated.agent.content.contains("[...truncated:")
                || gated.agent.render().len() < long_content.len(),
            "agent output should be truncated when over budget"
        );
    }

    // Verifies: INV-BUDGET-001 — Default full budget does not truncate
    #[test]
    fn budget_gate_default_full_budget_passthrough() {
        use crate::output::{AgentOutput, CommandOutput};

        // Default budget is 10000 tokens — typical output is well under that.
        let text = "store: 9314 datoms, 1563 entities. F(S)=0.77".to_string();
        let co = CommandOutput {
            json: serde_json::json!({"status": "ok"}),
            agent: AgentOutput {
                context: String::new(),
                content: text.clone(),
                footer: String::new(),
            },
            human: text.clone(),
        };

        let ctx = BudgetCtx::from_flags(None, None); // default = 10000 tokens
        let gated = apply_budget_gate(co, OutputMode::Human, &ctx);

        assert_eq!(
            gated.human, text,
            "default budget should not truncate normal output"
        );
    }
}

// Merge stays in its own section (rarely used, distinct semantics).
mod merge {
    use std::path::Path;

    use braid_kernel::merge::{verify_frontier_advancement, verify_monotonicity};

    use crate::error::BraidError;
    use crate::layout::DiskLayout;

    pub fn run(path: &Path, source_path: &Path) -> Result<String, BraidError> {
        let layout = DiskLayout::open(path)?;
        let mut store = layout.load_store()?;

        let source_layout = DiskLayout::open(source_path)?;
        let source = source_layout.load_store()?;

        let pre_datoms = store.datom_set().clone();
        let pre_frontier = store.frontier().clone();
        let pre_len = store.len();

        let receipt = store.merge(&source);

        let monotonic = verify_monotonicity(&pre_datoms, store.datom_set());
        let frontier_advanced = verify_frontier_advancement(&pre_frontier, store.frontier());

        let source_hashes = source_layout.list_tx_hashes()?;
        let our_hashes: std::collections::HashSet<String> =
            layout.list_tx_hashes()?.into_iter().collect();
        let mut new_files = 0;
        for hash in &source_hashes {
            if !our_hashes.contains(hash) {
                let tx = source_layout.read_tx(hash)?;
                layout.write_tx(&tx)?;
                new_files += 1;
            }
        }

        let mut out = String::new();
        out.push_str(&format!(
            "merge: {} \u{2192} {}\n",
            source_path.display(),
            path.display()
        ));
        out.push_str(&format!(
            "  datoms: {} \u{2192} {} (+{})\n",
            pre_len,
            store.len(),
            receipt.new_datoms
        ));
        out.push_str(&format!("  new tx files: {new_files}\n"));
        out.push_str(&format!(
            "  frontier agents: {} \u{2192} {}\n",
            pre_frontier.len(),
            store.frontier().len()
        ));
        out.push_str(&format!(
            "  monotonicity: {}\n",
            if monotonic { "OK" } else { "VIOLATED" }
        ));
        out.push_str(&format!(
            "  frontier advancement: {}\n",
            if frontier_advanced { "OK" } else { "NO CHANGE" }
        ));

        Ok(out)
    }
}
