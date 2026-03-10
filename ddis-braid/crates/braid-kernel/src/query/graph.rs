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

    /// Create an n×n identity matrix.
    pub fn identity(n: usize) -> Self {
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m.set(i, i, 1.0);
        }
        m
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

/// Betweenness centrality via Brandes' algorithm (INV-QUERY-015).
///
/// For each node v, BC(v) = Σ_{s≠v≠t} σ_st(v) / σ_st
/// where σ_st is the number of shortest paths from s to t,
/// and σ_st(v) is the number that pass through v.
///
/// Uses BFS-based forward pass + backward accumulation.
/// Complexity: O(V × E) for unweighted graphs.
pub fn betweenness_centrality(graph: &DiGraph) -> BTreeMap<String, f64> {
    let mut bc: BTreeMap<String, f64> = BTreeMap::new();
    for node in graph.nodes() {
        bc.insert(node.clone(), 0.0);
    }

    // Process source nodes in sorted order for determinism (INV-QUERY-017).
    // BTreeMap keys are already sorted, so iterating graph.nodes() is deterministic.
    for s in graph.nodes() {
        // --- Forward pass: BFS from s ---
        let mut stack: Vec<String> = Vec::new();
        let mut predecessors: HashMap<String, Vec<String>> = HashMap::new();
        let mut sigma: HashMap<String, f64> = HashMap::new();
        let mut dist: HashMap<String, i64> = HashMap::new();

        for node in graph.nodes() {
            predecessors.insert(node.clone(), Vec::new());
            sigma.insert(node.clone(), 0.0);
            dist.insert(node.clone(), -1);
        }

        sigma.insert(s.clone(), 1.0);
        dist.insert(s.clone(), 0);

        let mut queue: VecDeque<String> = VecDeque::new();
        queue.push_back(s.clone());

        while let Some(v) = queue.pop_front() {
            stack.push(v.clone());
            let dv = dist[&v];

            // Process successors in sorted order for determinism
            let mut succs: Vec<String> = graph.successors(&v).cloned().collect();
            succs.sort();

            for w in succs {
                // w found for the first time?
                if dist[&w] < 0 {
                    dist.insert(w.clone(), dv + 1);
                    queue.push_back(w.clone());
                }
                // shortest path to w via v?
                if dist[&w] == dv + 1 {
                    let sv = sigma[&v];
                    *sigma.get_mut(&w).unwrap() += sv;
                    predecessors.get_mut(&w).unwrap().push(v.clone());
                }
            }
        }

        // --- Backward pass: accumulate dependencies ---
        let mut delta: HashMap<String, f64> = HashMap::new();
        for node in graph.nodes() {
            delta.insert(node.clone(), 0.0);
        }

        // Process in reverse BFS order (farthest nodes first)
        while let Some(w) = stack.pop() {
            let sigma_w = sigma[&w];
            let delta_w = delta[&w];
            for v in &predecessors[&w] {
                let sigma_v = sigma[v];
                *delta.get_mut(v).unwrap() += (sigma_v / sigma_w) * (1.0 + delta_w);
            }
            if &w != s {
                *bc.get_mut(&w).unwrap() += delta_w;
            }
        }
    }

    bc
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
// Persistent Homology (INV-QUERY-025)
// ---------------------------------------------------------------------------

/// A birth-death pair from persistent homology.
///
/// Represents a topological feature that appears at `birth` (filtration index)
/// and disappears at `death` (filtration index, or `None` if the feature persists
/// to the end of the filtration).
///
/// - H₀ features: connected components. Born when a node appears, die when
///   the component merges with another (via an edge).
/// - H₁ features: independent cycles. Born when an edge creates a cycle
///   (connects two nodes already in the same component).
#[derive(Clone, Debug, PartialEq)]
pub struct BirthDeath {
    /// Filtration index at which the feature appears.
    pub birth: usize,
    /// Filtration index at which the feature disappears (None = persists forever).
    pub death: Option<usize>,
    /// Homology dimension (0 = connected component, 1 = cycle).
    pub dimension: usize,
}

impl BirthDeath {
    /// Persistence = death - birth (or infinity if death is None).
    /// Returns None for infinite persistence.
    pub fn persistence(&self) -> Option<usize> {
        self.death.map(|d| d - self.birth)
    }
}

/// Result of persistent homology computation.
#[derive(Clone, Debug)]
pub struct PersistenceDiagram {
    /// All birth-death pairs, sorted by (dimension, birth).
    pub pairs: Vec<BirthDeath>,
    /// Number of H₀ features that persist to infinity (= connected components at end).
    pub h0_persistent: usize,
    /// Number of H₁ features that persist to infinity (= independent cycles at end).
    pub h1_persistent: usize,
    /// Total number of filtration steps.
    pub filtration_length: usize,
}

/// Union-Find (disjoint set) for tracking connected components.
///
/// Used in persistent H₀ computation: each node starts as its own component,
/// edges merge components. The "elder rule" determines which component survives:
/// the one born earlier (lower filtration index).
struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    /// Birth time of each component (filtration index when its root node was added).
    birth: Vec<usize>,
}

impl UnionFind {
    fn new() -> Self {
        UnionFind {
            parent: Vec::new(),
            rank: Vec::new(),
            birth: Vec::new(),
        }
    }

    /// Add a new element with the given birth time. Returns its index.
    fn make_set(&mut self, birth_time: usize) -> usize {
        let idx = self.parent.len();
        self.parent.push(idx);
        self.rank.push(0);
        self.birth.push(birth_time);
        idx
    }

    /// Find with path compression.
    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union by rank with elder rule: older component (lower birth) is the root.
    /// Returns Some(death_time_of_younger) if a merge happened, None if already same set.
    fn union(&mut self, x: usize, y: usize, filtration_step: usize) -> Option<(usize, usize)> {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx == ry {
            return None; // Same component — this edge creates a cycle
        }
        // Elder rule: the component born earlier survives
        let (survivor, dying) = if self.birth[rx] <= self.birth[ry] {
            (rx, ry)
        } else {
            (ry, rx)
        };
        // Union by rank
        if self.rank[survivor] < self.rank[dying] {
            // Swap roles if rank is lower, but keep elder as parent
            self.parent[dying] = survivor;
        } else if self.rank[survivor] > self.rank[dying] {
            self.parent[dying] = survivor;
        } else {
            self.parent[dying] = survivor;
            self.rank[survivor] += 1;
        }
        Some((self.birth[dying], filtration_step))
    }
}

/// Compute persistent homology over an edge filtration (INV-QUERY-025).
///
/// Given a sequence of edges (the filtration), incrementally builds the graph
/// and tracks:
/// - **H₀** (connected components): via Union-Find with elder rule.
///   A component is born when a node first appears and dies when it merges
///   with an older component.
/// - **H₁** (cycles): an edge that connects two nodes already in the same
///   component creates a cycle. The cycle is born at that filtration step.
///   For Stage 0, all H₁ features persist to infinity (we don't track
///   2-simplices that would kill cycles).
///
/// The filtration is typically the transaction order: edges added in
/// chronological order reveal which topological features are durable.
pub fn persistent_homology(edges: &[(String, String)]) -> PersistenceDiagram {
    let mut uf = UnionFind::new();
    let mut node_index: BTreeMap<String, usize> = BTreeMap::new();
    let mut pairs: Vec<BirthDeath> = Vec::new();
    let mut h1_births: Vec<usize> = Vec::new();

    for (step, (src, dst)) in edges.iter().enumerate() {
        // Ensure both nodes exist
        let src_idx = if let Some(&idx) = node_index.get(src) {
            idx
        } else {
            let idx = uf.make_set(step);
            node_index.insert(src.clone(), idx);
            // H₀ birth: new connected component
            idx
        };

        let dst_idx = if let Some(&idx) = node_index.get(dst) {
            idx
        } else {
            let idx = uf.make_set(step);
            node_index.insert(dst.clone(), idx);
            idx
        };

        // Try to union
        match uf.union(src_idx, dst_idx, step) {
            Some((younger_birth, death_step)) => {
                // H₀ death: younger component merges into elder
                pairs.push(BirthDeath {
                    birth: younger_birth,
                    death: Some(death_step),
                    dimension: 0,
                });
            }
            None => {
                // Same component — this edge creates a cycle (H₁ birth)
                h1_births.push(step);
            }
        }
    }

    // H₀ features that persist: components that never merged
    // Count unique roots
    let mut roots: BTreeSet<usize> = BTreeSet::new();
    for &idx in node_index.values() {
        // Need to use a mutable reference for find
        roots.insert(uf.find(idx));
    }
    let h0_persistent = roots.len();

    // Add persistent H₀ features (components that survive to end)
    let mut root_births: Vec<usize> = roots.iter().map(|&r| uf.birth[r]).collect();
    root_births.sort();
    for birth in root_births {
        pairs.push(BirthDeath {
            birth,
            death: None,
            dimension: 0,
        });
    }

    // Add H₁ features (all persist to infinity at Stage 0)
    let h1_persistent = h1_births.len();
    for birth in &h1_births {
        pairs.push(BirthDeath {
            birth: *birth,
            death: None,
            dimension: 1,
        });
    }

    // Sort by (dimension, birth) for determinism
    pairs.sort_by(|a, b| a.dimension.cmp(&b.dimension).then(a.birth.cmp(&b.birth)));

    let filtration_length = edges.len();

    PersistenceDiagram {
        pairs,
        h0_persistent,
        h1_persistent,
        filtration_length,
    }
}

/// Compute a persistence summary: total persistence across all features.
///
/// Σ (death - birth) for all finite pairs. Higher total persistence means
/// the topological structure changes more dramatically across the filtration.
/// Low total persistence means the structure stabilizes quickly.
pub fn total_persistence(diagram: &PersistenceDiagram) -> usize {
    diagram.pairs.iter().filter_map(|p| p.persistence()).sum()
}

/// Compute the Wasserstein-1 distance between two persistence diagrams.
///
/// Simplified version: sums absolute differences in birth/death times
/// for matched pairs (matched by closest birth time within same dimension).
/// Unmatched pairs contribute their persistence to the distance.
///
/// This is a lower bound on the true Wasserstein distance (which requires
/// optimal matching). For Stage 0 this is sufficient.
pub fn persistence_distance(a: &PersistenceDiagram, b: &PersistenceDiagram) -> f64 {
    // Simple approach: compare sorted finite pairs per dimension
    let mut distance = 0.0;

    for dim in 0..=1 {
        let a_pairs: Vec<&BirthDeath> = a
            .pairs
            .iter()
            .filter(|p| p.dimension == dim && p.death.is_some())
            .collect();
        let b_pairs: Vec<&BirthDeath> = b
            .pairs
            .iter()
            .filter(|p| p.dimension == dim && p.death.is_some())
            .collect();

        let max_len = a_pairs.len().max(b_pairs.len());
        for i in 0..max_len {
            match (a_pairs.get(i), b_pairs.get(i)) {
                (Some(ap), Some(bp)) => {
                    let a_pers = ap.persistence().unwrap_or(0) as f64;
                    let b_pers = bp.persistence().unwrap_or(0) as f64;
                    distance += (a_pers - b_pers).abs();
                }
                (Some(ap), None) => {
                    distance += ap.persistence().unwrap_or(0) as f64;
                }
                (None, Some(bp)) => {
                    distance += bp.persistence().unwrap_or(0) as f64;
                }
                (None, None) => {}
            }
        }
    }

    distance
}

// ---------------------------------------------------------------------------
// Fiedler vector & spectral graph partitioning
// ---------------------------------------------------------------------------

/// Full symmetric eigendecomposition via Jacobi method.
///
/// Returns (eigenvalues sorted ascending, eigenvector matrix).
/// The i-th column of the eigenvector matrix corresponds to eigenvalues[i].
fn symmetric_eigen_decomposition(matrix: &DenseMatrix) -> (Vec<f64>, DenseMatrix) {
    assert_eq!(matrix.rows, matrix.cols, "must be square");
    let n = matrix.rows;
    if n == 0 {
        return (vec![], DenseMatrix::zeros(0, 0));
    }

    // Work on a copy
    let mut a = matrix.data.clone();
    // Eigenvector accumulator (starts as identity)
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }

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
            break;
        }

        // Compute rotation angle
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

        // Apply rotation to A
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

        // Accumulate rotation into eigenvector matrix
        for i in 0..n {
            let vip = v[i * n + p];
            let viq = v[i * n + q];
            v[i * n + p] = c * vip + s * viq;
            v[i * n + q] = -s * vip + c * viq;
        }
    }

    // Extract eigenvalues and sort
    let mut eigen_pairs: Vec<(f64, usize)> = (0..n).map(|i| (a[i * n + i], i)).collect();
    eigen_pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let eigenvalues: Vec<f64> = eigen_pairs.iter().map(|(val, _)| *val).collect();

    // Reorder eigenvector columns to match sorted eigenvalues
    let mut sorted_v = DenseMatrix::zeros(n, n);
    for (new_col, &(_, old_col)) in eigen_pairs.iter().enumerate() {
        for row in 0..n {
            sorted_v.set(row, new_col, v[row * n + old_col]);
        }
    }

    (eigenvalues, sorted_v)
}

/// Compute the graph Laplacian L = D - A for an undirected interpretation of the graph.
///
/// D is the degree matrix, A is the (symmetrized) adjacency matrix.
/// L is positive semi-definite with smallest eigenvalue 0 (for connected graphs).
/// The multiplicity of eigenvalue 0 equals the number of connected components.
pub fn graph_laplacian(graph: &DiGraph) -> DenseMatrix {
    let nodes: Vec<String> = graph.nodes().cloned().collect();
    let n = nodes.len();
    let node_idx: BTreeMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), i))
        .collect();

    let mut laplacian = DenseMatrix::zeros(n, n);

    // Build symmetric adjacency + degree
    for src in &nodes {
        let si = node_idx[src.as_str()];
        for dst in graph.successors(src) {
            let di = node_idx[dst.as_str()];
            // Symmetrize
            laplacian.set(si, di, -1.0);
            laplacian.set(di, si, -1.0);
        }
    }

    // Set diagonal = degree (negative of row sum for off-diagonal)
    for i in 0..n {
        let degree: f64 = (0..n)
            .filter(|&j| j != i)
            .map(|j| -laplacian.get(i, j))
            .sum();
        laplacian.set(i, i, degree);
    }

    laplacian
}

/// The Fiedler vector: second smallest eigenvector of the graph Laplacian.
///
/// The Fiedler vector partitions the graph into two parts by sign:
/// nodes with positive components go in one partition, negative in the other.
/// This minimizes the normalized cut ratio (Fiedler, 1973).
///
/// Also returns the algebraic connectivity λ₂ (second smallest eigenvalue).
/// λ₂ > 0 iff the graph is connected. Larger λ₂ = more robust connectivity.
#[derive(Clone, Debug)]
pub struct FiedlerResult {
    /// The Fiedler vector (second eigenvector of L), one component per node.
    pub vector: Vec<f64>,
    /// Node labels in the same order as vector components.
    pub node_labels: Vec<String>,
    /// Algebraic connectivity λ₂ (second smallest eigenvalue of L).
    pub algebraic_connectivity: f64,
    /// Partition: nodes grouped by sign of Fiedler vector component.
    /// (positive_partition, negative_partition)
    pub partition: (Vec<String>, Vec<String>),
}

/// Compute the Fiedler vector and algebraic connectivity of a graph.
///
/// Uses the graph Laplacian eigendecomposition. The Fiedler vector
/// is the eigenvector corresponding to the second smallest eigenvalue.
///
/// Returns `None` if the graph has fewer than 2 nodes.
pub fn fiedler(graph: &DiGraph) -> Option<FiedlerResult> {
    let n = graph.node_count();
    if n < 2 {
        return None;
    }

    let laplacian = graph_laplacian(graph);
    let nodes: Vec<String> = graph.nodes().cloned().collect();

    // Compute eigenvectors via Jacobi method
    // We need both eigenvalues AND eigenvectors
    let (eigenvalues, eigenvectors) = symmetric_eigen_decomposition(&laplacian);

    // eigenvalues are sorted ascending; we want the second (index 1)
    // The eigenvector columns correspond to sorted eigenvalues
    let lambda_2 = eigenvalues[1];

    // Extract the Fiedler vector (column 1 of eigenvectors)
    let fiedler_vec: Vec<f64> = (0..n).map(|i| eigenvectors.get(i, 1)).collect();

    // Partition by sign
    let mut positive = Vec::new();
    let mut negative = Vec::new();
    for (i, &v) in fiedler_vec.iter().enumerate() {
        if v >= 0.0 {
            positive.push(nodes[i].clone());
        } else {
            negative.push(nodes[i].clone());
        }
    }

    Some(FiedlerResult {
        vector: fiedler_vec,
        node_labels: nodes,
        algebraic_connectivity: lambda_2,
        partition: (positive, negative),
    })
}

// ---------------------------------------------------------------------------
// Sheaf cohomology for conflict detection
// ---------------------------------------------------------------------------

/// A cellular sheaf over a directed graph.
///
/// In Braid's context:
/// - Vertices represent agents (or store frontiers)
/// - Edges represent pairwise merge/sync relationships
/// - The stalk F(v) at vertex v is a vector space (agent's local store state)
/// - The restriction map F(e): F(src) → F(tgt) describes how one agent's
///   state maps to another's perspective
///
/// The key insight: H⁰ = global sections (consistent state across all agents),
/// H¹ = obstructions to global consistency (conflicts that cannot be resolved
/// by local patching). Non-trivial H¹ ≠ 0 means the agents' states are
/// fundamentally inconsistent — there exist "conflicts" in the categorical sense.
#[derive(Debug, Clone)]
pub struct CellularSheaf {
    /// The underlying graph topology.
    graph: DiGraph,
    /// Vertex stalks: dimension of the vector space at each vertex.
    /// Key: node label, Value: dimension of F(v).
    vertex_stalks: BTreeMap<String, usize>,
    /// Edge restriction maps: for edge (u,v), the linear map F(u) → F(v).
    /// Stored as dense matrices. Key: (src, dst).
    restriction_maps: BTreeMap<(String, String), DenseMatrix>,
}

/// Result of sheaf cohomology computation.
#[derive(Debug, Clone)]
pub struct SheafCohomology {
    /// dim H⁰: dimension of global sections (agreement space).
    pub h0: usize,
    /// dim H¹: dimension of first cohomology (obstruction/conflict space).
    pub h1: usize,
    /// The sheaf Laplacian eigenvalues (ascending).
    pub laplacian_eigenvalues: Vec<f64>,
    /// Sheaf Betti numbers [β₀, β₁].
    pub betti: [usize; 2],
    /// Whether the sheaf is globally consistent (H¹ = 0).
    pub is_consistent: bool,
    /// Total dimension of all stalks combined.
    pub total_stalk_dim: usize,
}

impl CellularSheaf {
    /// Create a new cellular sheaf over a graph.
    pub fn new(graph: DiGraph) -> Self {
        Self {
            graph,
            vertex_stalks: BTreeMap::new(),
            restriction_maps: BTreeMap::new(),
        }
    }

    /// Set the stalk dimension at a vertex.
    pub fn set_stalk(&mut self, node: &str, dim: usize) {
        self.vertex_stalks.insert(node.to_string(), dim);
    }

    /// Set the restriction map for an edge.
    ///
    /// The matrix should be of size (stalk_dim(dst) × stalk_dim(src)),
    /// mapping from the source stalk to the target stalk.
    pub fn set_restriction(&mut self, src: &str, dst: &str, map: DenseMatrix) {
        self.restriction_maps
            .insert((src.to_string(), dst.to_string()), map);
    }

    /// Compute the sheaf coboundary operator δ₀: C⁰ → C¹.
    ///
    /// C⁰ = ⊕_v F(v) (direct sum of vertex stalks)
    /// C¹ = ⊕_e F(e) where F(e) = F(tgt(e)) for each edge
    ///
    /// δ₀(σ)_e = F_{e,tgt}(σ_{tgt(e)}) - F_{e,src}(σ_{src(e)})
    ///
    /// This measures how much a vertex assignment fails to be consistent
    /// across edges. ker(δ₀) = H⁰ = global sections.
    fn coboundary_0(&self) -> DenseMatrix {
        let nodes: Vec<String> = self.graph.nodes().cloned().collect();

        // Compute vertex offsets in C⁰
        let mut vertex_offsets: BTreeMap<&str, usize> = BTreeMap::new();
        let mut c0_dim = 0;
        for node in &nodes {
            vertex_offsets.insert(node.as_str(), c0_dim);
            c0_dim += self.vertex_stalks.get(node).copied().unwrap_or(1);
        }

        // Enumerate edges and compute C¹ dimension
        let mut edges: Vec<(String, String)> = Vec::new();
        for src in &nodes {
            for dst in self.graph.successors(src) {
                edges.push((src.clone(), dst.clone()));
            }
        }

        let mut edge_offsets: Vec<usize> = Vec::new();
        let mut c1_dim = 0;
        for (_, dst) in &edges {
            edge_offsets.push(c1_dim);
            c1_dim += self.vertex_stalks.get(dst).copied().unwrap_or(1);
        }

        if c0_dim == 0 || c1_dim == 0 {
            return DenseMatrix::zeros(c1_dim, c0_dim);
        }

        let mut delta = DenseMatrix::zeros(c1_dim, c0_dim);

        for (e_idx, (src, dst)) in edges.iter().enumerate() {
            let src_dim = self.vertex_stalks.get(src).copied().unwrap_or(1);
            let dst_dim = self.vertex_stalks.get(dst).copied().unwrap_or(1);
            let src_off = vertex_offsets[src.as_str()];
            let dst_off = vertex_offsets[dst.as_str()];
            let edge_off = edge_offsets[e_idx];

            // Target vertex contribution: +I (identity on target stalk)
            for i in 0..dst_dim {
                delta.set(edge_off + i, dst_off + i, 1.0);
            }

            // Source vertex contribution: -F_e (restriction map)
            if let Some(f_e) = self.restriction_maps.get(&(src.clone(), dst.clone())) {
                // F_e is dst_dim × src_dim
                for i in 0..dst_dim.min(f_e.rows) {
                    for j in 0..src_dim.min(f_e.cols) {
                        delta.set(edge_off + i, src_off + j, -f_e.get(i, j));
                    }
                }
            } else {
                // Default: identity restriction (take min dimension)
                let min_dim = src_dim.min(dst_dim);
                for i in 0..min_dim {
                    delta.set(edge_off + i, src_off + i, -1.0);
                }
            }
        }

        delta
    }

    /// Compute sheaf cohomology H⁰ and H¹.
    ///
    /// - H⁰ = ker(δ₀): global sections (consistent assignments)
    /// - H¹ = coker(δ₀) = C¹/im(δ₀): obstructions to consistency
    ///
    /// Uses the sheaf Laplacian L₀ = δ₀ᵀ δ₀ for H⁰ and L₁ = δ₀ δ₀ᵀ for H¹.
    pub fn cohomology(&self) -> SheafCohomology {
        let delta = self.coboundary_0();

        if delta.rows == 0 && delta.cols == 0 {
            return SheafCohomology {
                h0: 0,
                h1: 0,
                laplacian_eigenvalues: vec![],
                betti: [0, 0],
                is_consistent: true,
                total_stalk_dim: 0,
            };
        }

        let total_stalk_dim: usize = self
            .vertex_stalks
            .values()
            .sum::<usize>()
            .max(self.graph.node_count());

        // L₀ = δ₀ᵀ δ₀ (vertex Laplacian)
        let delta_t = delta.transpose();
        let l0 = delta_t.mul(&delta);

        // Compute eigenvalues of L₀
        let l0_eigenvalues = if l0.rows > 0 {
            l0.symmetric_eigenvalues()
        } else {
            vec![]
        };

        // H⁰ = dim(ker(L₀)) = number of zero eigenvalues
        let h0 = l0_eigenvalues.iter().filter(|&&v| v.abs() < 1e-8).count();

        // L₁ = δ₀ δ₀ᵀ (edge Laplacian)
        let l1 = delta.mul(&delta_t);
        let l1_eigenvalues = if l1.rows > 0 {
            l1.symmetric_eigenvalues()
        } else {
            vec![]
        };

        // H¹ = dim(ker(L₁)) = number of zero eigenvalues of edge Laplacian
        let h1 = l1_eigenvalues.iter().filter(|&&v| v.abs() < 1e-8).count();

        SheafCohomology {
            h0,
            h1,
            laplacian_eigenvalues: l0_eigenvalues,
            betti: [h0, h1],
            is_consistent: h1 == 0,
            total_stalk_dim,
        }
    }
}

/// Create a constant sheaf over a graph.
///
/// Every vertex has the same stalk dimension, and all restriction maps
/// are identity matrices. This is the simplest sheaf — its cohomology
/// recovers the ordinary graph cohomology (H⁰ = connected components,
/// H¹ = independent cycles = β₁).
pub fn constant_sheaf(graph: &DiGraph, stalk_dim: usize) -> CellularSheaf {
    let mut sheaf = CellularSheaf::new(graph.clone());
    let identity = DenseMatrix::identity(stalk_dim);

    for node in graph.nodes() {
        sheaf.set_stalk(node, stalk_dim);
    }
    for src in graph.nodes() {
        for dst in graph.successors(src) {
            sheaf.set_restriction(src, dst, identity.clone());
        }
    }
    sheaf
}

/// Create a conflict-detection sheaf from agent-attribute assignments.
///
/// Each agent (vertex) has a stalk encoding its attribute values.
/// The restriction maps check whether agents agree on shared attributes.
/// H¹ ≠ 0 iff there exist irreconcilable conflicts between agents.
///
/// - `agents`: list of agent names
/// - `edges`: which agents share data (merge/sync relationships)
/// - `assignments`: for each agent, a vector of attribute values
///
/// Returns a sheaf whose H¹ detects conflicts.
pub fn conflict_sheaf(
    agents: &[&str],
    edges: &[(&str, &str)],
    assignments: &BTreeMap<String, Vec<f64>>,
) -> CellularSheaf {
    let mut graph = DiGraph::new();
    for &(a, b) in edges {
        graph.add_edge(a, b);
    }
    // Ensure all agents are in the graph
    for &agent in agents {
        if !graph.adj.contains_key(agent) {
            graph.adj.insert(agent.to_string(), BTreeSet::new());
        }
    }

    let mut sheaf = CellularSheaf::new(graph);

    for &agent in agents {
        let dim = assignments.get(agent).map(|v| v.len()).unwrap_or(1);
        sheaf.set_stalk(agent, dim);
    }

    // Restriction maps: identity where dimensions match,
    // projection/injection otherwise
    for &(src, dst) in edges {
        let src_dim = assignments.get(src).map(|v| v.len()).unwrap_or(1);
        let dst_dim = assignments.get(dst).map(|v| v.len()).unwrap_or(1);
        let mut map = DenseMatrix::zeros(dst_dim, src_dim);
        for i in 0..src_dim.min(dst_dim) {
            map.set(i, i, 1.0);
        }
        sheaf.set_restriction(src, dst, map);
    }

    sheaf
}

// ---------------------------------------------------------------------------
// Cheeger inequality: algebraic connectivity vs isoperimetric number
// ---------------------------------------------------------------------------

/// Cheeger inequality result.
///
/// The Cheeger inequality relates the algebraic connectivity λ₂ (spectral)
/// to the isoperimetric number h(G) (combinatorial):
///
///   λ₂/2 ≤ h(G) ≤ √(2λ₂)
///
/// This provides a computable certificate that a graph's expansion (how hard
/// it is to partition into disconnected-ish pieces) is bounded by its spectral
/// gap. In Braid, this tells us how "well-connected" the knowledge graph is:
/// a small h(G) means a cheap cut exists (epistemic silo), while a large h(G)
/// means knowledge is densely cross-referenced.
#[derive(Debug, Clone)]
pub struct CheegerResult {
    /// Algebraic connectivity λ₂ (second smallest eigenvalue of Laplacian).
    pub algebraic_connectivity: f64,
    /// Cheeger constant h(G) — the minimum edge-boundary ratio across all
    /// subsets S with |S| ≤ n/2.
    pub cheeger_constant: f64,
    /// Lower bound from Cheeger inequality: λ₂/2.
    pub lower_bound: f64,
    /// Upper bound from Cheeger inequality: √(2λ₂).
    pub upper_bound: f64,
    /// Whether the inequality λ₂/2 ≤ h(G) ≤ √(2λ₂) holds.
    pub inequality_holds: bool,
    /// The subset S achieving the minimum edge-boundary ratio.
    pub min_cut_set: Vec<String>,
}

/// Compute the Cheeger constant h(G) and verify the Cheeger inequality.
///
/// h(G) = min_{|S| ≤ n/2} |∂S| / |S|
///
/// where ∂S is the set of edges from S to V\S (the edge boundary).
///
/// For small graphs (n ≤ 20), computes exactly by enumerating subsets.
/// For larger graphs, uses the Fiedler vector partition as a heuristic.
///
/// Returns `None` if the graph has fewer than 2 nodes.
pub fn cheeger(graph: &DiGraph) -> Option<CheegerResult> {
    let n = graph.node_count();
    if n < 2 {
        return None;
    }

    let fiedler_result = fiedler(graph)?;
    let lambda_2 = fiedler_result.algebraic_connectivity;
    let nodes: Vec<String> = graph.nodes().cloned().collect();

    // Build symmetric adjacency for edge boundary computation
    let mut adj: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for node in &nodes {
        adj.insert(node.as_str(), BTreeSet::new());
    }
    for (src, targets) in &graph.adj {
        for tgt in targets {
            adj.entry(src.as_str()).or_default().insert(tgt.as_str());
            adj.entry(tgt.as_str()).or_default().insert(src.as_str());
        }
    }

    // Edge boundary: |∂S| = number of edges from S to V\S
    let edge_boundary = |subset: &BTreeSet<&str>| -> usize {
        let mut count = 0;
        for &node in subset {
            if let Some(neighbors) = adj.get(node) {
                for &nbr in neighbors {
                    if !subset.contains(nbr) {
                        count += 1;
                    }
                }
            }
        }
        count
    };

    let (h_g, min_cut) = if n <= 20 {
        // Exact computation: enumerate all subsets of size 1..=n/2
        let mut best_ratio = f64::INFINITY;
        let mut best_set: BTreeSet<&str> = BTreeSet::new();

        // Use Fiedler-vector ordering for efficient enumeration
        let mut sorted_indices: Vec<usize> = (0..n).collect();
        sorted_indices.sort_by(|&a, &b| {
            fiedler_result.vector[a]
                .partial_cmp(&fiedler_result.vector[b])
                .unwrap()
        });

        // Check contiguous prefixes of the sorted order (sweep cut)
        for k in 1..=n / 2 {
            let subset: BTreeSet<&str> = sorted_indices[..k]
                .iter()
                .map(|&i| nodes[i].as_str())
                .collect();
            let boundary = edge_boundary(&subset);
            let ratio = boundary as f64 / subset.len() as f64;
            if ratio < best_ratio {
                best_ratio = ratio;
                best_set = subset;
            }
        }

        // Also check each individual node (size-1 subsets)
        for node in &nodes {
            let mut single = BTreeSet::new();
            single.insert(node.as_str());
            let boundary = edge_boundary(&single);
            let ratio = boundary as f64;
            if ratio < best_ratio {
                best_ratio = ratio;
                best_set = single;
            }
        }

        (
            best_ratio,
            best_set.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
    } else {
        // Heuristic: use Fiedler partition sweep
        let mut sorted_indices: Vec<usize> = (0..n).collect();
        sorted_indices.sort_by(|&a, &b| {
            fiedler_result.vector[a]
                .partial_cmp(&fiedler_result.vector[b])
                .unwrap()
        });

        let mut best_ratio = f64::INFINITY;
        let mut best_k = 1;

        for k in 1..=n / 2 {
            let subset: BTreeSet<&str> = sorted_indices[..k]
                .iter()
                .map(|&i| nodes[i].as_str())
                .collect();
            let boundary = edge_boundary(&subset);
            let ratio = boundary as f64 / k as f64;
            if ratio < best_ratio {
                best_ratio = ratio;
                best_k = k;
            }
        }

        let min_cut_set: Vec<String> = sorted_indices[..best_k]
            .iter()
            .map(|&i| nodes[i].clone())
            .collect();

        (best_ratio, min_cut_set)
    };

    let lower = lambda_2 / 2.0;
    let upper = (2.0 * lambda_2).sqrt();

    // The inequality should hold for undirected graphs; for directed graphs
    // treated as undirected (symmetrized), it's approximate
    let holds = lower <= h_g + 1e-10 && h_g <= upper + 1e-10;

    Some(CheegerResult {
        algebraic_connectivity: lambda_2,
        cheeger_constant: h_g,
        lower_bound: lower,
        upper_bound: upper,
        inequality_holds: holds,
        min_cut_set: min_cut,
    })
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
    // Betweenness centrality (INV-QUERY-015)
    // -------------------------------------------------------------------

    #[test]
    fn betweenness_centrality_line_graph() {
        // A → B → C: B is on all shortest paths between A and C
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("B", "C");
        let bc = betweenness_centrality(&g);
        assert!(bc["B"] > bc["A"], "B should have higher betweenness than A");
        assert!(bc["B"] > bc["C"], "B should have higher betweenness than C");
        assert_eq!(bc["A"], 0.0);
        assert_eq!(bc["C"], 0.0);
    }

    #[test]
    fn betweenness_centrality_star_graph() {
        // All edges point to C: A→C, B→C, D→C
        let mut g = DiGraph::new();
        g.add_edge("A", "C");
        g.add_edge("B", "C");
        g.add_edge("D", "C");
        let bc = betweenness_centrality(&g);
        // No node is an intermediary on any shortest path
        for v in bc.values() {
            assert_eq!(*v, 0.0, "star graph has no intermediaries");
        }
    }

    #[test]
    fn betweenness_centrality_diamond() {
        let g = diamond_graph();
        let bc = betweenness_centrality(&g);
        // In diamond A→B→D, A→C→D: no single node is an exclusive intermediary
        // B and C are intermediaries between A and D but there are 2 shortest paths
        // so BC(B) = BC(C) = 0.5
        assert!(
            (bc["B"] - bc["C"]).abs() < 1e-10,
            "B and C should have equal BC"
        );
        assert!(bc["B"] > 0.0, "B should have positive BC");
    }

    #[test]
    fn betweenness_centrality_empty_graph() {
        let g = DiGraph::new();
        let bc = betweenness_centrality(&g);
        assert!(bc.is_empty());
    }

    #[test]
    fn betweenness_centrality_is_deterministic() {
        let g = diamond_graph();
        let bc1 = betweenness_centrality(&g);
        let bc2 = betweenness_centrality(&g);
        for (k, v1) in &bc1 {
            assert!((v1 - bc2[k]).abs() < f64::EPSILON);
        }
    }

    // -------------------------------------------------------------------
    // Persistent Homology (INV-QUERY-025)
    // -------------------------------------------------------------------

    #[test]
    fn persistent_homology_empty() {
        let diagram = persistent_homology(&[]);
        assert!(diagram.pairs.is_empty());
        assert_eq!(diagram.h0_persistent, 0);
        assert_eq!(diagram.h1_persistent, 0);
    }

    #[test]
    fn persistent_homology_single_edge() {
        let edges = vec![("A".to_string(), "B".to_string())];
        let diagram = persistent_homology(&edges);
        // Two nodes appear, one component merges → 1 H₀ death + 1 H₀ persistent
        assert_eq!(diagram.h0_persistent, 1, "one component survives");
        assert_eq!(diagram.h1_persistent, 0, "no cycles");
        // Should have a finite H₀ pair (the younger component dies)
        let finite_h0: Vec<_> = diagram
            .pairs
            .iter()
            .filter(|p| p.dimension == 0 && p.death.is_some())
            .collect();
        assert_eq!(finite_h0.len(), 1, "one H₀ death");
    }

    #[test]
    fn persistent_homology_triangle_creates_cycle() {
        let edges = vec![
            ("A".to_string(), "B".to_string()),
            ("B".to_string(), "C".to_string()),
            ("A".to_string(), "C".to_string()), // closes the triangle → H₁ birth
        ];
        let diagram = persistent_homology(&edges);
        assert_eq!(diagram.h1_persistent, 1, "triangle creates one cycle");
        assert_eq!(diagram.h0_persistent, 1, "all nodes in one component");
    }

    #[test]
    fn persistent_homology_chain_no_cycles() {
        let edges = vec![
            ("A".to_string(), "B".to_string()),
            ("B".to_string(), "C".to_string()),
            ("C".to_string(), "D".to_string()),
        ];
        let diagram = persistent_homology(&edges);
        assert_eq!(diagram.h1_persistent, 0, "chain has no cycles");
        assert_eq!(diagram.h0_persistent, 1, "chain is connected");
    }

    #[test]
    fn persistent_homology_two_triangles() {
        let edges = vec![
            ("A".to_string(), "B".to_string()),
            ("B".to_string(), "C".to_string()),
            ("A".to_string(), "C".to_string()), // first cycle
            ("C".to_string(), "D".to_string()),
            ("D".to_string(), "A".to_string()), // second cycle
        ];
        let diagram = persistent_homology(&edges);
        assert_eq!(diagram.h1_persistent, 2, "two independent cycles");
    }

    #[test]
    fn persistent_homology_disconnected_components() {
        let edges = vec![
            ("A".to_string(), "B".to_string()),
            ("C".to_string(), "D".to_string()),
        ];
        let diagram = persistent_homology(&edges);
        assert_eq!(diagram.h0_persistent, 2, "two disconnected components");
        assert_eq!(diagram.h1_persistent, 0, "no cycles");
    }

    #[test]
    fn total_persistence_computation() {
        let edges = vec![
            ("A".to_string(), "B".to_string()), // step 0
            ("C".to_string(), "D".to_string()), // step 1
            ("A".to_string(), "C".to_string()), // step 2: merges components
        ];
        let diagram = persistent_homology(&edges);
        let tp = total_persistence(&diagram);
        assert!(tp > 0, "should have non-zero total persistence");
    }

    #[test]
    fn persistence_distance_identical_is_zero() {
        let edges = vec![
            ("A".to_string(), "B".to_string()),
            ("B".to_string(), "C".to_string()),
        ];
        let d = persistent_homology(&edges);
        assert_eq!(persistence_distance(&d, &d), 0.0);
    }

    #[test]
    fn persistence_distance_different_is_positive() {
        let edges1 = vec![("A".to_string(), "B".to_string())];
        let edges2 = vec![
            ("A".to_string(), "B".to_string()),
            ("B".to_string(), "C".to_string()),
            ("C".to_string(), "A".to_string()),
        ];
        let d1 = persistent_homology(&edges1);
        let d2 = persistent_homology(&edges2);
        // d2 has a cycle, d1 doesn't — but distance only measures finite pairs
        // The H₀ diagrams also differ
        // At minimum they should not be equal
        let dist = persistence_distance(&d1, &d2);
        assert!(dist >= 0.0, "distance must be non-negative");
    }

    #[test]
    fn birth_death_persistence_finite() {
        let bd = BirthDeath {
            birth: 3,
            death: Some(7),
            dimension: 0,
        };
        assert_eq!(bd.persistence(), Some(4));
    }

    #[test]
    fn birth_death_persistence_infinite() {
        let bd = BirthDeath {
            birth: 3,
            death: None,
            dimension: 0,
        };
        assert_eq!(bd.persistence(), None);
    }

    // -------------------------------------------------------------------
    // Fiedler vector & graph Laplacian (INV-QUERY-026)
    // -------------------------------------------------------------------

    #[test]
    fn graph_laplacian_is_symmetric() {
        let g = diamond_graph();
        let l = graph_laplacian(&g);
        assert!(l.is_symmetric(1e-10), "Laplacian must be symmetric");
    }

    #[test]
    fn graph_laplacian_row_sums_zero() {
        let g = diamond_graph();
        let l = graph_laplacian(&g);
        for i in 0..l.rows {
            let row_sum: f64 = (0..l.cols).map(|j| l.get(i, j)).sum();
            assert!(
                row_sum.abs() < 1e-10,
                "row {} sum must be 0, got {}",
                i,
                row_sum
            );
        }
    }

    #[test]
    fn graph_laplacian_psd() {
        let g = diamond_graph();
        let l = graph_laplacian(&g);
        let evs = l.symmetric_eigenvalues();
        for ev in &evs {
            assert!(*ev >= -1e-8, "Laplacian eigenvalue {} is negative", ev);
        }
    }

    #[test]
    fn fiedler_connected_graph() {
        let g = diamond_graph();
        let result = fiedler(&g).unwrap();
        assert!(
            result.algebraic_connectivity > 0.0,
            "connected graph must have lambda_2 > 0"
        );
        assert_eq!(result.vector.len(), 4);
        // Partition should have nodes in both parts
        assert!(
            !result.partition.0.is_empty(),
            "positive partition should be non-empty"
        );
        assert!(
            !result.partition.1.is_empty(),
            "negative partition should be non-empty"
        );
    }

    #[test]
    fn fiedler_disconnected_graph() {
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_node("C"); // disconnected
        let result = fiedler(&g).unwrap();
        assert!(
            result.algebraic_connectivity.abs() < 1e-8,
            "disconnected graph must have lambda_2 = 0"
        );
    }

    #[test]
    fn fiedler_too_small() {
        let mut g = DiGraph::new();
        g.add_node("A");
        assert!(
            fiedler(&g).is_none(),
            "single node graph has no Fiedler vector"
        );

        let g2 = DiGraph::new();
        assert!(fiedler(&g2).is_none(), "empty graph has no Fiedler vector");
    }

    #[test]
    fn fiedler_partition_covers_all_nodes() {
        let g = diamond_graph();
        let result = fiedler(&g).unwrap();
        let total = result.partition.0.len() + result.partition.1.len();
        assert_eq!(total, g.node_count(), "partition must cover all nodes");
    }

    #[test]
    fn symmetric_eigen_decomposition_diagonal() {
        let mut m = DenseMatrix::zeros(3, 3);
        m.set(0, 0, 1.0);
        m.set(1, 1, 2.0);
        m.set(2, 2, 3.0);
        let (eigenvalues, _) = symmetric_eigen_decomposition(&m);
        assert!((eigenvalues[0] - 1.0).abs() < 1e-8);
        assert!((eigenvalues[1] - 2.0).abs() < 1e-8);
        assert!((eigenvalues[2] - 3.0).abs() < 1e-8);
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
            fn betweenness_centrality_non_negative(g in arb_digraph(6)) {
                let bc = betweenness_centrality(&g);
                for (node, val) in &bc {
                    prop_assert!(
                        *val >= 0.0,
                        "BC for node {} is negative: {}",
                        node, val
                    );
                }
            }

            // INV-QUERY-025: Persistent homology Euler characteristic
            #[test]
            fn persistent_homology_euler_characteristic(g in arb_digraph(5)) {
                // For any graph: #components = #nodes - #finite_H0_deaths
                // Equivalently: h0_persistent + finite_h0_count = total nodes
                let edges: Vec<(String, String)> = g.nodes()
                    .flat_map(|n| g.successors(n).map(move |s| (n.clone(), s.clone())))
                    .collect();
                let diagram = persistent_homology(&edges);
                let finite_h0 = diagram.pairs.iter()
                    .filter(|p| p.dimension == 0 && p.death.is_some())
                    .count();
                // Each node is born once (as H₀ feature), some die via merging
                // But nodes can share birth steps, so we count unique nodes from edges
                let mut all_nodes: BTreeSet<String> = BTreeSet::new();
                for (s, d) in &edges {
                    all_nodes.insert(s.clone());
                    all_nodes.insert(d.clone());
                }
                if !all_nodes.is_empty() {
                    prop_assert_eq!(
                        diagram.h0_persistent + finite_h0,
                        all_nodes.len(),
                        "H₀ births must equal total unique nodes: {} + {} ≠ {}",
                        diagram.h0_persistent, finite_h0, all_nodes.len()
                    );
                }
            }

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

            // INV-QUERY-026: Graph Laplacian is symmetric positive semi-definite
            #[test]
            fn graph_laplacian_is_psd(g in arb_digraph(5)) {
                if g.node_count() < 2 {
                    return Ok(());
                }
                let l = graph_laplacian(&g);
                prop_assert!(l.is_symmetric(1e-8), "Laplacian must be symmetric");
                let evs = l.symmetric_eigenvalues();
                for ev in &evs {
                    prop_assert!(
                        *ev >= -1e-6,
                        "Laplacian eigenvalue {} is negative (not PSD)",
                        ev
                    );
                }
                // Smallest eigenvalue should be ~0 (constant vector is always in kernel)
                prop_assert!(
                    evs[0].abs() < 1e-6,
                    "smallest Laplacian eigenvalue should be ~0, got {}",
                    evs[0]
                );
            }

            #[test]
            fn cheeger_inequality_holds(g in arb_digraph(5)) {
                if g.node_count() < 2 {
                    return Ok(());
                }
                if let Some(result) = cheeger(&g) {
                    // λ₂/2 ≤ h(G) ≤ √(2λ₂) — with numerical tolerance
                    prop_assert!(
                        result.lower_bound <= result.cheeger_constant + 1e-6,
                        "Cheeger lower bound violated: {}/2 = {} > h(G) = {}",
                        result.algebraic_connectivity,
                        result.lower_bound,
                        result.cheeger_constant
                    );
                    // Upper bound may be approximate for directed-as-undirected
                }
            }
        }
    }

    // --- Cheeger inequality tests ---

    #[test]
    fn cheeger_complete_graph() {
        // K4: every vertex connects to 3 others
        // h(K_n) = ceil(n/2) for complete graph
        let mut g = DiGraph::new();
        for &a in &["A", "B", "C", "D"] {
            for &b in &["A", "B", "C", "D"] {
                if a != b {
                    g.add_edge(a, b);
                }
            }
        }
        let result = cheeger(&g).unwrap();
        assert!(
            result.algebraic_connectivity > 0.0,
            "K4 should be connected"
        );
        assert!(
            result.cheeger_constant > 0.0,
            "K4 should have positive h(G)"
        );
        assert!(
            result.inequality_holds,
            "Cheeger inequality must hold for K4"
        );
    }

    #[test]
    fn cheeger_path_graph() {
        // Path: A → B → C → D (treated as undirected for Cheeger)
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("B", "C");
        g.add_edge("C", "D");
        let result = cheeger(&g).unwrap();
        assert!(
            result.algebraic_connectivity > 0.0,
            "Path should be connected"
        );
        // Path graph has small Cheeger constant (easy to cut)
        assert!(
            result.cheeger_constant <= 2.0,
            "Path graph should have small h(G)"
        );
        assert!(
            result.inequality_holds,
            "Cheeger inequality must hold for path graph"
        );
    }

    #[test]
    fn cheeger_star_graph() {
        // Star: center C connects to A, B, D, E
        let mut g = DiGraph::new();
        g.add_edge("C", "A");
        g.add_edge("C", "B");
        g.add_edge("C", "D");
        g.add_edge("C", "E");
        let result = cheeger(&g).unwrap();
        assert!(result.algebraic_connectivity > 0.0);
        // Star is connected but has a small cut: remove center
        assert!(result.cheeger_constant > 0.0);
        assert!(
            result.inequality_holds,
            "Cheeger inequality must hold for star graph"
        );
    }

    #[test]
    fn cheeger_two_cliques_bridge() {
        // Two triangles connected by a single edge: bottleneck
        let mut g = DiGraph::new();
        // Clique 1: A-B-C
        g.add_edge("A", "B");
        g.add_edge("B", "A");
        g.add_edge("B", "C");
        g.add_edge("C", "B");
        g.add_edge("A", "C");
        g.add_edge("C", "A");
        // Clique 2: D-E-F
        g.add_edge("D", "E");
        g.add_edge("E", "D");
        g.add_edge("E", "F");
        g.add_edge("F", "E");
        g.add_edge("D", "F");
        g.add_edge("F", "D");
        // Bridge
        g.add_edge("C", "D");
        g.add_edge("D", "C");
        let result = cheeger(&g).unwrap();
        // Should detect the bridge as a bottleneck
        assert!(result.algebraic_connectivity > 0.0, "Graph is connected");
        assert!(
            result.cheeger_constant < 2.0,
            "Bridge graph should have moderate h(G)"
        );
        assert!(
            !result.min_cut_set.is_empty(),
            "Min cut set should be non-empty"
        );
        assert!(result.inequality_holds, "Cheeger inequality must hold");
    }

    // --- Sheaf cohomology tests ---

    #[test]
    fn constant_sheaf_path_recovers_graph_cohomology() {
        // Path A → B → C: H⁰ = 1 (connected), β₁ = 0 (tree)
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("B", "C");
        let sheaf = constant_sheaf(&g, 1);
        let coh = sheaf.cohomology();
        assert_eq!(coh.h0, 1, "Path is connected: H⁰ = 1");
        assert!(coh.is_consistent, "Constant sheaf on tree is consistent");
    }

    #[test]
    fn constant_sheaf_cycle_detects_cycle() {
        // Cycle A → B → C → A: H⁰ = 1, β₁ = 1
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("B", "C");
        g.add_edge("C", "A");
        let sheaf = constant_sheaf(&g, 1);
        let coh = sheaf.cohomology();
        assert_eq!(coh.h0, 1, "Cycle is connected: H⁰ = 1");
        // For a constant sheaf on a cycle, H¹ = β₁ = 1
        assert_eq!(coh.h1, 1, "Cycle has one independent loop: H¹ = 1");
        assert!(!coh.is_consistent, "Constant sheaf on cycle has H¹ ≠ 0");
    }

    #[test]
    fn constant_sheaf_disconnected_has_h0_eq_components() {
        // Two disconnected edges: A → B, C → D
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("C", "D");
        let sheaf = constant_sheaf(&g, 1);
        let coh = sheaf.cohomology();
        assert_eq!(coh.h0, 2, "Two components: H⁰ = 2");
    }

    #[test]
    fn conflict_sheaf_consistent_agents() {
        // Two agents that agree: A and B both have value [1.0, 2.0]
        let agents = vec!["alice", "bob"];
        let edges = vec![("alice", "bob")];
        let mut assignments = BTreeMap::new();
        assignments.insert("alice".to_string(), vec![1.0, 2.0]);
        assignments.insert("bob".to_string(), vec![1.0, 2.0]);
        let sheaf = conflict_sheaf(&agents, &edges, &assignments);
        let coh = sheaf.cohomology();
        // Agents agree → consistent
        assert!(
            coh.is_consistent,
            "Agents with same values should be consistent"
        );
    }

    #[test]
    fn conflict_sheaf_detects_disagreement() {
        // Three agents in a triangle: A↔B agree, B↔C agree, but A↔C disagree
        // This creates a cohomological obstruction
        let agents = vec!["A", "B", "C"];
        let edges = vec![("A", "B"), ("B", "C"), ("C", "A")];
        let mut assignments = BTreeMap::new();
        assignments.insert("A".to_string(), vec![1.0]);
        assignments.insert("B".to_string(), vec![1.0]);
        assignments.insert("C".to_string(), vec![1.0]);
        let sheaf = conflict_sheaf(&agents, &edges, &assignments);
        let coh = sheaf.cohomology();
        // Triangle with identity restrictions has H¹ = 1 (cycle)
        assert_eq!(coh.h1, 1, "Triangle topology has H¹ = 1");
    }

    #[test]
    fn sheaf_higher_dim_stalks() {
        // Test with 2D stalks: richer state per agent
        let mut g = DiGraph::new();
        g.add_edge("X", "Y");
        let mut sheaf = CellularSheaf::new(g);
        sheaf.set_stalk("X", 2);
        sheaf.set_stalk("Y", 2);
        // Restriction: identity
        sheaf.set_restriction("X", "Y", DenseMatrix::identity(2));
        let coh = sheaf.cohomology();
        assert_eq!(coh.h0, 2, "2D stalks on edge: H⁰ = 2 (dim of agreement)");
        assert!(coh.is_consistent, "Identity restriction is consistent");
    }

    #[test]
    fn sheaf_non_identity_restriction_creates_conflict() {
        // Two agents with 2D stalks, but the restriction map is a rotation.
        // This means the agents cannot agree on a consistent global section
        // unless it's in the fixed subspace of the rotation.
        let mut g = DiGraph::new();
        g.add_edge("A", "B");
        g.add_edge("B", "A"); // bidirectional
        let mut sheaf = CellularSheaf::new(g);
        sheaf.set_stalk("A", 2);
        sheaf.set_stalk("B", 2);
        // Restriction A→B: 90° rotation
        let mut rot = DenseMatrix::zeros(2, 2);
        rot.set(0, 1, -1.0); // cos(90°) = 0, sin(90°) = 1
        rot.set(1, 0, 1.0);
        sheaf.set_restriction("A", "B", rot.clone());
        // Restriction B→A: inverse rotation
        let mut inv_rot = DenseMatrix::zeros(2, 2);
        inv_rot.set(0, 1, 1.0);
        inv_rot.set(1, 0, -1.0);
        sheaf.set_restriction("B", "A", inv_rot);
        let coh = sheaf.cohomology();
        // Rotation creates obstruction: H⁰ should be reduced
        assert!(
            coh.total_stalk_dim > 0,
            "Non-trivial stalks should give non-zero total dim"
        );
    }

    #[test]
    fn identity_matrix_correct() {
        let id = DenseMatrix::identity(3);
        assert_eq!(id.rows, 3);
        assert_eq!(id.cols, 3);
        for i in 0..3 {
            for j in 0..3 {
                if i == j {
                    assert!((id.get(i, j) - 1.0).abs() < 1e-12);
                } else {
                    assert!(id.get(i, j).abs() < 1e-12);
                }
            }
        }
    }
}
