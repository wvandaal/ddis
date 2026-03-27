//! `braid model` — Embedding model management (ADR-EMBEDDING-001).
//!
//! Manages the potion-base-8M model files used by the CCE for semantic
//! text embeddings. The kernel never performs IO (C8); this module handles
//! all model file discovery and loading.
//!
//! Model discovery order:
//! 1. `.braid/models/potion-base-8M/` (project-local)
//! 2. `~/.braid/models/potion-base-8M/` (global)
//!
//! Traces to: ADR-EMBEDDING-001 (Model Management), ADR-EMBEDDING-002 (Embedder Selection)

use std::path::{Path, PathBuf};

/// Model name constant.
pub const MODEL_NAME: &str = "potion-base-8M";

/// Required model files.
const MODEL_FILE: &str = "model.safetensors";
const TOKENIZER_FILE: &str = "tokenizer.json";

/// Result of model discovery.
#[derive(Debug)]
pub struct ModelInfo {
    /// Path to the model directory (contains model.safetensors + tokenizer.json).
    pub path: PathBuf,
    /// Whether this is project-local or global.
    pub scope: ModelScope,
    /// Size of model.safetensors in bytes.
    pub model_size: u64,
    /// Size of tokenizer.json in bytes.
    #[allow(dead_code)]
    pub tokenizer_size: u64,
}

/// Where the model was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelScope {
    /// Found in .braid/models/ (project-local).
    ProjectLocal,
    /// Found in ~/.braid/models/ (global).
    Global,
}

/// Discover the model files, checking project-local first, then global.
///
/// Returns `None` if model files are not found at either location.
/// Both `model.safetensors` and `tokenizer.json` must exist.
pub fn discover_model(store_path: &Path) -> Option<ModelInfo> {
    // 1. Project-local: .braid/models/<MODEL_NAME>/
    let project_local = store_path.join("models").join(MODEL_NAME);
    if let Some(info) = check_model_dir(&project_local, ModelScope::ProjectLocal) {
        return Some(info);
    }

    // 2. Global: ~/.braid/models/<MODEL_NAME>/
    if let Some(home) = dirs::home_dir() {
        let global = home.join(".braid").join("models").join(MODEL_NAME);
        if let Some(info) = check_model_dir(&global, ModelScope::Global) {
            return Some(info);
        }
    }

    None
}

/// Check if a directory contains valid model files.
fn check_model_dir(dir: &Path, scope: ModelScope) -> Option<ModelInfo> {
    let model_path = dir.join(MODEL_FILE);
    let tokenizer_path = dir.join(TOKENIZER_FILE);

    let model_meta = std::fs::metadata(&model_path).ok()?;
    let tokenizer_meta = std::fs::metadata(&tokenizer_path).ok()?;

    // Sanity check: model should be > 1MB, tokenizer > 1KB.
    if model_meta.len() < 1_000_000 || tokenizer_meta.len() < 1_000 {
        return None;
    }

    Some(ModelInfo {
        path: dir.to_path_buf(),
        scope,
        model_size: model_meta.len(),
        tokenizer_size: tokenizer_meta.len(),
    })
}

/// Resolve the best available TextEmbedder for the given store.
///
/// If model files are found, loads the model2vec Embedder (feature-gated).
/// Otherwise falls back to HashEmbedder.
///
/// ADR-EMBEDDING-002: Prefer model2vec when available, hash fallback.
pub fn resolve_embedder(
    store_path: &Path,
) -> (Box<dyn braid_kernel::embedding::TextEmbedder>, &'static str) {
    if let Some(info) = discover_model(store_path) {
        // Try to load model2vec embedder.
        #[cfg(feature = "embeddings")]
        {
            let model_path = info.path.join(MODEL_FILE);
            let tokenizer_path = info.path.join(TOKENIZER_FILE);

            match (std::fs::read(&model_path), std::fs::read(&tokenizer_path)) {
                (Ok(model_bytes), Ok(tokenizer_bytes)) => {
                    match braid_kernel::embedding::Embedder::from_bytes(
                        &model_bytes,
                        &tokenizer_bytes,
                    ) {
                        Ok(embedder) => {
                            return (Box::new(embedder), "model2vec");
                        }
                        Err(e) => {
                            eprintln!(
                                "warning: model load failed: {e}, falling back to hash embedder"
                            );
                        }
                    }
                }
                _ => {
                    eprintln!("warning: could not read model files, falling back to hash embedder");
                }
            }
        }

        #[cfg(not(feature = "embeddings"))]
        {
            let _ = info; // Suppress unused warning.
            eprintln!(
                "warning: model found but 'embeddings' feature not enabled, using hash embedder"
            );
        }
    }

    // Fallback: HashEmbedder (always available, INV-EMBEDDING-003).
    let hash = braid_kernel::embedding::HashEmbedder::new(braid_kernel::embedding::DEFAULT_DIM);
    (Box::new(hash), "hash")
}

/// Format model status for display.
pub fn format_status(store_path: &Path) -> String {
    match discover_model(store_path) {
        Some(info) => {
            let scope = match info.scope {
                ModelScope::ProjectLocal => "project-local",
                ModelScope::Global => "global",
            };
            format!(
                "model: {} ({}, {:.1}MB)\n  path: {}\n  embedder: model2vec",
                MODEL_NAME,
                scope,
                info.model_size as f64 / 1_000_000.0,
                info.path.display(),
            )
        }
        None => {
            let global_path = dirs::home_dir()
                .map(|h| h.join(".braid").join("models").join(MODEL_NAME))
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "~/.braid/models/potion-base-8M".to_string());
            format!(
                "model: not found (using hash embedder)\n  \
                 install: download potion-base-8M model files to:\n    \
                 {}/model.safetensors\n    \
                 {}/tokenizer.json",
                global_path, global_path,
            )
        }
    }
}
