//! CLI command definitions and dispatch.

use std::path::PathBuf;

use clap::Subcommand;

mod generate;
mod guidance;
mod harvest;
mod init;
mod log;
mod merge;
mod query;
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

    /// Query the store using entity/attribute filters.
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

    /// Generate a dynamic CLAUDE.md from the store.
    Generate {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Description of the task to work on.
        #[arg(short, long)]
        task: String,

        /// Token budget for the generated CLAUDE.md.
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

    /// Self-bootstrap: parse spec/*.md and transact elements as datoms.
    Bootstrap {
        /// Path to the .braid directory.
        #[arg(short, long, default_value = ".braid")]
        path: PathBuf,

        /// Path to the spec directory.
        #[arg(short, long, default_value = "spec")]
        spec_dir: PathBuf,
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
        Command::Query {
            path,
            entity,
            attribute,
        } => query::run(&path, entity.as_deref(), attribute.as_deref()),
        Command::Harvest {
            path,
            agent,
            task,
            knowledge,
        } => harvest::run(&path, &agent, &task, &knowledge),
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
        Command::Mcp { action } => match action {
            McpAction::Serve { path } => {
                mcp::serve(&path)?;
                Ok(String::new())
            }
        },
    }
}
