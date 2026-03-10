#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::all)]

//! `braid-kernel` — Pure computation library for the Braid datom store.
//!
//! This crate contains all domain logic for Braid. It has no IO, no async,
//! no filesystem access, and no network. Every function is deterministic:
//! same inputs produce same outputs. This is the verification surface for
//! all property-based testing and bounded model checking.

pub mod agent_md;
pub mod datom;
pub mod error;
pub mod guidance;
pub mod harvest;
#[cfg(kani)]
mod kani_proofs;
pub mod layout;
pub mod merge;
pub mod promote;
#[cfg(test)]
pub mod proptest_strategies;
pub mod query;
pub mod resolution;
pub mod schema;
pub mod seed;
pub mod stage;
pub mod store;
pub mod trilateral;

// Re-export core types at crate root for ergonomic access.
pub use agent_md::{generate_agent_md, AgentMdConfig, AgentMdSection, GeneratedAgentMd};
pub use datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
pub use error::KernelError;
pub use guidance::{
    build_footer, compute_methodology_score, compute_routing, default_derivation_rules,
    derive_tasks, format_footer, DerivationRule, DerivedTask, GuidanceFooter,
    MethodologyComponents, MethodologyScore, RoutingMetrics, SessionTelemetry, TaskNode,
    TaskRouting, Trend,
};
pub use harvest::{
    build_harvest_commit, calibrate_harvest, candidate_to_datoms, harvest_pipeline,
    optimal_threshold, CalibrationResult, CandidateStatus, HarvestCandidate, HarvestCategory,
    HarvestCommit, HarvestQuality, HarvestResult, SessionContext,
};
pub use layout::{
    collect_datoms, deserialize_tx, serialize_tx, tx_content_hash, verify_content_hash,
    ContentHash, EdnParseError, IntegrityError, IntegrityReport, LayoutConfig, TxFile, TxFilePath,
};
pub use merge::{
    detect_merge_conflicts, merge_stores, verify_frontier_advancement, verify_monotonicity,
};
pub use promote::{
    is_already_promoted, promote, promote_batch, verify_dual_identity, BatchPromotionResult,
    DualIdentityCheck, PromotionRequest, PromotionResult, PromotionTargetType,
};
pub use query::{
    betweenness_centrality, cheeger, conflict_sheaf, constant_sheaf, critical_path, density,
    edge_laplacian, evaluate, fiedler, fiedler_from_spectrum, first_betti_number, graph_laplacian,
    heat_kernel_from_spectrum, heat_kernel_trace, kirchhoff_from_partial_spectrum,
    kirchhoff_from_spectrum, kirchhoff_index, lanczos_k_smallest, ollivier_ricci_curvature,
    pagerank, persistence_distance, persistent_homology, ricci_curvature_adaptive, ricci_summary,
    scc, spectral_decomposition, spectral_decomposition_adaptive, structural_complexity, topo_sort,
    total_persistence, tx_barcode, tx_filtration, Binding, BirthDeath, CellularSheaf,
    CheegerResult, Clause, DenseMatrix, DiGraph, FiedlerResult, FindSpec, Pattern,
    PersistenceDiagram, QueryExpr, QueryResult, RicciSummary, SheafCohomology, SparseLaplacian,
    SpectralDecomposition, StructuralComplexity,
};
pub use resolution::{
    conflict_to_datoms, detect_conflicts, resolve, resolve_with_trail, ConflictEntity, ConflictSet,
    ResolutionRecord, ResolvedValue,
};
pub use schema::{
    domain_schema_datoms, full_schema_datoms, layer_1_attributes, layer_1_datoms,
    layer_2_attributes, layer_2_datoms, layer_3_attributes, layer_3_datoms, AttributeDef,
    AttributeSpec, Cardinality, ResolutionMode, Schema, Uniqueness, ValueType, LAYER_2_COUNT,
    LAYER_3_COUNT,
};
pub use seed::{
    assemble, assemble_seed, associate, verify_seed, AssembledContext, AssociateCue,
    ContextSection, ProjectionLevel, SchemaNeighborhood, SeedOutput, SeedVerification, StateEntry,
};
pub use stage::{capabilities, max_stage, stage_name};
pub use store::{Frontier, MergeReceipt, Store, TxData, TxReceipt};
pub use trilateral::{
    check_coherence, classify_attribute, compute_phi, compute_phi_default, formality_level,
    isp_check, live_projections, von_neumann_entropy, AttrNamespace, CoherenceEntropy,
    CoherenceQuadrant, CoherenceReport, DivergenceComponents, IspResult, LiveView,
};
