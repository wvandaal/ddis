//! One-off concept splitting for overgrown concept clusters.
//!
//! Opens the braid store from `.braid/txns/`, identifies concepts whose variance
//! exceeds the split threshold, and uses Fiedler bisection to split them into
//! coherent sub-concepts.
//!
//! Usage: `split_concepts [--dry-run] [--path .braid]`
//!
//! This binary is additive-only: it creates new concept datoms and re-assigns
//! membership via append-only transactions. Old membership datoms are retracted
//! (`:exploration/concept` is Cardinality::Many) and new ones asserted for the
//! sub-concepts. The parent concept entity is left intact (C1: append-only).
//!
//! Uses only `braid_kernel` APIs (no dependency on `braid` library target).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use braid_kernel::concept::{
    concept_inventory, concept_to_datoms, membership_datoms, split_concept, SPLIT_THRESHOLD,
};
use braid_kernel::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
use braid_kernel::embedding::bytes_to_embedding;
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

    // 1. Load the store.
    eprintln!("loading store from {}...", braid_path.display());
    let store = load_store_from_txns(&braid_path);
    let datom_count = store.datoms().count();
    eprintln!("store: {datom_count} datoms");

    // 2. Read split parameters from config with fallback to defaults (C9).
    let split_threshold: f64 = braid_kernel::config::get_config(&store, "concept.split-threshold")
        .and_then(|v| v.parse().ok())
        .unwrap_or(SPLIT_THRESHOLD);

    let min_split_size: usize = braid_kernel::config::get_config(&store, "concept.min-split-size")
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);

    eprintln!("parameters: split_threshold={split_threshold:.3}, min_split_size={min_split_size}");

    // 3. Collect concept inventory and identify split candidates.
    let concepts = concept_inventory(&store);
    eprintln!("concepts in store: {}", concepts.len());

    let candidates: Vec<_> = concepts
        .iter()
        .filter(|c| c.variance > split_threshold && c.member_count > min_split_size * 2)
        .collect();

    if candidates.is_empty() {
        eprintln!("no concepts exceed split threshold -- nothing to do");
        return;
    }

    eprintln!("\nsplit candidates ({}):", candidates.len());
    for c in &candidates {
        eprintln!(
            "  {} (members={}, variance={:.4})",
            c.name, c.member_count, c.variance
        );
    }

    // 4. Collect observation data: body text, embeddings, concept membership.
    let body_attr = Attribute::from_keyword(":exploration/body");
    let embed_attr = Attribute::from_keyword(":exploration/embedding");
    let concept_attr = Attribute::from_keyword(":exploration/concept");

    // Build entity->body map.
    let mut entity_body: BTreeMap<EntityId, String> = BTreeMap::new();
    for d in store.datoms() {
        if d.op == Op::Assert && d.attribute == body_attr {
            if let Value::String(ref s) = d.value {
                entity_body.insert(d.entity, s.clone());
            }
        }
    }

    // Build entity->embedding map.
    let mut entity_embedding: BTreeMap<EntityId, Vec<f32>> = BTreeMap::new();
    for d in store.datoms() {
        if d.op == Op::Assert && d.attribute == embed_attr {
            if let Value::Bytes(ref b) = d.value {
                entity_embedding.insert(d.entity, bytes_to_embedding(b));
            }
        }
    }

    // Build entity->concept map (latest assertion per entity by tx ordering).
    // Since Cardinality::Many, there may be multiple concept refs per entity.
    // We track all of them with their tx for retraction.
    let mut entity_concept_refs: BTreeMap<EntityId, Vec<(EntityId, TxId)>> = BTreeMap::new();
    for d in store.datoms() {
        if d.op == Op::Assert && d.attribute == concept_attr {
            if let Value::Ref(concept_e) = d.value {
                entity_concept_refs
                    .entry(d.entity)
                    .or_default()
                    .push((concept_e, d.tx));
            }
        }
    }

    // Build concept->members map (latest concept ref per entity).
    let mut concept_members: BTreeMap<EntityId, Vec<EntityId>> = BTreeMap::new();
    for (entity, refs) in &entity_concept_refs {
        // Take the latest concept ref by tx time.
        if let Some((concept_e, _)) = refs.iter().max_by_key(|(_, tx)| tx.wall_time()) {
            concept_members.entry(*concept_e).or_default().push(*entity);
        }
    }

    // Corpus texts for TF-IDF naming.
    let corpus_texts: Vec<&str> = entity_body.values().map(|s| s.as_str()).collect();

    // 5. Process each candidate concept.
    let agent = AgentId::from_name("braid:split");
    let mut total_concepts_split = 0usize;
    let mut total_subconcepts_created = 0usize;
    let mut total_memberships_reassigned = 0usize;
    let mut all_split_datoms: Vec<Datom> = Vec::new();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let tx = next_tx_id(&store, agent);

    for candidate in &candidates {
        let members = match concept_members.get(&candidate.entity) {
            Some(m) => m,
            None => {
                eprintln!("  skipping {} -- no members found in store", candidate.name);
                continue;
            }
        };

        // Collect member embeddings (entity, embedding, body_text).
        let mut member_data: Vec<(EntityId, Vec<f32>, String)> = Vec::new();
        for &member_entity in members {
            let embedding = match entity_embedding.get(&member_entity) {
                Some(e) => e.clone(),
                None => continue, // Skip members without embeddings.
            };
            let body = match entity_body.get(&member_entity) {
                Some(b) => b.clone(),
                None => continue, // Skip members without body text.
            };
            member_data.push((member_entity, embedding, body));
        }

        if member_data.len() < min_split_size * 2 {
            eprintln!(
                "  skipping {} -- only {} members with embeddings (need {})",
                candidate.name,
                member_data.len(),
                min_split_size * 2
            );
            continue;
        }

        // Recompute variance from actual embeddings for diagnostic.
        let diag_embeddings: Vec<&[f32]> =
            member_data.iter().map(|(_, e, _)| e.as_slice()).collect();
        let diag_centroid = braid_kernel::embedding::centroid(&diag_embeddings);
        let recomputed_var =
            braid_kernel::embedding::variance(&diag_embeddings, &diag_centroid) as f64;

        eprintln!(
            "\nsplitting '{}' ({} members with embeddings, stored_variance={:.4}, recomputed_variance={:.4})...",
            candidate.name,
            member_data.len(),
            candidate.variance,
            recomputed_var,
        );

        let sub_concepts =
            split_concept(&member_data, &corpus_texts, split_threshold, min_split_size);

        if sub_concepts.is_empty() {
            eprintln!("  no split produced (recomputed variance {:.4} <= threshold {:.3}, or bisection yielded single group)",
                recomputed_var, split_threshold);
            continue;
        }

        eprintln!("  -> {} sub-concepts:", sub_concepts.len());
        for sc in &sub_concepts {
            eprintln!(
                "     {} ({} members, variance={:.4})",
                sc.name,
                sc.members.len(),
                sc.variance
            );
        }

        // Generate datoms for sub-concepts.
        for sc in &sub_concepts {
            // Concept entity datoms.
            for (e, a, v) in concept_to_datoms(sc, now) {
                all_split_datoms.push(Datom::new(e, a, v, tx, Op::Assert));
            }

            // New membership datoms: assert :exploration/concept -> sub-concept.
            for (e, a, v) in membership_datoms(sc.entity, &sc.members) {
                all_split_datoms.push(Datom::new(e, a, v, tx, Op::Assert));
            }

            // Retract old membership datoms: retract :exploration/concept -> parent concept.
            // Since :exploration/concept is Cardinality::Many, we retract the specific
            // old Ref value so the member is no longer counted under the parent.
            for &member_entity in &sc.members {
                all_split_datoms.push(Datom::new(
                    member_entity,
                    concept_attr.clone(),
                    Value::Ref(candidate.entity),
                    tx,
                    Op::Retract,
                ));
            }

            total_memberships_reassigned += sc.members.len();
        }

        total_concepts_split += 1;
        total_subconcepts_created += sub_concepts.len();
    }

    if total_concepts_split == 0 {
        eprintln!("\nno concepts were split -- nothing to write");
        return;
    }

    // 6. Write transaction.
    eprintln!("\n=== Summary ===");
    eprintln!("  concepts split: {total_concepts_split}");
    eprintln!("  sub-concepts created: {total_subconcepts_created}");
    eprintln!("  memberships reassigned: {total_memberships_reassigned}");
    eprintln!("  datoms to write: {}", all_split_datoms.len());

    if dry_run {
        eprintln!(
            "\n[dry-run] would write {} datoms ({total_subconcepts_created} sub-concepts, {total_memberships_reassigned} memberships)",
            all_split_datoms.len()
        );
        return;
    }

    let tx_file = TxFile {
        tx_id: tx,
        agent,
        provenance: ProvenanceType::Derived,
        rationale: format!(
            "split_concepts: split {total_concepts_split} concepts into {total_subconcepts_created} sub-concepts, {total_memberships_reassigned} memberships reassigned"
        ),
        causal_predecessors: vec![],
        datoms: all_split_datoms,
    };
    write_tx_to_disk(&braid_path, &tx_file);
    eprintln!("wrote transaction to store");

    // Invalidate cache.
    let cache_path = braid_path.join(".cache").join("store.bin");
    if cache_path.exists() {
        let _ = std::fs::remove_file(&cache_path);
        eprintln!("invalidated store.bin cache");
    }

    eprintln!("\n=== Split Complete ===");
    eprintln!("  concepts split: {total_concepts_split}");
    eprintln!("  sub-concepts created: {total_subconcepts_created}");
    eprintln!("  memberships reassigned: {total_memberships_reassigned}");
    eprintln!("  verify with: braid observe list");
}

/// Load a Store by reading all transaction files from `.braid/txns/`.
fn load_store_from_txns(braid_path: &std::path::Path) -> Store {
    let txns_dir = braid_path.join("txns");
    let mut all_datoms: BTreeSet<Datom> = BTreeSet::new();

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
            let _ = file.sync_all();
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
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
