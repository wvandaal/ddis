//! Coordination Topology — file coupling extraction, spectral partitioning,
//! and compiled agent assignment.
//!
//! Implements ADR-TOPOLOGY-004 (Topology as Compilation):
//! 1. **Front-end**: Extract coupling from task→file overlap (Jaccard similarity).
//! 2. **Middle-end**: Partition tasks into disjoint groups via connected components.
//! 3. **Back-end**: Assign groups to agents via greedy bin-packing by R(t) impact.
//! 4. **Emit**: Produce a `TopologyPlan` with per-agent task lists, file sets,
//!    coupling entropy, parallelizability, and topology pattern classification.
//!
//! The normalized coupling matrix ρ_C = C / Tr(C) is a density matrix whose
//! von Neumann entropy S(ρ_C) equals the irreducible coordination complexity.
//! Effective rank r_eff = exp(S) = optimal number of parallel agent groups.
//! Parallelizability p = r_eff / n (topology-level Amdahl's Law).
//!
//! Traces to: spec/19-topology.md INV-TOPOLOGY-001..005, ADR-TOPOLOGY-004,
//! spec/20-coherence.md (density matrix formalism).

use std::collections::{BTreeMap, BTreeSet};

use crate::datom::{Attribute, EntityId, Value};
use crate::error::TopologyError;
use crate::store::Store;

/// Check if a path has a recognized source file extension.
///
/// Language-agnostic: covers all common source, config, and doc file types.
/// C8 compliance: no assumption about which language the project uses.
fn has_source_extension(path: &str) -> bool {
    let extensions = [
        ".rs", ".go", ".ts", ".tsx", ".js", ".jsx", ".py", ".rb", ".java", ".kt", ".swift", ".c",
        ".cpp", ".h", ".hpp", ".cs", ".fs", ".edn", ".toml", ".yaml", ".yml", ".json", ".md",
        ".sh",
    ];
    extensions.iter().any(|ext| path.ends_with(ext))
}

/// Strip file extension from a path component, for any language.
///
/// Returns the stem with the extension removed. If no known extension
/// is found, returns the original string unchanged.
fn strip_source_extension(part: &str) -> &str {
    let extensions = [
        ".rs", ".go", ".ts", ".tsx", ".js", ".jsx", ".py", ".rb", ".java", ".kt", ".swift", ".c",
        ".cpp", ".h", ".hpp", ".cs", ".fs", ".edn", ".toml", ".yaml", ".yml", ".json", ".md",
        ".sh",
    ];
    for ext in &extensions {
        if let Some(stem) = part.strip_suffix(ext) {
            return stem;
        }
    }
    part
}

/// Extract the set of files a task touches, from its title text.
///
/// Looks for:
/// 1. Explicit `FILE:` or `FILES:` markers in the title
/// 2. Inline file paths (any path with `/` separator and recognized source extension)
///
/// Returns a set of normalized file paths.
pub fn extract_task_files(title: &str) -> BTreeSet<String> {
    let mut files = BTreeSet::new();

    // Pattern 1: FILE: or FILES: markers
    let lower = title.to_lowercase();
    for marker in &["file:", "files:"] {
        if let Some(pos) = lower.find(marker) {
            let after = &title[pos + marker.len()..];
            // Extract file paths until next marker or end
            for word in after.split_whitespace() {
                let trimmed = word.trim_matches(|c: char| c == ',' || c == '.' || c == ')');
                if trimmed.contains('/') && trimmed.contains('.') {
                    files.insert(trimmed.to_string());
                }
                // Stop at next section marker
                if word.contains(':') && !word.contains('/') && word.len() > 3 {
                    break;
                }
            }
        }
    }

    // Pattern 2: Inline file paths (any path with directory separator + file extension)
    // Language-agnostic: detects any path containing '/' and a common source extension.
    // C8 compliance: no assumption about project layout or programming language.
    for word in title.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '/' && c != '.' && c != '-' && c != '_'
        });
        if trimmed.contains('/') && has_source_extension(trimmed) {
            files.insert(trimmed.to_string());
        }
    }

    files
}

/// Compute file coupling between tasks using Jaccard similarity.
///
/// Returns a map from (task_i, task_j) → coupling score.
/// Score = |files_i ∩ files_j| / |files_i ∪ files_j|.
/// Only includes pairs with score > 0.
pub fn compute_file_coupling(
    task_files: &BTreeMap<EntityId, BTreeSet<String>>,
) -> BTreeMap<(EntityId, EntityId), f64> {
    let mut coupling = BTreeMap::new();
    let entities: Vec<&EntityId> = task_files.keys().collect();

    for i in 0..entities.len() {
        for j in (i + 1)..entities.len() {
            let files_i = &task_files[entities[i]];
            let files_j = &task_files[entities[j]];

            if files_i.is_empty() || files_j.is_empty() {
                continue;
            }

            let intersection = files_i.intersection(files_j).count();
            if intersection == 0 {
                continue;
            }

            let union = files_i.union(files_j).count();
            let score = intersection as f64 / union as f64;

            coupling.insert((*entities[i], *entities[j]), score);
            coupling.insert((*entities[j], *entities[i]), score);
        }
    }

    coupling
}

/// Partition tasks into groups with disjoint file sets.
///
/// Uses connected-component detection on the coupling graph.
/// Tasks sharing ANY file end up in the same group.
/// Groups can execute in parallel; tasks within a group must be sequential.
///
/// Returns Vec of groups, sorted by size (largest first).
pub fn partition_by_file_coupling(
    task_files: &BTreeMap<EntityId, BTreeSet<String>>,
) -> Vec<Vec<EntityId>> {
    let entities: Vec<EntityId> = task_files.keys().copied().collect();
    if entities.is_empty() {
        return Vec::new();
    }

    // Build adjacency via file overlap
    let coupling = compute_file_coupling(task_files);
    let mut adjacency: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();
    for &entity in &entities {
        adjacency.entry(entity).or_default();
    }
    for &(a, b) in coupling.keys() {
        adjacency.entry(a).or_default().insert(b);
    }

    // Connected components via BFS
    let mut visited = BTreeSet::new();
    let mut groups = Vec::new();

    for &entity in &entities {
        if visited.contains(&entity) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = vec![entity];
        while let Some(current) = queue.pop() {
            if !visited.insert(current) {
                continue;
            }
            component.push(current);
            if let Some(neighbors) = adjacency.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        queue.push(neighbor);
                    }
                }
            }
        }
        groups.push(component);
    }

    // Sort by size descending
    groups.sort_by_key(|b| std::cmp::Reverse(b.len()));
    groups
}

/// Extract file sets for all ready tasks from the store.
pub fn ready_task_files(store: &Store) -> BTreeMap<EntityId, BTreeSet<String>> {
    let tasks = crate::task::all_tasks(store);
    let mut result = BTreeMap::new();

    for task in &tasks {
        if task.status != crate::task::TaskStatus::Open {
            continue;
        }
        let files = extract_task_files(&task.full_text());
        result.insert(task.entity, files);
    }

    result
}

// ---------------------------------------------------------------------------
// QUICK-1: AgentAssignment + agent naming (INV-TOPOLOGY-005)
// ---------------------------------------------------------------------------

/// A single agent's work assignment within a topology plan.
#[derive(Clone, Debug)]
pub struct AgentAssignment {
    /// Agent name derived from file cluster (e.g., "topology-guidance").
    pub name: String,
    /// Tasks assigned to this agent, ordered by R(t) impact descending.
    pub tasks: Vec<EntityId>,
    /// Union of all files touched by assigned tasks.
    pub files: BTreeSet<String>,
    /// Sum of R(t) impact scores for assigned tasks.
    pub total_impact: f64,
}

/// Derive an agent name from the most common file stems in a task group.
///
/// Extracts meaningful path components — file stems without extensions,
/// and parent directories excluding generic names ("src", "crates").
/// Takes the top 2 by frequency and joins with "-".
/// Falls back to "agent-N" if no useful stems found.
pub fn agent_name_from_files(files: &BTreeSet<String>, index: usize) -> String {
    if files.is_empty() {
        return format!("agent-{index}");
    }

    // Language-agnostic skip list — common directory names that are not meaningful
    let skip = [
        "src",
        "crates",
        "lib",
        "mod",
        "main",
        "tests",
        "test",
        "pkg",
        "internal",
        "cmd",
        "bin",
        "build",
        "dist",
        "out",
        "node_modules",
        "packages",
        "components",
        "utils",
    ];
    let mut stem_counts: BTreeMap<&str, usize> = BTreeMap::new();

    for path in files {
        let parts: Vec<&str> = path.split('/').collect();
        for &part in &parts {
            // Strip any recognized source extension (C8: not just .rs)
            let stem = strip_source_extension(part);
            if !stem.is_empty() && !skip.contains(&stem) && stem.len() > 1 {
                *stem_counts.entry(stem).or_default() += 1;
            }
        }
    }

    if stem_counts.is_empty() {
        return format!("agent-{index}");
    }

    let mut stems: Vec<(&str, usize)> = stem_counts.into_iter().collect();
    stems.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let top: Vec<&str> = stems.iter().take(2).map(|(s, _)| *s).collect();
    top.join("-")
}

// ---------------------------------------------------------------------------
// QUICK-2: balance_assign — greedy bin-packing (INV-TOPOLOGY-005)
// ---------------------------------------------------------------------------

/// Assign task groups to N agents using greedy bin-packing by total impact.
///
/// Each group is a connected component from `partition_by_file_coupling`.
/// Groups are assigned to the agent with the smallest current total impact
/// (longest-processing-time-first heuristic for makespan minimization).
///
/// Returns one `AgentAssignment` per agent (may be fewer than `agent_count`
/// if there are fewer groups than agents).
pub fn balance_assign(
    groups: &[Vec<EntityId>],
    task_files: &BTreeMap<EntityId, BTreeSet<String>>,
    impact_scores: &BTreeMap<EntityId, f64>,
    agent_count: usize,
) -> Result<Vec<AgentAssignment>, TopologyError> {
    if agent_count == 0 {
        return Err(TopologyError::AgentCountZero);
    }

    if groups.is_empty() {
        return Ok(Vec::new());
    }

    // Sort groups by total impact descending (LPT heuristic)
    let mut scored_groups: Vec<(usize, f64)> = groups
        .iter()
        .enumerate()
        .map(|(i, group)| {
            let total: f64 = group
                .iter()
                .map(|e| impact_scores.get(e).copied().unwrap_or(0.0))
                .sum();
            (i, total)
        })
        .collect();
    scored_groups.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Initialize agent bins
    let effective_agents = agent_count.min(groups.len());
    let mut bins: Vec<Vec<usize>> = vec![Vec::new(); effective_agents];
    let mut bin_loads: Vec<f64> = vec![0.0; effective_agents];

    // Assign each group to the least-loaded agent
    for (group_idx, group_impact) in &scored_groups {
        let min_bin = bin_loads
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);
        bins[min_bin].push(*group_idx);
        bin_loads[min_bin] += group_impact;
    }

    // Build AgentAssignment for each non-empty bin
    let mut assignments = Vec::new();
    for (bin_idx, group_indices) in bins.into_iter().enumerate() {
        if group_indices.is_empty() {
            continue;
        }

        let mut tasks = Vec::new();
        let mut files = BTreeSet::new();
        let mut total_impact = 0.0;

        for gi in &group_indices {
            for entity in &groups[*gi] {
                tasks.push(*entity);
                if let Some(fs) = task_files.get(entity) {
                    files.extend(fs.iter().cloned());
                }
                total_impact += impact_scores.get(entity).copied().unwrap_or(0.0);
            }
        }

        // Sort tasks by impact descending within assignment
        tasks.sort_by(|a, b| {
            let ia = impact_scores.get(a).copied().unwrap_or(0.0);
            let ib = impact_scores.get(b).copied().unwrap_or(0.0);
            ib.partial_cmp(&ia).unwrap_or(std::cmp::Ordering::Equal)
        });

        let name = agent_name_from_files(&files, bin_idx);
        assignments.push(AgentAssignment {
            name,
            tasks,
            files,
            total_impact,
        });
    }

    // Sort assignments by total_impact descending
    assignments.sort_by(|a, b| {
        b.total_impact
            .partial_cmp(&a.total_impact)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(assignments)
}

// ---------------------------------------------------------------------------
// Topology Plan (unified Quick + Spectral) (ADR-TOPOLOGY-004)
// ---------------------------------------------------------------------------

/// How the topology plan was computed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanMethod {
    /// Quick: connected-component partition + greedy bin-packing.
    Quick,
    /// Spectral: coupling density matrix + Fiedler partition.
    Spectral,
}

/// Topology pattern classification based on coupling structure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TopologyPattern {
    /// All tasks are independent — maximum parallelism.
    Mesh,
    /// One large coupled group + small satellites.
    Star,
    /// Multiple medium-sized coupled groups.
    Hybrid,
    /// Single agent — all tasks coupled or only 1 agent requested.
    Solo,
    /// Linear dependency chain — minimal parallelism.
    Pipeline,
}

impl TopologyPattern {
    /// Classify from group structure.
    fn classify(groups: &[Vec<EntityId>], agent_count: usize) -> Self {
        if agent_count <= 1 || groups.len() <= 1 {
            return TopologyPattern::Solo;
        }

        let total_tasks: usize = groups.iter().map(|g| g.len()).sum();
        if total_tasks == 0 {
            return TopologyPattern::Solo;
        }

        let max_group = groups.iter().map(|g| g.len()).max().unwrap_or(0);
        let max_ratio = max_group as f64 / total_tasks as f64;

        if groups.len() == total_tasks {
            // Every task is its own group — fully disjoint
            TopologyPattern::Mesh
        } else if max_ratio > 0.7 {
            // One dominant group
            TopologyPattern::Star
        } else if groups.len() == 1 {
            // Single chain
            TopologyPattern::Pipeline
        } else {
            TopologyPattern::Hybrid
        }
    }

    /// As keyword string for store serialization.
    pub fn as_keyword(&self) -> &'static str {
        match self {
            TopologyPattern::Mesh => ":topology.pattern/mesh",
            TopologyPattern::Star => ":topology.pattern/star",
            TopologyPattern::Hybrid => ":topology.pattern/hybrid",
            TopologyPattern::Solo => ":topology.pattern/solo",
            TopologyPattern::Pipeline => ":topology.pattern/pipeline",
        }
    }
}

impl std::fmt::Display for TopologyPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TopologyPattern::Mesh => write!(f, "mesh"),
            TopologyPattern::Star => write!(f, "star"),
            TopologyPattern::Hybrid => write!(f, "hybrid"),
            TopologyPattern::Solo => write!(f, "solo"),
            TopologyPattern::Pipeline => write!(f, "pipeline"),
        }
    }
}

/// A compiled topology plan — the output of the topology compilation pipeline.
///
/// Produced by `quick_plan` (connected-component method) or future spectral
/// partition. Contains everything needed to launch parallel agents with
/// guaranteed disjoint file sets.
#[derive(Clone, Debug)]
pub struct TopologyPlan {
    /// How this plan was computed.
    pub method: PlanMethod,
    /// Per-agent assignments, sorted by total impact descending.
    pub assignments: Vec<AgentAssignment>,
    /// Coupling entropy S(ρ_C) — irreducible coordination complexity.
    pub coupling_entropy: f64,
    /// Parallelizability p = r_eff / n.
    pub parallelizability: f64,
    /// Effective rank r_eff = exp(S).
    pub effective_rank: f64,
    /// Topology pattern classification.
    pub pattern: TopologyPattern,
    /// Total tasks included in the plan.
    pub total_tasks: usize,
    /// Number of connected components in coupling graph.
    pub component_count: usize,
}

impl TopologyPlan {
    /// Verify the disjointness invariant (INV-TOPOLOGY-003):
    /// no file appears in more than one agent's assignment.
    pub fn verify_disjointness(&self) -> Result<(), TopologyError> {
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for assignment in &self.assignments {
            for file in &assignment.files {
                if let Some(other_agent) = seen.get(file.as_str()) {
                    if *other_agent != assignment.name {
                        return Err(TopologyError::DisjointnessViolation { file: file.clone() });
                    }
                }
                seen.insert(file.as_str(), &assignment.name);
            }
        }
        Ok(())
    }

    /// Check that every task appears exactly once across all assignments.
    pub fn verify_completeness(&self, expected_tasks: &BTreeSet<EntityId>) -> bool {
        let mut assigned: BTreeSet<EntityId> = BTreeSet::new();
        for a in &self.assignments {
            for t in &a.tasks {
                assigned.insert(*t);
            }
        }
        assigned == *expected_tasks
    }
}

// ---------------------------------------------------------------------------
// QUICK-3: quick_plan — orchestrate existing functions (ADR-TOPOLOGY-004)
// ---------------------------------------------------------------------------

/// Compute a quick topology plan using connected-component partitioning.
///
/// Pipeline: extract files → compute coupling → partition → balance assign.
/// This is the "AOT compilation" path: static analysis of task metadata,
/// no runtime feedback needed.
///
/// # Errors
///
/// Returns `TopologyError` if:
/// - Fewer than 2 ready tasks (`InsufficientTasks`)
/// - Zero agents requested (`AgentCountZero`)
pub fn quick_plan(
    store: &Store,
    agent_count: usize,
    now: u64,
) -> Result<TopologyPlan, TopologyError> {
    if agent_count == 0 {
        return Err(TopologyError::AgentCountZero);
    }

    // Step 1: Extract file sets for all ready tasks
    let task_files = ready_task_files(store);

    if task_files.len() < 2 {
        return Err(TopologyError::InsufficientTasks {
            found: task_files.len(),
        });
    }

    // Step 2: Compute R(t) impact scores for balancing
    let routing = crate::guidance::compute_routing_from_store(store, now);
    let impact_scores: BTreeMap<EntityId, f64> =
        routing.iter().map(|r| (r.entity, r.impact)).collect();

    // Step 3: Partition into connected components
    let groups = partition_by_file_coupling(&task_files);
    let component_count = groups.len();

    // Step 4: Classify topology pattern
    let pattern = TopologyPattern::classify(&groups, agent_count);

    // Step 5: Balance-assign groups to agents
    let assignments = balance_assign(&groups, &task_files, &impact_scores, agent_count)?;

    // Step 6: Compute coupling entropy
    let (coupling_entropy, effective_rank, parallelizability) =
        compute_coupling_entropy(&task_files);

    let total_tasks = task_files.len();

    let plan = TopologyPlan {
        method: PlanMethod::Quick,
        assignments,
        coupling_entropy,
        parallelizability,
        effective_rank,
        pattern,
        total_tasks,
        component_count,
    };

    // Verify structural invariants
    plan.verify_disjointness()?;

    Ok(plan)
}

/// Compute coupling entropy from task file sets.
///
/// Builds the normalized coupling matrix ρ_C = C / Tr(C) and computes
/// von Neumann entropy S(ρ_C) = -Σ(λᵢ × ln(λᵢ)).
///
/// Returns (entropy, effective_rank, parallelizability).
fn compute_coupling_entropy(task_files: &BTreeMap<EntityId, BTreeSet<String>>) -> (f64, f64, f64) {
    let n = task_files.len();
    if n < 2 {
        return (0.0, 1.0, 1.0);
    }

    let entities: Vec<EntityId> = task_files.keys().copied().collect();
    let coupling = compute_file_coupling(task_files);

    // Build dense coupling matrix with self-coupling on diagonal
    let mut matrix = vec![0.0f64; n * n];
    for i in 0..n {
        // Self-coupling = 1.0 (every task is fully coupled with itself)
        matrix[i * n + i] = 1.0;
        for j in (i + 1)..n {
            if let Some(&score) = coupling.get(&(entities[i], entities[j])) {
                matrix[i * n + j] = score;
                matrix[j * n + i] = score;
            }
        }
    }

    // Normalize to density matrix: ρ = C / Tr(C)
    let trace: f64 = (0..n).map(|i| matrix[i * n + i]).sum();
    if trace <= 0.0 {
        return (0.0, 1.0, 1.0 / n as f64);
    }
    for val in &mut matrix {
        *val /= trace;
    }

    // Compute eigenvalues via Jacobi method
    let dm = crate::query::graph::DenseMatrix {
        rows: n,
        cols: n,
        data: matrix,
    };
    let (eigenvalues, _) = crate::query::graph::symmetric_eigen_decomposition(&dm);

    // Von Neumann entropy: S = -Σ(λᵢ × ln(λᵢ)) for λᵢ > 0
    let entropy = von_neumann_entropy_from_eigenvalues(&eigenvalues);

    // Effective rank: r_eff = exp(S)
    let effective_rank = entropy.exp();

    // Parallelizability: p = r_eff / n
    let parallelizability = effective_rank / n as f64;

    (entropy, effective_rank, parallelizability)
}

/// Compute von Neumann entropy from eigenvalues.
///
/// S(ρ) = -Σ(λᵢ × ln(λᵢ)) for λᵢ > 0.
/// Uses natural log. Returns 0.0 for empty or all-zero eigenvalues.
pub fn von_neumann_entropy_from_eigenvalues(eigenvalues: &[f64]) -> f64 {
    let epsilon = 1e-15;
    let mut entropy = 0.0;
    for &lambda in eigenvalues {
        if lambda > epsilon {
            entropy -= lambda * lambda.ln();
        }
    }
    entropy
}

// ---------------------------------------------------------------------------
// QUICK-4: format_plan — human+agent+json output (API-as-prompt)
// ---------------------------------------------------------------------------

/// Format a topology plan for human display.
///
/// Produces a multi-section output:
/// - Header: method, pattern, metrics
/// - Per-agent: name, tasks, files
/// - Footer: verification status
pub fn format_plan_human(plan: &TopologyPlan, task_titles: &BTreeMap<EntityId, String>) -> String {
    let mut out = String::new();

    // Header
    out.push_str(&format!(
        "topology: {} agents, {} tasks, {} components\n",
        plan.assignments.len(),
        plan.total_tasks,
        plan.component_count,
    ));
    out.push_str(&format!(
        "  method: {:?} | pattern: {} | S(ρ)={:.3} | r_eff={:.1} | p={:.2}\n\n",
        plan.method,
        plan.pattern,
        plan.coupling_entropy,
        plan.effective_rank,
        plan.parallelizability,
    ));

    // Per-agent sections
    for (i, assignment) in plan.assignments.iter().enumerate() {
        out.push_str(&format!(
            "agent {} \"{}\" ({} tasks, impact={:.3}):\n",
            i,
            assignment.name,
            assignment.tasks.len(),
            assignment.total_impact,
        ));

        for task in &assignment.tasks {
            let title = task_titles.get(task).map(|s| s.as_str()).unwrap_or("?");
            // Truncate long titles at a char boundary
            let display_title = if title.len() > 80 {
                let end = title
                    .char_indices()
                    .take_while(|(i, _)| *i <= 77)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(77.min(title.len()));
                format!("{}...", &title[..end])
            } else {
                title.to_string()
            };
            let impact_str = format!("{:.3}", 0.0); // placeholder if no score
            out.push_str(&format!("  [{impact_str}] {display_title}\n"));
        }

        if !assignment.files.is_empty() {
            out.push_str("  files: ");
            let file_list: Vec<&str> = assignment.files.iter().map(|s| s.as_str()).collect();
            out.push_str(&file_list.join(", "));
            out.push('\n');
        }
        out.push('\n');
    }

    // Disjointness verification
    match plan.verify_disjointness() {
        Ok(()) => out.push_str("disjointness: verified ✓\n"),
        Err(e) => out.push_str(&format!("disjointness: VIOLATED — {e}\n")),
    }

    out
}

/// Format a topology plan as compact agent-mode output.
///
/// Optimized for LLM context consumption: structured, terse, spec-language.
pub fn format_plan_agent(plan: &TopologyPlan, task_titles: &BTreeMap<EntityId, String>) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "topology: {agents}a/{tasks}t/{components}c | {pattern} | \
         S={entropy:.2} p={par:.2}\n",
        agents = plan.assignments.len(),
        tasks = plan.total_tasks,
        components = plan.component_count,
        pattern = plan.pattern,
        entropy = plan.coupling_entropy,
        par = plan.parallelizability,
    ));

    for assignment in &plan.assignments {
        out.push_str(&format!(
            "\n[{}] impact={:.2} files={}:\n",
            assignment.name,
            assignment.total_impact,
            assignment.files.len(),
        ));
        for task in &assignment.tasks {
            let title = task_titles.get(task).map(|s| s.as_str()).unwrap_or("?");
            let short = if title.len() > 60 {
                let end = title
                    .char_indices()
                    .take_while(|(i, _)| *i <= 57)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(57.min(title.len()));
                format!("{}...", &title[..end])
            } else {
                title.to_string()
            };
            out.push_str(&format!("  - {short}\n"));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// QUICK-5: emit_seed_files — per-agent seed prompt generation (C7)
// ---------------------------------------------------------------------------

/// Generate a per-agent seed prompt for the topology plan.
///
/// Each agent gets a focused context assembly containing only:
/// - Their assigned tasks (with titles)
/// - Their file set (what they're allowed to edit)
/// - Constraints relevant to their work
/// - The orchestrator protocol (edit only, no build)
pub fn emit_seed_for_agent(
    assignment: &AgentAssignment,
    task_titles: &BTreeMap<EntityId, String>,
    total_agents: usize,
) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "# Agent: {} (1 of {})\n\n",
        assignment.name, total_agents,
    ));

    out.push_str("## Your Tasks\n\n");
    for task in &assignment.tasks {
        let title = task_titles.get(task).map(|s| s.as_str()).unwrap_or("?");
        out.push_str(&format!("- {title}\n"));
    }

    out.push_str("\n## Your Files (ONLY edit these)\n\n");
    for file in &assignment.files {
        out.push_str(&format!("- {file}\n"));
    }

    out.push_str("\n## Protocol\n\n");
    out.push_str("1. Edit ONLY files in your assignment. Other files belong to other agents.\n");
    out.push_str("2. Do NOT run cargo fmt, cargo clippy, or cargo test.\n");
    out.push_str("3. Use `braid observe` to capture decisions and knowledge.\n");
    out.push_str("4. Use `-q` flag on all braid commands to suppress footer.\n");
    out.push_str("5. When done, the orchestrator will verify and commit.\n");

    out
}

// ---------------------------------------------------------------------------
// TOPO-SPEC-DEPS: Spec dependency edge transaction (INV-TOPOLOGY-004)
// ---------------------------------------------------------------------------

/// Parse `:element/traces-to` strings into `:spec/depends-on` Ref datoms.
///
/// Iterates all spec entities with `:element/traces-to`, extracts spec IDs
/// (INV-*, ADR-*, NEG-*) via `parse_spec_refs`, resolves them to store entities
/// via `SpecId`, and returns datoms linking source → target.
///
/// Returns (datoms, resolved_count, unresolved_count).
pub fn spec_dependency_datoms(
    store: &crate::store::Store,
    tx: crate::datom::TxId,
) -> (Vec<crate::datom::Datom>, usize, usize) {
    use crate::datom::*;

    let traces_attr = Attribute::from_keyword(":element/traces-to");
    let spec_depends_attr = Attribute::from_keyword(":spec/depends-on");

    let mut datoms = Vec::new();
    let mut resolved = 0usize;
    let mut unresolved = 0usize;

    for datom in store.attribute_datoms(&traces_attr).iter() {
        if datom.op != Op::Assert {
            continue;
        }
        let source_entity = datom.entity;
        let traces_text = match &datom.value {
            Value::String(s) => s.as_str(),
            _ => continue,
        };

        // Parse spec IDs from the traces-to text
        let spec_refs = crate::task::parse_spec_refs(traces_text);

        for human_id in &spec_refs {
            // Resolve human ID (e.g., "INV-STORE-001") to store ident
            if let Some(spec_id) = crate::spec_id::SpecId::from_any(human_id) {
                let store_ident = spec_id.store_ident();
                let target_entity = EntityId::from_ident(&store_ident);

                // Only create the edge if the target entity exists in the store
                let target_exists = !store.entity_datoms(target_entity).is_empty();
                if target_exists {
                    datoms.push(Datom::new(
                        source_entity,
                        spec_depends_attr.clone(),
                        Value::Ref(target_entity),
                        tx,
                        Op::Assert,
                    ));
                    resolved += 1;
                } else {
                    unresolved += 1;
                }
            }
        }
    }

    (datoms, resolved, unresolved)
}

// ---------------------------------------------------------------------------
// TOPO-INV-COUPLING: Invariant coupling dimension (INV-TOPOLOGY-004)
// ---------------------------------------------------------------------------

/// Compute invariant (spec-structural) coupling between tasks.
///
/// While file coupling (`compute_file_coupling`) measures heuristic overlap
/// (two tasks touching the same file *might* conflict), invariant coupling
/// measures **semantic** overlap: tasks implementing specs that transitively
/// depend on each other are structurally coupled even if they touch different
/// files.
///
/// # Algorithm
///
/// 1. Build a directed graph from `:spec/traces-to` `Value::Ref` datoms.
/// 2. Compute the transitive closure (reachability set) for every spec entity.
/// 3. For each task pair (i, j), let R_i and R_j be the union of reachability
///    sets of their respective spec refs.
/// 4. Coupling = Jaccard(R_i, R_j) = |R_i ∩ R_j| / |R_i ∪ R_j|.
///
/// Returns a map from (task_i, task_j) -> coupling score.
/// Only includes pairs with score > 0. Scores are symmetric and in [0, 1].
///
/// Traces to: spec/19-topology.md INV-TOPOLOGY-004, ADR-TOPOLOGY-004.
pub fn compute_invariant_coupling(
    store: &Store,
    task_specs: &BTreeMap<EntityId, BTreeSet<EntityId>>,
) -> BTreeMap<(EntityId, EntityId), f64> {
    let mut coupling = BTreeMap::new();

    if task_specs.len() < 2 {
        return coupling;
    }

    // Step 1: Build spec dependency DiGraph from :spec/depends-on Ref datoms.
    // Dual-read: also include legacy :spec/traces-to Ref datoms (C1: append-only,
    // old stores have ~113 Ref datoms under :spec/traces-to from prior sessions).
    let spec_depends_attr = crate::datom::Attribute::from_keyword(":spec/depends-on");
    let spec_traces_legacy = crate::datom::Attribute::from_keyword(":spec/traces-to");
    let mut adjacency: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();

    for attr in [&spec_depends_attr, &spec_traces_legacy] {
        for datom in store.attribute_datoms(attr) {
            if datom.op != crate::datom::Op::Assert {
                continue;
            }
            if let crate::datom::Value::Ref(target) = &datom.value {
                adjacency.entry(datom.entity).or_default().insert(*target);
            }
        }
    }

    // Step 2: Compute reachability set for each spec entity via BFS.
    //
    // Cache: spec_entity -> all transitively reachable spec entities (including self).
    let mut reachability_cache: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();

    // Collect all spec entities referenced by any task.
    let all_spec_entities: BTreeSet<EntityId> = task_specs.values().flatten().copied().collect();

    for &spec_e in &all_spec_entities {
        if reachability_cache.contains_key(&spec_e) {
            continue;
        }
        let mut reachable = BTreeSet::new();
        let mut queue = vec![spec_e];
        while let Some(current) = queue.pop() {
            if !reachable.insert(current) {
                continue; // already visited (handles cycles)
            }
            if let Some(neighbors) = adjacency.get(&current) {
                for &neighbor in neighbors {
                    if !reachable.contains(&neighbor) {
                        queue.push(neighbor);
                    }
                }
            }
        }
        reachability_cache.insert(spec_e, reachable);
    }

    // Step 3: For each task, compute the union of reachability sets of its spec refs.
    let task_reachable: BTreeMap<EntityId, BTreeSet<EntityId>> = task_specs
        .iter()
        .map(|(&task_e, spec_refs)| {
            let mut combined = BTreeSet::new();
            for spec_ref in spec_refs {
                if let Some(reach) = reachability_cache.get(spec_ref) {
                    combined.extend(reach.iter().copied());
                } else {
                    // Spec ref not in the graph — include it as its own reachable set.
                    combined.insert(*spec_ref);
                }
            }
            (task_e, combined)
        })
        .collect();

    // Step 4: For each pair of tasks, compute Jaccard similarity.
    let entities: Vec<&EntityId> = task_specs.keys().collect();
    for i in 0..entities.len() {
        let reach_i = match task_reachable.get(entities[i]) {
            Some(r) if !r.is_empty() => r,
            _ => continue,
        };
        for j in (i + 1)..entities.len() {
            let reach_j = match task_reachable.get(entities[j]) {
                Some(r) if !r.is_empty() => r,
                _ => continue,
            };

            let intersection = reach_i.intersection(reach_j).count();
            if intersection == 0 {
                continue;
            }
            let union = reach_i.union(reach_j).count();
            let score = intersection as f64 / union as f64;

            coupling.insert((*entities[i], *entities[j]), score);
            coupling.insert((*entities[j], *entities[i]), score);
        }
    }

    coupling
}

// ===========================================================================
// Coupling Density Matrix & Coordination Entropy (TOPO-DENSITY)
// INV-TOPOLOGY-005, INV-COHERENCE-001
// ===========================================================================

/// Analysis of the coupling structure for topology planning.
///
/// The normalized coupling matrix ρ_C = C / Tr(C) is a density matrix.
/// Its von Neumann entropy S(ρ_C) = -Tr(ρ_C log ρ_C) equals the
/// irreducible coordination complexity. The effective rank
/// r_eff = exp(S) gives the optimal number of independent agent groups.
/// Parallelizability p = r_eff / n (Amdahl's law for topology).
#[derive(Clone, Debug)]
pub struct CouplingAnalysis {
    /// Normalized coupling (density) matrix. Rows/columns ordered by entities.
    pub rho: Vec<Vec<f64>>,
    /// Entity ordering (matches rows/columns of rho).
    pub entities: Vec<EntityId>,
    /// Eigenvalues of the density matrix (non-negative, sum to 1).
    pub eigenvalues: Vec<f64>,
    /// Von Neumann entropy S(ρ) = -Σ λᵢ log λᵢ.
    pub entropy: f64,
    /// Effective rank r_eff = exp(S).
    pub effective_rank: f64,
    /// Parallelizability p = r_eff / n.
    pub parallelizability: f64,
}

/// Build a coupling density matrix from pairwise coupling scores.
///
/// Takes the output of `compute_file_coupling` or `compute_invariant_coupling`
/// (a map of entity pairs to coupling scores) and constructs:
/// 1. The symmetric coupling matrix C (diagonal = 1.0 for self-coupling)
/// 2. The normalized density matrix ρ = C / Tr(C)
/// 3. Eigenvalues via power iteration for the 2×2..N×N case
/// 4. Entropy, effective rank, parallelizability
///
/// INV-COHERENCE-001: ρ satisfies PSD, unit-trace, symmetric.
pub fn coupling_density_matrix(
    coupling: &BTreeMap<(EntityId, EntityId), f64>,
    entities: &[EntityId],
) -> CouplingAnalysis {
    let n = entities.len();

    if n == 0 {
        return CouplingAnalysis {
            rho: vec![],
            entities: vec![],
            eigenvalues: vec![],
            entropy: 0.0,
            effective_rank: 0.0,
            parallelizability: 0.0,
        };
    }

    // Build entity index map
    let idx: BTreeMap<EntityId, usize> =
        entities.iter().enumerate().map(|(i, e)| (*e, i)).collect();

    // Build coupling matrix C (symmetric, diagonal = 1.0)
    let mut c = vec![vec![0.0f64; n]; n];
    for (i, row) in c.iter_mut().enumerate().take(n) {
        row[i] = 1.0; // Self-coupling
    }
    for ((e1, e2), &score) in coupling {
        if let (Some(&i), Some(&j)) = (idx.get(e1), idx.get(e2)) {
            c[i][j] = score;
            c[j][i] = score; // Ensure symmetry
        }
    }

    // Normalize: ρ = C / Tr(C)
    let trace: f64 = (0..n).map(|i| c[i][i]).sum();
    let mut rho = vec![vec![0.0f64; n]; n];
    if trace > 0.0 {
        for i in 0..n {
            for j in 0..n {
                rho[i][j] = c[i][j] / trace;
            }
        }
    }

    // Compute eigenvalues via the existing infrastructure
    let eigenvalues = symmetric_eigenvalues(&rho, n);

    // Von Neumann entropy: S = -Σ λᵢ log₂(λᵢ) for λᵢ > 0
    let entropy = von_neumann_entropy_from_eigenvalues(&eigenvalues);

    // Effective rank: r_eff = e^S (von Neumann entropy uses natural log)
    let effective_rank = entropy.exp();

    // Parallelizability: p = r_eff / n
    let parallelizability = if n > 0 {
        (effective_rank / n as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    CouplingAnalysis {
        rho,
        entities: entities.to_vec(),
        eigenvalues,
        entropy,
        effective_rank,
        parallelizability,
    }
}

/// Compute eigenvalues of a real symmetric matrix using Jacobi iteration.
///
/// For small matrices (n ≤ 20 in topology), this is efficient and numerically stable.
/// Returns eigenvalues sorted descending.
fn symmetric_eigenvalues(matrix: &[Vec<f64>], n: usize) -> Vec<f64> {
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![matrix[0][0]];
    }

    // Copy matrix for iteration
    let mut a = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in 0..n {
            a[i][j] = matrix[i][j];
        }
    }

    // Jacobi eigenvalue iteration (converges for symmetric matrices)
    let max_iter = 100 * n * n;
    for _ in 0..max_iter {
        // Find largest off-diagonal element
        let mut max_off = 0.0f64;
        let mut p = 0;
        let mut q = 1;
        for (i, row) in a.iter().enumerate().take(n) {
            for (j, &val) in row.iter().enumerate().take(n).skip(i + 1) {
                if val.abs() > max_off {
                    max_off = val.abs();
                    p = i;
                    q = j;
                }
            }
        }

        if max_off < 1e-12 {
            break; // Converged
        }

        // Compute rotation angle
        let theta = if (a[p][p] - a[q][q]).abs() < 1e-15 {
            std::f64::consts::FRAC_PI_4
        } else {
            0.5 * ((2.0 * a[p][q]) / (a[p][p] - a[q][q])).atan()
        };

        let cos_t = theta.cos();
        let sin_t = theta.sin();

        // Apply Jacobi rotation
        let mut new_a = a.clone();
        for i in 0..n {
            if i != p && i != q {
                new_a[i][p] = cos_t * a[i][p] + sin_t * a[i][q];
                new_a[p][i] = new_a[i][p];
                new_a[i][q] = -sin_t * a[i][p] + cos_t * a[i][q];
                new_a[q][i] = new_a[i][q];
            }
        }
        new_a[p][p] =
            cos_t * cos_t * a[p][p] + 2.0 * sin_t * cos_t * a[p][q] + sin_t * sin_t * a[q][q];
        new_a[q][q] =
            sin_t * sin_t * a[p][p] - 2.0 * sin_t * cos_t * a[p][q] + cos_t * cos_t * a[q][q];
        new_a[p][q] = 0.0;
        new_a[q][p] = 0.0;

        a = new_a;
    }

    // Extract diagonal as eigenvalues
    let mut eigenvalues: Vec<f64> = (0..n).map(|i| a[i][i]).collect();
    eigenvalues.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    eigenvalues
}

/// Composite coupling: merge file coupling and invariant coupling.
///
/// Weighted combination: w_f * file_coupling + w_i * invariant_coupling
/// where w_f = 0.65 (heuristic) and w_i = 0.35 (semantic).
/// TOPO-COMPOSITE: Five-dimensional coupling weights (INV-TOPOLOGY-004).
///
/// Weights form a semilattice — they only grow through observation.
/// At Stage 0b: w_file=0.50, w_invariant=0.35, w_schema=0, w_causal=0, w_historical=0.15.
#[derive(Clone, Debug)]
pub struct CouplingWeights {
    /// w_f: File coupling weight (shared source files).
    pub file: f64,
    /// w_i: Invariant coupling weight (shared spec references).
    pub invariant: f64,
    /// w_s: Schema coupling weight (shared schema attributes). Stage 2+.
    pub schema: f64,
    /// w_c: Causal coupling weight (W_α dependency). Stage 2+.
    pub causal: f64,
    /// w_h: Historical coupling weight (past conflict data).
    pub historical: f64,
}

impl Default for CouplingWeights {
    /// Stage 0b defaults: [0.50, 0.35, 0, 0, 0.15].
    fn default() -> Self {
        Self {
            file: 0.50,
            invariant: 0.35,
            schema: 0.0,
            causal: 0.0,
            historical: 0.15,
        }
    }
}

impl CouplingWeights {
    /// Load weights from store datom, or fall back to defaults.
    pub fn from_store(store: &Store) -> Self {
        let attr = Attribute::from_keyword(":topology/coupling-weights");
        let datoms = store.attribute_datoms(&attr);
        if let Some(d) = datoms.last() {
            if let Value::String(s) = &d.value {
                if let Ok(parsed) = serde_json::from_str::<Vec<f64>>(s.as_str()) {
                    if parsed.len() == 5 {
                        return Self {
                            file: parsed[0],
                            invariant: parsed[1],
                            schema: parsed[2],
                            causal: parsed[3],
                            historical: parsed[4],
                        };
                    }
                }
            }
        }
        Self::default()
    }
}

/// Compute composite coupling from multiple dimensions with learnable weights.
///
/// INV-TOPOLOGY-004: Coupling = Σ wᵢ × sᵢ where s = [file, invariant, schema, causal, historical].
/// Result clamped to [0, 1] per pair. Zero-weight dimensions contribute nothing.
pub fn composite_coupling(
    file_coupling: &BTreeMap<(EntityId, EntityId), f64>,
    inv_coupling: &BTreeMap<(EntityId, EntityId), f64>,
) -> BTreeMap<(EntityId, EntityId), f64> {
    composite_coupling_weighted(
        file_coupling,
        inv_coupling,
        &BTreeMap::new(), // historical (empty at Stage 0b)
        &CouplingWeights::default(),
    )
}

/// Weighted composite coupling with explicit weights and historical dimension.
pub fn composite_coupling_weighted(
    file_coupling: &BTreeMap<(EntityId, EntityId), f64>,
    inv_coupling: &BTreeMap<(EntityId, EntityId), f64>,
    historical_coupling: &BTreeMap<(EntityId, EntityId), f64>,
    weights: &CouplingWeights,
) -> BTreeMap<(EntityId, EntityId), f64> {
    let mut all_pairs: BTreeSet<(EntityId, EntityId)> = BTreeSet::new();
    for key in file_coupling.keys() {
        all_pairs.insert(*key);
    }
    for key in inv_coupling.keys() {
        all_pairs.insert(*key);
    }
    for key in historical_coupling.keys() {
        all_pairs.insert(*key);
    }

    let mut result = BTreeMap::new();
    for pair in all_pairs {
        let f_score = file_coupling.get(&pair).unwrap_or(&0.0);
        let i_score = inv_coupling.get(&pair).unwrap_or(&0.0);
        let h_score = historical_coupling.get(&pair).unwrap_or(&0.0);
        // Schema and causal are zero at Stage 0b
        let combined =
            weights.file * f_score + weights.invariant * i_score + weights.historical * h_score;
        if combined > 0.0 {
            result.insert(pair, combined.clamp(0.0, 1.0));
        }
    }
    result
}

// ===========================================================================
// TOPO-SPECTRAL: Spectral topology selection (INV-TOPOLOGY-005, ADR-TOPOLOGY-004)
// ===========================================================================

/// Select the optimal topology pattern from the coupling density matrix.
///
/// Uses the parallelizability coefficient `p = r_eff / n` to classify:
/// - p > 0.8  => All agents work independently (Mesh)
/// - 0.3 < p <= 0.8 => Hybrid — Fiedler partition into clusters
/// - p <= 0.3 => Highly coupled — Pipeline (linear chain) or Star (hub)
///
/// For the high-coupling case (p <= 0.3), distinguishes Star from Pipeline by
/// examining the eigenvalue distribution: if the largest eigenvalue dominates
/// (> 0.7 of trace), one hub connects to everything (Star); otherwise the
/// coupling spreads more evenly (Pipeline).
///
/// Deterministic: same input always produces the same output (INV-TOPOLOGY-005).
///
/// Traces to: spec/19-topology.md INV-TOPOLOGY-005, ADR-TOPOLOGY-004.
pub fn select_topology(analysis: &CouplingAnalysis, agent_count: usize) -> TopologyPattern {
    if agent_count <= 1 || analysis.entities.len() <= 1 {
        return TopologyPattern::Solo;
    }

    let p = analysis.parallelizability;

    if p > 0.8 {
        // Nearly independent tasks — full parallelism
        TopologyPattern::Mesh
    } else if p > 0.3 {
        // Moderate coupling — partition into clusters
        TopologyPattern::Hybrid
    } else {
        // High coupling (p <= 0.3) — sequential or star
        // Distinguish Pipeline vs Star by examining eigenvalue distribution.
        // If the largest eigenvalue dominates (> 0.7 of trace), one hub
        // connects to everything => Star. Otherwise => Pipeline.
        if !analysis.eigenvalues.is_empty() {
            let max_ev = analysis.eigenvalues[0]; // sorted descending
            if max_ev > 0.7 {
                TopologyPattern::Star
            } else {
                TopologyPattern::Pipeline
            }
        } else {
            TopologyPattern::Pipeline
        }
    }
}

/// Recursively partition a coupling matrix into `k` groups using Fiedler bisection.
///
/// The Fiedler vector is the eigenvector corresponding to the second-smallest
/// eigenvalue of the graph Laplacian L = D - A. Partitioning on the sign of
/// the Fiedler vector yields a spectral bisection that minimizes the normalized
/// cut (Cheeger inequality).
///
/// Algorithm:
/// 1. Start with all indices in one group.
/// 2. Select the largest group with size >= 2.
/// 3. Build the graph Laplacian for the induced submatrix.
/// 4. Compute the Fiedler vector (2nd eigenvector of L).
/// 5. Split indices by sign of Fiedler vector components.
/// 6. Repeat from step 2 until `k` groups are achieved.
///
/// Returns at most `k` groups. Groups may be fewer than `k` if the matrix
/// structure doesn't support further bisection (e.g., fully connected).
///
/// Deterministic: same input always produces the same output (INV-TOPOLOGY-005).
///
/// Traces to: spec/19-topology.md INV-TOPOLOGY-005, ADR-TOPOLOGY-004.
pub fn spectral_partition(rho: &[Vec<f64>], k: usize) -> Vec<Vec<usize>> {
    let n = rho.len();
    if n == 0 || k == 0 {
        return vec![];
    }
    if k == 1 || n == 1 {
        return vec![(0..n).collect()];
    }

    // Start with all indices in one group
    let initial: Vec<usize> = (0..n).collect();
    let mut groups = vec![initial];

    // Recursively bisect the largest group until we have k groups
    while groups.len() < k {
        // Find the largest group that can be bisected (size >= 2)
        let largest_idx = groups
            .iter()
            .enumerate()
            .filter(|(_, g)| g.len() >= 2)
            .max_by_key(|(_, g)| g.len())
            .map(|(i, _)| i);

        let largest_idx = match largest_idx {
            Some(i) => i,
            None => break, // No group can be bisected further
        };

        let group = groups.remove(largest_idx);
        let (left, right) = fiedler_bisect(rho, &group);

        if left.is_empty() || right.is_empty() {
            // Bisection failed — put group back
            groups.push(group);
            break;
        }

        groups.push(left);
        groups.push(right);
    }

    // Sort groups by size descending, then by content for determinism
    groups.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    groups
}

/// Bisect a subset of indices using the Fiedler vector of the induced subgraph.
///
/// Builds the graph Laplacian for the submatrix induced by `indices`,
/// computes the Fiedler vector (2nd eigenvector of L), and splits on sign.
///
/// Returns (positive_group, negative_group). If the bisection is trivial
/// (all same sign), falls back to splitting in half by index order.
pub fn fiedler_bisect(rho: &[Vec<f64>], indices: &[usize]) -> (Vec<usize>, Vec<usize>) {
    let m = indices.len();
    if m < 2 {
        return (indices.to_vec(), vec![]);
    }

    // Build the induced submatrix
    let mut sub = vec![vec![0.0f64; m]; m];
    for (si, &i) in indices.iter().enumerate() {
        for (sj, &j) in indices.iter().enumerate() {
            sub[si][sj] = rho[i][j];
        }
    }

    // Build graph Laplacian: L = D - A
    // Diagonal = degree (sum of row, excluding self), off-diagonal = -coupling
    let mut laplacian_data = vec![0.0f64; m * m];
    for i in 0..m {
        let mut degree = 0.0;
        for j in 0..m {
            if i != j {
                degree += sub[i][j];
                laplacian_data[i * m + j] = -sub[i][j];
            }
        }
        laplacian_data[i * m + i] = degree;
    }

    // Compute eigenvalues and eigenvectors
    let dm = crate::query::graph::DenseMatrix {
        rows: m,
        cols: m,
        data: laplacian_data,
    };
    let (eigenvalues, eigenvectors) = crate::query::graph::symmetric_eigen_decomposition(&dm);

    // The Fiedler vector is the eigenvector for the 2nd smallest eigenvalue.
    // eigenvalues from symmetric_eigen_decomposition are sorted ascending,
    // so the 2nd smallest is at index 1 (matching the convention in query::graph::fiedler).
    if eigenvalues.len() < 2 {
        return (indices.to_vec(), vec![]);
    }

    // Check that algebraic connectivity (2nd smallest eigenvalue) is non-trivial
    let fiedler_idx = 1;
    let algebraic_connectivity = eigenvalues[fiedler_idx];
    if algebraic_connectivity.abs() < 1e-10 {
        // Graph is disconnected or nearly so — fall back to half-split
        let mid = m / 2;
        return (indices[..mid].to_vec(), indices[mid..].to_vec());
    }

    // Extract Fiedler vector (column fiedler_idx of eigenvectors)
    let fiedler_vec: Vec<f64> = (0..m)
        .map(|row| eigenvectors.data[row * m + fiedler_idx])
        .collect();

    // Split on sign of Fiedler vector
    let mut positive = Vec::new();
    let mut negative = Vec::new();
    for (si, &component) in fiedler_vec.iter().enumerate() {
        if component >= 0.0 {
            positive.push(indices[si]);
        } else {
            negative.push(indices[si]);
        }
    }

    // If all ended up on one side, split in half
    if positive.is_empty() || negative.is_empty() {
        let mid = m / 2;
        return (indices[..mid].to_vec(), indices[mid..].to_vec());
    }

    (positive, negative)
}

/// Measure partition quality: ratio of intra-cluster coupling to total coupling.
///
/// Quality = 1 - (inter_cluster_coupling / total_coupling).
/// Returns a value in [0, 1] where:
/// - 1.0 = perfect partition (zero inter-cluster coupling)
/// - 0.0 = worst partition (all coupling is inter-cluster)
///
/// For partitions with zero total coupling, returns 1.0 (trivially perfect).
///
/// Traces to: spec/19-topology.md INV-TOPOLOGY-005.
pub fn partition_quality(partition: &[Vec<usize>], coupling: &[Vec<f64>]) -> f64 {
    if partition.is_empty() || coupling.is_empty() {
        return 1.0;
    }

    let n = coupling.len();

    // Build cluster membership: index -> cluster_id
    let mut cluster_of = vec![0usize; n];
    for (cluster_id, group) in partition.iter().enumerate() {
        for &idx in group {
            if idx < n {
                cluster_of[idx] = cluster_id;
            }
        }
    }

    let mut total_coupling = 0.0;
    let mut inter_cluster_coupling = 0.0;

    for i in 0..n {
        for j in (i + 1)..n {
            let c = coupling[i][j].abs();
            if c > 1e-15 {
                total_coupling += c;
                if cluster_of[i] != cluster_of[j] {
                    inter_cluster_coupling += c;
                }
            }
        }
    }

    if total_coupling < 1e-15 {
        return 1.0; // No coupling at all — trivially perfect
    }

    1.0 - (inter_cluster_coupling / total_coupling)
}

// =============================================================================
// CALM Classification (ADR-TOPOLOGY-002, INV-TOPOLOGY-006)
// =============================================================================

/// CALM tier classification: monotonic (parallel) vs non-monotonic (barrier).
///
/// The CALM theorem (Consistency As Logical Monotonicity) partitions operations:
/// - **Tier M** (monotonic): can execute without coordination — e.g., editing code.
/// - **Tier NM** (non-monotonic): requires a sync barrier — e.g., verification,
///   merge, schema change.
///
/// Traces to: ADR-TOPOLOGY-002, INV-TOPOLOGY-006.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalmTier {
    /// Monotonic parallel: can execute without coordination (editing code).
    MonotonicParallel,
    /// Non-monotonic barrier: requires sync (verification, merge, schema change).
    NonMonotonicBarrier,
}

/// A phase in an execution plan — a group of tasks sharing the same CALM tier.
///
/// Consecutive same-tier tasks are grouped into a single phase.
/// Phase boundaries occur at tier transitions.
///
/// Traces to: ADR-TOPOLOGY-002, INV-TOPOLOGY-006.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phase {
    /// The CALM tier for all tasks in this phase.
    pub tier: CalmTier,
    /// The tasks assigned to this phase.
    pub tasks: Vec<EntityId>,
}

/// Non-monotonic keywords that indicate a task requires a sync barrier.
///
/// Tasks whose titles contain these keywords (case-insensitive) are classified
/// as Tier NM. The set covers: verification, merge, schema changes, validation,
/// cascading operations, and migration.
const NM_KEYWORDS: &[&str] = &[
    "merge",
    "verify",
    "test",
    "schema",
    "migrate",
    "cascade",
    "validate",
    "migration",
    "verification",
];

/// Classify a task's CALM tier from its title text.
///
/// The classification heuristic:
/// 1. If the title contains any non-monotonic keyword (case-insensitive),
///    classify as `NonMonotonicBarrier`.
/// 2. Otherwise, classify as `MonotonicParallel` (default — most tasks are edits).
///
/// Tasks with `FILE:` markers are always `MonotonicParallel` since they represent
/// concrete file-editing work regardless of other keywords in the title.
///
/// Traces to: ADR-TOPOLOGY-002, INV-TOPOLOGY-006.
pub fn classify_task_phase(title: &str) -> CalmTier {
    let lower = title.to_lowercase();

    // FILE: marker signals a concrete edit task — always Tier M.
    if lower.contains("file:") || lower.contains("files:") {
        return CalmTier::MonotonicParallel;
    }

    // Check for non-monotonic keywords.
    for kw in NM_KEYWORDS {
        if lower.contains(kw) {
            return CalmTier::NonMonotonicBarrier;
        }
    }

    // Default: most tasks are edits (Tier M).
    CalmTier::MonotonicParallel
}

/// Build a phase plan from classified tasks.
///
/// Groups consecutive same-tier tasks into `Phase` structs.
/// The input order determines grouping: switching from M to NM (or vice versa)
/// starts a new phase.
///
/// Empty input produces an empty plan.
///
/// Traces to: ADR-TOPOLOGY-002, INV-TOPOLOGY-006.
pub fn phase_plan(tasks: &[(EntityId, CalmTier)]) -> Vec<Phase> {
    if tasks.is_empty() {
        return Vec::new();
    }

    let mut phases: Vec<Phase> = Vec::new();
    let mut current_tier = tasks[0].1;
    let mut current_tasks = vec![tasks[0].0];

    for &(entity, tier) in &tasks[1..] {
        if tier == current_tier {
            current_tasks.push(entity);
        } else {
            phases.push(Phase {
                tier: current_tier,
                tasks: std::mem::take(&mut current_tasks),
            });
            current_tier = tier;
            current_tasks.push(entity);
        }
    }

    // Flush the final group.
    phases.push(Phase {
        tier: current_tier,
        tasks: current_tasks,
    });

    phases
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Helper ===
    fn files(paths: &[&str]) -> BTreeSet<String> {
        paths.iter().map(|s| s.to_string()).collect()
    }

    fn entity(name: &str) -> EntityId {
        EntityId::from_ident(name)
    }

    // === File extraction tests ===

    #[test]
    fn extract_files_from_file_marker() {
        let title = "Fix bug. FILE: crates/braid-kernel/src/guidance.rs. ACCEPTANCE: compiles.";
        let files = extract_task_files(title);
        assert!(files.contains("crates/braid-kernel/src/guidance.rs"));
    }

    #[test]
    fn extract_files_from_inline_path() {
        let title = "Update crates/braid/src/commands/task.rs for CBV";
        let files = extract_task_files(title);
        assert!(files.contains("crates/braid/src/commands/task.rs"));
    }

    #[test]
    fn extract_files_no_files() {
        let title = "Abstract design task with no file references";
        let files = extract_task_files(title);
        assert!(files.is_empty());
    }

    #[test]
    fn extract_files_multiple_inline() {
        let title = "Fix crates/braid-kernel/src/topology.rs and crates/braid-kernel/src/error.rs";
        let result = extract_task_files(title);
        assert_eq!(result.len(), 2);
        assert!(result.contains("crates/braid-kernel/src/topology.rs"));
        assert!(result.contains("crates/braid-kernel/src/error.rs"));
    }

    #[test]
    fn extract_files_marker_with_multiple() {
        let title = "FILES: crates/a/src/b.rs, crates/c/src/d.rs ACCEPTANCE: tests pass";
        let result = extract_task_files(title);
        assert!(result.contains("crates/a/src/b.rs"));
        assert!(result.contains("crates/c/src/d.rs"));
    }

    // === Coupling tests ===

    #[test]
    fn coupling_shared_file() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/a.rs"]));
        task_files.insert(e2, files(&["crates/a.rs"]));
        let coupling = compute_file_coupling(&task_files);
        assert_eq!(*coupling.get(&(e1, e2)).unwrap(), 1.0);
    }

    #[test]
    fn coupling_disjoint_files() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/a.rs"]));
        task_files.insert(e2, files(&["crates/b.rs"]));
        let coupling = compute_file_coupling(&task_files);
        assert!(coupling.is_empty());
    }

    #[test]
    fn coupling_partial_overlap() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/a.rs", "crates/b.rs"]));
        task_files.insert(e2, files(&["crates/b.rs", "crates/c.rs"]));
        let coupling = compute_file_coupling(&task_files);
        // Jaccard: |{b}| / |{a,b,c}| = 1/3
        let score = *coupling.get(&(e1, e2)).unwrap();
        assert!((score - 1.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn coupling_symmetric() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/a.rs", "crates/b.rs"]));
        task_files.insert(e2, files(&["crates/b.rs"]));
        let coupling = compute_file_coupling(&task_files);
        assert_eq!(
            coupling.get(&(e1, e2)),
            coupling.get(&(e2, e1)),
            "coupling must be symmetric"
        );
    }

    #[test]
    fn coupling_empty_file_set_excluded() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/a.rs"]));
        task_files.insert(e2, BTreeSet::new()); // no files
        let coupling = compute_file_coupling(&task_files);
        assert!(
            coupling.is_empty(),
            "empty file set should produce no coupling"
        );
    }

    // === Partition tests ===

    #[test]
    fn partition_disjoint_tasks() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let e3 = entity(":task/t-3");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/a.rs"]));
        task_files.insert(e2, files(&["crates/b.rs"]));
        task_files.insert(e3, files(&["crates/a.rs"])); // shares with e1
        let groups = partition_by_file_coupling(&task_files);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 2); // e1 + e3
        assert_eq!(groups[1].len(), 1); // e2
    }

    #[test]
    fn partition_all_disjoint() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/a.rs"]));
        task_files.insert(e2, files(&["crates/b.rs"]));
        let groups = partition_by_file_coupling(&task_files);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn partition_all_coupled() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let e3 = entity(":task/t-3");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/shared.rs"]));
        task_files.insert(e2, files(&["crates/shared.rs"]));
        task_files.insert(e3, files(&["crates/shared.rs"]));
        let groups = partition_by_file_coupling(&task_files);
        assert_eq!(groups.len(), 1, "all coupled tasks → single group");
        assert_eq!(groups[0].len(), 3);
    }

    #[test]
    fn partition_empty() {
        let task_files: BTreeMap<EntityId, BTreeSet<String>> = BTreeMap::new();
        let groups = partition_by_file_coupling(&task_files);
        assert!(groups.is_empty());
    }

    #[test]
    fn partition_transitive_coupling() {
        // a-b share file1, b-c share file2, so a-b-c are one component
        let e_a = entity(":task/a");
        let e_b = entity(":task/b");
        let e_c = entity(":task/c");
        let e_d = entity(":task/d");
        let mut task_files = BTreeMap::new();
        task_files.insert(e_a, files(&["crates/x.rs"]));
        task_files.insert(e_b, files(&["crates/x.rs", "crates/y.rs"]));
        task_files.insert(e_c, files(&["crates/y.rs"]));
        task_files.insert(e_d, files(&["crates/z.rs"])); // isolated
        let groups = partition_by_file_coupling(&task_files);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 3); // a + b + c
        assert_eq!(groups[1].len(), 1); // d
    }

    // === Agent naming tests ===

    #[test]
    fn agent_name_from_files_empty() {
        let f = BTreeSet::new();
        assert_eq!(agent_name_from_files(&f, 0), "agent-0");
    }

    #[test]
    fn agent_name_from_single_file() {
        let f = files(&["crates/braid-kernel/src/topology.rs"]);
        let name = agent_name_from_files(&f, 0);
        // Should pick parent dir "braid-kernel" (or similar meaningful stem)
        assert!(!name.is_empty());
        assert_ne!(name, "agent-0");
    }

    #[test]
    fn agent_name_from_multiple_files() {
        let f = files(&[
            "crates/braid-kernel/src/topology.rs",
            "crates/braid-kernel/src/error.rs",
            "crates/braid/src/commands/mod.rs",
        ]);
        let name = agent_name_from_files(&f, 0);
        assert!(!name.is_empty());
        assert!(!name.contains("src"), "should skip 'src' generic dir");
    }

    // === Balance assign tests ===

    #[test]
    fn balance_assign_zero_agents() {
        let groups = vec![vec![entity(":task/t-1")]];
        let task_files = BTreeMap::new();
        let impacts = BTreeMap::new();
        let result = balance_assign(&groups, &task_files, &impacts, 0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), TopologyError::AgentCountZero);
    }

    #[test]
    fn balance_assign_empty_groups() {
        let groups: Vec<Vec<EntityId>> = Vec::new();
        let task_files = BTreeMap::new();
        let impacts = BTreeMap::new();
        let result = balance_assign(&groups, &task_files, &impacts, 2).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn balance_assign_single_agent() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let groups = vec![vec![e1], vec![e2]];
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/a.rs"]));
        task_files.insert(e2, files(&["crates/b.rs"]));
        let mut impacts = BTreeMap::new();
        impacts.insert(e1, 0.5);
        impacts.insert(e2, 0.3);
        let result = balance_assign(&groups, &task_files, &impacts, 1).unwrap();
        assert_eq!(result.len(), 1, "single agent gets all groups");
        assert_eq!(result[0].tasks.len(), 2);
    }

    #[test]
    fn balance_assign_two_agents_two_groups() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let groups = vec![vec![e1], vec![e2]];
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/a.rs"]));
        task_files.insert(e2, files(&["crates/b.rs"]));
        let mut impacts = BTreeMap::new();
        impacts.insert(e1, 0.8);
        impacts.insert(e2, 0.3);
        let result = balance_assign(&groups, &task_files, &impacts, 2).unwrap();
        assert_eq!(result.len(), 2, "two agents, two groups");
        // Each agent gets one task
        assert_eq!(result[0].tasks.len(), 1);
        assert_eq!(result[1].tasks.len(), 1);
    }

    #[test]
    fn balance_assign_more_agents_than_groups() {
        let e1 = entity(":task/t-1");
        let groups = vec![vec![e1]];
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/a.rs"]));
        let mut impacts = BTreeMap::new();
        impacts.insert(e1, 0.5);
        let result = balance_assign(&groups, &task_files, &impacts, 5).unwrap();
        // Can't have more assignments than groups
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn balance_assign_disjoint_files() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let groups = vec![vec![e1], vec![e2]];
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, files(&["crates/a.rs"]));
        task_files.insert(e2, files(&["crates/b.rs"]));
        // Use different impacts to ensure 2 separate bins
        let mut impacts = BTreeMap::new();
        impacts.insert(e1, 0.8);
        impacts.insert(e2, 0.3);
        let result = balance_assign(&groups, &task_files, &impacts, 2).unwrap();
        assert_eq!(result.len(), 2, "should have 2 agents");
        // Verify file disjointness across all pairs
        for i in 0..result.len() {
            for j in (i + 1)..result.len() {
                let fi: BTreeSet<_> = result[i].files.iter().collect();
                let fj: BTreeSet<_> = result[j].files.iter().collect();
                assert!(
                    fi.is_disjoint(&fj),
                    "agent file sets must be disjoint: {} vs {}",
                    result[i].name,
                    result[j].name,
                );
            }
        }
    }

    // === Topology pattern tests ===

    #[test]
    fn pattern_solo_single_agent() {
        let groups = vec![vec![entity(":task/t-1"), entity(":task/t-2")]];
        assert_eq!(TopologyPattern::classify(&groups, 1), TopologyPattern::Solo);
    }

    #[test]
    fn pattern_mesh_all_disjoint() {
        let groups = vec![
            vec![entity(":task/t-1")],
            vec![entity(":task/t-2")],
            vec![entity(":task/t-3")],
        ];
        assert_eq!(TopologyPattern::classify(&groups, 3), TopologyPattern::Mesh);
    }

    #[test]
    fn pattern_star_dominant_group() {
        let groups = vec![
            vec![
                entity(":t/1"),
                entity(":t/2"),
                entity(":t/3"),
                entity(":t/4"),
            ],
            vec![entity(":t/5")],
        ];
        assert_eq!(TopologyPattern::classify(&groups, 2), TopologyPattern::Star);
    }

    // === Coupling entropy tests ===

    #[test]
    fn entropy_disjoint_tasks_is_maximal() {
        let mut task_files = BTreeMap::new();
        for i in 0..4 {
            task_files.insert(
                entity(&format!(":task/t-{i}")),
                files(&[&format!("crates/f{i}.rs")]),
            );
        }
        let (entropy, r_eff, parallelizability) = compute_coupling_entropy(&task_files);
        // Fully disjoint → coupling matrix is identity → max entropy
        assert!(entropy > 0.0, "entropy should be positive");
        assert!(
            r_eff >= 3.5,
            "effective rank should be ~4 for 4 disjoint tasks"
        );
        assert!(parallelizability > 0.8, "parallelizability should be high");
    }

    #[test]
    fn entropy_fully_coupled_is_minimal() {
        let mut task_files = BTreeMap::new();
        for i in 0..4 {
            task_files.insert(
                entity(&format!(":task/t-{i}")),
                files(&["crates/shared.rs"]),
            );
        }
        let (entropy, r_eff, _) = compute_coupling_entropy(&task_files);
        // Fully coupled → one dominant eigenvalue → low entropy
        assert!(
            entropy < 2.0,
            "entropy should be low for fully coupled, got {entropy}"
        );
        assert!(r_eff < 3.0, "effective rank should be low, got {r_eff}");
    }

    #[test]
    fn entropy_single_task() {
        let mut task_files = BTreeMap::new();
        task_files.insert(entity(":task/t-1"), files(&["crates/a.rs"]));
        let (entropy, r_eff, par) = compute_coupling_entropy(&task_files);
        assert_eq!(entropy, 0.0);
        assert_eq!(r_eff, 1.0);
        assert_eq!(par, 1.0);
    }

    #[test]
    fn von_neumann_entropy_identity_matrix() {
        // n×n identity → uniform eigenvalues 1/n → max entropy ln(n)
        let n = 4;
        let eigenvalues: Vec<f64> = vec![1.0 / n as f64; n];
        let entropy = von_neumann_entropy_from_eigenvalues(&eigenvalues);
        let expected = (n as f64).ln();
        assert!(
            (entropy - expected).abs() < 1e-10,
            "entropy of uniform dist should be ln({n}), got {entropy}"
        );
    }

    #[test]
    fn von_neumann_entropy_pure_state() {
        // Single eigenvalue = 1.0 → entropy = 0
        let eigenvalues = vec![1.0, 0.0, 0.0, 0.0];
        let entropy = von_neumann_entropy_from_eigenvalues(&eigenvalues);
        assert!(
            entropy.abs() < 1e-10,
            "pure state should have zero entropy, got {entropy}"
        );
    }

    // === Format tests ===

    #[test]
    fn format_plan_human_includes_pattern() {
        let plan = TopologyPlan {
            method: PlanMethod::Quick,
            assignments: vec![AgentAssignment {
                name: "test-agent".to_string(),
                tasks: vec![entity(":task/t-1")],
                files: files(&["crates/a.rs"]),
                total_impact: 0.5,
            }],
            coupling_entropy: 1.386,
            parallelizability: 0.75,
            effective_rank: 3.0,
            pattern: TopologyPattern::Mesh,
            total_tasks: 4,
            component_count: 4,
        };
        let mut titles = BTreeMap::new();
        titles.insert(entity(":task/t-1"), "Fix topology".to_string());
        let output = format_plan_human(&plan, &titles);
        assert!(output.contains("mesh"), "should include pattern");
        assert!(output.contains("test-agent"), "should include agent name");
        assert!(output.contains("Fix topology"), "should include task title");
        assert!(
            output.contains("disjointness: verified"),
            "should verify disjointness"
        );
    }

    #[test]
    fn format_plan_agent_is_compact() {
        let plan = TopologyPlan {
            method: PlanMethod::Quick,
            assignments: vec![AgentAssignment {
                name: "kernel".to_string(),
                tasks: vec![entity(":task/t-1")],
                files: files(&["crates/a.rs"]),
                total_impact: 0.5,
            }],
            coupling_entropy: 1.0,
            parallelizability: 0.5,
            effective_rank: 2.0,
            pattern: TopologyPattern::Hybrid,
            total_tasks: 2,
            component_count: 2,
        };
        let mut titles = BTreeMap::new();
        titles.insert(entity(":task/t-1"), "Do thing".to_string());
        let output = format_plan_agent(&plan, &titles);
        assert!(output.contains("1a/2t/2c"), "should have compact header");
        assert!(output.contains("[kernel]"), "should have agent name");
    }

    // === Seed generation tests ===

    #[test]
    fn emit_seed_contains_protocol() {
        let assignment = AgentAssignment {
            name: "test".to_string(),
            tasks: vec![entity(":task/t-1")],
            files: files(&["crates/a.rs"]),
            total_impact: 0.5,
        };
        let mut titles = BTreeMap::new();
        titles.insert(entity(":task/t-1"), "Fix thing".to_string());
        let seed = emit_seed_for_agent(&assignment, &titles, 2);
        assert!(seed.contains("Agent: test"), "should include agent name");
        assert!(seed.contains("Fix thing"), "should include task title");
        assert!(seed.contains("crates/a.rs"), "should include file");
        assert!(seed.contains("Protocol"), "should include protocol");
        assert!(seed.contains("cargo fmt"), "should warn about cargo");
    }

    // === Plan verification tests ===

    #[test]
    fn verify_disjointness_passes() {
        let plan = TopologyPlan {
            method: PlanMethod::Quick,
            assignments: vec![
                AgentAssignment {
                    name: "a".to_string(),
                    tasks: vec![],
                    files: files(&["crates/x.rs"]),
                    total_impact: 0.0,
                },
                AgentAssignment {
                    name: "b".to_string(),
                    tasks: vec![],
                    files: files(&["crates/y.rs"]),
                    total_impact: 0.0,
                },
            ],
            coupling_entropy: 0.0,
            parallelizability: 1.0,
            effective_rank: 2.0,
            pattern: TopologyPattern::Mesh,
            total_tasks: 2,
            component_count: 2,
        };
        assert!(plan.verify_disjointness().is_ok());
    }

    #[test]
    fn verify_disjointness_fails_on_overlap() {
        let plan = TopologyPlan {
            method: PlanMethod::Quick,
            assignments: vec![
                AgentAssignment {
                    name: "a".to_string(),
                    tasks: vec![],
                    files: files(&["crates/shared.rs"]),
                    total_impact: 0.0,
                },
                AgentAssignment {
                    name: "b".to_string(),
                    tasks: vec![],
                    files: files(&["crates/shared.rs"]),
                    total_impact: 0.0,
                },
            ],
            coupling_entropy: 0.0,
            parallelizability: 0.0,
            effective_rank: 1.0,
            pattern: TopologyPattern::Solo,
            total_tasks: 2,
            component_count: 1,
        };
        let err = plan.verify_disjointness().unwrap_err();
        match err {
            TopologyError::DisjointnessViolation { file } => {
                assert_eq!(file, "crates/shared.rs");
            }
            _ => panic!("expected DisjointnessViolation"),
        }
    }

    #[test]
    fn verify_completeness_passes() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let plan = TopologyPlan {
            method: PlanMethod::Quick,
            assignments: vec![
                AgentAssignment {
                    name: "a".to_string(),
                    tasks: vec![e1],
                    files: BTreeSet::new(),
                    total_impact: 0.0,
                },
                AgentAssignment {
                    name: "b".to_string(),
                    tasks: vec![e2],
                    files: BTreeSet::new(),
                    total_impact: 0.0,
                },
            ],
            coupling_entropy: 0.0,
            parallelizability: 1.0,
            effective_rank: 2.0,
            pattern: TopologyPattern::Mesh,
            total_tasks: 2,
            component_count: 2,
        };
        let expected: BTreeSet<EntityId> = [e1, e2].into_iter().collect();
        assert!(plan.verify_completeness(&expected));
    }

    // === TopologyError tests ===

    #[test]
    fn topology_error_display_has_spec_refs() {
        let err = TopologyError::InsufficientTasks { found: 1 };
        let msg = err.to_string();
        assert!(
            msg.contains("INV-TOPOLOGY-001"),
            "error display should reference spec"
        );
    }

    #[test]
    fn topology_error_recovery_hints_nonempty() {
        let errors = vec![
            TopologyError::InsufficientTasks { found: 0 },
            TopologyError::NoCouplingData,
            TopologyError::AgentCountZero,
            TopologyError::NoCouplingWeights,
            TopologyError::PartitionImbalance {
                ratio: "3.50".to_string(),
                threshold: "2.00".to_string(),
            },
            TopologyError::DisjointnessViolation {
                file: "test.rs".to_string(),
            },
            TopologyError::NoSpecDependencies,
        ];
        for err in &errors {
            let hint = err.recovery_hint();
            assert!(
                !hint.is_empty(),
                "recovery_hint should be non-empty for {err}"
            );
            assert!(
                hint.contains("braid"),
                "recovery_hint should contain a braid command for {err}"
            );
        }
    }

    // === Invariant coupling tests (INV-TOPOLOGY-004) ===

    /// Helper: build a store with spec dependency edges (`:spec/traces-to` Ref datoms).
    ///
    /// `edges` is a list of (source_ident, target_ident) pairs.
    fn store_with_spec_deps(edges: &[(&str, &str)]) -> Store {
        use crate::datom::*;
        use crate::schema::genesis_datoms;

        let agent = AgentId::from_name("test");
        let tx = TxId::new(1, 0, agent);
        let mut datoms: BTreeSet<Datom> = BTreeSet::new();
        let genesis_tx = TxId::new(0, 0, agent);
        for d in genesis_datoms(genesis_tx) {
            datoms.insert(d);
        }

        let spec_depends_attr = Attribute::from_keyword(":spec/depends-on");

        for (src, tgt) in edges {
            let src_e = EntityId::from_ident(src);
            let tgt_e = EntityId::from_ident(tgt);
            datoms.insert(Datom::new(
                src_e,
                spec_depends_attr.clone(),
                Value::Ref(tgt_e),
                tx,
                Op::Assert,
            ));
        }

        Store::from_datoms(datoms)
    }

    #[test]
    fn invariant_coupling_shared_spec_deps() {
        // Spec graph: A -> C, B -> C (both A and B trace to C)
        let store = store_with_spec_deps(&[
            (":spec/inv-a", ":spec/inv-c"),
            (":spec/inv-b", ":spec/inv-c"),
        ]);

        let spec_a = EntityId::from_ident(":spec/inv-a");
        let spec_b = EntityId::from_ident(":spec/inv-b");
        let task1 = EntityId::from_ident(":task/t-1");
        let task2 = EntityId::from_ident(":task/t-2");

        let mut task_specs = BTreeMap::new();
        task_specs.insert(task1, [spec_a].into_iter().collect());
        task_specs.insert(task2, [spec_b].into_iter().collect());

        let coupling = compute_invariant_coupling(&store, &task_specs);

        // Both tasks transitively reach :spec/inv-c, so coupling > 0
        let score = coupling.get(&(task1, task2)).copied().unwrap_or(0.0);
        assert!(
            score > 0.0,
            "tasks with shared transitive spec deps should have coupling > 0, got {score}"
        );
        assert!(score <= 1.0, "coupling must be <= 1.0, got {score}");
    }

    #[test]
    fn invariant_coupling_disjoint_specs() {
        // Spec graph: A -> B, C -> D (two disconnected components)
        let store = store_with_spec_deps(&[
            (":spec/inv-a", ":spec/inv-b"),
            (":spec/inv-c", ":spec/inv-d"),
        ]);

        let spec_a = EntityId::from_ident(":spec/inv-a");
        let spec_c = EntityId::from_ident(":spec/inv-c");
        let task1 = EntityId::from_ident(":task/t-1");
        let task2 = EntityId::from_ident(":task/t-2");

        let mut task_specs = BTreeMap::new();
        task_specs.insert(task1, [spec_a].into_iter().collect());
        task_specs.insert(task2, [spec_c].into_iter().collect());

        let coupling = compute_invariant_coupling(&store, &task_specs);

        // Disjoint reachability sets -> coupling = 0
        assert!(
            !coupling.contains_key(&(task1, task2)),
            "tasks in disjoint spec namespaces should have zero coupling"
        );
    }

    #[test]
    fn invariant_coupling_symmetric_and_bounded() {
        // Spec graph: A -> C, B -> C, A -> D
        // Task1 refs A (reaches {A, C, D}), Task2 refs B (reaches {B, C})
        // Intersection = {C}, Union = {A, B, C, D} -> Jaccard = 1/4 = 0.25
        let store = store_with_spec_deps(&[
            (":spec/inv-a", ":spec/inv-c"),
            (":spec/inv-a", ":spec/inv-d"),
            (":spec/inv-b", ":spec/inv-c"),
        ]);

        let spec_a = EntityId::from_ident(":spec/inv-a");
        let spec_b = EntityId::from_ident(":spec/inv-b");
        let task1 = EntityId::from_ident(":task/t-1");
        let task2 = EntityId::from_ident(":task/t-2");

        let mut task_specs = BTreeMap::new();
        task_specs.insert(task1, [spec_a].into_iter().collect());
        task_specs.insert(task2, [spec_b].into_iter().collect());

        let coupling = compute_invariant_coupling(&store, &task_specs);

        // Symmetry: (t1, t2) == (t2, t1)
        let score_12 = coupling.get(&(task1, task2)).copied().unwrap_or(0.0);
        let score_21 = coupling.get(&(task2, task1)).copied().unwrap_or(0.0);
        assert!(
            (score_12 - score_21).abs() < 1e-15,
            "coupling must be symmetric: {score_12} != {score_21}"
        );

        // Bounded: 0 < score <= 1
        assert!(score_12 > 0.0, "should have nonzero coupling");
        assert!(score_12 <= 1.0, "coupling must be <= 1.0");

        // Exact value: {C} / {A, B, C, D} = 1/4
        assert!(
            (score_12 - 0.25).abs() < 1e-10,
            "expected Jaccard = 0.25, got {score_12}"
        );
    }

    #[test]
    fn invariant_coupling_identical_spec_refs() {
        // Both tasks reference the same spec -> coupling = 1.0
        let store = store_with_spec_deps(&[(":spec/inv-a", ":spec/inv-b")]);

        let spec_a = EntityId::from_ident(":spec/inv-a");
        let task1 = EntityId::from_ident(":task/t-1");
        let task2 = EntityId::from_ident(":task/t-2");

        let mut task_specs = BTreeMap::new();
        task_specs.insert(task1, [spec_a].into_iter().collect());
        task_specs.insert(task2, [spec_a].into_iter().collect());

        let coupling = compute_invariant_coupling(&store, &task_specs);

        let score = coupling.get(&(task1, task2)).copied().unwrap_or(0.0);
        assert!(
            (score - 1.0).abs() < 1e-10,
            "identical spec refs should yield coupling = 1.0, got {score}"
        );
    }

    #[test]
    fn invariant_coupling_empty_task_specs() {
        let store = store_with_spec_deps(&[(":spec/inv-a", ":spec/inv-b")]);

        // No tasks -> empty result
        let task_specs: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();
        let coupling = compute_invariant_coupling(&store, &task_specs);
        assert!(coupling.is_empty());

        // Single task -> empty result (need at least 2)
        let mut single = BTreeMap::new();
        single.insert(
            EntityId::from_ident(":task/t-1"),
            [EntityId::from_ident(":spec/inv-a")].into_iter().collect(),
        );
        let coupling = compute_invariant_coupling(&store, &single);
        assert!(coupling.is_empty());
    }

    #[test]
    fn invariant_coupling_handles_cycles() {
        // Spec graph has a cycle: A -> B -> C -> A
        // All specs are mutually reachable, so any two tasks
        // referencing ANY of these specs have coupling = 1.0.
        let store = store_with_spec_deps(&[
            (":spec/inv-a", ":spec/inv-b"),
            (":spec/inv-b", ":spec/inv-c"),
            (":spec/inv-c", ":spec/inv-a"),
        ]);

        let spec_a = EntityId::from_ident(":spec/inv-a");
        let spec_b = EntityId::from_ident(":spec/inv-b");
        let task1 = EntityId::from_ident(":task/t-1");
        let task2 = EntityId::from_ident(":task/t-2");

        let mut task_specs = BTreeMap::new();
        task_specs.insert(task1, [spec_a].into_iter().collect());
        task_specs.insert(task2, [spec_b].into_iter().collect());

        let coupling = compute_invariant_coupling(&store, &task_specs);

        // Both reach {A, B, C} -> identical sets -> Jaccard = 1.0
        let score = coupling.get(&(task1, task2)).copied().unwrap_or(0.0);
        assert!(
            (score - 1.0).abs() < 1e-10,
            "tasks in a spec cycle should have coupling = 1.0, got {score}"
        );
    }

    #[test]
    fn invariant_coupling_no_spec_edges_in_store() {
        // Store has no :spec/traces-to edges at all.
        // Tasks with spec refs that aren't in the graph still get
        // coupling from direct spec ref overlap.
        let store = store_with_spec_deps(&[]);

        let spec_a = EntityId::from_ident(":spec/inv-a");
        let spec_b = EntityId::from_ident(":spec/inv-b");
        let task1 = EntityId::from_ident(":task/t-1");
        let task2 = EntityId::from_ident(":task/t-2");

        // Disjoint spec refs, no edges -> no coupling
        let mut task_specs = BTreeMap::new();
        task_specs.insert(task1, [spec_a].into_iter().collect());
        task_specs.insert(task2, [spec_b].into_iter().collect());
        let coupling = compute_invariant_coupling(&store, &task_specs);
        assert!(
            coupling.is_empty(),
            "disjoint refs with no edges -> zero coupling"
        );

        // Same spec ref, no edges -> coupling = 1.0 (self-reachability)
        let mut shared = BTreeMap::new();
        shared.insert(task1, [spec_a].into_iter().collect());
        shared.insert(task2, [spec_a].into_iter().collect());
        let coupling = compute_invariant_coupling(&store, &shared);
        let score = coupling.get(&(task1, task2)).copied().unwrap_or(0.0);
        assert!(
            (score - 1.0).abs() < 1e-10,
            "same spec ref -> coupling = 1.0 even without edges, got {score}"
        );
    }

    // ===================================================================
    // Coupling Density Matrix (TOPO-DENSITY) Tests
    // ===================================================================

    #[test]
    fn density_matrix_identity_is_uniform() {
        // No coupling → identity matrix → ρ = I/n → max entropy
        let entities = vec![entity(":t/a"), entity(":t/b"), entity(":t/c")];
        let coupling = BTreeMap::new(); // No coupling edges

        let analysis = coupling_density_matrix(&coupling, &entities);

        // ρ should be I/3 (diagonal 1/3, off-diagonal 0)
        assert_eq!(analysis.rho.len(), 3);
        for i in 0..3 {
            assert!((analysis.rho[i][i] - 1.0 / 3.0).abs() < 1e-10);
        }
        // Entropy should be ln(3) ≈ 1.099 (von Neumann entropy uses natural log)
        assert!(
            (analysis.entropy - 3.0f64.ln()).abs() < 0.1,
            "identity matrix entropy should be ln(n): got {}",
            analysis.entropy
        );
        // r_eff = e^S ≈ 3 (since S = ln(3))
        assert!(
            (analysis.effective_rank - 3.0).abs() < 0.5,
            "effective rank should be ~3: got {}",
            analysis.effective_rank
        );
        // Parallelizability should be ≈ 1.0
        assert!(
            analysis.parallelizability > 0.8,
            "fully independent tasks should have high parallelizability: {}",
            analysis.parallelizability
        );
    }

    #[test]
    fn density_matrix_fully_coupled() {
        // Full coupling → one dominant eigenvalue → low entropy
        let entities = vec![entity(":t/a"), entity(":t/b")];
        let mut coupling = BTreeMap::new();
        coupling.insert((entities[0], entities[1]), 1.0);
        coupling.insert((entities[1], entities[0]), 1.0);

        let analysis = coupling_density_matrix(&coupling, &entities);

        // Fully coupled: rho = [[0.5, 0.5], [0.5, 0.5]]
        assert!((analysis.rho[0][1] - 0.5).abs() < 1e-10);
        // One eigenvalue = 1, other = 0 → entropy = 0
        assert!(
            analysis.entropy < 0.1,
            "fully coupled should have near-zero entropy: {}",
            analysis.entropy
        );
        assert!(
            analysis.effective_rank < 1.5,
            "effective rank should be ~1: {}",
            analysis.effective_rank
        );
        assert!(
            analysis.parallelizability < 0.8,
            "fully coupled tasks should have low parallelizability: {}",
            analysis.parallelizability
        );
    }

    #[test]
    fn density_matrix_empty() {
        let analysis = coupling_density_matrix(&BTreeMap::new(), &[]);
        assert_eq!(analysis.entities.len(), 0);
        assert_eq!(analysis.entropy, 0.0);
    }

    #[test]
    fn density_matrix_single_entity() {
        let entities = vec![entity(":t/single")];
        let analysis = coupling_density_matrix(&BTreeMap::new(), &entities);
        assert_eq!(analysis.rho.len(), 1);
        assert!((analysis.rho[0][0] - 1.0).abs() < 1e-10);
        // Single entity: entropy = 0 (only one eigenvalue = 1)
        assert!(analysis.entropy < 0.01);
    }

    #[test]
    fn density_matrix_psd_unit_trace_symmetric() {
        // INV-COHERENCE-001: ρ must be PSD, unit-trace, symmetric
        let entities = vec![entity(":t/a"), entity(":t/b"), entity(":t/c")];
        let mut coupling = BTreeMap::new();
        coupling.insert((entities[0], entities[1]), 0.5);
        coupling.insert((entities[1], entities[0]), 0.5);
        coupling.insert((entities[1], entities[2]), 0.3);
        coupling.insert((entities[2], entities[1]), 0.3);

        let analysis = coupling_density_matrix(&coupling, &entities);
        let n = analysis.rho.len();

        // Symmetric
        for i in 0..n {
            for j in 0..n {
                assert!(
                    (analysis.rho[i][j] - analysis.rho[j][i]).abs() < 1e-10,
                    "rho must be symmetric: rho[{i}][{j}]={} vs rho[{j}][{i}]={}",
                    analysis.rho[i][j],
                    analysis.rho[j][i]
                );
            }
        }

        // Unit trace
        let trace: f64 = (0..n).map(|i| analysis.rho[i][i]).sum();
        assert!(
            (trace - 1.0).abs() < 1e-10,
            "trace must be 1.0: got {trace}"
        );

        // PSD (all eigenvalues >= 0)
        for &ev in &analysis.eigenvalues {
            assert!(ev >= -1e-10, "eigenvalue must be non-negative: {ev}");
        }
    }

    #[test]
    fn composite_coupling_merges_correctly() {
        let a = entity(":t/a");
        let b = entity(":t/b");

        let mut file_c = BTreeMap::new();
        file_c.insert((a, b), 0.8);
        file_c.insert((b, a), 0.8);

        let mut inv_c = BTreeMap::new();
        inv_c.insert((a, b), 0.4);
        inv_c.insert((b, a), 0.4);

        let combined = composite_coupling(&file_c, &inv_c);
        let score = combined.get(&(a, b)).unwrap();
        // Stage 0b weights: 0.50 * 0.8 + 0.35 * 0.4 = 0.40 + 0.14 = 0.54
        assert!(
            (*score - 0.54).abs() < 0.01,
            "composite should be 0.54: got {score}"
        );
    }

    // ===================================================================
    // CALM Classification (TOPO-CALM, ADR-TOPOLOGY-002, INV-TOPOLOGY-006)
    // ===================================================================

    #[test]
    fn calm_pure_edit_tasks_are_tier_m() {
        // Pure edit tasks — implement, add, create, fix, refactor — all Tier M.
        let edit_titles = [
            "Implement invariant coupling dimension",
            "Add topology pattern classification",
            "Create task summary view",
            "Fix Unicode boundary panics",
            "Refactor guidance footer generation",
            "Edit crates/braid-kernel/src/topology.rs for coupling",
        ];
        for title in &edit_titles {
            assert_eq!(
                classify_task_phase(title),
                CalmTier::MonotonicParallel,
                "'{title}' should be Tier M (parallel)"
            );
        }
    }

    #[test]
    fn calm_nm_keywords_are_tier_nm() {
        // Tasks with non-monotonic keywords — merge, verify, test, schema, etc.
        let nm_titles = [
            "Merge stores after parallel editing",
            "Verify coherence invariants",
            "Run test suite for regression",
            "Schema evolution for layer 4 attributes",
            "Migrate datom store to new format",
            "Cascade step1 conflict detection",
            "Validate specification elements",
        ];
        for title in &nm_titles {
            assert_eq!(
                classify_task_phase(title),
                CalmTier::NonMonotonicBarrier,
                "'{title}' should be Tier NM (barrier)"
            );
        }
    }

    #[test]
    fn calm_file_marker_overrides_nm_keywords() {
        // FILE: marker makes it Tier M even if NM keywords are present.
        // This is because FILE: signals a concrete edit task.
        let title = "Verify test coverage for merge logic. FILE: crates/braid-kernel/src/merge.rs";
        assert_eq!(
            classify_task_phase(title),
            CalmTier::MonotonicParallel,
            "FILE: marker should force Tier M despite NM keywords"
        );
    }

    #[test]
    fn calm_default_is_tier_m() {
        // Titles with no recognizable keywords default to Tier M.
        assert_eq!(
            classify_task_phase("Abstract design discussion about coherence"),
            CalmTier::MonotonicParallel,
            "unrecognized title should default to Tier M"
        );
    }

    #[test]
    fn calm_phase_plan_all_m() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let e3 = entity(":task/t-3");
        let tasks = vec![
            (e1, CalmTier::MonotonicParallel),
            (e2, CalmTier::MonotonicParallel),
            (e3, CalmTier::MonotonicParallel),
        ];
        let phases = phase_plan(&tasks);
        assert_eq!(phases.len(), 1, "all-M tasks → single phase");
        assert_eq!(phases[0].tier, CalmTier::MonotonicParallel);
        assert_eq!(phases[0].tasks, vec![e1, e2, e3]);
    }

    #[test]
    fn calm_phase_plan_m_nm_m_produces_3_phases() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let e3 = entity(":task/t-3");
        let e4 = entity(":task/t-4");
        let tasks = vec![
            (e1, CalmTier::MonotonicParallel),   // Phase 1: M
            (e2, CalmTier::NonMonotonicBarrier), // Phase 2: NM (barrier)
            (e3, CalmTier::MonotonicParallel),   // Phase 3: M
            (e4, CalmTier::MonotonicParallel),   // Phase 3: M (same tier, grouped)
        ];
        let phases = phase_plan(&tasks);
        assert_eq!(phases.len(), 3, "M,NM,M,M → 3 phases");
        assert_eq!(phases[0].tier, CalmTier::MonotonicParallel);
        assert_eq!(phases[0].tasks, vec![e1]);
        assert_eq!(phases[1].tier, CalmTier::NonMonotonicBarrier);
        assert_eq!(phases[1].tasks, vec![e2]);
        assert_eq!(phases[2].tier, CalmTier::MonotonicParallel);
        assert_eq!(phases[2].tasks, vec![e3, e4]);
    }

    #[test]
    fn calm_phase_plan_empty_input() {
        let phases = phase_plan(&[]);
        assert!(phases.is_empty(), "empty input → empty plan");
    }

    #[test]
    fn calm_phase_plan_consecutive_nm_grouped() {
        let e1 = entity(":task/t-1");
        let e2 = entity(":task/t-2");
        let tasks = vec![
            (e1, CalmTier::NonMonotonicBarrier),
            (e2, CalmTier::NonMonotonicBarrier),
        ];
        let phases = phase_plan(&tasks);
        assert_eq!(phases.len(), 1, "consecutive NM tasks → single NM phase");
        assert_eq!(phases[0].tier, CalmTier::NonMonotonicBarrier);
        assert_eq!(phases[0].tasks, vec![e1, e2]);
    }

    // ===================================================================
    // Spectral Topology Selection (TOPO-SPECTRAL) Tests
    // ===================================================================

    /// Build a CouplingAnalysis from a raw coupling matrix for testing.
    fn analysis_from_matrix(rho: Vec<Vec<f64>>) -> CouplingAnalysis {
        let n = rho.len();
        let entities: Vec<EntityId> = (0..n)
            .map(|i| entity(&format!(":t/spectral-{i}")))
            .collect();

        // Compute eigenvalues via the same Jacobi method used in production
        let eigenvalues = super::symmetric_eigenvalues(&rho, n);

        let entropy = von_neumann_entropy_from_eigenvalues(&eigenvalues);
        let effective_rank = entropy.exp();
        let parallelizability = if n > 0 {
            (effective_rank / n as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };

        CouplingAnalysis {
            rho,
            entities,
            eigenvalues,
            entropy,
            effective_rank,
            parallelizability,
        }
    }

    #[test]
    fn spectral_identity_coupling_yields_mesh() {
        // Identity matrix: no coupling, all tasks independent.
        // rho = I/n => uniform eigenvalues => max entropy => p ~= 1.0 => Mesh.
        let mut rho = vec![vec![0.0; 4]; 4];
        for (i, row) in rho.iter_mut().enumerate() {
            row[i] = 0.25;
        }
        let analysis = analysis_from_matrix(rho);
        assert!(
            analysis.parallelizability > 0.8,
            "identity coupling should have high p: got {}",
            analysis.parallelizability
        );
        let pattern = select_topology(&analysis, 4);
        assert_eq!(
            pattern,
            TopologyPattern::Mesh,
            "identity coupling should yield Mesh"
        );
    }

    #[test]
    fn spectral_fully_coupled_yields_star_or_pipeline() {
        // All-ones matrix normalized: rho[i][j] = 1/n for all i,j.
        // One dominant eigenvalue => low entropy => p << 0.3 => Star or Pipeline.
        let n = 4;
        let rho = vec![vec![1.0 / n as f64; n]; n];
        let analysis = analysis_from_matrix(rho);
        assert!(
            analysis.parallelizability <= 0.3,
            "fully coupled should have low p: got {}",
            analysis.parallelizability
        );
        let pattern = select_topology(&analysis, 4);
        assert!(
            pattern == TopologyPattern::Star || pattern == TopologyPattern::Pipeline,
            "fully coupled should yield Star or Pipeline, got {:?}",
            pattern
        );
    }

    #[test]
    fn spectral_block_diagonal_yields_hybrid() {
        // Block-diagonal: two 2x2 blocks, no inter-block coupling.
        // rho = [[0.25, 0.25, 0, 0],
        //        [0.25, 0.25, 0, 0],
        //        [0, 0, 0.25, 0.25],
        //        [0, 0, 0.25, 0.25]]
        // Two independent clusters => moderate entropy => Hybrid.
        let rho = vec![
            vec![0.25, 0.25, 0.0, 0.0],
            vec![0.25, 0.25, 0.0, 0.0],
            vec![0.0, 0.0, 0.25, 0.25],
            vec![0.0, 0.0, 0.25, 0.25],
        ];
        let analysis = analysis_from_matrix(rho);
        // With two equal blocks, p should be moderate (around 0.5)
        let pattern = select_topology(&analysis, 4);
        assert!(
            pattern == TopologyPattern::Hybrid || pattern == TopologyPattern::Mesh,
            "block diagonal should yield Hybrid or Mesh, got {:?} (p={})",
            pattern,
            analysis.parallelizability
        );
    }

    #[test]
    fn spectral_select_topology_deterministic() {
        // INV-TOPOLOGY-005: same input must always produce the same output.
        let rho = vec![
            vec![1.0 / 3.0, 0.1, 0.0],
            vec![0.1, 1.0 / 3.0, 0.1],
            vec![0.0, 0.1, 1.0 / 3.0],
        ];

        let analysis = analysis_from_matrix(rho);
        let pattern1 = select_topology(&analysis, 3);
        let pattern2 = select_topology(&analysis, 3);
        let pattern3 = select_topology(&analysis, 3);

        assert_eq!(pattern1, pattern2, "select_topology must be deterministic");
        assert_eq!(pattern2, pattern3, "select_topology must be deterministic");
    }

    #[test]
    fn spectral_select_topology_solo_cases() {
        // Solo when agent_count <= 1
        let rho = vec![vec![0.5, 0.0], vec![0.0, 0.5]];
        let analysis = analysis_from_matrix(rho);
        assert_eq!(select_topology(&analysis, 1), TopologyPattern::Solo);
        assert_eq!(select_topology(&analysis, 0), TopologyPattern::Solo);

        // Solo when single entity
        let single = analysis_from_matrix(vec![vec![1.0]]);
        assert_eq!(select_topology(&single, 3), TopologyPattern::Solo);
    }

    // === Spectral Partition Tests ===

    #[test]
    fn spectral_partition_identity_matrix() {
        // Identity matrix: each task is its own cluster.
        let mut rho = vec![vec![0.0; 4]; 4];
        for (i, row) in rho.iter_mut().enumerate() {
            row[i] = 0.25;
        }
        let groups = spectral_partition(&rho, 4);
        // Should produce 4 singleton groups (or close to it)
        assert!(
            groups.len() >= 2,
            "identity matrix should partition into multiple groups: got {}",
            groups.len()
        );
        // All indices must be present exactly once
        let mut all_indices: Vec<usize> = groups.iter().flat_map(|g| g.iter().copied()).collect();
        all_indices.sort();
        assert_eq!(all_indices, vec![0, 1, 2, 3], "all indices must be present");
    }

    #[test]
    fn spectral_partition_block_diagonal() {
        // Two clear blocks with strong within-block coupling, zero cross-block.
        // Use a coupling matrix (not density matrix) with explicit block structure.
        // Block A = {0,1} with coupling 0.9, Block B = {2,3} with coupling 0.9.
        let rho = vec![
            vec![0.0, 0.9, 0.0, 0.0],
            vec![0.9, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.9],
            vec![0.0, 0.0, 0.9, 0.0],
        ];
        let groups = spectral_partition(&rho, 2);
        assert_eq!(groups.len(), 2, "block diagonal should yield 2 groups");

        // All 4 indices must be present exactly once
        let mut all_indices: Vec<usize> = groups.iter().flat_map(|g| g.iter().copied()).collect();
        all_indices.sort();
        assert_eq!(all_indices, vec![0, 1, 2, 3], "all indices must be present");

        // Each group should have exactly 2 elements (matching the blocks)
        let mut sizes: Vec<usize> = groups.iter().map(|g| g.len()).collect();
        sizes.sort();
        assert_eq!(sizes, vec![2, 2], "each block should be a group of 2");

        // Check that 0,1 are together and 2,3 are together (or vice versa)
        let g0: std::collections::BTreeSet<usize> = groups[0].iter().copied().collect();
        let g1: std::collections::BTreeSet<usize> = groups[1].iter().copied().collect();
        let block_a: std::collections::BTreeSet<usize> = [0, 1].into_iter().collect();
        let block_b: std::collections::BTreeSet<usize> = [2, 3].into_iter().collect();
        assert!(
            (g0 == block_a && g1 == block_b) || (g0 == block_b && g1 == block_a),
            "blocks should match: {:?} vs {:?}",
            groups[0],
            groups[1]
        );
    }

    #[test]
    fn spectral_partition_deterministic() {
        // INV-TOPOLOGY-005: same input always produces the same output.
        let rho = vec![
            vec![0.25, 0.20, 0.0, 0.0],
            vec![0.20, 0.25, 0.0, 0.0],
            vec![0.0, 0.0, 0.25, 0.20],
            vec![0.0, 0.0, 0.20, 0.25],
        ];
        let groups1 = spectral_partition(&rho, 2);
        let groups2 = spectral_partition(&rho, 2);
        let groups3 = spectral_partition(&rho, 2);
        assert_eq!(groups1, groups2, "spectral_partition must be deterministic");
        assert_eq!(groups2, groups3, "spectral_partition must be deterministic");
    }

    #[test]
    fn spectral_partition_k_one_returns_all() {
        let rho = vec![vec![0.5, 0.1], vec![0.1, 0.5]];
        let groups = spectral_partition(&rho, 1);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![0, 1]);
    }

    #[test]
    fn spectral_partition_empty() {
        let groups = spectral_partition(&[], 3);
        assert!(groups.is_empty());
    }

    // === Partition Quality Tests ===

    #[test]
    fn partition_quality_perfect_block_diagonal() {
        // Block diagonal: partition that matches blocks should have quality 1.0.
        let coupling = vec![
            vec![0.0, 0.5, 0.0, 0.0],
            vec![0.5, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.5],
            vec![0.0, 0.0, 0.5, 0.0],
        ];
        let partition = vec![vec![0, 1], vec![2, 3]];
        let quality = partition_quality(&partition, &coupling);
        assert!(
            (quality - 1.0).abs() < 1e-10,
            "perfect partition should have quality 1.0, got {quality}"
        );
    }

    #[test]
    fn partition_quality_worst_split() {
        // Block diagonal, but partition splits each block across groups.
        let coupling = vec![
            vec![0.0, 0.5, 0.0, 0.0],
            vec![0.5, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.5],
            vec![0.0, 0.0, 0.5, 0.0],
        ];
        // Worst split: put 0 and 2 together, 1 and 3 together
        let partition = vec![vec![0, 2], vec![1, 3]];
        let quality = partition_quality(&partition, &coupling);
        assert!(
            quality < 0.01,
            "worst partition should have quality near 0.0, got {quality}"
        );
    }

    #[test]
    fn partition_quality_block_diagonal_better_than_random() {
        // A block-diagonal coupling matrix with the correct partition
        // must have better quality than a random split.
        let coupling = vec![
            vec![0.0, 0.8, 0.0, 0.0],
            vec![0.8, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.8],
            vec![0.0, 0.0, 0.8, 0.0],
        ];
        let good_partition = vec![vec![0, 1], vec![2, 3]];
        let bad_partition = vec![vec![0, 2], vec![1, 3]];
        let good_q = partition_quality(&good_partition, &coupling);
        let bad_q = partition_quality(&bad_partition, &coupling);
        assert!(
            good_q > bad_q,
            "block-diagonal partition ({good_q}) should have better quality than random ({bad_q})"
        );
    }

    #[test]
    fn partition_quality_no_coupling() {
        let coupling = vec![vec![0.0, 0.0], vec![0.0, 0.0]];
        let partition = vec![vec![0], vec![1]];
        let quality = partition_quality(&partition, &coupling);
        assert!(
            (quality - 1.0).abs() < 1e-10,
            "no coupling should yield quality 1.0, got {quality}"
        );
    }
}
