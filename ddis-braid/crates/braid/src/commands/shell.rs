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
    eprintln!("          attrs, schema, seed, harvest, recent, count, session, log, help, quit");

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
                Ok(co) => DispatchResult::Output(co.human),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        "show" | "s" => {
            if args.is_empty() {
                return DispatchResult::Error("usage: show <entity-ident>".into());
            }
            match super::query::run(path, Some(args), None, false) {
                Ok(co) => DispatchResult::Output(co.human),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        "find" | "f" => {
            if args.is_empty() {
                return DispatchResult::Error("usage: find <attribute>".into());
            }
            match super::query::run(path, None, Some(args), false) {
                Ok(co) => DispatchResult::Output(co.human),
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
                rationale: None,
                alternatives: None,
            }) {
                Ok(co) => DispatchResult::Output(co.human),
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
                Ok(co) => DispatchResult::Output(co.human),
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
                Ok(co) => DispatchResult::Output(co.human),
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
                Ok(co) => DispatchResult::Output(co.human),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        "log" | "l" => match super::log::run(path, 10, None, false, false, false) {
            Ok(co) => DispatchResult::Output(co.human),
            Err(e) => DispatchResult::Error(e.to_string()),
        },

        "seed" | "sd" => {
            let task = if args.is_empty() { "continue" } else { args };
            match super::seed::run(path, task, 500, "braid:shell", true, false, false) {
                Ok(co) => DispatchResult::Output(co.human),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        "harvest" | "hv" => {
            let task = if args.is_empty() { None } else { Some(args) };
            match super::harvest::run(path, "braid:shell", task, &[], false, false) {
                Ok(co) => DispatchResult::Output(co.human),
                Err(e) => DispatchResult::Error(e.to_string()),
            }
        }

        // S0.4.2: Navigation — most recent entities
        "recent" | "r" => {
            let count: usize = args.parse().unwrap_or(10);
            let layout = match DiskLayout::open(path) {
                Ok(l) => l,
                Err(e) => return DispatchResult::Error(e.to_string()),
            };
            let store = match layout.load_store() {
                Ok(s) => s,
                Err(e) => return DispatchResult::Error(e.to_string()),
            };

            // Find most recent entities by latest tx wall_time
            let mut entity_times: std::collections::BTreeMap<braid_kernel::EntityId, u64> =
                std::collections::BTreeMap::new();
            for datom in store.datoms() {
                if datom.op == braid_kernel::Op::Assert {
                    let entry = entity_times.entry(datom.entity).or_default();
                    let wall = datom.tx.wall_time();
                    if wall > *entry {
                        *entry = wall;
                    }
                }
            }
            let mut sorted: Vec<_> = entity_times.into_iter().collect();
            sorted.sort_by_key(|(_, t)| std::cmp::Reverse(*t));

            let shown = count.min(sorted.len());
            let mut out = format!("recent {} entities:\n", shown);
            for (entity, _wall) in sorted.iter().take(count) {
                // Resolve label from :db/ident
                let label = store
                    .entity_datoms(*entity)
                    .iter()
                    .find(|d| {
                        d.attribute.as_str() == ":db/ident" && d.op == braid_kernel::Op::Assert
                    })
                    .and_then(|d| {
                        if let braid_kernel::Value::Keyword(ref k) = d.value {
                            Some(k.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| {
                        let bytes = entity.as_bytes();
                        format!(
                            "#{:02x}{:02x}{:02x}{:02x}\u{2026}",
                            bytes[0], bytes[1], bytes[2], bytes[3]
                        )
                    });

                let doc = store
                    .entity_datoms(*entity)
                    .iter()
                    .find(|d| d.attribute.as_str() == ":db/doc" && d.op == braid_kernel::Op::Assert)
                    .and_then(|d| {
                        if let braid_kernel::Value::String(ref s) = d.value {
                            Some(if s.len() > 60 {
                                format!("{}...", &s[..60])
                            } else {
                                s.clone()
                            })
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                if doc.is_empty() {
                    out.push_str(&format!("  {label}\n"));
                } else {
                    out.push_str(&format!("  {label} \u{2014} {doc}\n"));
                }
            }
            DispatchResult::Output(out)
        }

        // S0.4.2: Navigation — store counts
        "count" | "ct" => {
            let layout = match DiskLayout::open(path) {
                Ok(l) => l,
                Err(e) => return DispatchResult::Error(e.to_string()),
            };
            let store = match layout.load_store() {
                Ok(s) => s,
                Err(e) => return DispatchResult::Error(e.to_string()),
            };

            let datom_count = store.len();
            let entity_count = store.entity_count();
            let tx_count: std::collections::BTreeSet<u64> =
                store.datoms().map(|d| d.tx.wall_time()).collect();

            DispatchResult::Output(format!(
                "store: {} datoms, {} entities, {} transactions\n",
                datom_count,
                entity_count,
                tx_count.len()
            ))
        }

        // S0.4.3: Session-aware CLI markers
        "session" | "ss" => {
            let layout = match DiskLayout::open(path) {
                Ok(l) => l,
                Err(e) => return DispatchResult::Error(e.to_string()),
            };
            let store = match layout.load_store() {
                Ok(s) => s,
                Err(e) => return DispatchResult::Error(e.to_string()),
            };

            let harvest_due = braid_kernel::guidance::count_txns_since_last_harvest(&store);

            let mut out = String::new();
            out.push_str(&format!(
                "session state: {} tx since last harvest",
                harvest_due
            ));
            if harvest_due > 5 {
                out.push_str(" \u{26a0} harvest recommended");
            }
            out.push('\n');

            // Discover recent harvest sessions (up to 3)
            let mut sessions: Vec<(braid_kernel::EntityId, u64)> = Vec::new();
            for datom in store.datoms() {
                if datom.attribute.as_str() == ":harvest/agent"
                    && datom.op == braid_kernel::Op::Assert
                {
                    sessions.push((datom.entity, datom.tx.wall_time()));
                }
            }
            sessions.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
            sessions.truncate(3);

            if sessions.is_empty() {
                out.push_str("  no prior sessions found\n");
            } else {
                out.push_str("  recent sessions:\n");
                for (i, (entity, _wall)) in sessions.iter().enumerate() {
                    // Extract goal from :db/doc
                    let goal = store
                        .entity_datoms(*entity)
                        .iter()
                        .find(|d| {
                            d.attribute.as_str() == ":db/doc" && d.op == braid_kernel::Op::Assert
                        })
                        .and_then(|d| {
                            if let braid_kernel::Value::String(ref s) = d.value {
                                Some(if s.len() > 80 {
                                    format!("{}...", &s[..80])
                                } else {
                                    s.clone()
                                })
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| "(no task)".into());

                    // Count observations in this session's temporal window
                    let session_time = *_wall;
                    let window_start = session_time.saturating_sub(3600);
                    let window_end = session_time.saturating_add(60);
                    let mut obs_count = 0usize;
                    let mut decision_count = 0usize;
                    for d in store.datoms() {
                        if d.op != braid_kernel::Op::Assert {
                            continue;
                        }
                        if d.attribute.as_str() != ":exploration/source" {
                            continue;
                        }
                        let obs_time = d.tx.wall_time();
                        if obs_time < window_start || obs_time > window_end {
                            continue;
                        }
                        obs_count += 1;
                        // Check if this observation is a design decision
                        for ed in store.entity_datoms(d.entity) {
                            if ed.attribute.as_str() == ":exploration/category"
                                && ed.op == braid_kernel::Op::Assert
                            {
                                if let braid_kernel::Value::String(ref s) = ed.value {
                                    if s == "design-decision" {
                                        decision_count += 1;
                                    }
                                }
                                if let braid_kernel::Value::Keyword(ref k) = ed.value {
                                    if k == "design-decision" {
                                        decision_count += 1;
                                    }
                                }
                            }
                        }
                    }

                    out.push_str(&format!(
                        "    {}. {} ({} obs, {} decisions)\n",
                        i + 1,
                        goal,
                        obs_count,
                        decision_count
                    ));
                }
            }

            DispatchResult::Output(out)
        }

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
  seed [task] (sd)          Session briefing (default: --for-human, 500 tokens)
  harvest [task] (hv)       End-of-session: show candidates (use --commit in CLI)
  recent [N] (r)            Most recent N entities (default: 10)
  count (ct)                Store datom/entity/transaction counts
  session (ss)              Session lifecycle state: harvests, observations
  analyze (az)             Graph analytics (budget-aware, 200 token cap)
  guidance (g)             Full methodology + all actions (= status --verbose)
  deep (bi)                Bilateral F(S) + convergence (= status --deep)
  log (l)                  Last 10 transactions
  help (h, ?)              This help text
  quit (q, exit)           Exit the shell
"
    .to_string()
}
