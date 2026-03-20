//! Topology command — compiled agent coordination (ADR-TOPOLOGY-004).
//!
//! `braid topology plan --agents N` computes a topology plan for parallel
//! agent execution with guaranteed disjoint file sets.
//!
//! Traces to: spec/19-topology.md INV-TOPOLOGY-001..005, ADR-TOPOLOGY-004.

use std::collections::BTreeMap;
use std::path::Path;

use braid_kernel::topology;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

/// Run `braid topology plan` — compute and emit a compiled topology plan.
pub fn run_plan(
    path: &Path,
    agents: usize,
    emit_seeds: bool,
    json: bool,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Compute the plan
    let plan = topology::quick_plan(&store, agents)
        .map_err(|e| BraidError::Kernel(braid_kernel::KernelError::Topology(e)))?;

    // Build task title lookup for display
    let all_tasks = braid_kernel::task::all_tasks(&store);
    let task_titles: BTreeMap<braid_kernel::EntityId, String> = all_tasks
        .iter()
        .map(|t| (t.entity, t.title.clone()))
        .collect();

    if json {
        // JSON output
        let assignments: Vec<serde_json::Value> = plan
            .assignments
            .iter()
            .map(|a| {
                let tasks: Vec<serde_json::Value> = a
                    .tasks
                    .iter()
                    .map(|t| {
                        let title = task_titles.get(t).map(|s| s.as_str()).unwrap_or("?");
                        serde_json::json!({
                            "entity": format!("{t:?}"),
                            "title": title,
                        })
                    })
                    .collect();
                let files: Vec<&str> = a.files.iter().map(|s| s.as_str()).collect();
                serde_json::json!({
                    "name": a.name,
                    "tasks": tasks,
                    "files": files,
                    "total_impact": a.total_impact,
                })
            })
            .collect();

        let json_val = serde_json::json!({
            "method": format!("{:?}", plan.method),
            "pattern": plan.pattern.to_string(),
            "agents": plan.assignments.len(),
            "total_tasks": plan.total_tasks,
            "components": plan.component_count,
            "coupling_entropy": plan.coupling_entropy,
            "effective_rank": plan.effective_rank,
            "parallelizability": plan.parallelizability,
            "assignments": assignments,
            "disjointness_verified": plan.verify_disjointness().is_ok(),
        });

        let json_str = serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| "{}".to_string());
        return Ok(CommandOutput::from_human(json_str));
    }

    // Human output
    let human = topology::format_plan_human(&plan, &task_titles);

    // Agent output
    let _agent_content = topology::format_plan_agent(&plan, &task_titles);

    let mut full_output = human;

    // Optionally emit per-agent seed prompts
    if emit_seeds {
        full_output.push_str("\n--- Per-Agent Seeds ---\n\n");
        for assignment in &plan.assignments {
            let seed =
                topology::emit_seed_for_agent(assignment, &task_titles, plan.assignments.len());
            full_output.push_str(&format!("=== {} ===\n{seed}\n", assignment.name));
        }
    }

    // ACP: topology plan as projection
    let action = braid_kernel::budget::ProjectedAction {
        command: format!("braid topology plan --agents {}", plan.assignments.len()),
        rationale: format!(
            "{}a/{}t {} p={:.2}",
            plan.assignments.len(),
            plan.total_tasks,
            plan.pattern,
            plan.parallelizability,
        ),
        impact: plan.parallelizability,
    };

    let mut context_blocks = vec![braid_kernel::budget::ContextBlock {
        precedence: braid_kernel::budget::OutputPrecedence::System,
        content: format!(
            "topology: {}a/{}t {} S={:.2} p={:.2}",
            plan.assignments.len(),
            plan.total_tasks,
            plan.pattern,
            plan.coupling_entropy,
            plan.parallelizability,
        ),
        tokens: 15,
    }];

    for a in &plan.assignments {
        context_blocks.push(braid_kernel::budget::ContextBlock {
            precedence: braid_kernel::budget::OutputPrecedence::UserRequested,
            content: format!(
                "{}: {} tasks, {} files, impact={:.2}",
                a.name,
                a.tasks.len(),
                a.files.len(),
                a.total_impact,
            ),
            tokens: 12,
        });
    }

    let projection = braid_kernel::ActionProjection {
        action,
        context: context_blocks,
        evidence_pointer: "details: braid topology plan --agents N --seeds".to_string(),
    };

    let mut json_val = if json {
        serde_json::from_str::<serde_json::Value>(
            &serde_json::to_string_pretty(&serde_json::json!({
                "method": format!("{:?}", plan.method),
                "pattern": plan.pattern.to_string(),
                "agents": plan.assignments.len(),
                "total_tasks": plan.total_tasks,
                "coupling_entropy": plan.coupling_entropy,
                "parallelizability": plan.parallelizability,
            }))
            .unwrap_or_default(),
        )
        .unwrap_or(serde_json::json!(null))
    } else {
        serde_json::json!(null)
    };

    // Set _acp field for ACP bypass
    if let serde_json::Value::Object(ref mut map) = json_val {
        map.insert("_acp".to_string(), serde_json::json!(true));
    } else {
        json_val = serde_json::json!({"_acp": true});
    }

    let rendered = crate::output::CommandOutput::from_human(String::new()).render_projected(
        crate::output::OutputMode::Agent,
        Some(&projection),
        braid_kernel::budget::ActivationStrategy::Navigate,
    );

    Ok(CommandOutput {
        json: json_val,
        agent: AgentOutput {
            context: format!(
                "topology: {}a/{}t {} S={:.2}",
                plan.assignments.len(),
                plan.total_tasks,
                plan.pattern,
                plan.coupling_entropy,
            ),
            content: rendered,
            footer: String::new(),
        },
        human: full_output,
    })
}

/// Run `braid topology deps` — transact spec dependency edges.
///
/// Parses :element/traces-to strings into :spec/traces-to Ref datoms.
/// TOPO-SPEC-DEPS: Enriches the coupling matrix for topology planning.
pub fn run_deps(path: &Path) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    use braid_kernel::datom::*;
    let agent = AgentId::from_name("braid:topology");
    let tx = crate::commands::write::next_tx_id(&store, agent);
    let (datoms, resolved, unresolved) = topology::spec_dependency_datoms(&store, tx);

    if datoms.is_empty() {
        return Ok(CommandOutput::from_human(
            "No new spec dependency edges to transact.\n".to_string(),
        ));
    }

    let datom_count = datoms.len();
    let tx_file = braid_kernel::layout::TxFile {
        tx_id: tx,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: format!("TOPO-SPEC-DEPS: {resolved} resolved, {unresolved} unresolved"),
        causal_predecessors: vec![],
        datoms,
    };
    layout.write_tx(&tx_file)?;

    let human = format!(
        "transacted: {} spec dependency edges ({} resolved, {} unresolved)\n",
        datom_count, resolved, unresolved,
    );
    let json = serde_json::json!({
        "datom_count": datom_count,
        "resolved": resolved,
        "unresolved": unresolved,
    });
    let agent_out = crate::output::AgentOutput {
        context: format!("spec deps: {resolved} resolved"),
        content: format!("{datom_count} edges transacted"),
        footer: "coupling: braid topology plan".to_string(),
    };
    let mut json_with_acp = json;
    if let serde_json::Value::Object(ref mut map) = json_with_acp {
        map.insert("_acp".to_string(), serde_json::json!(true));
    }
    Ok(CommandOutput {
        json: json_with_acp,
        agent: agent_out,
        human,
    })
}

/// Run `braid topology status` — show current topology state.
pub fn run_status(path: &Path, json: bool) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    // Check if any topology plan exists by looking for :topology/assignment datoms
    let tasks = braid_kernel::task::all_tasks(&store);
    let ready_count = tasks
        .iter()
        .filter(|t| t.status == braid_kernel::TaskStatus::Open)
        .count();

    let task_files = topology::ready_task_files(&store);
    let groups = topology::partition_by_file_coupling(&task_files);

    let has_files = task_files.values().any(|f| !f.is_empty());

    if json {
        let json_val = serde_json::json!({
            "ready_tasks": ready_count,
            "tasks_with_files": task_files.values().filter(|f| !f.is_empty()).count(),
            "components": groups.len(),
            "has_coupling_data": has_files,
        });
        let json_str = serde_json::to_string_pretty(&json_val).unwrap_or_else(|_| "{}".to_string());
        return Ok(CommandOutput::from_human(json_str));
    }

    let mut out = String::new();
    out.push_str(&format!("topology: {ready_count} ready tasks\n"));
    out.push_str(&format!(
        "  tasks with files: {}\n",
        task_files.values().filter(|f| !f.is_empty()).count(),
    ));
    out.push_str(&format!("  coupling groups: {}\n", groups.len()));

    if !has_files {
        out.push_str(
            "\nhint: Add FILE: markers to task titles for topology-aware coordination.\n\
             Example: braid task create \"Fix X. FILE: crates/a/src/b.rs\"\n",
        );
    } else {
        out.push_str("\nrun: braid topology plan --agents N\n");
    }

    let json_val = serde_json::json!({
        "_acp": true,
        "ready_tasks": ready_count,
        "coupling_groups": groups.len(),
        "has_coupling_data": has_files,
    });
    Ok(CommandOutput {
        json: json_val,
        agent: AgentOutput {
            context: format!("topology: {} ready, {} groups", ready_count, groups.len()),
            content: if has_files {
                "run: braid topology plan --agents N".to_string()
            } else {
                "hint: add FILE: markers for topology".to_string()
            },
            footer: String::new(),
        },
        human: out,
    })
}
