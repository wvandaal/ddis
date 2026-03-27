//! `braid extract` — Invoke registered extractors to populate component datoms.
//!
//! Queries the store for `:extractor/*` entities, invokes their commands,
//! validates output, and updates `:extractor/last-run` timestamps.
//!
//! C8 compliance: extractors are stored as datoms (ADR-FOUNDATION-009).
//! The kernel knows nothing about specific extractors — they are discovered
//! at runtime from the store.
//!
//! Traces to: INV-FOUNDATION-003 (extractor invocation), ADR-FOUNDATION-009.

use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, Value};
use braid_kernel::layout::TxFile;

use crate::error::BraidError;
use crate::live_store::LiveStore;
use crate::output::{AgentOutput, CommandOutput};

/// A discovered extractor entity from the store.
#[derive(Debug)]
struct ExtractorEntity {
    entity: EntityId,
    command: String,
    produces: Vec<String>,
    status: String,
    language: Vec<String>,
    timeout_ms: Option<i64>,
    _boundary: Option<String>,
}

/// Run `braid extract [name]` — invoke extractors and populate component datoms.
pub fn run(
    path: &Path,
    filter: Option<&str>,
    commit: bool,
    agent_name: &str,
) -> Result<CommandOutput, BraidError> {
    let mut live = LiveStore::open(path)?;

    // Discover extractor entities from store
    let extractors = discover_extractors(live.store());

    if extractors.is_empty() {
        let msg = "no extractors registered — use braid transact to add :extractor/* entities";
        return Ok(CommandOutput {
            json: serde_json::json!({
                "action": "extract",
                "extractors": 0,
                "message": msg,
            }),
            agent: AgentOutput {
                context: "extract: no extractors".into(),
                content: msg.to_string(),
                footer: "register: braid transact with :extractor/command attribute".into(),
            },
            human: msg.to_string(),
        });
    }

    // Filter by name if specified
    let active: Vec<&ExtractorEntity> = if let Some(name) = filter {
        extractors
            .iter()
            .filter(|e| {
                e.command.contains(name)
                    || e.language.iter().any(|l| l.contains(name))
                    || e.produces.iter().any(|p| p.contains(name))
            })
            .collect()
    } else {
        extractors
            .iter()
            .filter(|e| e.status != ":extractor.status/disabled")
            .collect()
    };

    let mut out = String::new();
    let mut results = Vec::new();
    let mut total_datoms = 0usize;

    let project_root = path
        .parent()
        .unwrap_or(Path::new("."))
        .canonicalize()
        .unwrap_or_else(|_| path.parent().unwrap_or(Path::new(".")).to_path_buf());

    for ext in &active {
        let start = std::time::Instant::now();
        out.push_str(&format!("  extracting: {} ...", ext.command));

        // Invoke the extractor command
        let timeout_secs = ext.timeout_ms.unwrap_or(30_000) / 1000;
        let cmd_result = Command::new("sh")
            .args(["-c", &ext.command])
            .current_dir(&project_root)
            .env("BRAID_STORE", path.display().to_string())
            .output();

        let elapsed = start.elapsed();

        match cmd_result {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let line_count = stdout.lines().count();
                out.push_str(&format!(
                    " ok ({} lines, {:.1}s)\n",
                    line_count,
                    elapsed.as_secs_f64()
                ));

                // If --commit, update last-run timestamp
                if commit {
                    let agent = AgentId::from_name(agent_name);
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let tx_id = super::write::next_tx_id(live.store(), agent);
                    let tx = TxFile {
                        tx_id,
                        agent,
                        provenance: ProvenanceType::Observed,
                        rationale: format!("extract: updated last-run for {}", ext.command),
                        causal_predecessors: vec![],
                        datoms: vec![Datom::new(
                            ext.entity,
                            Attribute::from_keyword(":extractor/last-run"),
                            Value::Instant(now),
                            tx_id,
                            Op::Assert,
                        )],
                    };
                    let _ = live.write_tx(&tx);
                    total_datoms += 1;
                }

                results.push(serde_json::json!({
                    "command": ext.command,
                    "status": "ok",
                    "lines": line_count,
                    "elapsed_ms": elapsed.as_millis(),
                }));
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let code = output.status.code().unwrap_or(-1);
                out.push_str(&format!(" FAILED (exit {})\n", code));
                if !stderr.is_empty() {
                    out.push_str(&format!(
                        "    stderr: {}\n",
                        stderr.lines().next().unwrap_or("")
                    ));
                }
                results.push(serde_json::json!({
                    "command": ext.command,
                    "status": "failed",
                    "exit_code": code,
                    "elapsed_ms": elapsed.as_millis(),
                }));
            }
            Err(e) => {
                out.push_str(&format!(" ERROR ({})\n", e));
                results.push(serde_json::json!({
                    "command": ext.command,
                    "status": "error",
                    "error": e.to_string(),
                }));
            }
        }
        let _ = timeout_secs; // Used for future timeout enforcement
    }

    let summary = format!(
        "extract: {} extractors invoked, {} datoms committed",
        active.len(),
        total_datoms
    );
    out.push_str(&format!("\n{summary}\n"));

    Ok(CommandOutput {
        json: serde_json::json!({
            "action": "extract",
            "extractors": active.len(),
            "datoms_committed": total_datoms,
            "results": results,
        }),
        agent: AgentOutput {
            context: summary.clone(),
            content: out.clone(),
            footer: "status: braid status | register: braid transact :extractor/* datoms".into(),
        },
        human: out,
    })
}

/// Discover extractor entities from the store.
fn discover_extractors(store: &braid_kernel::store::Store) -> Vec<ExtractorEntity> {
    let cmd_attr = Attribute::from_keyword(":extractor/command");
    let cmd_datoms = store.attribute_datoms(&cmd_attr);

    let mut extractors = Vec::new();

    for d in cmd_datoms {
        if d.op != Op::Assert {
            continue;
        }
        let command = match &d.value {
            Value::String(s) => s.clone(),
            _ => continue,
        };
        let entity = d.entity;
        let entity_datoms = store.entity_datoms(entity);

        let mut produces = Vec::new();
        let mut language = Vec::new();

        for ed in &entity_datoms {
            if ed.op != Op::Assert {
                continue;
            }
            match ed.attribute.as_str() {
                ":extractor/produces" => {
                    if let Value::String(s) = &ed.value {
                        produces.push(s.clone());
                    }
                }
                ":extractor/language" => {
                    if let Value::Keyword(k) = &ed.value {
                        language.push(k.clone());
                    }
                }
                _ => {}
            }
        }

        let status = entity_datoms
            .iter()
            .rfind(|ed| ed.attribute.as_str() == ":extractor/status" && ed.op == Op::Assert)
            .and_then(|ed| match &ed.value {
                Value::Keyword(k) => Some(k.clone()),
                _ => None,
            })
            .unwrap_or_else(|| ":extractor.status/active".to_string());

        let timeout_ms = entity_datoms
            .iter()
            .rfind(|ed| ed.attribute.as_str() == ":extractor/timeout-ms" && ed.op == Op::Assert)
            .and_then(|ed| match ed.value {
                Value::Long(n) => Some(n),
                _ => None,
            });

        let boundary = entity_datoms
            .iter()
            .rfind(|ed| ed.attribute.as_str() == ":extractor/boundary" && ed.op == Op::Assert)
            .and_then(|ed| match &ed.value {
                Value::String(s) => Some(s.clone()),
                _ => None,
            });

        extractors.push(ExtractorEntity {
            entity,
            command,
            produces,
            status,
            language,
            timeout_ms,
            _boundary: boundary,
        });
    }

    extractors
}
