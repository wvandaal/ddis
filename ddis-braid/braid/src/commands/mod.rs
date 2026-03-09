//! CLI command definitions and dispatch.

/// All subcommands available in the `braid` CLI.
#[derive(clap::Subcommand)]
pub enum Command {
    /// Show store status: datom count, frontier, schema summary.
    Status,
}

/// Execute a CLI command and return the output string.
pub fn run(cmd: Command) -> Result<String, crate::error::BraidError> {
    match cmd {
        Command::Status => Ok("braid: no store initialized\n".to_string()),
    }
}
