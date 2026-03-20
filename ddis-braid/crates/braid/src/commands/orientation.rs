//! `braid --orientation`: Optimal seed turn for agent onboarding (Phase 2C, INV-ORIENTATION-001).
//!
//! The orientation prompt is the agent's FIRST interaction with braid after installation.
//! Per trajectory-dynamics (prompt-optimization Rule 8), turns 1-2 establish the activation
//! basin for the entire session. Every line is a demonstration, not an instruction.
//!
//! Design:
//! - Demonstrations encode format, style, depth, and workflow — more information per
//!   attention unit than a constraint list (prompt-optimization Rule 2).
//! - When a store exists, demonstrations use LIVE metrics (not placeholders).
//! - The three-part agent structure (context/content/footer) applies even here.

use crate::output::OutputMode;

/// Build the orientation prompt — the optimal seed turn.
///
/// When a .braid store exists, weaves live metrics into demonstrations.
/// Otherwise, guides toward `braid init`.
pub fn build_orientation(mode: OutputMode) -> String {
    let info = load_store_info();

    match mode {
        OutputMode::Json => build_json(&info),
        OutputMode::Tsv => {
            let json: serde_json::Value =
                serde_json::from_str(&build_json(&info)).unwrap_or_default();
            braid_kernel::budget::json_to_tsv(&json)
        }
        OutputMode::Agent => build_agent(&info),
        OutputMode::Human => build_human(&info),
    }
}

struct StoreInfo {
    exists: bool,
    datom_count: usize,
    entity_count: usize,
    fitness: String,
    methodology: String,
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
                methodology: "N/A".into(),
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
                methodology: "N/A".into(),
            }
        }
    };

    let datom_count = store.datom_set().len();
    let entity_count = store.entity_count();
    let history = braid_kernel::bilateral::load_trajectory(&store);
    let cycle = braid_kernel::bilateral::run_cycle(&store, &history, false);
    let fitness = format!("{:.2}", cycle.fitness.total);
    let methodology = format!("{:.2}", cycle.fitness.components.harvest_quality);

    StoreInfo {
        exists: true,
        datom_count,
        entity_count,
        fitness,
        methodology,
    }
}

/// Agent mode: three-part navigative output with demonstrations.
///
/// Each command shown includes its expected output — a micro-demonstration
/// that activates understanding of what the command DOES, not just what it IS.
fn build_agent(info: &StoreInfo) -> String {
    let mut out = String::new();

    // Context: identity in spec-language
    out.push_str("braid — append-only datom store for human/AI coherence verification\n");

    // Store state (live or setup instruction)
    if info.exists {
        out.push_str(&format!(
            "store: {} datoms, {} entities, F(S)={}, M(t)={}\n",
            info.datom_count, info.entity_count, info.fitness, info.methodology
        ));
    } else {
        out.push_str("store: not initialized\n");
    }

    out.push('\n');

    // Content: demonstration-first workflow
    // Each line shows command → expected output (the demonstration encodes the behavior)
    if info.exists {
        out.push_str(&format!(
            "Session lifecycle (demonstrations with live metrics):
  braid status              → store: {} datoms, F(S)={}, M(t)={}, next action
  braid observe \"insight\"   → entity :observation/insight-hash (confidence: 0.7)
  braid next                → top ready task + claim command
  braid harvest --commit    → candidates crystallized into datoms
  braid seed --inject AGENTS.md → context refreshed for next session

Knowledge model: datom [entity, attribute, value, tx, op] — append-only, CRDT merge = set union
Verification: F(S) = weighted sum of coverage, validation, drift, harvest, contradiction, uncertainty
",
            info.datom_count, info.fitness, info.methodology,
        ));
    } else {
        out.push_str(
            "Quick start:
  braid init                → .braid/ + AGENTS.md + environment detection
  braid status              → store health, coherence score, next action
  braid observe \"insight\"   → entity :observation/insight-hash (confidence: 0.7)
  braid harvest --commit    → session knowledge crystallized into datoms
  braid seed --inject AGENTS.md → context refreshed for next session

Knowledge model: datom [entity, attribute, value, tx, op] — append-only, CRDT merge = set union
",
        );
    }

    out.push('\n');

    // Footer: single next action
    if info.exists {
        out.push_str("start: braid status | help: braid <command> --help\n");
    } else {
        out.push_str("start: braid init | help: braid <command> --help\n");
    }

    out
}

/// Human mode: compact workflow guide with progressive disclosure.
fn build_human(info: &StoreInfo) -> String {
    let mut out = String::new();

    out.push_str("Braid: append-only datom store for human/AI coherence verification.\n\n");

    if info.exists {
        out.push_str(&format!(
            "Store: {} datoms, {} entities, F(S)={}, M(t)={}\n\n",
            info.datom_count, info.entity_count, info.fitness, info.methodology,
        ));
    } else {
        out.push_str("Store: not initialized. Run `braid init` to create.\n\n");
    }

    out.push_str(
        "Workflow:
  START:   braid session start --task \"your task\"
  WORK:    braid observe \"insight\" --confidence 0.8
           braid next \u{2192} braid go <id> \u{2192} work \u{2192} braid done <id>
  CHECK:   braid status
  END:     braid harvest --commit && braid seed --inject AGENTS.md

Commands:
  braid status              Dashboard: F(S), M(t), tasks, next action
  braid observe \"...\"       Capture knowledge (decisions, questions, findings)
  braid note \"...\"          Quick observation (shortcut, confidence 0.7)
  braid query               Datalog or entity/attribute filter
  braid bilateral           Coherence scan: F(S) + CC-1..5
  braid next                Show top ready task with claim command
  braid go <id>             Claim a task (set to in-progress)
  braid done <id>           Close a task
  braid harvest --commit    Extract session knowledge into store
  braid seed --inject FILE  Refresh context section in AGENTS.md

Protocol: observe decisions/questions during work, harvest before ending session.
",
    );

    out
}

fn build_json(info: &StoreInfo) -> String {
    let json = serde_json::json!({
        "tool": "braid",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Append-only datom store for human/AI coherence verification",
        "store": {
            "exists": info.exists,
            "datoms": info.datom_count,
            "entities": info.entity_count,
            "fitness": info.fitness,
            "methodology": info.methodology,
        },
        "workflow": {
            "phases": ["start", "work", "check", "end"],
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
            {"name": "status", "demo": "braid status → store: N datoms, F(S)=0.77, next action"},
            {"name": "observe", "demo": "braid observe \"insight\" → entity :observation/... (confidence: 0.7)"},
            {"name": "note", "demo": "braid note \"quick note\" → entity :observation/... (confidence: 0.7)"},
            {"name": "query", "demo": "braid query '[:find ?e :where [?e :spec/type \"invariant\"]]' → results"},
            {"name": "bilateral", "demo": "braid bilateral → F(S)=0.77, CC=4/5, coverage/validation/drift"},
            {"name": "next", "demo": "braid next → top ready task + braid go <id>"},
            {"name": "harvest", "demo": "braid harvest --commit → N candidates crystallized"},
            {"name": "seed", "demo": "braid seed --inject AGENTS.md → context refreshed"},
        ],
        "model": "datom [entity, attribute, value, tx, op] — append-only, CRDT merge = set union",
        "protocol": "Observe decisions/questions during work, harvest before ending session.",
    });
    serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
}
