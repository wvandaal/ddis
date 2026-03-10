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
mod promote;
mod query;
mod retract;
mod seed;
mod status;
mod transact;

// Re-export mcp serve as a special case (runs an event loop, not a single command).
pub use crate::mcp;

/// All subcommands available in the `braid` CLI.
#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new braid store at the given path.
    Init {
        /// Path for the .braid directory (default: .braid in current dir).
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,
    },

    /// Show store status: datom count, frontier, schema summary.
    Status {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,
    },

    /// Assert datoms into the store.
    Transact {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Agent name performing the transaction.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,

        /// Rationale for this transaction.
        #[arg(short, long)]
        rationale: String,

        /// Assertions in "entity attribute value" format (repeatable).
        #[arg(short = 'd', long = "datom", num_args = 3, action = clap::ArgAction::Append)]
        datoms: Vec<String>,
    },

    /// Retract existing assertions from the store (append-only: creates retraction datoms).
    Retract {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Agent name performing the retraction.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,

        /// Entity ident to retract from (e.g., ":spec/inv-store-001").
        #[arg(short, long)]
        entity: String,

        /// Attribute to retract (e.g., ":db/doc").
        #[arg(long)]
        attribute: String,

        /// Optional value filter — only retract assertions with this value.
        #[arg(short, long)]
        value: Option<String>,
    },

    /// Query the store using entity/attribute filters or Datalog.
    Query {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Filter by entity (keyword ident).
        #[arg(short, long)]
        entity: Option<String>,

        /// Filter by attribute (keyword).
        #[arg(short, long)]
        attribute: Option<String>,

        /// Datalog query expression (e.g., '[:find ?e ?v :where [?e :db/doc ?v]]').
        #[arg(long)]
        datalog: Option<String>,
    },

    /// Run the harvest pipeline to detect knowledge gaps.
    Harvest {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Agent name performing the harvest.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,

        /// Description of the task worked on.
        #[arg(short, long)]
        task: String,

        /// Knowledge items in "key value" format (repeatable).
        #[arg(short = 'k', long = "knowledge", num_args = 2, action = clap::ArgAction::Append)]
        knowledge: Vec<String>,

        /// Commit approved candidates to the store (persist as datoms).
        #[arg(long)]
        commit: bool,
    },

    /// Assemble a seed context for a new session.
    Seed {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Description of the task to work on.
        #[arg(short, long)]
        task: String,

        /// Token budget for the seed output.
        #[arg(short, long, default_value = "2000")]
        budget: usize,

        /// Agent name for the seed.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,
    },

    /// Display current guidance state: divergence, coherence, methodology score.
    Guidance {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Agent name to show guidance for.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,
    },

    /// Merge another store into the current store (CRDT set union).
    Merge {
        /// Path to the .braid directory (target).
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Path to the source .braid directory to merge from.
        #[arg(short, long)]
        source: PathBuf,
    },

    /// Browse the transaction log with filtering.
    Log {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Maximum number of transactions to show.
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,

        /// Filter by agent name.
        #[arg(short, long)]
        agent: Option<String>,

        /// Show individual datoms in each transaction.
        #[arg(long)]
        datoms: bool,
    },

    /// Generate dynamic agent instructions from the store.
    Generate {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Description of the task to work on.
        #[arg(short, long)]
        task: String,

        /// Token budget for the generated agent instructions.
        #[arg(short, long, default_value = "4000")]
        budget: usize,

        /// Agent name for the generation context.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,
    },

    /// Verify integrity of the on-disk store.
    Verify {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,
    },

    /// Promote an exploration entity to a formal spec element (store-first pipeline).
    Promote {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Entity ident to promote (e.g., ":exploration/topo-cold-start").
        #[arg(short, long)]
        entity: String,

        /// Target spec element ID (e.g., "INV-TOPOLOGY-001").
        #[arg(long)]
        target_id: String,

        /// Target namespace (e.g., "TOPOLOGY").
        #[arg(short, long)]
        namespace: String,

        /// Target element type: invariant, adr, negative-case.
        #[arg(short = 't', long = "type")]
        target_type: String,

        /// Agent name performing the promotion.
        #[arg(short, long, default_value = "braid:user")]
        agent: String,

        /// Formal statement text (for invariants).
        #[arg(long)]
        statement: Option<String>,

        /// Falsification condition (for invariants and negative cases).
        #[arg(long)]
        falsification: Option<String>,

        /// Verification method.
        #[arg(long)]
        verification: Option<String>,

        /// Problem statement (for ADRs).
        #[arg(long)]
        problem: Option<String>,

        /// Decision text (for ADRs).
        #[arg(long)]
        decision: Option<String>,
    },

    /// Generate spec markdown from store entities (inverse bootstrap).
    GenerateSpec {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Output directory for generated spec files.
        #[arg(short, long, default_value = "spec")]
        output: PathBuf,

        /// Only generate for this namespace (e.g., "TOPOLOGY"). Omit for all.
        #[arg(short, long)]
        namespace: Option<String>,
    },

    /// Self-bootstrap: parse spec/*.md and transact elements as datoms.
    Bootstrap {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Path to the spec directory.
        #[arg(short, long, default_value = "spec")]
        spec_dir: PathBuf,
    },

    /// Run comprehensive graph analytics on the store (coherence dashboard).
    Analyze {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Force recomputation (ignore cache).
        #[arg(long)]
        force: bool,
    },

    /// Run the MCP (Model Context Protocol) server over JSON-RPC stdio.
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
        Command::Status { path } => status::run(&path),
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
        } => {
            if let Some(ref dq) = datalog {
                query::run_datalog(&path, dq)
            } else {
                query::run(&path, entity.as_deref(), attribute.as_deref())
            }
        }
        Command::Harvest {
            path,
            agent,
            task,
            knowledge,
            commit,
        } => harvest::run(&path, &agent, &task, &knowledge, commit),
        Command::Seed {
            path,
            task,
            budget,
            agent,
        } => seed::run(&path, &task, budget, &agent),
        Command::Guidance { path, agent } => guidance::run(&path, &agent),
        Command::Merge { path, source } => merge::run(&path, &source),
        Command::Log {
            path,
            limit,
            agent,
            datoms,
        } => log::run(&path, limit, agent.as_deref(), datoms),
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
        Command::Analyze { path, force } => {
            if force {
                analyze::run_force(&path)
            } else {
                analyze::run(&path)
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
