//! `braid seed` — Assemble a seed context for a new session.

use std::path::Path;

use braid_kernel::datom::AgentId;
use braid_kernel::seed::{assemble_seed, ContextSection};

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(path: &Path, task: &str, budget: usize, agent_name: &str) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let agent = AgentId::from_name(agent_name);
    let seed = assemble_seed(&store, task, budget, agent);

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
                    out.push_str("## Constraints\n");
                    for r in refs {
                        let status = match r.satisfied {
                            Some(true) => "✓",
                            Some(false) => "✗",
                            None => "?",
                        };
                        out.push_str(&format!("  [{status}] {}: {}\n", r.id, r.summary));
                    }
                    out.push('\n');
                }
            }
            ContextSection::State(entries) => {
                if !entries.is_empty() {
                    out.push_str("## State\n");
                    for entry in entries {
                        out.push_str(&format!("{}\n", entry.content));
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

    Ok(out)
}
