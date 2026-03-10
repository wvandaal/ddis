//! CLI command definitions and dispatch.

use std::path::PathBuf;

use clap::Subcommand;

mod analyze;
mod generate;
mod generate_spec;
mod guidance;
mod harvest;
mod init;
mod log;
mod merge;
mod observe;
mod promote;
mod query;
mod retract;
mod seed;
mod status;
mod transact;

// Re-export mcp serve as a special case (runs an event loop, not a single command).
pub use crate::mcp;

/// All subcommands available in the `braid` CLI.
///
/// Commands are organized by workflow phase:
///   SETUP:     init, bootstrap, verify
///   CAPTURE:   observe, transact, retract, promote
///   QUERY:     query, status, log, analyze
///   LIFECYCLE: harvest, seed, guidance, generate
///   ADMIN:     merge, generate-spec, mcp
#[derive(Subcommand)]
pub enum Command {
    // ── SETUP ──────────────────────────────────────────────────────────
    /// Create a new .braid store with schema datoms.
    ///
    /// Run once per project. Creates the directory and transacts all schema
    /// attributes (Layer 1-3). Follow with `bootstrap` to load spec elements.
    #[command(after_long_help = "Example:\n  braid init\n  braid init -p /tmp/mystore/.braid")]
    Init {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,
    },

    // ── CAPTURE ────────────────────────────────────────────────────────
    /// Capture a knowledge observation as an exploration entity.
    ///
    /// Fastest way to record what you learned. Creates a content-addressed
    /// entity with :exploration/* attributes. Use instead of manual transact
    /// for knowledge capture.
    #[command(after_long_help = "\
Examples:
  braid observe \"merge is a bottleneck\" -c 0.8 --tag bottleneck
  braid observe \"CRDT merge is commutative\" --category theorem --relates-to :spec/inv-store-004
  braid observe \"query returns wrong results\" -c 0.3 --category conjecture")]
    Observe {
        /// The observation text.
        text: String,

        /// Epistemic confidence (0.0=uncertain, 1.0=certain).
        #[arg(short, long, default_value = "0.7")]
        confidence: f64,

        /// Tags for filtering (repeatable).
        #[arg(short, long, action = clap::ArgAction::Append)]
        tag: Vec<String>,

        /// Category: observation|conjecture|theorem|definition|algorithm|design-decision|open-question.
        #[arg(long)]
        category: Option<String>,

        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,

        /// Cross-reference to a spec element (e.g., ":spec/inv-store-001").
        #[arg(long)]
        relates_to: Option<String>,
    },

    /// Assert datoms into the store (low-level).
    ///
    /// For structured data. Each -d flag takes 3 args: entity attribute value.
    /// Prefer `observe` for knowledge capture, `transact` for schema/metadata.
    #[command(after_long_help = "\
Example:
  braid transact -r \"add spec element\" -d :spec/inv-store-001 :db/doc \"Append-only store\"")]
    Transact {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,

        /// Why this transaction exists.
        #[arg(short, long)]
        rationale: String,

        /// Datom triples: entity attribute value (repeatable).
        #[arg(short = 'd', long = "datom", num_args = 3, action = clap::ArgAction::Append)]
        datoms: Vec<String>,
    },

    /// Retract assertions (append-only: creates retraction datoms, never deletes).
    #[command(after_long_help = "\
Example:
  braid retract -e :spec/inv-store-001 --attribute :db/doc")]
    Retract {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,

        /// Entity ident (e.g., ":spec/inv-store-001").
        #[arg(short, long)]
        entity: String,

        /// Attribute to retract (e.g., ":db/doc").
        #[arg(long)]
        attribute: String,

        /// Only retract if value matches this.
        #[arg(short, long)]
        value: Option<String>,
    },

    /// Promote an exploration → formal spec element (observation → invariant/ADR/neg).
    #[command(after_long_help = "\
Example:
  braid promote -e :observation/merge-bottleneck --target-id INV-STORE-042 \
    -n STORE -t invariant --statement \"Merge is O(n)\" --falsification \"Merge > O(n log n)\"")]
    Promote {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Entity ident to promote.
        #[arg(short, long)]
        entity: String,

        /// Target spec element ID (e.g., "INV-STORE-042").
        #[arg(long)]
        target_id: String,

        /// Target namespace (e.g., "STORE").
        #[arg(short, long)]
        namespace: String,

        /// Target type: invariant, adr, negative-case.
        #[arg(short = 't', long = "type")]
        target_type: String,

        /// Agent identity.
        #[arg(short, long, default_value = "braid:user")]
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

    // ── QUERY ──────────────────────────────────────────────────────────
    /// Query the store: entity/attribute filter or Datalog.
    ///
    /// Three modes: (1) entity filter (-e), (2) attribute filter (-a),
    /// (3) Datalog (--datalog). Datalog supports variables (?x), keywords (:ns/name),
    /// anonymous wildcard (_), and multi-clause joins.
    #[command(after_long_help = "\
Examples:
  braid query -e :spec/inv-store-001                           # all datoms for entity
  braid query -a :db/doc                                       # all values of attribute
  braid query --datalog '[:find ?e ?v :where [?e :db/doc ?v]]' # Datalog query
  braid query --datalog '[:find ?e :where [?e :exploration/body _]]'  # wildcard")]
    Query {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Filter by entity ident (keyword).
        #[arg(short, long)]
        entity: Option<String>,

        /// Filter by attribute (keyword).
        #[arg(short, long)]
        attribute: Option<String>,

        /// Datalog expression: [:find ?vars :where [clauses]].
        #[arg(long)]
        datalog: Option<String>,

        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Show store summary: datom count, entity count, frontier, schema stats.
    Status {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// Full output with frontier details and per-agent breakdown.
        #[arg(short, long)]
        verbose: bool,
    },

    /// Browse transaction log with optional agent filter.
    #[command(after_long_help = "\
Examples:
  braid log -n 5               # last 5 transactions (terse)
  braid log -v                  # verbose with rationale/provenance
  braid log -a braid:user      # only this agent's transactions
  braid log --datoms            # show individual datoms")]
    Log {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Max transactions to show.
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,

        /// Filter by agent name.
        #[arg(short, long)]
        agent: Option<String>,

        /// Include individual datoms per transaction.
        #[arg(long)]
        datoms: bool,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// Full multi-line output per transaction.
        #[arg(short, long)]
        verbose: bool,
    },

    /// Graph analytics: topology, spectrum, curvature, coherence, actions.
    ///
    /// Default: adaptive output that maximizes information density.
    /// Sections are emitted in priority order (actions > coherence > topology > spectral).
    /// Use --full for the complete 14-algorithm dashboard.
    #[command(after_long_help = "\
Examples:
  braid analyze                # adaptive: best info per token
  braid analyze --budget 200   # explicit token cap
  braid analyze --full         # complete 14-algorithm dashboard
  braid analyze --force        # recompute ignoring cache")]
    Analyze {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Force recomputation (ignore cache).
        #[arg(long)]
        force: bool,

        /// Token budget: limits output to highest-priority sections.
        /// Default: auto-calibrated based on store complexity.
        #[arg(short, long)]
        budget: Option<usize>,

        /// Full 14-algorithm dashboard (verbose).
        #[arg(long)]
        full: bool,

        /// Output as JSON.
        #[arg(long)]
        json: bool,
    },

    // ── LIFECYCLE ──────────────────────────────────────────────────────
    /// End-of-session: extract knowledge gaps and commit discoveries.
    ///
    /// Scores knowledge items by novelty, specificity, and relevance.
    /// Use --commit to persist approved candidates as datoms.
    /// Crystallization guard (INV-HARVEST-006) gates commitment by stability.
    /// Use --force to bypass the crystallization threshold.
    #[command(after_long_help = "\
Examples:
  braid harvest -t \"implemented query engine\" -k gap \"missing join optimization\" --commit
  braid harvest -t \"bugfix\" --commit --force")]
    Harvest {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,

        /// Task description (what you worked on).
        #[arg(short, long)]
        task: String,

        /// Knowledge items: key value (repeatable).
        #[arg(short = 'k', long = "knowledge", num_args = 2, action = clap::ArgAction::Append)]
        knowledge: Vec<String>,

        /// Persist approved candidates to the store.
        #[arg(long)]
        commit: bool,

        /// Bypass crystallization guard (commit all candidates regardless of stability).
        #[arg(short, long)]
        force: bool,
    },

    /// Start-of-session: assemble relevant context from the store.
    ///
    /// Produces a token-budgeted context document with the most relevant
    /// entities, recent transactions, and methodology guidance for the task.
    /// Use --for-human for a narrative briefing instead of structured sections.
    /// Use --agent-md to also generate dynamic AGENTS.md from store state.
    #[command(after_long_help = "\
Examples:
  braid seed -t \"fix query engine joins\" -b 3000
  braid seed -t \"implement harvest\" --for-human
  braid seed -t \"implement harvest\" --agent-md")]
    Seed {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Task description (what you will work on).
        #[arg(short, long)]
        task: String,

        /// Token budget for output.
        #[arg(short, long, default_value = "2000")]
        budget: usize,

        /// Agent identity.
        #[arg(short, long, default_value = "braid:user")]
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
    },

    /// Show coherence metrics and prioritized next actions.
    ///
    /// Default: action-first terse output. Use --verbose for full metrics.
    Guidance {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Agent identity.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,

        /// Output as JSON.
        #[arg(long)]
        json: bool,

        /// Full output with all coherence metrics and methodology components.
        #[arg(short, long)]
        verbose: bool,
    },

    /// Generate dynamic CLAUDE.md/AGENTS.md from store state.
    #[command(after_long_help = "\
Example:
  braid generate -t \"implement harvest pipeline\" -b 4000 > CLAUDE.md")]
    Generate {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Task description.
        #[arg(short, long)]
        task: String,

        /// Token budget for output.
        #[arg(short, long, default_value = "4000")]
        budget: usize,

        /// Agent identity.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,
    },

    // ── ADMIN ──────────────────────────────────────────────────────────
    /// Verify on-disk store integrity (content hashes).
    Verify {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,
    },

    /// Merge another store into this one (CRDT set union, no conflicts).
    Merge {
        /// Target store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Source store to merge from.
        #[arg(short, long)]
        source: PathBuf,
    },

    /// Generate spec/*.md from store entities (inverse of bootstrap).
    GenerateSpec {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Output directory for spec files.
        #[arg(short, long, default_value = "spec")]
        output: PathBuf,

        /// Filter to one namespace (e.g., "STORE"). Omit for all.
        #[arg(short, long)]
        namespace: Option<String>,
    },

    /// Load spec/*.md into the store as datoms (self-bootstrap).
    Bootstrap {
        /// Store directory path.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Spec directory to parse.
        #[arg(short, long, default_value = "spec")]
        spec_dir: PathBuf,
    },

    /// Start MCP server (JSON-RPC over stdio).
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
}

/// MCP server subcommands.
#[derive(Subcommand)]
pub enum McpAction {
    /// Start the MCP server (reads JSON-RPC from stdin, writes to stdout).
    Serve {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,
    },
}

/// Execute a CLI command and return the output string.
pub fn run(cmd: Command) -> Result<String, crate::error::BraidError> {
    match cmd {
        Command::Init { path } => init::run(&path),
        Command::Status {
            path,
            json,
            verbose,
        } => status::run(&path, json, verbose),
        Command::Transact {
            path,
            agent,
            rationale,
            datoms,
        } => transact::run(&path, &agent, &rationale, &datoms),
        Command::Retract {
            path,
            agent,
            entity,
            attribute,
            value,
        } => retract::run(&path, &agent, &entity, &attribute, value.as_deref()),
        Command::Query {
            path,
            entity,
            attribute,
            datalog,
            json,
        } => {
            if let Some(ref dq) = datalog {
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
        } => harvest::run(&path, &agent, &task, &knowledge, commit, force),
        Command::Seed {
            path,
            task,
            budget,
            agent,
            for_human,
            json,
            agent_md,
        } => seed::run(&path, &task, budget, &agent, for_human, json, agent_md),
        Command::Guidance {
            path,
            agent,
            json,
            verbose,
        } => guidance::run(&path, &agent, json, verbose),
        Command::Merge { path, source } => merge::run(&path, &source),
        Command::Log {
            path,
            limit,
            agent,
            datoms,
            json,
            verbose,
        } => log::run(&path, limit, agent.as_deref(), datoms, json, verbose),
        Command::Generate {
            path,
            task,
            budget,
            agent,
        } => generate::run(&path, &task, budget, &agent),
        Command::Promote {
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
        } => promote::run(promote::PromoteArgs {
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
        Command::GenerateSpec {
            path,
            output,
            namespace,
        } => generate_spec::run(&path, &output, namespace.as_deref()),
        Command::Bootstrap { path, spec_dir } => {
            let layout = crate::layout::DiskLayout::open(&path)?;
            let elements = crate::bootstrap::parse_spec_dir(&spec_dir);
            if elements.is_empty() {
                return Ok("bootstrap: no spec elements found\n".to_string());
            }
            let agent = braid_kernel::datom::AgentId::from_name("braid:bootstrap");
            let tx = crate::bootstrap::elements_to_tx(&elements, agent);
            let datom_count = tx.datoms.len();
            let file_path = layout.write_tx(&tx)?;

            let invs = elements
                .iter()
                .filter(|e| e.kind == crate::bootstrap::SpecElementKind::Invariant)
                .count();
            let adrs = elements
                .iter()
                .filter(|e| e.kind == crate::bootstrap::SpecElementKind::Adr)
                .count();
            let negs = elements
                .iter()
                .filter(|e| e.kind == crate::bootstrap::SpecElementKind::NegativeCase)
                .count();

            Ok(format!(
                "bootstrap: {} elements ({} INV, {} ADR, {} NEG) → {} datoms\n  → {}\n",
                elements.len(),
                invs,
                adrs,
                negs,
                datom_count,
                file_path.relative_path(),
            ))
        }
        Command::Verify { path } => {
            let layout = crate::layout::DiskLayout::open(&path)?;
            let report = layout.verify_integrity()?;
            if report.is_clean() {
                Ok(format!(
                    "integrity: OK ({} files verified)\n",
                    report.verified
                ))
            } else {
                Ok(format!(
                    "integrity: FAILED ({} corrupted, {} orphaned out of {})\n",
                    report.corrupted.len(),
                    report.orphaned.len(),
                    report.total_files,
                ))
            }
        }
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
        Command::Analyze {
            path,
            force,
            budget,
            full,
            json,
        } => {
            if json {
                analyze::run_json(&path)
            } else if full || force {
                if force {
                    analyze::run_force(&path)
                } else {
                    analyze::run(&path)
                }
            } else {
                // Auto-calibrate budget: enough for coherence + actions + topology + spectral
                // Explicit --budget overrides auto-calibration.
                let b = budget.unwrap_or(500);
                analyze::run_budget(&path, b, force)
            }
        }
        Command::Mcp { action } => match action {
            McpAction::Serve { path } => {
                mcp::serve(&path)?;
                Ok(String::new())
            }
        },
    }
}
