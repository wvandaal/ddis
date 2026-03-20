//! `braid seed` — Assemble a seed context for a new session.
//!
//! Three modes:
//! - **Structured** (default): sections with State, Constraints, Orientation, etc.
//! - **Human** (`--for-human`): narrative briefing < 200 words with actions.
//! - **Agent MD** (`--agent-md`): generate dynamic AGENTS.md from store state.

use std::path::Path;

use braid_kernel::agent_md::{generate_agent_md, AgentMdConfig};
use braid_kernel::datom::AgentId;
use braid_kernel::guidance::{derive_actions, format_actions};
use braid_kernel::seed::{assemble_seed, group_state_entries, ContextSection};
use braid_kernel::trilateral::check_coherence_fast;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

pub fn run(
    path: &Path,
    task: &str,
    budget: usize,
    agent_name: &str,
    for_human: bool,
    json: bool,
    agent_md: bool,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let agent = AgentId::from_name(agent_name);
    let seed = assemble_seed(&store, task, budget, agent);

    if json {
        let human = format_json(&seed, budget)?;
        let json_val = serde_json::from_str::<serde_json::Value>(&human)
            .unwrap_or_else(|_| serde_json::json!({ "output": &human }));
        let agent_out = AgentOutput {
            context: format!(
                "seed --json: {} entities, {} tokens",
                seed.entities_discovered, seed.context.total_tokens
            ),
            content: human.clone(),
            footer: "start: braid session start | observe: braid observe '...'".to_string(),
        };
        return Ok(CommandOutput {
            json: json_val,
            agent: agent_out,
            human,
        });
    }

    if for_human {
        let human = format_human_briefing(&store, &seed, task)?;
        let json_val = serde_json::json!({
            "mode": "human-briefing",
            "task": seed.task,
            "entities_discovered": seed.entities_discovered,
            "tokens": seed.context.total_tokens,
        });
        let agent_out = AgentOutput {
            context: format!("seed --for-human: \"{}\"", seed.task),
            content: human.clone(),
            footer: "start: braid session start | observe: braid observe '...'".to_string(),
        };
        return Ok(CommandOutput {
            json: json_val,
            agent: agent_out,
            human,
        });
    }

    // Structured output (default)
    let mut out = String::new();
    out.push_str(&format!("seed for: {}\n", seed.task));
    out.push_str(&format!(
        "  entities discovered: {}\n",
        seed.entities_discovered
    ));
    out.push_str(&format!(
        "  tokens: {} / {} (remaining: {})\n",
        seed.context.total_tokens, budget, seed.context.budget_remaining,
    ));
    out.push_str(&format!(
        "  projection: {:?}\n\n",
        seed.context.projection_pattern
    ));

    for section in &seed.context.sections {
        match section {
            ContextSection::Orientation(text) => {
                out.push_str(&format!("## Orientation\n{text}\n\n"));
            }
            ContextSection::Constraints(refs) => {
                if !refs.is_empty() {
                    // When ALL constraints have unknown status, compress to a single
                    // dense line — individual [?] markers waste tokens for zero information.
                    let any_known = refs.iter().any(|r| r.satisfied.is_some());
                    if any_known {
                        out.push_str("## Constraints\n");
                        for (i, r) in refs.iter().enumerate() {
                            // Only show status markers for constraints with known status;
                            // unknown (None) constraints get no bracket noise.
                            let prefix = match r.satisfied {
                                Some(true) => "[ok] ",
                                Some(false) => "[!!] ",
                                None => "",
                            };
                            if r.summary.is_empty() {
                                out.push_str(&format!("  {}{}\n", prefix, r.id));
                            } else {
                                out.push_str(&format!("  {}{}: {}\n", prefix, r.id, r.summary));
                            }
                            if i < 3 {
                                if let Some(ref stmt) = r.statement {
                                    out.push_str(&format!("    > {stmt}\n"));
                                }
                            }
                        }
                        out.push('\n');
                    } else {
                        // Compressed format: IDs with summaries, one per line, no [?] noise
                        out.push_str("## Active Constraints\n");
                        for (i, r) in refs.iter().take(8).enumerate() {
                            if r.summary.is_empty() {
                                out.push_str(&format!("- {}\n", r.id));
                            } else {
                                out.push_str(&format!("- {} ({})\n", r.id, r.summary));
                            }
                            if i < 3 {
                                if let Some(ref stmt) = r.statement {
                                    out.push_str(&format!("  > {stmt}\n"));
                                }
                            }
                        }
                        if refs.len() > 8 {
                            out.push_str(&format!("  ... and {} more\n", refs.len() - 8));
                        }
                        out.push('\n');
                    }
                }
            }
            ContextSection::State(entries) => {
                if !entries.is_empty() {
                    out.push_str("## State\n");
                    // E6: Group entities by semantic type for comprehension
                    let groups = group_state_entries(entries);
                    for (label, group) in &groups {
                        if !label.is_empty() {
                            out.push_str(&format!("{label}:\n"));
                        }
                        for entry in group {
                            out.push_str(&format!("  {}\n", entry.content));
                        }
                    }
                    out.push('\n');
                }
            }
            ContextSection::Warnings(warnings) => {
                if !warnings.is_empty() {
                    out.push_str("## Warnings\n");
                    for w in warnings {
                        out.push_str(&format!("  - {w}\n"));
                    }
                    out.push('\n');
                }
            }
            ContextSection::Directive(text) => {
                out.push_str(&format!("## Directive\n{text}\n"));
            }
        }
    }

    // Agent MD generation: produce dynamic AGENTS.md alongside seed
    if agent_md {
        let config = AgentMdConfig {
            task: task.to_string(),
            agent,
            budget,
            ..AgentMdConfig::default()
        };
        let generated = generate_agent_md(&store, &config);
        let rendered = generated.render();
        let output_path = path.join(&config.output_filename);
        std::fs::write(&output_path, &rendered)?;

        out.push_str(&format!(
            "\nagent-md: {} ({} sections, ~{} tokens) → {}\n",
            config.output_filename,
            generated.sections.len(),
            generated.total_tokens,
            output_path.display(),
        ));
    }

    // Build section names for structured JSON
    let section_names: Vec<&str> = seed
        .context
        .sections
        .iter()
        .map(|s| match s {
            ContextSection::Orientation(_) => "orientation",
            ContextSection::Constraints(_) => "constraints",
            ContextSection::State(_) => "state",
            ContextSection::Warnings(_) => "warnings",
            ContextSection::Directive(_) => "directive",
        })
        .collect();

    // --- ACP: Build ActionProjection (INV-BUDGET-007) ---
    // Derive the first next-action from guidance
    let actions = derive_actions(&store);
    let (action_cmd, action_rationale, action_impact) = if let Some(a) = actions.first() {
        (
            a.command.clone().unwrap_or_else(|| "braid status".to_string()),
            a.summary.clone(),
            0.5,
        )
    } else {
        (
            "braid session start".to_string(),
            "begin working session".to_string(),
            0.3,
        )
    };

    let action = braid_kernel::budget::ProjectedAction {
        command: action_cmd,
        rationale: action_rationale,
        impact: action_impact,
    };

    let mut context_blocks = Vec::new();

    // Seed summary (System)
    context_blocks.push(braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::System,
        content: format!(
            "seed: \"{}\" | {} tokens/{} budget | {} entities",
            seed.task,
            seed.context.total_tokens,
            budget,
            store.entity_count(),
        ),
        tokens: 15,
    });

    // Seed sections as context (Methodology)
    for section in &seed.context.sections {
        let (label, snippet) = match section {
            ContextSection::Orientation(text) => {
                let short = if text.len() > 80 {
                    format!("{}...", &text[..77.min(text.len())])
                } else {
                    text.clone()
                };
                ("orientation", short)
            }
            ContextSection::Constraints(refs) => {
                let ids: Vec<&str> = refs.iter().take(5).map(|r| r.id.as_str()).collect();
                ("constraints", ids.join(", "))
            }
            ContextSection::State(entries) => (
                "state",
                format!("{} entries", entries.len()),
            ),
            ContextSection::Warnings(ws) => {
                if ws.is_empty() {
                    continue;
                }
                ("warnings", format!("{} items", ws.len()))
            }
            ContextSection::Directive(text) => {
                let short = if text.len() > 80 {
                    format!("{}...", &text[..77.min(text.len())])
                } else {
                    text.clone()
                };
                ("directive", short)
            }
        };
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::Methodology,
            content: format!("{label}: {snippet}"),
            tokens: 10,
        });
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: "start: braid session start | observe: braid observe '...' --confidence 0.X".to_string(),
    };

    // Agent output uses ACP Navigate projection
    let agent_text = projection.project_at_strategy(braid_kernel::ActivationStrategy::Navigate);
    let agent_out = AgentOutput {
        context: String::new(),
        content: agent_text,
        footer: String::new(),
    };

    // JSON with _acp merged
    let mut json = serde_json::json!({
        "mode": "seed",
        "task": seed.task,
        "tokens": seed.context.total_tokens,
        "budget": budget,
        "datoms": store.len(),
        "entities": store.entity_count(),
        "sections": section_names,
    });
    if let serde_json::Value::Object(ref mut map) = json {
        let acp = projection.to_json();
        if let serde_json::Value::Object(acp_map) = acp {
            for (k, v) in acp_map {
                map.insert(k, v);
            }
        }
    }

    Ok(CommandOutput {
        json,
        agent: agent_out,
        human: out,
    })
}

/// Inject seed context into a file's `<braid-seed>` tags (SB.3.3).
///
/// Reads the target file, finds the injection point, generates content
/// from the store, replaces the tagged section, and writes back.
/// Content outside the tags is never modified (lens law).
pub fn run_inject(
    store_path: &Path,
    inject_path: &Path,
    task: &str,
    budget: usize,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(store_path)?;
    let store = layout.load_store()?;

    // Read the target file
    let file_content = std::fs::read_to_string(inject_path)?;

    // Find injection point
    let point = crate::inject::find_injection_point(&file_content)
        .map_err(|e| BraidError::Parse(format!("{e}")))?;

    // Generate content
    let content = crate::inject::format_for_injection(&store, Some(task), budget);
    let token_estimate = content.split_whitespace().count() * 4 / 3; // ~tokens

    // Inject seed content
    let result = crate::inject::inject(&file_content, &point, &content);

    // Inject methodology section (DMP: INV-GUIDANCE-022)
    // k_eff estimated from session telemetry (turn count since last harvest)
    let ctx = braid_kernel::guidance::GuidanceContext::from_store(&store, None);
    let k_eff = ctx.k_eff;
    let result = crate::inject::inject_methodology(&result, &store, k_eff);

    // Write back
    std::fs::write(inject_path, &result)?;

    let target = inject_path.display().to_string();
    let human = format!(
        "injected: ~{} tokens → {} ({} datoms, {} entities)\n",
        token_estimate,
        target,
        store.len(),
        store.entity_count(),
    );

    let json = serde_json::json!({
        "mode": "inject",
        "target": target,
        "tokens": token_estimate,
        "datoms": store.len(),
        "entities": store.entity_count(),
    });

    let agent_out = AgentOutput {
        context: format!("injected: ~{} tokens → {}", token_estimate, target),
        content: human.clone(),
        footer: format!(
            "verify: cat {} | refresh: braid seed --inject {}",
            target, target,
        ),
    };

    Ok(CommandOutput {
        json,
        agent: agent_out,
        human,
    })
}

/// Format a narrative briefing for human or LLM orientation.
///
/// Target: < 200 words, structured as:
/// 1. Store health summary (one line)
/// 2. Task context (one line)
/// 3. Open questions / warnings (numbered)
/// 4. Priority action (from guidance)
fn format_human_briefing(
    store: &braid_kernel::Store,
    seed: &braid_kernel::SeedOutput,
    task: &str,
) -> Result<String, BraidError> {
    let coherence = check_coherence_fast(store);
    let actions = derive_actions(store);

    let mut out = String::new();
    out.push_str("## Session Briefing\n\n");

    // Store health
    out.push_str(&format!(
        "**Store**: {} datoms, {} entities. ",
        store.len(),
        store.entity_count(),
    ));
    out.push_str(&format!(
        "Coherence: {:?} (Phi={:.1}, B1={}).\n\n",
        coherence.quadrant, coherence.phi, coherence.beta_1,
    ));

    // Task
    out.push_str(&format!(
        "**Task**: {} ({} relevant entities found)\n\n",
        task, seed.entities_discovered,
    ));

    // Warnings from seed
    let warnings: Vec<&str> = seed
        .context
        .sections
        .iter()
        .filter_map(|s| match s {
            ContextSection::Warnings(ws) => Some(ws.iter().map(|w| w.as_str()).collect::<Vec<_>>()),
            _ => None,
        })
        .flatten()
        .collect();

    if !warnings.is_empty() {
        out.push_str("**Open questions**:\n");
        for (i, w) in warnings.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, w));
        }
        out.push('\n');
    }

    // Actions from guidance
    if !actions.is_empty() {
        out.push_str(&format_actions(&actions));
    }

    Ok(out)
}

/// Format seed output as JSON.
fn format_json(seed: &braid_kernel::SeedOutput, budget: usize) -> Result<String, BraidError> {
    let mut sections = Vec::new();

    for section in &seed.context.sections {
        match section {
            ContextSection::Orientation(text) => {
                sections.push(serde_json::json!({
                    "type": "orientation",
                    "text": text,
                }));
            }
            ContextSection::Constraints(refs) => {
                let constraints: Vec<serde_json::Value> = refs
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "id": r.id,
                            "summary": r.summary,
                            "satisfied": r.satisfied,
                        })
                    })
                    .collect();
                sections.push(serde_json::json!({
                    "type": "constraints",
                    "items": constraints,
                }));
            }
            ContextSection::State(entries) => {
                let items: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "content": e.content,
                            "tokens": e.tokens,
                            "projection": format!("{:?}", e.projection),
                        })
                    })
                    .collect();
                sections.push(serde_json::json!({
                    "type": "state",
                    "items": items,
                }));
            }
            ContextSection::Warnings(warnings) => {
                sections.push(serde_json::json!({
                    "type": "warnings",
                    "items": warnings,
                }));
            }
            ContextSection::Directive(text) => {
                sections.push(serde_json::json!({
                    "type": "directive",
                    "text": text,
                }));
            }
        }
    }

    let result = serde_json::json!({
        "task": seed.task,
        "entities_discovered": seed.entities_discovered,
        "tokens_used": seed.context.total_tokens,
        "budget": budget,
        "budget_remaining": seed.context.budget_remaining,
        "projection": format!("{:?}", seed.context.projection_pattern),
        "sections": sections,
    });

    Ok(serde_json::to_string_pretty(&result).unwrap() + "\n")
}
