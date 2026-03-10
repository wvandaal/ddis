// Standalone entry points (run, run_force, run_json) are retained for potential
// future direct use but currently only run_budget is called via status --deep.
#![allow(dead_code)]
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

/// Resolve a display label to its namespace (or empty string if unknown).
fn entity_namespace(store: &Store, label: &str) -> String {
    // For labeled entities (e.g., ":spec/inv-store-001"), look up the namespace datom
    for datom in store.datoms() {
        if datom.op != Op::Assert {
            continue;
        }
        if datom.attribute.as_str() == ":db/ident" {
            if let Value::Keyword(kw) = &datom.value {
                if kw == label {
                    // Found the entity — look for its namespace
                    for d2 in store.entity_datoms(datom.entity) {
                        if d2.attribute.as_str() == ":spec/namespace" {
                            if let Value::Keyword(ns) = &d2.value {
                                return ns.clone();
                            }
                        }
                    }
                    return String::new();
                }
            }
        }
    }
    String::new()
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
            // Count connected components (multiplicity of λ ≈ 0)
            let num_components = sd.eigenvalues.iter().filter(|&&l| l.abs() < 1e-6).count();
            if num_components > 1 {
                out.push_str(&format!(
                    "  connected components (undirected): {}\n",
                    num_components
                ));
            }

            let fiedler_result = fiedler_from_spectrum(sd);
            out.push_str(&format!(
                "  algebraic connectivity (λ₂): {:.6}\n",
                fiedler_result.algebraic_connectivity
            ));
            if fiedler_result.algebraic_connectivity > 1e-8 {
                out.push_str(&format!(
                    "  Fiedler partition: {} / {} nodes\n",
                    fiedler_result.partition.0.len(),
                    fiedler_result.partition.1.len()
                ));
            } else {
                out.push_str("  Fiedler partition: N/A (graph is disconnected)\n");
            }
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
        // Adaptive Ricci: exact BFS for n≤2000, landmark-based for n>2000
        let is_approx = n > 2000;
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
        // Namespace-level curvature: average κ for edges between namespaces.
        // This reveals inter-domain structural relationships at the semantic level.
        let mut ns_curvature_sum: BTreeMap<(String, String), (f64, usize)> = BTreeMap::new();

        for ((src, dst), &kappa) in &curvatures {
            let src_ns = entity_namespace(store, src);
            let dst_ns = entity_namespace(store, dst);
            let src_label = if src_ns.is_empty() {
                "(tx)".to_string()
            } else {
                src_ns
            };
            let dst_label = if dst_ns.is_empty() {
                "(tx)".to_string()
            } else {
                dst_ns
            };
            let key = if src_label <= dst_label {
                (src_label, dst_label)
            } else {
                (dst_label, src_label)
            };
            let entry = ns_curvature_sum.entry(key).or_insert((0.0, 0));
            entry.0 += kappa;
            entry.1 += 1;
        }

        if !ns_curvature_sum.is_empty() {
            // Sort by mean curvature (most negative first = biggest bottlenecks)
            let mut ns_pairs: Vec<_> = ns_curvature_sum
                .iter()
                .map(|((a, b), (sum, count))| {
                    let mean = sum / *count as f64;
                    (a.clone(), b.clone(), mean, *count)
                })
                .collect();
            ns_pairs.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

            out.push_str("  namespace curvature (inter-domain structure):\n");
            // Bottom 3 (bottlenecks between domains)
            out.push_str("    bottlenecks (negative κ = bridging):\n");
            for (a, b, mean_k, count) in ns_pairs.iter().take(3) {
                let label = if a == b {
                    format!("{} (intra)", a)
                } else {
                    format!("{} ↔ {}", a, b)
                };
                out.push_str(&format!(
                    "      κ={:+.4}  {} ({} edges)\n",
                    mean_k, label, count
                ));
            }
            // Top 3 (tightest clusters)
            let positive_pairs: Vec<_> = ns_pairs.iter().rev().take(3).collect();
            if positive_pairs
                .first()
                .is_some_and(|(_, _, k, _)| *k > 1e-10)
            {
                out.push_str("    clusters (positive κ = cohesion):\n");
                for (a, b, mean_k, count) in &positive_pairs {
                    if *mean_k > 1e-10 {
                        let label = if a == b {
                            format!("{} (intra)", a)
                        } else {
                            format!("{} ↔ {}", a, b)
                        };
                        out.push_str(&format!(
                            "      κ={:+.4}  {} ({} edges)\n",
                            mean_k, label, count
                        ));
                    }
                }
            }
            algo_count += 1;
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

/// Run with JSON output — structured analytics for machine consumption.
pub fn run_json(path: &Path) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let graph = build_entity_graph(&store);
    let n = graph.node_count();
    let e = graph.edge_count();

    // Topology
    let components = scc(&graph);
    let beta_1 = first_betti_number(&graph);
    let d = density(&graph);

    // Coherence
    let (phi, div_components) = compute_phi_default(&store);

    let quadrant = match (phi > 0.0, beta_1 > 0) {
        (false, false) => "Coherent",
        (true, false) => "GapsOnly",
        (false, true) => "CyclesOnly",
        (true, true) => "GapsAndCycles",
    };

    // Spectral (if enough nodes)
    let mut spectral = serde_json::json!(null);
    if n >= 2 {
        let sd = spectral_decomposition_adaptive(&graph);
        if let Some(ref sd) = sd {
            let fiedler_result = fiedler_from_spectrum(sd);
            let is_partial = sd.eigenvalues.len() < n;
            let ki = if is_partial {
                kirchhoff_from_partial_spectrum(&sd.eigenvalues, n)
            } else {
                kirchhoff_from_spectrum(sd)
            };
            spectral = serde_json::json!({
                "algebraic_connectivity": fiedler_result.algebraic_connectivity,
                "kirchhoff_index": ki,
                "normalized_resistance": ki / (n * (n - 1)) as f64,
                "partial_spectrum": is_partial,
            });
        }
        if n <= 2000 {
            let entropy = von_neumann_entropy(&store);
            spectral["von_neumann_entropy"] = serde_json::json!({
                "entropy": entropy.entropy,
                "max_entropy": entropy.max_entropy,
                "normalized": entropy.normalized,
                "effective_rank": entropy.effective_rank,
            });
        }
    }

    // Centrality
    let mut centrality = serde_json::json!(null);
    if n >= 2 {
        let pr = pagerank(&graph, 20);
        let bc = betweenness_centrality(&graph);

        let mut pr_sorted: Vec<_> = pr.iter().collect();
        pr_sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_pagerank: Vec<serde_json::Value> = pr_sorted
            .iter()
            .take(5)
            .map(|(name, score)| serde_json::json!({"entity": name, "score": score}))
            .collect();

        let mut bc_sorted: Vec<_> = bc.iter().collect();
        bc_sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_betweenness: Vec<serde_json::Value> = bc_sorted
            .iter()
            .take(5)
            .map(|(name, score)| serde_json::json!({"entity": name, "score": score}))
            .collect();

        centrality = serde_json::json!({
            "pagerank_top5": top_pagerank,
            "betweenness_top5": top_betweenness,
        });
    }

    // Curvature
    let mut curvature = serde_json::json!(null);
    if n >= 2 && e > 0 {
        let curvatures = ricci_curvature_adaptive(&graph);
        let summary = ricci_summary(&curvatures);
        curvature = serde_json::json!({
            "mean": summary.mean_curvature,
            "min": summary.min_curvature,
            "max": summary.max_curvature,
            "positive_edges": summary.positive_edges,
            "negative_edges": summary.negative_edges,
            "bottleneck_edge": summary.bottleneck_edge,
            "tightest_edge": summary.tightest_edge,
        });
    }

    // Namespace distribution
    let mut ns_counts: BTreeMap<String, usize> = BTreeMap::new();
    for datom in store.datoms() {
        if datom.op == Op::Assert && datom.attribute.as_str() == ":spec/namespace" {
            if let Value::Keyword(ns) = &datom.value {
                *ns_counts.entry(ns.clone()).or_default() += 1;
            }
        }
    }

    let result = serde_json::json!({
        "store": path.display().to_string(),
        "datom_count": store.len(),
        "entity_count": store.entity_count(),
        "topology": {
            "nodes": n,
            "edges": e,
            "density": d,
            "scc_count": components.len(),
            "largest_scc": components.iter().max_by_key(|c| c.len()).map(|c| c.len()).unwrap_or(0),
            "beta_1": beta_1,
        },
        "coherence": {
            "phi": phi,
            "d_is": div_components.d_is,
            "d_sp": div_components.d_sp,
            "quadrant": quadrant,
        },
        "spectral": spectral,
        "centrality": centrality,
        "curvature": curvature,
        "namespaces": ns_counts,
    });

    Ok(serde_json::to_string_pretty(&result).unwrap() + "\n")
}

/// Run with a token budget — emit sections in priority order until exhausted.
///
/// Section priorities (highest first):
/// 1. Store summary (~30 tokens)
/// 2. Trilateral coherence (~40 tokens)
/// 3. Actions from guidance (~80 tokens)
/// 4. Graph topology (~50 tokens)
/// 5. Spectral: λ₂, Cheeger, entropy (~60 tokens)
/// 6. Centrality top 5 (~80 tokens)
/// 7. Curvature summary (~50 tokens)
/// 8. Persistent homology (~60 tokens)
/// 9. Namespace distribution (~120 tokens)
pub fn run_budget(path: &Path, budget: usize, force: bool) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let mut out = String::new();
    let mut tokens_used = 0_usize;

    // Helper: estimate tokens (chars/4 heuristic, ADR-BUDGET-004)
    let estimate_tokens = |s: &str| -> usize { s.len().div_ceil(4) };

    // Macro: emit a section if budget allows
    macro_rules! emit {
        ($section:expr) => {{
            let t = estimate_tokens(&$section);
            if tokens_used + t <= budget {
                out.push_str(&$section);
                tokens_used += t;
                true
            } else {
                false
            }
        }};
    }

    // Section 1: Store summary
    let s1 = format!(
        "analyze: {} datoms, {} entities\n",
        store.len(),
        store.entity_count(),
    );
    emit!(s1);

    // Section 2: Trilateral coherence (fast — skips O(n³) entropy)
    let coherence = braid_kernel::trilateral::check_coherence_fast(&store);
    let s2 = format!(
        "coherence: phi={:.1} beta1={} quadrant={:?} ISP_bypasses={}\n",
        coherence.phi, coherence.beta_1, coherence.quadrant, coherence.isp_bypasses,
    );
    if !emit!(s2) {
        return Ok(out);
    }

    // Section 3: Guidance actions
    let actions = braid_kernel::guidance::derive_actions(&store);
    let s3 = braid_kernel::guidance::format_actions(&actions);
    if !emit!(s3) {
        return Ok(out);
    }

    // Budget guard: skip expensive graph sections if budget nearly exhausted
    let budget_remaining = budget.saturating_sub(tokens_used);
    if budget_remaining < 50 {
        out.push_str(&format!(
            "(budget: {}/{} tokens used)\n",
            tokens_used, budget
        ));
        return Ok(out);
    }

    // Section 4+: Graph analytics (requires graph construction)
    let graph = build_entity_graph(&store);
    let n = graph.node_count();
    let components = scc(&graph);
    let beta_1 = first_betti_number(&graph);
    let s4 = format!(
        "topology: {} nodes, {} edges, density={:.6}, SCC={}, B1={}\n",
        n,
        graph.edge_count(),
        density(&graph),
        components.len(),
        beta_1,
    );
    if !emit!(s4) {
        out.push_str(&format!(
            "(budget: {}/{} tokens used)\n",
            tokens_used, budget
        ));
        return Ok(out);
    }

    // Section 5: Spectral summary (Lanczos-adaptive: O(k·E) for large graphs)
    if n >= 2 && budget.saturating_sub(tokens_used) >= 60 {
        let sd = spectral_decomposition_adaptive(&graph);
        let entropy = braid_kernel::trilateral::von_neumann_entropy(&store);
        if let Some(ref sd) = sd {
            let fiedler_result = fiedler_from_spectrum(sd);
            let s5 = format!(
                "spectral: lambda2={:.6} S_vN={:.3} effective_rank={}\n",
                fiedler_result.algebraic_connectivity, entropy.entropy, entropy.effective_rank,
            );
            if !emit!(s5) {
                out.push_str(&format!(
                    "(budget: {}/{} tokens used)\n",
                    tokens_used, budget
                ));
                return Ok(out);
            }

            // Section 5b: Cheeger
            if let Some(ch) = cheeger(&graph) {
                let s5b = format!(
                    "  Cheeger: h={:.6} [{:.4} <= h <= {:.4}]\n",
                    ch.cheeger_constant, ch.lower_bound, ch.upper_bound,
                );
                if !emit!(s5b) {
                    out.push_str(&format!(
                        "(budget: {}/{} tokens used)\n",
                        tokens_used, budget
                    ));
                    return Ok(out);
                }
            }

            // Section 5c: Kirchhoff
            let is_partial = sd.eigenvalues.len() < n;
            let ki = if is_partial {
                kirchhoff_from_partial_spectrum(&sd.eigenvalues, n)
            } else {
                kirchhoff_from_spectrum(sd)
            };
            let s5c = format!(
                "  Kirchhoff: {:.2} resistance={:.4}{}\n",
                ki,
                ki / (n * (n - 1)) as f64,
                if is_partial { " (approx)" } else { "" },
            );
            if !emit!(s5c) {
                out.push_str(&format!(
                    "(budget: {}/{} tokens used)\n",
                    tokens_used, budget
                ));
                return Ok(out);
            }
        }
    }

    // Section 6: Centrality top 5
    if n >= 2 && budget.saturating_sub(tokens_used) >= 80 {
        let pr = pagerank(&graph, 20);
        let mut pr_sorted: Vec<_> = pr.iter().collect();
        pr_sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut s6 = String::from("PageRank top 5:\n");
        for (name, score) in pr_sorted.iter().take(5) {
            s6.push_str(&format!("  {:.4}  {}\n", score, name));
        }
        if !emit!(s6) {
            out.push_str(&format!(
                "(budget: {}/{} tokens used)\n",
                tokens_used, budget
            ));
            return Ok(out);
        }
    }

    // Section 7: Curvature summary (landmark-adaptive: O(k·(V+E)) for large graphs)
    if n >= 2 && graph.edge_count() > 0 && budget.saturating_sub(tokens_used) >= 50 {
        let curvatures = ricci_curvature_adaptive(&graph);
        let summary = ricci_summary(&curvatures);
        let s7 = format!(
            "Ricci: mean={:.4} min={:.4} max={:.4} ({} pos, {} neg)\n",
            summary.mean_curvature,
            summary.min_curvature,
            summary.max_curvature,
            summary.positive_edges,
            summary.negative_edges,
        );
        if !emit!(s7) {
            out.push_str(&format!(
                "(budget: {}/{} tokens used)\n",
                tokens_used, budget
            ));
            return Ok(out);
        }
        if let Some((ref src, ref dst)) = summary.bottleneck_edge {
            let s7b = format!(
                "  bottleneck: {} → {} (k={:.4})\n",
                src, dst, summary.min_curvature
            );
            let _ = emit!(s7b);
        }
    }

    // Section 8: Persistent homology (very expensive — only with --force)
    if !force {
        // skip
    } else {
        let mut edges: Vec<(String, String)> = Vec::new();
        for datom in store.datoms() {
            if datom.op == Op::Assert {
                if let Value::Ref(target) = &datom.value {
                    edges.push((
                        resolve_label(&store, datom.entity),
                        resolve_label(&store, *target),
                    ));
                }
            }
        }
        let diagram = persistent_homology(&edges);
        let h0 = diagram.pairs.iter().filter(|p| p.dimension == 0).count();
        let h1 = diagram.pairs.iter().filter(|p| p.dimension == 1).count();
        let tp = total_persistence(&diagram);
        let s8 = format!("homology: H0={} H1={} total_persistence={}\n", h0, h1, tp);
        let _ = emit!(s8);
    }

    if !force {
        out.push_str(&format!(
            "(budget: {}/{} tokens used)\n",
            tokens_used, budget
        ));
    }

    Ok(out)
}
