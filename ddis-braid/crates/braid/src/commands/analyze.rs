//! `braid analyze` — Run comprehensive graph analytics on the store.
//!
//! Computes topological, spectral, cohomological, and differential-geometric
//! invariants of the entity graph. This is the coherence dashboard: a single
//! command that surfaces structural properties of the knowledge base.
//!
//! Analytics are cached in `.braid/.cache/analytics.json` and reused when
//! the store has not changed (same tx file count and datom count).
//! Pass `--force` to recompute.

use std::collections::BTreeMap;
use std::path::Path;

use braid_kernel::datom::{EntityId, Op, Value};
use braid_kernel::query::graph::{
    betweenness_centrality, cheeger, density, fiedler_from_spectrum, first_betti_number,
    heat_kernel_from_spectrum, kirchhoff_from_partial_spectrum, kirchhoff_from_spectrum, pagerank,
    persistent_homology, ricci_curvature_adaptive, ricci_summary, scc,
    spectral_decomposition_adaptive, structural_complexity, total_persistence, DiGraph,
};
use braid_kernel::trilateral::{compute_phi_default, von_neumann_entropy};
use braid_kernel::Store;

use crate::error::BraidError;
use crate::layout::DiskLayout;

// ---------------------------------------------------------------------------
// Analytics Cache
// ---------------------------------------------------------------------------

/// Cached analytics output, stored in `.braid/.cache/analytics.json`.
#[derive(serde::Serialize, serde::Deserialize)]
struct AnalyticsCache {
    /// Number of transaction files at computation time.
    tx_count: usize,
    /// Number of datoms at computation time.
    datom_count: usize,
    /// Number of entities at computation time.
    entity_count: usize,
    /// The pre-formatted analytics output.
    output: String,
}

fn cache_path(layout: &DiskLayout) -> std::path::PathBuf {
    layout.root.join(".cache").join("analytics.json")
}

fn try_load_cache(layout: &DiskLayout, store: &Store, tx_count: usize) -> Option<String> {
    let data = std::fs::read_to_string(cache_path(layout)).ok()?;
    let cache: AnalyticsCache = serde_json::from_str(&data).ok()?;
    if cache.tx_count == tx_count
        && cache.datom_count == store.len()
        && cache.entity_count == store.entity_count()
    {
        Some(cache.output)
    } else {
        None
    }
}

fn save_cache(layout: &DiskLayout, store: &Store, tx_count: usize, output: &str) {
    let cache = AnalyticsCache {
        tx_count,
        datom_count: store.len(),
        entity_count: store.entity_count(),
        output: output.to_string(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = std::fs::write(cache_path(layout), json);
    }
}

// ---------------------------------------------------------------------------
// Entity graph construction
// ---------------------------------------------------------------------------

/// Build the entity reference graph from the store.
fn build_entity_graph(store: &Store) -> DiGraph {
    let mut graph = DiGraph::new();

    for entity in store.entities() {
        let label = resolve_label(store, entity);
        graph.add_node(&label);
    }

    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
        }
        if let Value::Ref(target) = &datom.value {
            let src = resolve_label(store, datom.entity);
            let dst = resolve_label(store, *target);
            graph.add_edge(&src, &dst);
        }
    }

    graph
}

/// Resolve an EntityId to a display label.
fn resolve_label(store: &Store, entity: EntityId) -> String {
    for datom in store.entity_datoms(entity) {
        if datom.attribute.as_str() == ":db/ident" {
            if let Value::Keyword(kw) = &datom.value {
                return kw.clone();
            }
        }
    }
    let bytes = entity.as_bytes();
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3]
    )
}

// ---------------------------------------------------------------------------
// Dashboard computation
// ---------------------------------------------------------------------------

fn compute_dashboard(store: &Store, path: &Path) -> String {
    let mut out = String::new();
    let mut algo_count = 0_usize;

    // --- Store metrics ---
    out.push_str("═══ Braid Coherence Dashboard ═══\n\n");
    out.push_str(&format!("Store: {}\n", path.display()));
    out.push_str(&format!(
        "  datoms: {}  entities: {}\n\n",
        store.len(),
        store.entity_count(),
    ));

    // --- Build entity graph ---
    let graph = build_entity_graph(store);
    let n = graph.node_count();
    let e = graph.edge_count();

    out.push_str("── Graph Topology ──\n");
    out.push_str(&format!("  nodes: {}  edges: {}\n", n, e));
    out.push_str(&format!("  density: {:.6}\n", density(&graph)));
    algo_count += 1;

    // SCC
    let components = scc(&graph);
    out.push_str(&format!(
        "  strongly connected components: {}\n",
        components.len()
    ));
    if let Some(largest) = components.iter().max_by_key(|c| c.len()) {
        out.push_str(&format!("  largest SCC: {} nodes\n", largest.len()));
    }
    algo_count += 1;

    // Betti number (O(V+E) via Euler characteristic)
    let beta_1 = first_betti_number(&graph);
    out.push_str(&format!("  β₁ (independent cycles): {}\n", beta_1));
    algo_count += 1;

    // --- Spectral Analysis ---
    out.push_str("\n── Spectral Analysis ──\n");

    if n >= 2 {
        // Adaptive spectral decomposition: Jacobi for n≤1000, Lanczos for n>1000.
        // Compute ONCE, reuse for all spectral metrics.
        let sd = spectral_decomposition_adaptive(&graph);
        let is_partial = sd.as_ref().is_some_and(|s| s.eigenvalues.len() < n);

        if is_partial {
            out.push_str(&format!(
                "  (Lanczos: {} of {} eigenvalues computed)\n",
                sd.as_ref().unwrap().eigenvalues.len(),
                n
            ));
        }

        if let Some(ref sd) = sd {
            let fiedler_result = fiedler_from_spectrum(sd);
            out.push_str(&format!(
                "  algebraic connectivity (λ₂): {:.6}\n",
                fiedler_result.algebraic_connectivity
            ));
            out.push_str(&format!(
                "  Fiedler partition: {} / {} nodes\n",
                fiedler_result.partition.0.len(),
                fiedler_result.partition.1.len()
            ));
            algo_count += 1;
        }

        if let Some(cheeger_result) = cheeger(&graph) {
            out.push_str(&format!(
                "  Cheeger constant h(G): {:.6}\n",
                cheeger_result.cheeger_constant
            ));
            out.push_str(&format!(
                "  Cheeger inequality: {:.4} ≤ {:.4} ≤ {:.4} [{}]\n",
                cheeger_result.lower_bound,
                cheeger_result.cheeger_constant,
                cheeger_result.upper_bound,
                if cheeger_result.inequality_holds {
                    "holds"
                } else {
                    "VIOLATED"
                }
            ));
            out.push_str(&format!(
                "  min cut set: {} nodes\n",
                cheeger_result.min_cut_set.len()
            ));
            algo_count += 1;
        }

        if let Some(ref sd) = sd {
            // Kirchhoff index: exact for full spectrum, approximate for partial
            let ki = if is_partial {
                kirchhoff_from_partial_spectrum(&sd.eigenvalues, n)
            } else {
                kirchhoff_from_spectrum(sd)
            };
            out.push_str(&format!(
                "  Kirchhoff index: {:.4}{}\n",
                ki,
                if is_partial { " (approx)" } else { "" }
            ));
            let normalized_ki = ki / (n * (n - 1)) as f64;
            out.push_str(&format!(
                "  normalized resistance: {:.4} (lower = more robust)\n",
                normalized_ki
            ));
            algo_count += 1;

            // Heat kernel trace (from shared eigenvalues)
            let times = [0.01, 0.1, 1.0, 10.0];
            let hkt = heat_kernel_from_spectrum(sd, &times);
            out.push_str("  heat kernel Z(t):");
            for (t, z) in &hkt {
                out.push_str(&format!(" [{:.2}]{:.2}", t, z));
            }
            if is_partial {
                out.push_str(" (partial spectrum)");
            }
            out.push('\n');
            algo_count += 1;
        }

        // Von Neumann entropy (separate eigendecomposition on the ref-edge graph)
        // Only for graphs ≤ 2000 nodes (VN entropy needs full spectrum)
        if n <= 2000 {
            let entropy = von_neumann_entropy(store);
            out.push_str(&format!(
                "  von Neumann entropy: {:.4} / {:.4} (normalized: {:.4})\n",
                entropy.entropy, entropy.max_entropy, entropy.normalized
            ));
            out.push_str(&format!("  effective rank: {}\n", entropy.effective_rank));
            algo_count += 1;
        }
    } else {
        out.push_str("  (need ≥ 2 nodes for spectral analysis)\n");
    }

    // --- Differential Geometry ---
    out.push_str("\n── Differential Geometry ──\n");

    if n >= 2 && e > 0 {
        // Adaptive Ricci: exact BFS for n≤500, landmark-based for n>500
        let is_approx = n > 500;
        let curvatures = ricci_curvature_adaptive(&graph);
        let summary = ricci_summary(&curvatures);

        if is_approx {
            out.push_str("  (landmark-based approximation for large graph)\n");
        }
        out.push_str(&format!(
            "  Ollivier-Ricci curvature: mean={:.4} min={:.4} max={:.4}\n",
            summary.mean_curvature, summary.min_curvature, summary.max_curvature
        ));
        out.push_str(&format!(
            "  curvature distribution: {} positive, {} negative ({} edges)\n",
            summary.positive_edges,
            summary.negative_edges,
            curvatures.len()
        ));
        if let Some((ref src, ref dst)) = summary.bottleneck_edge {
            out.push_str(&format!(
                "  worst bottleneck: {} → {} (κ={:.4})\n",
                src, dst, summary.min_curvature
            ));
        }
        if let Some((ref src, ref dst)) = summary.tightest_edge {
            out.push_str(&format!(
                "  tightest cluster: {} → {} (κ={:.4})\n",
                src, dst, summary.max_curvature
            ));
        }
        algo_count += 1;
    } else {
        out.push_str("  (need ≥ 2 nodes and ≥ 1 edge for curvature analysis)\n");
    }

    // --- Persistent Homology ---
    out.push_str("\n── Persistent Homology ──\n");

    let mut edges: Vec<(String, String)> = Vec::new();
    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
        }
        if let Value::Ref(target) = &datom.value {
            let src = resolve_label(store, datom.entity);
            let dst = resolve_label(store, *target);
            edges.push((src, dst));
        }
    }

    out.push_str(&format!("  ref edges: {}\n", edges.len()));

    let diagram = persistent_homology(&edges);
    let h0_births = diagram.pairs.iter().filter(|p| p.dimension == 0).count();
    let h0_deaths = diagram
        .pairs
        .iter()
        .filter(|p| p.dimension == 0 && p.death.is_some())
        .count();
    let h1_pairs = diagram.pairs.iter().filter(|p| p.dimension == 1).count();
    let tp = total_persistence(&diagram);

    out.push_str(&format!(
        "  H₀ pairs: {} ({} births, {} merges)\n",
        h0_births, h0_births, h0_deaths
    ));
    out.push_str(&format!("  H₁ pairs: {} (cycle formations)\n", h1_pairs));
    out.push_str(&format!("  total persistence: {}\n", tp));
    algo_count += 1;

    // Transaction-filtration barcode
    let sc = structural_complexity(store);
    out.push_str(&format!(
        "  tx barcode: {} merges, {} cycles over {} tx steps\n",
        sc.h0_deaths, sc.h1_births, sc.tx_count
    ));
    out.push_str(&format!(
        "  max component lifetime: {}  total persistence: {}\n",
        sc.max_component_lifetime, sc.total_persistence
    ));
    algo_count += 1;

    // --- Centrality ---
    out.push_str("\n── Centrality Analysis ──\n");

    let pr = pagerank(&graph, 20);
    algo_count += 1;
    let bc = betweenness_centrality(&graph);
    algo_count += 1;

    // Top 5 by PageRank
    let mut pr_sorted: Vec<_> = pr.iter().collect();
    pr_sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    out.push_str("  Top 5 by PageRank:\n");
    for (name, score) in pr_sorted.iter().take(5) {
        out.push_str(&format!("    {:.4}  {}\n", score, name));
    }

    // Top 5 by betweenness
    let mut bc_sorted: Vec<_> = bc.iter().collect();
    bc_sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    out.push_str("  Top 5 by betweenness centrality:\n");
    for (name, score) in bc_sorted.iter().take(5) {
        out.push_str(&format!("    {:.4}  {}\n", score, name));
    }

    // --- Coherence ---
    out.push_str("\n── Trilateral Coherence ──\n");

    let (phi, div_components) = compute_phi_default(store);
    out.push_str(&format!("  Φ (divergence): {:.4}\n", phi));
    out.push_str(&format!(
        "  D_IS (intent→spec gap): {}  D_SP (spec→impl gap): {}\n",
        div_components.d_is, div_components.d_sp
    ));

    let quadrant = match (phi > 0.0, beta_1 > 0) {
        (false, false) => "Coherent",
        (true, false) => "GapsOnly",
        (false, true) => "CyclesOnly",
        (true, true) => "GapsAndCycles",
    };
    out.push_str(&format!("  quadrant: {}\n", quadrant));

    // --- Namespace distribution ---
    out.push_str("\n── Namespace Distribution ──\n");
    let mut ns_counts: BTreeMap<String, usize> = BTreeMap::new();
    for datom in store.datoms() {
        if datom.op == Op::Assert && datom.attribute.as_str() == ":spec/namespace" {
            if let Value::Keyword(ns) = &datom.value {
                *ns_counts.entry(ns.clone()).or_default() += 1;
            }
        }
    }
    if ns_counts.is_empty() {
        out.push_str("  (no namespace annotations found)\n");
    } else {
        let mut ns_sorted: Vec<_> = ns_counts.iter().collect();
        ns_sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (ns, count) in &ns_sorted {
            out.push_str(&format!("  {:>3}  {}\n", count, ns));
        }
    }

    out.push_str(&format!(
        "\n═══ {} datoms, {} entities, {} graph algorithms applied ═══\n",
        store.len(),
        store.entity_count(),
        algo_count
    ));

    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(path: &Path) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let tx_count = layout.list_tx_hashes()?.len();
    let store = layout.load_store()?;

    // Check cache first
    if let Some(cached) = try_load_cache(&layout, &store, tx_count) {
        let mut result = String::new();
        result.push_str("(cached — use `braid analyze --force` to recompute)\n\n");
        result.push_str(&cached);
        return Ok(result);
    }

    // Compute fresh analytics
    let output = compute_dashboard(&store, path);

    // Save to cache
    save_cache(&layout, &store, tx_count, &output);

    Ok(output)
}

/// Run with forced recomputation (ignores cache).
pub fn run_force(path: &Path) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let tx_count = layout.list_tx_hashes()?.len();
    let store = layout.load_store()?;

    let output = compute_dashboard(&store, path);
    save_cache(&layout, &store, tx_count, &output);

    Ok(output)
}
