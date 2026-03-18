#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::all)]

//! `braid-kernel` — Pure computation library for the Braid datom store.
//!
//! This crate contains all domain logic for Braid. It has no IO, no async,
//! no filesystem access, and no network. Every function is deterministic:
//! same inputs produce same outputs. This is the verification surface for
//! all property-based testing and bounded model checking.
//!
//! # Foundation Design Decisions
//!
//! - ADR-FOUNDATION-001: Braid replaces Go CLI — new implementation from spec.
//! - ADR-FOUNDATION-003: D-centric agent system formalism.
//! - ADR-FOUNDATION-004: Specification uses DDIS formalism (INV/ADR/NEG).
//! - ADR-FOUNDATION-005: Structural over procedural coherence.
//! - ADR-FOUNDATION-006: Self-bootstrap fixed-point property (C7).
//! - ADR-STORE-007: File-backed store with Git transport.
//! - ADR-STORE-009: Crash-recovery model (replay from durable tx files).
//! - ADR-STORE-010: At-least-once delivery semantics.
//! - ADR-STORE-017: Datom store over vector DB / RAG.
//! - ADR-STORE-018: Datom store replaces JSONL event stream.
//! - ADR-VERIFICATION-001: Property-based testing + model checking.

pub mod agent_md;
pub mod agent_store;
pub mod bilateral;
pub mod branch;
pub mod budget;
pub mod coherence;
pub mod compiler;
pub mod config;
pub mod datom;
pub mod deliberation;
pub mod error;
pub mod guidance;
pub mod harvest;
#[cfg(kani)]
mod kani_proofs;
pub mod layout;
pub mod merge;
pub mod promote;
pub mod proposal;
#[cfg(test)]
pub mod proptest_strategies;
pub mod query;
pub mod resolution;
pub mod schema;
pub mod seed;
pub mod signal;
pub mod stage;
pub mod store;
pub mod task;
pub mod trace;
pub mod trilateral;

// Re-export core types at crate root for ergonomic access.
pub use agent_md::{generate_agent_md, AgentMdConfig, AgentMdSection, GeneratedAgentMd};
pub use agent_store::{AgentStore, CommitError};
pub use bilateral::{
    analyze_convergence, backward_scan, compute_fitness, cycle_to_datoms, depth_weight,
    evaluate_conditions, format_terse, format_verbose, forward_scan, load_trajectory, run_cycle,
    spectral_certificate, BilateralScan, BilateralState, Boundary, CoherenceConditions,
    ConditionResult, ConvergenceAnalysis, EntropyDecomposition, FitnessComponents, FitnessScore,
    Gap, GapSeverity, RenyiSpectrum, ScanResult, SpectralCertificate,
};
pub use branch::{branch_datoms, compare_branches, create_branch, merge_branch, prune_branch};
pub use budget::{
    attention_decay, classify_command, enforce_ceiling, quality_adjusted_budget,
    safe_truncate_bytes, safe_truncate_display, ApproxTokenCounter, AttentionProfile,
    BudgetManager, BudgetProjection, GuidanceLevel, OutputBlock, OutputPrecedence, TokenCounter,
    TokenEfficiency, AGENT_MODE_CEILING, BUDGET_FRACTION, DEFAULT_WINDOW_SIZE,
    ERROR_MESSAGE_CEILING, GUIDANCE_FOOTER_CEILING, MIN_OUTPUT,
};
pub use coherence::{
    coherence_check, tier1_check, tier2_check, transact_with_coherence, CoherenceError,
    CoherenceTier, CoherenceViolation,
};
pub use compiler::{
    detect_patterns, detect_patterns_for_text, emit_proptest, emit_test_module,
    extract_test_property, summarize_patterns, InvariantPattern, PatternMatch, PatternSummary,
    TestProperty,
};
pub use config::{
    all_config, defaults as config_defaults, get_config, get_config_or, set_config_datoms,
};
pub use datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
pub use deliberation::{
    add_position, check_stability, coherence_violation_to_deliberation, decide, find_precedent,
    open_deliberation, DecisionMethod, DeliberationStatus, StabilityScore,
};
pub use error::KernelError;
pub use guidance::{
    build_command_footer, build_footer, build_footer_with_budget, compute_methodology_score,
    compute_routing, compute_routing_from_store, default_derivation_rules, derive_actions,
    derive_actions_with_budget, derive_tasks, format_actions, format_footer,
    format_footer_at_level, harvest_warning_from_k_eff, harvest_warning_level, modulate_actions,
    observation_staleness, should_warn_on_exit, ActionCategory, DerivationRule, DerivedTask,
    GuidanceAction, GuidanceFooter, HarvestWarningLevel, MethodologyComponents, MethodologyScore,
    RoutingMetrics, SessionTelemetry, TaskNode, TaskRouting, Trend,
};
pub use harvest::{
    build_harvest_commit, calibrate_harvest, candidate_to_datoms, classify_spec_candidate,
    contains_negative_constraint, contains_universal_quantifier, crystallization_guard,
    harvest_pipeline, has_alternatives, infer_task_description, optimal_threshold, propose_adr,
    propose_invariant, propose_negative, stability_score, CalibrationResult, CandidateStatus,
    CrystallizationResult, HarvestCandidate, HarvestCategory, HarvestCommit, HarvestQuality,
    HarvestResult, SessionContext, SpecCandidate, SpecCandidateType,
    DEFAULT_CRYSTALLIZATION_THRESHOLD,
};
pub use layout::{
    collect_datoms, deserialize_tx, serialize_tx, tx_content_hash, verify_content_hash,
    ContentHash, EdnParseError, IntegrityError, IntegrityReport, LayoutConfig, TxFile, TxFilePath,
};
pub use merge::{
    cascade_step1_conflicts, cascade_stub_datoms, detect_merge_conflicts, merge_stores,
    run_cascade, verify_frontier_advancement, verify_monotonicity, CascadeReceipt,
};
pub use promote::{
    is_already_promoted, promote, promote_batch, verify_dual_identity, BatchPromotionResult,
    DualIdentityCheck, PromotionRequest, PromotionResult, PromotionTargetType,
};
pub use proposal::{
    accept_proposal, accept_with_coherence_check, auto_accept_threshold, pending_proposals,
    proposal_to_datoms, reject_proposal,
};
pub use query::{
    aggregate, betweenness_centrality, cheeger, conflict_sheaf, constant_sheaf, critical_path,
    density, edge_laplacian, evaluate, fiedler, fiedler_from_spectrum, first_betti_number,
    graph_laplacian, heat_kernel_from_spectrum, heat_kernel_trace, hits, k_core_decomposition,
    k_shell, kirchhoff_from_partial_spectrum, kirchhoff_from_spectrum, kirchhoff_index,
    lanczos_k_smallest, ollivier_ricci_curvature, pagerank, persistence_distance,
    persistent_homology, ricci_curvature_adaptive, ricci_summary, scc, spectral_decomposition,
    spectral_decomposition_adaptive, structural_complexity, topo_sort, total_persistence,
    tx_barcode, tx_filtration, AggregateFunction, AggregateSpec, Binding, BirthDeath,
    CellularSheaf, CheegerResult, Clause, DenseMatrix, DiGraph, FiedlerResult, FindSpec, Pattern,
    PersistenceDiagram, QueryExpr, QueryResult, RicciSummary, SheafCohomology, SparseLaplacian,
    SpectralDecomposition, StructuralComplexity,
};
pub use resolution::{
    conflict_to_datoms, detect_conflicts, has_conflict, live_entity, resolve, resolve_with_trail,
    verify_convergence, ConflictEntity, ConflictSet, ResolutionRecord, ResolvedValue,
};
pub use schema::{
    domain_schema_datoms, full_schema_datoms, has_layer_4, layer_1_attributes, layer_1_datoms,
    layer_2_attributes, layer_2_datoms, layer_3_attributes, layer_3_datoms, layer_4_attributes,
    layer_4_datoms, layer_4_evolution_tx, validate_lattice, AttributeDef, AttributeSpec,
    Cardinality, ResolutionMode, Schema, Uniqueness, ValueType, GENESIS_ATTR_COUNT, LAYER_1_COUNT,
    LAYER_2_COUNT, LAYER_3_COUNT, LAYER_4_COUNT,
};
pub use seed::{
    assemble, assemble_seed, associate, verify_seed, AssembledContext, AssociateCue,
    ContextSection, ProjectionLevel, SchemaNeighborhood, SeedOutput, SeedVerification, StateEntry,
};
pub use signal::{
    corrective_footer, count_signals, detect_confusion, dispatch, signal_to_datoms,
    ConfusionDetector, Severity, Signal, SignalAction, SignalType,
};
pub use stage::{capabilities, max_stage, stage_name};
pub use store::{
    Frontier, MergeCascadeReceipt, MergeReceipt, SnapshotView, Store, TxData, TxReceipt,
};
pub use task::{
    all_tasks, check_dependency_acyclicity, close_task_datoms, compute_ready_set,
    create_task_datoms, dep_add_datom, find_task_by_id, generate_task_id, resolve_task_status,
    task_counts, task_summary, update_status_datom, CreateTaskParams, TaskStatus, TaskSummary,
    TaskType,
};
pub use trace::{
    links_to_datoms, scan_source, summarize, TraceLink, TraceSummary, VerificationDepth,
};
pub use trilateral::{
    check_coherence, check_coherence_fast, classify_attribute, compute_phi, compute_phi_default,
    formality_level, isp_check, live_projections, von_neumann_entropy, AttrNamespace,
    CoherenceEntropy, CoherenceQuadrant, CoherenceReport, DivergenceComponents, IspResult,
    LiveView,
};
