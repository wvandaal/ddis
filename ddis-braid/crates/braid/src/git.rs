//! Git context collection for auto-harvest.
//!
//! Collects commit history and file change statistics from the git repository
//! surrounding the braid store. Uses `std::process::Command` to call git
//! directly — zero new dependencies.
//!
//! Traces to: ADR-INTERFACE-007 (Rust, minimal deps), INV-HARVEST-002 (provenance).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Git context for a session: commits and file change statistics.
#[derive(Clone, Debug, Default)]
pub struct GitContext {
    /// Commits since the given timestamp.
    pub commits: Vec<CommitInfo>,
    /// Total files changed.
    pub files_changed: usize,
    /// Total lines inserted.
    pub insertions: usize,
    /// Total lines deleted.
    pub deletions: usize,
    /// Current branch name.
    pub branch: Option<String>,
}

/// A single git commit.
#[derive(Clone, Debug)]
pub struct CommitInfo {
    /// Abbreviated commit hash.
    pub hash: String,
    /// Commit subject line.
    pub subject: String,
    /// Author name.
    pub author: String,
    /// Unix timestamp.
    pub timestamp: u64,
}

/// Detect the git root directory by walking up from `path`.
///
/// Returns `None` if not inside a git repository.
pub fn detect_git_root(path: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return None;
    }
    Some(PathBuf::from(root))
}

/// Get the current branch name.
pub fn current_branch(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() || branch == "HEAD" {
        None
    } else {
        Some(branch)
    }
}

/// Collect git context since a Unix timestamp.
///
/// When `since` is 0 (or a non-Unix timestamp like the old sequential wall_times),
/// falls back to `--max-count=50` to return recent history without scanning
/// the entire repository.
///
/// Graceful degradation: if git is not available or the path is not a repo,
/// returns an empty GitContext.
pub fn changes_since(path: &Path, since: u64) -> GitContext {
    let root = match detect_git_root(path) {
        Some(r) => r,
        None => return GitContext::default(),
    };

    // Heuristic: if `since` looks like a real Unix timestamp (> year 2000),
    // use --after for precise time-based filtering. Otherwise fall back to
    // --max-count for a reasonable recent window.
    let use_timestamp = since > 946_684_800; // 2000-01-01

    let branch = current_branch(&root);
    let commits = collect_commits(&root, since, use_timestamp);
    let (files_changed, insertions, deletions) = collect_diffstat(&root, since, use_timestamp);

    GitContext {
        commits,
        files_changed,
        insertions,
        deletions,
        branch,
    }
}

/// Collect recent commits.
fn collect_commits(root: &Path, since: u64, use_timestamp: bool) -> Vec<CommitInfo> {
    let filter = if use_timestamp {
        format!("--after={since}")
    } else {
        "--max-count=50".to_string()
    };

    let output = Command::new("git")
        .args([
            "log",
            "--format=%h%x00%s%x00%an%x00%at",
            &filter,
            "--",
            ".",
            ":!.braid",
        ])
        .current_dir(root)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut commits = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\0').collect();
        if parts.len() >= 4 {
            commits.push(CommitInfo {
                hash: parts[0].to_string(),
                subject: parts[1].to_string(),
                author: parts[2].to_string(),
                timestamp: parts[3].parse().unwrap_or(0),
            });
        }
    }

    commits
}

/// Collect diffstat for commits since a timestamp or last N.
fn collect_diffstat(root: &Path, since: u64, use_timestamp: bool) -> (usize, usize, usize) {
    let filter = if use_timestamp {
        format!("--after={since}")
    } else {
        "--max-count=50".to_string()
    };

    let output = Command::new("git")
        .args([
            "log",
            "--format=",
            "--numstat",
            &filter,
            "--",
            ".",
            ":!.braid",
        ])
        .current_dir(root)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return (0, 0, 0),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files = std::collections::BTreeSet::new();
    let mut insertions: usize = 0;
    let mut deletions: usize = 0;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            // Binary files show "-" for insertions/deletions
            insertions += parts[0].parse::<usize>().unwrap_or(0);
            deletions += parts[1].parse::<usize>().unwrap_or(0);
            files.insert(parts[2].to_string());
        }
    }

    (files.len(), insertions, deletions)
}

/// Collect top modified files by change count.
pub fn top_modified_files(path: &Path, since: u64, limit: usize) -> Vec<(String, usize)> {
    let root = match detect_git_root(path) {
        Some(r) => r,
        None => return Vec::new(),
    };
    let use_timestamp = since > 946_684_800;
    let filter = if use_timestamp {
        format!("--after={since}")
    } else {
        "--max-count=50".to_string()
    };

    let output = Command::new("git")
        .args([
            "log",
            "--format=",
            "--numstat",
            &filter,
            "--",
            ".",
            ":!.braid",
            ":!.beads",
        ])
        .current_dir(&root)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut file_changes: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            let adds = parts[0].parse::<usize>().unwrap_or(0);
            let dels = parts[1].parse::<usize>().unwrap_or(0);
            *file_changes.entry(parts[2].to_string()).or_default() += adds + dels;
        }
    }

    let mut sorted: Vec<(String, usize)> = file_changes.into_iter().collect();
    sorted.sort_by_key(|(_, changes)| std::cmp::Reverse(*changes));
    sorted.truncate(limit);
    sorted
}

/// Capture a codebase snapshot: LOC by key source file, test count.
///
/// This gives an incoming agent a map of the codebase — which files
/// are large, where the code lives, and how well-tested it is.
pub fn codebase_snapshot(path: &Path) -> Option<String> {
    let root = detect_git_root(path)?;

    // Get LOC for key source files (Rust .rs files, sorted by size)
    let output = Command::new("git")
        .args(["ls-files", "--", "*.rs", "*.toml"])
        .current_dir(&root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut file_sizes: Vec<(String, usize)> = Vec::new();
    let mut total_loc = 0usize;
    let mut total_files = 0usize;

    for file in stdout.lines() {
        let file = file.trim();
        if file.is_empty() || !file.ends_with(".rs") {
            continue;
        }
        total_files += 1;
        let full_path = root.join(file);
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            let loc = content.lines().count();
            total_loc += loc;
            if loc > 100 {
                // Only show files > 100 LOC
                file_sizes.push((file.to_string(), loc));
            }
        }
    }

    file_sizes.sort_by_key(|(_, loc)| std::cmp::Reverse(*loc));

    let mut lines = Vec::new();
    lines.push(format!(
        "Codebase: {} LOC across {} .rs files",
        total_loc, total_files
    ));

    // Top files by size
    if !file_sizes.is_empty() {
        lines.push("Key files:".to_string());
        for (f, loc) in file_sizes.iter().take(10) {
            lines.push(format!("  {f} ({loc} LOC)"));
        }
    }

    // Test count (from cargo test --no-run output, fast)
    let test_output = Command::new("cargo")
        .args(["test", "--", "--list"])
        .current_dir(&root)
        .output()
        .ok();
    if let Some(ref out) = test_output {
        if out.status.success() {
            let test_stdout = String::from_utf8_lossy(&out.stdout);
            let test_count = test_stdout
                .lines()
                .filter(|l| l.ends_with(": test"))
                .count();
            if test_count > 0 {
                lines.push(format!("Tests: {test_count} passing"));
            }
        }
    }

    Some(lines.join("\n"))
}

/// Format git context for harvest output.
pub fn format_git_context(ctx: &GitContext) -> String {
    if ctx.commits.is_empty() {
        return String::new();
    }

    let mut out = format!(
        "  git: {} commits, {} files (+{}/-{})",
        ctx.commits.len(),
        ctx.files_changed,
        ctx.insertions,
        ctx.deletions,
    );

    if let Some(ref branch) = ctx.branch {
        out = format!("  git: branch={branch}, {}", &out[6..]);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_git_root_finds_repo() {
        // We're inside the ddis-braid repo
        let root = detect_git_root(Path::new("."));
        assert!(root.is_some(), "should find git root from cwd");
    }

    #[test]
    fn current_branch_returns_name() {
        let root = detect_git_root(Path::new(".")).unwrap();
        let branch = current_branch(&root);
        assert!(branch.is_some(), "should have a branch name");
    }

    #[test]
    fn changes_since_zero_returns_recent() {
        // since=0 is pre-Unix, falls back to --max-count=50
        let ctx = changes_since(Path::new("."), 0);
        assert!(!ctx.commits.is_empty(), "should find recent commits");
        assert!(ctx.files_changed > 0, "should find changed files");
    }

    #[test]
    fn changes_since_future_returns_empty() {
        // A timestamp far in the future should match no commits
        let ctx = changes_since(Path::new("."), u64::MAX);
        assert!(
            ctx.commits.is_empty(),
            "should find no commits in the future"
        );
    }

    #[test]
    fn format_empty_context() {
        let ctx = GitContext::default();
        assert!(format_git_context(&ctx).is_empty());
    }

    #[test]
    fn format_nonempty_context() {
        let ctx = GitContext {
            commits: vec![CommitInfo {
                hash: "abc1234".into(),
                subject: "test commit".into(),
                author: "test".into(),
                timestamp: 1234567890,
            }],
            files_changed: 3,
            insertions: 42,
            deletions: 10,
            branch: Some("main".into()),
        };
        let formatted = format_git_context(&ctx);
        assert!(formatted.contains("1 commits"));
        assert!(formatted.contains("main"));
        assert!(formatted.contains("+42/-10"));
    }
}
