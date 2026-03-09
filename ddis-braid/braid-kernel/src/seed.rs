//! SEED namespace — start-of-session context assembly.
//!
//! Seed is the complement of harvest: where harvest extracts knowledge at session
//! end, seed assembles relevant knowledge at session start. The seed provides a
//! fresh agent with full relevant context, zero irrelevant noise, and stays within
//! the declared attention budget.
//!
//! # Pipeline (INV-SEED-001)
//!
//! ASSOCIATE → QUERY → ASSEMBLE → EMIT
//!
//! # Invariants
//!
//! - **INV-SEED-001**: Seed as store projection — every datum traces to a datom.
//! - **INV-SEED-002**: Budget compliance — output ≤ declared budget.
//! - **INV-SEED-003**: ASSOCIATE boundedness — `|result| ≤ depth × breadth`.
//! - **INV-SEED-004**: Section compression priority (State first, Directive last).
//! - **INV-SEED-006**: Intention anchoring — intentions pinned at π₀ regardless.

use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, Value};
use crate::store::Store;

/// Projection levels for rate-distortion compression (ADR-SEED-002).
///
/// Forms a total order: Full > Summary > TypeLevel > Pointer.
/// Higher compression = less detail = fewer tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectionLevel {
    /// π₃ — single-line reference (entity ID only).
    Pointer,
    /// π₂ — type summary (entity type + count of attributes).
    TypeLevel,
    /// π₁ — entity summary (entity + attribute names, no values).
    Summary,
    /// π₀ — all datoms for the entity.
    Full,
}

/// How the seed was discovered — semantic search or explicit entity list.
#[derive(Clone, Debug)]
pub enum AssociateCue {
    /// Natural language search with bounded traversal.
    Semantic {
        /// Search text.
        text: String,
        /// Maximum traversal depth from matching entities.
        depth: usize,
        /// Maximum neighbors per hop.
        breadth: usize,
    },
    /// Known entity IDs with bounded neighborhood expansion.
    Explicit {
        /// Starting entities.
        seeds: Vec<EntityId>,
        /// Maximum traversal depth.
        depth: usize,
        /// Maximum neighbors per hop.
        breadth: usize,
    },
}

impl AssociateCue {
    /// Upper bound on result size: depth × breadth (INV-SEED-003).
    pub fn max_results(&self) -> usize {
        match self {
            AssociateCue::Semantic { depth, breadth, .. } => depth * breadth,
            AssociateCue::Explicit {
                seeds,
                depth,
                breadth,
            } => seeds.len() + depth * breadth,
        }
    }
}

/// Schema neighborhood discovered by ASSOCIATE — entities, attributes, types.
/// NOT values (those come from ASSEMBLE at chosen projection level).
#[derive(Clone, Debug, Default)]
pub struct SchemaNeighborhood {
    /// Entities discovered by ASSOCIATE.
    pub entities: Vec<EntityId>,
    /// Attributes relevant to discovered entities.
    pub attributes: Vec<Attribute>,
    /// Type keywords for discovered entities.
    pub entity_types: Vec<String>,
}

/// Sections of the assembled seed output (ADR-SEED-004).
///
/// Five-part unified template. Compression order (first-to-compress → last):
/// State → Constraints → Orientation → Warnings → Directive.
#[derive(Clone, Debug)]
pub enum ContextSection {
    /// Project identity, current phase, recent session history.
    Orientation(String),
    /// Relevant invariants, settled ADRs, negative cases.
    Constraints(Vec<ConstraintRef>),
    /// Relevant datoms at chosen projection level.
    State(Vec<StateEntry>),
    /// Drift signals, open questions, uncertainties.
    Warnings(Vec<String>),
    /// Next task, acceptance criteria, active guidance corrections.
    Directive(String),
}

/// A reference to a specification constraint.
#[derive(Clone, Debug)]
pub struct ConstraintRef {
    /// Constraint ID (e.g., "INV-STORE-001").
    pub id: String,
    /// Brief description.
    pub summary: String,
    /// Whether this constraint is currently satisfied.
    pub satisfied: Option<bool>,
}

/// A state entry at a chosen projection level.
#[derive(Clone, Debug)]
pub struct StateEntry {
    /// The entity this entry describes.
    pub entity: EntityId,
    /// Projection level used.
    pub projection: ProjectionLevel,
    /// Token-counted representation.
    pub content: String,
    /// Estimated token count.
    pub tokens: usize,
}

/// The assembled seed context (INV-SEED-002: total_tokens ≤ budget).
#[derive(Clone, Debug)]
pub struct AssembledContext {
    /// Ordered sections of the seed output.
    pub sections: Vec<ContextSection>,
    /// Total estimated token count.
    pub total_tokens: usize,
    /// Remaining budget after assembly.
    pub budget_remaining: usize,
    /// Dominant projection pattern used.
    pub projection_pattern: ProjectionLevel,
}

/// Complete seed output for a session.
#[derive(Clone, Debug)]
pub struct SeedOutput {
    /// The assembled context.
    pub context: AssembledContext,
    /// The agent this seed is for.
    pub agent: AgentId,
    /// The task description used for association.
    pub task: String,
    /// Number of entities discovered by ASSOCIATE.
    pub entities_discovered: usize,
}

// ---------------------------------------------------------------------------
// Core functions
// ---------------------------------------------------------------------------

/// ASSOCIATE: Discover the schema neighborhood relevant to a task.
///
/// Returns entities, attributes, and types — NOT values.
/// INV-SEED-003: `|result.entities| ≤ cue.max_results()`.
pub fn associate(store: &Store, cue: &AssociateCue) -> SchemaNeighborhood {
    let max = cue.max_results();
    let mut neighborhood = SchemaNeighborhood::default();

    match cue {
        AssociateCue::Semantic { text, .. } => {
            // Stage 0: Simple keyword matching against entity idents and doc strings.
            // Full semantic search requires embedding infrastructure (Stage 1+).
            let keywords: Vec<&str> = text.split_whitespace().collect();

            for datom in store.datoms() {
                if datom.op != Op::Assert {
                    continue;
                }
                if neighborhood.entities.len() >= max {
                    break;
                }

                let matches = match &datom.value {
                    Value::String(s) => keywords.iter().any(|kw| s.contains(kw)),
                    Value::Keyword(k) => keywords.iter().any(|kw| k.contains(kw)),
                    _ => false,
                };

                if matches && !neighborhood.entities.contains(&datom.entity) {
                    neighborhood.entities.push(datom.entity);
                    if !neighborhood.attributes.contains(&datom.attribute) {
                        neighborhood.attributes.push(datom.attribute.clone());
                    }
                }
            }
        }
        AssociateCue::Explicit { seeds, .. } => {
            // Start from known entities and expand to neighbors
            for seed in seeds {
                if !neighborhood.entities.contains(seed) {
                    neighborhood.entities.push(*seed);
                }
                // Gather attributes for this entity
                for datom in store.datoms() {
                    if datom.entity == *seed
                        && datom.op == Op::Assert
                        && !neighborhood.attributes.contains(&datom.attribute)
                    {
                        neighborhood.attributes.push(datom.attribute.clone());
                    }
                }
            }
        }
    }

    // Truncate to bound
    neighborhood.entities.truncate(max);
    neighborhood
}

/// Score an entity for relevance to a task (ADR-SEED-002).
///
/// `score(e) = α × relevance + β × significance + γ × recency`
/// where α = 0.5, β = 0.3, γ = 0.2 (defaults).
fn score_entity(
    store: &Store,
    entity: EntityId,
    task_keywords: &[&str],
    max_tx_wall_time: u64,
) -> f64 {
    let datoms: Vec<&Datom> = store
        .datoms()
        .filter(|d| d.entity == entity && d.op == Op::Assert)
        .collect();

    if datoms.is_empty() {
        return 0.0;
    }

    // Relevance: fraction of task keywords that match entity content
    let mut keyword_hits = 0usize;
    for kw in task_keywords {
        for d in &datoms {
            let hit = match &d.value {
                Value::String(s) if s.contains(kw) => true,
                Value::Keyword(k) if k.contains(kw) => true,
                _ => false,
            };
            if hit {
                keyword_hits += 1;
                break;
            }
        }
    }
    let relevance = if task_keywords.is_empty() {
        0.5
    } else {
        keyword_hits as f64 / task_keywords.len() as f64
    };

    // Significance: number of attributes (proxy for information density)
    let unique_attrs = datoms
        .iter()
        .map(|d| d.attribute.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let significance = (unique_attrs as f64 / 10.0).min(1.0);

    // Recency: newest transaction time relative to max
    let newest_tx = datoms.iter().map(|d| d.tx.wall_time()).max().unwrap_or(0);
    let recency = if max_tx_wall_time == 0 {
        0.5
    } else {
        newest_tx as f64 / max_tx_wall_time as f64
    };

    0.5 * relevance + 0.3 * significance + 0.2 * recency
}

/// Estimate token count for text (rough: 1 token ≈ 4 chars).
fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Project an entity at a given projection level.
fn project_entity(store: &Store, entity: EntityId, level: ProjectionLevel) -> StateEntry {
    let datoms: Vec<&Datom> = store
        .datoms()
        .filter(|d| d.entity == entity && d.op == Op::Assert)
        .collect();

    let content = match level {
        ProjectionLevel::Pointer => {
            format!("{:?}", entity)
        }
        ProjectionLevel::TypeLevel => {
            let type_kw = datoms
                .iter()
                .find(|d| d.attribute.as_str() == ":db/ident")
                .map(|d| format!("{:?}", d.value))
                .unwrap_or_else(|| format!("{:?}", entity));
            format!("{} ({} attrs)", type_kw, datoms.len())
        }
        ProjectionLevel::Summary => {
            let attrs: Vec<&str> = datoms.iter().map(|d| d.attribute.as_str()).collect();
            format!("{:?}: [{}]", entity, attrs.join(", "))
        }
        ProjectionLevel::Full => {
            let mut lines = Vec::new();
            for d in &datoms {
                lines.push(format!("  {} = {:?}", d.attribute.as_str(), d.value));
            }
            format!("{:?}:\n{}", entity, lines.join("\n"))
        }
    };

    let tokens = estimate_tokens(&content);
    StateEntry {
        entity,
        projection: level,
        content,
        tokens,
    }
}

/// ASSEMBLE: Build the seed context within a token budget (INV-SEED-002).
///
/// Compression priority (first-to-compress → last):
/// State → Constraints → Orientation → Warnings → Directive.
pub fn assemble(
    store: &Store,
    neighborhood: &SchemaNeighborhood,
    task: &str,
    budget: usize,
) -> AssembledContext {
    let task_keywords: Vec<&str> = task.split_whitespace().collect();
    let max_wall = store.datoms().map(|d| d.tx.wall_time()).max().unwrap_or(0);

    // Score and sort entities by relevance
    let mut scored: Vec<(EntityId, f64)> = neighborhood
        .entities
        .iter()
        .map(|&e| (e, score_entity(store, e, &task_keywords, max_wall)))
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Select projection level based on budget
    let projection = if budget > 2000 {
        ProjectionLevel::Full
    } else if budget > 500 {
        ProjectionLevel::Summary
    } else if budget > 200 {
        ProjectionLevel::TypeLevel
    } else {
        ProjectionLevel::Pointer
    };

    // Build sections
    let directive = ContextSection::Directive(format!("Task: {task}"));
    let directive_tokens = estimate_tokens(task) + 6;

    let orientation =
        ContextSection::Orientation(format!("Braid datom store | {} datoms", store.len()));
    let orientation_tokens = 10;

    let warnings = ContextSection::Warnings(Vec::new());
    let warnings_tokens = 0;

    let constraints = ContextSection::Constraints(Vec::new());
    let constraints_tokens = 0;

    // Allocate remaining budget to state entries
    let overhead = directive_tokens + orientation_tokens + warnings_tokens + constraints_tokens;
    let state_budget = budget.saturating_sub(overhead);

    let mut state_entries = Vec::new();
    let mut state_tokens = 0;
    for (entity, _score) in &scored {
        let entry = project_entity(store, *entity, projection);
        if state_tokens + entry.tokens > state_budget {
            break;
        }
        state_tokens += entry.tokens;
        state_entries.push(entry);
    }

    let state = ContextSection::State(state_entries);

    let total_tokens = overhead + state_tokens;
    let budget_remaining = budget.saturating_sub(total_tokens);

    AssembledContext {
        sections: vec![orientation, constraints, state, warnings, directive],
        total_tokens,
        budget_remaining,
        projection_pattern: projection,
    }
}

/// Top-level seed assembly: ASSOCIATE → ASSEMBLE in one call.
///
/// This is the primary entry point for start-of-session context loading.
pub fn assemble_seed(store: &Store, task: &str, budget: usize, agent: AgentId) -> SeedOutput {
    let cue = AssociateCue::Semantic {
        text: task.to_string(),
        depth: 5,
        breadth: 10,
    };

    let neighborhood = associate(store, &cue);
    let entities_discovered = neighborhood.entities.len();
    let context = assemble(store, &neighborhood, task, budget);

    SeedOutput {
        context,
        agent,
        task: task.to_string(),
        entities_discovered,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::AgentId;

    #[test]
    fn associate_boundedness() {
        let store = Store::genesis();
        let cue = AssociateCue::Semantic {
            text: "db ident".to_string(),
            depth: 2,
            breadth: 3,
        };
        let max = cue.max_results();
        let neighborhood = associate(&store, &cue);
        assert!(
            neighborhood.entities.len() <= max,
            "INV-SEED-003: |result| <= depth * breadth"
        );
    }

    #[test]
    fn associate_explicit_includes_seeds() {
        let store = Store::genesis();
        let seed_entity = EntityId::from_ident(":db/ident");
        let cue = AssociateCue::Explicit {
            seeds: vec![seed_entity],
            depth: 1,
            breadth: 5,
        };
        let neighborhood = associate(&store, &cue);
        assert!(neighborhood.entities.contains(&seed_entity));
    }

    #[test]
    fn assemble_respects_budget() {
        let store = Store::genesis();
        let budget = 500;
        let neighborhood = SchemaNeighborhood {
            entities: store.entities().into_iter().collect(),
            attributes: vec![],
            entity_types: vec![],
        };
        let ctx = assemble(&store, &neighborhood, "test task", budget);
        assert!(
            ctx.total_tokens <= budget,
            "INV-SEED-002: total_tokens ({}) <= budget ({})",
            ctx.total_tokens,
            budget
        );
    }

    #[test]
    fn assemble_selects_projection_by_budget() {
        let store = Store::genesis();
        let neighborhood = SchemaNeighborhood::default();

        let high = assemble(&store, &neighborhood, "task", 3000);
        assert_eq!(high.projection_pattern, ProjectionLevel::Full);

        let mid = assemble(&store, &neighborhood, "task", 1000);
        assert_eq!(mid.projection_pattern, ProjectionLevel::Summary);

        let low = assemble(&store, &neighborhood, "task", 300);
        assert_eq!(low.projection_pattern, ProjectionLevel::TypeLevel);

        let tiny = assemble(&store, &neighborhood, "task", 100);
        assert_eq!(tiny.projection_pattern, ProjectionLevel::Pointer);
    }

    #[test]
    fn assemble_seed_end_to_end() {
        let store = Store::genesis();
        let agent = AgentId::from_name("test-agent");
        let seed = assemble_seed(&store, "datom store schema", 2000, agent);
        assert_eq!(seed.task, "datom store schema");
        assert!(seed.context.total_tokens <= 2000);
        assert_eq!(seed.context.sections.len(), 5);
    }

    #[test]
    fn projection_level_ordering() {
        assert!(ProjectionLevel::Pointer < ProjectionLevel::TypeLevel);
        assert!(ProjectionLevel::TypeLevel < ProjectionLevel::Summary);
        assert!(ProjectionLevel::Summary < ProjectionLevel::Full);
    }

    #[test]
    fn score_entity_nonzero_for_matching() {
        let store = Store::genesis();
        let entity = EntityId::from_ident(":db/ident");
        let score = score_entity(&store, entity, &["ident"], 100);
        assert!(score > 0.0, "matching entity should have positive score");
    }

    // -------------------------------------------------------------------
    // Rate-distortion bound witnesses (proptest)
    //
    // Seed assembly is a lossy compression of the store subject to
    // information-theoretic bounds. These tests witness:
    //
    // 1. Token budget monotonicity: larger budget => equal or larger output
    // 2. Budget respect: output tokens never exceed budget
    // 3. Relevance monotonicity: more relevant datoms => no decrease in output
    // 4. Rate function: information retained decreases as budget decreases
    // -------------------------------------------------------------------

    mod seed_proptests {
        use super::*;
        use crate::datom::AgentId;
        use crate::proptest_strategies::arb_store;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn assemble_seed_deterministic(
                store in arb_store(3),
                budget in 100usize..5000,
            ) {
                let agent = AgentId::from_name("det-test");
                let task = "determinism check";
                let seed_a = assemble_seed(&store, task, budget, agent);
                let seed_b = assemble_seed(&store, task, budget, agent);

                prop_assert_eq!(
                    seed_a.context.sections.len(),
                    seed_b.context.sections.len(),
                    "section count must be identical across calls"
                );
                prop_assert_eq!(
                    seed_a.context.total_tokens,
                    seed_b.context.total_tokens,
                    "total_tokens must be identical across calls"
                );
                prop_assert_eq!(
                    seed_a.context.budget_remaining,
                    seed_b.context.budget_remaining,
                    "budget_remaining must be identical across calls"
                );
                prop_assert_eq!(
                    seed_a.entities_discovered,
                    seed_b.entities_discovered,
                    "entities_discovered must be identical across calls"
                );
                prop_assert_eq!(
                    seed_a.context.projection_pattern,
                    seed_b.context.projection_pattern,
                    "projection_pattern must be identical across calls"
                );
            }

            #[test]
            fn assemble_seed_always_five_sections(
                store in arb_store(3),
                budget in 50usize..5000,
            ) {
                let agent = AgentId::from_name("sections-test");
                let seed = assemble_seed(&store, "any task", budget, agent);

                prop_assert_eq!(
                    seed.context.sections.len(),
                    5,
                    "Seed must always have exactly 5 sections \
                     (Orientation, Constraints, State, Warnings, Directive), got {}",
                    seed.context.sections.len()
                );
            }

            #[test]
            fn assemble_seed_token_budget_respected(
                store in arb_store(3),
                budget in 50usize..5000,
            ) {
                let agent = AgentId::from_name("budget-test");
                let seed = assemble_seed(&store, "budget test", budget, agent);

                prop_assert!(
                    seed.context.total_tokens <= budget,
                    "INV-SEED-002: total_tokens ({}) must be <= budget ({})",
                    seed.context.total_tokens,
                    budget
                );
            }
        }
    }

    mod rate_distortion_proptests {
        use super::*;
        use crate::datom::{AgentId, Attribute, ProvenanceType, Value};
        use crate::proptest_strategies::arb_store;
        use crate::store::Transaction;
        use proptest::prelude::*;

        fn count_state_entries(ctx: &AssembledContext) -> usize {
            ctx.sections
                .iter()
                .map(|s| match s {
                    ContextSection::State(entries) => entries.len(),
                    _ => 0,
                })
                .sum()
        }

        fn state_token_sum(ctx: &AssembledContext) -> usize {
            ctx.sections
                .iter()
                .map(|s| match s {
                    ContextSection::State(entries) => {
                        entries.iter().map(|e| e.tokens).sum::<usize>()
                    }
                    _ => 0,
                })
                .sum()
        }

        proptest! {
            // RD-1: Budget respect -- seed output never exceeds the declared budget.
            // This is INV-SEED-002 witnessed via proptest.
            #[test]
            fn rate_distortion_budget_respect(
                store in arb_store(3),
                budget in 50usize..5000,
            ) {
                let agent = AgentId::from_name("rd-test");
                let seed = assemble_seed(&store, "test task", budget, agent);

                prop_assert!(
                    seed.context.total_tokens <= budget,
                    "INV-SEED-002 violated: total_tokens ({}) > budget ({})",
                    seed.context.total_tokens,
                    budget
                );
            }

            // RD-2: Fixed-projection budget monotonicity -- at the same
            // projection level, more budget fits more or equal state entries.
            // We pick two budgets in the same projection-level band (>2000)
            // to hold projection constant, then verify entry count is monotone.
            #[test]
            fn rate_distortion_budget_monotonicity_fixed_projection(
                store in arb_store(3),
                budget_small in 2001usize..3000,
            ) {
                let budget_large = budget_small + 1000;

                let neighborhood = SchemaNeighborhood {
                    entities: store.entities().into_iter().collect(),
                    attributes: vec![],
                    entity_types: vec![],
                };

                let small_ctx = assemble(&store, &neighborhood, "test task", budget_small);
                let large_ctx = assemble(&store, &neighborhood, "test task", budget_large);

                // Both should use Full projection (budget > 2000)
                prop_assert_eq!(
                    small_ctx.projection_pattern,
                    ProjectionLevel::Full,
                    "Expected Full projection for budget {}",
                    budget_small
                );
                prop_assert_eq!(
                    large_ctx.projection_pattern,
                    ProjectionLevel::Full,
                    "Expected Full projection for budget {}",
                    budget_large
                );

                let small_entries = count_state_entries(&small_ctx);
                let large_entries = count_state_entries(&large_ctx);

                prop_assert!(
                    large_entries >= small_entries,
                    "Budget monotonicity (fixed projection) violated: budget {} => {} entries, \
                     budget {} => {} entries",
                    budget_large, large_entries, budget_small, small_entries
                );
            }

            // RD-3: Projection level monotonicity -- larger budgets produce equal
            // or higher projection levels (more detail per entity).
            #[test]
            fn rate_distortion_projection_monotonicity(
                store in arb_store(2),
                budget_small in 50usize..500,
            ) {
                let budget_large = budget_small + 2000;
                let neighborhood = SchemaNeighborhood::default();

                let small_ctx = assemble(&store, &neighborhood, "task", budget_small);
                let large_ctx = assemble(&store, &neighborhood, "task", budget_large);

                prop_assert!(
                    large_ctx.projection_pattern >= small_ctx.projection_pattern,
                    "Projection monotonicity violated: larger budget selected lower projection \
                     ({:?} < {:?})",
                    large_ctx.projection_pattern,
                    small_ctx.projection_pattern
                );
            }

            // RD-4: Rate function -- at fixed projection level, state tokens
            // consumed are non-decreasing as budget grows. This is the
            // rate-distortion R(D): more budget => more information retained.
            #[test]
            fn rate_distortion_rate_function(
                store in arb_store(3),
                budget_small in 2001usize..3000,
            ) {
                let budget_large = budget_small + 1000;

                let neighborhood = SchemaNeighborhood {
                    entities: store.entities().into_iter().collect(),
                    attributes: vec![],
                    entity_types: vec![],
                };

                let small_ctx = assemble(&store, &neighborhood, "test", budget_small);
                let large_ctx = assemble(&store, &neighborhood, "test", budget_large);

                let small_tokens = state_token_sum(&small_ctx);
                let large_tokens = state_token_sum(&large_ctx);

                prop_assert!(
                    large_tokens >= small_tokens,
                    "Rate function violated: budget {} => {} state tokens, \
                     budget {} => {} state tokens",
                    budget_large, large_tokens, budget_small, small_tokens
                );
            }

            // RD-5: Budget remaining is non-negative and consistent.
            // total_tokens + budget_remaining <= budget.
            #[test]
            fn rate_distortion_budget_accounting(
                store in arb_store(2),
                budget in 50usize..5000,
            ) {
                let neighborhood = SchemaNeighborhood {
                    entities: store.entities().into_iter().collect(),
                    attributes: vec![],
                    entity_types: vec![],
                };

                let ctx = assemble(&store, &neighborhood, "test", budget);

                prop_assert!(
                    ctx.total_tokens + ctx.budget_remaining <= budget,
                    "Budget accounting violated: total ({}) + remaining ({}) > budget ({})",
                    ctx.total_tokens,
                    ctx.budget_remaining,
                    budget
                );
            }

            // RD-6: Relevance monotonicity -- adding datoms matching the task
            // keywords to the store does not decrease the number of entities
            // discovered by ASSOCIATE.
            #[test]
            fn rate_distortion_relevance_monotonicity(
                store in arb_store(2),
            ) {
                let agent = AgentId::from_name("rd-test");
                let task = "documentation";
                let budget = 3000;

                let seed_before = assemble_seed(&store, task, budget, agent);
                let discovered_before = seed_before.entities_discovered;

                // Add a datom that matches the task keyword "documentation"
                let mut enriched = store.clone_store();
                let enrich_agent = AgentId::from_name("enricher");
                let entity = EntityId::from_ident(":test/enrichment");
                let tx = Transaction::new(
                    enrich_agent,
                    ProvenanceType::Observed,
                    "add relevant datom",
                )
                .assert(
                    entity,
                    Attribute::from_keyword(":db/doc"),
                    Value::String("documentation for testing relevance".into()),
                )
                .commit(&enriched)
                .unwrap();
                enriched.transact(tx).unwrap();

                let seed_after = assemble_seed(&enriched, task, budget, agent);
                let discovered_after = seed_after.entities_discovered;

                prop_assert!(
                    discovered_after >= discovered_before,
                    "Relevance monotonicity violated: adding relevant datoms decreased \
                     entities discovered from {} to {}",
                    discovered_before,
                    discovered_after
                );
            }

            // RD-7: Genesis store seed -- always well-formed with 5 sections
            // and respects any budget.
            #[test]
            fn rate_distortion_genesis_wellformed(
                budget in 100usize..5000,
            ) {
                let store = Store::genesis();
                let agent = AgentId::from_name("rd-test");
                let seed = assemble_seed(&store, "anything", budget, agent);

                prop_assert_eq!(
                    seed.context.sections.len(),
                    5,
                    "Seed must always have exactly 5 sections"
                );
                prop_assert!(
                    seed.context.total_tokens <= budget,
                    "Budget violated even for genesis store"
                );
            }
        }
    }
}
