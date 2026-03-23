//! `braid init` — Initialize a new braid store with zero-config onboarding (WP3).
//!
//! Auto-detects git, language, tools. Optionally bootstraps spec elements.
//! Records detection results as config datoms. Idempotent: safe to re-run.
//!
//! Traces to: INV-INIT-001 (idempotency), ADR-INTERFACE-005 (config as datoms).

use std::path::Path;
use std::process::Command;

use braid_kernel::datom::{AgentId, ProvenanceType};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

/// Detection results from scanning the project environment.
struct Detection {
    git_present: bool,
    git_branch: Option<String>,
    lang: Option<&'static str>,
    total_loc: usize,
    tools: Vec<(&'static str, bool)>,
}

/// Run `braid init [path]` with zero-config onboarding.
///
/// - Creates `.braid/` with genesis transaction
/// - Detects git, language, tools
/// - Records config datoms for detection results
/// - Optionally bootstraps spec elements from spec_dir
/// - Idempotent: re-running refreshes detection without duplicating data
pub fn run(
    path: &Path,
    spec_dir: &Path,
    manifest: Option<&Path>,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::init(path)?;
    let hashes = layout.list_tx_hashes()?;
    let store = layout.load_store()?;

    let mut out = String::new();

    // Determine project root.
    // When -p is absolute, the store lives outside cwd — project root is cwd.
    // When -p is relative (default ".braid"), project root is the parent dir.
    let project_root = if path.is_absolute() {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    } else {
        let parent = path.parent().unwrap_or(Path::new("."));
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf())
    };

    // --- Detection phase ---
    let detection = detect_environment(&project_root);

    // Report init
    let is_reinit = hashes.len() > 1; // More than just genesis means re-init
    if is_reinit {
        out.push_str(&format!(
            "refreshed braid store at {}\n  existing: {} transaction(s), {} datom(s)\n",
            path.display(),
            hashes.len(),
            store.len(),
        ));
    } else {
        out.push_str(&format!(
            "initialized braid store at {}\n  genesis: {} transaction(s), {} datom(s)\n",
            path.display(),
            hashes.len(),
            store.len(),
        ));
    }

    // --- Record detection as config datoms ---
    let agent = AgentId::from_name("braid:init");
    let tx_id = super::write::next_tx_id(&store, agent);

    let mut config_datoms = Vec::new();

    // Git config
    let git_val = if detection.git_present {
        "auto"
    } else {
        "never"
    };
    config_datoms.extend(braid_kernel::config::set_config_datoms(
        "git.enabled",
        git_val,
        "project",
        tx_id,
    ));

    // Language detection
    if let Some(lang) = detection.lang {
        config_datoms.extend(braid_kernel::config::set_config_datoms(
            "project.language",
            lang,
            "project",
            tx_id,
        ));
    }

    // Tool availability
    for (tool, available) in &detection.tools {
        let key = format!("tools.{tool}");
        let val = if *available { "available" } else { "not-found" };
        config_datoms.extend(braid_kernel::config::set_config_datoms(
            &key, val, "project", tx_id,
        ));
    }

    if !config_datoms.is_empty() {
        let tx = TxFile {
            tx_id,
            agent,
            provenance: ProvenanceType::Observed,
            rationale: "init: environment detection".to_string(),
            causal_predecessors: vec![],
            datoms: config_datoms,
        };
        layout.write_tx(&tx)?;
    }

    // --- Report detection ---
    out.push_str(&format!(
        "  detected: git={}, lang={}, LOC={}\n",
        if detection.git_present { "yes" } else { "no" },
        detection.lang.unwrap_or("unknown"),
        detection.total_loc,
    ));

    if let Some(ref branch) = detection.git_branch {
        out.push_str(&format!("  git branch: {branch}\n"));
    }

    let available_tools: Vec<&str> = detection
        .tools
        .iter()
        .filter(|(_, a)| *a)
        .map(|(n, _)| *n)
        .collect();
    if !available_tools.is_empty() {
        out.push_str(&format!("  tools: {}\n", available_tools.join(", ")));
    }

    // --- Auto-bootstrap spec elements ---
    let mut bootstrap_element_count: usize = 0;
    let mut bootstrap_datom_count: usize = 0;
    if spec_dir.is_dir() {
        let elements = crate::bootstrap::parse_spec_dir(spec_dir);
        if !elements.is_empty() {
            let bootstrap_agent = AgentId::from_name("braid:bootstrap");
            let tx = crate::bootstrap::elements_to_tx(&elements, bootstrap_agent);
            let datom_count = tx.datoms.len();
            let file_path = layout.write_tx(&tx)?;

            let invs = elements
                .iter()
                .filter(|e| e.kind == crate::bootstrap::SpecElementKind::Invariant)
                .count();
            let adrs = elements
                .iter()
                .filter(|e| e.kind == crate::bootstrap::SpecElementKind::Adr)
                .count();
            let negs = elements
                .iter()
                .filter(|e| e.kind == crate::bootstrap::SpecElementKind::NegativeCase)
                .count();

            bootstrap_element_count = elements.len();
            bootstrap_datom_count = datom_count;

            out.push_str(&format!(
                "  bootstrap: {} elements ({} INV, {} ADR, {} NEG) \u{2192} {} datoms\n    \u{2192} {}\n",
                elements.len(), invs, adrs, negs, datom_count,
                file_path.relative_path(),
            ));
        }
    }

    // --- Auto-trace: populate :impl/implements datoms (D1) ---
    // Only run if source files were detected (lang + LOC > 0)
    if detection.lang.is_some() && detection.total_loc > 0 {
        match super::trace::run(path, &project_root, "braid:init", true, false) {
            Ok(trace_output) => {
                // Extract just the summary line from trace output
                if let Some(summary) = trace_output
                    .human
                    .lines()
                    .find(|l| l.starts_with("Trace scan:"))
                {
                    out.push_str(&format!("  {summary}\n"));
                } else if !trace_output.human.is_empty() {
                    out.push_str(&format!("  trace: {} LOC scanned\n", detection.total_loc));
                }
            }
            Err(e) => {
                out.push_str(&format!("  trace: skipped ({e})\n"));
            }
        }
    }

    // --- Meta-extractor: recommend extractors based on detected language (EXTRACTOR-3) ---
    // Scans for language signals and transacts :extractor/* recommendations.
    // C8: no hardcoded extractors — recommendations stored as datoms, invoked via `braid extract`.
    if let Some(lang) = detection.lang {
        let meta_agent = AgentId::from_name("braid:meta-extractor");
        let meta_tx_id = super::write::next_tx_id(&store, meta_agent);
        let mut extractor_datoms = Vec::new();
        let mut recommended = Vec::new();

        match lang {
            "rust" => {
                let eid = braid_kernel::datom::EntityId::from_ident(":extractor/rust");
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/command"), braid_kernel::datom::Value::String("braid extract rust".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/produces"), braid_kernel::datom::Value::String(":component/*".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/schedule"), braid_kernel::datom::Value::Keyword(":extractor.schedule/on-init".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/status"), braid_kernel::datom::Value::Keyword(":extractor.status/recommended".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/language"), braid_kernel::datom::Value::Keyword(":lang/rust".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                recommended.push("rust");
            }
            "go" => {
                let eid = braid_kernel::datom::EntityId::from_ident(":extractor/go");
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/command"), braid_kernel::datom::Value::String("braid extract go".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/produces"), braid_kernel::datom::Value::String(":component/*".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/schedule"), braid_kernel::datom::Value::Keyword(":extractor.schedule/on-init".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/status"), braid_kernel::datom::Value::Keyword(":extractor.status/recommended".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/language"), braid_kernel::datom::Value::Keyword(":lang/go".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                recommended.push("go");
            }
            "typescript" | "javascript" => {
                let eid = braid_kernel::datom::EntityId::from_ident(":extractor/typescript");
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/command"), braid_kernel::datom::Value::String("braid extract typescript".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/produces"), braid_kernel::datom::Value::String(":component/*".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/schedule"), braid_kernel::datom::Value::Keyword(":extractor.schedule/on-init".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/status"), braid_kernel::datom::Value::Keyword(":extractor.status/recommended".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/language"), braid_kernel::datom::Value::Keyword(":lang/typescript".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                recommended.push(lang);
            }
            "python" => {
                let eid = braid_kernel::datom::EntityId::from_ident(":extractor/python");
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/command"), braid_kernel::datom::Value::String("braid extract python".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/produces"), braid_kernel::datom::Value::String(":component/*".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/schedule"), braid_kernel::datom::Value::Keyword(":extractor.schedule/on-init".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/status"), braid_kernel::datom::Value::Keyword(":extractor.status/recommended".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                extractor_datoms.push(braid_kernel::datom::Datom::new(eid, braid_kernel::datom::Attribute::from_keyword(":extractor/language"), braid_kernel::datom::Value::Keyword(":lang/python".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
                recommended.push("python");
            }
            _ => {}
        }

        // Always recommend git extractor (language-independent)
        let git_eid = braid_kernel::datom::EntityId::from_ident(":extractor/git");
        extractor_datoms.push(braid_kernel::datom::Datom::new(git_eid, braid_kernel::datom::Attribute::from_keyword(":extractor/command"), braid_kernel::datom::Value::String("braid extract git".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
        extractor_datoms.push(braid_kernel::datom::Datom::new(git_eid, braid_kernel::datom::Attribute::from_keyword(":extractor/produces"), braid_kernel::datom::Value::String(":component/*".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
        extractor_datoms.push(braid_kernel::datom::Datom::new(git_eid, braid_kernel::datom::Attribute::from_keyword(":extractor/schedule"), braid_kernel::datom::Value::Keyword(":extractor.schedule/on-harvest".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
        extractor_datoms.push(braid_kernel::datom::Datom::new(git_eid, braid_kernel::datom::Attribute::from_keyword(":extractor/status"), braid_kernel::datom::Value::Keyword(":extractor.status/recommended".into()), meta_tx_id, braid_kernel::datom::Op::Assert));
        recommended.push("git");

        if !extractor_datoms.is_empty() {
            let tx = TxFile {
                tx_id: meta_tx_id,
                agent: meta_agent,
                provenance: ProvenanceType::Inferred,
                rationale: format!("meta-extractor: recommended extractors for {lang}"),
                causal_predecessors: vec![],
                datoms: extractor_datoms,
            };
            layout.write_tx(&tx)?;
            out.push_str(&format!(
                "  extractors: recommended [{}]\n",
                recommended.join(", ")
            ));
        }
    }

    // --- Policy manifest loading (C8, ADR-FOUNDATION-013) ---
    // Without --manifest: empty substrate (no policy datoms, F(S)=1.0).
    // With --manifest <file>: parse EDN, validate, transact policy datoms.
    // C8 proven by construction: the kernel works without any policy.
    if let Some(manifest_path) = manifest {
        if !manifest_path.exists() {
            return Err(BraidError::Validation(format!(
                "manifest file not found: {}",
                manifest_path.display()
            )));
        }
        // Copy the manifest .edn into the txns/ directory.
        // The manifest must be a valid TxFile .edn (same format as any transaction).
        let manifest_bytes = std::fs::read(manifest_path).map_err(|e| {
            BraidError::Validation(format!(
                "failed to read manifest {}: {e}",
                manifest_path.display()
            ))
        })?;

        // Write the manifest as a transaction file
        let manifest_hash = blake3::hash(&manifest_bytes);
        let hex = manifest_hash.to_hex();
        let dir = path.join("txns").join(&hex[..2]);
        std::fs::create_dir_all(&dir)?;
        let tx_path = dir.join(format!("{hex}.edn"));
        if !tx_path.exists() {
            std::fs::write(&tx_path, &manifest_bytes)?;
        }

        // Reload store to pick up the manifest, then validate
        let updated_store = layout.load_store()?;
        if let Some(config) = braid_kernel::policy::PolicyConfig::from_store(&updated_store) {
            let errors = braid_kernel::policy::validate_policy(&config);
            if errors.is_empty() {
                out.push_str(&format!(
                    "  policy: {} boundaries from {}\n",
                    config.boundaries.len(),
                    manifest_path.display()
                ));
            } else {
                out.push_str(&format!(
                    "  policy: loaded with {} warnings from {}\n",
                    errors.len(),
                    manifest_path.display()
                ));
                for e in &errors {
                    out.push_str(&format!("    warn: {}\n", e.constraint));
                }
            }
        } else {
            out.push_str(&format!(
                "  policy: loaded {} but no boundaries found\n",
                manifest_path.display()
            ));
        }
    } else {
        // Empty substrate: no policy datoms. C8 proven by construction.
        // F(S) = 1.0 (vacuously coherent when no boundaries declared).
    }

    // --- Auto-inject seed section into AGENTS.md/CLAUDE.md (D1) ---
    let agents_md = project_root.join("AGENTS.md");
    let claude_md = project_root.join("CLAUDE.md");
    let mut agents_md_created = false;
    let mut inject_target = if agents_md.is_file() {
        Some(agents_md.clone())
    } else if claude_md.is_file() {
        Some(claude_md)
    } else {
        None
    };

    // C7 self-bootstrap: create AGENTS.md with <braid-seed> tags if neither exists.
    if inject_target.is_none() {
        let minimal_content = r#"# AGENTS.md

> Use braid — append-only knowledge store with coherence verification.

## Session Lifecycle

```bash
braid status                              # Where you are + next action
braid observe "insight" --confidence 0.8  # Capture knowledge
braid harvest --commit                    # End-of-session: knowledge → datoms
braid seed --inject AGENTS.md             # Refresh this section
```

## Dynamic Store Context

<braid-seed>
<!-- braid will inject dynamic context here on `braid seed --inject AGENTS.md` -->
</braid-seed>
"#;
        if let Err(e) = std::fs::write(&agents_md, minimal_content) {
            out.push_str(&format!("  AGENTS.md: create failed ({e})\n"));
        } else {
            out.push_str("  created: AGENTS.md (with <braid-seed> tags)\n");
            agents_md_created = true;
            inject_target = Some(agents_md);
        }
    }

    if let Some(ref target) = inject_target {
        match super::seed::run_inject(path, target, "continue", 2000) {
            Ok(inject_msg) => {
                out.push_str(&format!("  seed: injected into {}\n", target.display()));
                let _ = inject_msg;
            }
            Err(e) => {
                out.push_str(&format!("  seed: inject skipped ({e})\n"));
            }
        }
    }

    // --- Next steps guidance ---
    out.push_str("\nready: braid status | workflow: observe \u{2192} harvest \u{2192} seed\n");

    // --- Build structured output ---

    // Reload store to get final counts (after config + bootstrap + trace txns)
    let final_store = layout.load_store()?;
    let final_hashes = layout.list_tx_hashes()?;
    let final_datom_count = final_store.len();
    let final_txn_count = final_hashes.len();

    let action_str = if is_reinit { "reinit" } else { "init" };

    // Collect available tools
    let available_tools: Vec<&str> = detection
        .tools
        .iter()
        .filter(|(_, a)| *a)
        .map(|(n, _)| *n)
        .collect();

    // Build JSON
    let mut json = serde_json::json!({
        "action": action_str,
        "path": path.display().to_string(),
        "store": {
            "txns": final_txn_count,
            "datoms": final_datom_count,
        },
        "detection": {
            "git": detection.git_present,
            "git_branch": detection.git_branch,
            "language": detection.lang.unwrap_or("unknown"),
            "loc": detection.total_loc,
        },
        "tools": available_tools,
    });

    // Add bootstrap info if spec elements were processed
    if bootstrap_element_count > 0 {
        json.as_object_mut().unwrap().insert(
            "bootstrap".to_string(),
            serde_json::json!({
                "elements": bootstrap_element_count,
                "datoms": bootstrap_datom_count,
            }),
        );
    }

    // Agents.md status
    let agents_md_status = if inject_target.is_some() {
        if agents_md_created {
            "created"
        } else {
            "existing"
        }
    } else {
        "null"
    };
    json.as_object_mut().unwrap().insert(
        "agents_md".to_string(),
        if agents_md_status == "null" {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(agents_md_status.to_string())
        },
    );

    let agent_output = AgentOutput {
        context: format!(
            "init: {} ({}, {} datoms)",
            path.display(),
            action_str,
            final_datom_count
        ),
        content: out.clone(),
        footer: "next: braid status | workflow: observe \u{2192} harvest \u{2192} seed".to_string(),
    };

    Ok(CommandOutput {
        json,
        agent: agent_output,
        human: out,
    })
}

/// Detect the project environment: git, language, tools.
fn detect_environment(project_root: &Path) -> Detection {
    // Git
    let git_present = project_root.join(".git").is_dir();
    let git_branch = if git_present {
        crate::git::current_branch(project_root)
    } else {
        None
    };

    // Language detection by marker files
    let lang = detect_language(project_root);

    // LOC estimate (fast: count .rs/.go/.ts/.py files)
    let total_loc = estimate_loc(project_root);

    // Tool availability
    let tools = vec![
        ("git", tool_available("git")),
        ("cargo", tool_available("cargo")),
        ("go", tool_available("go")),
        ("npm", tool_available("npm")),
        ("bun", tool_available("bun")),
    ];

    Detection {
        git_present,
        git_branch,
        lang,
        total_loc,
        tools,
    }
}

/// Detect primary language from marker files.
fn detect_language(root: &Path) -> Option<&'static str> {
    if root.join("Cargo.toml").is_file() {
        Some("rust")
    } else if root.join("go.mod").is_file() {
        Some("go")
    } else if root.join("package.json").is_file() {
        if root.join("tsconfig.json").is_file() {
            Some("typescript")
        } else {
            Some("javascript")
        }
    } else if root.join("pyproject.toml").is_file() || root.join("setup.py").is_file() {
        Some("python")
    } else {
        None
    }
}

/// Estimate LOC by counting lines in source files.
fn estimate_loc(root: &Path) -> usize {
    let output = Command::new("git")
        .args(["ls-files", "--", "*.rs", "*.go", "*.ts", "*.py", "*.js"])
        .current_dir(root)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return 0,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut total = 0usize;

    for file in stdout.lines() {
        let file = file.trim();
        if file.is_empty() {
            continue;
        }
        let full_path = root.join(file);
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            total += content.lines().count();
        }
    }

    total
}

/// Check if a tool is available in PATH (POSIX-portable).
fn tool_available(name: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {} >/dev/null 2>&1", name)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
