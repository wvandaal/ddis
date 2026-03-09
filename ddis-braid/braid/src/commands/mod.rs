//! CLI command definitions and dispatch.

use std::path::PathBuf;

use clap::Subcommand;

mod harvest;
mod init;
mod query;
mod seed;
mod status;
mod transact;

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

    /// Verify integrity of the on-disk store.
    Verify {
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
    }
}
