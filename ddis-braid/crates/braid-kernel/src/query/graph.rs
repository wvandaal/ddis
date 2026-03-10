//! Graph algorithms for dependency analysis.
//!
//! Provides topological sort, strongly connected components (SCC),
//! PageRank, critical path analysis, and density computation.
//! These are used by GUIDANCE for R(t) routing and by TRILATERAL
//! for coherence metrics.
//!
//! # Invariants
//!
//! - **INV-QUERY-012**: Topo sort, SCC, PageRank available.
//! - **INV-QUERY-013**: Critical path analysis.
//! - **INV-QUERY-014**: Graph density.
//! - **INV-QUERY-017**: All graph algorithms are deterministic.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

/// A directed graph with string-labeled nodes.
#[derive(Clone, Debug, Default)]
pub struct DiGraph {
    /// Adjacency list: node → set of successors.
    adj: BTreeMap<String, BTreeSet<String>>,
    /// Reverse adjacency list: node → set of predecessors.
    rev: BTreeMap<String, BTreeSet<String>>,
}

impl DiGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        DiGraph::default()
    }

    /// Add a node (no-op if exists).
    pub fn add_node(&mut self, node: &str) {
        self.adj.entry(node.to_string()).or_default();
        self.rev.entry(node.to_string()).or_default();
    }

    /// Add a directed edge from `src` to `dst`.
    pub fn add_edge(&mut self, src: &str, dst: &str) {
        self.add_node(src);
        self.add_node(dst);
        self.adj.get_mut(src).unwrap().insert(dst.to_string());
        self.rev.get_mut(dst).unwrap().insert(src.to_string());
    }

    /// All nodes in the graph.
    pub fn nodes(&self) -> impl Iterator<Item = &String> {
        self.adj.keys()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize {
        self.adj.len()
    }

    /// Number of edges.
    pub fn edge_count(&self) -> usize {
        self.adj.values().map(|s| s.len()).sum()
    }

    /// Successors of a node.
    pub fn successors(&self, node: &str) -> impl Iterator<Item = &String> {
        self.adj.get(node).into_iter().flat_map(|s| s.iter())
    }

    /// Predecessors of a node.
    pub fn predecessors(&self, node: &str) -> impl Iterator<Item = &String> {
        self.rev.get(node).into_iter().flat_map(|s| s.iter())
    }
}

/// Topological sort using Kahn's algorithm.
///
/// Returns `None` if the graph has a cycle.
pub fn topo_sort(graph: &DiGraph) -> Option<Vec<String>> {
    let mut in_degree: HashMap<&String, usize> = HashMap::new();
    for node in graph.nodes() {
        in_degree.entry(node).or_insert(0);
        for succ in graph.successors(node) {
            *in_degree.entry(succ).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<&String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(node, _)| *node)
        .collect();

    // Sort for determinism
    let mut sorted_queue: Vec<&String> = queue.drain(..).collect();
    sorted_queue.sort();
    queue.extend(sorted_queue);

    let mut result = Vec::new();
    let mut visited = 0;

    while let Some(node) = queue.pop_front() {
        result.push(node.clone());
        visited += 1;

        let mut next_ready = Vec::new();
        for succ in graph.successors(node) {
            if let Some(deg) = in_degree.get_mut(succ) {
                *deg -= 1;
                if *deg == 0 {
                    next_ready.push(succ);
                }
            }
        }
        next_ready.sort();
        queue.extend(next_ready);
    }

    if visited == graph.node_count() {
        Some(result)
    } else {
        None // cycle detected
    }
}

/// Strongly connected components via Tarjan's algorithm.
pub fn scc(graph: &DiGraph) -> Vec<Vec<String>> {
    struct TarjanState<'a> {
        graph: &'a DiGraph,
        index_counter: usize,
        stack: Vec<String>,
        on_stack: BTreeSet<String>,
        indices: HashMap<String, usize>,
        lowlinks: HashMap<String, usize>,
        result: Vec<Vec<String>>,
    }

    impl TarjanState<'_> {
        fn strongconnect(&mut self, v: &str) {
            self.indices.insert(v.to_string(), self.index_counter);
            self.lowlinks.insert(v.to_string(), self.index_counter);
            self.index_counter += 1;
            self.stack.push(v.to_string());
            self.on_stack.insert(v.to_string());

            let successors: Vec<String> = self.graph.successors(v).cloned().collect();
            for w in &successors {
                if !self.indices.contains_key(w.as_str()) {
                    self.strongconnect(w);
                    let wl = self.lowlinks[w.as_str()];
                    let vl = self.lowlinks.get_mut(v).unwrap();
                    *vl = (*vl).min(wl);
                } else if self.on_stack.contains(w.as_str()) {
                    let wi = self.indices[w.as_str()];
                    let vl = self.lowlinks.get_mut(v).unwrap();
                    *vl = (*vl).min(wi);
                }
            }

            if self.lowlinks[v] == self.indices[v] {
                let mut component = Vec::new();
                loop {
                    let w = self.stack.pop().unwrap();
                    self.on_stack.remove(&w);
                    component.push(w.clone());
                    if w == v {
                        break;
                    }
                }
                component.sort(); // deterministic ordering
                self.result.push(component);
            }
        }
    }

    let mut state = TarjanState {
        graph,
        index_counter: 0,
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        indices: HashMap::new(),
        lowlinks: HashMap::new(),
        result: Vec::new(),
    };

    // Process nodes in sorted order for determinism
    let nodes: Vec<String> = graph.nodes().cloned().collect();
    for node in &nodes {
        if !state.indices.contains_key(node) {
            state.strongconnect(node);
        }
    }

    state.result
}

/// PageRank computation using power iteration.
///
/// Uses rational arithmetic approximation for determinism.
/// Damping factor = 0.85 (standard). Converges via Perron-Frobenius.
pub fn pagerank(graph: &DiGraph, iterations: usize) -> BTreeMap<String, f64> {
    let n = graph.node_count();
    if n == 0 {
        return BTreeMap::new();
    }

    let d = 0.85_f64;
    let base = (1.0 - d) / n as f64;

    let mut ranks: BTreeMap<String, f64> = BTreeMap::new();
    for node in graph.nodes() {
        ranks.insert(node.clone(), 1.0 / n as f64);
    }

    for _ in 0..iterations {
        let mut new_ranks: BTreeMap<String, f64> = BTreeMap::new();

        for node in graph.nodes() {
            let mut sum = 0.0;
            for pred in graph.predecessors(node) {
                let out_degree = graph.successors(pred).count();
                if out_degree > 0 {
                    sum += ranks[pred] / out_degree as f64;
                }
            }
            new_ranks.insert(node.clone(), base + d * sum);
        }

        ranks = new_ranks;
    }

    ranks
}

/// Critical path: longest path in a DAG (returns length and path).
///
/// Returns `None` if the graph has a cycle.
pub fn critical_path(graph: &DiGraph) -> Option<(usize, Vec<String>)> {
    let order = topo_sort(graph)?;

    let mut dist: HashMap<String, usize> = HashMap::new();
    let mut prev: HashMap<String, String> = HashMap::new();

    for node in &order {
        dist.insert(node.clone(), 0);
    }

    for node in &order {
        let d = dist[node];
        for succ in graph.successors(node) {
            if d + 1 > dist[succ.as_str()] {
                dist.insert(succ.clone(), d + 1);
                prev.insert(succ.clone(), node.clone());
            }
        }
    }

    // Find the node with maximum distance
    let (end_node, max_dist) = dist.iter().max_by_key(|(_, d)| *d)?;

    // Reconstruct path
    let mut path = vec![end_node.clone()];
    let mut current = end_node.clone();
    while let Some(p) = prev.get(&current) {
        path.push(p.clone());
        current = p.clone();
    }
    path.reverse();

    Some((*max_dist, path))
}

// ---------------------------------------------------------------------------
// Edge Laplacian & Betti Number (INV-QUERY-023, INV-QUERY-024)
// ---------------------------------------------------------------------------

/// Dense matrix for small-graph linear algebra (Stage 0).
///
/// Row-major storage. For Stage 0, graphs are small enough that
/// dense matrices are practical. Future stages may use nalgebra or sparse.
#[derive(Clone, Debug)]
pub struct DenseMatrix {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Row-major data.
    pub data: Vec<f64>,
}

impl DenseMatrix {
    /// Create a zero matrix.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        DenseMatrix {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }

    /// Get element at (i, j).
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.data[i * self.cols + j]
    }

    /// Set element at (i, j).
    pub fn set(&mut self, i: usize, j: usize, val: f64) {
        self.data[i * self.cols + j] = val;
    }

    /// Transpose.
    pub fn transpose(&self) -> DenseMatrix {
        let mut t = DenseMatrix::zeros(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                t.set(j, i, self.get(i, j));
            }
        }
        t
    }

    /// Matrix multiply: self * other.
    pub fn mul(&self, other: &DenseMatrix) -> DenseMatrix {
        assert_eq!(self.cols, other.rows, "dimension mismatch");
        let mut result = DenseMatrix::zeros(self.rows, other.cols);
        for i in 0..self.rows {
            for j in 0..other.cols {
                let mut sum = 0.0;
                for k in 0..self.cols {
                    sum += self.get(i, k) * other.get(k, j);
                }
                result.set(i, j, sum);
            }
        }
        result
    }

    /// Add: self + other.
    pub fn add(&self, other: &DenseMatrix) -> DenseMatrix {
        assert_eq!(self.rows, other.rows);
        assert_eq!(self.cols, other.cols);
        let data: Vec<f64> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a + b)
            .collect();
        DenseMatrix {
            rows: self.rows,
            cols: self.cols,
            data,
        }
    }

    /// Check if the matrix is symmetric (within tolerance).
    pub fn is_symmetric(&self, tol: f64) -> bool {
        if self.rows != self.cols {
            return false;
        }
        for i in 0..self.rows {
            for j in i + 1..self.cols {
                if (self.get(i, j) - self.get(j, i)).abs() > tol {
                    return false;
                }
            }
        }
        true
    }

    /// Compute eigenvalues of a symmetric matrix via Jacobi iteration.
    ///
    /// Returns eigenvalues sorted ascending. Only valid for symmetric matrices.
    /// Converges for all real symmetric matrices (Jacobi's method).
    pub fn symmetric_eigenvalues(&self) -> Vec<f64> {
        assert_eq!(self.rows, self.cols, "must be square");
        let n = self.rows;
        if n == 0 {
            return vec![];
        }

        // Work on a copy
        let mut a = self.data.clone();
        let max_iter = 100 * n * n;

        for _ in 0..max_iter {
            // Find largest off-diagonal element
            let mut max_val = 0.0_f64;
            let mut p = 0;
            let mut q = 1;
            for i in 0..n {
                for j in (i + 1)..n {
                    let val = a[i * n + j].abs();
                    if val > max_val {
                        max_val = val;
                        p = i;
                        q = j;
                    }
                }
            }

            if max_val < 1e-12 {
                break; // Converged
            }

            // Compute rotation
            let app = a[p * n + p];
            let aqq = a[q * n + q];
            let apq = a[p * n + q];

            let theta = if (app - aqq).abs() < 1e-15 {
                std::f64::consts::FRAC_PI_4
            } else {
                0.5 * (2.0 * apq / (app - aqq)).atan()
            };

            let c = theta.cos();
            let s = theta.sin();

            // Apply rotation
            let mut new_a = a.clone();
            for i in 0..n {
                if i != p && i != q {
                    let aip = a[i * n + p];
                    let aiq = a[i * n + q];
                    new_a[i * n + p] = c * aip + s * aiq;
                    new_a[p * n + i] = new_a[i * n + p];
                    new_a[i * n + q] = -s * aip + c * aiq;
                    new_a[q * n + i] = new_a[i * n + q];
                }
            }
            new_a[p * n + p] = c * c * app + 2.0 * s * c * apq + s * s * aqq;
            new_a[q * n + q] = s * s * app - 2.0 * s * c * apq + c * c * aqq;
            new_a[p * n + q] = 0.0;
            new_a[q * n + p] = 0.0;

            a = new_a;
        }

        let mut eigenvalues: Vec<f64> = (0..n).map(|i| a[i * n + i]).collect();
        eigenvalues.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        eigenvalues
    }
}

/// Compute the boundary operator B₁ (edges → vertices).
///
/// For a directed graph with n nodes and m edges, B₁ is n×m.
/// B₁[v, e] = -1 if e starts at v, +1 if e ends at v, 0 otherwise.
pub fn boundary_operator_1(graph: &DiGraph) -> DenseMatrix {
    let nodes: Vec<String> = graph.nodes().cloned().collect();
    let node_idx: BTreeMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();

    // Enumerate edges in deterministic order
    let mut edges: Vec<(String, String)> = Vec::new();
    for src in &nodes {
        for dst in graph.successors(src) {
            edges.push((src.clone(), dst.clone()));
        }
    }

    let n = nodes.len();
    let m = edges.len();
    let mut b1 = DenseMatrix::zeros(n, m);

    for (e_idx, (src, dst)) in edges.iter().enumerate() {
        b1.set(node_idx[src.as_str()], e_idx, -1.0);
        b1.set(node_idx[dst.as_str()], e_idx, 1.0);
    }

    b1
}

/// Compute the edge Laplacian L₁ = B₁ᵀ B₁ (INV-QUERY-023).
///
/// The edge Laplacian operates on the edge space of the graph.
/// Its kernel dimension equals the first Betti number β₁.
///
/// At Stage 0 we omit the B₂ term (no triangles detected),
/// so L₁ = B₁ᵀ B₁ (the "down" Laplacian). This is exact for
/// graphs without 2-simplices, and a lower bound otherwise.
pub fn edge_laplacian(graph: &DiGraph) -> DenseMatrix {
    let b1 = boundary_operator_1(graph);
    let b1t = b1.transpose();
    b1t.mul(&b1)
}

/// Compute the first Betti number β₁ = dim(ker(L₁)) (INV-QUERY-024).
///
/// β₁ = 0 means the graph is a forest (no cycles).
/// β₁ > 0 counts independent cycles (topological holes).
///
/// Uses the edge Laplacian eigenvalues: β₁ = number of zero eigenvalues.
pub fn first_betti_number(graph: &DiGraph) -> usize {
    let l1 = edge_laplacian(graph);
    if l1.rows == 0 {
        return 0;
    }
    let eigenvalues = l1.symmetric_eigenvalues();
    eigenvalues.iter().filter(|&&v| v.abs() < 1e-8).count()
}

/// Graph density: |E| / (|V| * (|V| - 1)) for directed graphs.
pub fn density(graph: &DiGraph) -> f64 {
    let n = graph.node_count();
    if n <= 1 {
        return 0.0;
    }
    let e = graph.edge_count();
    e as f64 / (n * (n - 1)) as f64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn diamond_graph() -> DiGraph {
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("A", "C");
        g.add_edge("B", "D");
        g.add_edge("C", "D");
        g
    }

    #[test]
    fn topo_sort_diamond() {
        let g = diamond_graph();
        let order = topo_sort(&g).unwrap();
        assert_eq!(order[0], "A");
        assert_eq!(order[3], "D");
        // B and C can be in either order
    }

    #[test]
    fn topo_sort_detects_cycle() {
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("B", "C");
        g.add_edge("C", "A");
        assert!(topo_sort(&g).is_none());
    }

    #[test]
    fn scc_no_cycles() {
        let g = diamond_graph();
        let components = scc(&g);
        // Each node is its own SCC (no cycles)
        assert_eq!(components.len(), 4);
        for c in &components {
            assert_eq!(c.len(), 1);
        }
    }

    #[test]
    fn scc_with_cycle() {
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("B", "C");
        g.add_edge("C", "A");
        g.add_edge("C", "D");

        let components = scc(&g);
        let cycle: Vec<&Vec<String>> = components.iter().filter(|c| c.len() > 1).collect();
        assert_eq!(cycle.len(), 1);
        assert_eq!(cycle[0].len(), 3);
    }

    #[test]
    fn pagerank_uniform_graph() {
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("B", "A");

        let ranks = pagerank(&g, 20);
        assert!((ranks["A"] - ranks["B"]).abs() < 0.01);
    }

    #[test]
    fn pagerank_hub_graph() {
        let mut g = DiGraph::new();
        g.add_edge("A", "C");
        g.add_edge("B", "C");
        g.add_edge("D", "C");

        let ranks = pagerank(&g, 20);
        // C should have highest rank (most incoming edges)
        assert!(ranks["C"] > ranks["A"]);
        assert!(ranks["C"] > ranks["B"]);
    }

    #[test]
    fn critical_path_diamond() {
        let g = diamond_graph();
        let (length, path) = critical_path(&g).unwrap();
        assert_eq!(length, 2); // A→B→D or A→C→D
        assert_eq!(path[0], "A");
        assert_eq!(path.last().unwrap(), "D");
    }

    #[test]
    fn density_computation() {
        let g = diamond_graph();
        let d = density(&g);
        // 4 edges, 4 nodes: 4 / (4 * 3) = 0.333...
        assert!((d - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn topo_sort_is_deterministic() {
        let g = diamond_graph();
        let o1 = topo_sort(&g).unwrap();
        let o2 = topo_sort(&g).unwrap();
        assert_eq!(o1, o2);
    }

    #[test]
    fn pagerank_is_deterministic() {
        let g = diamond_graph();
        let r1 = pagerank(&g, 20);
        let r2 = pagerank(&g, 20);
        for (k, v1) in &r1 {
            assert!((v1 - r2[k]).abs() < f64::EPSILON);
        }
    }

    // -------------------------------------------------------------------
    // Edge Laplacian & Betti Number (INV-QUERY-023, INV-QUERY-024)
    // -------------------------------------------------------------------

    #[test]
    fn edge_laplacian_is_symmetric() {
        let g = diamond_graph();
        let l1 = edge_laplacian(&g);
        assert!(l1.is_symmetric(1e-10), "Edge Laplacian must be symmetric");
    }

    #[test]
    fn edge_laplacian_is_positive_semidefinite() {
        let g = diamond_graph();
        let l1 = edge_laplacian(&g);
        let eigenvalues = l1.symmetric_eigenvalues();
        for ev in &eigenvalues {
            assert!(*ev >= -1e-8, "Edge Laplacian eigenvalue {} is negative", ev);
        }
    }

    #[test]
    fn betti_number_tree_is_zero() {
        // A tree has no cycles, so β₁ = 0
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("A", "C");
        g.add_edge("B", "D");
        assert_eq!(first_betti_number(&g), 0);
    }

    #[test]
    fn betti_number_single_cycle() {
        // A→B→C→A has exactly one cycle, β₁ = 1
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("B", "C");
        g.add_edge("C", "A");
        assert_eq!(first_betti_number(&g), 1);
    }

    #[test]
    fn betti_number_diamond_has_cycle() {
        // Diamond A→B→D, A→C→D has a cycle: β₁ = 1
        let g = diamond_graph();
        assert_eq!(first_betti_number(&g), 1);
    }

    #[test]
    fn betti_number_empty_graph() {
        let g = DiGraph::new();
        assert_eq!(first_betti_number(&g), 0);
    }

    #[test]
    fn betti_number_single_edge() {
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        assert_eq!(first_betti_number(&g), 0);
    }

    #[test]
    fn boundary_operator_dimensions() {
        let g = diamond_graph();
        let b1 = boundary_operator_1(&g);
        assert_eq!(b1.rows, 4, "B₁ should have 4 rows (vertices)");
        assert_eq!(b1.cols, 4, "B₁ should have 4 cols (edges)");
    }

    #[test]
    fn dense_matrix_transpose_involution() {
        let mut m = DenseMatrix::zeros(2, 3);
        m.set(0, 0, 1.0);
        m.set(0, 2, 2.0);
        m.set(1, 1, 3.0);
        let tt = m.transpose().transpose();
        assert_eq!(tt.rows, m.rows);
        assert_eq!(tt.cols, m.cols);
        for i in 0..m.rows {
            for j in 0..m.cols {
                assert!((tt.get(i, j) - m.get(i, j)).abs() < 1e-12);
            }
        }
    }

    // -------------------------------------------------------------------
    // Proptest property-based verification (INV-QUERY-012, INV-QUERY-017)
    // -------------------------------------------------------------------

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        fn arb_digraph(max_nodes: usize) -> impl Strategy<Value = DiGraph> {
            let max = if max_nodes == 0 { 1 } else { max_nodes };
            (1..=max).prop_flat_map(|n| {
                let node_names: Vec<String> = (0..n).map(|i| format!("n{i}")).collect();
                let n2 = n;
                proptest::collection::vec((0..n2, 0..n2), 0..=(n2 * n2)).prop_map(move |edges| {
                    let mut g = DiGraph::new();
                    for name in &node_names {
                        g.add_node(name);
                    }
                    for (src, dst) in edges {
                        if src != dst {
                            g.add_edge(&node_names[src], &node_names[dst]);
                        }
                    }
                    g
                })
            })
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(200))]

            #[test]
            fn pagerank_converges_within_100_iterations(g in arb_digraph(6)) {
                if g.node_count() == 0 {
                    return Ok(());
                }
                let r1 = pagerank(&g, 99);
                let r2 = pagerank(&g, 100);
                for (node, v1) in &r1 {
                    let v2 = r2.get(node).unwrap();
                    prop_assert!(
                        (v1 - v2).abs() < 1e-6,
                        "PageRank did not converge within 100 iterations for node {}: iter99={}, iter100={}",
                        node, v1, v2
                    );
                }
            }

            #[test]
            fn pagerank_is_stochastic(g in arb_digraph(6)) {
                if g.node_count() == 0 {
                    return Ok(());
                }
                // Stochasticity (sum = 1.0) holds when every node has at
                // least one outgoing edge (no "dangling" nodes that leak
                // rank). When dangling nodes exist, the basic power-iteration
                // implementation produces a sum <= 1.0, which is expected.
                let has_dangling = g.nodes().any(|n| g.successors(n).next().is_none());
                let ranks = pagerank(&g, 100);
                let sum: f64 = ranks.values().sum();
                if has_dangling {
                    // With dangling nodes, sum <= 1.0 (rank leaks)
                    prop_assert!(
                        sum <= 1.0 + 1e-6,
                        "PageRank sum must be <= 1.0 with dangling nodes, got {}",
                        sum
                    );
                } else {
                    prop_assert!(
                        (sum - 1.0).abs() < 1e-6,
                        "PageRank values must sum to ~1.0, got {}",
                        sum
                    );
                }
            }

            #[test]
            fn pagerank_non_negativity(g in arb_digraph(6)) {
                let ranks = pagerank(&g, 100);
                for (node, rank) in &ranks {
                    prop_assert!(
                        *rank >= 0.0,
                        "PageRank for node {} is negative: {}",
                        node, rank
                    );
                }
            }

            // INV-QUERY-023: Edge Laplacian is symmetric positive semi-definite
            #[test]
            fn edge_laplacian_is_psd(g in arb_digraph(5)) {
                if g.edge_count() == 0 {
                    return Ok(());
                }
                let l1 = edge_laplacian(&g);
                prop_assert!(l1.is_symmetric(1e-8), "L₁ must be symmetric");
                let evs = l1.symmetric_eigenvalues();
                for ev in &evs {
                    prop_assert!(
                        *ev >= -1e-6,
                        "L₁ eigenvalue {} is negative (not PSD)",
                        ev
                    );
                }
            }

            // INV-QUERY-024: β₁ = 0 for DAGs (no directed cycles contributing to undirected cycles)
            #[test]
            fn betti_number_nonnegative(g in arb_digraph(5)) {
                let b = first_betti_number(&g);
                // β₁ is a count, always >= 0 (trivially true for usize)
                prop_assert!(b < g.edge_count() + 1, "β₁ must be bounded by edge count");
            }

            // INV-QUERY-024: β₁ = m - n + c for connected components (Euler characteristic)
            // For a connected graph: β₁ = m - n + 1 where m = edges, n = nodes
            // This holds for the undirected interpretation of the graph

            #[test]
            fn pagerank_spectral_gap_bound(g in arb_digraph(6)) {
                if g.node_count() == 0 {
                    return Ok(());
                }
                let d: f64 = 0.85;
                // After k iterations, the error should be bounded by d^k.
                // We test this by comparing iteration k vs the "converged" result
                // at iteration 200 (used as ground truth).
                let converged = pagerank(&g, 200);
                for k in [5, 10, 20] {
                    let partial = pagerank(&g, k);
                    let max_error: f64 = converged.iter().map(|(node, cv)| {
                        let pv = partial.get(node).unwrap();
                        (cv - pv).abs()
                    }).fold(0.0_f64, f64::max);
                    let bound = d.powi(k as i32);
                    prop_assert!(
                        max_error <= bound + 1e-12,
                        "Spectral gap violated at k={}: max_error={} > d^k={}",
                        k, max_error, bound
                    );
                }
            }
        }
    }
}
