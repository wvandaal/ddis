//! Datalog query engine with graph algorithms.
//!
//! The query engine evaluates Datalog-like queries against the store using
//! semi-naive bottom-up fixpoint evaluation. Stage 0 supports strata 0-1
//! (monotonic queries only, CALM compliant).
//!
//! # Invariants
//!
//! - **INV-QUERY-001**: Semi-naive fixpoint convergence (Knaster-Tarski).
//! - **INV-QUERY-002**: CALM compliance for S0/S1 (monotonic, no coordination).
//! - **INV-QUERY-003**: Query significance tracking.
//! - **INV-QUERY-004**: Branch visibility in query results.
//! - **INV-QUERY-005**: Strata 0-1 at Stage 0 (S2+ rejected).
//! - **INV-QUERY-006**: Entity-centric view via index scan.
//! - **INV-QUERY-007**: Frontier as queryable attribute.
//! - **INV-QUERY-008**: FFI boundary purity (derived functions are pure).
//! - **INV-QUERY-009**: Bilateral query symmetry.
//! - **INV-QUERY-010**: Topology-agnostic results.
//! - **INV-QUERY-011**: Projection reification.
//! - **INV-QUERY-012**: Graph topology operations (topo sort, SCC, PageRank).
//! - **INV-QUERY-013**: Critical path analysis.
//! - **INV-QUERY-014**: Graph density computation.
//! - **INV-QUERY-016**: HITS hub/authority scoring.
//! - **INV-QUERY-018**: k-core decomposition.
//! - **INV-QUERY-019**: Eigenvector centrality.
//! - **INV-QUERY-020**: Articulation points.
//! - **INV-QUERY-021**: Graph density metrics.
//! - **INV-QUERY-022**: Spectral computation correctness.
//!
//! # Design Decisions
//!
//! - ADR-QUERY-002: Semi-naive bottom-up evaluation over top-down.
//! - ADR-QUERY-003: Six-stratum classification.
//! - ADR-QUERY-004: FFI for derived functions (pure Rust, no external calls).
//! - ADR-QUERY-005: Local frontier as default query scope.
//! - ADR-QUERY-006: Frontier as datom attribute (queryable).
//! - ADR-QUERY-007: Projection pyramid (π₀–π₃).
//! - ADR-QUERY-008: Bilateral query layer.
//! - ADR-QUERY-009: Full graph engine in kernel.
//! - ADR-QUERY-010: Agent-store composition in three layers.
//! - ADR-QUERY-011: Query stability score.
//! - ADR-QUERY-012: Spectral graph operations via nalgebra.
//! - ADR-QUERY-013: Hodge-theoretic coherence via edge Laplacian.
//!
//! # Negative Cases
//!
//! - NEG-QUERY-001: No non-monotonic queries in monotonic mode.
//! - NEG-QUERY-002: No query side effects — queries are pure reads.
//! - NEG-QUERY-003: No unbounded query evaluation — termination guaranteed.
//! - NEG-QUERY-004: No access events in main store (local to working set).

pub mod aggregate;
pub mod clause;
pub mod evaluator;
pub mod graph;
pub mod stratum;

pub use aggregate::{aggregate, AggregateFunction, AggregateSpec};
pub use clause::{Binding, Clause, FindSpec, Pattern, QueryExpr};
pub use evaluator::{evaluate, QueryResult};
pub use graph::{
    betweenness_centrality, cheeger, conflict_sheaf, constant_sheaf, critical_path, density,
    edge_laplacian, fiedler, fiedler_from_spectrum, first_betti_number, graph_laplacian,
    heat_kernel_from_spectrum, heat_kernel_trace, kirchhoff_from_partial_spectrum,
    kirchhoff_from_spectrum, kirchhoff_index, lanczos_k_smallest, ollivier_ricci_curvature,
    pagerank, persistence_distance, persistent_homology, ricci_curvature_adaptive, ricci_summary,
    scc, spectral_decomposition, spectral_decomposition_adaptive, structural_complexity, topo_sort,
    total_persistence, tx_barcode, tx_filtration, BirthDeath, CellularSheaf, CheegerResult,
    DenseMatrix, DiGraph, FiedlerResult, PersistenceDiagram, RicciSummary, SheafCohomology,
    SparseLaplacian, SpectralDecomposition, StructuralComplexity,
};
pub use stratum::{check_stage0, classify, Stratum};
