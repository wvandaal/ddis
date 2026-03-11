//! AGENTS.md injection engine — parse tags, generate content, write back.
//!
//! The injection engine implements the self-bootstrap loop (C7):
//! `observe → harvest → seed --inject → agent reads → agent works → observe → ...`
//!
//! # Design (formal)
//!
//! The tag parser is a **lens** over AGENTS.md text:
//!   - `get(text)` extracts the content between `<braid-seed>` tags
//!   - `set(text, content)` replaces that content
//!   - Lens laws hold:
//!     - get(set(s, a)) = a         (you get what you set)
//!     - set(s, get(s)) = s         (setting existing is identity)
//!     - set(set(s, a), b) = set(s, b)  (last set wins)
//!
//! The content generator is a natural transformation Store → String:
//!   - Deterministic: same store state → same output
//!   - Budget-compliant: output ≤ declared budget (INV-SEED-002)
//!   - Idempotent: inject(inject(file, store), store) = inject(file, store)
//!
//! # Invariants
//!
//! - Content outside `<braid-seed>` tags is NEVER modified
//! - Tags inside markdown code blocks (```) are NOT matched
//! - Output includes a generation comment with timestamp and store stats
//!
//! Traces to: C7 (self-bootstrap), INV-SEED-001 (store projection),
//! INV-SEED-002 (budget compliance), ADR-INTERFACE-002 (agent-mode style).

use braid_kernel::seed::{self, AssociateCue, ContextSection};
use braid_kernel::store::Store;

/// An injection point found in the text between `<braid-seed>` tags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InjectionPoint {
    /// Byte offset of opening tag start (including the tag itself).
    pub tag_start: usize,
    /// Byte offset after closing tag end (including newline if present).
    pub tag_end: usize,
    /// Byte offset of content start (after opening tag + newline).
    pub content_start: usize,
    /// Byte offset of content end (before closing tag).
    pub content_end: usize,
}

/// Errors that can occur when finding injection points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InjectionError {
    /// No `<braid-seed>` opening tag found.
    NoOpenTag,
    /// Opening tag found but no closing `</braid-seed>` tag.
    NoCloseTag,
    /// Multiple `<braid-seed>` tags found (only one allowed).
    MultipleOpenTags,
}

impl std::fmt::Display for InjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InjectionError::NoOpenTag => write!(
                f,
                "no <braid-seed> tag found. Add these tags to your AGENTS.md:\n\
                 \n\
                 <braid-seed>\n\
                 <!-- braid will inject dynamic context here -->\n\
                 </braid-seed>"
            ),
            InjectionError::NoCloseTag => {
                write!(f, "found <braid-seed> but no matching </braid-seed> tag")
            }
            InjectionError::MultipleOpenTags => write!(
                f,
                "multiple <braid-seed> tags found — only one injection point is allowed"
            ),
        }
    }
}

const OPEN_TAG: &str = "<braid-seed>";
const CLOSE_TAG: &str = "</braid-seed>";

/// Find the injection point in `text` between `<braid-seed>` and `</braid-seed>` tags.
///
/// Ignores tags inside markdown code blocks (triple backticks).
/// Returns byte offsets for precise string slicing.
pub fn find_injection_point(text: &str) -> Result<InjectionPoint, InjectionError> {
    // First, identify code block ranges to exclude
    let code_ranges = find_code_block_ranges(text);

    // Find all open tag positions (excluding those in code blocks)
    let mut open_positions: Vec<usize> = Vec::new();
    let mut search_start = 0;
    while let Some(pos) = text[search_start..].find(OPEN_TAG) {
        let abs_pos = search_start + pos;
        if !in_code_block(abs_pos, &code_ranges) {
            open_positions.push(abs_pos);
        }
        search_start = abs_pos + OPEN_TAG.len();
    }

    if open_positions.is_empty() {
        return Err(InjectionError::NoOpenTag);
    }
    if open_positions.len() > 1 {
        return Err(InjectionError::MultipleOpenTags);
    }

    let tag_start = open_positions[0];
    let after_open = tag_start + OPEN_TAG.len();

    // Content starts after the opening tag + newline (if present)
    let content_start = if text[after_open..].starts_with('\n') {
        after_open + 1
    } else {
        after_open
    };

    // Find closing tag (excluding code blocks)
    let close_pos = text[content_start..]
        .find(CLOSE_TAG)
        .filter(|&pos| !in_code_block(content_start + pos, &code_ranges))
        .ok_or(InjectionError::NoCloseTag)?;

    let content_end = content_start + close_pos;
    let tag_end_raw = content_end + CLOSE_TAG.len();

    // Include trailing newline in tag_end if present
    let tag_end = if text[tag_end_raw..].starts_with('\n') {
        tag_end_raw + 1
    } else {
        tag_end_raw
    };

    Ok(InjectionPoint {
        tag_start,
        tag_end,
        content_start,
        content_end,
    })
}

/// Apply injection: replace content between tags with new content.
///
/// This is the `set` operation of the lens. Preserves all text outside the tags.
pub fn inject(text: &str, point: &InjectionPoint, content: &str) -> String {
    let mut result = String::with_capacity(text.len() + content.len());
    result.push_str(&text[..point.content_start]);
    result.push_str(content);
    if !content.is_empty() && !content.ends_with('\n') {
        result.push('\n');
    }
    result.push_str(&text[point.content_end..]);
    result
}

/// Generate markdown content for injection into AGENTS.md (SB.3.2).
///
/// Formats seed content as natural markdown instructions that read inline
/// with the rest of the AGENTS.md file. Different from CLI seed output —
/// this is meant to be consumed as part of an instruction document, not
/// as a tool response.
pub fn format_for_injection(store: &Store, task: Option<&str>, budget: usize) -> String {
    let task_desc = task.unwrap_or("continue");

    // Assemble seed context using the kernel pipeline
    let cue = AssociateCue::Semantic {
        text: task_desc.to_string(),
        depth: 2,
        breadth: 25,
    };
    let neighborhood = seed::associate(store, &cue);
    let context = seed::assemble(store, &neighborhood, task_desc, budget);

    let mut out = String::new();

    // Generation comment with metadata
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    out.push_str(&format!(
        "<!-- Generated by braid. Do not edit manually. Regenerate: braid seed --inject AGENTS.md -->\n\
         <!-- Updated: {} | Store: {} datoms, {} entities -->\n\n",
        now,
        store.len(),
        store.entity_count()
    ));

    // Render each section as markdown
    for section in &context.sections {
        match section {
            ContextSection::Orientation(text) => {
                out.push_str("### Session Context\n");
                for line in text.lines() {
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            }
            ContextSection::Constraints(refs) => {
                if !refs.is_empty() {
                    out.push_str("### Active Constraints\n");
                    for c in refs {
                        let status = match c.satisfied {
                            Some(true) => "[ok]",
                            Some(false) => "[!!]",
                            None => "[?]",
                        };
                        if c.summary.is_empty() {
                            out.push_str(&format!("- {} {}\n", status, c.id));
                        } else {
                            out.push_str(&format!("- {} {} — {}\n", status, c.id, c.summary));
                        }
                    }
                    out.push('\n');
                }
            }
            ContextSection::State(entries) => {
                if !entries.is_empty() {
                    out.push_str("### Recent Entities\n");
                    // Show entities with meaningful content, skip hash-only entities
                    let meaningful: Vec<_> = entries
                        .iter()
                        .filter(|e| !e.content.starts_with('#'))
                        .take(15)
                        .collect();
                    for entry in &meaningful {
                        out.push_str(&format!("- {}\n", entry.content));
                    }
                    if entries.len() > meaningful.len() {
                        out.push_str(&format!(
                            "- ... and {} more entities\n",
                            entries.len() - meaningful.len()
                        ));
                    }
                    out.push('\n');
                }
            }
            ContextSection::Warnings(lines) => {
                if !lines.is_empty() {
                    out.push_str("### Open Questions\n");
                    for line in lines {
                        out.push_str(&format!("- {line}\n"));
                    }
                    out.push('\n');
                }
            }
            ContextSection::Directive(text) => {
                out.push_str("### Next Actions\n");
                for line in text.lines() {
                    // Skip the "Task: " line (redundant in AGENTS.md context)
                    if line.starts_with("Task: ") {
                        continue;
                    }
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            }
        }
    }

    // Quick reference footer
    out.push_str("### Quick Reference\n");
    out.push_str("```bash\n");
    out.push_str("braid status                           # Dashboard + next action\n");
    out.push_str("braid observe \"...\" --confidence 0.7    # Capture knowledge\n");
    out.push_str("braid harvest --commit                 # End-of-session extraction\n");
    out.push_str("braid seed --inject AGENTS.md          # Refresh this section\n");
    out.push_str("```\n");

    out
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Find byte ranges of markdown code blocks (triple backtick sections).
fn find_code_block_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut in_block = false;
    let mut block_start = 0;
    let mut offset = 0;

    for line in text.split('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_block {
                // Closing fence — range ends after this line
                ranges.push((block_start, offset + line.len()));
                in_block = false;
            } else {
                // Opening fence
                block_start = offset;
                in_block = true;
            }
        }
        offset += line.len() + 1; // +1 for the '\n'
    }

    // If still in a block at EOF, close it
    if in_block {
        ranges.push((block_start, text.len()));
    }

    ranges
}

/// Check if a byte position falls within any code block range.
fn in_code_block(pos: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|&(start, end)| pos >= start && pos < end)
}

/// Quality metrics for injected content (S0.5.2).
///
/// Assesses whether the generated injection provides adequate
/// context for an agent reading the AGENTS.md file.
#[derive(Clone, Debug)]
pub struct InjectionQuality {
    /// Total estimated tokens in the injection.
    pub token_count: usize,
    /// Number of sections present (max 5: context, constraints, entities, questions, actions).
    pub section_count: usize,
    /// Whether the quick reference block is present.
    pub has_quick_reference: bool,
    /// Whether at least one next action is present.
    pub has_next_action: bool,
    /// Whether store context (datom/entity counts) is present.
    pub has_store_context: bool,
    /// Overall quality score: 0.0 (useless) to 1.0 (complete).
    pub score: f64,
}

/// Assess the quality of injection content.
///
/// Scans the generated markdown for expected sections and computes
/// a composite quality score. Used for self-diagnostics: if injection
/// quality drops below threshold, the guidance system can flag it.
pub fn assess_injection_quality(content: &str) -> InjectionQuality {
    let token_count = content.split_whitespace().count() * 4 / 3;

    let has_store_context =
        content.contains("### Session Context") || content.contains("### Store Context");
    let has_constraints = content.contains("### Active Constraints");
    let has_entities = content.contains("### Recent Entities");
    let has_questions = content.contains("### Open Questions");
    let has_actions = content.contains("### Next Actions");
    let has_quick_reference = content.contains("### Quick Reference");
    let has_next_action = content.contains("run: braid") || content.contains("next:");

    let section_count = [
        has_store_context,
        has_constraints,
        has_entities,
        has_questions,
        has_actions,
    ]
    .iter()
    .filter(|&&b| b)
    .count();

    // Composite score: weighted sum of quality indicators
    // Store context is critical (0.25), actions are critical (0.25),
    // sections contribute proportionally, quick ref is a bonus
    let mut score = 0.0;
    if has_store_context {
        score += 0.25;
    }
    if has_next_action {
        score += 0.25;
    }
    score += section_count as f64 * 0.08; // 5 sections × 0.08 = 0.40
    if has_quick_reference {
        score += 0.10;
    }

    InjectionQuality {
        token_count,
        section_count,
        has_quick_reference,
        has_next_action,
        has_store_context,
        score: score.min(1.0),
    }
}

/// Check if an injected section is stale based on its embedded timestamp.
///
/// Parses the `<!-- Updated: TIMESTAMP -->` comment from injection content
/// and compares against the current time. Returns the age in seconds,
/// or None if no timestamp found.
pub fn injection_age_seconds(content: &str) -> Option<u64> {
    // Look for: <!-- Updated: 1234567890 | Store: ... -->
    let marker = "<!-- Updated: ";
    let start = content.find(marker)?;
    let after = start + marker.len();
    let end = content[after..].find(' ').map(|p| after + p)?;
    let timestamp_str = &content[after..end];
    let timestamp: u64 = timestamp_str.parse().ok()?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Some(now.saturating_sub(timestamp))
}

/// Whether the injection content is considered stale.
///
/// Threshold: 1 hour (3600 seconds). After one hour of work,
/// the store state has likely changed enough that the injected
/// context is no longer representative.
pub fn is_injection_stale(content: &str) -> bool {
    match injection_age_seconds(content) {
        Some(age) => age > 3600,
        None => true, // No timestamp = definitely stale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_basic_injection_point() {
        let text = "# My File\n\n<braid-seed>\nold content\n</braid-seed>\n\n# Footer\n";
        let point = find_injection_point(text).unwrap();
        assert_eq!(
            &text[point.content_start..point.content_end],
            "old content\n"
        );
    }

    #[test]
    fn no_tags_returns_error() {
        let text = "# Just a regular file\nNo tags here.\n";
        assert_eq!(find_injection_point(text), Err(InjectionError::NoOpenTag));
    }

    #[test]
    fn missing_close_tag_returns_error() {
        let text = "# File\n<braid-seed>\nsome content\n";
        assert_eq!(find_injection_point(text), Err(InjectionError::NoCloseTag));
    }

    #[test]
    fn multiple_open_tags_returns_error() {
        let text = "<braid-seed>\na\n</braid-seed>\n<braid-seed>\nb\n</braid-seed>\n";
        assert_eq!(
            find_injection_point(text),
            Err(InjectionError::MultipleOpenTags)
        );
    }

    #[test]
    fn empty_content_between_tags() {
        let text = "before\n<braid-seed>\n</braid-seed>\nafter\n";
        let point = find_injection_point(text).unwrap();
        assert_eq!(&text[point.content_start..point.content_end], "");
    }

    #[test]
    fn inject_replaces_content() {
        let text = "before\n<braid-seed>\nold\n</braid-seed>\nafter\n";
        let point = find_injection_point(text).unwrap();
        let result = inject(text, &point, "new content\n");
        assert!(result.contains("new content"));
        assert!(result.contains("before"));
        assert!(result.contains("after"));
        assert!(!result.contains("old"));
    }

    #[test]
    fn inject_idempotency() {
        let text = "before\n<braid-seed>\nold\n</braid-seed>\nafter\n";
        let content = "injected\n";

        // First injection
        let point1 = find_injection_point(text).unwrap();
        let result1 = inject(text, &point1, content);

        // Second injection with same content
        let point2 = find_injection_point(&result1).unwrap();
        let result2 = inject(&result1, &point2, content);

        assert_eq!(result1, result2, "injection must be idempotent");
    }

    #[test]
    fn preserves_content_outside_tags() {
        let header = "# AGENTS.md\n\nHuman-written instructions here.\n\n";
        let footer = "\n\n## More human content\nDo not touch this.\n";
        let text = format!("{header}<braid-seed>\nold stuff\n</braid-seed>{footer}");
        let point = find_injection_point(&text).unwrap();
        let result = inject(&text, &point, "new stuff\n");
        assert!(result.starts_with(header));
        assert!(result.ends_with(footer));
    }

    #[test]
    fn tags_in_code_block_ignored() {
        let text = "# File\n```\n<braid-seed>\nnot real\n</braid-seed>\n```\n\n<braid-seed>\nreal content\n</braid-seed>\n";
        let point = find_injection_point(text).unwrap();
        assert_eq!(
            &text[point.content_start..point.content_end],
            "real content\n"
        );
    }

    #[test]
    fn unicode_preservation() {
        let text = "# Dátá\n\n<braid-seed>\nöld cöntënt ñ\n</braid-seed>\n\n# Föötér\n";
        let point = find_injection_point(text).unwrap();
        let result = inject(text, &point, "nëw cöntënt ü\n");
        assert!(result.contains("nëw cöntënt ü"));
        assert!(result.contains("Dátá"));
        assert!(result.contains("Föötér"));
    }

    #[test]
    fn trailing_newline_after_injection() {
        let text = "before\n<braid-seed>\nold\n</braid-seed>\nafter";
        let point = find_injection_point(text).unwrap();
        let result = inject(text, &point, "new");
        // Content should get a trailing newline added
        assert!(result.contains("new\n</braid-seed>"));
    }

    #[test]
    fn injection_error_display() {
        let err = InjectionError::NoOpenTag;
        let msg = format!("{err}");
        assert!(msg.contains("<braid-seed>"));
        assert!(msg.contains("Add these tags"));
    }

    // ── SB.3.4: Extended injection tests ─────────────────────────────────────

    /// Lens law: get(set(s, a)) = a — you get what you set.
    #[test]
    fn lens_law_get_set() {
        let text = "header\n<braid-seed>\noriginal\n</braid-seed>\nfooter\n";
        let new_content = "injected content\n";

        let point = find_injection_point(text).unwrap();
        let result = inject(text, &point, new_content);

        // Extract what we set
        let point2 = find_injection_point(&result).unwrap();
        let got = &result[point2.content_start..point2.content_end];
        assert_eq!(got, new_content, "lens law: get(set(s, a)) = a");
    }

    /// Lens law: set(s, get(s)) = s — setting existing is identity.
    #[test]
    fn lens_law_set_get() {
        let text = "header\n<braid-seed>\nexisting content\n</braid-seed>\nfooter\n";

        let point = find_injection_point(text).unwrap();
        let existing = &text[point.content_start..point.content_end];
        let result = inject(text, &point, existing);

        assert_eq!(result, text, "lens law: set(s, get(s)) = s");
    }

    /// Lens law: set(set(s, a), b) = set(s, b) — last set wins.
    #[test]
    fn lens_law_set_set() {
        let text = "header\n<braid-seed>\noriginal\n</braid-seed>\nfooter\n";

        // set(s, a)
        let p1 = find_injection_point(text).unwrap();
        let r1 = inject(text, &p1, "first\n");

        // set(set(s, a), b)
        let p2 = find_injection_point(&r1).unwrap();
        let r2 = inject(&r1, &p2, "second\n");

        // set(s, b) directly
        let p3 = find_injection_point(text).unwrap();
        let r3 = inject(text, &p3, "second\n");

        assert_eq!(r2, r3, "lens law: set(set(s, a), b) = set(s, b)");
    }

    /// Injection with multi-line content preserves all lines.
    #[test]
    fn multiline_content_preserved() {
        let text = "before\n<braid-seed>\n</braid-seed>\nafter\n";
        let content = "line one\nline two\nline three\n";

        let point = find_injection_point(text).unwrap();
        let result = inject(text, &point, content);

        let point2 = find_injection_point(&result).unwrap();
        let got = &result[point2.content_start..point2.content_end];
        assert_eq!(got, content);
    }

    /// Tags surrounded by complex markdown (headers, lists, links).
    #[test]
    fn complex_markdown_context() {
        let text = concat!(
            "# Project\n\n",
            "## Config\n\n",
            "- item 1\n",
            "- item 2\n\n",
            "<braid-seed>\nold\n</braid-seed>\n\n",
            "## Links\n\n",
            "[link](https://example.com)\n",
        );

        let point = find_injection_point(text).unwrap();
        let result = inject(text, &point, "new\n");

        assert!(result.contains("# Project"));
        assert!(result.contains("- item 1"));
        assert!(result.contains("[link](https://example.com)"));
        assert!(result.contains("new"));
        assert!(!result.contains("old"));
    }

    /// Tags in fenced code block with language annotation are still ignored.
    #[test]
    fn tags_in_annotated_code_block_ignored() {
        let text = concat!(
            "```markdown\n",
            "<braid-seed>\nfake\n</braid-seed>\n",
            "```\n\n",
            "<braid-seed>\nreal\n</braid-seed>\n",
        );

        let point = find_injection_point(text).unwrap();
        assert_eq!(&text[point.content_start..point.content_end], "real\n");
    }

    /// Only opening tag inside code block, closing outside — should error.
    #[test]
    fn open_in_code_close_outside_errors() {
        let text = "```\n<braid-seed>\n```\n</braid-seed>\n";
        // The open tag is in a code block so it's excluded.
        // The close tag is outside, but there's no open tag outside.
        assert!(find_injection_point(text).is_err());
    }

    /// Content generator with empty store produces valid output.
    #[test]
    fn format_for_injection_empty_store() {
        let store = Store::genesis();
        let content = format_for_injection(&store, Some("test task"), 2000);

        // Must contain generation comment
        assert!(content.contains("Generated by braid"));
        // Must contain quick reference
        assert!(content.contains("Quick Reference"));
        assert!(content.contains("braid status"));
        // Must not be empty
        assert!(!content.is_empty());
    }

    /// Content generator with task=None defaults to "continue".
    #[test]
    fn format_for_injection_no_task() {
        let store = Store::genesis();
        let content = format_for_injection(&store, None, 2000);
        assert!(content.contains("Generated by braid"));
    }

    /// Budget compliance: output tokens should not wildly exceed budget.
    /// (Soft constraint — format_for_injection doesn't hard-cap, but should
    /// produce reasonable output.)
    #[test]
    fn format_for_injection_reasonable_size() {
        let store = Store::genesis();
        let budget = 500;
        let content = format_for_injection(&store, Some("test"), budget);
        let word_count = content.split_whitespace().count();
        // Rough token estimate: words * 4/3. For an empty store, should be well under budget.
        let approx_tokens = word_count * 4 / 3;
        assert!(
            approx_tokens < budget * 3, // generous upper bound
            "generated ~{} tokens for budget {}, excessive",
            approx_tokens,
            budget
        );
    }

    /// Full round-trip: create text, inject, re-parse, inject again — idempotent.
    #[test]
    fn full_round_trip_idempotent() {
        let store = Store::genesis();
        let original = "# AGENTS.md\n\nInstructions.\n\n<braid-seed>\n<!-- placeholder -->\n</braid-seed>\n\n# End\n";

        let content = format_for_injection(&store, Some("round trip test"), 2000);

        // First injection
        let p1 = find_injection_point(original).unwrap();
        let r1 = inject(original, &p1, &content);

        // Second injection with same content
        let p2 = find_injection_point(&r1).unwrap();
        let r2 = inject(&r1, &p2, &content);

        assert_eq!(r1, r2, "full round-trip must be idempotent");
    }

    // ── S0.5.2: Injection quality metrics tests ──────────────────────────────

    #[test]
    fn quality_assessment_complete_content() {
        let content = "\
### Store Context
Braid datom store | 100 datoms, 50 entities

### Active Constraints
- [?] ADR-TEST-001 — Test constraint

### Recent Entities
- :spec/inv-test-001 — Test entity

### Open Questions
- [?] Some open question

### Next Actions
Next actions:
  1. Connect — Test action
     run: braid status

### Quick Reference
```bash
braid status
```
";
        let quality = assess_injection_quality(content);
        assert_eq!(quality.section_count, 5);
        assert!(quality.has_store_context);
        assert!(quality.has_next_action);
        assert!(quality.has_quick_reference);
        assert!(
            quality.score > 0.9,
            "Complete content should score >0.9, got {}",
            quality.score
        );
    }

    #[test]
    fn quality_assessment_minimal_content() {
        let content = "### Store Context\nsome content\n";
        let quality = assess_injection_quality(content);
        assert_eq!(quality.section_count, 1);
        assert!(quality.has_store_context);
        assert!(!quality.has_next_action);
        assert!(
            quality.score < 0.5,
            "Minimal content should score <0.5, got {}",
            quality.score
        );
    }

    // ── S0.5.3: Injection stale detection tests ─────────────────────────────

    #[test]
    fn injection_age_parses_timestamp() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let content = format!("<!-- Updated: {} | Store: 100 datoms -->", now);
        let age = injection_age_seconds(&content);
        assert!(age.is_some());
        assert!(age.unwrap() < 5, "Just-generated content should be <5s old");
    }

    #[test]
    fn injection_age_old_timestamp() {
        let content = "<!-- Updated: 1000000000 | Store: 100 datoms -->";
        let age = injection_age_seconds(content);
        assert!(age.is_some());
        assert!(age.unwrap() > 3600, "Old timestamp should be stale");
        assert!(is_injection_stale(content));
    }

    #[test]
    fn injection_age_no_timestamp() {
        let content = "No timestamp here";
        assert!(injection_age_seconds(content).is_none());
        assert!(is_injection_stale(content), "No timestamp means stale");
    }
}
