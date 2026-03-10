/// Output mode dispatch for CLI responses.
///
/// Three modes per INV-INTERFACE-009:
/// - `Json`: Machine-parseable JSON output
/// - `Agent`: LLM-native output with guidance footers
/// - `Human`: Formatted human-readable output
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputMode {
    /// Machine-parseable JSON.
    Json,
    /// LLM-native output with guidance footer.
    Agent,
    /// Formatted human-readable output.
    #[default]
    Human,
}
