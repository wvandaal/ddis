# Jeffrey Emanuel's Rust Guidance for Agentic Systems — Complete Index

> Comprehensive inventory of all Rust-related documentation, AGENTS.md files,
> skills, best practices guides, configs, and prompts across the Dicklesworthstone
> GitHub ecosystem. Compiled 2026-03-02.

---

## 1. AGENTS.md Files (Per-Repository Agent Instructions)

Every Rust project carries an `AGENTS.md` at root with project-specific Rust guidance.
`CLAUDE.md` is a local symlink (not on GitHub).

### Core Agent Infrastructure (Rust)

- [storage_ballast_helper/AGENTS.md](https://github.com/Dicklesworthstone/storage_ballast_helper/blob/main/AGENTS.md) — Rust 2024 stable, `forbid(unsafe_code)`, no async runtime, `parking_lot`+`crossbeam-channel`
- [asupersync/AGENTS.md](https://github.com/Dicklesworthstone/asupersync/blob/main/AGENTS.md) — The async runtime itself; Tokio FORBIDDEN; structured concurrency invariants, `Cx`/`Outcome<T,E>` patterns, lock ordering
- [mcp_agent_mail_rust/AGENTS.md](https://github.com/Dicklesworthstone/mcp_agent_mail_rust/blob/main/AGENTS.md) — 9-crate workspace, 34 MCP tools, asupersync mandatory, Tokio FORBIDDEN, broadcast messaging hardcoded to error
- [pi_agent_rust/AGENTS.md](https://github.com/Dicklesworthstone/pi_agent_rust/blob/main/AGENTS.md) — AI coding agent CLI, asupersync, jemalloc, `CARGO_TARGET_DIR` isolation for multi-agent disk pressure
- [destructive_command_guard/AGENTS.md](https://github.com/Dicklesworthstone/destructive_command_guard/blob/main/AGENTS.md) — SIMD pattern matching, Tokio (MCP server mode), `fancy-regex`+`ast-grep-core`, 80+ tests
- [remote_compilation_helper/AGENTS.md](https://github.com/Dicklesworthstone/remote_compilation_helper/blob/main/AGENTS.md) — Cargo build interception, <1ms hook decisions, `memchr` SIMD keyword filter, fail-open design
- [meta_skill/AGENTS.md](https://github.com/Dicklesworthstone/meta_skill/blob/main/AGENTS.md) — Tokio (not asupersync), `rusqlite`/`tantivy`/`git2`, Thompson sampling, profiling profile
- [beads_rust/AGENTS.md](https://github.com/Dicklesworthstone/beads_rust/blob/main/AGENTS.md) — `fsqlite` stack, SHA-256 content-addressed IDs, non-invasive (never runs git)
- [beads_viewer_rust/AGENTS.md](https://github.com/Dicklesworthstone/beads_viewer_rust/blob/main/AGENTS.md) — DCG-based template, `aho-corasick` quick-reject, SARIF output, memory leak CI tests
- [coding_agent_session_search/AGENTS.md](https://github.com/Dicklesworthstone/coding_agent_session_search/blob/main/AGENTS.md) — `tantivy`+`fastembed`+`hnsw_rs`, `dotenvy` for config, connection pool rules
- [vibe_cockpit/AGENTS.md](https://github.com/Dicklesworthstone/vibe_cockpit/blob/main/AGENTS.md) — 12-crate workspace, DuckDB over SQLite rationale, `russh` SSH client, `RobotEnvelope` output
- [cross_agent_session_resumer/AGENTS.md](https://github.com/Dicklesworthstone/cross_agent_session_resumer/blob/main/AGENTS.md) — Canonical IR pivot format, Sigstore-signed releases, `cargo nextest`, coverage thresholds
- [flywheel_connectors/AGENTS.md](https://github.com/Dicklesworthstone/flywheel_connectors/blob/main/AGENTS.md) — Tokio, `wasmtime`/WASI sandboxing, `ed25519-dalek`/`chacha20poly1305` crypto, no interpreted runtimes
- [xf/AGENTS.md](https://github.com/Dicklesworthstone/xf/blob/main/AGENTS.md) — X/Twitter archive search, SIMD `f32x8` + `half` f16 quantization, FSVI binary format, daemon mode
- [rano/AGENTS.md](https://github.com/Dicklesworthstone/rano/blob/main/AGENTS.md) — Network observer for AI CLI processes
- [coding_agent_usage_tracker/AGENTS.md](https://github.com/Dicklesworthstone/coding_agent_usage_tracker/blob/main/AGENTS.md) — Cross-provider LLM cost tracking
- [franken_agent_detection/AGENTS.md](https://github.com/Dicklesworthstone/franken_agent_detection/blob/main/AGENTS.md) — Filesystem-based agent connector detection

### FrankenSuite Reimplementations (Rust)

- [frankensqlite/AGENTS.md](https://github.com/Dicklesworthstone/frankensqlite/blob/main/AGENTS.md) — 24-crate workspace, asupersync MANDATORY, concurrent-writer mode MUST stay ON, clippy pedantic+nursery
- [frankentui/AGENTS.md](https://github.com/Dicklesworthstone/frankentui/blob/main/AGENTS.md) — 18-crate workspace, 16-byte Cell SIMD, diff-based rendering, WASM compilation
- [frankenterm/AGENTS.md](https://github.com/Dicklesworthstone/frankenterm/blob/main/AGENTS.md) — Tokio primary + optional asupersync MCP, `frankenterm-core` deletion FORBIDDEN (deleted 3x by agents), 860+ files
- [frankensearch/AGENTS.md](https://github.com/Dicklesworthstone/frankensearch/blob/main/AGENTS.md) — asupersync MANDATORY, `wide::f32x8` SIMD, `half` f16, RRF fusion, `LabRuntime` testing
- [frankenfs/AGENTS.md](https://github.com/Dicklesworthstone/frankenfs/blob/main/AGENTS.md) — asupersync MANDATORY, spec-first porting doctrine, 15-crate workspace, Alien-Artifact Mode for high-risk decisions
- [frankenlibc/AGENTS.md](https://github.com/Dicklesworthstone/frankenlibc/blob/main/AGENTS.md) — Per-crate unsafe policy table, `// SAFETY:` comments required, Transparent Safety Membrane, P(undetected corruption) <= 2^-64
- [frankenmermaid/AGENTS.md](https://github.com/Dicklesworthstone/frankenmermaid/blob/main/AGENTS.md) — `opt-level = "z"` with per-package `opt-level = 3` override for layout engine, deterministic output
- [franken_engine/AGENTS.md](https://github.com/Dicklesworthstone/franken_engine/blob/main/AGENTS.md) — No rusty_v8/rquickjs bindings, de novo Rust-native execution lanes only
- [franken_node/AGENTS.md](https://github.com/Dicklesworthstone/franken_node/blob/main/AGENTS.md) — Trust-native runtime, 80+ tests, `main:master` sync due to 497-commit desync incident
- [franken_whisper/AGENTS.md](https://github.com/Dicklesworthstone/franken_whisper/blob/main/AGENTS.md) — ASR orchestration, `asupersync`+`frankentorch`+`frankensqlite`, robot/TUI dual mode
- [frankenpandas/AGENTS.md](https://github.com/Dicklesworthstone/frankenpandas/blob/main/AGENTS.md) — Full pandas API parity mandate, AACE innovation, `opt-level = 3`
- [frankentorch/AGENTS.md](https://github.com/Dicklesworthstone/frankentorch/blob/main/AGENTS.md) — asupersync MANDATORY, Deterministic Autograd Contract, correctness outranks speed
- [franken_numpy/AGENTS.md](https://github.com/Dicklesworthstone/franken_numpy/blob/main/AGENTS.md) — Stride Calculus Engine (SCE), strict vs hardened dual-mode, `opt-level = 3`
- [frankenredis/AGENTS.md](https://github.com/Dicklesworthstone/frankenredis/blob/main/AGENTS.md) — Deterministic Latency Replication Core, Redis as behavioral oracle, 10-crate workspace
- [frankenjax/AGENTS.md](https://github.com/Dicklesworthstone/frankenjax/blob/main/AGENTS.md) — Trace Transform Ledger IR, `egg` e-graph optimization, RaptorQ durability pipeline
- [frankenscipy/AGENTS.md](https://github.com/Dicklesworthstone/frankenscipy/blob/main/AGENTS.md) — asupersync MANDATORY, Condition-Aware Solver Portfolio, numerical stability outranks speed
- [franken_networkx/AGENTS.md](https://github.com/Dicklesworthstone/franken_networkx/blob/main/AGENTS.md) — PyO3/Maturin Python bindings, CGSE deterministic semantics, `opt-level = 3`

### Terminal UI Libraries (Rust)

- [charmed_rust/AGENTS.md](https://github.com/Dicklesworthstone/charmed_rust/blob/main/AGENTS.md) — 12-crate workspace, Go-to-Rust translation patterns (interface->trait, goroutine->async, channel->mpsc, nil->Option)
- [rich_rust/AGENTS.md](https://github.com/Dicklesworthstone/rich_rust/blob/main/AGENTS.md) — Rendering pipeline docs, conformance against Python Rich, golden file regression
- [opentui_rust/AGENTS.md](https://github.com/Dicklesworthstone/opentui_rust/blob/main/AGENTS.md) — `warn(unsafe_code)` (not forbid) for terminal FFI, per-module coverage targets (90% color.rs, 80% buffer/)

### ORM / Web / Protocol (Rust)

- [sqlmodel_rust/AGENTS.md](https://github.com/Dicklesworthstone/sqlmodel_rust/blob/main/AGENTS.md) — asupersync MANDATORY, `Outcome<T,E>` 4-valued returns, `#[derive(Model)]` proc macro, legacy Python repos for spec extraction only
- [fastapi_rust/AGENTS.md](https://github.com/Dicklesworthstone/fastapi_rust/blob/main/AGENTS.md) — asupersync MANDATORY, zero-copy HTTP parsing, radix trie router, spec-first porting (never translate Python line-by-line)
- [fastmcp_rust/AGENTS.md](https://github.com/Dicklesworthstone/fastmcp_rust/blob/main/AGENTS.md) — asupersync exclusively, `#[tool]`/`#[resource]`/`#[prompt]` proc macros

### Other Rust Projects

- [toon_rust/AGENTS.md](https://github.com/Dicklesworthstone/toon_rust/blob/main/AGENTS.md) — Single crate, `opt-level = "z"`, clippy pedantic+nursery
- [rust_scriptbots/AGENTS.md](https://github.com/Dicklesworthstone/rust_scriptbots/blob/main/AGENTS.md) — `gpui`/`bevy`/`wgpu` rendering, `candle-core` ML, `warn(unsafe_code)`, `opt-level = 3` + `lto = "thin"`, WASM target
- [rust_proxy/AGENTS.md](https://github.com/Dicklesworthstone/rust_proxy/blob/main/AGENTS.md) — Rust 2021 stable, Tokio/reqwest
- [surface-dial-rust/AGENTS.md](https://github.com/Dicklesworthstone/surface-dial-rust/blob/main/AGENTS.md) — Rust 2021 stable (not 2024), most complete verbatim AGENTS.md template reference
- [rust_stream_deck/AGENTS.md](https://github.com/Dicklesworthstone/rust_stream_deck/blob/main/AGENTS.md) — Elgato Stream Deck driver

---

## 2. Claude Code Agent Farm — Best Practices Guides

Comprehensive multi-thousand-line Rust guides for AI agent swarms:

- [RUST_CLI_TOOLS_BEST_PRACTICES.md](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/best_practices_guides/RUST_CLI_TOOLS_BEST_PRACTICES.md) — 2,729 lines; clap 4.5, anyhow, tokio, dialoguer/indicatif, distribution via cargo-dist, plugin systems, security
- [RUST_SYSTEM_PROGRAMMING_BEST_PRACTICES.md](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/best_practices_guides/RUST_SYSTEM_PROGRAMMING_BEST_PRACTICES.md) — 2,011 lines; zero-copy, SIMD, lock-free concurrency, unsafe guidelines, FFI, `tower` service composition
- [RUST_WEBAPPS_BEST_PRACTICES.md](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/best_practices_guides/RUST_WEBAPPS_BEST_PRACTICES.md) — 1,805 lines; Axum 0.8, SeaORM 1.2, PostgreSQL 16, JWT auth, event sourcing, CQRS, WebSocket
- [SOLANA_ANCHOR_RUST_BEST_PRACTICES.md](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/best_practices_guides/SOLANA_ANCHOR_RUST_BEST_PRACTICES.md) — 2,363 lines; Anchor 0.30, Solana CLI 1.18+, zero-copy accounts, PDA patterns, on-chain security

---

## 3. Claude Code Agent Farm — Configs and Prompts

### Agent Farm Configs (JSON)

- [rust_cli_config.json](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/configs/rust_cli_config.json) — 20 agents, chunk 50, `cargo clippy -- -D warnings`
- [rust_system_config.json](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/configs/rust_system_config.json) — 8 agents, chunk 20, `cargo clippy --all-targets --all-features -- -D warnings`
- [rust_webapps_config.json](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/configs/rust_webapps_config.json) — 10 agents, chunk 25, `cargo clippy --all-targets --all-features -- -D warnings`
- [solana_anchor_config.json](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/configs/solana_anchor_config.json) — 20 agents, chunk 50, `anchor build`/`anchor test`

### Agent Farm Prompts (TXT)

- [default_best_practices_prompt_rust_cli.txt](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/prompts/default_best_practices_prompt_rust_cli.txt) — CLI tools agent instructions with progress tracking
- [default_best_practices_prompt_rust_system.txt](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/prompts/default_best_practices_prompt_rust_system.txt) — Systems programming agent instructions, `cargo miri` mentioned
- [default_best_practices_prompt_rust_web.txt](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/prompts/default_best_practices_prompt_rust_web.txt) — Web applications agent instructions, Axum/Tokio focus
- [default_best_practices_prompt_solana.txt](https://github.com/Dicklesworthstone/claude_code_agent_farm/blob/main/prompts/default_best_practices_prompt_solana.txt) — Solana/Anchor agent instructions

---

## 4. Meta Skill — Rust Skills and Meta-Skills

- [skills/examples/rust-complete/SKILL.md](https://github.com/Dicklesworthstone/meta_skill/blob/main/skills/examples/rust-complete/SKILL.md) — Composite skill: clippy, rustfmt, unsafe minimization, merges error-handling + testing + logging
- [skills/examples/rust-error-handling/SKILL.md](https://github.com/Dicklesworthstone/meta_skill/blob/main/skills/examples/rust-error-handling/SKILL.md) — `thiserror` for libraries, `anyhow` for apps, no `unwrap()` in production
- [.ms/meta-skills/rust-safety.toml](https://github.com/Dicklesworthstone/meta_skill/blob/main/.ms/meta-skills/rust-safety.toml) — Meta-skill compositor assembling 6 Rust safety slices (error-handling, memory-safety, concurrency, defensive, async-safety, testing)

---

## 5. ACFS Infrastructure — Rust Toolchain and Build Pipeline

- [acfs/AGENTS.md](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup/blob/main/acfs/AGENTS.md) — Global agent instructions with `cargo check/clippy/fmt` quality gates
- [scripts/lib/newproj_agents.sh](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup/blob/main/scripts/lib/newproj_agents.sh) — Auto-generates `## Rust Toolchain` section in new project AGENTS.md
- [scripts/lib/newproj_detect.sh](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup/blob/main/scripts/lib/newproj_detect.sh) — Detects Rust projects via `Cargo.toml` presence
- [acfs.manifest.yaml](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup/blob/main/acfs.manifest.yaml) — `lang.rust` entry (nightly toolchain), `tools.ast_grep` (installed via cargo)
- [scripts/generated/install_lang.sh](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup/blob/main/scripts/generated/install_lang.sh) — Rust nightly installation via rustup with SHA256 checksum verification
- [scripts/generated/install_tools.sh](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup/blob/main/scripts/generated/install_tools.sh) — `ast-grep`, `rust_proxy`, `aadc`, `caut` built from source via cargo
- [checksums.yaml](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup/blob/main/checksums.yaml) — SHA256 checksums for `rust` (rustup), `br` (beads_rust), `tru` (toon_rust) installers
- [.claude/hooks/on-file-write.sh](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup/blob/main/.claude/hooks/on-file-write.sh) — UBS auto-runs on `.rs` file saves
- [acfs/onboard/lessons/17_rch.md](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup/blob/main/acfs/onboard/lessons/17_rch.md) — RCH lesson: `rch exec -- cargo build/test/clippy`
- [acfs/onboard/lessons/16_beads_rust.md](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup/blob/main/acfs/onboard/lessons/16_beads_rust.md) — beads_rust CLI onboarding
- [acfs/onboard/lessons/23_srps.md](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup/blob/main/acfs/onboard/lessons/23_srps.md) — `rustc` deprioritized by ananicy-cpp for system responsiveness

---

## 6. Per-Repo `.claude/skills/` Directories

- [flywheel_connectors/.claude/skills/apr/SKILL.md](https://github.com/Dicklesworthstone/flywheel_connectors/blob/main/.claude/skills/apr/SKILL.md) — APR spec refinement for FCP Rust workspace
- [remote_compilation_helper/.claude/skills/rch/SKILL.md](https://github.com/Dicklesworthstone/remote_compilation_helper/blob/main/.claude/skills/rch/SKILL.md) — RCH worker configuration and diagnostics
- [remote_compilation_helper/.claude/skills/remote-compilation-helper-setup/SKILL.md](https://github.com/Dicklesworthstone/remote_compilation_helper/blob/main/.claude/skills/remote-compilation-helper-setup/SKILL.md) — RCH installation and hook setup

---

## 7. Clawdbot Skills (Rust-Adjacent)

- [skills/ubs/SKILL.md](https://github.com/Dicklesworthstone/agent_flywheel_clawdbot_skills_and_integrations/blob/main/skills/ubs/SKILL.md) — UBS scanner covering `.rs` files, `.unwrap()` panics, `unsafe` blocks
- [skills/dcg/SKILL.md](https://github.com/Dicklesworthstone/agent_flywheel_clawdbot_skills_and_integrations/blob/main/skills/dcg/SKILL.md) — DCG usage (Rust binary with SIMD, `cargo +nightly install`)
- [skills/cm/SKILL.md](https://github.com/Dicklesworthstone/agent_flywheel_clawdbot_skills_and_integrations/blob/main/skills/cm/SKILL.md) — `cm init --starter rust` playbook template

---

## 8. Personal Website Project Pages (Rust)

- [jeffreyemanuel.com/projects/frankensqlite](https://jeffreyemanuel.com/projects/frankensqlite)
- [jeffreyemanuel.com/projects/meta-skill](https://jeffreyemanuel.com/projects/meta-skill)
- [jeffreyemanuel.com/projects/remote-compilation-helper](https://jeffreyemanuel.com/projects/remote-compilation-helper)
- [jeffreyemanuel.com/projects/pi-agent-rust](https://jeffreyemanuel.com/projects/pi-agent-rust)
- [jeffreyemanuel.com/projects/frankentui](https://jeffreyemanuel.com/projects/frankentui)
- [jeffreyemanuel.com/projects/beads-rust](https://jeffreyemanuel.com/projects/beads-rust)
- [jeffreyemanuel.com/projects/wezterm-automata](https://jeffreyemanuel.com/projects/wezterm-automata)
- [jeffreyemanuel.com/projects/fastmcp-rust](https://jeffreyemanuel.com/projects/fastmcp-rust)
- [jeffreyemanuel.com/projects/rano](https://jeffreyemanuel.com/projects/rano)
- [jeffreyemanuel.com/projects/toon-rust](https://jeffreyemanuel.com/projects/toon-rust)
- [jeffreyemanuel.com/projects/coding-agent-usage-tracker](https://jeffreyemanuel.com/projects/coding-agent-usage-tracker)
- [jeffreyemanuel.com/projects/fast-vector-similarity](https://jeffreyemanuel.com/projects/fast-vector-similarity)
- [jeffreyemanuel.com/projects/ultrasearch](https://jeffreyemanuel.com/projects/ultrasearch)
- [jeffreyemanuel.com/projects/charmed-rust](https://jeffreyemanuel.com/projects/charmed-rust)
- [jeffreyemanuel.com/projects/opentui-rust](https://jeffreyemanuel.com/projects/opentui-rust)
- [jeffreyemanuel.com/projects/rich-rust](https://jeffreyemanuel.com/projects/rich-rust)
- [jeffreyemanuel.com/tldr](https://jeffreyemanuel.com/tldr) — Lists CASS, DCG, XF, MS as Rust-powered tools

---

## 9. X/Twitter Posts (@doodlestein)

- [beads_rust announcement](https://x.com/doodlestein/status/2012972038332260744)
- [Complex Rust program via agents](https://x.com/doodlestein/status/2007826358974427251)
- [DCG announcement](https://x.com/doodlestein/status/2015510232869245033)
- [Rust Agent Mail port](https://x.com/doodlestein/status/2026872257172161002)
- [Pi Agent Rust port](https://x.com/doodlestein/status/2018482331825172672)
- [FrankenTUI plan](https://x.com/doodlestein/status/2017719001594380703)
- [Hybrid search system in Rust](https://x.com/doodlestein/status/2025758721888924091)
- [Steve Yegge endorses beads_rust](https://x.com/Steve_Yegge/status/2012974366188032144)

---

## Universal Patterns Across All Rust AGENTS.md Files

### Mandatory Quality Gates (every commit)

```bash
cargo check --all-targets            # or --workspace --all-targets
cargo clippy --all-targets -- -D warnings   # pedantic + nursery enabled
cargo fmt --check
cargo test
ubs <changed-files>                  # UBS before every commit
```

### Toolchain Standard

- **Edition**: Rust 2024, nightly channel (pinned via `rust-toolchain.toml`)
- **Components**: `rustfmt`, `clippy`
- **Unsafe**: `#![forbid(unsafe_code)]` (default) or `#![deny(unsafe_code)]` with per-module exceptions

### Release Profile Variants

**Size-optimized** (CLI tools, WASM):
```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

**Speed-optimized** (libraries, databases, scientific):
```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

### Async Runtime Split

| Runtime | Repos |
|---------|-------|
| asupersync (Tokio FORBIDDEN) | frankensqlite, mcp_agent_mail_rust, asupersync, frankensearch, frankenfs, fastmcp_rust, fastapi_rust, sqlmodel_rust, frankentorch, frankenscipy, pi_agent_rust, coding_agent_session_search |
| Tokio | meta_skill, destructive_command_guard, frankenterm, flywheel_connectors, remote_compilation_helper, rust_proxy |
| None (sync) | beads_rust, storage_ballast_helper, rich_rust, frankentui, toon_rust, charmed_rust, franken_numpy, frankenpandas, frankenredis, frankenjax, franken_networkx |
