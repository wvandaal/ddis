//! `braid verify` -- Verification status and coherence test generation.
//!
//! Two modes:
//!
//! - `braid verify` (no flags): Show current verification status -- how many
//!   spec elements exist, how many matched a mathematical pattern, coverage %.
//!
//! - `braid verify --generate`: Run the coherence compiler's pattern detection
//!   on spec elements in the store, extract test properties for each match, and
//!   emit a Rust test module to stdout.
//!
//! Traces to:
//! - SEED.md S7 (Self-Improvement Loop): automated coherence verification
//! - INV-BILATERAL-005: Test results as datoms
//! - ADR-FOUNDATION-005: Structural over procedural coherence

use std::path::Path;

use braid_kernel::compiler::{
    detect_patterns, emit_test_module, extract_test_property, summarize_patterns, PatternMatch,
};
use braid_kernel::datom::{Attribute, Op};
use braid_kernel::store::Store;

use crate::error::BraidError;
use crate::layout::DiskLayout;
use crate::output::{AgentOutput, CommandOutput};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the namespace portion from a spec ID like "INV-STORE-001" -> "STORE".
fn extract_namespace(spec_id: &str) -> Option<&str> {
    // Pattern: PREFIX-NAMESPACE-NNN
    let rest = spec_id
        .strip_prefix("INV-")
        .or_else(|| spec_id.strip_prefix("ADR-"))
        .or_else(|| spec_id.strip_prefix("NEG-"))?;
    // Namespace is everything up to the last hyphen-digits group
    rest.rfind('-').map(|pos| &rest[..pos])
}

/// Count total spec elements in the store (entities with :spec/element-type).
fn count_spec_elements(store: &Store) -> usize {
    let spec_type_attr = Attribute::from_keyword(":spec/element-type");
    store
        .entities()
        .iter()
        .filter(|e| {
            store
                .entity_datoms(**e)
                .iter()
                .any(|d| d.attribute == spec_type_attr && d.op == Op::Assert)
        })
        .count()
}

/// Filter matches by namespace (case-insensitive comparison).
fn filter_by_namespace<'a>(matches: &'a [PatternMatch], namespace: &str) -> Vec<&'a PatternMatch> {
    let ns_upper = namespace.to_uppercase();
    matches
        .iter()
        .filter(|m| {
            extract_namespace(&m.spec_id)
                .map(|ns| ns.to_uppercase() == ns_upper)
                .unwrap_or(false)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Status mode (braid verify)
// ---------------------------------------------------------------------------

fn run_status(store: &Store) -> CommandOutput {
    let total = count_spec_elements(store);
    let matches = detect_patterns(store);
    let summary = summarize_patterns(&matches, total);

    let coverage_pct = if total == 0 {
        0.0
    } else {
        (summary.matched_elements as f64 / total as f64) * 100.0
    };

    // Human output
    let mut human = String::new();
    human.push_str(&format!(
        "Verification Status\n\
         ===================\n\
         Spec elements:  {}\n\
         Matched:        {} ({:.1}%)\n\
         Unmatched:      {}\n\
         Mean confidence: {:.2}\n\n\
         Pattern Breakdown:\n",
        total,
        summary.matched_elements,
        coverage_pct,
        summary.unmatched_elements,
        summary.mean_confidence,
    ));

    for (pattern, count) in &summary.pattern_counts {
        if *count > 0 {
            human.push_str(&format!("  {:25} {}\n", pattern.name(), count));
        }
    }

    if summary.unmatched_elements > 0 {
        human.push_str(&format!(
            "\nHint: {} spec elements have no detected pattern. \
             Use `braid verify --generate --dry-run` to see matched elements.",
            summary.unmatched_elements,
        ));
    }

    // JSON output
    let pattern_json: serde_json::Value = summary
        .pattern_counts
        .iter()
        .map(|(p, c)| (p.name().to_string(), serde_json::json!(c)))
        .collect::<serde_json::Map<String, serde_json::Value>>()
        .into();

    let json = serde_json::json!({
        "total_spec_elements": total,
        "matched_elements": summary.matched_elements,
        "unmatched_elements": summary.unmatched_elements,
        "coverage_pct": (coverage_pct * 100.0).round() / 100.0,
        "mean_confidence": (summary.mean_confidence * 1000.0).round() / 1000.0,
        "pattern_counts": pattern_json,
    });

    // Agent output
    let agent = AgentOutput {
        context: format!(
            "verify: {} spec elements, {:.1}% pattern coverage",
            total, coverage_pct,
        ),
        content: format!(
            "{} of {} spec elements matched ({:.1}%). Mean confidence: {:.2}. \
             Top patterns: {}.",
            summary.matched_elements,
            total,
            coverage_pct,
            summary.mean_confidence,
            summary
                .pattern_counts
                .iter()
                .filter(|(_, c)| *c > 0)
                .map(|(p, c)| format!("{}({})", p.name(), c))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        footer: if summary.unmatched_elements > 0 {
            format!(
                "Next: `braid verify --generate` to emit coherence tests for {} matched elements",
                summary.matched_elements,
            )
        } else {
            "All spec elements have pattern matches. Run `braid verify --generate` to emit tests."
                .to_string()
        },
    };

    CommandOutput { json, agent, human }
}

// ---------------------------------------------------------------------------
// Generate mode (braid verify --generate)
// ---------------------------------------------------------------------------

fn run_generate(store: &Store, dry_run: bool, namespace: Option<&str>) -> CommandOutput {
    let total = count_spec_elements(store);
    let all_matches = detect_patterns(store);

    // Apply namespace filter if provided
    let matches: Vec<&PatternMatch> = if let Some(ns) = namespace {
        filter_by_namespace(&all_matches, ns)
    } else {
        all_matches.iter().collect()
    };

    if dry_run {
        return run_dry_run(&matches, total, namespace);
    }

    // Extract test properties for each match
    let properties: Vec<_> = matches.iter().map(|m| extract_test_property(m)).collect();

    if properties.is_empty() {
        let msg = if let Some(ns) = namespace {
            format!(
                "No pattern matches found for namespace '{}'. Try without --namespace.",
                ns
            )
        } else {
            "No pattern matches found. Spec elements may lack :spec/statement or :spec/falsification text.".to_string()
        };
        let json = serde_json::json!({
            "match_count": 0,
            "property_count": 0,
            "namespace_filter": namespace,
        });
        let agent = AgentOutput {
            context: "verify --generate: 0 matches".to_string(),
            content: msg.clone(),
            footer: "check: braid verify (status) | add: braid spec create".to_string(),
        };
        return CommandOutput {
            json,
            agent,
            human: msg,
        };
    }

    let code = emit_test_module(&properties);

    // Pattern breakdown for summary
    let mut pattern_counts = std::collections::HashMap::new();
    for m in &matches {
        *pattern_counts.entry(m.pattern.name()).or_insert(0usize) += 1;
    }

    // Human output: the generated code
    let human = code.clone();

    // JSON output
    let json = serde_json::json!({
        "match_count": matches.len(),
        "property_count": properties.len(),
        "namespace_filter": namespace,
        "pattern_breakdown": pattern_counts,
        "code": code,
    });

    // Agent output: summary with code preview
    let preview_lines: Vec<&str> = code.lines().take(10).collect();
    let agent = AgentOutput {
        context: format!(
            "verify --generate: {} matches -> {} test properties{}",
            matches.len(),
            properties.len(),
            namespace
                .map(|ns| format!(" (namespace: {})", ns))
                .unwrap_or_default(),
        ),
        content: format!(
            "Generated {} proptest properties from {} pattern matches. \
             Patterns: {}. Preview:\n{}{}",
            properties.len(),
            matches.len(),
            pattern_counts
                .iter()
                .map(|(p, c)| format!("{}({})", p, c))
                .collect::<Vec<_>>()
                .join(", "),
            preview_lines.join("\n"),
            if code.lines().count() > 10 {
                "\n..."
            } else {
                ""
            },
        ),
        footer: "Pipe to file: `braid verify --generate > tests/generated_coherence.rs`"
            .to_string(),
    };

    CommandOutput { json, agent, human }
}

// ---------------------------------------------------------------------------
// Dry-run mode (braid verify --generate --dry-run)
// ---------------------------------------------------------------------------

fn run_dry_run(matches: &[&PatternMatch], total: usize, namespace: Option<&str>) -> CommandOutput {
    let mut human = String::new();
    human.push_str(&format!(
        "Dry Run: {} matches from {} spec elements{}\n\n",
        matches.len(),
        total,
        namespace
            .map(|ns| format!(" (namespace: {})", ns))
            .unwrap_or_default(),
    ));
    human.push_str(&format!(
        "{:<30} {:<25} {}\n",
        "SPEC_ID", "PATTERN", "CONFIDENCE"
    ));
    human.push_str(&format!("{}\n", "-".repeat(70)));
    for m in matches {
        human.push_str(&format!(
            "{:<30} {:<25} {:.2}\n",
            m.spec_id,
            m.pattern.name(),
            m.confidence,
        ));
    }

    let entries: Vec<serde_json::Value> = matches
        .iter()
        .map(|m| {
            serde_json::json!({
                "spec_id": m.spec_id,
                "pattern": m.pattern.name(),
                "confidence": (m.confidence * 1000.0).round() / 1000.0,
                "subject": m.subject,
                "property": m.property,
            })
        })
        .collect();

    let json = serde_json::json!({
        "dry_run": true,
        "match_count": matches.len(),
        "total_spec_elements": total,
        "namespace_filter": namespace,
        "matches": entries,
    });

    let agent = AgentOutput {
        context: format!(
            "verify --generate --dry-run: {} matches{}",
            matches.len(),
            namespace
                .map(|ns| format!(" (namespace: {})", ns))
                .unwrap_or_default(),
        ),
        content: format!(
            "{} spec elements would generate tests. Top patterns: {}.",
            matches.len(),
            {
                let mut counts = std::collections::HashMap::new();
                for m in matches {
                    *counts.entry(m.pattern.name()).or_insert(0usize) += 1;
                }
                counts
                    .iter()
                    .map(|(p, c)| format!("{}({})", p, c))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        ),
        footer: "Remove --dry-run to generate the test module.".to_string(),
    };

    CommandOutput { json, agent, human }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the `braid verify` command.
///
/// - `generate=false`: Show verification status (coverage, pattern breakdown).
/// - `generate=true, dry_run=true`: Show matches without generating code.
/// - `generate=true, dry_run=false`: Emit a Rust test module to stdout.
pub fn run(
    path: &Path,
    generate: bool,
    dry_run: bool,
    namespace: Option<&str>,
) -> Result<CommandOutput, BraidError> {
    let layout = DiskLayout::open(path)?;
    let store = layout.load_store()?;

    let output = if generate {
        run_generate(&store, dry_run, namespace)
    } else {
        run_status(&store)
    };

    Ok(output)
}
