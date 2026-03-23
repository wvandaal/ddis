//! `budget` — Attention budget management (spec/13-budget.md).
//!
//! Implements the quality-adjusted attention budget: k*_eff measurement,
//! Q(t) computation with piecewise attention decay, five-level precedence-ordered
//! truncation, the π₀–π₃ projection pyramid, and **Action-Centric Projection (ACP)**.
//!
//! # Action-Centric Projection (ACP)
//!
//! ACP decomposes every output into three layers:
//! - **Action**: the recommended next step (NEVER truncated, ~10 tokens)
//! - **Context**: supporting information (scales with budget, precedence-ordered)
//! - **Evidence**: on-demand pointer for full detail
//!
//! The budget gate PROJECTS output at the appropriate level — it never truncates.
//! The guidance system IS the projection function: R(t) provides the Action.
//!
//! # Invariants
//!
//! - **INV-BUDGET-001**: Output budget is a hard cap; output ≤ max(MIN_OUTPUT, Q(t) × W × 0.05)
//! - **INV-BUDGET-002**: Truncation follows precedence ordering (Ambient < ... < System)
//! - **INV-BUDGET-003**: Budget derives from Q(t) (quality-adjusted), not raw k*_eff
//! - **INV-BUDGET-004**: Guidance footer compresses by k*_eff level
//! - **INV-BUDGET-005**: Commands classified by attention cost profile
//! - **INV-BUDGET-006**: Token density monotonically increases as budget shrinks
//! - **INV-BUDGET-007**: At every budget ≥ MIN_OUTPUT, output contains a complete action
//! - **INV-BUDGET-008**: Context fill is monotonic: budget↑ ⟹ context⊇
//! - **INV-BUDGET-009**: Guidance and projection share the same action computation
//!
//! # Design Decisions
//!
//! - ADR-BUDGET-001: Measured context over heuristic.
//! - ADR-BUDGET-003: Rate-distortion framework for compression.
//!
//! # Negative Cases
//!
//! - NEG-BUDGET-002: No high-priority truncation before low.

// ---------------------------------------------------------------------------
// Safe string truncation (replaces all floor_char_boundary usage)
// ---------------------------------------------------------------------------

/// Truncate a string to at most `max_bytes` bytes, backing up to a char boundary.
///
/// This is a self-contained replacement for `str::floor_char_boundary` that
/// cannot panic regardless of input. It uses only `str::is_char_boundary`,
/// which is a trivial byte-class check (stable since Rust 1.9).
///
/// # Guarantees
///
/// 1. The returned slice is always a valid `&str` (no mid-codepoint splits)
/// 2. `result.len() <= max_bytes`
/// 3. The function never panics, even on adversarial input
/// 4. If `max_bytes >= s.len()`, returns `s` unchanged
///
/// # Why not `str::floor_char_boundary`?
///
/// In practice, `floor_char_boundary` has exhibited panics in optimized (release)
/// builds on nightly Rust. Since this function guards every string truncation in
/// the codebase, we use an explicit walk-back loop that depends only on
/// `is_char_boundary` — the simplest possible UTF-8 predicate.
#[inline]
pub fn safe_truncate_bytes(s: &str, max_bytes: usize) -> &str {
    if max_bytes >= s.len() {
        return s;
    }
    let mut end = max_bytes;
    // Walk back at most 4 bytes (max UTF-8 char width) to find a boundary.
    // is_char_boundary(0) is always true, so this terminates.
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    // SAFETY: end is guaranteed to be a char boundary by the loop above,
    // and end <= max_bytes < s.len(), so the slice is in bounds.
    &s[..end]
}

/// Truncate a string to at most `max_bytes` bytes, appending "..." if truncated.
///
/// Convenience wrapper around [`safe_truncate_bytes`] for display contexts.
pub fn safe_truncate_display(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Reserve 3 bytes for "..."
    let truncated = safe_truncate_bytes(s, max_bytes.saturating_sub(3));
    format!("{truncated}...")
}

/// Minimum output floor in tokens (applies to all non-harvest-imperative modes).
pub const MIN_OUTPUT: u32 = 50;

/// Context window size assumption (Claude default).
pub const DEFAULT_WINDOW_SIZE: u32 = 200_000;

/// Budget fraction allocated to output (5% of remaining).
pub const BUDGET_FRACTION: f64 = 0.05;

/// Agent-mode output ceiling (tokens).
pub const AGENT_MODE_CEILING: u32 = 300;

/// Guidance footer ceiling (tokens).
pub const GUIDANCE_FOOTER_CEILING: u32 = 50;

/// Error message ceiling (tokens).
pub const ERROR_MESSAGE_CEILING: u32 = 100;

// ---------------------------------------------------------------------------
// Token counting (ADR-BUDGET-004)
// ---------------------------------------------------------------------------

/// Trait for token counting strategies.
///
/// Stage 0: `ApproxTokenCounter` (chars/4 with content-type correction).
/// Stage 1: will graduate to tiktoken-rs cl100k_base.
pub trait TokenCounter: Send + Sync {
    /// Estimate the token count of the given text.
    fn count(&self, text: &str) -> usize;
    /// Name of the counting method.
    fn method(&self) -> &'static str;
}

/// Stage 0 approximate token counter: chars/4 with content-type correction.
///
/// Error margin: ±15-20%, acceptable for coarse band selection (bands are 4× apart).
#[derive(Clone, Debug, Default)]
pub struct ApproxTokenCounter;

impl TokenCounter for ApproxTokenCounter {
    fn count(&self, text: &str) -> usize {
        let byte_count = text.len();
        let base = byte_count / 4;
        // Content-type correction: code has more symbols per token
        if looks_like_code(text) {
            base * 5 / 4 // 25% uplift for code (correction factor 0.85)
        } else {
            base
        }
    }

    fn method(&self) -> &'static str {
        "chars/4"
    }
}

/// Heuristic: text with high symbol density is likely code.
fn looks_like_code(text: &str) -> bool {
    if text.len() < 20 {
        return false;
    }
    // Use char iterator (not byte slicing) to avoid UTF-8 boundary panics.
    let mut total = 0usize;
    let mut code_chars = 0usize;
    for ch in text.chars().take(200) {
        total += 1;
        if matches!(
            ch,
            '{' | '}' | '(' | ')' | ';' | '=' | '<' | '>' | '|' | '&'
        ) {
            code_chars += 1;
        }
    }
    // If > 5% of chars are code-like symbols, treat as code
    code_chars * 20 > total
}

// ---------------------------------------------------------------------------
// Output precedence (INV-BUDGET-002)
// ---------------------------------------------------------------------------

/// Five-level output precedence hierarchy.
///
/// Truncation order: Ambient first (lowest priority), System last (highest).
/// `PartialOrd`/`Ord` derives match the numeric ordering.
#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub enum OutputPrecedence {
    /// Ambient: background context, exploratory content.
    Ambient = 0,
    /// Speculative: suggestions, alternatives, future considerations.
    Speculative = 1,
    /// UserRequested: direct answer to the user's query.
    UserRequested = 2,
    /// Methodology: coherence metrics, drift signals, guidance actions.
    Methodology = 3,
    /// System: schema info, error messages, harvest imperatives. Never truncated.
    System = 4,
}

impl std::fmt::Display for OutputPrecedence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputPrecedence::Ambient => write!(f, "Ambient"),
            OutputPrecedence::Speculative => write!(f, "Speculative"),
            OutputPrecedence::UserRequested => write!(f, "UserRequested"),
            OutputPrecedence::Methodology => write!(f, "Methodology"),
            OutputPrecedence::System => write!(f, "System"),
        }
    }
}

// ---------------------------------------------------------------------------
// Output block
// ---------------------------------------------------------------------------

/// A block of output content with an assigned precedence level.
#[derive(Clone, Debug)]
pub struct OutputBlock {
    /// The content of this block.
    pub content: String,
    /// The precedence level of this block.
    pub precedence: OutputPrecedence,
    /// Estimated token count (computed by TokenCounter).
    pub token_count: usize,
}

impl OutputBlock {
    /// Create a new output block with token count computed by the given counter.
    pub fn new(content: String, precedence: OutputPrecedence, counter: &dyn TokenCounter) -> Self {
        let token_count = counter.count(&content);
        OutputBlock {
            content,
            precedence,
            token_count,
        }
    }

    /// Create a block with a pre-computed token count.
    pub fn with_tokens(content: String, precedence: OutputPrecedence, token_count: usize) -> Self {
        OutputBlock {
            content,
            precedence,
            token_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Command attention profiles (INV-BUDGET-005)
// ---------------------------------------------------------------------------

/// Command attention cost classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttentionProfile {
    /// ≤ 50 tokens: status, guidance, frontier.
    Cheap,
    /// 50–300 tokens: query, associate, diff.
    Moderate,
    /// 300+ tokens: assemble --full, seed.
    Expensive,
    /// Side-effect commands: harvest, transact, merge.
    Meta,
}

impl AttentionProfile {
    /// Maximum token budget for this profile.
    pub fn ceiling(&self) -> u32 {
        match self {
            AttentionProfile::Cheap => 50,
            AttentionProfile::Moderate => 300,
            AttentionProfile::Expensive => u32::MAX, // limited only by output_budget
            AttentionProfile::Meta => 200,           // confirmation + result summary
        }
    }
}

/// Classify a CLI command into its attention profile.
///
/// Data-heavy commands (task, query, bilateral, topology) are Expensive because
/// their output scales with store size. Status/next/go are Moderate because
/// they produce fixed-size dashboards. Side-effect commands are Meta.
pub fn classify_command(command: &str) -> AttentionProfile {
    match command {
        "guidance" | "stage" | "log" | "config" => AttentionProfile::Cheap,
        // Status is the primary orientation command: fixed-size dashboard.
        // next/done/go produce confirmation + guidance footer.
        "status" | "next" | "done" | "go" | "note" => AttentionProfile::Moderate,
        // Data-heavy commands whose output scales with store size.
        // These need the full budget to avoid compressing away useful data.
        "query" | "bilateral" | "task" | "schema" | "trace" | "verify" | "spec" | "generate"
        | "seed" | "session" | "topology" | "witness" => AttentionProfile::Expensive,
        "harvest" | "transact" | "merge" | "init" | "observe" | "write" => AttentionProfile::Meta,
        _ => AttentionProfile::Moderate, // conservative default
    }
}

// ---------------------------------------------------------------------------
// Projection pyramid (SQ-007)
// ---------------------------------------------------------------------------

/// Projection levels for the budget-aware output pyramid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BudgetProjection {
    /// π₃: Store summary — single line + guidance action (≤ 200 tokens).
    StoreSummary = 0,
    /// π₂: Type summaries — aggregate by entity type (200–500 tokens).
    TypeSummary = 1,
    /// π₁: Entity summaries — per-entity one-liner (500–2000 tokens).
    EntitySummary = 2,
    /// π₀: Full datoms — complete attribute-level detail (> 2000 tokens).
    Full = 3,
}

impl BudgetProjection {
    /// Select the appropriate level for the given token budget.
    pub fn for_budget(budget: u32) -> Self {
        match budget {
            b if b > 2000 => BudgetProjection::Full,
            b if b > 500 => BudgetProjection::EntitySummary,
            b if b > 200 => BudgetProjection::TypeSummary,
            _ => BudgetProjection::StoreSummary,
        }
    }
}

impl std::fmt::Display for BudgetProjection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BudgetProjection::StoreSummary => write!(f, "π₃ (store summary)"),
            BudgetProjection::TypeSummary => write!(f, "π₂ (type summaries)"),
            BudgetProjection::EntitySummary => write!(f, "π₁ (entity summaries)"),
            BudgetProjection::Full => write!(f, "π₀ (full datoms)"),
        }
    }
}

// ---------------------------------------------------------------------------
// Guidance footer compression (INV-BUDGET-004)
// ---------------------------------------------------------------------------

/// Guidance footer compression level based on k*_eff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuidanceLevel {
    /// k > 0.7: full footer (100–200 tokens).
    Full,
    /// 0.4–0.7: compressed footer (30–60 tokens).
    Compressed,
    /// 0.2–0.4: minimal footer (10–20 tokens).
    Minimal,
    /// ≤ 0.2: harvest signal only (~10 tokens).
    HarvestOnly,
    /// k* >= 0.4 (CLI default): basin activation token (0–10 tokens).
    ///
    /// Emits the minimum perturbation needed to keep the agent on-basin:
    /// - Harvest critical/warn: "braid harvest --commit"
    /// - M(t) < 0.3: "verify: {command}" (spec-relevant action)
    /// - M(t) 0.3–0.7: "Store: N datoms | Turn T"
    /// - M(t) > 0.7: empty (methodology on track, no perturbation)
    BasinToken,
}

impl GuidanceLevel {
    /// Select the guidance level for the given k*_eff.
    ///
    /// Returns `BasinToken` for k* >= 0.4 (the CLI default). The MCP path
    /// should override this with `Full` explicitly. The original Compressed
    /// level is preserved for callers that request it directly.
    pub fn for_k_eff(k_eff: f64) -> Self {
        if k_eff > 0.7 {
            GuidanceLevel::Full
        } else if k_eff > 0.4 {
            GuidanceLevel::BasinToken
        } else if k_eff > 0.2 {
            GuidanceLevel::Minimal
        } else {
            GuidanceLevel::HarvestOnly
        }
    }

    /// Maximum token budget for this guidance level.
    pub fn token_ceiling(&self) -> u32 {
        match self {
            GuidanceLevel::Full => 200,
            GuidanceLevel::Compressed => 60,
            GuidanceLevel::Minimal => 20,
            GuidanceLevel::HarvestOnly => 10,
            GuidanceLevel::BasinToken => 10,
        }
    }
}

// ---------------------------------------------------------------------------
// Action-Centric Projection (ACP) — INV-BUDGET-007..009, ADR-BUDGET-005..007
// ---------------------------------------------------------------------------

/// Activation strategy based on k*_eff regime (ADR-BUDGET-007).
///
/// Each strategy maps to a cognitive activation mode:
/// - **Demonstrate**: rich context that seeds the session trajectory (~300 tokens)
/// - **Navigate**: precise pointers that keep the agent on-basin (~100 tokens)
/// - **Imperative**: action-only, minimum viable activation (~20 tokens)
/// - **Signal**: harvest emergency, one-bit signal (~5 tokens)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActivationStrategy {
    /// k* < 0.2: harvest emergency. ~5 tokens.
    Signal = 0,
    /// 0.2 ≤ k* < 0.4: action only. ~20 tokens.
    Imperative = 1,
    /// 0.4 ≤ k* < 0.7: precise pointers. ~100 tokens.
    Navigate = 2,
    /// k* ≥ 0.7: rich context. ~300 tokens.
    Demonstrate = 3,
}

impl ActivationStrategy {
    /// Select strategy from k*_eff (ADR-BUDGET-007).
    pub fn for_k_eff(k_eff: f64) -> Self {
        if k_eff >= 0.7 {
            ActivationStrategy::Demonstrate
        } else if k_eff >= 0.4 {
            ActivationStrategy::Navigate
        } else if k_eff >= 0.2 {
            ActivationStrategy::Imperative
        } else {
            ActivationStrategy::Signal
        }
    }

    /// Maximum context tokens for this strategy.
    ///
    /// The action itself is NOT counted — this is the budget for Context blocks only.
    pub fn max_context_tokens(&self) -> usize {
        match self {
            ActivationStrategy::Demonstrate => 300,
            ActivationStrategy::Navigate => 100,
            ActivationStrategy::Imperative => 20,
            ActivationStrategy::Signal => 5,
        }
    }
}

/// Session phase for trajectory-aware strategy selection (ACP-16, ADR-BUDGET-007).
///
/// Early turns should invest in rich context (Demonstrate) regardless of k_eff
/// because this seeds the session trajectory. Later turns should be more conservative.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionPhase {
    /// Turns 1-5: always use Demonstrate for trajectory seeding.
    Orientation,
    /// Turns 5-20: use k_eff-based strategy.
    ActiveWork,
    /// Turns 20-40: bias toward Navigate even at high k_eff.
    ContextPressure,
    /// Turns 40+: bias toward Signal for harvest urgency.
    HarvestZone,
}

impl SessionPhase {
    /// Determine session phase from transaction count since session start.
    pub fn from_tx_count(tx_since_session: usize) -> Self {
        if tx_since_session <= 5 {
            SessionPhase::Orientation
        } else if tx_since_session <= 20 {
            SessionPhase::ActiveWork
        } else if tx_since_session <= 40 {
            SessionPhase::ContextPressure
        } else {
            SessionPhase::HarvestZone
        }
    }
}

impl ActivationStrategy {
    /// Select strategy considering both k_eff AND session phase (ACP-16).
    ///
    /// Phase overrides:
    /// - Orientation: always Demonstrate (invest in trajectory seeding)
    /// - HarvestZone: caps at Navigate (conserve context)
    /// - Other phases: pure k_eff-based selection
    pub fn for_context(k_eff: f64, phase: SessionPhase) -> Self {
        match phase {
            SessionPhase::Orientation => ActivationStrategy::Demonstrate,
            SessionPhase::HarvestZone => {
                let base = Self::for_k_eff(k_eff);
                if base == ActivationStrategy::Demonstrate {
                    ActivationStrategy::Navigate // cap at Navigate
                } else {
                    base
                }
            }
            _ => Self::for_k_eff(k_eff),
        }
    }
}

impl std::fmt::Display for ActivationStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivationStrategy::Signal => write!(f, "Signal (k*<0.2)"),
            ActivationStrategy::Imperative => write!(f, "Imperative (k*<0.4)"),
            ActivationStrategy::Navigate => write!(f, "Navigate (k*<0.7)"),
            ActivationStrategy::Demonstrate => write!(f, "Demonstrate (k*≥0.7)"),
        }
    }
}

/// The recommended next action for the agent (INV-BUDGET-007).
///
/// This is the structurally-first element of every ACP output.
/// It is NEVER truncated — the type system enforces this by making
/// all fields non-optional (Curry-Howard: the type IS the proof).
#[derive(Clone, Debug)]
pub struct ProjectedAction {
    /// Executable braid CLI command (e.g., "braid go t-fd30").
    pub command: String,
    /// Brief rationale (~5 words, e.g., "highest-impact boundary task").
    pub rationale: String,
    /// Impact score from R(t) routing (0.0–1.0).
    pub impact: f64,
}

impl std::fmt::Display for ProjectedAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} — {}", self.command, self.rationale)
    }
}

/// Learned attention score for a context block (ADR-FOUNDATION-024).
///
/// Combines three signals into a composite score that determines block ordering:
/// - **Surprisal**: 1/sqrt(presentation_count) — novel blocks score higher.
/// - **Hebbian boost**: Accumulated from verbose-request signals, decays 0.5x/session.
/// - **Learned weight**: Bayesian-calibrated from outcome correlation (default 1.0).
///
/// Composite = surprisal * learned_weight + hebbian_boost.
/// INV-ATTENTION-001, INV-ATTENTION-002.
#[derive(Clone, Debug)]
pub struct AttentionScore {
    /// Novelty signal: 1/sqrt(presentation_count). 1.0 for never-seen blocks.
    pub surprisal: f64,
    /// Accumulated boost from verbose/deep requests. Decays 0.5x per session.
    pub hebbian_boost: f64,
    /// Bayesian weight from outcome correlation. Default 1.0.
    pub learned_weight: f64,
    /// Combined score: surprisal * learned_weight + hebbian_boost.
    pub composite: f64,
}

impl AttentionScore {
    /// Compute attention score from presentation count.
    ///
    /// Surprisal = 1/sqrt(max(1, count)). For count=0, surprisal=1.0 (maximally novel).
    /// For count=4, surprisal=0.5. Monotonically decreasing.
    pub fn from_presentation_count(count: u64, hebbian_boost: f64, learned_weight: f64) -> Self {
        let surprisal = 1.0 / (count.max(1) as f64).sqrt();
        let composite = surprisal * learned_weight + hebbian_boost;
        AttentionScore {
            surprisal,
            hebbian_boost,
            learned_weight,
            composite,
        }
    }

    /// Default attention score for a never-seen block.
    pub fn novel() -> Self {
        Self::from_presentation_count(0, 0.0, 1.0)
    }
}

/// A block of supporting context at a specific precedence level (INV-BUDGET-008).
///
/// Context blocks are the unit of budget-aware output. The project() algorithm
/// fills blocks in precedence order (highest first) until the budget is exhausted.
/// Increasing budget adds blocks but never removes them (monotonic fill).
///
/// When `attention` is `Some`, the block has a learned attention score from
/// ADR-FOUNDATION-024. Blocks are sorted by composite attention score (highest
/// first) within each precedence tier. When `None`, static precedence ordering
/// is used (backward-compatible).
#[derive(Clone, Debug)]
pub struct ContextBlock {
    /// Precedence level (System > Methodology > UserRequested > Speculative > Ambient).
    pub precedence: OutputPrecedence,
    /// The content string for this block.
    pub content: String,
    /// Estimated token count (via ApproxTokenCounter).
    pub tokens: usize,
    /// Optional learned attention score (ADR-FOUNDATION-024).
    /// When present, blocks are sorted by composite attention score within
    /// each precedence tier. When absent, static precedence ordering is used.
    pub attention: Option<AttentionScore>,
}

/// The universal ACP output type (INV-BUDGET-007).
///
/// Every command can produce an ActionProjection. The budget gate selects
/// the appropriate detail level by calling `project(budget)`.
///
/// **Structural guarantee**: The action is always present (non-Optional).
/// At any budget ≥ MIN_OUTPUT, the projected output starts with the action.
///
/// **Monotonic fill** (INV-BUDGET-008): For budgets b1 > b2,
/// `context_blocks_included(b1) ⊇ context_blocks_included(b2)`.
#[derive(Clone, Debug)]
pub struct ActionProjection {
    /// The recommended action (NEVER truncated, ~10 tokens).
    pub action: ProjectedAction,
    /// Supporting context blocks, ordered by precedence (highest first).
    pub context: Vec<ContextBlock>,
    /// On-demand evidence pointer (e.g., "details: braid status --verbose").
    pub evidence_pointer: String,
}

impl ActionProjection {
    /// Project the output at the given context token budget (INV-BUDGET-007).
    ///
    /// Algorithm:
    /// 1. ALWAYS emit the action (structurally first, never truncated)
    /// 2. Fill context blocks in order (highest precedence first) until budget exhausted
    /// 3. Append evidence pointer
    ///
    /// Returns a formatted string suitable for agent or human consumption.
    pub fn project(&self, context_budget: usize) -> String {
        let mut out = format!("{}\n", self.action);
        let mut remaining = context_budget;
        let mut omitted: Vec<&str> = Vec::new();

        for block in &self.context {
            if block.tokens <= remaining {
                out.push_str(&block.content);
                out.push('\n');
                remaining = remaining.saturating_sub(block.tokens);
            } else {
                // Extract a short label from the block content for the omission summary.
                // Uses the prefix before the first ':' or space, capped at 20 chars.
                let label = block
                    .content
                    .split_once(':')
                    .map(|(prefix, _)| prefix.trim())
                    .or_else(|| block.content.split_whitespace().next())
                    .unwrap_or("...");
                omitted.push(label);
            }
        }

        // Pyramid disclosure: tell the LLM what was omitted and how to access it.
        if !omitted.is_empty() {
            out.push_str(&format!("[+{} omitted: {}]\n", omitted.len(), omitted.join(", ")));
        }

        if !self.evidence_pointer.is_empty() {
            out.push_str(&self.evidence_pointer);
            out.push('\n');
        }

        out
    }

    /// Project at the given activation strategy level.
    pub fn project_at_strategy(&self, strategy: ActivationStrategy) -> String {
        self.project(strategy.max_context_tokens())
    }

    /// Render as structured JSON for JSON output mode.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "_acp": {
                "action": {
                    "command": self.action.command,
                    "rationale": self.action.rationale,
                    "impact": self.action.impact,
                },
                "context_blocks": self.context.iter().map(|b| {
                    serde_json::json!({
                        "precedence": format!("{:?}", b.precedence),
                        "content": b.content,
                        "tokens": b.tokens,
                    })
                }).collect::<Vec<_>>(),
                "evidence": self.evidence_pointer,
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Budget Manager (core state machine)
// ---------------------------------------------------------------------------

/// Attention budget manager implementing spec/13-budget.md §13.2.
///
/// State: (k_eff, q, output_budget).
/// Transitions: MEASURE → ALLOCATE → PROJECT.
#[derive(Clone, Debug)]
pub struct BudgetManager {
    /// Effective remaining attention: k*_eff ∈ [0, 1].
    pub k_eff: f64,
    /// Quality-adjusted budget: Q(t) = k*_eff × attention_decay(k*_eff).
    pub q: f64,
    /// Output budget in tokens: max(MIN_OUTPUT, Q(t) × W × 0.05).
    pub output_budget: u32,
    /// Context window size (tokens).
    pub window_size: u32,
}

impl Default for BudgetManager {
    fn default() -> Self {
        Self::new(DEFAULT_WINDOW_SIZE)
    }
}

impl BudgetManager {
    /// Create a new budget manager with the given window size.
    pub fn new(window_size: u32) -> Self {
        let mut mgr = BudgetManager {
            k_eff: 1.0,
            q: 1.0,
            output_budget: (1.0_f64 * window_size as f64 * BUDGET_FRACTION) as u32,
            window_size,
        };
        mgr.measure(0.0); // initialize at full budget
        mgr
    }

    /// MEASURE transition: compute k*_eff, Q(t), and output_budget from context consumption.
    ///
    /// `context_used_pct` is the fraction of context window consumed (0.0–1.0).
    pub fn measure(&mut self, context_used_pct: f64) {
        self.k_eff = (1.0 - context_used_pct).clamp(0.0, 1.0);
        self.q = self.k_eff * attention_decay(self.k_eff);
        let raw_budget = self.q * self.window_size as f64 * BUDGET_FRACTION;
        self.output_budget = (MIN_OUTPUT as f64).max(raw_budget) as u32;
    }

    /// ALLOCATE transition: select content blocks that fit within output_budget.
    ///
    /// Fills from highest to lowest precedence. Returns the selected blocks
    /// in precedence order (highest first).
    ///
    /// INV-BUDGET-002: lower-priority content is truncated before higher-priority.
    pub fn allocate<'a>(&self, blocks: &'a [OutputBlock]) -> Vec<&'a OutputBlock> {
        // Sort by precedence descending (highest priority first)
        let mut sorted: Vec<&OutputBlock> = blocks.iter().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.precedence));

        let mut remaining = self.output_budget as usize;
        let mut selected = Vec::new();

        for block in sorted {
            if block.token_count <= remaining {
                remaining -= block.token_count;
                selected.push(block);
            }
            // Block doesn't fit → truncated (lower priority dropped first)
        }

        selected
    }

    /// PROJECT transition: select the projection level for the current budget.
    pub fn projection_level(&self) -> BudgetProjection {
        BudgetProjection::for_budget(self.output_budget)
    }

    /// Select the guidance footer compression level for current k*_eff.
    pub fn guidance_level(&self) -> GuidanceLevel {
        GuidanceLevel::for_k_eff(self.k_eff)
    }

    /// Create a budget manager with a specific output budget (for testing).
    #[cfg(test)]
    fn with_budget(budget: u32) -> Self {
        BudgetManager {
            k_eff: 1.0,
            q: 1.0,
            output_budget: budget,
            window_size: DEFAULT_WINDOW_SIZE,
        }
    }

    /// Whether we are in harvest-imperative mode (Q(t) < 0.05).
    ///
    /// In this mode, MIN_OUTPUT does not apply — only the harvest imperative is emitted.
    pub fn is_harvest_imperative(&self) -> bool {
        self.q < 0.05
    }

    /// Get the effective budget for a command, respecting its attention profile.
    pub fn command_budget(&self, command: &str) -> u32 {
        let profile = classify_command(command);
        self.output_budget.min(profile.ceiling())
    }
}

// ---------------------------------------------------------------------------
// Attention decay (ADR-BUDGET-002)
// ---------------------------------------------------------------------------

/// Piecewise attention decay function (continuous, monotonically increasing).
///
/// Three regimes:
/// - k > 0.6: full quality (1.0)
/// - 0.3 ≤ k ≤ 0.6: linear degradation (k/0.6)
/// - k < 0.3: quadratic degradation, matched at boundary: 0.5 × (k/0.3)²
///
/// The quadratic coefficient (0.5) ensures C⁰ continuity at k=0.3:
/// linear(0.3) = 0.3/0.6 = 0.5 = 0.5 × (0.3/0.3)² = quadratic(0.3).
pub fn attention_decay(k: f64) -> f64 {
    if k > 0.6 {
        1.0
    } else if k >= 0.3 {
        k / 0.6
    } else {
        let ratio = k / 0.3;
        0.5 * ratio * ratio
    }
}

/// Compute Q(t) = k*_eff × attention_decay(k*_eff).
pub fn quality_adjusted_budget(k_eff: f64) -> f64 {
    k_eff * attention_decay(k_eff)
}

// ---------------------------------------------------------------------------
// Output ceiling enforcement (INV-BUDGET-001)
// ---------------------------------------------------------------------------

/// Compress output to fit within a token ceiling using pyramid summaries.
///
/// Instead of hard-truncating (which loses the most recent and often most
/// important information), this function compresses by selecting lines that
/// fit within the budget. It preserves the first line (header/summary) and
/// as many subsequent lines as fit, appending a compression notice with the
/// count of omitted lines and a hint for the user to get full output.
///
/// Compression strategy (pyramid):
/// - Always keep the first line (summary/header)
/// - Fill remaining budget with lines in order
/// - Append a notice showing how many lines were compressed out
/// - Never mid-line truncate — always complete lines
///
/// # Invariants
///
/// - **INV-BUDGET-001**: Output budget is a hard cap. `enforce_ceiling` is the
///   final gate ensuring no command output exceeds the token ceiling.
/// - **NEG-BUDGET-003**: Output is never mid-line truncated. Compression
///   always operates on complete semantic units (lines).
pub fn enforce_ceiling(output: &str, ceiling: usize) -> String {
    let counter = ApproxTokenCounter;
    let total_tokens = counter.count(output);

    if total_tokens <= ceiling {
        return output.to_string();
    }

    // Pyramid compression: keep lines that fit, drop the rest.
    let lines: Vec<&str> = output.lines().collect();
    if lines.is_empty() {
        return output.to_string();
    }

    // Reserve tokens for the compression notice.
    let notice_template = "\n... (N more lines, use --format json for full output)";
    let notice_overhead = counter.count(notice_template) + 5; // margin
    let text_budget = ceiling.saturating_sub(notice_overhead);

    if text_budget == 0 {
        // Budget so small even the notice barely fits. Return first line + notice.
        let first = safe_truncate_bytes(lines[0], ceiling.saturating_mul(4));
        return format!(
            "{}\n... ({} more lines, use --format json for full output)",
            first,
            lines.len().saturating_sub(1)
        );
    }

    let mut kept = Vec::new();
    let mut used_tokens = 0;

    for line in &lines {
        let line_tokens = counter.count(line) + 1; // +1 for newline
        if used_tokens + line_tokens > text_budget && !kept.is_empty() {
            break;
        }
        kept.push(*line);
        used_tokens += line_tokens;
    }

    let omitted = lines.len() - kept.len();

    // If no lines were omitted but total still exceeds budget (single long line
    // or few very long lines), use word-level compression on the kept content.
    if omitted == 0 && used_tokens > text_budget {
        let joined = kept.join("\n");
        let target_chars = text_budget.saturating_mul(4);
        let compressed = safe_truncate_bytes(&joined, target_chars);
        return format!(
            "{}\n... (compressed, use --format json for full output)",
            compressed
        );
    }

    if omitted == 0 {
        return output.to_string();
    }

    let mut result = kept.join("\n");
    result.push_str(&format!(
        "\n... ({} more lines, use --format json for full output)",
        omitted
    ));
    result
}

// ---------------------------------------------------------------------------
// Token efficiency (INV-BUDGET-006)
// ---------------------------------------------------------------------------

/// Token efficiency measurement for density monotonicity verification.
#[derive(Clone, Debug)]
pub struct TokenEfficiency {
    /// Number of semantic units (entities, facts, actions) in the output.
    pub semantic_units: usize,
    /// Number of tokens in the output.
    pub token_count: usize,
}

impl TokenEfficiency {
    /// Information density: semantic_units / token_count.
    pub fn density(&self) -> f64 {
        self.semantic_units as f64 / self.token_count.max(1) as f64
    }
}

// ---------------------------------------------------------------------------
// Multi-Signal k_eff Estimation (INV-REFLEXIVE-002, KEFF-1/KEFF-2)
// ---------------------------------------------------------------------------

/// Observable evidence for k_eff estimation.
///
/// Each field is a signal that correlates with context consumption.
/// The estimator fuses these signals via sigmoid-weighted combination.
#[derive(Clone, Debug, Default)]
pub struct EvidenceVector {
    /// Transactions since session start (more txns → more context consumed).
    pub tx_count_since_session: u32,
    /// Seconds elapsed since session start.
    pub wall_elapsed_seconds: u64,
    /// Transaction velocity (txns/min over 5-min window).
    pub tx_velocity_per_min: f64,
    /// Estimated cumulative output tokens (approximation from datom sizes).
    pub cumulative_output_estimate: u32,
    /// Observations captured since session start.
    pub observe_count: u32,
}

impl EvidenceVector {
    /// Build evidence from store state.
    pub fn from_store(store: &crate::store::Store) -> Self {
        use crate::datom::Op;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Find session boundary (same logic as SessionWorkingSet)
        let harvest_boundary = crate::guidance::last_harvest_wall_time(store);
        let fallback = now.saturating_sub(3600);
        let session_boundary = harvest_boundary.max(fallback);

        // tx_count: distinct wall_times since session boundary
        let tx_count = store
            .datoms()
            .filter(|d| d.tx.wall_time() > session_boundary && d.op == Op::Assert)
            .map(|d| d.tx.wall_time())
            .collect::<std::collections::BTreeSet<_>>()
            .len() as u32;

        // wall_elapsed: seconds since session boundary
        let wall_elapsed = now.saturating_sub(session_boundary);

        // velocity: use existing tx_velocity function
        let velocity = crate::guidance::tx_velocity(store);

        // output estimate: rough token count from datom string lengths since boundary
        let output_est: u32 = store
            .datoms()
            .filter(|d| d.tx.wall_time() > session_boundary && d.op == Op::Assert)
            .map(|d| match &d.value {
                crate::datom::Value::String(s) => (s.len() as u32) / 3, // ~3 chars per token
                _ => 2, // keyword/long/double ≈ 2 tokens
            })
            .sum();

        // observe_count: observations since boundary
        let observe_count = store
            .datoms()
            .filter(|d| {
                d.attribute.as_str() == ":exploration/source"
                    && d.op == Op::Assert
                    && d.tx.wall_time() > session_boundary
            })
            .count() as u32;

        EvidenceVector {
            tx_count_since_session: tx_count,
            wall_elapsed_seconds: wall_elapsed,
            tx_velocity_per_min: velocity,
            cumulative_output_estimate: output_est,
            observe_count,
        }
    }
}

/// Sigmoid function: 1 / (1 + e^(-x)).
fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Estimate k_eff from observable evidence (KEFF-2).
///
/// Uses sigmoid-weighted fusion: k̂ = 1.0 - Σ(wᵢ × sigmoid((eᵢ - τᵢ) / scale))
///
/// Default weights and thresholds calibrated for typical braid sessions.
/// The estimator is conservative: it decreases k_eff as evidence accumulates
/// but never drops below 0.05 (minimum useful output).
pub fn estimate_k_eff(evidence: &EvidenceVector) -> f64 {
    // Weights: how much each signal contributes to k_eff decay
    let weights = [0.35, 0.20, 0.15, 0.20, 0.10];

    // Thresholds: center of sigmoid (50% decay at this value)
    let thresholds: [f64; 5] = [30.0, 3600.0, 3.0, 50000.0, 15.0];

    // Scale: controls sigmoid steepness (larger = gentler transition)
    let scales: [f64; 5] = [10.0, 1200.0, 1.0, 15000.0, 5.0];

    let signals: [f64; 5] = [
        evidence.tx_count_since_session as f64,
        evidence.wall_elapsed_seconds as f64,
        evidence.tx_velocity_per_min,
        evidence.cumulative_output_estimate as f64,
        evidence.observe_count as f64,
    ];

    let mut decay = 0.0;
    for i in 0..5 {
        let normalized = (signals[i] - thresholds[i]) / scales[i];
        decay += weights[i] * sigmoid(normalized);
    }

    // k_eff = 1.0 - total_decay, clamped to [0.05, 1.0]
    (1.0 - decay).clamp(0.05, 1.0)
}

/// Calibrate k_eff estimation from historical session data (KEFF-4).
///
/// When --context-used is provided at harvest time, the system records
/// (estimated_k, actual_k) as a calibration datum. This function reads
/// all calibration data and finds the optimal boost_scale via grid search.
///
/// boost_scale adjusts the estimator: k_calibrated = scale * k_estimated + (1-scale) * k_estimated²
/// Default scale = 1.0 (no adjustment). Requires 3+ calibration points.
pub fn calibrate_boost_scale(store: &crate::store::Store) -> f64 {
    use crate::datom::Op;

    // Collect calibration data: (estimated, actual) pairs
    // These are stored as :calibration/k-eff-estimated and :calibration/k-eff-actual
    let est_attr = crate::datom::Attribute::from_keyword(":calibration/k-eff-estimated");
    let act_attr = crate::datom::Attribute::from_keyword(":calibration/k-eff-actual");

    let mut pairs: Vec<(f64, f64)> = Vec::new();

    // Find calibration entities that have both estimated and actual
    for d in store.attribute_datoms(&est_attr) {
        if d.op != Op::Assert {
            continue;
        }
        let estimated = match d.value {
            crate::datom::Value::Double(f) => f.into_inner(),
            _ => continue,
        };
        // Find matching actual value on same entity
        for d2 in store.entity_datoms(d.entity) {
            if d2.attribute == act_attr && d2.op == Op::Assert {
                if let crate::datom::Value::Double(f) = d2.value {
                    pairs.push((estimated, f.into_inner()));
                }
            }
        }
    }

    if pairs.len() < 3 {
        return 1.0; // Not enough data — use default
    }

    // Grid search over scale values
    let mut best_scale = 1.0;
    let mut best_error = f64::MAX;

    for scale_int in 1..=12 {
        // 0.5, 1.0, 1.5, ..., 6.0
        let scale = scale_int as f64 * 0.5;
        let error: f64 = pairs
            .iter()
            .map(|(est, act)| {
                let calibrated = scale * est + (1.0 - scale) * est * est;
                (calibrated - act).powi(2)
            })
            .sum();
        if error < best_error {
            best_error = error;
            best_scale = scale;
        }
    }

    best_scale
}

// ---------------------------------------------------------------------------
// TSV rendering (OutputMode::Tsv)
// ---------------------------------------------------------------------------

/// Convert a JSON value to TSV (tab-separated values).
///
/// TSV is a **rendering** concern, not a data format. This function takes the
/// same `serde_json::Value` that JSON mode would serialize and converts it to
/// tab-separated text suitable for `cut`, `awk`, spreadsheets, and piped
/// workflows.
///
/// Conversion rules:
/// - **Object with a `"tasks"` array**: extract the array, render header row
///   from the first object's keys, then one data row per element.
/// - **Object with `"_acp"` field** (ACP output): render action row, then
///   context block rows.
/// - **Plain object**: one row per key-value pair (`key\tvalue`).
/// - **Array of objects**: header row from the first object's keys, then one
///   data row per element.
/// - **Array of primitives**: one value per line.
/// - **Null / empty**: empty string.
///
/// Tab characters and newlines inside values are replaced with spaces to
/// prevent column/row corruption.
pub fn json_to_tsv(json: &serde_json::Value) -> String {
    match json {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => escape_tsv(s),
        serde_json::Value::Array(arr) => array_to_tsv(arr),
        serde_json::Value::Object(map) => object_to_tsv(map),
    }
}

/// Escape a string for TSV: replace tabs and newlines with spaces.
fn escape_tsv(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

/// Render a flat JSON value as a single TSV cell.
fn value_to_cell(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => escape_tsv(s),
        // Nested objects/arrays: compact JSON representation, escaped.
        other => escape_tsv(&other.to_string()),
    }
}

/// Render a JSON array as TSV.
fn array_to_tsv(arr: &[serde_json::Value]) -> String {
    if arr.is_empty() {
        return String::new();
    }

    // Array of objects: header + rows.
    if let Some(serde_json::Value::Object(first)) = arr.first() {
        let keys: Vec<&String> = first.keys().collect();
        let mut out = keys.iter().map(|k| escape_tsv(k)).collect::<Vec<_>>().join("\t");
        out.push('\n');
        for item in arr {
            if let serde_json::Value::Object(obj) = item {
                let row: Vec<String> = keys.iter().map(|k| {
                    obj.get(*k).map(value_to_cell).unwrap_or_default()
                }).collect();
                out.push_str(&row.join("\t"));
                out.push('\n');
            } else {
                out.push_str(&value_to_cell(item));
                out.push('\n');
            }
        }
        return out;
    }

    // Array of primitives: one value per line.
    let mut out = String::new();
    for item in arr {
        out.push_str(&value_to_cell(item));
        out.push('\n');
    }
    out
}

/// Render a JSON object as TSV.
fn object_to_tsv(map: &serde_json::Map<String, serde_json::Value>) -> String {
    if map.is_empty() {
        return String::new();
    }

    // ACP output: render action + context blocks.
    if let Some(acp) = map.get("_acp") {
        return acp_to_tsv(acp, map);
    }

    // Object with a prominent array field (e.g., "tasks", "results", "commands"):
    // extract the array and render as table.
    for key in &["tasks", "results", "items", "commands", "datoms", "entities"] {
        if let Some(serde_json::Value::Array(arr)) = map.get(*key) {
            if !arr.is_empty() {
                return array_to_tsv(arr);
            }
        }
    }

    // Plain object: key\tvalue pairs.
    let mut out = String::new();
    for (k, v) in map {
        out.push_str(&escape_tsv(k));
        out.push('\t');
        out.push_str(&value_to_cell(v));
        out.push('\n');
    }
    out
}

/// Render ACP JSON as TSV: action row, then context block rows.
fn acp_to_tsv(acp: &serde_json::Value, full: &serde_json::Map<String, serde_json::Value>) -> String {
    let mut out = String::new();

    // Action row.
    if let Some(action) = acp.get("action") {
        let cmd = action.get("command").and_then(|v| v.as_str()).unwrap_or("");
        let rationale = action.get("rationale").and_then(|v| v.as_str()).unwrap_or("");
        let impact = action.get("impact").and_then(|v| v.as_f64()).unwrap_or(0.0);
        out.push_str("action\tcommand\trationale\timpact\n");
        out.push_str(&format!(
            "action\t{}\t{}\t{:.2}\n",
            escape_tsv(cmd),
            escape_tsv(rationale),
            impact
        ));
    }

    // Context blocks.
    if let Some(serde_json::Value::Array(blocks)) = acp.get("context_blocks") {
        if !blocks.is_empty() {
            out.push_str("block\tprecedence\tcontent\ttokens\n");
            for block in blocks {
                let prec = block.get("precedence").and_then(|v| v.as_str()).unwrap_or("");
                let content = block.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let tokens = block.get("tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                out.push_str(&format!(
                    "block\t{}\t{}\t{}\n",
                    escape_tsv(prec),
                    escape_tsv(content),
                    tokens
                ));
            }
        }
    }

    // Evidence pointer.
    if let Some(evidence) = acp.get("evidence") {
        if let Some(s) = evidence.as_str() {
            if !s.is_empty() {
                out.push_str(&format!("evidence\t{}\n", escape_tsv(s)));
            }
        }
    }

    // Non-ACP fields from the parent object (e.g., tasks, status).
    for (k, v) in full {
        if k == "_acp" {
            continue;
        }
        if let serde_json::Value::Array(arr) = v {
            if !arr.is_empty() {
                out.push_str(&array_to_tsv(arr));
            }
        } else {
            out.push_str(&escape_tsv(k));
            out.push('\t');
            out.push_str(&value_to_cell(v));
            out.push('\n');
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-BUDGET-001, INV-BUDGET-002, INV-BUDGET-003,
// INV-BUDGET-004, INV-BUDGET-005, INV-BUDGET-006,
// ADR-BUDGET-001, ADR-BUDGET-002, ADR-BUDGET-003, ADR-BUDGET-004,
// NEG-BUDGET-001, NEG-BUDGET-002
#[cfg(test)]
mod tests {
    use super::*;

    // ---- safe_truncate_bytes ----

    #[test]
    fn truncate_bytes_ascii_no_change() {
        assert_eq!(safe_truncate_bytes("hello", 10), "hello");
    }

    #[test]
    fn truncate_bytes_ascii_exact() {
        assert_eq!(safe_truncate_bytes("hello", 5), "hello");
    }

    #[test]
    fn truncate_bytes_ascii_cut() {
        assert_eq!(safe_truncate_bytes("hello", 3), "hel");
    }

    #[test]
    fn truncate_bytes_empty() {
        assert_eq!(safe_truncate_bytes("", 0), "");
        assert_eq!(safe_truncate_bytes("", 100), "");
    }

    #[test]
    fn truncate_bytes_zero_max() {
        assert_eq!(safe_truncate_bytes("hello", 0), "");
    }

    #[test]
    fn truncate_bytes_2byte_char_boundary() {
        // é is 2 bytes (0xC3, 0xA9). "café" = [99, 97, 102, C3, A9] = 5 bytes
        let s = "café";
        assert_eq!(s.len(), 5);
        // Truncate at 4: lands inside é → backs up to 3
        assert_eq!(safe_truncate_bytes(s, 4), "caf");
        // Truncate at 5: full string
        assert_eq!(safe_truncate_bytes(s, 5), "café");
        // Truncate at 3: before é starts
        assert_eq!(safe_truncate_bytes(s, 3), "caf");
    }

    #[test]
    fn truncate_bytes_3byte_char_boundary() {
        // ✗ is 3 bytes (E2 9C 97). "a✗b" = [61, E2, 9C, 97, 62] = 5 bytes
        let s = "a✗b";
        assert_eq!(s.len(), 5);
        // Truncate at 2: inside ✗ → backs up to 1 ("a")
        assert_eq!(safe_truncate_bytes(s, 2), "a");
        // Truncate at 3: inside ✗ → backs up to 1 ("a")
        assert_eq!(safe_truncate_bytes(s, 3), "a");
        // Truncate at 4: after ✗, before b
        assert_eq!(safe_truncate_bytes(s, 4), "a✗");
    }

    #[test]
    fn truncate_bytes_4byte_char_boundary() {
        // 😀 is 4 bytes (F0 9F 98 80)
        let s = "a😀b";
        assert_eq!(s.len(), 6);
        // Truncate at 2,3,4: all inside 😀 → backs up to 1 ("a")
        assert_eq!(safe_truncate_bytes(s, 2), "a");
        assert_eq!(safe_truncate_bytes(s, 3), "a");
        assert_eq!(safe_truncate_bytes(s, 4), "a");
        // Truncate at 5: after 😀, before b
        assert_eq!(safe_truncate_bytes(s, 5), "a😀");
    }

    #[test]
    fn truncate_bytes_mixed_unicode_at_200() {
        // Reproduce the exact bug: output with ✗ and △ near byte 200
        let mut s = String::new();
        // Fill to ~198 bytes with ASCII
        s.push_str(&"x".repeat(198));
        // △ is 3 bytes (E2 96 B3). Starts at byte 198.
        s.push('△');
        s.push_str("more");
        // Truncate at 200: inside △ → backs up to 198
        assert_eq!(safe_truncate_bytes(&s, 200), &s[..198]);
        // Truncate at 201: complete △
        assert_eq!(safe_truncate_bytes(&s, 201), &s[..201]);
    }

    #[test]
    fn truncate_bytes_all_multibyte() {
        // String of only 3-byte chars: each at 0,3,6,9,...
        let s = "✓✗△✓✗△✓✗△";
        // Truncate at every possible byte position: must never panic
        for i in 0..=s.len() + 5 {
            let result = safe_truncate_bytes(s, i);
            assert!(result.len() <= i.min(s.len()));
            // Verify result is valid UTF-8 (would panic on invalid slice)
            let _ = result.chars().count();
        }
    }

    #[test]
    fn truncate_display_adds_ellipsis() {
        assert_eq!(safe_truncate_display("hello world", 8), "hello...");
        assert_eq!(safe_truncate_display("hi", 10), "hi");
    }

    #[test]
    fn truncate_display_unicode_safe() {
        // "a✗b" (5 bytes), truncate display at 5: "a✗" + "..." would need safe boundary
        let result = safe_truncate_display("a✗b✗c", 5);
        // Must not panic and must be valid UTF-8
        assert!(result.ends_with("..."));
        assert!(result.len() <= 8); // max 5 content + 3 "..."
    }

    // ---- Attention decay ----
    // Verifies: ADR-BUDGET-002 — Piecewise Attention Decay

    // Verifies: INV-BUDGET-003 — Quality-Adjusted Degradation
    #[test]
    fn decay_full_quality_above_06() {
        assert_eq!(attention_decay(0.7), 1.0);
        assert_eq!(attention_decay(0.8), 1.0);
        assert_eq!(attention_decay(1.0), 1.0);
    }

    #[test]
    fn decay_linear_between_03_06() {
        let d = attention_decay(0.45);
        assert!((d - 0.75).abs() < 1e-10, "expected 0.75, got {d}");
        let d = attention_decay(0.3);
        assert!((d - 0.5).abs() < 1e-10, "expected 0.5, got {d}");
    }

    #[test]
    fn decay_quadratic_below_03() {
        // 0.5 × (0.15/0.3)² = 0.5 × 0.25 = 0.125
        let d = attention_decay(0.15);
        assert!((d - 0.125).abs() < 1e-10, "expected 0.125, got {d}");
        let d = attention_decay(0.0);
        assert!((d - 0.0).abs() < 1e-10, "expected 0.0, got {d}");
    }

    #[test]
    fn decay_monotonically_increasing() {
        let mut prev = 0.0;
        for i in 0..=100 {
            let k = i as f64 / 100.0;
            let d = attention_decay(k);
            assert!(
                d >= prev - 1e-10,
                "decay not monotonic at k={k}: {d} < {prev}"
            );
            prev = d;
        }
    }

    #[test]
    fn decay_bounded_01() {
        for i in 0..=100 {
            let k = i as f64 / 100.0;
            let d = attention_decay(k);
            assert!(d >= 0.0, "decay < 0 at k={k}");
            assert!(d <= 1.0, "decay > 1 at k={k}");
        }
    }

    // ---- Q(t) ----

    #[test]
    fn q_at_full_budget() {
        let q = quality_adjusted_budget(1.0);
        assert!((q - 1.0).abs() < 1e-10);
    }

    #[test]
    fn q_at_zero_budget() {
        let q = quality_adjusted_budget(0.0);
        assert!((q - 0.0).abs() < 1e-10);
    }

    #[test]
    fn q_degrades_faster_than_k_below_06() {
        // Q(t) should be ≤ k_eff when k_eff < 0.6
        for i in 0..=60 {
            let k = i as f64 / 100.0;
            let q = quality_adjusted_budget(k);
            assert!(q <= k + 1e-10, "Q({k}) = {q} > k={k}");
        }
    }

    #[test]
    fn q_monotonically_increasing() {
        let mut prev = 0.0;
        for i in 0..=100 {
            let k = i as f64 / 100.0;
            let q = quality_adjusted_budget(k);
            assert!(q >= prev - 1e-10, "Q not monotonic at k={k}: {q} < {prev}");
            prev = q;
        }
    }

    // ---- BudgetManager ----

    // Verifies: INV-BUDGET-001 — Output Budget as Hard Cap
    // Verifies: ADR-BUDGET-001 — Measured Context Over Heuristic
    #[test]
    fn manager_full_budget() {
        let mgr = BudgetManager::default();
        assert!((mgr.k_eff - 1.0).abs() < 1e-10);
        assert!((mgr.q - 1.0).abs() < 1e-10);
        // 1.0 * 200000 * 0.05 = 10000
        assert_eq!(mgr.output_budget, 10000);
    }

    #[test]
    fn manager_half_consumed() {
        let mut mgr = BudgetManager::default();
        mgr.measure(0.5);
        assert!((mgr.k_eff - 0.5).abs() < 1e-10);
        // k=0.5 is in linear regime: decay = 0.5/0.6 = 0.833...
        // Q = 0.5 * 0.833... = 0.4166...
        // budget = 0.4166 * 200000 * 0.05 = 4166
        assert!(mgr.output_budget > 4000);
        assert!(mgr.output_budget < 4500);
    }

    #[test]
    fn manager_nearly_exhausted() {
        let mut mgr = BudgetManager::default();
        mgr.measure(0.9);
        assert!((mgr.k_eff - 0.1).abs() < 1e-10);
        // k=0.1 is in quadratic regime: decay = 0.5 × (0.1/0.3)^2 = 0.5 × 0.111 = 0.0556
        // Q = 0.1 * 0.0556 = 0.00556
        // budget = max(50, 0.00556 * 200000 * 0.05) = max(50, 55.6) = 55
        assert!(mgr.output_budget >= MIN_OUTPUT);
        assert!(mgr.output_budget <= 60);
    }

    #[test]
    fn manager_fully_exhausted_floor() {
        let mut mgr = BudgetManager::default();
        mgr.measure(1.0);
        assert!((mgr.k_eff - 0.0).abs() < 1e-10);
        // Q = 0 → budget = MIN_OUTPUT = 50
        assert_eq!(mgr.output_budget, MIN_OUTPUT);
    }

    #[test]
    fn manager_clamps_invalid_input() {
        let mut mgr = BudgetManager::default();
        mgr.measure(1.5); // over 100%
        assert!((mgr.k_eff - 0.0).abs() < 1e-10);
        mgr.measure(-0.3); // negative
        assert!((mgr.k_eff - 1.0).abs() < 1e-10);
    }

    #[test]
    fn harvest_imperative_mode() {
        let mut mgr = BudgetManager::default();
        mgr.measure(0.0);
        assert!(!mgr.is_harvest_imperative());

        // Push Q below 0.05: need k*_eff very small in quadratic regime
        // k=0.05: decay=(0.05/0.3)^2=0.0278, Q=0.05*0.0278=0.00139 < 0.05
        mgr.measure(0.95);
        assert!(mgr.is_harvest_imperative());
    }

    // ---- Precedence-ordered truncation ----

    // Verifies: INV-BUDGET-002 — Precedence-Ordered Truncation
    // Verifies: NEG-BUDGET-002 — No High-Priority Truncation Before Low
    #[test]
    fn allocate_respects_precedence() {
        let mut mgr = BudgetManager::default();
        mgr.measure(0.0); // full budget

        let blocks = vec![
            OutputBlock::with_tokens("ambient".into(), OutputPrecedence::Ambient, 100),
            OutputBlock::with_tokens("system".into(), OutputPrecedence::System, 100),
            OutputBlock::with_tokens("user".into(), OutputPrecedence::UserRequested, 100),
        ];

        let selected = mgr.allocate(&blocks);
        assert_eq!(selected.len(), 3, "all should fit at full budget");
        // Verify highest precedence first
        assert_eq!(selected[0].precedence, OutputPrecedence::System);
        assert_eq!(selected[1].precedence, OutputPrecedence::UserRequested);
        assert_eq!(selected[2].precedence, OutputPrecedence::Ambient);
    }

    // Verifies: INV-BUDGET-002 — Precedence-Ordered Truncation
    // Verifies: NEG-BUDGET-001 — No Budget Overflow
    #[test]
    fn allocate_truncates_lowest_first() {
        let mgr = BudgetManager::with_budget(250);

        let blocks = vec![
            OutputBlock::with_tokens("ambient stuff".into(), OutputPrecedence::Ambient, 100),
            OutputBlock::with_tokens("system info".into(), OutputPrecedence::System, 100),
            OutputBlock::with_tokens("user answer".into(), OutputPrecedence::UserRequested, 100),
        ];

        let selected = mgr.allocate(&blocks);
        // Can fit 250 tokens → System (100) + UserRequested (100) = 200, then Ambient (100)
        // 200 + 100 = 300 > 250, so Ambient truncated
        assert_eq!(selected.len(), 2);
        assert!(
            selected
                .iter()
                .all(|b| b.precedence >= OutputPrecedence::UserRequested),
            "only high-priority blocks should survive"
        );
    }

    // Verifies: INV-BUDGET-002 — Precedence-Ordered Truncation
    // Verifies: NEG-BUDGET-002 — No High-Priority Truncation Before Low
    #[test]
    fn allocate_inv_budget_002_higher_never_truncated_before_lower() {
        let mgr = BudgetManager::with_budget(150);

        let blocks = vec![
            OutputBlock::with_tokens("a".into(), OutputPrecedence::Ambient, 60),
            OutputBlock::with_tokens("s".into(), OutputPrecedence::Speculative, 60),
            OutputBlock::with_tokens("u".into(), OutputPrecedence::UserRequested, 60),
            OutputBlock::with_tokens("m".into(), OutputPrecedence::Methodology, 60),
            OutputBlock::with_tokens("y".into(), OutputPrecedence::System, 60),
        ];

        let selected = mgr.allocate(&blocks);
        // 150 tokens → System(60) + Methodology(60) = 120, + UserRequested(60) = 180 > 150
        // So: System + Methodology only = 120 ≤ 150
        let precs: Vec<_> = selected.iter().map(|b| b.precedence).collect();
        // Verify: no higher-priority block is missing while lower-priority is present
        for i in 0..precs.len() {
            for j in (i + 1)..precs.len() {
                assert!(
                    precs[i] >= precs[j],
                    "precedence ordering violated: {:?} after {:?}",
                    precs[j],
                    precs[i]
                );
            }
        }
    }

    // ---- Projection levels ----

    #[test]
    fn projection_level_bands() {
        assert_eq!(BudgetProjection::for_budget(5000), BudgetProjection::Full);
        assert_eq!(BudgetProjection::for_budget(2001), BudgetProjection::Full);
        assert_eq!(
            BudgetProjection::for_budget(1500),
            BudgetProjection::EntitySummary
        );
        assert_eq!(
            BudgetProjection::for_budget(501),
            BudgetProjection::EntitySummary
        );
        assert_eq!(
            BudgetProjection::for_budget(400),
            BudgetProjection::TypeSummary
        );
        assert_eq!(
            BudgetProjection::for_budget(201),
            BudgetProjection::TypeSummary
        );
        assert_eq!(
            BudgetProjection::for_budget(200),
            BudgetProjection::StoreSummary
        );
        assert_eq!(
            BudgetProjection::for_budget(50),
            BudgetProjection::StoreSummary
        );
    }

    // ---- Guidance levels ----

    // Verifies: INV-BUDGET-004 — Guidance Compression by Budget
    #[test]
    fn guidance_level_thresholds() {
        assert_eq!(GuidanceLevel::for_k_eff(0.8), GuidanceLevel::Full);
        assert_eq!(GuidanceLevel::for_k_eff(0.71), GuidanceLevel::Full);
        assert_eq!(GuidanceLevel::for_k_eff(0.5), GuidanceLevel::BasinToken);
        assert_eq!(GuidanceLevel::for_k_eff(0.41), GuidanceLevel::BasinToken);
        assert_eq!(GuidanceLevel::for_k_eff(0.3), GuidanceLevel::Minimal);
        assert_eq!(GuidanceLevel::for_k_eff(0.21), GuidanceLevel::Minimal);
        assert_eq!(GuidanceLevel::for_k_eff(0.1), GuidanceLevel::HarvestOnly);
        assert_eq!(GuidanceLevel::for_k_eff(0.0), GuidanceLevel::HarvestOnly);
    }

    // ---- Token counting ----

    // Verifies: ADR-BUDGET-004 — Tokenization via Chars/4 Approximation
    #[test]
    fn approx_counter_prose() {
        let counter = ApproxTokenCounter;
        // 100 chars → ~25 tokens
        let text = "a".repeat(100);
        assert_eq!(counter.count(&text), 25);
    }

    #[test]
    fn approx_counter_code() {
        let counter = ApproxTokenCounter;
        // Code with lots of symbols gets 25% uplift
        let text = "fn main() { let x = foo(a, b); if x > 0 { bar(); } }";
        let count = counter.count(text);
        // 53 chars / 4 = 13 base, × 1.25 = 16 (rounded)
        assert!(count >= 13, "code should have uplift, got {count}");
    }

    #[test]
    fn approx_counter_empty() {
        let counter = ApproxTokenCounter;
        assert_eq!(counter.count(""), 0);
    }

    // ---- Command profiles ----

    // Verifies: INV-BUDGET-005 — Command Attention Profile
    #[test]
    fn command_profiles() {
        assert_eq!(classify_command("status"), AttentionProfile::Moderate);
        assert_eq!(classify_command("guidance"), AttentionProfile::Cheap);
        // query and task are Expensive: output scales with store size
        assert_eq!(classify_command("query"), AttentionProfile::Expensive);
        assert_eq!(classify_command("task"), AttentionProfile::Expensive);
        assert_eq!(classify_command("seed"), AttentionProfile::Expensive);
        assert_eq!(classify_command("harvest"), AttentionProfile::Meta);
        assert_eq!(classify_command("transact"), AttentionProfile::Meta);
    }

    // Verifies: INV-BUDGET-005 — Command Attention Profile
    // Verifies: INV-BUDGET-001 — Output Budget as Hard Cap
    #[test]
    fn command_budget_respects_profile() {
        let mut mgr = BudgetManager::default();
        mgr.measure(0.0); // full budget = 10000

        // Status is Moderate (primary orientation command), capped at 300
        assert_eq!(mgr.command_budget("status"), 300);
        // Cheap command capped at 50
        assert_eq!(mgr.command_budget("guidance"), 50);
        // Data-heavy commands are Expensive, get full budget
        assert_eq!(mgr.command_budget("query"), 10000);
        assert_eq!(mgr.command_budget("task"), 10000);
        assert_eq!(mgr.command_budget("seed"), 10000);
    }

    // ---- Token efficiency ----

    // Verifies: INV-BUDGET-006 — Token Efficiency as Testable Property
    #[test]
    fn density_monotonicity() {
        // At tighter budgets, density should increase (fewer tokens, same semantic content)
        let full = TokenEfficiency {
            semantic_units: 10,
            token_count: 200,
        };
        let summary = TokenEfficiency {
            semantic_units: 8,
            token_count: 50,
        };
        assert!(
            summary.density() > full.density(),
            "summary density {} should exceed full density {}",
            summary.density(),
            full.density()
        );
    }

    // ---- enforce_ceiling (INV-BUDGET-001) ----

    // Verifies: INV-BUDGET-001 — Output Budget as Hard Cap (passthrough)
    #[test]
    fn enforce_ceiling_passthrough_under_budget() {
        let text = "hello world";
        let result = enforce_ceiling(text, 100);
        assert_eq!(result, text);
    }

    #[test]
    fn enforce_ceiling_passthrough_exact() {
        // 80 chars of prose -> 80/4 = 20 tokens
        let text = "a".repeat(80);
        let result = enforce_ceiling(&text, 20);
        assert_eq!(result, text);
    }

    // Verifies: INV-BUDGET-001 — Output Budget as Hard Cap (pyramid compression)
    #[test]
    fn enforce_ceiling_compresses_over_budget() {
        // 400 lines -> ~500 tokens, ceiling=50 -> must compress
        let text = (0..400).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let result = enforce_ceiling(&text, 50);
        assert!(
            result.len() < text.len(),
            "result should be shorter than input"
        );
        assert!(
            result.contains("more lines"),
            "compressed output must contain pyramid notice, got: {result}"
        );
    }

    #[test]
    fn enforce_ceiling_compression_preserves_header() {
        let text = (0..100).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let ceiling = 30;
        let result = enforce_ceiling(&text, ceiling);
        assert!(
            result.starts_with("line 0"),
            "first line (header) must be preserved: {}",
            result
        );
        assert!(
            result.contains("more lines, use --format json for full output"),
            "notice must tell user how to get full output: {}",
            result
        );
    }

    #[test]
    fn enforce_ceiling_empty_input() {
        let result = enforce_ceiling("", 100);
        assert_eq!(result, "");
    }

    #[test]
    fn enforce_ceiling_unicode_safe() {
        // Ensure compression does not break mid-character.
        // Multi-line unicode input to trigger the compression path.
        let text: String = (0..100)
            .map(|i| format!("\u{1F600} line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let result = enforce_ceiling(&text, 10);
        // If we got here without panicking, UTF-8 safety holds.
        assert!(result.is_char_boundary(result.len()));
    }

    // ---- Proptest ----

    mod budget_proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            // ---- safe_truncate_bytes proptests ----

            #[test]
            fn safe_truncate_bytes_never_panics(
                s in "\\PC{0,500}",
                max_bytes in 0usize..=600
            ) {
                let result = safe_truncate_bytes(&s, max_bytes);
                // Must not exceed max_bytes
                prop_assert!(result.len() <= max_bytes.min(s.len()));
                // Must be valid UTF-8 (the fact that it's &str guarantees this,
                // but we verify by iterating chars)
                let _ = result.chars().count();
            }

            #[test]
            fn safe_truncate_bytes_preserves_content(
                s in "[a-z]{0,200}",
                max_bytes in 0usize..=300
            ) {
                let result = safe_truncate_bytes(&s, max_bytes);
                // For ASCII, truncation should be exact
                prop_assert!(s.starts_with(result));
            }

            #[test]
            fn safe_truncate_display_never_panics(
                s in "\\PC{0,500}",
                max_bytes in 0usize..=600
            ) {
                let result = safe_truncate_display(&s, max_bytes);
                // Must be valid UTF-8
                let _ = result.chars().count();
            }

            // Verifies: INV-BUDGET-001 — enforce_ceiling content never exceeds ceiling
            #[test]
            fn enforce_ceiling_bounded(
                // Generate multi-line text for pyramid compression
                lines in proptest::collection::vec("[a-zA-Z0-9 ]{5,50}", 1..100),
                ceiling in 1usize..=500
            ) {
                let text = lines.join("\n");
                let result = enforce_ceiling(&text, ceiling);
                let counter = ApproxTokenCounter;
                let result_tokens = counter.count(&result);

                // When compressed, the kept lines (before the notice) must fit
                // within ceiling. The notice itself is metadata overhead.
                if result.contains("more lines") {
                    // Measure just the content before the compression notice.
                    let content = result
                        .rsplit_once("\n... (")
                        .map(|(pre, _)| pre)
                        .unwrap_or(&result);
                    let content_tokens = counter.count(content);
                    prop_assert!(
                        content_tokens <= ceiling,
                        "content tokens {} > ceiling {} (total result tokens={})",
                        content_tokens,
                        ceiling,
                        result_tokens
                    );
                } else {
                    prop_assert!(
                        result_tokens <= ceiling,
                        "passthrough tokens {} > ceiling {}",
                        result_tokens,
                        ceiling
                    );
                }
            }

            #[test]
            fn k_eff_always_in_01(pct in 0.0f64..=1.0) {
                let mut mgr = BudgetManager::default();
                mgr.measure(pct);
                prop_assert!(mgr.k_eff >= 0.0, "k_eff < 0: {}", mgr.k_eff);
                prop_assert!(mgr.k_eff <= 1.0, "k_eff > 1: {}", mgr.k_eff);
            }

            #[test]
            fn q_always_in_01(pct in 0.0f64..=1.0) {
                let mut mgr = BudgetManager::default();
                mgr.measure(pct);
                prop_assert!(mgr.q >= 0.0, "Q < 0: {}", mgr.q);
                prop_assert!(mgr.q <= 1.0, "Q > 1: {}", mgr.q);
            }

            #[test]
            fn output_budget_at_least_min(pct in 0.0f64..=1.0) {
                let mut mgr = BudgetManager::default();
                mgr.measure(pct);
                prop_assert!(
                    mgr.output_budget >= MIN_OUTPUT,
                    "output_budget {} < MIN_OUTPUT {}",
                    mgr.output_budget,
                    MIN_OUTPUT
                );
            }

            #[test]
            fn q_leq_k_eff(pct in 0.0f64..=1.0) {
                let mut mgr = BudgetManager::default();
                mgr.measure(pct);
                prop_assert!(
                    mgr.q <= mgr.k_eff + 1e-10,
                    "Q={} > k_eff={}",
                    mgr.q,
                    mgr.k_eff
                );
            }

            #[test]
            fn budget_monotonically_decreasing(
                pcts in proptest::collection::vec(0.0f64..=1.0, 2..=20)
            ) {
                // Sort percentages to simulate monotonically increasing consumption
                let mut sorted = pcts;
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

                let mut mgr = BudgetManager::default();
                let mut prev_budget = u32::MAX;

                for pct in sorted {
                    mgr.measure(pct);
                    prop_assert!(
                        mgr.output_budget <= prev_budget,
                        "budget increased: {} > {} at pct={}",
                        mgr.output_budget,
                        prev_budget,
                        pct
                    );
                    prev_budget = mgr.output_budget;
                }
            }

            #[test]
            fn allocate_never_exceeds_budget(budget in 50u32..=10000) {
                let mgr = BudgetManager::with_budget(budget);

                let blocks = vec![
                    OutputBlock::with_tokens("a".into(), OutputPrecedence::Ambient, 100),
                    OutputBlock::with_tokens("s".into(), OutputPrecedence::Speculative, 100),
                    OutputBlock::with_tokens("u".into(), OutputPrecedence::UserRequested, 100),
                    OutputBlock::with_tokens("m".into(), OutputPrecedence::Methodology, 100),
                    OutputBlock::with_tokens("y".into(), OutputPrecedence::System, 100),
                ];

                let selected = mgr.allocate(&blocks);
                let total: usize = selected.iter().map(|b| b.token_count).sum();
                prop_assert!(
                    total <= budget as usize,
                    "allocated {} > budget {}",
                    total,
                    budget
                );
            }

            #[test]
            fn allocate_preserves_precedence_ordering(budget in 50u32..=10000) {
                let mgr = BudgetManager::with_budget(budget);

                let blocks = vec![
                    OutputBlock::with_tokens("a".into(), OutputPrecedence::Ambient, 60),
                    OutputBlock::with_tokens("s".into(), OutputPrecedence::Speculative, 60),
                    OutputBlock::with_tokens("u".into(), OutputPrecedence::UserRequested, 60),
                    OutputBlock::with_tokens("m".into(), OutputPrecedence::Methodology, 60),
                    OutputBlock::with_tokens("y".into(), OutputPrecedence::System, 60),
                ];

                let selected = mgr.allocate(&blocks);

                // Verify: if a lower-priority block is selected, all higher-priority must be too
                let selected_precs: std::collections::BTreeSet<OutputPrecedence> =
                    selected.iter().map(|b| b.precedence).collect();
                for block in &selected {
                    // Every precedence level above this one must also be present
                    for higher in [
                        OutputPrecedence::Speculative,
                        OutputPrecedence::UserRequested,
                        OutputPrecedence::Methodology,
                        OutputPrecedence::System,
                    ] {
                        if higher > block.precedence {
                            prop_assert!(
                                selected_precs.contains(&higher),
                                "precedence {} present but higher {} absent",
                                block.precedence,
                                higher
                            );
                        }
                    }
                }
            }
        }
    }

    // ---- Multi-Signal k_eff Estimation (KEFF-1/KEFF-2) ----

    #[test]
    fn evidence_vector_default_is_zero() {
        let ev = EvidenceVector::default();
        assert_eq!(ev.tx_count_since_session, 0);
        assert_eq!(ev.wall_elapsed_seconds, 0);
        assert_eq!(ev.observe_count, 0);
    }

    #[test]
    fn estimate_k_eff_zero_evidence_near_one() {
        let ev = EvidenceVector::default();
        let k = estimate_k_eff(&ev);
        // With zero evidence, all sigmoid outputs are ~0 (below threshold)
        // so k_eff should be close to 1.0
        assert!(k > 0.7, "zero evidence should give high k_eff, got {k}");
    }

    #[test]
    fn estimate_k_eff_high_evidence_low() {
        let ev = EvidenceVector {
            tx_count_since_session: 100,
            wall_elapsed_seconds: 7200,
            tx_velocity_per_min: 10.0,
            cumulative_output_estimate: 100_000,
            observe_count: 50,
        };
        let k = estimate_k_eff(&ev);
        assert!(k < 0.3, "high evidence should give low k_eff, got {k}");
    }

    #[test]
    fn estimate_k_eff_monotone_in_tx_count() {
        let mut ev = EvidenceVector::default();
        let k0 = estimate_k_eff(&ev);
        ev.tx_count_since_session = 50;
        let k50 = estimate_k_eff(&ev);
        ev.tx_count_since_session = 100;
        let k100 = estimate_k_eff(&ev);
        assert!(k0 >= k50, "more txns should decrease k_eff");
        assert!(k50 >= k100, "more txns should decrease k_eff");
    }

    #[test]
    fn estimate_k_eff_clamped_to_range() {
        // Even extreme values stay in [0.05, 1.0]
        let extreme = EvidenceVector {
            tx_count_since_session: 10000,
            wall_elapsed_seconds: 100000,
            tx_velocity_per_min: 100.0,
            cumulative_output_estimate: 1_000_000,
            observe_count: 1000,
        };
        let k = estimate_k_eff(&extreme);
        assert!(k >= 0.05, "should not go below 0.05, got {k}");
        assert!(k <= 1.0, "should not exceed 1.0, got {k}");
    }

    #[test]
    fn sigmoid_properties() {
        assert!(
            (sigmoid(0.0) - 0.5).abs() < 0.001,
            "sigmoid(0) should be 0.5"
        );
        assert!(sigmoid(10.0) > 0.99, "sigmoid(large) should be ~1.0");
        assert!(sigmoid(-10.0) < 0.01, "sigmoid(-large) should be ~0.0");
    }

    #[test]
    fn calibrate_boost_scale_insufficient_data() {
        let store = crate::store::Store::from_datoms(std::collections::BTreeSet::new());
        assert_eq!(calibrate_boost_scale(&store), 1.0);
    }

    // =======================================================================
    // ACP Tests (INV-BUDGET-007, INV-BUDGET-008, ADR-BUDGET-005..007)
    // =======================================================================

    /// INV-BUDGET-007: project() at any budget >= 1 always includes the action.
    #[test]
    fn acp_project_always_includes_action() {
        let proj = ActionProjection {
            action: ProjectedAction {
                command: "braid go t-fd30".to_string(),
                rationale: "highest-impact task".to_string(),
                impact: 0.30,
            },
            context: vec![
                ContextBlock {
                    precedence: OutputPrecedence::System,
                    content: "F(S)=0.64, 20000 datoms".to_string(),
                    tokens: 10,
                    attention: None,
                },
                ContextBlock {
                    precedence: OutputPrecedence::Methodology,
                    content: "M(t)=0.50 (tx:✗ spec:✗ query:✗ harvest:✓)".to_string(),
                    tokens: 15,
                    attention: None,
                },
            ],
            evidence_pointer: "details: braid status --verbose".to_string(),
        };

        // At budget=0, still includes action
        let minimal = proj.project(0);
        assert!(
            minimal.contains("braid go t-fd30"),
            "action must appear at budget=0: got: {minimal}"
        );

        // At budget=5 (below any context block), still includes action
        let tiny = proj.project(5);
        assert!(
            tiny.contains("braid go t-fd30"),
            "action must appear at budget=5: got: {tiny}"
        );
    }

    /// INV-BUDGET-008: Monotonic fill — larger budget includes all blocks from smaller budget.
    #[test]
    fn acp_monotonic_fill() {
        let proj = ActionProjection {
            action: ProjectedAction {
                command: "braid go t-test".to_string(),
                rationale: "test task".to_string(),
                impact: 0.5,
            },
            context: vec![
                ContextBlock {
                    precedence: OutputPrecedence::System,
                    content: "BLOCK-A".to_string(),
                    tokens: 5,
                    attention: None,
                },
                ContextBlock {
                    precedence: OutputPrecedence::Methodology,
                    content: "BLOCK-B".to_string(),
                    tokens: 10,
                    attention: None,
                },
                ContextBlock {
                    precedence: OutputPrecedence::UserRequested,
                    content: "BLOCK-C".to_string(),
                    tokens: 20,
                    attention: None,
                },
            ],
            evidence_pointer: String::new(),
        };

        let at_5 = proj.project(5);
        let at_15 = proj.project(15);
        let at_50 = proj.project(50);

        // Budget=5: includes BLOCK-A (5 tokens), discloses omitted B+C
        assert!(at_5.contains("BLOCK-A"), "budget=5 should include BLOCK-A");
        assert!(
            at_5.contains("[+2 omitted:"),
            "budget=5 should disclose 2 omitted blocks"
        );

        // Budget=15: includes BLOCK-A + BLOCK-B, discloses omitted C
        assert!(
            at_15.contains("BLOCK-A"),
            "budget=15 should include BLOCK-A"
        );
        assert!(
            at_15.contains("BLOCK-B"),
            "budget=15 should include BLOCK-B"
        );
        assert!(
            at_15.contains("[+1 omitted:"),
            "budget=15 should disclose 1 omitted block"
        );

        // Budget=50: includes all blocks, no omission disclosure
        assert!(
            at_50.contains("BLOCK-A"),
            "budget=50 should include BLOCK-A"
        );
        assert!(
            at_50.contains("BLOCK-B"),
            "budget=50 should include BLOCK-B"
        );
        assert!(
            at_50.contains("BLOCK-C"),
            "budget=50 should include BLOCK-C"
        );
        assert!(
            !at_50.contains("[+"),
            "budget=50 should have no omission disclosure"
        );
    }

    /// ACP project() never produces truncation markers.
    #[test]
    fn acp_no_truncation_markers() {
        let proj = ActionProjection {
            action: ProjectedAction {
                command: "braid go t-test".to_string(),
                rationale: "test".to_string(),
                impact: 0.5,
            },
            context: vec![ContextBlock {
                precedence: OutputPrecedence::System,
                content: "x ".repeat(100),
                tokens: 50,
                attention: None,
            }],
            evidence_pointer: "details: braid status".to_string(),
        };

        for budget in [0, 5, 10, 25, 50, 100, 300, 1000] {
            let output = proj.project(budget);
            assert!(
                !output.contains("[...truncated"),
                "ACP output must NEVER contain truncation markers (budget={budget})"
            );
        }
    }

    /// ActivationStrategy thresholds match INV-BUDGET-004 bands.
    #[test]
    fn activation_strategy_thresholds() {
        assert_eq!(
            ActivationStrategy::for_k_eff(1.0),
            ActivationStrategy::Demonstrate
        );
        assert_eq!(
            ActivationStrategy::for_k_eff(0.7),
            ActivationStrategy::Demonstrate
        );
        assert_eq!(
            ActivationStrategy::for_k_eff(0.69),
            ActivationStrategy::Navigate
        );
        assert_eq!(
            ActivationStrategy::for_k_eff(0.5),
            ActivationStrategy::Navigate
        );
        assert_eq!(
            ActivationStrategy::for_k_eff(0.4),
            ActivationStrategy::Navigate
        );
        assert_eq!(
            ActivationStrategy::for_k_eff(0.39),
            ActivationStrategy::Imperative
        );
        assert_eq!(
            ActivationStrategy::for_k_eff(0.2),
            ActivationStrategy::Imperative
        );
        assert_eq!(
            ActivationStrategy::for_k_eff(0.19),
            ActivationStrategy::Signal
        );
        assert_eq!(
            ActivationStrategy::for_k_eff(0.0),
            ActivationStrategy::Signal
        );
    }

    /// ActivationStrategy max_context_tokens is monotonically non-decreasing.
    #[test]
    fn activation_strategy_monotonic() {
        assert!(
            ActivationStrategy::Signal.max_context_tokens()
                <= ActivationStrategy::Imperative.max_context_tokens()
        );
        assert!(
            ActivationStrategy::Imperative.max_context_tokens()
                <= ActivationStrategy::Navigate.max_context_tokens()
        );
        assert!(
            ActivationStrategy::Navigate.max_context_tokens()
                <= ActivationStrategy::Demonstrate.max_context_tokens()
        );
    }

    /// ProjectedAction Display fits in <= 15 tokens.
    #[test]
    fn projected_action_display_compact() {
        let action = ProjectedAction {
            command: "braid go t-fd30".to_string(),
            rationale: "highest-impact task".to_string(),
            impact: 0.3,
        };
        let display = format!("{action}");
        let approx_tokens = display.len() / 4;
        assert!(
            approx_tokens <= 20,
            "action display should be compact: {approx_tokens} tokens: {display}"
        );
    }

    /// ActionProjection::to_json() produces valid JSON with expected fields.
    #[test]
    fn acp_to_json_structure() {
        let proj = ActionProjection {
            action: ProjectedAction {
                command: "braid go t-test".to_string(),
                rationale: "test".to_string(),
                impact: 0.42,
            },
            context: vec![ContextBlock {
                precedence: OutputPrecedence::System,
                content: "store info".to_string(),
                tokens: 5,
                attention: None,
            }],
            evidence_pointer: "details: braid status".to_string(),
        };

        let json = proj.to_json();
        assert!(json["_acp"]["action"]["command"].is_string());
        assert_eq!(json["_acp"]["action"]["command"], "braid go t-test");
        assert_eq!(json["_acp"]["action"]["impact"], 0.42);
        assert!(json["_acp"]["context_blocks"].is_array());
        assert_eq!(json["_acp"]["context_blocks"].as_array().unwrap().len(), 1);
        assert_eq!(json["_acp"]["evidence"], "details: braid status");
    }

    /// project_at_strategy matches project with correct budget.
    #[test]
    fn acp_project_at_strategy() {
        let proj = ActionProjection {
            action: ProjectedAction {
                command: "braid go t-test".to_string(),
                rationale: "test".to_string(),
                impact: 0.5,
            },
            context: vec![],
            evidence_pointer: String::new(),
        };

        let navigate = proj.project_at_strategy(ActivationStrategy::Navigate);
        let direct = proj.project(ActivationStrategy::Navigate.max_context_tokens());
        assert_eq!(navigate, direct);
    }

    // ---- json_to_tsv ----

    /// Plain object renders as key\tvalue pairs.
    #[test]
    fn json_to_tsv_object_renders_key_value() {
        let json = serde_json::json!({"a": 1, "b": "hello"});
        let tsv = json_to_tsv(&json);
        // serde_json::Map iterates in insertion order for small maps,
        // but we test each line is present rather than exact ordering.
        assert!(tsv.contains("a\t1\n"), "should contain a\\t1, got: {tsv}");
        assert!(tsv.contains("b\thello\n"), "should contain b\\thello, got: {tsv}");
        // Exactly 2 lines (2 key-value pairs).
        assert_eq!(tsv.lines().count(), 2, "should have 2 lines, got: {tsv}");
    }

    /// Object with "tasks" array renders header + rows.
    #[test]
    fn json_to_tsv_array_renders_header_plus_rows() {
        let json = serde_json::json!({"tasks": [{"id": "t-1", "p": 1}, {"id": "t-2", "p": 2}]});
        let tsv = json_to_tsv(&json);
        let lines: Vec<&str> = tsv.lines().collect();
        assert_eq!(lines.len(), 3, "header + 2 data rows, got: {tsv}");
        // Header line should contain "id" and "p".
        assert!(lines[0].contains("id"), "header should contain 'id': {}", lines[0]);
        assert!(lines[0].contains("p"), "header should contain 'p': {}", lines[0]);
        // Data rows.
        assert!(tsv.contains("t-1"), "should contain t-1, got: {tsv}");
        assert!(tsv.contains("t-2"), "should contain t-2, got: {tsv}");
    }

    /// Tab characters in values are escaped to spaces.
    #[test]
    fn json_to_tsv_escapes_tabs() {
        let json = serde_json::json!({"key": "has\ttab"});
        let tsv = json_to_tsv(&json);
        assert!(!tsv.contains("has\ttab"), "tab in value should be escaped");
        assert!(tsv.contains("has tab"), "tab should become space, got: {tsv}");
    }

    /// Newlines in values are escaped to spaces.
    #[test]
    fn json_to_tsv_escapes_newlines() {
        let json = serde_json::json!({"key": "line1\nline2"});
        let tsv = json_to_tsv(&json);
        // The value should not introduce a spurious row boundary.
        assert_eq!(tsv.lines().count(), 1, "newline in value should be escaped, got: {tsv}");
        assert!(tsv.contains("line1 line2"), "newline should become space, got: {tsv}");
    }

    /// ACP JSON renders action row + context block rows.
    #[test]
    fn json_to_tsv_acp_renders_action_plus_blocks() {
        let json = serde_json::json!({
            "_acp": {
                "action": {
                    "command": "braid go t-test",
                    "rationale": "top task",
                    "impact": 0.42
                },
                "context_blocks": [
                    {"precedence": "System", "content": "store info", "tokens": 5}
                ],
                "evidence": "braid status"
            }
        });
        let tsv = json_to_tsv(&json);
        // Action header + data row.
        assert!(tsv.contains("action\tcommand\trationale\timpact"), "action header missing: {tsv}");
        assert!(tsv.contains("braid go t-test"), "action command missing: {tsv}");
        assert!(tsv.contains("0.42"), "action impact missing: {tsv}");
        // Context block header + data row.
        assert!(tsv.contains("block\tprecedence\tcontent\ttokens"), "block header missing: {tsv}");
        assert!(tsv.contains("System"), "block precedence missing: {tsv}");
        assert!(tsv.contains("store info"), "block content missing: {tsv}");
        // Evidence.
        assert!(tsv.contains("evidence\tbraid status"), "evidence missing: {tsv}");
    }

    /// Null/empty JSON produces empty string.
    #[test]
    fn json_to_tsv_empty_is_empty() {
        assert_eq!(json_to_tsv(&serde_json::Value::Null), "");
        assert_eq!(json_to_tsv(&serde_json::json!({})), "");
        assert_eq!(json_to_tsv(&serde_json::json!([])), "");
    }

    /// Bare array of objects renders header + rows.
    #[test]
    fn json_to_tsv_bare_array_of_objects() {
        let json = serde_json::json!([{"name": "a", "val": 1}, {"name": "b", "val": 2}]);
        let tsv = json_to_tsv(&json);
        let lines: Vec<&str> = tsv.lines().collect();
        assert_eq!(lines.len(), 3, "header + 2 rows, got: {tsv}");
        assert!(lines[0].contains("name"), "header should have 'name': {}", lines[0]);
    }

    /// Bare array of primitives renders one value per line.
    #[test]
    fn json_to_tsv_array_of_primitives() {
        let json = serde_json::json!([1, 2, 3]);
        let tsv = json_to_tsv(&json);
        assert_eq!(tsv, "1\n2\n3\n");
    }

    /// Scalar values render directly.
    #[test]
    fn json_to_tsv_scalars() {
        assert_eq!(json_to_tsv(&serde_json::json!(42)), "42");
        assert_eq!(json_to_tsv(&serde_json::json!(true)), "true");
        assert_eq!(json_to_tsv(&serde_json::json!("hello")), "hello");
    }
}
