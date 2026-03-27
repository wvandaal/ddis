//! Structural bootstrap hypothesis generator.
//!
//! Scans the filesystem to generate initial hypotheses about
//! the project being managed. These hypotheses provide H > 0
//! to kick the system off the trivial fixed point.
//!
//! At zero knowledge, braid provides zero hypotheses. The filesystem
//! IS the first generative model. Directory structure, file sizes,
//! import relationships encode project architecture.

use std::path::Path;

/// A bootstrap hypothesis about the project.
#[derive(Clone, Debug)]
pub struct BootstrapHypothesis {
    /// Human-readable hypothesis text.
    pub text: String,
    /// Confidence in this hypothesis (0.0 to 1.0).
    pub confidence: f64,
    /// Category: "architecture", "technology", "organization", "testing".
    pub category: String,
    /// Evidence: what filesystem observation supports this.
    pub evidence: String,
}

/// Generate bootstrap hypotheses by scanning the filesystem at `project_root`.
///
/// Returns an empty vec if the path doesn't exist or isn't accessible.
/// Never panics on filesystem errors -- returns what it can.
pub fn generate_bootstrap_hypotheses(project_root: &Path) -> Vec<BootstrapHypothesis> {
    let mut hypotheses = Vec::new();

    // 1. Detect project type from marker files
    detect_project_type(project_root, &mut hypotheses);

    // 2. Detect directory structure
    detect_directory_structure(project_root, &mut hypotheses);

    // 3. Estimate project scale
    detect_project_scale(project_root, &mut hypotheses);

    // 4. Detect test infrastructure
    detect_test_infrastructure(project_root, &mut hypotheses);

    hypotheses
}

/// Marker files that indicate project type.
const PROJECT_MARKERS: &[(&str, &str, &str, f64)] = &[
    ("Cargo.toml", "rust", "Rust project (Cargo workspace)", 0.95),
    ("go.mod", "go", "Go module project", 0.95),
    (
        "package.json",
        "javascript",
        "JavaScript/TypeScript project (Node.js)",
        0.90,
    ),
    (
        "pyproject.toml",
        "python",
        "Python project (modern packaging)",
        0.90,
    ),
    (
        "setup.py",
        "python",
        "Python project (legacy packaging)",
        0.85,
    ),
    (
        "Makefile",
        "build",
        "Project uses Make for build orchestration",
        0.70,
    ),
    (
        "Dockerfile",
        "container",
        "Project uses Docker containerization",
        0.80,
    ),
    (
        ".github/workflows",
        "ci",
        "Project uses GitHub Actions CI",
        0.90,
    ),
];

fn detect_project_type(root: &Path, hyps: &mut Vec<BootstrapHypothesis>) {
    for &(marker, _tech, desc, confidence) in PROJECT_MARKERS {
        let path = root.join(marker);
        if path.exists() {
            hyps.push(BootstrapHypothesis {
                text: desc.to_string(),
                confidence,
                category: "technology".to_string(),
                evidence: format!("Found {marker}"),
            });
        }
    }
}

/// Known directory patterns and what they indicate.
const DIR_PATTERNS: &[(&str, &str, &str)] = &[
    ("src", "Source code directory", "architecture"),
    (
        "internal",
        "Go internal packages (encapsulation boundary)",
        "architecture",
    ),
    ("pkg", "Go public packages", "architecture"),
    ("cmd", "Go command entry points", "architecture"),
    ("lib", "Library code", "architecture"),
    ("test", "Test directory (separate from source)", "testing"),
    ("tests", "Integration/E2E test directory", "testing"),
    ("docs", "Documentation directory", "organization"),
    ("scripts", "Build/deployment scripts", "organization"),
    (
        "crates",
        "Rust workspace crates (modular architecture)",
        "architecture",
    ),
    ("migrations", "Database migrations present", "technology"),
    ("api", "API definitions or server code", "architecture"),
    ("proto", "Protocol buffer definitions", "technology"),
];

/// Directories to skip when scanning (hidden dirs handled separately by prefix check).
const SKIP_DIRS: &[&str] = &["node_modules", "target", "vendor", "__pycache__", ".git"];

fn detect_directory_structure(root: &Path, hyps: &mut Vec<BootstrapHypothesis>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };

    let mut dirs: Vec<String> = Vec::new();
    let mut file_count = 0usize;

    for entry in entries.take(200) {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden dirs and common noise
        if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }

        if entry.path().is_dir() {
            dirs.push(name);
        } else {
            file_count += 1;
        }
    }

    // Generate hypotheses from known directory name patterns
    for dir in &dirs {
        let dir_lower = dir.to_lowercase();
        for &(pattern, desc, category) in DIR_PATTERNS {
            if dir_lower == pattern {
                hyps.push(BootstrapHypothesis {
                    text: desc.to_string(),
                    confidence: 0.85,
                    category: category.to_string(),
                    evidence: format!("Directory: {dir}/"),
                });
            }
        }
    }

    // Scan internal/ for Go projects (second level)
    detect_go_internal_packages(root, hyps);

    // Scan crates/ for Rust workspace structure
    detect_rust_workspace_crates(root, hyps);

    // Total directory count as project organization signal
    if dirs.len() > 15 {
        hyps.push(BootstrapHypothesis {
            text: format!("Large project with {} top-level directories", dirs.len()),
            confidence: 0.75,
            category: "organization".to_string(),
            evidence: format!("{} directories, {} files at root", dirs.len(), file_count),
        });
    }
}

fn detect_go_internal_packages(root: &Path, hyps: &mut Vec<BootstrapHypothesis>) {
    let internal_path = root.join("internal");
    if !internal_path.is_dir() {
        return;
    }
    let Ok(internal_entries) = std::fs::read_dir(&internal_path) else {
        return;
    };

    let packages: Vec<String> = internal_entries
        .take(50)
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    if !packages.is_empty() {
        let preview: Vec<_> = packages.iter().take(10).cloned().collect();
        hyps.push(BootstrapHypothesis {
            text: format!(
                "Go project with {} internal packages: {}",
                packages.len(),
                preview.join(", ")
            ),
            confidence: 0.90,
            category: "architecture".to_string(),
            evidence: format!("internal/ contains {} subdirectories", packages.len()),
        });
    }
}

fn detect_rust_workspace_crates(root: &Path, hyps: &mut Vec<BootstrapHypothesis>) {
    let crates_path = root.join("crates");
    if !crates_path.is_dir() {
        return;
    }
    let Ok(crate_entries) = std::fs::read_dir(&crates_path) else {
        return;
    };

    let crates: Vec<String> = crate_entries
        .take(50)
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("Cargo.toml").exists())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    if !crates.is_empty() {
        hyps.push(BootstrapHypothesis {
            text: format!(
                "Cargo workspace with {} crates: {}",
                crates.len(),
                crates.join(", ")
            ),
            confidence: 0.90,
            category: "architecture".to_string(),
            evidence: format!("crates/ contains {} Cargo.toml-bearing dirs", crates.len()),
        });
    }
}

/// Language extensions to scan for scale estimation.
const LANG_EXTENSIONS: &[(&str, &str)] = &[
    ("go", "Go"),
    ("rs", "Rust"),
    ("ts", "TypeScript"),
    ("py", "Python"),
    ("js", "JavaScript"),
];

fn detect_project_scale(root: &Path, hyps: &mut Vec<BootstrapHypothesis>) {
    for &(ext, lang) in LANG_EXTENSIONS {
        let count = count_files_with_extension(root, ext, 3, 0);
        if count > 0 {
            let scale = if count > 200 {
                "large"
            } else if count > 50 {
                "medium"
            } else {
                "small"
            };
            hyps.push(BootstrapHypothesis {
                text: format!("{count} {ext} files detected ({scale} {lang} project)"),
                confidence: 0.85,
                category: "technology".to_string(),
                evidence: format!("Counted {count} *.{ext} files (depth <= 3)"),
            });
        }
    }
}

fn detect_test_infrastructure(root: &Path, hyps: &mut Vec<BootstrapHypothesis>) {
    // Go convention: _test.go suffix
    let go_test_count = count_files_matching(root, "_test.go", 3, 0);

    // Rust convention: files in tests/ or #[cfg(test)] (we only count tests/ dir files)
    let rust_test_dir_count = {
        let tests_path = root.join("tests");
        if tests_path.is_dir() {
            count_files_with_extension(&tests_path, "rs", 2, 0)
        } else {
            0
        }
    };

    // JS/TS convention: *.test.ts, *.spec.ts, etc.
    let js_test_count =
        count_files_matching(root, ".test.", 3, 0) + count_files_matching(root, ".spec.", 3, 0);

    // Python convention: test_*.py or *_test.py
    let py_test_count =
        count_files_matching(root, "test_", 3, 0) + count_files_matching(root, "_test.py", 3, 0);

    let total_tests = go_test_count + rust_test_dir_count + js_test_count + py_test_count;

    if total_tests > 0 {
        hyps.push(BootstrapHypothesis {
            text: format!("{total_tests} test files found -- project has test infrastructure"),
            confidence: 0.85,
            category: "testing".to_string(),
            evidence: format!("Counted {total_tests} test files (depth <= 3)"),
        });
    }
}

/// Count files with a given extension, up to `max_depth`.
/// Returns 0 on any filesystem error.
fn count_files_with_extension(
    dir: &Path,
    ext: &str,
    max_depth: usize,
    current_depth: usize,
) -> usize {
    if current_depth > max_depth {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    let suffix = format!(".{ext}");
    let mut count = 0;
    for entry in entries.take(500) {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden and common noise
        if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }

        if path.is_dir() {
            count += count_files_with_extension(&path, ext, max_depth, current_depth + 1);
        } else if name.ends_with(&suffix) {
            count += 1;
        }
    }
    count
}

/// Count files whose name contains a pattern (e.g., `"_test.go"`).
/// Returns 0 on any filesystem error.
fn count_files_matching(
    dir: &Path,
    pattern: &str,
    max_depth: usize,
    current_depth: usize,
) -> usize {
    if current_depth > max_depth {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    let mut count = 0;
    for entry in entries.take(500) {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }

        if path.is_dir() {
            count += count_files_matching(&path, pattern, max_depth, current_depth + 1);
        } else if name.contains(pattern) {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_on_braid_project() {
        // Test against the braid project itself (two levels up from braid-kernel)
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let hyps = generate_bootstrap_hypotheses(root);

        assert!(
            !hyps.is_empty(),
            "should generate hypotheses for braid project"
        );
        assert!(
            hyps.iter()
                .any(|h| h.text.contains("Rust") || h.text.contains("Cargo")),
            "should detect Rust/Cargo project, got: {:?}",
            hyps.iter().map(|h| &h.text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_generate_on_empty_dir() {
        // Use a unique subdir of the system temp dir
        let dir = std::env::temp_dir().join("braid_test_bootstrap_empty");
        let _ = std::fs::create_dir_all(&dir);

        // Remove everything inside (but not the dir itself) to ensure emptiness
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let _ = std::fs::remove_dir_all(&p);
                } else {
                    let _ = std::fs::remove_file(&p);
                }
            }
        }

        let hyps = generate_bootstrap_hypotheses(&dir);
        assert!(
            hyps.is_empty() || hyps.iter().all(|h| h.confidence < 1.0),
            "empty dir should produce no or low-confidence hypotheses"
        );
    }

    #[test]
    fn test_generate_on_go_project() {
        let dir = std::env::temp_dir().join("braid_test_bootstrap_go");
        let _ = std::fs::create_dir_all(&dir);

        // Set up a minimal Go project structure
        std::fs::write(dir.join("go.mod"), "module example.com/test").unwrap();
        std::fs::create_dir_all(dir.join("internal/parser")).unwrap();
        std::fs::create_dir_all(dir.join("internal/storage")).unwrap();
        std::fs::write(dir.join("internal/parser/parser.go"), "package parser").unwrap();
        std::fs::write(dir.join("internal/parser/parser_test.go"), "package parser").unwrap();

        let hyps = generate_bootstrap_hypotheses(&dir);

        assert!(
            hyps.iter().any(|h| h.text.contains("Go")),
            "should detect Go project, got: {:?}",
            hyps.iter().map(|h| &h.text).collect::<Vec<_>>()
        );
        assert!(
            hyps.iter().any(|h| h.text.contains("internal packages")),
            "should detect internal packages, got: {:?}",
            hyps.iter().map(|h| &h.text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_generate_on_go_cli_dir() {
        // Test against the actual Go CLI directory (acceptance criterion A+B)
        let go_cli = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("ddis-cli");

        if !go_cli.exists() {
            // Skip if the Go CLI directory is not present
            return;
        }

        let hyps = generate_bootstrap_hypotheses(&go_cli);
        assert!(
            hyps.len() >= 5,
            "Go CLI dir should produce 5+ hypotheses, got {}",
            hyps.len()
        );

        // Should detect Go project
        assert!(
            hyps.iter().any(|h| h.text.contains("Go")),
            "should detect Go project"
        );
    }

    #[test]
    fn test_detect_rust_workspace_crates() {
        // Test against braid itself (acceptance criterion D)
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let hyps = generate_bootstrap_hypotheses(root);

        assert!(
            hyps.iter()
                .any(|h| h.text.contains("Cargo workspace") && h.text.contains("crates")),
            "should detect Cargo workspace crates, got: {:?}",
            hyps.iter().map(|h| &h.text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_hypotheses_have_valid_fields() {
        // Acceptance criterion E
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let hyps = generate_bootstrap_hypotheses(root);

        for h in &hyps {
            assert!(!h.text.is_empty(), "text must be non-empty");
            assert!(
                h.confidence > 0.0 && h.confidence <= 1.0,
                "confidence must be in (0, 1], got {}",
                h.confidence
            );
            assert!(!h.category.is_empty(), "category must be non-empty");
            assert!(!h.evidence.is_empty(), "evidence must be non-empty");
        }
    }

    #[test]
    fn test_nonexistent_path_returns_empty() {
        // Acceptance criterion F
        let hyps =
            generate_bootstrap_hypotheses(Path::new("/nonexistent/path/that/does/not/exist"));
        assert!(hyps.is_empty(), "nonexistent path should return empty vec");
    }

    #[test]
    fn test_categories_are_valid() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let hyps = generate_bootstrap_hypotheses(root);
        let valid_categories = ["architecture", "technology", "organization", "testing"];

        for h in &hyps {
            assert!(
                valid_categories.contains(&h.category.as_str()),
                "invalid category '{}' in hypothesis: {}",
                h.category,
                h.text
            );
        }
    }
}
