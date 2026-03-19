//! Output mode dispatch for CLI responses (INV-INTERFACE-009, INV-OUTPUT-001..004).
//!
//! Three modes:
//! - `Json`: Machine-parseable JSON (all information, no formatting)
//! - `Agent`: LLM-native three-part structure (context + content + footer, ≤300 tokens)
//! - `Human`: Formatted human-readable output (progressive disclosure)
//!
//! Resolution priority (INV-OUTPUT-001: deterministic):
//!   1. Explicit --format flag
//!   2. BRAID_OUTPUT environment variable
//!   3. TTY detection: stdout is a TTY → Human
//!   4. Default: Agent (braid's primary consumer is an AI agent)
//!
//! Scripts that need JSON should use `--format json` or `BRAID_OUTPUT=json`.

use serde::Serialize;

/// Output mode enum.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum OutputMode {
    /// Machine-parseable JSON.
    Json,
    /// LLM-native output with guidance footer.
    #[default]
    Agent,
    /// Formatted human-readable output.
    Human,
}

impl std::fmt::Display for OutputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputMode::Json => write!(f, "json"),
            OutputMode::Agent => write!(f, "agent"),
            OutputMode::Human => write!(f, "human"),
        }
    }
}

impl std::str::FromStr for OutputMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(OutputMode::Json),
            "agent" => Ok(OutputMode::Agent),
            "human" => Ok(OutputMode::Human),
            other => Err(format!(
                "unknown output mode '{}': expected json, agent, or human",
                other
            )),
        }
    }
}

/// Resolve the output mode from CLI flag, environment variable, and TTY state.
///
/// INV-OUTPUT-001: Given identical (flag, env, tty) inputs, resolve() always returns the same Mode.
/// Priority: explicit flag > BRAID_OUTPUT env > pipe detection > TTY detection > default (Agent).
pub fn resolve_mode(format_flag: Option<&str>) -> OutputMode {
    // 1. Explicit --format flag
    if let Some(flag) = format_flag {
        if let Ok(mode) = flag.parse::<OutputMode>() {
            return mode;
        }
    }

    // 2. BRAID_OUTPUT environment variable
    if let Ok(env_val) = std::env::var("BRAID_OUTPUT") {
        if let Ok(mode) = env_val.parse::<OutputMode>() {
            return mode;
        }
    }

    // 3. TTY detection: stdout is a TTY → Human
    if is_stdout_tty() {
        return OutputMode::Human;
    }

    // 4. Default: Agent (AI agents are the primary consumer, even in piped contexts).
    // Scripts needing JSON should use --format json or BRAID_OUTPUT=json.
    OutputMode::Agent
}

/// Check if stdout is a terminal (TTY).
///
/// Uses libc isatty on Unix. Falls back to false (non-TTY) on error.
fn is_stdout_tty() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Agent-mode three-part output structure (INV-OUTPUT-002).
///
/// Each part has a soft token budget:
/// - context: ≤50 tokens — what store/entity this is about
/// - content: ≤200 tokens — the answer/result
/// - footer:  ≤50 tokens — next action + command
///
/// Total: ≤300 tokens.
#[derive(Clone, Debug, Serialize)]
pub struct AgentOutput {
    /// What store/entity this is about (≤50 tokens).
    pub context: String,
    /// The answer/result (≤200 tokens).
    pub content: String,
    /// Next action + command (≤50 tokens).
    pub footer: String,
}

impl AgentOutput {
    /// Render agent output as a compact multi-line string.
    pub fn render(&self) -> String {
        let mut out = String::new();
        if !self.context.is_empty() {
            out.push_str(&self.context);
            out.push('\n');
        }
        if !self.content.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&self.content);
            out.push('\n');
        }
        if !self.footer.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&self.footer);
            out.push('\n');
        }
        out
    }
}

/// Unified command output that can be rendered in any mode (INV-OUTPUT-003).
///
/// Every command builds a `CommandOutput` containing all three representations.
/// The caller renders the appropriate one based on the resolved `OutputMode`.
///
/// INV-OUTPUT-003: JSON contains all information available in Human and Agent modes.
///
/// **ACP (INV-BUDGET-007)**: Commands that adopt Action-Centric Projection
/// use `CommandOutput::with_projection()` to attach an `ActionProjection`.
/// When present, `render_projected()` uses ACP rendering instead of
/// byte-level truncation. Legacy commands work unchanged.
#[derive(Clone, Debug)]
pub struct CommandOutput {
    /// Structured JSON representation (all fields).
    pub json: serde_json::Value,
    /// Agent-mode three-part structure.
    pub agent: AgentOutput,
    /// Human-readable formatted string.
    pub human: String,
}

impl CommandOutput {
    /// Render the output in the specified mode (legacy path).
    pub fn render(&self, mode: OutputMode) -> String {
        match mode {
            OutputMode::Json => {
                serde_json::to_string_pretty(&self.json).unwrap_or_else(|_| "{}".to_string())
            }
            OutputMode::Agent => self.agent.render(),
            OutputMode::Human => self.human.clone(),
        }
    }

    /// Render with an ACP projection override (INV-BUDGET-007).
    ///
    /// When a projection is provided:
    /// - JSON: merges `_acp` field into the existing JSON
    /// - Agent: uses `projection.project_at_strategy(strategy)` for budget-constrained text
    /// - Human: uses `projection.project(MAX)` for full output
    ///
    /// When no projection is provided, falls back to `render()`.
    pub fn render_projected(
        &self,
        mode: OutputMode,
        projection: Option<&braid_kernel::ActionProjection>,
        strategy: braid_kernel::ActivationStrategy,
    ) -> String {
        match projection {
            None => self.render(mode),
            Some(proj) => match mode {
                OutputMode::Json => {
                    let mut json = self.json.clone();
                    if let serde_json::Value::Object(ref mut map) = json {
                        let acp_json = proj.to_json();
                        if let serde_json::Value::Object(acp_map) = acp_json {
                            for (k, v) in acp_map {
                                map.insert(k, v);
                            }
                        }
                    }
                    serde_json::to_string_pretty(&json)
                        .unwrap_or_else(|_| "{}".to_string())
                }
                OutputMode::Agent => proj.project_at_strategy(strategy),
                OutputMode::Human => proj.project(usize::MAX),
            },
        }
    }

    /// Create a simple command output from a human string, deriving agent and JSON.
    ///
    /// Use this for commands that haven't been fully ported to tri-mode output yet.
    /// The agent output uses the human string as content; the JSON wraps it as {"output": "..."}.
    pub fn from_human(human: String) -> Self {
        let json = serde_json::json!({ "output": &human });
        let agent = AgentOutput {
            context: String::new(),
            content: human.clone(),
            footer: String::new(),
        };
        CommandOutput { json, agent, human }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // INV-OUTPUT-001: Mode resolution determinism
    #[test]
    fn resolve_mode_explicit_flag_wins() {
        assert_eq!(resolve_mode(Some("json")), OutputMode::Json);
        assert_eq!(resolve_mode(Some("agent")), OutputMode::Agent);
        assert_eq!(resolve_mode(Some("human")), OutputMode::Human);
        assert_eq!(resolve_mode(Some("JSON")), OutputMode::Json); // case-insensitive
    }

    #[test]
    fn resolve_mode_invalid_flag_falls_through() {
        // Invalid flag falls through to env/tty detection
        let mode = resolve_mode(Some("invalid"));
        // In test context (non-TTY), should resolve to Agent (primary consumer is AI)
        assert!(mode == OutputMode::Agent || mode == OutputMode::Human);
    }

    #[test]
    fn output_mode_display() {
        assert_eq!(OutputMode::Json.to_string(), "json");
        assert_eq!(OutputMode::Agent.to_string(), "agent");
        assert_eq!(OutputMode::Human.to_string(), "human");
    }

    #[test]
    fn output_mode_parse() {
        assert_eq!("json".parse::<OutputMode>().unwrap(), OutputMode::Json);
        assert_eq!("agent".parse::<OutputMode>().unwrap(), OutputMode::Agent);
        assert_eq!("human".parse::<OutputMode>().unwrap(), OutputMode::Human);
        assert!("invalid".parse::<OutputMode>().is_err());
    }

    #[test]
    fn agent_output_render_three_parts() {
        let agent = AgentOutput {
            context: "store: 7427 datoms".into(),
            content: "tasks: 5 open".into(),
            footer: "next: braid task ready".into(),
        };
        let rendered = agent.render();
        assert!(rendered.contains("store: 7427 datoms"));
        assert!(rendered.contains("tasks: 5 open"));
        assert!(rendered.contains("next: braid task ready"));
    }

    #[test]
    fn agent_output_render_empty_parts_omitted() {
        let agent = AgentOutput {
            context: String::new(),
            content: "result".into(),
            footer: String::new(),
        };
        let rendered = agent.render();
        assert_eq!(rendered, "result\n");
    }

    #[test]
    fn command_output_render_modes() {
        let co = CommandOutput {
            json: serde_json::json!({"datoms": 100}),
            agent: AgentOutput {
                context: "ctx".into(),
                content: "body".into(),
                footer: "foot".into(),
            },
            human: "Human output here".into(),
        };
        assert!(co.render(OutputMode::Json).contains("\"datoms\""));
        assert!(co.render(OutputMode::Agent).contains("body"));
        assert_eq!(co.render(OutputMode::Human), "Human output here");
    }

    #[test]
    fn command_output_from_human() {
        let co = CommandOutput::from_human("hello world".into());
        assert_eq!(co.human, "hello world");
        assert!(co.render(OutputMode::Json).contains("hello world"));
        assert!(co.render(OutputMode::Agent).contains("hello world"));
    }

    #[test]
    fn default_mode_is_agent() {
        assert_eq!(OutputMode::default(), OutputMode::Agent);
    }
}
