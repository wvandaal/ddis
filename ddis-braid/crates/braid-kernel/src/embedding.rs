//! Semantic text embedding for concept crystallization (OBSERVER-6).
//!
//! Provides two embedding strategies:
//!
//! - [`Embedder`] (feature `embeddings`): Model2Vec static embeddings.
//!   Tokenize → vocabulary lookup → mean-pool → L2-normalize.
//!   Requires pre-loaded safetensors weights and tokenizer JSON.
//!
//! - [`HashEmbedder`]: BLAKE3-based random indexing (always available).
//!   Preserves word-overlap similarity but has no semantic understanding.
//!
//! Both implement [`TextEmbedder`] so callers can use either interchangeably.
//! The CLI crate handles model loading (IO); this module is pure computation.
//!
//! # Invariants
//!
//! - **INV-EMBEDDING-001**: Determinism — same input text always produces same vector.
//! - **INV-EMBEDDING-002**: Normalization — non-empty output vectors have L2 norm ≈ 1.0.
//! - **INV-EMBEDDING-003**: Graceful degradation — no panics on invalid input.
//!
//! # Design Decisions
//!
//! - ADR-FOUNDATION-015: Observer Formalism — embeddings expand the epistemological horizon.
//! - C8 compliance: kernel takes pre-loaded bytes, no filesystem/network access.
//! - The embedding model is policy-layer configuration, not kernel identity.

use std::fmt;

/// Default embedding dimension (matches potion-base-8M).
pub const DEFAULT_DIM: usize = 256;

/// Errors during embedder construction.
#[derive(Debug, Clone)]
pub struct EmbeddingError(
    /// Human-readable description of what went wrong.
    pub String,
);

impl fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EmbeddingError {}

/// Trait for text embedding implementations (INV-EMBEDDING-001).
///
/// Both [`Embedder`] and [`HashEmbedder`] implement this trait so the CCE
/// pipeline can use either interchangeably via `Box<dyn TextEmbedder>`.
pub trait TextEmbedder: Send + Sync {
    /// Embed text into a fixed-dimension vector.
    ///
    /// INV-EMBEDDING-001: Same text always produces the same vector.
    /// Returns a zero vector for empty input.
    fn embed(&self, text: &str) -> Vec<f32>;

    /// The dimensionality of output vectors.
    fn dim(&self) -> usize;

    /// Recommended cosine similarity threshold for concept assignment (STEER-2).
    ///
    /// The threshold is a property of the metric space (embedder), not the domain.
    /// Model2vec produces dense cosine distributions [0.15-0.40]; hash embedder
    /// produces sparse distributions [0.0, 0.6-1.0]. Default is 0.65 (hash-calibrated).
    /// Policy config `concept.join-threshold` can override this at runtime.
    fn join_threshold(&self) -> f32 {
        0.65
    }
}

// ===================================================================
// Model2Vec Embedder (feature-gated)
// ===================================================================

/// Model2Vec static text embedder.
///
/// Tokenizes input text, looks up token embeddings from a pre-loaded weight
/// matrix, mean-pools the vectors, and L2-normalizes the result.
///
/// Construction invariant: `weights.len() == vocab_size * dim`.
///
/// INV-EMBEDDING-001: Deterministic — same input always produces same vector.
/// INV-EMBEDDING-002: Non-empty output has L2 norm ≈ 1.0.
#[cfg(feature = "embeddings")]
pub struct Embedder {
    /// Flattened weight matrix: weights[id * dim .. (id+1) * dim] = embedding for token `id`.
    weights: Vec<f32>,
    /// Embedding dimension.
    dim: usize,
    /// Number of tokens in the vocabulary.
    vocab_size: usize,
    /// HuggingFace tokenizer for text → token IDs.
    tokenizer: tokenizers::Tokenizer,
}

#[cfg(feature = "embeddings")]
impl Embedder {
    /// Construct from pre-loaded safetensors bytes and tokenizer JSON.
    ///
    /// No IO — caller provides the raw bytes. C8 compliant.
    /// Looks for a tensor named `"embeddings"` first, then falls back to
    /// the first 2D F32 tensor found in the safetensors file.
    pub fn from_bytes(
        safetensor_bytes: &[u8],
        tokenizer_json: &[u8],
    ) -> Result<Self, EmbeddingError> {
        use safetensors::{Dtype, SafeTensors};

        let tensors = SafeTensors::deserialize(safetensor_bytes)
            .map_err(|e| EmbeddingError(format!("safetensors parse: {e}")))?;

        // Model2Vec convention: tensor named "embeddings".
        // Fallback: first 2D F32 tensor.
        let tensor = tensors
            .tensor("embeddings")
            .or_else(|_| {
                tensors
                    .names()
                    .into_iter()
                    .find_map(|name| {
                        let t = tensors.tensor(name).ok()?;
                        if t.shape().len() == 2 && t.dtype() == Dtype::F32 {
                            Some(t)
                        } else {
                            None
                        }
                    })
                    .ok_or(safetensors::SafeTensorError::TensorNotFound(
                        "no 2D F32 tensor found".into(),
                    ))
            })
            .map_err(|e| EmbeddingError(format!("tensor lookup: {e}")))?;

        let shape = tensor.shape();
        if shape.len() != 2 {
            return Err(EmbeddingError(format!(
                "expected 2D tensor, got {len}D",
                len = shape.len()
            )));
        }
        let vocab_size = shape[0];
        let dim = shape[1];

        if tensor.dtype() != Dtype::F32 {
            return Err(EmbeddingError(format!(
                "expected F32 dtype, got {:?}",
                tensor.dtype()
            )));
        }

        let weights = bytes_to_f32_le(tensor.data());
        if weights.len() != vocab_size * dim {
            return Err(EmbeddingError(format!(
                "weight count mismatch: {} != {} * {}",
                weights.len(),
                vocab_size,
                dim
            )));
        }

        let tokenizer = tokenizers::Tokenizer::from_bytes(tokenizer_json)
            .map_err(|e| EmbeddingError(format!("tokenizer: {e}")))?;

        Ok(Self {
            weights,
            dim,
            vocab_size,
            tokenizer,
        })
    }

    /// Number of tokens in the vocabulary.
    pub fn vocab_size(&self) -> usize {
        self.vocab_size
    }
}

#[cfg(feature = "embeddings")]
impl TextEmbedder for Embedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        let encoding = match self.tokenizer.encode(text, false) {
            Ok(enc) => enc,
            Err(_) => return vec![0.0; self.dim],
        };

        let ids = encoding.get_ids();
        if ids.is_empty() {
            return vec![0.0; self.dim];
        }

        let mut sum = vec![0.0f32; self.dim];
        let mut count = 0u32;

        for &id in ids {
            let id = id as usize;
            if id < self.vocab_size {
                let start = id * self.dim;
                for (i, &w) in self.weights[start..start + self.dim].iter().enumerate() {
                    sum[i] += w;
                }
                count += 1;
            }
        }

        if count == 0 {
            return vec![0.0; self.dim];
        }

        // Mean-pool.
        let inv_count = 1.0 / count as f32;
        for v in &mut sum {
            *v *= inv_count;
        }

        // L2-normalize (INV-EMBEDDING-002).
        l2_normalize(&mut sum);
        sum
    }

    fn dim(&self) -> usize {
        self.dim
    }

    /// Model2vec cosine distribution is dense [0.15-0.40].
    /// Empirically calibrated from potion-base-8M on DOGFOOD-3 observations.
    fn join_threshold(&self) -> f32 {
        0.20
    }
}

// ===================================================================
// Hash Embedder (always available, zero external data)
// ===================================================================

/// BLAKE3-based pseudo-embedder using random indexing.
///
/// Each word hashes to a deterministic ±1 activation pattern across
/// dimensions. Shared words between texts contribute shared signal,
/// so word-overlap similarity is preserved geometrically.
///
/// INV-EMBEDDING-001: Same text always produces same vector (BLAKE3 determinism).
/// INV-EMBEDDING-002: Non-empty output has L2 norm ≈ 1.0.
pub struct HashEmbedder {
    dim: usize,
}

impl HashEmbedder {
    /// Create a hash embedder with the given dimension.
    ///
    /// For compatibility with model2vec embeddings, use [`DEFAULT_DIM`] (256).
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }
}

impl TextEmbedder for HashEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        let words = tokenize_simple(text);
        if words.is_empty() {
            return vec![0.0; self.dim];
        }

        let mut sum = vec![0.0f32; self.dim];

        for word in &words {
            let hash = blake3::hash(word.as_bytes());
            let hash_bytes = hash.as_bytes();
            for (i, s) in sum.iter_mut().enumerate() {
                let byte_idx = (i / 8) % 32;
                let bit_idx = i % 8;
                if hash_bytes[byte_idx] & (1 << bit_idx) != 0 {
                    *s += 1.0;
                } else {
                    *s -= 1.0;
                }
            }
        }

        l2_normalize(&mut sum);
        sum
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

// ===================================================================
// Pure math functions
// ===================================================================

/// Cosine similarity between two vectors.
///
/// Returns 1.0 for identical directions, 0.0 for orthogonal, -1.0 for opposite.
/// Returns 0.0 if either vector is zero or if dimensions mismatch.
///
/// INV-EMBEDDING-003: Never panics. Returns 0.0 for degenerate inputs.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (&ai, &bi) in a.iter().zip(b.iter()) {
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
    }

    let denom = (norm_a * norm_b).sqrt();
    if denom < 1e-10 {
        return 0.0;
    }
    (dot / denom).clamp(-1.0, 1.0)
}

/// Compute the centroid (element-wise mean) of a set of vectors.
///
/// Returns an empty vector if `vectors` is empty.
/// Precondition: all vectors have the same dimension.
pub fn centroid(vectors: &[&[f32]]) -> Vec<f32> {
    if vectors.is_empty() {
        return Vec::new();
    }
    let dim = vectors[0].len();
    let mut sum = vec![0.0f32; dim];
    for v in vectors {
        for (i, &val) in v.iter().enumerate() {
            sum[i] += val;
        }
    }
    let inv = 1.0 / vectors.len() as f32;
    for s in &mut sum {
        *s *= inv;
    }
    sum
}

/// Intra-cluster variance (mean squared Euclidean distance from centroid).
///
/// Returns 0.0 if `vectors` is empty.
/// Precondition: `centroid.len()` matches each vector's dimension.
pub fn variance(vectors: &[&[f32]], centroid: &[f32]) -> f32 {
    if vectors.is_empty() {
        return 0.0;
    }
    let total: f32 = vectors
        .iter()
        .map(|v| {
            v.iter()
                .zip(centroid.iter())
                .map(|(a, c)| (a - c) * (a - c))
                .sum::<f32>()
        })
        .sum();
    total / vectors.len() as f32
}

// ===================================================================
// Serialization helpers (embedding ↔ datom Bytes value)
// ===================================================================

/// Serialize an embedding vector to bytes (little-endian f32).
///
/// The result is suitable for storing as a `Value::Bytes` datom.
/// 256 floats → 1024 bytes.
pub fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(embedding.len() * 4);
    for &v in embedding {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// Deserialize bytes (little-endian f32) back to an embedding vector.
///
/// Returns an empty vector if `bytes` length is not a multiple of 4.
pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes_to_f32_le(bytes)
}

// ===================================================================
// Internal helpers
// ===================================================================

/// L2-normalize a vector in place. Zero vectors remain zero.
///
/// Used by embedders, by `crystallize_concepts` to normalize merged centroids,
/// and by the CLI to normalize updated centroids on concept join.
pub fn l2_normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-10 {
        let inv = 1.0 / norm;
        for x in v.iter_mut() {
            *x *= inv;
        }
    }
}

/// Simple word tokenizer: lowercase, split on non-alphanumeric, filter short tokens.
///
/// Used by [`HashEmbedder`] for word-level hashing.
fn tokenize_simple(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(String::from)
        .collect()
}

/// Convert little-endian byte slice to f32 vector.
fn bytes_to_f32_le(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

// ===================================================================
// Tests
// ===================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- cosine_similarity --

    #[test]
    fn cosine_identical_vectors() {
        let v = [1.0, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_vectors() {
        let a = [1.0, 0.0];
        let b = [0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn cosine_opposite_vectors() {
        let a = [1.0, 0.0];
        let b = [-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_zero_vector_returns_zero() {
        let z = [0.0; 10];
        let v = [1.0; 10];
        assert_eq!(cosine_similarity(&z, &v), 0.0);
        assert_eq!(cosine_similarity(&z, &z), 0.0);
    }

    #[test]
    fn cosine_scaled_vectors_are_identical() {
        let a = [1.0, 2.0, 3.0];
        let b = [2.0, 4.0, 6.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    // -- centroid --

    #[test]
    fn centroid_single_vector() {
        let v: &[f32] = &[1.0, 2.0, 3.0];
        let c = centroid(&[v]);
        assert_eq!(c, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn centroid_mean_of_two() {
        let a: &[f32] = &[0.0, 0.0];
        let b: &[f32] = &[2.0, 4.0];
        let c = centroid(&[a, b]);
        assert!((c[0] - 1.0).abs() < 1e-6);
        assert!((c[1] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn centroid_empty_returns_empty() {
        let c = centroid(&[]);
        assert!(c.is_empty());
    }

    // -- variance --

    #[test]
    fn variance_identical_vectors_is_zero() {
        let v: &[f32] = &[1.0, 2.0];
        let c = centroid(&[v, v]);
        let var = variance(&[v, v], &c);
        assert!(var < 1e-6);
    }

    #[test]
    fn variance_spread_vectors() {
        let a: &[f32] = &[0.0];
        let b: &[f32] = &[2.0];
        let c = centroid(&[a, b]);
        let var = variance(&[a, b], &c);
        // Each point is 1.0 from centroid, variance = (1+1)/2 = 1.0
        assert!((var - 1.0).abs() < 1e-6);
    }

    #[test]
    fn variance_empty_is_zero() {
        assert_eq!(variance(&[], &[]), 0.0);
    }

    // -- serialization roundtrip --

    #[test]
    fn embedding_bytes_roundtrip() {
        let original = [1.0f32, -2.5, 0.0, std::f32::consts::PI];
        let bytes = embedding_to_bytes(&original);
        let recovered = bytes_to_embedding(&bytes);
        assert_eq!(&original[..], &recovered[..]);
    }

    #[test]
    fn embedding_bytes_empty() {
        let bytes = embedding_to_bytes(&[]);
        assert!(bytes.is_empty());
        let recovered = bytes_to_embedding(&bytes);
        assert!(recovered.is_empty());
    }

    // -- HashEmbedder --

    #[test]
    fn hash_embedder_deterministic() {
        let h = HashEmbedder::new(DEFAULT_DIM);
        let v1 = h.embed("hello world");
        let v2 = h.embed("hello world");
        assert_eq!(
            v1, v2,
            "INV-EMBEDDING-001: same input must produce same output"
        );
    }

    #[test]
    fn hash_embedder_correct_dimension() {
        let h = HashEmbedder::new(DEFAULT_DIM);
        let v = h.embed("test input");
        assert_eq!(v.len(), DEFAULT_DIM);
    }

    #[test]
    fn hash_embedder_normalized() {
        let h = HashEmbedder::new(DEFAULT_DIM);
        let v = h.embed("some test text here");
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "INV-EMBEDDING-002: expected norm ≈ 1.0, got {norm}"
        );
    }

    #[test]
    fn hash_embedder_similar_texts_positive_cosine() {
        let h = HashEmbedder::new(DEFAULT_DIM);
        let v1 = h.embed("event sourcing engine");
        let v2 = h.embed("event replay engine");
        // Share 2/3 words → positive cosine.
        let sim = cosine_similarity(&v1, &v2);
        assert!(
            sim > 0.0,
            "texts sharing words should have positive similarity, got {sim}"
        );
    }

    #[test]
    fn hash_embedder_different_texts_low_cosine() {
        let h = HashEmbedder::new(DEFAULT_DIM);
        let v1 = h.embed("event sourcing pipeline");
        let v2 = h.embed("SQL injection vulnerability attack");
        // No shared words → cosine near 0.
        let sim = cosine_similarity(&v1, &v2);
        assert!(
            sim.abs() < 0.5,
            "texts with no shared words should have low similarity, got {sim}"
        );
    }

    #[test]
    fn hash_embedder_empty_text_returns_zero_vector() {
        let h = HashEmbedder::new(DEFAULT_DIM);
        let v = h.embed("");
        assert!(v.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn hash_embedder_short_words_filtered() {
        let h = HashEmbedder::new(DEFAULT_DIM);
        // "a" and "is" are < 3 chars, filtered out.
        let v1 = h.embed("a is the");
        let v2 = h.embed("the");
        // Both should only contain the word "the".
        assert_eq!(v1, v2);
    }

    // -- l2_normalize --

    #[test]
    fn l2_normalize_unit_vector() {
        let mut v = [3.0f32, 4.0];
        l2_normalize(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_zero_vector_stays_zero() {
        let mut v = [0.0f32, 0.0, 0.0];
        l2_normalize(&mut v);
        assert!(v.iter().all(|&x| x == 0.0));
    }

    // -- tokenize_simple --

    #[test]
    fn tokenize_simple_basic() {
        let words = tokenize_simple("Hello World! Test-123");
        assert_eq!(words, ["hello", "world", "test", "123"]);
    }

    #[test]
    fn tokenize_simple_filters_short() {
        let words = tokenize_simple("a is the big");
        assert_eq!(words, ["the", "big"]);
    }

    // -- CCE-TEST additional embedding tests --

    /// (14) Similar texts should have higher cosine than dissimilar texts.
    #[test]
    fn cosine_similar_higher_than_dissimilar() {
        let h = HashEmbedder::new(DEFAULT_DIM);
        let event_sourcing = h.embed("event sourcing engine pipeline");
        let event_replay = h.embed("event replay engine pipeline");
        let sql_injection = h.embed("SQL injection vulnerability attack");

        let sim_similar = cosine_similarity(&event_sourcing, &event_replay);
        let sim_dissimilar = cosine_similarity(&event_sourcing, &sql_injection);

        assert!(
            sim_similar > sim_dissimilar,
            "similar texts cosine ({sim_similar}) should exceed dissimilar ({sim_dissimilar})"
        );
    }

    /// (18) Serialization roundtrip preserves embedding values.
    #[test]
    fn embedding_roundtrip_preserves_values() {
        let v = [0.3f32, 0.5, 0.7, 0.1];
        // update_centroid with old_count=0 means: (old * 0 + new) / 1 = new.
        let centroid = super::embedding_to_bytes(&v);
        let recovered = super::bytes_to_embedding(&centroid);
        for (i, (&a, &b)) in recovered.iter().zip(v.iter()).enumerate() {
            assert!(
                (a - b).abs() < 1e-6,
                "dim {i}: centroid of one should equal input, got {a} vs {b}"
            );
        }
    }

    /// Embed is deterministic (same text → same vector, INV-EMBEDDING-001).
    #[test]
    fn hash_embedder_deterministic_repeated() {
        let h = HashEmbedder::new(DEFAULT_DIM);
        let v1 = h.embed("deterministic embedding test");
        let v2 = h.embed("deterministic embedding test");
        let v3 = h.embed("deterministic embedding test");
        assert_eq!(v1, v2);
        assert_eq!(v2, v3);
    }

    /// Hash embedder output is normalized (L2 norm ≈ 1.0, INV-EMBEDDING-002).
    #[test]
    fn hash_embedder_output_normalized() {
        let h = HashEmbedder::new(DEFAULT_DIM);
        let v = h.embed("event sourcing pipeline architecture");
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "non-empty embedding should have L2 norm ≈ 1.0, got {norm}"
        );
    }
}
