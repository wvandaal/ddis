//! `braid --orientation`: Complete agent onboarding prompt (WP4, INV-ORIENTATION-001).
//!
//! Outputs the full workflow guide an AI agent needs to become productive with braid.
//! If a store exists, interpolates live metrics. If not, shows setup instructions.

use crate::output::OutputMode;

/// Build the orientation prompt, optionally with live store metrics.
pub fn build_orientation(mode: OutputMode) -> String {
    // Try to load store for live metrics
    let store_info = load_store_info();

    match mode {
        OutputMode::Json => build_json(&store_info),
        OutputMode::Agent | OutputMode::Human => build_text(&store_info),
    }
}

struct StoreInfo {
    exists: bool,
    datom_count: usize,
    entity_count: usize,
    fitness: String,
}

fn load_store_info() -> StoreInfo {
    let path = std::path::Path::new(".braid");
    let layout = match crate::layout::DiskLayout::open(path) {
        Ok(l) => l,
        Err(_) => {
            return StoreInfo {
                exists: false,
                datom_count: 0,
                entity_count: 0,
                fitness: "N/A".into(),
            }
        }
    };
    let store = match layout.load_store() {
        Ok(s) => s,
        Err(_) => {
            return StoreInfo {
                exists: false,
                datom_count: 0,
                entity_count: 0,
                fitness: "N/A".into(),
            }
        }
    };

    let datom_count = store.datom_set().len();
    let entity_count = store.entity_count();
    let history = braid_kernel::bilateral::load_trajectory(&store);
    let cycle = braid_kernel::bilateral::run_cycle(&store, &history, false);
    let fitness = format!("{:.2}", cycle.fitness.total);

    StoreInfo {
        exists: true,
        datom_count,
        entity_count,
        fitness,
    }
}

fn build_text(info: &StoreInfo) -> String {
    let store_line = if info.exists {
        format!(
            "Store: {} datoms, {} entities, F(S)={}",
            info.datom_count, info.entity_count, info.fitness
        )
    } else {
        "Store: not initialized (run `braid init` first)".into()
    };

    format!(
        r#"Braid: append-only datom store for maintaining coherence between intent, spec, and implementation.

Workflow:
  START:   braid session start --task "your task"
  WORK:    braid observe "insight" --confidence 0.8
           braid next → braid go <id> → work → braid done <id>
  CHECK:   braid status
  END:     braid harvest --commit && braid seed --inject AGENTS.md

Key commands:
  braid status              Dashboard: store health, coherence, next action
  braid observe "..."       Capture knowledge (decisions, questions, findings)
  braid note "..."          Quick observation (shortcut, confidence 0.7)
  braid next                Show top ready task with claim command
  braid go <id>             Claim a task (set to in-progress)
  braid done <id>           Close a task
  braid harvest --commit    Extract session knowledge into store
  braid seed --inject FILE  Refresh context section in AGENTS.md/CLAUDE.md

{store_line}
Protocol: observe decisions/questions during work, harvest before ending session.
"#
    )
}

fn build_json(info: &StoreInfo) -> String {
    let json = serde_json::json!({
        "tool": "braid",
        "description": "Append-only datom store for maintaining coherence between intent, spec, and implementation",
        "store": {
            "exists": info.exists,
            "datoms": info.datom_count,
            "entities": info.entity_count,
            "fitness": info.fitness,
        },
        "workflow": {
            "start": "braid session start --task \"your task\"",
            "observe": "braid observe \"insight\" --confidence 0.8",
            "note": "braid note \"quick observation\"",
            "next_task": "braid next",
            "claim_task": "braid go <id>",
            "close_task": "braid done <id>",
            "check": "braid status",
            "end": "braid harvest --commit && braid seed --inject AGENTS.md",
        },
        "commands": [
            {"name": "status", "description": "Dashboard: store health, coherence, next action"},
            {"name": "observe", "description": "Capture knowledge (decisions, questions, findings)"},
            {"name": "note", "description": "Quick observation (shortcut, confidence 0.7)"},
            {"name": "next", "description": "Show top ready task with claim command"},
            {"name": "go", "description": "Claim a task (set to in-progress)"},
            {"name": "done", "description": "Close a task"},
            {"name": "harvest", "description": "Extract session knowledge into store"},
            {"name": "seed", "description": "Refresh context section in AGENTS.md"},
        ],
        "protocol": "Observe decisions/questions during work, harvest before ending session.",
    });
    serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
}
