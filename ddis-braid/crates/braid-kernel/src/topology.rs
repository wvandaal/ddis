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

use crate::datom::EntityId;
use crate::error::TopologyError;
use crate::store::Store;

/// Extract the set of files a task touches, from its title text.
///
/// Looks for:
/// 1. Explicit `FILE:` or `FILES:` markers in the title
/// 2. Known Rust source file patterns (crates/*/src/*.rs)
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

    // Pattern 2: Inline file paths (crates/*/src/*.rs pattern)
    for word in title.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| {
            !c.is_alphanumeric() && c != '/' && c != '.' && c != '-' && c != '_'
        });
        if trimmed.starts_with("crates/") && trimmed.ends_with(".rs") {
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
        let files = extract_task_files(&task.title);
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

    let skip = ["src", "crates", "lib", "mod", "main", "tests"];
    let mut stem_counts: BTreeMap<&str, usize> = BTreeMap::new();

    for path in files {
        let parts: Vec<&str> = path.split('/').collect();
        for &part in &parts {
            // Strip .rs extension for file stems
            let stem = part.strip_suffix(".rs").unwrap_or(part);
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
pub fn quick_plan(store: &Store, agent_count: usize) -> Result<TopologyPlan, TopologyError> {
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
    let routing = crate::guidance::compute_routing_from_store(store);
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
}
