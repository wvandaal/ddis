#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::all)]

//! `braid-kernel` — Pure computation library for the Braid datom store.
//!
//! This crate contains all domain logic for Braid. It has no IO, no async,
//! no filesystem access, and no network. Every function is deterministic:
//! same inputs produce same outputs. This is the verification surface for
//! all property-based testing and bounded model checking.

pub mod claude_md;
pub mod datom;
pub mod error;
pub mod guidance;
pub mod harvest;
pub mod layout;
pub mod merge;
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
pub use claude_md::{generate_claude_md, ClaudeMdConfig, ClaudeMdSection, GeneratedClaudeMd};
pub use datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
pub use error::KernelError;
pub use guidance::{
    build_footer, compute_methodology_score, compute_routing, default_derivation_rules,
    derive_tasks, format_footer, DerivationRule, DerivedTask, GuidanceFooter,
    MethodologyComponents, MethodologyScore, RoutingMetrics, SessionTelemetry, TaskNode,
    TaskRouting, Trend,
};
pub use harvest::{
    candidate_to_datoms, harvest_pipeline, CandidateStatus, HarvestCandidate, HarvestCategory,
    HarvestQuality, HarvestResult, SessionContext,
};
pub use layout::{
    collect_datoms, deserialize_tx, serialize_tx, tx_content_hash, verify_content_hash,
    ContentHash, EdnParseError, IntegrityError, IntegrityReport, LayoutConfig, TxFile, TxFilePath,
};
pub use merge::{
    detect_merge_conflicts, merge_stores, verify_frontier_advancement, verify_monotonicity,
};
pub use query::{
    critical_path, density, edge_laplacian, evaluate, first_betti_number, pagerank, scc, topo_sort,
    Binding, Clause, DenseMatrix, FindSpec, Pattern, QueryExpr, QueryResult,
};
pub use resolution::{
    conflict_to_datoms, detect_conflicts, resolve, resolve_with_trail, ConflictEntity, ConflictSet,
    ResolutionRecord, ResolvedValue,
};
pub use schema::{
    domain_schema_datoms, layer_1_attributes, layer_1_datoms, layer_2_attributes, layer_2_datoms,
    AttributeDef, AttributeSpec, Cardinality, ResolutionMode, Schema, Uniqueness, ValueType,
    LAYER_2_COUNT,
};
pub use seed::{
    assemble, assemble_seed, associate, AssembledContext, AssociateCue, ContextSection,
    ProjectionLevel, SchemaNeighborhood, SeedOutput, StateEntry,
};
pub use stage::{capabilities, max_stage, stage_name};
pub use store::{Frontier, MergeReceipt, Store, TxData, TxReceipt};
pub use trilateral::{
    check_coherence, classify_attribute, compute_phi, compute_phi_default, formality_level,
    isp_check, live_projections, AttrNamespace, CoherenceQuadrant, CoherenceReport,
    DivergenceComponents, IspResult, LiveView,
};
