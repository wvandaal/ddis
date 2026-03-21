//! `braid wrap` — Auto-observe build/test/lint results.
//!
//! Proxies a subprocess command, streams output in real time, and auto-creates
//! observations for failures and warnings. Clean successes produce zero
//! observations (INV-WRAP-001).
//!
//! # Examples
//!
//! ```bash
//! braid wrap cargo test                    # Auto-observe test failures
//! braid wrap cargo clippy -- -D warnings   # Auto-observe warnings
//! braid wrap cargo fmt --check             # Auto-observe format issues
//! ```
//!
//! Traces to: INV-WRAP-001 (clean = no observation), INV-WRAP-002 (fail = observation)

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use super::observe::{self, ObserveArgs};
use crate::error::BraidError;
use crate::output::{AgentOutput, CommandOutput};

/// Classification of subprocess result.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ResultClass {
    /// Exit 0, no warnings.
    Clean,
    /// Exit 0, but warnings in stderr.
    Warn,
    /// Exit != 0.
    Fail,
}

/// Parsed output summary.
struct OutputSummary {
    class: ResultClass,
    category: &'static str,
    confidence: f64,
    body: String,
}

/// Run a subprocess, stream its output, and auto-observe failures.
///
/// INV-WRAP-001: Clean success produces zero observations.
/// INV-WRAP-002: Every failed invocation produces exactly one observation.
pub fn run(
    path: &Path,
    agent: &str,
    cmd_args: &[String],
    timeout_secs: Option<u64>,
) -> Result<CommandOutput, BraidError> {
    if cmd_args.is_empty() {
        return Err(BraidError::Validation(
            "braid wrap requires a command to run".to_string(),
        ));
    }

    let (program, args) = (&cmd_args[0], &cmd_args[1..]);

    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BraidError::Validation(format!("failed to spawn `{program}`: {e}")))?;

    // Read stdout and stderr, streaming to terminal and buffering
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    // Read stdout in background thread
    let stdout_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(stdout) = stdout_handle {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                eprintln!("{line}"); // Stream to terminal
                buf.push_str(&line);
                buf.push('\n');
            }
        }
        buf
    });

    // Read stderr in background thread
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(stderr) = stderr_handle {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                eprintln!("{line}"); // Stream to terminal
                buf.push_str(&line);
                buf.push('\n');
            }
        }
        buf
    });

    // Wait for process with optional timeout
    let exit_status = if let Some(secs) = timeout_secs {
        let start = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if start.elapsed().as_secs() > secs {
                        let _ = child.kill();
                        return Err(BraidError::Validation(format!(
                            "command timed out after {secs}s"
                        )));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(e) => return Err(BraidError::Io(e)),
            }
        }
    } else {
        child.wait().map_err(BraidError::Io)?
    };

    // Collect thread output
    let stdout_buf = stdout_thread.join().unwrap_or_default();
    let stderr_buf = stderr_thread.join().unwrap_or_default();

    let exit_code = exit_status.code().unwrap_or(-1);
    let cmd_str = cmd_args.join(" ");

    // Classify result
    let summary = classify_result(exit_code, &cmd_str, &stdout_buf, &stderr_buf);

    let mut human = String::new();
    human.push_str(&format!("wrap: `{cmd_str}` exited {exit_code}\n"));

    let result_label = match summary.class {
        ResultClass::Clean => "clean",
        ResultClass::Warn => "WARN",
        ResultClass::Fail => "FAIL",
    };

    // INV-WRAP-001: Clean success → no observation
    if summary.class == ResultClass::Clean {
        human.push_str("  result: clean (no observation)\n");
        let json = serde_json::json!({
            "command": cmd_str,
            "exit_code": exit_code,
            "result": "clean",
            "observed": false,
        });
        let agent_out = AgentOutput {
            context: format!("wrap: `{cmd_str}` clean (exit {exit_code})"),
            content: String::new(),
            footer: String::new(),
        };
        return Ok(CommandOutput {
            json,
            agent: agent_out,
            human,
        });
    }

    // INV-WRAP-002: Fail/Warn → create observation
    let observe_result = observe::run(ObserveArgs {
        path,
        text: &summary.body,
        confidence: summary.confidence,
        tags: &[format!("wrap:{program}")],
        category: Some(summary.category),
        agent,
        relates_to: None,
        rationale: None,
        alternatives: None,
        no_auto_crystallize: false,
    })?;

    human.push_str(&format!(
        "  result: {} \u{2192} auto-observed\n",
        result_label
    ));
    human.push_str(&observe_result.human);

    let json = serde_json::json!({
        "command": cmd_str,
        "exit_code": exit_code,
        "result": result_label,
        "observed": true,
        "category": summary.category,
        "confidence": summary.confidence,
    });

    let agent_out = AgentOutput {
        context: format!("wrap: `{cmd_str}` {} (exit {})", result_label, exit_code),
        content: format!(
            "auto-observed as {} (c={:.2})",
            summary.category, summary.confidence
        ),
        footer: "review: braid log --limit 1".to_string(),
    };

    Ok(CommandOutput {
        json,
        agent: agent_out,
        human,
    })
}

/// Classify subprocess output into Clean/Warn/Fail with a summary body.
fn classify_result(exit_code: i32, cmd: &str, stdout: &str, stderr: &str) -> OutputSummary {
    if exit_code != 0 {
        // Failed — extract useful summary
        let body = if cmd.contains("cargo test") {
            parse_cargo_test_failure(stdout, stderr)
        } else if cmd.contains("cargo clippy") {
            parse_cargo_clippy(stderr)
        } else if cmd.contains("cargo fmt") {
            parse_cargo_fmt(stdout)
        } else {
            format!("Command `{cmd}` failed (exit {exit_code})")
        };

        OutputSummary {
            class: ResultClass::Fail,
            category: "build-failure",
            confidence: 0.95,
            body,
        }
    } else if has_warnings(stderr) {
        let body = if cmd.contains("cargo clippy") {
            parse_cargo_clippy(stderr)
        } else {
            format!("Command `{cmd}` succeeded with warnings")
        };

        OutputSummary {
            class: ResultClass::Warn,
            category: "build-warning",
            confidence: 0.7,
            body,
        }
    } else {
        OutputSummary {
            class: ResultClass::Clean,
            category: "observation",
            confidence: 1.0,
            body: String::new(),
        }
    }
}

/// Check if stderr contains warning patterns.
fn has_warnings(stderr: &str) -> bool {
    stderr.contains("warning:") && !stderr.contains("0 warnings")
}

/// Parse cargo test failure output for a concise summary.
fn parse_cargo_test_failure(stdout: &str, stderr: &str) -> String {
    // Look for "test result: FAILED" line
    let combined = format!("{stdout}\n{stderr}");
    let result_line = combined
        .lines()
        .find(|l| l.contains("test result: FAILED"))
        .unwrap_or("tests failed");

    // Collect failed test names
    let failed: Vec<&str> = combined
        .lines()
        .filter(|l| l.starts_with("test ") && l.ends_with("... FAILED"))
        .map(|l| {
            l.strip_prefix("test ")
                .unwrap_or(l)
                .strip_suffix(" ... FAILED")
                .unwrap_or(l)
        })
        .take(10)
        .collect();

    if failed.is_empty() {
        format!("cargo test: {result_line}")
    } else {
        format!(
            "cargo test: {} | Failed: {}",
            result_line,
            failed.join(", ")
        )
    }
}

/// Parse cargo clippy output for a concise summary.
fn parse_cargo_clippy(stderr: &str) -> String {
    let warning_count = stderr.lines().filter(|l| l.starts_with("warning:")).count();
    let error_count = stderr.lines().filter(|l| l.starts_with("error")).count();

    if error_count > 0 {
        format!("cargo clippy: {error_count} errors, {warning_count} warnings")
    } else if warning_count > 0 {
        format!("cargo clippy: {warning_count} warnings")
    } else {
        "cargo clippy: clean".to_string()
    }
}

/// Parse cargo fmt output for a concise summary.
fn parse_cargo_fmt(stdout: &str) -> String {
    let diff_count = stdout.lines().filter(|l| l.starts_with("Diff in")).count();
    if diff_count > 0 {
        format!("cargo fmt: {diff_count} files need formatting")
    } else {
        "cargo fmt: formatting issues detected".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_clean_success() {
        let s = classify_result(0, "echo hello", "hello\n", "");
        assert_eq!(s.class, ResultClass::Clean);
    }

    #[test]
    fn classify_failure() {
        let s = classify_result(1, "cargo test", "", "error: test failed");
        assert_eq!(s.class, ResultClass::Fail);
        assert_eq!(s.category, "build-failure");
        assert!((s.confidence - 0.95).abs() < 0.01);
    }

    #[test]
    fn classify_warnings() {
        let s = classify_result(0, "cargo clippy", "", "warning: unused variable\n");
        assert_eq!(s.class, ResultClass::Warn);
        assert_eq!(s.category, "build-warning");
    }

    #[test]
    fn parse_test_failure_extracts_names() {
        let stdout = "test foo::bar ... ok\ntest foo::baz ... FAILED\n\ntest result: FAILED. 1 passed; 1 failed\n";
        let result = parse_cargo_test_failure(stdout, "");
        assert!(result.contains("foo::baz"));
        assert!(result.contains("FAILED"));
    }

    #[test]
    fn no_warnings_when_zero_warnings() {
        assert!(!has_warnings("warning: 0 warnings emitted"));
        assert!(has_warnings("warning: unused variable `x`"));
    }
}
