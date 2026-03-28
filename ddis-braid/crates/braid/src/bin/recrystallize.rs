//! One-off bulk recrystallization of observations.
//!
//! Opens the braid store from `.braid/txns/`, re-embeds ALL observations with
//! the current embedder (model2vec if available, hash fallback), then runs
//! `crystallize_concepts()` on the full set to replace stale concept assignments
//! from the old hash embedder.
//!
//! Usage: `recrystallize [--dry-run] [--path .braid]`
//!
//! This binary is additive-only: it creates new concept and membership datoms
//! via append-only transactions. It never deletes or mutates existing datoms (C1).
//!
//! Uses only `braid_kernel` APIs (no dependency on `braid` library target).

use std::collections::BTreeSet;
use std::path::PathBuf;

use braid_kernel::concept::{concept_to_datoms, crystallize_concepts, membership_datoms};
use braid_kernel::datom::{AgentId, Attribute, Datom, Op, ProvenanceType, TxId, Value};
use braid_kernel::embedding::{embedding_to_bytes, TextEmbedder};
use braid_kernel::layout::{serialize_tx, ContentHash, TxFile, TxFilePath};
use braid_kernel::Store;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let braid_path = args
        .iter()
        .position(|a| a == "--path")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".braid"));

    if !braid_path.join("txns").is_dir() {
        eprintln!("error: no braid store found at {}", braid_path.display());
        std::process::exit(1);
    }

    // 1. Load the store from transaction files.
    eprintln!("loading store from {}...", braid_path.display());
    let store = load_store_from_txns(&braid_path);
    let datom_count = store.datoms().count();
    eprintln!("store: {datom_count} datoms");

    // 2. Resolve the current embedder (model2vec if available, hash fallback).
    let (embedder, embedder_type) = resolve_embedder(&braid_path);
    let embedder_kw = format!(":embedder/{embedder_type}");
    eprintln!("embedder: {embedder_type}");

    // 3. Collect ALL observations that have :exploration/body text.
    let body_attr = Attribute::from_keyword(":exploration/body");
    let embed_attr = Attribute::from_keyword(":exploration/embedding");
    let concept_attr = Attribute::from_keyword(":exploration/concept");

    // Collect observation entities that have a body (deduplicated by entity).
    let mut entity_body: std::collections::BTreeMap<braid_kernel::datom::EntityId, String> =
        std::collections::BTreeMap::new();
    for d in store.datoms() {
        if d.op == Op::Assert && d.attribute == body_attr {
            if let Value::String(ref s) = d.value {
                entity_body.insert(d.entity, s.clone());
            }
        }
    }

    let total_obs = entity_body.len();
    eprintln!("observations with body: {total_obs}");

    // Count currently categorized vs uncategorized.
    let categorized: std::collections::HashSet<braid_kernel::datom::EntityId> = store
        .datoms()
        .filter(|d| d.op == Op::Assert && d.attribute == concept_attr)
        .map(|d| d.entity)
        .collect();
    let uncategorized_count = entity_body
        .keys()
        .filter(|e| !categorized.contains(e))
        .count();
    eprintln!(
        "currently categorized: {}, uncategorized: {uncategorized_count}",
        categorized.len()
    );

    if total_obs == 0 {
        eprintln!("no observations to recrystallize");
        return;
    }

    // 4. Re-embed each observation with the current embedder.
    eprintln!("embedding {total_obs} observations with {embedder_type}...");
    let mut observations: Vec<(braid_kernel::datom::EntityId, Vec<f32>, String)> = Vec::new();
    let mut embed_datoms: Vec<Datom> = Vec::new();

    let agent = AgentId::from_name("braid:recrystallize");
    let embed_tx = next_tx_id(&store, agent);

    for (i, (&eid, body)) in entity_body.iter().enumerate() {
        let emb = embedder.embed(body);

        // Store the new embedding as a datom (overrides old hash embedding via LWW).
        embed_datoms.push(Datom::new(
            eid,
            embed_attr.clone(),
            Value::Bytes(embedding_to_bytes(&emb)),
            embed_tx,
            Op::Assert,
        ));

        observations.push((eid, emb, body.clone()));

        if (i + 1) % 100 == 0 {
            eprintln!("  embedded {}/{total_obs}", i + 1);
        }
    }
    eprintln!("  embedded {total_obs}/{total_obs} -- done");

    // Write embedding update transaction.
    if !dry_run && !embed_datoms.is_empty() {
        let embed_tx_file = TxFile {
            tx_id: embed_tx,
            agent,
            provenance: ProvenanceType::Derived,
            rationale: format!(
                "recrystallize: re-embedded {} observations with {embedder_type}",
                embed_datoms.len()
            ),
            causal_predecessors: vec![],
            datoms: embed_datoms,
        };
        write_tx_to_disk(&braid_path, &embed_tx_file);
        eprintln!("wrote embedding transaction ({total_obs} datoms)");
    }

    // 5. Run crystallize_concepts on the full set.
    let join_threshold: f32 = braid_kernel::config::get_config(&store, "concept.join-threshold")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| embedder.join_threshold());
    let crystallize_threshold: f32 =
        braid_kernel::config::get_config(&store, "concept.crystallize-threshold")
            .and_then(|v| v.parse().ok())
            .unwrap_or(join_threshold);
    let min_cluster_size: usize =
        braid_kernel::config::get_config(&store, "concept.min-cluster-size")
            .and_then(|v| v.parse().ok())
            .unwrap_or(braid_kernel::concept::MIN_CLUSTER_SIZE);

    eprintln!("crystallizing: threshold={crystallize_threshold:.3}, min_size={min_cluster_size}");

    let new_concepts = crystallize_concepts(&observations, crystallize_threshold, min_cluster_size);

    eprintln!("crystallized {} new concepts:", new_concepts.len());
    let mut total_memberships = 0usize;
    for c in &new_concepts {
        eprintln!(
            "  {} ({} members, variance={:.4})",
            c.name,
            c.members.len(),
            c.variance
        );
        total_memberships += c.members.len();
    }

    if new_concepts.is_empty() {
        eprintln!("no new concepts formed -- nothing to write");
        return;
    }

    if dry_run {
        eprintln!(
            "[dry-run] would write {} concepts, {total_memberships} memberships",
            new_concepts.len()
        );
        return;
    }

    // 6. Write concept datoms and membership datoms as a transaction.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // Use embed_tx + 1 second to ensure monotonicity.
    let cryst_tx = TxId::new(embed_tx.wall_time() + 1, 0, agent);
    let mut cryst_datoms: Vec<Datom> = Vec::new();

    for concept in &new_concepts {
        // Concept entity datoms (name, description, embedding, member-count, etc.).
        for (e, a, v) in concept_to_datoms(concept, now) {
            cryst_datoms.push(Datom::new(e, a, v, cryst_tx, Op::Assert));
        }
        // Embedder type for the concept.
        cryst_datoms.push(Datom::new(
            concept.entity,
            Attribute::from_keyword(":concept/embedder-type"),
            Value::Keyword(embedder_kw.clone()),
            cryst_tx,
            Op::Assert,
        ));
        // Membership datoms (:exploration/concept -> Ref).
        for (e, a, v) in membership_datoms(concept.entity, &concept.members) {
            cryst_datoms.push(Datom::new(e, a, v, cryst_tx, Op::Assert));
        }
    }

    let concept_tx_file = TxFile {
        tx_id: cryst_tx,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: format!(
            "recrystallize: {} concepts from {} observations, {total_memberships} memberships",
            new_concepts.len(),
            total_obs,
        ),
        causal_predecessors: vec![],
        datoms: cryst_datoms,
    };
    write_tx_to_disk(&braid_path, &concept_tx_file);
    eprintln!(
        "wrote crystallization transaction ({} concepts, {total_memberships} memberships)",
        new_concepts.len()
    );

    // Invalidate the store.bin cache so next `braid` invocation rebuilds.
    let cache_path = braid_path.join(".cache").join("store.bin");
    if cache_path.exists() {
        let _ = std::fs::remove_file(&cache_path);
        eprintln!("invalidated store.bin cache");
    }

    // 7. Report summary.
    eprintln!("\n=== Recrystallization Complete ===");
    eprintln!("  embedder: {embedder_type}");
    eprintln!("  observations re-embedded: {total_obs}");
    eprintln!("  concepts created: {}", new_concepts.len());
    eprintln!("  memberships assigned: {total_memberships}");
    eprintln!(
        "  previously uncategorized: {uncategorized_count} -> verify with `braid observe list`"
    );
}

/// Load a Store by reading all transaction files from `.braid/txns/`.
fn load_store_from_txns(braid_path: &std::path::Path) -> Store {
    let txns_dir = braid_path.join("txns");
    let mut all_datoms: BTreeSet<Datom> = BTreeSet::new();

    // Walk shard directories (00..ff) and read all .edn files.
    let mut entries: Vec<_> = std::fs::read_dir(&txns_dir)
        .expect("failed to read txns directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut file_count = 0usize;
    for shard_entry in &entries {
        let shard_dir = shard_entry.path();
        let files: Vec<_> = std::fs::read_dir(&shard_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "edn")
                    .unwrap_or(false)
            })
            .collect();

        for file_entry in &files {
            let path = file_entry.path();
            match std::fs::read(&path) {
                Ok(bytes) => match braid_kernel::layout::deserialize_tx(&bytes) {
                    Ok(tx) => {
                        for d in &tx.datoms {
                            all_datoms.insert(d.clone());
                        }
                        file_count += 1;
                    }
                    Err(e) => {
                        eprintln!("warning: failed to parse {}: {e}", path.display());
                    }
                },
                Err(e) => {
                    eprintln!("warning: failed to read {}: {e}", path.display());
                }
            }
        }
    }

    eprintln!("loaded {file_count} transaction files");
    Store::from_datoms(all_datoms)
}

/// Write a TxFile to disk in the `.braid/txns/` directory.
fn write_tx_to_disk(braid_path: &std::path::Path, tx: &TxFile) {
    let bytes = serialize_tx(tx);
    let hash = ContentHash::of(&bytes);
    let file_path = TxFilePath::from_hash(&hash);

    let shard_dir = braid_path.join("txns").join(&file_path.shard);
    let _ = std::fs::create_dir_all(&shard_dir);

    let full_path = shard_dir.join(&file_path.filename);

    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&full_path)
    {
        Ok(mut file) => {
            use std::io::Write;
            if let Err(e) = file.write_all(&bytes) {
                eprintln!("error: failed to write {}: {e}", full_path.display());
                std::process::exit(1);
            }
            // fsync for crash safety.
            let _ = file.sync_all();
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            // Idempotent: same content hash means same transaction already exists.
            eprintln!(
                "note: tx file already exists (idempotent): {}",
                file_path.filename
            );
        }
        Err(e) => {
            eprintln!("error: failed to create {}: {e}", full_path.display());
            std::process::exit(1);
        }
    }
}

/// Generate a TxId that advances past the store's current frontier.
fn next_tx_id(store: &Store, agent: AgentId) -> TxId {
    let current_wall = store
        .frontier()
        .values()
        .map(|tx| tx.wall_time())
        .max()
        .unwrap_or(0);
    let unix_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    TxId::new(current_wall.max(unix_now) + 1, 0, agent)
}

/// Resolve the best available TextEmbedder for the given store path.
fn resolve_embedder(store_path: &std::path::Path) -> (Box<dyn TextEmbedder>, &'static str) {
    let model_name = "potion-base-8M";
    let model_file = "model.safetensors";
    let tokenizer_file = "tokenizer.json";

    let candidates = [
        store_path.join("models").join(model_name),
        dirs::home_dir()
            .map(|h| h.join(".braid").join("models").join(model_name))
            .unwrap_or_default(),
    ];

    for dir in &candidates {
        let model_path = dir.join(model_file);
        let tokenizer_path = dir.join(tokenizer_file);

        if model_path.is_file() && tokenizer_path.is_file() {
            #[cfg(feature = "embeddings")]
            {
                match (std::fs::read(&model_path), std::fs::read(&tokenizer_path)) {
                    (Ok(model_bytes), Ok(tokenizer_bytes)) => {
                        match braid_kernel::embedding::Embedder::from_bytes(
                            &model_bytes,
                            &tokenizer_bytes,
                        ) {
                            Ok(embedder) => return (Box::new(embedder), "model2vec"),
                            Err(e) => {
                                eprintln!(
                                    "warning: model load failed: {e}, falling back to hash embedder"
                                );
                            }
                        }
                    }
                    _ => {
                        eprintln!(
                            "warning: could not read model files, falling back to hash embedder"
                        );
                    }
                }
            }

            #[cfg(not(feature = "embeddings"))]
            {
                let _ = dir; // Suppress unused warning.
                eprintln!(
                    "warning: model found but 'embeddings' feature not enabled, using hash embedder"
                );
            }
        }
    }

    // Fallback: HashEmbedder.
    let hash = braid_kernel::embedding::HashEmbedder::new(braid_kernel::embedding::DEFAULT_DIM);
    (Box::new(hash), "hash")
}
