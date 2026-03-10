//! CLI command definitions and dispatch.

use std::path::{Path, PathBuf};

use clap::Subcommand;

pub(crate) mod analyze;
mod harvest;
mod init;
mod log;
pub(crate) mod observe;
mod query;
mod seed;
pub(crate) mod shell;
mod status;
pub(crate) mod write;

// Re-export mcp serve as a special case (runs an event loop, not a single command).
pub use crate::mcp;

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
  braid init --spec-dir spec    # auto-bootstrap spec elements")]
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
  braid observe \"query returns wrong results\" --confidence 0.3 --category conjecture")]
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
  braid query --datalog '[:find ?e :where [?e :exploration/body _]]'  # Datalog (explicit)")]
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
  braid status --json                  # structured output")]
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
  braid log --datoms                   # show individual datoms")]
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
  braid harvest --commit --force                          # bypass crystallization guard
  braid harvest --knowledge gap \"missing join optimization\" --commit")]
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
        #[arg(long)]
        commit: bool,

        /// Bypass crystallization guard (commit all candidates regardless of stability).
        #[arg(long, short = 'f')]
        force: bool,
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
fn try_append_footer(output: String, path: &Path) -> String {
    let Ok(layout) = crate::layout::DiskLayout::open(path) else {
        return output;
    };
    let Ok(store) = layout.load_store() else {
        return output;
    };

    let footer = braid_kernel::guidance::build_command_footer(&store, None);
    format!("{output}{footer}\n")
}

/// Execute a CLI command and return the output string.
pub fn run(cmd: Command) -> Result<String, crate::error::BraidError> {
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
        Command::Harvest {
            path,
            agent,
            task,
            knowledge,
            commit,
            force,
        } => harvest::run(&path, &agent, task.as_deref(), &knowledge, commit, force),
        Command::Seed {
            path,
            task,
            budget,
            agent,
            for_human,
            json,
            agent_md,
            compact,
        } => {
            let effective_budget = if compact { 200 } else { budget };
            let effective_task = task.as_deref().unwrap_or("continue");
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
        } => observe::run(observe::ObserveArgs {
            path: &path,
            text: &text,
            confidence,
            tags: &tag,
            category: category.as_deref(),
            agent: &agent,
            relates_to: relates_to.as_deref(),
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
    match (result, path_for_footer) {
        (Ok(output), Some(path)) if !skip_footer => Ok(try_append_footer(output, &path)),
        (result, _) => result,
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
