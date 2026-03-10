//! `braid shell` — Interactive exploration mode (zero external dependencies).
//!
//! A simple readline loop over std::io::stdin() that dispatches to existing
//! CLI command functions. No line editing, no history — just fast exploration.
//!
//! Traces to: INV-INTERFACE-001 (Five-Layer Channel, layer 2: interactive).

use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::error::BraidError;
use crate::layout::DiskLayout;

/// Run the interactive shell.
///
/// Loads the store once, then loops: prompt → parse → dispatch → print.
/// Commands operate on the loaded store; mutating commands (observe, transact)
/// reload the store after writing to reflect the new state.
///
/// Exit: Ctrl-D (EOF) or `quit`/`exit`.
pub fn run(path: &Path) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    // Verify the store loads before entering the loop.
    let _ = layout.load_store()?;

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let path_owned = path.to_path_buf();

    eprintln!("braid shell (Ctrl-D or 'quit' to exit)");
    eprintln!("commands: status, show <entity>, find <attr>, observe <text>, datalog <expr>,");
    eprintln!("          analyze, deep, log, help, quit");

    loop {
        eprint!("braid> ");
        io::stderr().flush().ok();

        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF (Ctrl-D)
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            }
            Ok(_) => {}
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (cmd, rest) = match line.split_once(char::is_whitespace) {
            Some((c, r)) => (c, r.trim()),
            None => (line, ""),
        };

        let result = dispatch(cmd, rest, &path_owned);
        match result {
            DispatchResult::Output(s) => print!("{s}"),
            DispatchResult::Error(e) => eprintln!("error: {e}"),
            DispatchResult::Quit => break,
            DispatchResult::Unknown => eprintln!("unknown command: '{cmd}'. Type 'help' for list."),
        }
    }

    eprintln!("goodbye.");
    Ok(String::new())
}

/// Result of dispatching a single shell command.
enum DispatchResult {
    /// Command produced output to display.
    Output(String),
    /// Command produced an error.
    Error(String),
    /// User wants to quit.
    Quit,
    /// Unrecognized command.
    Unknown,
}

/// Dispatch a parsed shell command to the appropriate CLI function.
fn dispatch(cmd: &str, args: &str, path: &Path) -> DispatchResult {
    match cmd {
        "quit" | "exit" | "q" => DispatchResult::Quit,

        "help" | "h" | "?" => DispatchResult::Output(help_text()),

        "status" | "st" => {
            match super::status::run(
                path,
                "braid:shell",
                false,
                false,
                false,
                false,
                false,
                false,
                false,
            ) {
                Ok(s) => DispatchResult::Output(s),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        "show" | "s" => {
            if args.is_empty() {
                return DispatchResult::Error("usage: show <entity-ident>".into());
            }
            match super::query::run(path, Some(args), None, false) {
                Ok(s) => DispatchResult::Output(s),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        "find" | "f" => {
            if args.is_empty() {
                return DispatchResult::Error("usage: find <attribute>".into());
            }
            match super::query::run(path, None, Some(args), false) {
                Ok(s) => DispatchResult::Output(s),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        "observe" | "o" => {
            if args.is_empty() {
                return DispatchResult::Error("usage: observe <text>".into());
            }
            match super::observe::run(super::observe::ObserveArgs {
                path,
                text: args,
                confidence: 0.7,
                tags: &[],
                category: None,
                agent: "braid:shell",
                relates_to: None,
            }) {
                Ok(s) => DispatchResult::Output(s),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        "datalog" | "dl" | "dq" => {
            if args.is_empty() {
                return DispatchResult::Error(
                    "usage: datalog [:find ?e :where [?e :db/doc ?v]]".into(),
                );
            }
            match super::query::run_datalog(path, args, false) {
                Ok(s) => DispatchResult::Output(s),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        "analyze" | "az" => match super::analyze::run_budget(path, 200, false) {
            Ok(s) => DispatchResult::Output(s),
            Err(e) => DispatchResult::Error(e.to_string()),
        },

        "guidance" | "g" => {
            // Guidance absorbed into status (verbose mode shows full methodology)
            match super::status::run(
                path,
                "braid:shell",
                false,
                true,
                false,
                false,
                false,
                false,
                false,
            ) {
                Ok(s) => DispatchResult::Output(s),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        "bilateral" | "bi" | "deep" => {
            // Bilateral absorbed into status --deep
            match super::status::run(
                path,
                "braid:shell",
                false,
                false,
                true,
                false,
                false,
                false,
                false,
            ) {
                Ok(s) => DispatchResult::Output(s),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        "log" | "l" => match super::log::run(path, 10, None, false, false, false) {
            Ok(s) => DispatchResult::Output(s),
            Err(e) => DispatchResult::Error(e.to_string()),
        },

        _ => DispatchResult::Unknown,
    }
}

/// Help text listing available shell commands.
fn help_text() -> String {
    "\
commands:
  status (st)              Dashboard: datoms, coherence, M(t), next action
  show <entity> (s)        All datoms for an entity (e.g., show :spec/inv-store-001)
  find <attribute> (f)     All values of an attribute (e.g., find :db/doc)
  observe <text> (o)       Capture a knowledge observation (confidence 0.7)
  datalog <expr> (dl)      Datalog query (e.g., datalog [:find ?e :where [?e :db/doc ?v]])
  analyze (az)             Graph analytics (budget-aware, 200 token cap)
  guidance (g)             Full methodology + all actions (= status --verbose)
  deep (bi)                Bilateral F(S) + convergence (= status --deep)
  log (l)                  Last 10 transactions
  help (h, ?)              This help text
  quit (q, exit)           Exit the shell
"
    .to_string()
}
