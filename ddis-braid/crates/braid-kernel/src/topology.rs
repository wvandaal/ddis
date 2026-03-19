//! Coordination Topology — file coupling extraction and task partitioning.
//!
//! Implements the first concrete step of ADR-TOPOLOGY-004 (Topology as Compilation):
//! derive agent assignment from task→file coupling structure.
//!
//! The coupling matrix C[i][j] measures file overlap between tasks i and j.
//! Tasks with shared files must be serialized; disjoint tasks can be parallelized.
//!
//! Traces to: spec/19-topology.md INV-TOPOLOGY-004 (Composite Coupling Signal),
//! INV-TOPOLOGY-005 (Coupling-to-Topology Determinism), ADR-TOPOLOGY-004.

use std::collections::{BTreeMap, BTreeSet};

use crate::datom::{EntityId, Op};
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
    groups.sort_by(|a, b| b.len().cmp(&a.len()));
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn coupling_shared_file() {
        let e1 = EntityId::from_ident(":task/t-1");
        let e2 = EntityId::from_ident(":task/t-2");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, {
            let mut s = BTreeSet::new();
            s.insert("crates/a.rs".to_string());
            s
        });
        task_files.insert(e2, {
            let mut s = BTreeSet::new();
            s.insert("crates/a.rs".to_string());
            s
        });
        let coupling = compute_file_coupling(&task_files);
        assert_eq!(*coupling.get(&(e1, e2)).unwrap(), 1.0);
    }

    #[test]
    fn coupling_disjoint_files() {
        let e1 = EntityId::from_ident(":task/t-1");
        let e2 = EntityId::from_ident(":task/t-2");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, {
            let mut s = BTreeSet::new();
            s.insert("crates/a.rs".to_string());
            s
        });
        task_files.insert(e2, {
            let mut s = BTreeSet::new();
            s.insert("crates/b.rs".to_string());
            s
        });
        let coupling = compute_file_coupling(&task_files);
        assert!(coupling.is_empty());
    }

    #[test]
    fn partition_disjoint_tasks() {
        let e1 = EntityId::from_ident(":task/t-1");
        let e2 = EntityId::from_ident(":task/t-2");
        let e3 = EntityId::from_ident(":task/t-3");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, {
            let mut s = BTreeSet::new();
            s.insert("crates/a.rs".to_string());
            s
        });
        task_files.insert(e2, {
            let mut s = BTreeSet::new();
            s.insert("crates/b.rs".to_string());
            s
        });
        task_files.insert(e3, {
            let mut s = BTreeSet::new();
            s.insert("crates/a.rs".to_string()); // shares with e1
            s
        });
        let groups = partition_by_file_coupling(&task_files);
        // e1 and e3 share a file → same group. e2 is separate.
        assert_eq!(groups.len(), 2);
        // Largest group first
        assert_eq!(groups[0].len(), 2); // e1 + e3
        assert_eq!(groups[1].len(), 1); // e2
    }

    #[test]
    fn partition_all_disjoint() {
        let e1 = EntityId::from_ident(":task/t-1");
        let e2 = EntityId::from_ident(":task/t-2");
        let mut task_files = BTreeMap::new();
        task_files.insert(e1, {
            let mut s = BTreeSet::new();
            s.insert("crates/a.rs".to_string());
            s
        });
        task_files.insert(e2, {
            let mut s = BTreeSet::new();
            s.insert("crates/b.rs".to_string());
            s
        });
        let groups = partition_by_file_coupling(&task_files);
        assert_eq!(groups.len(), 2); // Each task is its own group
    }
}
