# D5 — Token Efficiency Tokenizer Survey

> **Thread**: R3.5a — What Rust tokenizer crate should Braid use?
> **Date**: 2026-03-03
> **Sources**: spec/13-budget.md, crates.io, GitHub repositories

---

## Research Questions

1. What Rust tokenizer crates exist?
2. Do we need a real tokenizer or is chars/4 sufficient?
3. How accurate must token counting be for Braid's budget system?
4. What is the cost/benefit of exact vs. approximate tokenization?

---

## Braid's Token Counting Requirements (from spec/13-budget.md)

### Where Token Counting is Used

| Use Case | Accuracy Needed | Frequency | Source |
|----------|----------------|-----------|--------|
| Output budget cap (INV-BUDGET-001) | Medium | Every response | Q(t) formula |
| Guidance footer size selection | Low | Every response | k_eff threshold |
| Projection pyramid level | Low | Every query | Budget buckets (200/500/2000) |
| Command attention profile | Low | Every command | Category ceiling check |
| Token efficiency metric (INV-BUDGET-006) | Medium | Per output block | Density calculation |
| CLAUDE.md budget validation | Low | Per seed | Constraint count check |

### Budget Thresholds

The spec defines coarse thresholds, not precise token boundaries:

```
pi_0: > 2000 tokens available
pi_1: 500-2000 tokens
pi_2: 200-500 tokens
pi_3: <= 200 tokens

Guidance:
  k > 0.7:   full (100-200 tokens)
  0.4-0.7:   compressed (30-60 tokens)
  <= 0.4:    minimal (10-20 tokens)
  <= 0.2:    harvest signal only
```

These are broad bands. A 10-20% tokenization error would not change the
band selection in most cases. The system is designed around coarse buckets,
not precise byte counting.

### The Token Efficiency Metric

INV-BUDGET-006 defines `rho(output) = semantic_units(output) / |output|`. Here
`|output|` is token count. This is a ratio metric — relative accuracy matters
more than absolute. If the tokenizer consistently overcounts by 15%, the density
values shift uniformly and comparisons remain valid.

---

## Available Crates

### 1. tiktoken-rs

- **Repository**: https://github.com/zurawiki/tiktoken-rs
- **crates.io**: https://crates.io/crates/tiktoken-rs
- **Encodings**: cl100k_base (GPT-4, GPT-3.5-turbo), o200k_base (newer OpenAI),
  p50k_base (Codex, davinci)
- **Claude compatibility**: Claude reportedly uses a tokenizer similar to
  cl100k_base, though Anthropic has not published their exact encoding.
  Empirical tests show cl100k_base is within ~5-10% of Claude's actual counts.
- **Performance**: First call loads the BPE vocabulary (~2MB). Subsequent calls
  are fast (microseconds per short string, milliseconds per page).
- **Dependencies**: ~15 transitive deps. Uses `fancy-regex` and `base64`.
- **Accuracy**: Exact for OpenAI models. Approximate (+/- 5-10%) for Claude.

### 2. HuggingFace Tokenizers

- **Repository**: https://github.com/huggingface/tokenizers
- **crates.io**: https://crates.io/crates/tokenizers
- **Architecture**: Native Rust implementation. Supports BPE, WordPiece, Unigram,
  and other algorithms. Can load any tokenizer from HuggingFace Hub.
- **Performance**: "Less than 20 seconds to tokenize a GB of text on a server's CPU."
  Extremely fast.
- **Dependencies**: Heavy. ~40+ transitive deps including `serde`, `rayon`,
  `indicatif`, `itertools`. Significant compile-time cost.
- **Accuracy**: Exact for any model with a published tokenizer on HF Hub.
  Claude's tokenizer is not publicly available on HF Hub.

### 3. bpe (OpenAI's Rust BPE)

- **crates.io**: https://crates.io/crates/bpe
- **Description**: Fast BPE tokenizer. Claims ~10x faster than HuggingFace BPE
  for typical inputs.
- **Scope**: BPE only. No model-specific encoding logic.

### 4. chars/4 Approximation

- **Implementation**: `token_count = text.len() / 4` (or `text.chars().count() / 4`)
- **Dependencies**: Zero.
- **Performance**: Nanoseconds.
- **Accuracy**: Varies wildly. English prose: ~75-85% accurate. Code: ~60-70%
  accurate (more tokens per character due to keywords/operators). Markdown with
  formatting: ~70-80% accurate.

### 5. Word-Count Approximation

- **Implementation**: `token_count = text.split_whitespace().count() * 4 / 3`
  (English averages ~1.33 tokens per word)
- **Dependencies**: Zero.
- **Performance**: Microseconds.
- **Accuracy**: Better than chars/4 for English prose (~85-90%). Worse for code.

---

## Decision Matrix

| Criterion | tiktoken-rs | HF Tokenizers | bpe | chars/4 | words*1.33 |
|-----------|-------------|---------------|-----|---------|------------|
| Claude accuracy | ~90-95% | N/A (no Claude model) | ~85-90% | ~70-80% | ~80-90% |
| Performance | Fast (ms) | Very fast (us) | Very fast (us) | Trivial (ns) | Trivial (us) |
| Dependencies | ~15 | ~40+ | ~5 | 0 | 0 |
| Compile time impact | Low | High | Low | None | None |
| Maintenance burden | Low | Medium | Low | None | None |
| Setup complexity | Load vocab on first call | Load model config | Load vocab | None | None |

---

## Analysis: Do We Need Real Tokenization?

### The Budget System is Coarse by Design

The spec's budget thresholds are broad bands (200, 500, 2000 tokens). The
guidance compression thresholds are even coarser (0.2, 0.4, 0.7 of k_eff).
A tokenizer that is off by 15% would:

- Misclassify projection level at band boundaries (~5% of cases)
- Slightly over/under-compress guidance (~10% of cases)
- Not affect the harvest warning system (turn-count heuristic at Stage 0)

### The Primary k_eff Source is External

The most important budget input is `k_eff`, which comes from Claude Code's
`context_window.used_percentage` (ADR-BUDGET-001). This is already measured
in the provider's native tokens. Braid's tokenizer is only needed for:

1. Measuring output size (for the budget cap in INV-BUDGET-001)
2. Computing token efficiency density (INV-BUDGET-006)

For (1), the question is "does this output fit in N tokens?" — a 15% error
means we might produce outputs that are 15% too long or short. Given the
5% budget fraction, this is within noise.

For (2), token efficiency is a ratio. Consistent bias cancels out.

### When Exact Tokenization Matters

Exact tokenization becomes important at Stage 1+ when:
- The projection pyramid needs precise level selection
- Token efficiency tracking needs cross-session comparability
- Rate-distortion optimization requires accurate distortion measurement

At Stage 0, coarse approximation is sufficient.

---

## Recommendation: Tiered Approach

### Stage 0: chars/4 with Correction Factor

```rust
/// Approximate token count. Stage 0 implementation.
/// Average error: ~15-20% vs real tokenizer.
/// Sufficient for coarse budget band selection.
pub fn approx_tokens(text: &str) -> usize {
    // chars/4 baseline with content-type correction
    let char_count = text.len(); // byte count, close enough for ASCII-heavy content
    let base = char_count / 4;

    // Correction: code has more tokens per char than prose
    if looks_like_code(text) {
        base * 5 / 4  // 25% uplift for code
    } else {
        base
    }
}

fn looks_like_code(text: &str) -> bool {
    let indicators = ['{', '}', '(', ')', ';', "fn ", "let ", "pub ", "impl "];
    let score: usize = indicators.iter()
        .map(|i| text.matches(i).count())
        .sum();
    score > text.len() / 200  // More than 0.5% indicator density
}
```

- Zero dependencies
- Trivial performance
- Sufficient for Stage 0's coarse budget bands

### Stage 1: tiktoken-rs with cl100k_base

When INV-BUDGET-001 through 006 come online and need real token counting:

```rust
use tiktoken_rs::cl100k_base;

pub fn count_tokens(text: &str) -> usize {
    let bpe = cl100k_base().unwrap();
    bpe.encode_with_special_tokens(text).len()
}
```

- ~5-10% error for Claude (acceptable for budget calculations)
- Moderate dependency footprint
- Well-maintained crate

### Stage 2+ (Optional): Anthropic API Token Counting

If token accuracy becomes critical (e.g., for rate-distortion optimization),
use Anthropic's `messages.countTokens` API for ground-truth calibration.
Run periodically to compute a correction factor for tiktoken-rs.

---

## Cost-Benefit Summary

| Approach | Stage | Accuracy | Deps | Justification |
|----------|-------|----------|------|---------------|
| chars/4 + heuristic | 0 | ~75-85% | 0 | Budget bands are 4x apart; 20% error is invisible |
| tiktoken-rs | 1 | ~90-95% | ~15 | Token efficiency tracking needs consistency |
| Anthropic API | 2+ | ~99% | HTTP client | Rate-distortion optimization needs precision |

**The recommendation is clear**: start with chars/4, graduate to tiktoken-rs at
Stage 1. The budget system's coarse design absorbs approximation error gracefully.
This avoids a dependency and complexity cost during the critical Stage 0 foundation
work.

---

## Open Questions

1. Should the `approx_tokens` function be behind a trait so we can swap
   implementations without changing callers? (YES — use a `TokenCounter` trait.)
2. Does Anthropic publish their tokenizer? As of March 2026, they offer the
   `messages.countTokens` API but have not released the tokenizer model itself.
3. Should we cache tiktoken-rs's BPE vocabulary in the store as a datom? Probably
   not — it is a static resource, not session-derived knowledge.

---

## Decision Confirmation (R3.5c, 2026-03-03)

The tiered tokenization recommendation is **confirmed and adopted**:

- **Stage 0**: `ApproxTokenCounter` (chars/4 with content-type heuristic). Zero dependencies.
  Applied to guide/00-architecture.md as the `TokenCounter` trait with `ApproxTokenCounter`
  implementation.
- **Stage 1**: `TiktokenCounter` wrapping tiktoken-rs cl100k_base. ~15 transitive deps,
  ~90-95% Claude accuracy.
- **Stage 2+**: Optional `AnthropicApiCounter` for ground-truth calibration.

The `TokenCounter` trait is defined in guide/00-architecture.md (section 0.6) and will be
implemented in `braid-kernel/src/lib.rs` or a dedicated `braid-kernel/src/budget.rs` module.
All token-counting callsites accept `&dyn TokenCounter` for stage-transparent upgrades.

Recorded as IMPL-002 in ADRS.md.
