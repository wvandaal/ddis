//! `braid generate` — Generate dynamic CLAUDE.md from the store.
//!
//! This command implements the store-to-prompt pipeline:
//! 1. Load the store from disk
//! 2. Call `generate_claude_md()` from braid-kernel
//! 3. Write the rendered markdown to `.braid/CLAUDE.md`
//!
//! Traces to: INV-SEED-007, INV-SEED-008, INV-GUIDANCE-007.

use std::path::Path;

use braid_kernel::claude_md::{generate_claude_md, ClaudeMdConfig};
use braid_kernel::datom::AgentId;

use crate::error::BraidError;
use crate::layout::DiskLayout;

pub fn run(path: &Path, task: &str, budget: usize, agent_name: &str) -> Result<String, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let config = ClaudeMdConfig {
        task: task.to_string(),
        agent: AgentId::from_name(agent_name),
        budget,
        ..ClaudeMdConfig::default()
    };

    let generated = generate_claude_md(&store, &config);

    // Render and write the CLAUDE.md to the store directory
    let rendered = generated.render();
    let output_path = path.join("CLAUDE.md");
    std::fs::write(&output_path, &rendered)?;

    let mut out = String::new();
    out.push_str(&format!("generate: {}\n", output_path.display()));
    out.push_str(&format!("  sections: {}\n", generated.sections.len()));
    for section in &generated.sections {
        out.push_str(&format!(
            "    - {} ({} tokens)\n",
            section.heading, section.tokens
        ));
    }
    out.push_str(&format!("  total tokens: ~{}\n", generated.total_tokens));
    out.push_str(&format!(
        "  methodology score: {:.2}\n",
        generated.methodology_score
    ));

    Ok(out)
}
