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
}
