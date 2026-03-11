//! `braid session` — Zero-ceremony lifecycle commands.
//!
//! Two commands that encapsulate the entire session protocol:
//! - `session start` — inject seed + actionable summary (replaces 4-step start)
//! - `session end`   — harvest + re-inject + git guidance (replaces 6-step end)
//!
//! Traces to: INV-INTERFACE-011 (CLI Surface as Optimized Prompt),
//!            ADR-FOUNDATION-002 (manual → tooling graduation),
//!            INV-HARVEST-009 (Continuous Externalization Protocol)

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use braid_kernel::datom::Op;
use braid_kernel::guidance::{count_txns_since_last_harvest, last_harvest_wall_time};

use crate::error::BraidError;
use crate::layout::DiskLayout;

use super::{harvest, seed};

// ---------------------------------------------------------------------------
// session start
// ---------------------------------------------------------------------------

/// Start a new session: inject seed, show actionable summary.
///
/// Task resolution precedence:
/// 1. Explicit `--task` flag
/// 2. Last harvest's synthesis directive (auto-continuation)
/// 3. Fallback: "session work"
pub fn run_start(
    path: &Path,
    inject_path: &Path,
    task: Option<&str>,
    budget: usize,
    _agent_name: &str,
) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Resolve task: explicit > last harvest directive > fallback
    let (resolved_task, task_source) = match task {
        Some(t) => (t.to_string(), "explicit"),
        None => match find_last_synthesis_directive(&store) {
            Some(directive) => (directive, "last harvest"),
            None => ("session work".to_string(), "default"),
        },
    };

    // Inject seed into target file
    let inject_output = seed::run_inject(path, inject_path, &resolved_task, budget)?;

    // Compute session context
    let tx_since_harvest = count_txns_since_last_harvest(&store);
    let last_harvest = last_harvest_wall_time(&store);
    let harvest_age = format_age(last_harvest);

    let mut out = String::new();
    out.push_str("Session started.\n");
    out.push_str(&inject_output);
    out.push_str(&format!(
        "Task: {} (source: {})\n",
        resolved_task, task_source
    ));
    out.push_str(&format!(
        "Store: {} datoms, {} entities | Last harvest: {} | {} tx since\n",
        store.len(),
        store.entity_count(),
        harvest_age,
        tx_since_harvest,
    ));
    out.push_str(
        "Next: braid observe \"...\" to capture knowledge | braid session end when done\n",
    );

    Ok(out)
}

// ---------------------------------------------------------------------------
// session end
// ---------------------------------------------------------------------------

/// End a session: harvest → re-inject seed → show git guidance.
///
/// Does NOT run git commands (respects user git discipline, AGENTS.md).
/// Shows suggested git workflow as guidance.
pub fn run_end(
    path: &Path,
    inject_path: &Path,
    task: Option<&str>,
    budget: usize,
    agent_name: &str,
) -> Result<String, BraidError> {
    // Check for observations since last harvest — refuse if nothing to harvest
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;
    let tx_since_harvest = count_txns_since_last_harvest(&store);

    if tx_since_harvest == 0 {
        return Err(BraidError::Validation(
            "nothing to harvest (0 transactions since last harvest). \
             Use `braid observe` to capture knowledge first."
                .into(),
        ));
    }

    let mut out = String::new();
    out.push_str("Ending session...\n\n");

    // Step 1: Harvest with commit (Stage 0: force=true bypasses guard)
    let harvest_output = harvest::run(path, agent_name, task, &[], true, true)?;
    out.push_str(&harvest_output);

    // Step 2: Re-inject seed for next session
    // Reload store to include the harvest commit just written
    let task_for_inject = task.unwrap_or("continue");
    let inject_output = seed::run_inject(path, inject_path, task_for_inject, budget)?;
    out.push_str(&format!("\nRefreshed seed: {}", inject_output));

    // Step 3: Git guidance (advisory only — does NOT run git commands)
    out.push_str("\nNext steps (manual):\n");
    out.push_str("  git add -A && git commit -m \"Session NNN: ...\"\n");
    out.push_str("  git push\n");

    Ok(out)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the most recent synthesis directive from harvest history.
///
/// Scans for `:harvest/synthesis-directive` datoms and returns the one
/// with the highest wall_time. Returns None if no harvests exist.
fn find_last_synthesis_directive(store: &braid_kernel::Store) -> Option<String> {
    let mut latest_wall = 0u64;
    let mut latest_directive = None;

    for d in store.datoms() {
        if d.attribute.as_str() == ":harvest/synthesis-directive" && d.op == Op::Assert {
            let wall = d.tx.wall_time();
            if wall > latest_wall {
                if let braid_kernel::datom::Value::String(ref s) = d.value {
                    latest_wall = wall;
                    // Extract the task from the directive.
                    // Directive format: "## Session Synthesis Directive\n\n**Next session task**: ..."
                    // We want just the task part, not the full directive.
                    let task = extract_task_from_directive(s);
                    latest_directive = Some(task);
                }
            }
        }
    }

    latest_directive
}

/// Extract the actionable task from a synthesis directive string.
///
/// The directive may contain markdown formatting. We extract the task
/// description, stripping "continue:" prefixes and markdown headers.
fn extract_task_from_directive(directive: &str) -> String {
    // Look for "**Next session task**: <task>" pattern
    for line in directive.lines() {
        if let Some(rest) = line.strip_prefix("**Next session task**: ") {
            let task = rest.trim();
            // Strip "continue: " prefix if present
            let task = task.strip_prefix("continue: ").unwrap_or(task);
            if !task.is_empty() {
                return task.to_string();
            }
        }
    }

    // Fallback: use the directive text itself, truncated
    let clean = directive
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if clean.len() > 120 {
        format!("{}...", &clean[..117])
    } else {
        clean
    }
}

/// Format a wall_time as a human-readable age string.
///
/// Returns "N minutes/hours/days ago" or "never" if wall_time is 0.
fn format_age(wall_time: u64) -> String {
    if wall_time == 0 {
        return "never".to_string();
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if now < wall_time {
        // Clock skew or non-Unix wall_time format
        return "recently".to_string();
    }

    let elapsed = now - wall_time;
    if elapsed < 60 {
        "just now".to_string()
    } else if elapsed < 3600 {
        let mins = elapsed / 60;
        format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if elapsed < 86400 {
        let hours = elapsed / 3600;
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else {
        let days = elapsed / 86400;
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_task_from_directive_with_next_session_prefix() {
        let directive =
            "## Session Synthesis Directive\n\n**Next session task**: implement bilateral CLI\n";
        assert_eq!(
            extract_task_from_directive(directive),
            "implement bilateral CLI"
        );
    }

    #[test]
    fn extract_task_from_directive_with_continue_prefix() {
        let directive = "## Session Synthesis Directive\n\n**Next session task**: continue: budget-aware output integration\n";
        assert_eq!(
            extract_task_from_directive(directive),
            "budget-aware output integration"
        );
    }

    #[test]
    fn extract_task_from_directive_fallback() {
        let directive = "Some raw directive text without the expected format";
        assert_eq!(
            extract_task_from_directive(directive),
            "Some raw directive text without the expected format"
        );
    }

    #[test]
    fn extract_task_truncates_long_fallback() {
        let directive = "A".repeat(200);
        let result = extract_task_from_directive(&directive);
        assert!(result.len() <= 120);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn format_age_zero_is_never() {
        assert_eq!(format_age(0), "never");
    }

    #[test]
    fn format_age_future_is_recently() {
        // Wall time in the future (clock skew)
        let future = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 1000;
        assert_eq!(format_age(future), "recently");
    }

    #[test]
    fn format_age_minutes() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_age(now - 300), "5 minutes ago");
    }

    #[test]
    fn format_age_hours() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_age(now - 7200), "2 hours ago");
    }

    #[test]
    fn format_age_days() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_age(now - 172800), "2 days ago");
    }

    #[test]
    fn format_age_singular() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_age(now - 60), "1 minute ago");
        assert_eq!(format_age(now - 3600), "1 hour ago");
        assert_eq!(format_age(now - 86400), "1 day ago");
    }
}
