//! Compile-time stage definitions and stage-gated feature boundaries.
//!
//! Braid has 4 stages (0-3), each building on the previous. Feature flags
//! control which functionality is compiled in, following these rules:
//!
//! 1. **Type definitions are always compiled** — types are the algebraic
//!    structure; gating them breaks exhaustive matching.
//! 2. **Implementations are feature-gated** — `#[cfg(feature = "stageN")]`
//!    on impl blocks that require stage N capabilities.
//! 3. **Tests use feature gates** — `#[cfg(feature = "stageN")]` on test
//!    modules for stage-appropriate test suites.
//! 4. **The CLI binary enables exactly one stage** via `--features` flag.
//!
//! # Invariants
//!
//! - **INV-STAGE-001**: Stage flags are cumulative (stage1 implies stage0).
//! - **INV-STAGE-002**: Type definitions compile at all stages.
//! - **INV-STAGE-003**: No dead code in default (stage0) builds.
//!
//! # Stage-Gated Spec Namespaces
//!
//! Stage 0 (current): STORE, SCHEMA, QUERY, HARVEST, SEED, GUIDANCE,
//!   MERGE (basic), RESOLUTION (LWW only), LAYOUT, BILATERAL, TRILATERAL, BUDGET
//! Stage 2: DELIBERATION (INV-DELIBERATION-001 through INV-DELIBERATION-006),
//!   MERGE (branching: INV-MERGE-003 through INV-MERGE-007)
//! Stage 3: SYNC (INV-SYNC-001 through INV-SYNC-005)
//!
//! Design decisions for staged activation:
//! - ADR-STORE-012: Three-phase implementation path (Stage 0 → 1 → 2+).
//! - ADR-DELIBERATION-004: Crystallization guard over immediate commit.
//! - ADR-SYNC-001: Barrier as explicit coordination point.
//! - ADR-SYNC-002: Topology-agnostic protocol.
//! - ADR-SYNC-003: Barrier timeout over blocking.

/// The current maximum compiled stage.
///
/// This is determined at compile time by which feature flags are active.
/// Stage flags are cumulative: stage2 implies stage1 implies stage0.
pub const fn max_stage() -> u8 {
    if cfg!(feature = "stage3") {
        3
    } else if cfg!(feature = "stage2") {
        2
    } else if cfg!(feature = "stage1") {
        1
    } else {
        0
    }
}

/// Human-readable stage name.
pub const fn stage_name() -> &'static str {
    match max_stage() {
        0 => "Stage 0: Harvest/Seed Cycle",
        1 => "Stage 1: Budget-Aware Output + Guidance",
        2 => "Stage 2: Branching + Deliberation",
        3 => "Stage 3: Multi-Agent Coordination",
        _ => "Unknown Stage",
    }
}

/// Feature capabilities available at the current stage.
#[allow(unused_mut)]
pub fn capabilities() -> Vec<&'static str> {
    let mut caps = vec![
        "transact",
        "query (strata 0-1)",
        "harvest",
        "seed",
        "guidance",
        "dynamic-claude-md",
        "merge (CRDT set union)",
        "trilateral-coherence",
        "bootstrap",
    ];

    #[cfg(feature = "stage1")]
    {
        caps.extend_from_slice(&[
            "budget-aware-output",
            "attention-decay",
            "query (strata 2-3)",
        ]);
    }

    #[cfg(feature = "stage2")]
    {
        caps.extend_from_slice(&["branching", "deliberation", "query (strata 4-5)"]);
    }

    #[cfg(feature = "stage3")]
    {
        caps.extend_from_slice(&["multi-agent-sync", "frontier-barriers", "consensus"]);
    }

    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_stage_is_zero_in_default_build() {
        // In default builds (stage0 only), max_stage should be 0
        assert_eq!(max_stage(), 0);
    }

    #[test]
    fn stage_name_is_not_empty() {
        assert!(!stage_name().is_empty());
    }

    #[test]
    fn capabilities_include_core_stage0() {
        let caps = capabilities();
        assert!(caps.contains(&"transact"));
        assert!(caps.contains(&"query (strata 0-1)"));
        assert!(caps.contains(&"harvest"));
        assert!(caps.contains(&"seed"));
        assert!(caps.contains(&"guidance"));
        assert!(caps.contains(&"merge (CRDT set union)"));
    }

    #[test]
    fn capabilities_count_matches_stage() {
        let caps = capabilities();
        // Stage 0 has exactly 9 capabilities
        assert_eq!(caps.len(), 9);
    }
}
