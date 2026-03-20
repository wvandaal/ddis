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
pub mod census;
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
pub mod spec_id;
pub mod stage;
pub mod store;
pub mod task;
pub mod topology;
pub mod trace;
pub mod trilateral;
pub mod witness;

// Re-export core types at crate root for ergonomic access.
pub use agent_md::{generate_agent_md, AgentMdConfig, AgentMdSection, GeneratedAgentMd};
pub use agent_store::{AgentStore, CommitError};
pub use bilateral::{
    analyze_convergence, backward_scan, compute_fitness, compute_fitness_with_registry,
    cycle_to_datoms, default_boundaries, depth_weight, evaluate_conditions, format_terse,
    format_verbose, forward_scan, load_trajectory, run_cycle, spectral_certificate, BilateralScan,
    BilateralState, Boundary, BoundaryCheck, BoundaryDivergence, BoundaryEvaluation,
    BoundaryRegistry, CoherenceConditions, ConditionResult, ConvergenceAnalysis,
    DivergenceDirection, EntropyDecomposition, FitnessComponents, FitnessScore, Gap, GapSeverity,
    RenyiSpectrum, ScanResult, SetRelation, SpecImplBoundary, SpectralCertificate,
};
pub use branch::{branch_datoms, compare_branches, create_branch, merge_branch, prune_branch};
pub use budget::{
    attention_decay, classify_command, enforce_ceiling, quality_adjusted_budget,
    safe_truncate_bytes, safe_truncate_display, ActionProjection, ActivationStrategy,
    ApproxTokenCounter, AttentionProfile, BudgetManager, BudgetProjection, ContextBlock,
    GuidanceLevel, OutputBlock, OutputPrecedence, ProjectedAction, SessionPhase, TokenCounter,
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
pub use error::{KernelError, TopologyError};
pub use guidance::{
    adjust_gaps, build_command_footer, build_command_footer_with_hint, build_footer,
    build_footer_with_budget, classify_action_outcome, compute_action_from_store,
    compute_methodology_score, compute_routing, compute_routing_from_store,
    contextual_observation_hint, create_session_start_datoms,
    create_session_start_datoms_with_name, crystallization_candidates,
    default_derivation_rules, derive_actions, derive_actions_with_budget, derive_tasks,
    detect_activity_mode, detect_session_start, dynamic_threshold, format_actions, format_footer,
    format_footer_at_level, harvest_urgency_multi, harvest_warning_from_k_eff,
    harvest_warning_level, is_actionable_decision, knowledge_relevance_scan,
    methodology_context_blocks, methodology_gaps, modulate_actions, observation_staleness,
    orphaned_decisions, reconciliation_check, refit_routing_weights, routing_dashboard,
    routing_weights, should_warn_on_exit, spec_anchor_factor, spec_relevance_scan,
    suggest_task_title, telemetry_from_store, tx_velocity, ActionCategory, ActivityMode,
    AdjustedGaps, ContextualHint, DerivationRule, DerivedTask, GuidanceAction, GuidanceContext,
    GuidanceFooter, HarvestWarningLevel, MethodologyComponents, MethodologyGaps, MethodologyScore,
    ReconciliationResult, RoutingDashboard, RoutingMetrics, SessionTelemetry, TaskNode, TaskRouting,
    Trend, ROUTING_FEATURE_NAMES,
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
    cascade_full, cascade_step1_conflicts, cascade_stub_datoms, detect_merge_conflicts,
    merge_stores, run_cascade, verify_frontier_advancement, verify_monotonicity, CascadeReceipt,
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
    aggregate, betweenness_centrality, check_stage0, cheeger, classify, conflict_sheaf,
    constant_sheaf, critical_path, density, edge_laplacian, evaluate, fiedler,
    fiedler_from_spectrum, first_betti_number, graph_laplacian, heat_kernel_from_spectrum,
    heat_kernel_trace, hits, k_core_decomposition, k_shell, kirchhoff_from_partial_spectrum,
    kirchhoff_from_spectrum, kirchhoff_index, lanczos_k_smallest, ollivier_ricci_curvature,
    pagerank, persistence_distance, persistent_homology, ricci_curvature_adaptive, ricci_summary,
    scc, spectral_decomposition, spectral_decomposition_adaptive, structural_complexity, topo_sort,
    total_persistence, tx_barcode, tx_filtration, AggregateFunction, AggregateSpec, Binding,
    BirthDeath, CellularSheaf, CheegerResult, Clause, DenseMatrix, DiGraph, FiedlerResult,
    FindSpec, Pattern, PersistenceDiagram, QueryExpr, QueryMode, QueryResult, RicciSummary,
    SheafCohomology, SparseLaplacian, SpectralDecomposition, Stratum, StructuralComplexity,
};
pub use resolution::{
    conflict_to_datoms, detect_conflicts, has_conflict, live_entity, resolve, resolve_with_trail,
    verify_convergence, ConflictEntity, ConflictSet, ResolutionRecord, ResolvedValue,
};
pub use schema::{
    domain_schema_datoms, full_schema_datoms, genesis_attr_count, has_layer_4, layer_1_attributes,
    layer_1_count, layer_1_datoms, layer_2_attributes, layer_2_count, layer_2_datoms,
    layer_3_attributes, layer_3_count, layer_3_datoms, layer_4_attributes, layer_4_count,
    layer_4_datoms, layer_4_evolution_tx, validate_cardinality, validate_lattice,
    validate_retraction_consistency, AttributeDef, AttributeSpec, Cardinality,
    CardinalityViolation, ResolutionMode, RetractionViolation, Schema, Uniqueness, ValueType,
};
pub use seed::{
    assemble, assemble_seed, associate, verify_seed, AssembledContext, AssociateCue,
    ContextSection, ProjectionLevel, SchemaNeighborhood, SeedOutput, SeedVerification, StateEntry,
};
pub use signal::{
    corrective_footer, count_signals, detect_aleatory, detect_all_divergence, detect_axiological,
    detect_confusion, detect_consequential, detect_logical, detect_procedural,
    detect_procedural_with_threshold, detect_temporal, detect_temporal_with_threshold, dispatch,
    signal_to_datoms, ConfusionDetector, DivergenceType, Severity, Signal, SignalAction,
    SignalType,
};
pub use stage::{capabilities, max_stage, stage_name};
pub use store::{
    Frontier, MergeCascadeReceipt, MergeReceipt, SnapshotView, Store, TxData, TxReceipt,
};
pub use task::{
    all_tasks, audit_tasks_from_store, check_dependency_acyclicity, close_task_datoms,
    compute_ready_set, create_task_datoms, dep_add_datom, extract_criterion_identifiers,
    find_task_by_id, generate_task_id, generate_title_levels, parse_acceptance_criteria,
    parse_spec_refs, resolve_spec_refs, resolve_task_status, set_attribute_datom, short_title,
    task_counts, task_summary, update_status_datom, AuditEvidence, CreateTaskParams, TaskStatus,
    TaskSummary, TaskType,
};
pub use topology::{
    agent_name_from_files, balance_assign, classify_task_phase, composite_coupling,
    compute_file_coupling, compute_invariant_coupling, coupling_density_matrix,
    emit_seed_for_agent, extract_task_files, format_plan_agent, format_plan_human,
    partition_by_file_coupling, partition_quality, phase_plan, quick_plan, ready_task_files,
    select_topology, spec_dependency_datoms, spectral_partition,
    von_neumann_entropy_from_eigenvalues, AgentAssignment, CalmTier, CouplingAnalysis, Phase,
    PlanMethod, TopologyPattern, TopologyPlan,
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
pub use witness::{
    alignment_threshold, all_witnesses, auto_task_on_refutation, challenge_witness,
    check_depth_monotonic, completeness_guard, content_hash, create_fbw, current_spec_hashes,
    detect_stale_witnesses, fbw_to_datoms, keyword_alignment_score, mark_stale_datoms,
    witness_and_challenge, witness_gaps, witness_validation_score, ChallengeResult,
    CurrentSpecHashes, StaleReason, WitnessParams, WitnessStatus, WitnessVerdict, FBW,
};
