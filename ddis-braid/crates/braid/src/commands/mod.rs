//! CLI command definitions and dispatch.

use std::path::{Path, PathBuf};

use braid_kernel::budget::{self, BudgetManager};
use clap::Subcommand;

pub(crate) mod analyze;
mod harvest;
mod init;
mod log;
pub(crate) mod observe;
mod query;
mod schema;
mod seed;
pub(crate) mod shell;
mod status;
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
    /// Create a new .braid store with schema datoms.
    ///
    /// Run once per project. Creates the directory and transacts all schema
    /// attributes (Layer 1-3). If a spec/ directory exists, auto-bootstraps
    /// spec elements into the store.
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
    /// Capture a knowledge observation as an exploration entity.
    ///
    /// Fastest way to record what you learned. Creates a content-addressed
    /// entity with :exploration/* attributes. Use instead of write for
    /// knowledge capture.
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
    /// Query the store: entity/attribute filter or Datalog.
    ///
    /// Three modes: (1) entity filter (--entity), (2) attribute filter (--attribute),
    /// (3) Datalog -- pass as positional arg or --datalog flag.
    /// Datalog auto-detected when arg starts with "[:find".
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

        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Show store status, coherence, methodology, and next actions.
    ///
    /// Default: terse 6-line dashboard with top action.
    /// --verbose: full methodology breakdown, all actions.
    /// --deep: bilateral F(S) fitness, graph analytics, convergence.
    /// --verify: check on-disk integrity (content hashes).
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

        /// Include spectral certificate (with --deep).
        #[arg(long)]
        spectral: bool,

        /// Full 14-algorithm dashboard (with --deep).
        #[arg(long)]
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

    /// Browse transaction log with optional agent filter.
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

    /// Inspect the datom store schema: attributes, types, cardinality.
    ///
    /// Lists all known attributes with their type, cardinality, and documentation.
    /// Optimized for AI agent consumption: provides the information needed to
    /// write correct queries and transactions (INV-INTERFACE-011).
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
    /// End-of-session: extract knowledge and commit discoveries.
    ///
    /// Scores knowledge items by novelty, specificity, and relevance.
    /// Use --commit to persist approved candidates as datoms.
    /// Crystallization guard (INV-HARVEST-006) gates commitment by stability.
    /// Task auto-detected from active session, git branch, or recent tx rationales.
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

    /// Start-of-session: assemble relevant context from the store.
    ///
    /// Produces a token-budgeted context document with the most relevant
    /// entities, recent transactions, and methodology guidance for the task.
    /// Creates an active session entity for harvest auto-detection.
    /// Task auto-detected from last session if omitted.
    #[command(after_long_help = "\
Examples:
  braid seed --task \"fix query engine joins\" --budget 3000
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

        /// Token budget for output.
        #[arg(long, short = 'b', default_value = "2000")]
        budget: usize,

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

/// Extract the store path from a command variant (if the command uses a store).
fn store_path(cmd: &Command) -> Option<&Path> {
    match cmd {
        Command::Mcp { .. } => None,
        Command::Init { path, .. }
        | Command::Status { path, .. }
        | Command::Query { path, .. }
        | Command::Schema { path, .. }
        | Command::Harvest { path, .. }
        | Command::Seed { path, .. }
        | Command::Merge { path, .. }
        | Command::Log { path, .. }
        | Command::Observe { path, .. }
        | Command::Shell { path, .. } => Some(path),
        Command::Write { action } => match action {
            WriteAction::Assert { path, .. }
            | WriteAction::Retract { path, .. }
            | WriteAction::Promote { path, .. }
            | WriteAction::Export { path, .. } => Some(path),
        },
    }
}

/// Whether a command produces JSON output (footers must not corrupt JSON).
fn is_json_output(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Query { json: true, .. }
            | Command::Status { json: true, .. }
            | Command::Schema { json: true, .. }
            | Command::Log { json: true, .. }
            | Command::Seed { json: true, .. }
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
            | Command::Write {
                action: WriteAction::Export { .. }
            }
    )
}

/// Try to append a guidance footer to command output (INV-GUIDANCE-001).
///
/// Best-effort: if the store can't be loaded, returns the original output
/// unchanged. Skips footer for JSON, guidance, and generative commands.
///
/// INV-BUDGET-004: Guidance footer compresses by k*_eff level from `budget_ctx`.
fn try_append_footer(output: String, path: &Path, budget_ctx: &BudgetCtx) -> String {
    let Ok(layout) = crate::layout::DiskLayout::open(path) else {
        return output;
    };
    let Ok(store) = layout.load_store() else {
        return output;
    };

    let footer = braid_kernel::guidance::build_command_footer(&store, Some(budget_ctx.k_eff()));
    format!("{output}{footer}\n")
}

/// Execute a CLI command and return the output string.
///
/// INV-BUDGET-001: Output respects `budget_ctx` for guidance footer compression
/// and command attention profiles. Commands that exceed their attention profile
/// ceiling are truncated.
pub fn run(cmd: Command, budget_ctx: &BudgetCtx) -> Result<String, crate::error::BraidError> {
    // Pre-extract metadata needed for footer injection (before cmd is consumed).
    let path_for_footer = store_path(&cmd).map(|p| p.to_path_buf());
    let skip_footer =
        is_json_output(&cmd) || is_guidance_command(&cmd) || is_generative_output(&cmd);

    let result = match cmd {
        Command::Init { path, spec_dir } => init::run(&path, &spec_dir),
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
        } => status::run(
            &path, &agent, json, verbose, deep, spectral, full, verify, commit,
        ),
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
            json,
        } => {
            let dq = datalog.or(positional_datalog);
            if let Some(ref dq) = dq {
                query::run_datalog(&path, dq, json)
            } else {
                query::run(&path, entity.as_deref(), attribute.as_deref(), json)
            }
        }
        Command::Schema {
            path,
            pattern,
            verbose,
            json,
        } => schema::run(&path, pattern.as_deref(), verbose, json),
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
            harvest::run(
                &path,
                &agent,
                task.as_deref(),
                &knowledge,
                commit,
                effective_force,
            )
        }
        Command::Seed {
            path,
            task,
            budget,
            agent,
            for_human,
            json,
            agent_md,
            compact,
            inject,
        } => {
            let mut effective_budget = if compact { 200 } else { budget };
            // INV-BUDGET-001: Global budget acts as ceiling for seed output.
            // Seed's own --budget controls content assembly; global --budget
            // is a hard cap from the caller's remaining context window.
            effective_budget = effective_budget.min(budget_ctx.manager.output_budget as usize);
            let effective_task = task.as_deref().unwrap_or("continue");

            // --inject mode: update file in place with seed content (SB.3.3)
            if let Some(ref inject_path) = inject {
                return seed::run_inject(&path, inject_path, effective_task, effective_budget);
            }

            seed::run(
                &path,
                effective_task,
                effective_budget,
                &agent,
                for_human,
                json,
                agent_md,
            )
        }
        Command::Merge { path, source } => merge::run(&path, &source),
        Command::Log {
            path,
            limit,
            agent,
            datoms,
            json,
            verbose,
        } => log::run(&path, limit, agent.as_deref(), datoms, json, verbose),
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
        } => observe::run(observe::ObserveArgs {
            path: &path,
            text: &text,
            confidence,
            tags: &tag,
            category: category.as_deref(),
            agent: &agent,
            relates_to: relates_to.as_deref(),
            rationale: rationale.as_deref(),
            alternatives: alternatives.as_deref(),
        }),
        Command::Shell { path } => shell::run(&path),
        Command::Mcp { action } => match action {
            McpAction::Serve { path } => {
                mcp::serve(&path)?;
                Ok(String::new())
            }
        },
    };

    // INV-GUIDANCE-001: Append guidance footer to applicable command outputs.
    // INV-BUDGET-004: Footer compressed by k*_eff from budget_ctx.
    match (result, path_for_footer) {
        (Ok(output), Some(path)) if !skip_footer => {
            Ok(try_append_footer(output, &path, budget_ctx))
        }
        (result, _) => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_append_footer_on_missing_store_returns_unchanged() {
        let output = "some output".to_string();
        let ctx = BudgetCtx::from_flags(None, None);
        let result = try_append_footer(output.clone(), Path::new("/nonexistent/.braid"), &ctx);
        assert_eq!(
            result, output,
            "Missing store should return original output"
        );
    }

    #[test]
    fn is_json_output_detects_all_json_variants() {
        let query_json = Command::Query {
            path: PathBuf::from(".braid"),
            entity: None,
            attribute: None,
            datalog: None,
            positional_datalog: None,
            json: true,
        };
        assert!(is_json_output(&query_json));

        let query_no_json = Command::Query {
            path: PathBuf::from(".braid"),
            entity: None,
            attribute: None,
            datalog: None,
            positional_datalog: None,
            json: false,
        };
        assert!(!is_json_output(&query_no_json));

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
        assert!(is_json_output(&status_json));

        let log_json = Command::Log {
            path: PathBuf::from(".braid"),
            limit: 20,
            agent: None,
            datoms: false,
            json: true,
            verbose: false,
        };
        assert!(is_json_output(&log_json));

        let seed_json = Command::Seed {
            path: PathBuf::from(".braid"),
            task: None,
            budget: 2000,
            agent: "test".into(),
            for_human: false,
            json: true,
            agent_md: false,
            compact: false,
            inject: None,
        };
        assert!(is_json_output(&seed_json));
    }

    #[test]
    fn is_json_output_false_for_non_json_commands() {
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
        assert!(!is_json_output(&status_no_json));

        let harvest = Command::Harvest {
            path: PathBuf::from(".braid"),
            agent: "test".into(),
            task: None,
            knowledge: vec![],
            commit: false,
            force: false,
            guard: false,
        };
        assert!(!is_json_output(&harvest));

        let init = Command::Init {
            path: PathBuf::from(".braid"),
            spec_dir: PathBuf::from("spec"),
        };
        assert!(!is_json_output(&init));
    }

    #[test]
    fn is_generative_output_detects_seed() {
        let seed = Command::Seed {
            path: PathBuf::from(".braid"),
            task: None,
            budget: 2000,
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
            json: true,
        };
        assert!(is_json_output(&json_query), "JSON query must skip footer");

        let seed = Command::Seed {
            path: PathBuf::from(".braid"),
            task: None,
            budget: 2000,
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
        assert!(!is_json_output(&observe), "Observe must get footer");
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
        assert!(!is_json_output(&harvest), "Harvest must get footer");
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
                budget: 2000,
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
        // JSON output should skip footer
        let json_cmd = Command::Query {
            path: PathBuf::from(".braid"),
            entity: None,
            attribute: None,
            datalog: None,
            positional_datalog: None,
            json: true,
        };
        assert!(
            is_json_output(&json_cmd),
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
            budget: 2000,
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
