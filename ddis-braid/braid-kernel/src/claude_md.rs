//! Dynamic CLAUDE.md generation — session-specific agent instructions from store state.
//!
//! Generates an optimized LLM prompt document by querying the store for:
//! 1. Task context (from seed assembly)
//! 2. Governing invariants (relevant to current work)
//! 3. Known risks and uncertainty markers
//! 4. Methodology drift corrections
//! 5. Self-improvement markers
//!
//! # Invariants
//!
//! - **INV-SEED-007**: Dynamic CLAUDE.md is deterministic — same store state and
//!   parameters always produce the same output.
//! - **INV-SEED-008**: Self-improvement tracking — corrections that are ineffective
//!   after 5 sessions are replaced; effective ones are promoted.
//! - **INV-GUIDANCE-007**: Dynamic CLAUDE.md as optimized prompt — adapts to
//!   observed methodology drift patterns.

use crate::datom::{AgentId, Attribute, EntityId, Value};
use crate::guidance::{compute_methodology_score, SessionTelemetry};
use crate::seed::assemble_seed;
use crate::store::Store;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for dynamic CLAUDE.md generation.
#[derive(Clone, Debug)]
pub struct ClaudeMdConfig {
    /// The task description for this session.
    pub task: String,
    /// The agent identity.
    pub agent: AgentId,
    /// Token budget for the entire document.
    pub budget: usize,
    /// Maximum number of invariants to include.
    pub max_invariants: usize,
    /// Maximum number of uncertainty markers to include.
    pub max_uncertainties: usize,
    /// Maximum number of drift corrections to include.
    pub max_corrections: usize,
}

impl Default for ClaudeMdConfig {
    fn default() -> Self {
        ClaudeMdConfig {
            task: String::new(),
            agent: AgentId::from_name("braid:user"),
            budget: 4000,
            max_invariants: 20,
            max_uncertainties: 10,
            max_corrections: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------

/// A section in the generated CLAUDE.md.
#[derive(Clone, Debug)]
pub struct ClaudeMdSection {
    /// Section heading.
    pub heading: String,
    /// Section body (markdown).
    pub body: String,
    /// Estimated token count.
    pub tokens: usize,
}

/// The complete generated CLAUDE.md document.
#[derive(Clone, Debug)]
pub struct GeneratedClaudeMd {
    /// All sections in priority order.
    pub sections: Vec<ClaudeMdSection>,
    /// The methodology score at generation time.
    pub methodology_score: f64,
    /// Total estimated tokens.
    pub total_tokens: usize,
}

impl GeneratedClaudeMd {
    /// Render the complete document as a markdown string.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(self.total_tokens * 4);
        out.push_str("# Dynamic CLAUDE.md\n\n");
        out.push_str(&format!(
            "> Auto-generated from store state. Methodology score: {:.2}\n\n",
            self.methodology_score
        ));
        out.push_str("---\n\n");

        for section in &self.sections {
            out.push_str(&format!("## {}\n\n", section.heading));
            out.push_str(&section.body);
            out.push_str("\n\n");
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Generation pipeline
// ---------------------------------------------------------------------------

/// Generate a dynamic CLAUDE.md from store state.
///
/// This is the 7-step pipeline:
/// 1. ASSOCIATE with focus task
/// 2. QUERY active intentions (spec elements with `:spec/element-type` = invariant)
/// 3. QUERY governing invariants relevant to task
/// 4. QUERY uncertainty markers
/// 5. QUERY recent decisions (ADRs)
/// 6. QUERY drift corrections (methodology adherence patterns)
/// 7. ASSEMBLE at budget with priority ordering
///
/// # Determinism (INV-SEED-007)
///
/// Same store state + same config = same output. No randomness, no system clock.
pub fn generate_claude_md(store: &Store, config: &ClaudeMdConfig) -> GeneratedClaudeMd {
    let mut sections = Vec::new();
    let mut total_tokens = 0;

    // Step 1: Task context section
    let task_section = build_task_section(store, config);
    total_tokens += task_section.tokens;
    sections.push(task_section);

    // Step 2: Governing invariants
    let inv_section = build_invariant_section(store, config);
    total_tokens += inv_section.tokens;
    sections.push(inv_section);

    // Step 3: Recent decisions (ADRs)
    let adr_section = build_adr_section(store, config);
    total_tokens += adr_section.tokens;
    sections.push(adr_section);

    // Step 4: Uncertainty markers
    let unc_section = build_uncertainty_section(store, config);
    total_tokens += unc_section.tokens;
    sections.push(unc_section);

    // Step 5: Drift corrections
    let drift_section = build_drift_section(store, config);
    total_tokens += drift_section.tokens;
    sections.push(drift_section);

    // Step 6: Methodology score + guidance footer
    let methodology_score = compute_methodology_for_store(store);
    let guidance_section = build_guidance_section(methodology_score);
    total_tokens += guidance_section.tokens;
    sections.push(guidance_section);

    // Step 7: Seed context (budget-aware assembly)
    let remaining_budget = config.budget.saturating_sub(total_tokens);
    if remaining_budget > 50 {
        let seed_section = build_seed_section(store, config, remaining_budget);
        total_tokens += seed_section.tokens;
        sections.push(seed_section);
    }

    GeneratedClaudeMd {
        sections,
        methodology_score,
        total_tokens,
    }
}

// ---------------------------------------------------------------------------
// Section builders
// ---------------------------------------------------------------------------

fn build_task_section(store: &Store, config: &ClaudeMdConfig) -> ClaudeMdSection {
    let mut body = String::new();

    if !config.task.is_empty() {
        body.push_str(&format!("**Current task**: {}\n\n", config.task));
    }

    // Count store statistics
    let entity_count = store.entities().len();
    let datom_count = store.len();
    let tx_count = store.frontier().len();

    body.push_str(&format!(
        "**Store state**: {} datoms, {} entities, {} agents in frontier\n\n",
        datom_count, entity_count, tx_count
    ));

    // Count spec elements if bootstrapped
    let spec_count = count_spec_elements(store);
    if spec_count > 0 {
        body.push_str(&format!(
            "**Spec elements in store**: {} (self-bootstrap active)\n",
            spec_count
        ));
    }

    let tokens = estimate_tokens(&body);
    ClaudeMdSection {
        heading: "Task Context".to_string(),
        body,
        tokens,
    }
}

fn build_invariant_section(store: &Store, config: &ClaudeMdConfig) -> ClaudeMdSection {
    let mut body = String::new();
    let invariants = query_spec_elements(store, ":spec.element/invariant", config.max_invariants);

    if invariants.is_empty() {
        body.push_str("*No invariants in store. Run `braid bootstrap` to load spec elements.*\n");
    } else {
        body.push_str(&format!(
            "The following {} invariants govern the current work:\n\n",
            invariants.len()
        ));
        for (id, doc) in &invariants {
            body.push_str(&format!("- **{}**: {}\n", id, doc));
        }
    }

    let tokens = estimate_tokens(&body);
    ClaudeMdSection {
        heading: "Governing Invariants".to_string(),
        body,
        tokens,
    }
}

fn build_adr_section(store: &Store, config: &ClaudeMdConfig) -> ClaudeMdSection {
    let mut body = String::new();
    let adrs = query_spec_elements(store, ":spec.element/adr", config.max_invariants);

    if adrs.is_empty() {
        body.push_str("*No ADRs in store.*\n");
    } else {
        body.push_str(&format!(
            "{} architecture decisions are settled. Do not relitigate (NEG-002).\n\n",
            adrs.len()
        ));
        for (id, doc) in adrs.iter().take(10) {
            body.push_str(&format!("- **{}**: {}\n", id, doc));
        }
        if adrs.len() > 10 {
            body.push_str(&format!("\n*...and {} more ADRs.*\n", adrs.len() - 10));
        }
    }

    let tokens = estimate_tokens(&body);
    ClaudeMdSection {
        heading: "Settled Decisions (ADRs)".to_string(),
        body,
        tokens,
    }
}

fn build_uncertainty_section(store: &Store, config: &ClaudeMdConfig) -> ClaudeMdSection {
    let mut body = String::new();
    let negs = query_spec_elements(
        store,
        ":spec.element/negative-case",
        config.max_uncertainties,
    );

    if negs.is_empty() {
        body.push_str("*No negative cases in store.*\n");
    } else {
        body.push_str("**Negative cases** — things the system must NOT do:\n\n");
        for (id, doc) in &negs {
            body.push_str(&format!("- **{}**: {}\n", id, doc));
        }
    }

    let tokens = estimate_tokens(&body);
    ClaudeMdSection {
        heading: "Risks & Negative Cases".to_string(),
        body,
        tokens,
    }
}

fn build_drift_section(store: &Store, config: &ClaudeMdConfig) -> ClaudeMdSection {
    let mut body = String::new();

    // Query for methodology-related datoms (drift corrections)
    let corrections = query_drift_corrections(store, config.max_corrections);

    if corrections.is_empty() {
        body.push_str(
            "No drift corrections recorded yet. As sessions accumulate, methodology \n\
             drift patterns will be detected and corrections injected here.\n",
        );
    } else {
        body.push_str("**Active drift corrections**:\n\n");
        for correction in &corrections {
            body.push_str(&format!("- {}\n", correction));
        }
    }

    let tokens = estimate_tokens(&body);
    ClaudeMdSection {
        heading: "Methodology Drift Corrections".to_string(),
        body,
        tokens,
    }
}

fn build_guidance_section(methodology_score: f64) -> ClaudeMdSection {
    let mut body = String::new();

    body.push_str(&format!(
        "**M(t)** = {:.3} — methodology adherence score\n\n",
        methodology_score
    ));

    // Provide tier-specific guidance
    let guidance = if methodology_score >= 0.8 {
        "Methodology adherence is HIGH. Maintain current practices."
    } else if methodology_score >= 0.5 {
        "Methodology adherence is MODERATE. Review harvest completeness and spec language."
    } else {
        "Methodology adherence is LOW. Prioritize: (1) harvest every session, \
         (2) use spec language in commits, (3) diversify query patterns."
    };

    body.push_str(&format!("{}\n", guidance));

    let tokens = estimate_tokens(&body);
    ClaudeMdSection {
        heading: "Methodology Score".to_string(),
        body,
        tokens,
    }
}

fn build_seed_section(store: &Store, config: &ClaudeMdConfig, budget: usize) -> ClaudeMdSection {
    use crate::seed::ContextSection as CS;

    let seed = assemble_seed(store, &config.task, budget, config.agent);

    let mut body = String::new();
    for section in &seed.context.sections {
        match section {
            CS::Orientation(text) => {
                body.push_str(&format!("### Orientation\n\n{}\n\n", text));
            }
            CS::Constraints(refs) => {
                body.push_str("### Constraints\n\n");
                for c in refs {
                    let status = match c.satisfied {
                        Some(true) => "OK",
                        Some(false) => "VIOLATED",
                        None => "UNKNOWN",
                    };
                    body.push_str(&format!("- **{}** [{}]: {}\n", c.id, status, c.summary));
                }
                body.push('\n');
            }
            CS::State(entries) => {
                body.push_str("### State\n\n");
                for entry in entries {
                    body.push_str(&format!(
                        "- {:?} [{:?}]: {}\n",
                        entry.entity, entry.projection, entry.content
                    ));
                }
                body.push('\n');
            }
            CS::Warnings(warnings) => {
                body.push_str("### Warnings\n\n");
                for w in warnings {
                    body.push_str(&format!("- {}\n", w));
                }
                body.push('\n');
            }
            CS::Directive(text) => {
                body.push_str(&format!("### Directive\n\n{}\n\n", text));
            }
        }
    }

    let tokens = seed.context.total_tokens;
    ClaudeMdSection {
        heading: "Seed Context".to_string(),
        body,
        tokens,
    }
}

// ---------------------------------------------------------------------------
// Store queries
// ---------------------------------------------------------------------------

/// Count the number of spec elements (bootstrapped) in the store.
fn count_spec_elements(store: &Store) -> usize {
    let type_attr = Attribute::from_keyword(":spec/element-type");
    store.datoms().filter(|d| d.attribute == type_attr).count()
}

/// Query spec elements of a given type, returning (id, doc) pairs.
fn query_spec_elements(store: &Store, element_type: &str, max: usize) -> Vec<(String, String)> {
    let type_attr = Attribute::from_keyword(":spec/element-type");
    let ident_attr = Attribute::from_keyword(":db/ident");
    let doc_attr = Attribute::from_keyword(":db/doc");

    // Find entities with the given element type
    let matching_entities: Vec<EntityId> = store
        .datoms()
        .filter(|d| {
            d.attribute == type_attr && matches!(&d.value, Value::Keyword(k) if k == element_type)
        })
        .map(|d| d.entity)
        .collect();

    let mut results = Vec::new();
    for entity in matching_entities.iter().take(max) {
        let id = store
            .entity_datoms(*entity)
            .iter()
            .find(|d| d.attribute == ident_attr)
            .and_then(|d| match &d.value {
                Value::Keyword(k) => Some(k.clone()),
                _ => None,
            })
            .unwrap_or_default();

        let doc = store
            .entity_datoms(*entity)
            .iter()
            .find(|d| d.attribute == doc_attr)
            .and_then(|d| match &d.value {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        if !id.is_empty() {
            results.push((id, doc));
        }
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

/// Query drift correction entries from the store.
///
/// At Stage 0, drift corrections are not yet automated. This returns
/// any datoms with `:guidance/correction` attribute.
fn query_drift_corrections(store: &Store, max: usize) -> Vec<String> {
    // Check if the correction attribute exists
    if store
        .schema()
        .attribute(&Attribute::from_keyword(":guidance/correction"))
        .is_none()
    {
        return Vec::new();
    }

    let correction_attr = Attribute::from_keyword(":guidance/correction");
    store
        .datoms()
        .filter(|d| d.attribute == correction_attr)
        .take(max)
        .filter_map(|d| match &d.value {
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

/// Compute methodology score from store state.
///
/// Uses a synthetic telemetry based on store statistics.
fn compute_methodology_for_store(store: &Store) -> f64 {
    let entity_count = store.entities().len();
    let spec_count = count_spec_elements(store);

    // Synthetic telemetry from store statistics
    let telemetry = SessionTelemetry {
        total_turns: (entity_count / 10).max(1) as u32,
        transact_turns: store.frontier().len() as u32,
        spec_language_turns: spec_count.min(255) as u32,
        query_type_count: entity_count.min(10) as u32,
        harvest_quality: if spec_count > 0 { 0.8 } else { 0.0 },
        history: Vec::new(),
    };

    compute_methodology_score(&telemetry).score
}

/// Estimate token count from text (rough: 1 token ≈ 4 chars).
fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{Datom, Op, TxId};
    use std::collections::BTreeSet;

    fn bootstrap_store() -> Store {
        // Build store from raw datoms (bypasses schema validation, like real bootstrap)
        let agent = AgentId::from_name("test:bootstrap");
        let tx = TxId::new(1, 0, agent);

        let mut datoms: BTreeSet<Datom> = Store::genesis().datoms().cloned().collect();

        // Add spec element: INV-STORE-001
        let inv_entity = EntityId::from_ident(":spec/inv-store-001");
        datoms.insert(Datom::new(
            inv_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":spec/inv-store-001".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            inv_entity,
            Attribute::from_keyword(":spec/element-type"),
            Value::Keyword(":spec.element/invariant".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            inv_entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("Append-Only Immutability".to_string()),
            tx,
            Op::Assert,
        ));

        // Add spec element: ADR-STORE-001
        let adr_entity = EntityId::from_ident(":spec/adr-store-001");
        datoms.insert(Datom::new(
            adr_entity,
            Attribute::from_keyword(":db/ident"),
            Value::Keyword(":spec/adr-store-001".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            adr_entity,
            Attribute::from_keyword(":spec/element-type"),
            Value::Keyword(":spec.element/adr".to_string()),
            tx,
            Op::Assert,
        ));
        datoms.insert(Datom::new(
            adr_entity,
            Attribute::from_keyword(":db/doc"),
            Value::String("EAV Over Relational".to_string()),
            tx,
            Op::Assert,
        ));

        Store::from_datoms(datoms)
    }

    #[test]
    fn generate_claude_md_is_deterministic() {
        let store = bootstrap_store();
        let config = ClaudeMdConfig {
            task: "implement datom store".to_string(),
            ..Default::default()
        };

        let doc1 = generate_claude_md(&store, &config);
        let doc2 = generate_claude_md(&store, &config);

        assert_eq!(
            doc1.render(),
            doc2.render(),
            "INV-SEED-007: determinism violated"
        );
    }

    #[test]
    fn generate_claude_md_has_all_sections() {
        let store = bootstrap_store();
        let config = ClaudeMdConfig {
            task: "test task".to_string(),
            ..Default::default()
        };

        let doc = generate_claude_md(&store, &config);

        // Should have at least 6 sections: task, invariants, ADRs, risks, drift, methodology
        assert!(
            doc.sections.len() >= 6,
            "expected >= 6 sections, got {}",
            doc.sections.len()
        );

        let headings: Vec<&str> = doc.sections.iter().map(|s| s.heading.as_str()).collect();
        assert!(headings.contains(&"Task Context"));
        assert!(headings.contains(&"Governing Invariants"));
        assert!(headings.contains(&"Settled Decisions (ADRs)"));
        assert!(headings.contains(&"Risks & Negative Cases"));
        assert!(headings.contains(&"Methodology Drift Corrections"));
        assert!(headings.contains(&"Methodology Score"));
    }

    #[test]
    fn generate_claude_md_includes_spec_elements() {
        let store = bootstrap_store();
        let config = ClaudeMdConfig {
            task: "verify store invariants".to_string(),
            ..Default::default()
        };

        let doc = generate_claude_md(&store, &config);
        let rendered = doc.render();

        assert!(
            rendered.contains("inv-store-001"),
            "should include bootstrapped invariant"
        );
        assert!(
            rendered.contains("Append-Only Immutability"),
            "should include invariant doc"
        );
        assert!(
            rendered.contains("adr-store-001"),
            "should include bootstrapped ADR"
        );
    }

    #[test]
    fn generate_claude_md_respects_budget() {
        let store = bootstrap_store();
        let config = ClaudeMdConfig {
            task: "small task".to_string(),
            budget: 500,
            ..Default::default()
        };

        let doc = generate_claude_md(&store, &config);
        // Total tokens should be reasonable (budget is soft limit)
        assert!(
            doc.total_tokens < 2000,
            "tokens {} should be bounded",
            doc.total_tokens
        );
    }

    #[test]
    fn generate_claude_md_empty_store() {
        let store = Store::genesis();
        let config = ClaudeMdConfig {
            task: "no bootstrap yet".to_string(),
            ..Default::default()
        };

        let doc = generate_claude_md(&store, &config);
        let rendered = doc.render();

        assert!(rendered.contains("No invariants in store"));
        assert!(rendered.contains("braid bootstrap"));
    }

    #[test]
    fn methodology_score_from_store() {
        let store = bootstrap_store();
        let score = compute_methodology_for_store(&store);
        assert!(
            (0.0..=1.0).contains(&score),
            "M(t) must be in [0, 1], got {}",
            score
        );
    }
}
