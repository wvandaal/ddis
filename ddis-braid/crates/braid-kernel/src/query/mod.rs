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
//! - **INV-QUERY-005**: Strata 0-1 at Stage 0 (S2+ rejected).
//! - **INV-QUERY-006**: Entity-centric view via index scan.
//! - **INV-QUERY-007**: Frontier as queryable attribute.
//! - **INV-QUERY-012**: Graph topology operations (topo sort, SCC, PageRank).
//! - **INV-QUERY-013**: Critical path analysis.
//! - **INV-QUERY-014**: Graph density computation.

pub mod clause;
pub mod evaluator;
pub mod graph;
pub mod stratum;

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
