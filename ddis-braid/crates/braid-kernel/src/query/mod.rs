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
    critical_path, density, edge_laplacian, first_betti_number, pagerank, scc, topo_sort,
    DenseMatrix,
};
pub use stratum::{check_stage0, classify, Stratum};
