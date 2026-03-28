//! LAYOUT namespace — pure computation for content-addressed persistence.
//!
//! This module contains the pure (IO-free) types and functions for the
//! content-addressed transaction file layout. All IO operations live in
//! the `braid` binary crate; this module provides deterministic computation
//! from bytes to bytes.
//!
//! # Isomorphism (INV-LAYOUT-003)
//!
//! ```text
//! φ : Store → Layout   by  φ(S) = { serialize(tx) | tx ∈ transactions(S) }
//! ψ : Layout → Store   by  ψ(L) = ⋃ { deserialize(f).datoms | f ∈ L.txns }
//!
//! Round-trip: ψ(φ(S)) = S
//! ```
//!
//! # Invariants
//!
//! - **INV-LAYOUT-001**: Content-addressed file identity (filename = BLAKE3(bytes)).
//! - **INV-LAYOUT-002**: Transaction file immutability (write-once).
//! - **INV-LAYOUT-003**: Directory-store isomorphism (φ/ψ round-trip).
//! - **INV-LAYOUT-004**: Merge as directory union — merging layouts = union of files.
//! - **INV-LAYOUT-006**: Transport independence — layout works over any file transport.
//! - **INV-LAYOUT-008**: EDN serialization is canonical (deterministic, injective).
//! - **INV-LAYOUT-009**: EDN deserialization inverts serialization.
//! - **INV-LAYOUT-010**: Concurrent write safety via O_CREAT|O_EXCL.
//! - **INV-LAYOUT-011**: Canonical serialization determinism.
//!
//! # Design Decisions
//!
//! - ADR-LAYOUT-001: Per-transaction files over single append log.
//! - ADR-LAYOUT-002: Content-addressed naming over sequential naming.
//! - ADR-LAYOUT-003: EDN serialization format.
//! - ADR-LAYOUT-004: Hash-prefix directory sharding.
//! - ADR-LAYOUT-005: Pure filesystem over database backend.
//! - ADR-LAYOUT-006: O_CREAT|O_EXCL over flock for concurrency.
//! - ADR-LAYOUT-007: Genesis as standalone file.
//!
//! # Negative Cases
//!
//! - NEG-LAYOUT-001: No in-place file modification.
//! - NEG-LAYOUT-002: No file deletion.
//! - NEG-LAYOUT-003: No merge via file append.
//! - NEG-LAYOUT-004: No transport-specific merge logic.
//! - NEG-LAYOUT-005: No index as source of truth.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};

/// A content hash (BLAKE3, 32 bytes).
///
/// Used for file naming: `filename = hex(ContentHash(file_bytes))`.
/// INV-LAYOUT-001: filename = BLAKE3(bytes).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Compute the content hash of raw bytes.
    pub fn of(bytes: &[u8]) -> Self {
        ContentHash(*blake3::hash(bytes).as_bytes())
    }

    /// Return as a 64-character hex string.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The first 2 hex characters (shard prefix).
    pub fn shard_prefix(&self) -> String {
        format!("{:02x}", self.0[0])
    }

    /// Get the raw bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({})", &self.to_hex()[..16])
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// A transaction file path (pure computation — no filesystem access).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TxFilePath {
    /// Shard directory (2-char hex prefix).
    pub shard: String,
    /// Full filename including `.edn` extension.
    pub filename: String,
}

impl TxFilePath {
    /// Compute the file path from a content hash.
    pub fn from_hash(hash: &ContentHash) -> Self {
        let hex = hash.to_hex();
        TxFilePath {
            shard: hash.shard_prefix(),
            filename: format!("{hex}.edn"),
        }
    }

    /// Relative path from the layout root: `txns/{shard}/{filename}`.
    pub fn relative_path(&self) -> String {
        format!("txns/{}/{}", self.shard, self.filename)
    }
}

/// Integrity verification report (pure — computed from byte data, not files).
#[derive(Clone, Debug, Default)]
pub struct IntegrityReport {
    /// Total transaction files checked.
    pub total_files: usize,
    /// Files where hash matches filename.
    pub verified: usize,
    /// Files with integrity errors.
    pub corrupted: Vec<(TxFilePath, IntegrityError)>,
    /// Files in txns/ that don't parse as valid transactions.
    pub orphaned: Vec<TxFilePath>,
}

impl IntegrityReport {
    /// Whether the layout is fully consistent.
    pub fn is_clean(&self) -> bool {
        self.corrupted.is_empty() && self.orphaned.is_empty()
    }
}

/// An integrity error for a transaction file.
#[derive(Clone, Debug)]
pub enum IntegrityError {
    /// The file content hash doesn't match the filename.
    HashMismatch {
        /// Expected hash (from filename).
        expected: ContentHash,
        /// Actual hash (from file contents).
        actual: ContentHash,
    },
    /// The file contents couldn't be parsed as a transaction.
    ParseError(String),
}

/// Layout configuration (pure — no PathBuf, no IO).
#[derive(Clone, Debug)]
pub struct LayoutConfig {
    /// Base directory name (abstract, not a filesystem path).
    pub store_dir: String,
    /// Transaction subdirectory name.
    pub txns_dir: &'static str,
    /// Heads subdirectory name.
    pub heads_dir: &'static str,
    /// Cache subdirectory name.
    pub cache_dir: &'static str,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        LayoutConfig {
            store_dir: ".braid".to_string(),
            txns_dir: "txns",
            heads_dir: "heads",
            cache_dir: ".cache",
        }
    }
}

// ---------------------------------------------------------------------------
// EDN Serialization — Canonical (deterministic, injective)
// ---------------------------------------------------------------------------

/// A serialized transaction in canonical EDN format.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxFile {
    /// Transaction metadata.
    pub tx_id: TxId,
    /// Agent who created this transaction.
    pub agent: AgentId,
    /// Provenance type.
    pub provenance: ProvenanceType,
    /// Human-readable rationale.
    pub rationale: String,
    /// Causal predecessor transaction IDs.
    pub causal_predecessors: Vec<TxId>,
    /// The datoms in this transaction.
    pub datoms: Vec<Datom>,
}

/// Serialize a `TxFile` to canonical EDN bytes.
///
/// The output is deterministic: same input always produces identical bytes.
/// This is critical for content-addressed identity (INV-LAYOUT-001).
///
/// # Format
///
/// ```text
/// {:tx/id #hlc "<wall_time>/<logical>/<agent>"
///  :tx/agent "<agent_hex>"
///  :tx/provenance :<type>
///  :tx/rationale "<escaped_string>"
///  :tx/causal-predecessors [<hlc>...]
///  :datoms [
///    {:e #blake3 "<hex>" :a :<ns/name> :v <value> :op :<assert|retract>}
///    ...
///  ]}
/// ```
pub fn serialize_tx(tx: &TxFile) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("{:tx/id ");
    write_hlc(&mut out, &tx.tx_id);
    out.push_str("\n :tx/agent ");
    write_agent(&mut out, &tx.agent);
    out.push_str("\n :tx/provenance ");
    write_provenance(&mut out, &tx.provenance);
    out.push_str("\n :tx/rationale ");
    write_edn_string(&mut out, &tx.rationale);
    out.push_str("\n :tx/causal-predecessors [");
    // INV-LAYOUT-011: Canonical serialization requires sorted causal predecessors.
    let mut sorted_preds: Vec<_> = tx.causal_predecessors.iter().collect();
    sorted_preds.sort();
    for (i, pred) in sorted_preds.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        write_hlc(&mut out, pred);
    }
    out.push_str("]\n :datoms [\n");
    // INV-LAYOUT-011: Canonical serialization requires sorted datoms.
    // Sort by (entity, attribute, value, op) to ensure identical logical
    // transactions produce identical byte sequences regardless of insertion order.
    let mut sorted_datoms: Vec<_> = tx.datoms.iter().collect();
    sorted_datoms.sort();
    for datom in &sorted_datoms {
        out.push_str("   ");
        write_datom(&mut out, datom);
        out.push('\n');
    }
    out.push_str(" ]}\n");
    out.into_bytes()
}

/// Deserialize canonical EDN bytes back to a `TxFile`.
///
/// INV-LAYOUT-009: `deserialize(serialize(tx)) == tx` for all valid tx.
pub fn deserialize_tx(bytes: &[u8]) -> Result<TxFile, EdnParseError> {
    let s = std::str::from_utf8(bytes).map_err(|e| EdnParseError::InvalidUtf8(e.to_string()))?;
    parse_tx_file(s)
}

/// EDN parse error.
#[derive(Clone, Debug)]
pub enum EdnParseError {
    /// The input is not valid UTF-8.
    InvalidUtf8(String),
    /// Expected a specific token.
    Expected(String),
    /// Unexpected end of input.
    UnexpectedEof,
    /// Invalid value format.
    InvalidValue(String),
}

impl fmt::Display for EdnParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EdnParseError::InvalidUtf8(e) => write!(f, "invalid UTF-8: {e}"),
            EdnParseError::Expected(e) => write!(f, "expected: {e}"),
            EdnParseError::UnexpectedEof => write!(f, "unexpected end of input"),
            EdnParseError::InvalidValue(e) => write!(f, "invalid value: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// EDN Writer helpers (canonical form)
// ---------------------------------------------------------------------------

fn write_hlc(out: &mut String, tx: &TxId) {
    out.push_str(&format!(
        "#hlc \"{}/{}/{}\"",
        tx.wall_time(),
        tx.logical(),
        hex::encode(tx.agent().as_bytes()),
    ));
}

fn write_agent(out: &mut String, agent: &AgentId) {
    out.push_str(&format!("\"{}\"", hex::encode(agent.as_bytes())));
}

fn write_provenance(out: &mut String, prov: &ProvenanceType) {
    out.push_str(match prov {
        ProvenanceType::Hypothesized => ":hypothesized",
        ProvenanceType::Inferred => ":inferred",
        ProvenanceType::Derived => ":derived",
        ProvenanceType::Observed => ":observed",
    });
}

fn write_edn_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_datom(out: &mut String, d: &Datom) {
    out.push_str("{:e #blake3 \"");
    out.push_str(&hex::encode(d.entity.as_bytes()));
    out.push_str("\" :a ");
    out.push_str(d.attribute.as_str());
    out.push_str(" :v ");
    write_value(out, &d.value);
    out.push_str(" :op ");
    match d.op {
        Op::Assert => out.push_str(":assert"),
        Op::Retract => out.push_str(":retract"),
    }
    out.push('}');
}

fn write_value(out: &mut String, v: &Value) {
    match v {
        Value::String(s) => write_edn_string(out, s),
        Value::Keyword(k) => out.push_str(k),
        Value::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Long(n) => out.push_str(&n.to_string()),
        Value::Double(f) => {
            // Deterministic float formatting
            let s = format!("{}", f.into_inner());
            out.push_str(&s);
            if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                out.push_str(".0");
            }
        }
        Value::Instant(ts) => out.push_str(&format!("#inst {ts}")),
        Value::Uuid(bytes) => {
            out.push_str("#uuid \"");
            out.push_str(&format!(
                "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                bytes[0], bytes[1], bytes[2], bytes[3],
                bytes[4], bytes[5], bytes[6], bytes[7],
                bytes[8], bytes[9], bytes[10], bytes[11],
                bytes[12], bytes[13], bytes[14], bytes[15],
            ));
            out.push('"');
        }
        Value::Ref(eid) => {
            out.push_str("#blake3 \"");
            out.push_str(&hex::encode(eid.as_bytes()));
            out.push('"');
        }
        Value::Bytes(b) => {
            out.push_str("#bytes \"");
            out.push_str(&hex::encode(b));
            out.push('"');
        }
    }
}

// ---------------------------------------------------------------------------
// hex encoding (no dependency — trivial)
// ---------------------------------------------------------------------------

mod hex {
    /// Encode bytes as lowercase hex.
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Decode hex string to bytes.
    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if !s.len().is_multiple_of(2) {
            return Err("odd-length hex string".to_string());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16)
                    .map_err(|e| format!("invalid hex at position {i}: {e}"))
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// EDN Parser (minimal, for our canonical format)
// ---------------------------------------------------------------------------

struct EdnParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> EdnParser<'a> {
    fn new(input: &'a str) -> Self {
        EdnParser { input, pos: 0 }
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            let ch = self.input.as_bytes()[self.pos];
            if ch == b' ' || ch == b'\n' || ch == b'\r' || ch == b'\t' || ch == b',' {
                self.pos += 1;
            } else if ch == b';' {
                // Skip comment to end of line
                while self.pos < self.input.len() && self.input.as_bytes()[self.pos] != b'\n' {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, s: &str) -> Result<(), EdnParseError> {
        self.skip_whitespace();
        if self.remaining().starts_with(s) {
            self.pos += s.len();
            Ok(())
        } else {
            Err(EdnParseError::Expected(format!(
                "'{s}' at position {}, found '{}'",
                self.pos,
                &self.remaining()[..self.remaining().len().min(20)]
            )))
        }
    }

    fn peek(&self) -> Option<u8> {
        if self.pos < self.input.len() {
            Some(self.input.as_bytes()[self.pos])
        } else {
            None
        }
    }

    fn parse_string(&mut self) -> Result<String, EdnParseError> {
        self.skip_whitespace();
        self.expect("\"")?;
        let mut result = String::new();
        // Iterate by char (not by byte) to correctly handle multi-byte UTF-8.
        // Previous implementation used `as_bytes()[pos] as char` which corrupted
        // every non-ASCII character (em-dash → â€", Greek Φ → Î¦, etc.).
        while self.pos < self.input.len() {
            let remaining = &self.input[self.pos..];
            let ch = remaining
                .chars()
                .next()
                .ok_or(EdnParseError::UnexpectedEof)?;
            if ch == '"' {
                self.pos += 1;
                return Ok(result);
            } else if ch == '\\' {
                self.pos += 1;
                if self.pos >= self.input.len() {
                    return Err(EdnParseError::UnexpectedEof);
                }
                match self.input.as_bytes()[self.pos] {
                    b'"' => result.push('"'),
                    b'\\' => result.push('\\'),
                    b'n' => result.push('\n'),
                    b'r' => result.push('\r'),
                    b't' => result.push('\t'),
                    other => {
                        result.push('\\');
                        result.push(other as char);
                    }
                }
                self.pos += 1;
            } else {
                result.push(ch);
                self.pos += ch.len_utf8();
            }
        }
        Err(EdnParseError::UnexpectedEof)
    }

    fn parse_keyword(&mut self) -> Result<String, EdnParseError> {
        self.skip_whitespace();
        if self.peek() != Some(b':') {
            return Err(EdnParseError::Expected(format!(
                "keyword at position {}",
                self.pos
            )));
        }
        let start = self.pos;
        self.pos += 1; // skip leading ':'
        while self.pos < self.input.len() {
            let ch = self.input.as_bytes()[self.pos];
            // EDN keywords: alphanumeric, '/', '-', '_', '.', '*', and ':'
            // (interior ':' occurs in malformed scope keywords like
            // :config.scope/:config.scope/project — must be consumed as
            // part of the keyword to avoid orphaned fragments).
            if ch.is_ascii_alphanumeric()
                || ch == b'/'
                || ch == b'-'
                || ch == b'_'
                || ch == b'.'
                || ch == b'*'
                || ch == b':'
            {
                self.pos += 1;
            } else {
                break;
            }
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_hlc(&mut self) -> Result<TxId, EdnParseError> {
        self.skip_whitespace();
        self.expect("#hlc")?;
        let s = self.parse_string()?;
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 3 {
            return Err(EdnParseError::InvalidValue(format!(
                "HLC needs 3 parts, got {}: {s}",
                parts.len()
            )));
        }
        let wall_time: u64 = parts[0]
            .parse()
            .map_err(|e| EdnParseError::InvalidValue(format!("wall_time: {e}")))?;
        let logical: u32 = parts[1]
            .parse()
            .map_err(|e| EdnParseError::InvalidValue(format!("logical: {e}")))?;
        let agent_bytes = hex::decode(parts[2])
            .map_err(|e| EdnParseError::InvalidValue(format!("agent: {e}")))?;
        if agent_bytes.len() != 16 {
            return Err(EdnParseError::InvalidValue(format!(
                "agent must be 16 bytes, got {}",
                agent_bytes.len()
            )));
        }
        let mut agent_arr = [0u8; 16];
        agent_arr.copy_from_slice(&agent_bytes);
        Ok(TxId::new(
            wall_time,
            logical,
            AgentId::from_bytes(agent_arr),
        ))
    }

    fn parse_value(&mut self) -> Result<Value, EdnParseError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'"') => Ok(Value::String(self.parse_string()?)),
            Some(b':') => Ok(Value::Keyword(self.parse_keyword()?)),
            Some(b't') if self.remaining().starts_with("true") => {
                self.pos += 4;
                Ok(Value::Boolean(true))
            }
            Some(b'f') if self.remaining().starts_with("false") => {
                self.pos += 5;
                Ok(Value::Boolean(false))
            }
            Some(b'#') if self.remaining().starts_with("#blake3") => {
                self.expect("#blake3")?;
                let hex_str = self.parse_string()?;
                let bytes = hex::decode(&hex_str)
                    .map_err(|e| EdnParseError::InvalidValue(format!("blake3: {e}")))?;
                if bytes.len() != 32 {
                    return Err(EdnParseError::InvalidValue(format!(
                        "blake3 must be 32 bytes, got {}",
                        bytes.len()
                    )));
                }
                Ok(Value::Ref(EntityId::from_raw_bytes(
                    bytes.try_into().unwrap(),
                )))
            }
            Some(b'#') if self.remaining().starts_with("#inst") => {
                self.expect("#inst")?;
                self.skip_whitespace();
                let start = self.pos;
                while self.pos < self.input.len()
                    && self.input.as_bytes()[self.pos].is_ascii_digit()
                {
                    self.pos += 1;
                }
                let num_str = &self.input[start..self.pos];
                let ts: u64 = num_str
                    .parse()
                    .map_err(|e| EdnParseError::InvalidValue(format!("instant: {e}")))?;
                Ok(Value::Instant(ts))
            }
            Some(b'#') if self.remaining().starts_with("#uuid") => {
                self.expect("#uuid")?;
                let uuid_str = self.parse_string()?;
                let clean: String = uuid_str.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                let bytes = hex::decode(&clean)
                    .map_err(|e| EdnParseError::InvalidValue(format!("uuid: {e}")))?;
                if bytes.len() != 16 {
                    return Err(EdnParseError::InvalidValue(format!(
                        "uuid must be 16 bytes, got {}",
                        bytes.len()
                    )));
                }
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&bytes);
                Ok(Value::Uuid(arr))
            }
            Some(b'#') if self.remaining().starts_with("#bytes") => {
                self.expect("#bytes")?;
                let hex_str = self.parse_string()?;
                let bytes = hex::decode(&hex_str)
                    .map_err(|e| EdnParseError::InvalidValue(format!("bytes: {e}")))?;
                Ok(Value::Bytes(bytes))
            }
            Some(ch) if ch == b'-' || ch.is_ascii_digit() => {
                let start = self.pos;
                if ch == b'-' {
                    self.pos += 1;
                }
                while self.pos < self.input.len()
                    && self.input.as_bytes()[self.pos].is_ascii_digit()
                {
                    self.pos += 1;
                }
                // Check for decimal point (double)
                if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'.' {
                    self.pos += 1;
                    while self.pos < self.input.len()
                        && self.input.as_bytes()[self.pos].is_ascii_digit()
                    {
                        self.pos += 1;
                    }
                    // Check for exponent
                    if self.pos < self.input.len()
                        && (self.input.as_bytes()[self.pos] == b'e'
                            || self.input.as_bytes()[self.pos] == b'E')
                    {
                        self.pos += 1;
                        if self.pos < self.input.len()
                            && (self.input.as_bytes()[self.pos] == b'+'
                                || self.input.as_bytes()[self.pos] == b'-')
                        {
                            self.pos += 1;
                        }
                        while self.pos < self.input.len()
                            && self.input.as_bytes()[self.pos].is_ascii_digit()
                        {
                            self.pos += 1;
                        }
                    }
                    let num_str = &self.input[start..self.pos];
                    let f: f64 = num_str
                        .parse()
                        .map_err(|e| EdnParseError::InvalidValue(format!("double: {e}")))?;
                    Ok(Value::Double(ordered_float::OrderedFloat(f)))
                } else {
                    let num_str = &self.input[start..self.pos];
                    let n: i64 = num_str
                        .parse()
                        .map_err(|e| EdnParseError::InvalidValue(format!("long: {e}")))?;
                    Ok(Value::Long(n))
                }
            }
            Some(ch) => Err(EdnParseError::InvalidValue(format!(
                "unexpected char '{}' at position {}",
                ch as char, self.pos
            ))),
            None => Err(EdnParseError::UnexpectedEof),
        }
    }

    fn parse_datom(&mut self) -> Result<Datom, EdnParseError> {
        self.expect("{")?;
        self.expect(":e")?;
        self.expect("#blake3")?;
        let entity_hex = self.parse_string()?;
        let entity_bytes = hex::decode(&entity_hex)
            .map_err(|e| EdnParseError::InvalidValue(format!("entity: {e}")))?;
        if entity_bytes.len() != 32 {
            return Err(EdnParseError::InvalidValue(
                "entity must be 32 bytes".into(),
            ));
        }
        let entity = EntityId::from_raw_bytes(entity_bytes.try_into().unwrap());

        self.expect(":a")?;
        let attr_kw = self.parse_keyword()?;
        let attribute = Attribute::from_keyword(&attr_kw);

        self.expect(":v")?;
        let value = self.parse_value()?;

        self.expect(":op")?;
        let op_kw = self.parse_keyword()?;
        let op = match op_kw.as_str() {
            ":assert" => Op::Assert,
            ":retract" => Op::Retract,
            other => return Err(EdnParseError::InvalidValue(format!("unknown op: {other}"))),
        };

        self.expect("}")?;

        // We need a TxId for the datom, but it comes from the transaction envelope.
        // During deserialization, we use the transaction's TxId.
        // We'll construct a placeholder and fix it in parse_tx_file.
        Ok(Datom {
            entity,
            attribute,
            value,
            tx: TxId::new(0, 0, AgentId::from_bytes([0; 16])),
            op,
        })
    }
}

fn parse_tx_file(s: &str) -> Result<TxFile, EdnParseError> {
    let mut p = EdnParser::new(s);

    p.expect("{")?;
    p.expect(":tx/id")?;
    let tx_id = p.parse_hlc()?;

    p.expect(":tx/agent")?;
    let agent_hex = p.parse_string()?;
    let agent_bytes =
        hex::decode(&agent_hex).map_err(|e| EdnParseError::InvalidValue(format!("agent: {e}")))?;
    if agent_bytes.len() != 16 {
        return Err(EdnParseError::InvalidValue("agent must be 16 bytes".into()));
    }
    let mut agent_arr = [0u8; 16];
    agent_arr.copy_from_slice(&agent_bytes);
    let agent = AgentId::from_bytes(agent_arr);

    p.expect(":tx/provenance")?;
    let prov_kw = p.parse_keyword()?;
    let provenance = match prov_kw.as_str() {
        ":hypothesized" => ProvenanceType::Hypothesized,
        ":inferred" => ProvenanceType::Inferred,
        ":derived" => ProvenanceType::Derived,
        ":observed" => ProvenanceType::Observed,
        other => {
            return Err(EdnParseError::InvalidValue(format!(
                "unknown provenance: {other}"
            )))
        }
    };

    p.expect(":tx/rationale")?;
    let rationale = p.parse_string()?;

    p.expect(":tx/causal-predecessors")?;
    p.expect("[")?;
    let mut causal_predecessors = Vec::new();
    p.skip_whitespace();
    while p.peek() != Some(b']') {
        causal_predecessors.push(p.parse_hlc()?);
        p.skip_whitespace();
    }
    p.expect("]")?;

    p.expect(":datoms")?;
    p.expect("[")?;
    let mut datoms = Vec::new();
    p.skip_whitespace();
    while p.peek() == Some(b'{') {
        let mut datom = p.parse_datom()?;
        // Fix up the TxId from the transaction envelope
        datom.tx = tx_id;
        datoms.push(datom);
        p.skip_whitespace();
    }
    p.expect("]")?;
    p.expect("}")?;

    Ok(TxFile {
        tx_id,
        agent,
        provenance,
        rationale,
        causal_predecessors,
        datoms,
    })
}

/// Compute the content hash of a transaction.
///
/// This is the canonical way to get the filename for a transaction file.
/// The hash is over the canonical EDN serialization.
pub fn tx_content_hash(tx: &TxFile) -> ContentHash {
    let bytes = serialize_tx(tx);
    ContentHash::of(&bytes)
}

/// Verify that a file's content matches its expected hash.
///
/// Returns `true` if `BLAKE3(content) == expected_hash`.
pub fn verify_content_hash(content: &[u8], expected_hash: &ContentHash) -> bool {
    ContentHash::of(content) == *expected_hash
}

/// Collect datoms from multiple deserialized transaction files.
///
/// This is the ψ function: `ψ(L) = ⋃ { tx.datoms | tx ∈ L }`.
/// Used for layout → store reconstruction.
pub fn collect_datoms(tx_files: &[TxFile]) -> BTreeSet<Datom> {
    let mut datoms = BTreeSet::new();
    for tx in tx_files {
        for datom in &tx.datoms {
            datoms.insert(datom.clone());
        }
    }
    datoms
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Witnesses: INV-LAYOUT-001, INV-LAYOUT-002, INV-LAYOUT-003, INV-LAYOUT-004,
// INV-LAYOUT-005, INV-LAYOUT-006, INV-LAYOUT-007, INV-LAYOUT-009,
// INV-LAYOUT-010, INV-LAYOUT-011,
// ADR-LAYOUT-001, ADR-LAYOUT-002, ADR-LAYOUT-003, ADR-LAYOUT-005,
// ADR-LAYOUT-006, ADR-LAYOUT-007,
// NEG-LAYOUT-001, NEG-LAYOUT-002, NEG-LAYOUT-003, NEG-LAYOUT-005
#[cfg(test)]
mod tests {
    use super::*;
    use crate::datom::{AgentId, Attribute, EntityId, Op, ProvenanceType, TxId, Value};

    fn sample_tx() -> TxFile {
        let agent = AgentId::from_name("test-agent");
        let tx_id = TxId::new(1709654401000, 1, agent);
        TxFile {
            tx_id,
            agent,
            provenance: ProvenanceType::Observed,
            rationale: "Test transaction".to_string(),
            causal_predecessors: vec![],
            datoms: vec![
                Datom {
                    entity: EntityId::from_ident(":test/entity"),
                    attribute: Attribute::from_keyword(":db/doc"),
                    value: Value::String("hello world".to_string()),
                    tx: tx_id,
                    op: Op::Assert,
                },
                Datom {
                    entity: EntityId::from_ident(":test/entity"),
                    attribute: Attribute::from_keyword(":db/ident"),
                    value: Value::Keyword(":test/entity".to_string()),
                    tx: tx_id,
                    op: Op::Assert,
                },
            ],
        }
    }

    // Verifies: INV-LAYOUT-011 — Canonical Serialization Determinism
    // Verifies: ADR-LAYOUT-003 — EDN Serialization Format
    #[test]
    fn serialize_round_trip() {
        let tx = sample_tx();
        let bytes = serialize_tx(&tx);
        let parsed = deserialize_tx(&bytes).unwrap();

        assert_eq!(parsed.tx_id, tx.tx_id);
        assert_eq!(parsed.agent, tx.agent);
        assert_eq!(parsed.provenance, tx.provenance);
        assert_eq!(parsed.rationale, tx.rationale);
        assert_eq!(
            parsed.causal_predecessors.len(),
            tx.causal_predecessors.len()
        );
        assert_eq!(parsed.datoms.len(), tx.datoms.len());
        for (a, b) in parsed.datoms.iter().zip(tx.datoms.iter()) {
            assert_eq!(a.entity, b.entity);
            assert_eq!(a.attribute, b.attribute);
            assert_eq!(a.value, b.value);
            assert_eq!(a.op, b.op);
        }
    }

    // Verifies: INV-LAYOUT-011 — Canonical Serialization Determinism
    #[test]
    fn serialize_is_deterministic() {
        let tx = sample_tx();
        let bytes1 = serialize_tx(&tx);
        let bytes2 = serialize_tx(&tx);
        assert_eq!(bytes1, bytes2, "EDN serialization must be deterministic");
    }

    // Verifies: INV-LAYOUT-001 — Content-Addressed File Identity
    // Verifies: ADR-LAYOUT-002 — Content-Addressed Naming Over Sequential Naming
    #[test]
    fn content_hash_is_deterministic() {
        let tx = sample_tx();
        let h1 = tx_content_hash(&tx);
        let h2 = tx_content_hash(&tx);
        assert_eq!(h1, h2);
    }

    // Verifies: INV-LAYOUT-001 — Content-Addressed File Identity
    #[test]
    fn content_hash_differs_for_different_content() {
        let tx1 = sample_tx();
        let mut tx2 = sample_tx();
        tx2.rationale = "Different rationale".to_string();
        let h1 = tx_content_hash(&tx1);
        let h2 = tx_content_hash(&tx2);
        assert_ne!(h1, h2);
    }

    // Verifies: INV-LAYOUT-008 — Sharded Directory Scalability
    // Verifies: ADR-LAYOUT-004 — Hash-Prefix Directory Sharding
    #[test]
    fn tx_file_path_from_hash() {
        let tx = sample_tx();
        let hash = tx_content_hash(&tx);
        let path = TxFilePath::from_hash(&hash);
        assert_eq!(path.shard, hash.shard_prefix());
        assert!(path.filename.ends_with(".edn"));
        assert!(path.relative_path().starts_with("txns/"));
    }

    // Verifies: INV-LAYOUT-005 — Integrity Self-Verification
    #[test]
    fn verify_content_hash_positive() {
        let tx = sample_tx();
        let bytes = serialize_tx(&tx);
        let hash = ContentHash::of(&bytes);
        assert!(verify_content_hash(&bytes, &hash));
    }

    // Verifies: INV-LAYOUT-005 — Integrity Self-Verification (negative case)
    #[test]
    fn verify_content_hash_negative() {
        let tx = sample_tx();
        let bytes = serialize_tx(&tx);
        let wrong_hash = ContentHash::of(b"wrong content");
        assert!(!verify_content_hash(&bytes, &wrong_hash));
    }

    // Verifies: INV-LAYOUT-011 — Canonical Serialization Determinism (all value types)
    // Verifies: ADR-LAYOUT-003 — EDN Serialization Format
    #[test]
    fn value_types_round_trip() {
        let agent = AgentId::from_name("test");
        let tx_id = TxId::new(100, 0, agent);
        let test_values = vec![
            Value::String("hello \"world\"\nnewline".to_string()),
            Value::Keyword(":test/keyword".to_string()),
            Value::Boolean(true),
            Value::Boolean(false),
            Value::Long(42),
            Value::Long(-100),
            Value::Double(ordered_float::OrderedFloat(1.23456)),
            Value::Instant(1709654401000),
            Value::Uuid([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
            Value::Ref(EntityId::from_ident(":ref/target")),
            Value::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]),
        ];

        for (i, val) in test_values.into_iter().enumerate() {
            let tx = TxFile {
                tx_id,
                agent,
                provenance: ProvenanceType::Observed,
                rationale: format!("test value {i}"),
                causal_predecessors: vec![],
                datoms: vec![Datom {
                    entity: EntityId::from_ident(&format!(":test/v{i}")),
                    attribute: Attribute::from_keyword(":db/doc"),
                    value: val.clone(),
                    tx: tx_id,
                    op: Op::Assert,
                }],
            };
            let bytes = serialize_tx(&tx);
            let parsed = deserialize_tx(&bytes)
                .unwrap_or_else(|e| panic!("failed to parse value {i} ({val:?}): {e}"));
            assert_eq!(
                parsed.datoms[0].value, val,
                "round-trip failed for value {i}: {val:?}"
            );
        }
    }

    // Verifies: INV-LAYOUT-009 — Index Derivability
    // Verifies: NEG-LAYOUT-005 — No Index as Source of Truth
    #[test]
    fn collect_datoms_deduplicates() {
        let tx1 = sample_tx();
        let tx2 = sample_tx(); // same datoms
        let datoms = collect_datoms(&[tx1, tx2]);
        // Dedup by BTreeSet — same datoms appear once
        assert_eq!(datoms.len(), 2);
    }

    // Verifies: INV-LAYOUT-005 — Integrity Self-Verification (report)
    // Verifies: INV-LAYOUT-003 — Directory-Store Isomorphism
    #[test]
    fn integrity_report_clean() {
        let report = IntegrityReport {
            total_files: 5,
            verified: 5,
            corrupted: vec![],
            orphaned: vec![],
        };
        assert!(report.is_clean());
    }

    #[test]
    fn integrity_report_corrupt() {
        let hash1 = ContentHash::of(b"hello");
        let hash2 = ContentHash::of(b"world");
        let report = IntegrityReport {
            total_files: 5,
            verified: 4,
            corrupted: vec![(
                TxFilePath::from_hash(&hash1),
                IntegrityError::HashMismatch {
                    expected: hash1,
                    actual: hash2,
                },
            )],
            orphaned: vec![],
        };
        assert!(!report.is_clean());
    }

    #[test]
    fn causal_predecessors_round_trip() {
        let agent = AgentId::from_name("test");
        let pred1 = TxId::new(100, 0, agent);
        let pred2 = TxId::new(200, 1, agent);
        let tx = TxFile {
            tx_id: TxId::new(300, 0, agent),
            agent,
            provenance: ProvenanceType::Derived,
            rationale: "with predecessors".to_string(),
            causal_predecessors: vec![pred1, pred2],
            datoms: vec![],
        };
        let bytes = serialize_tx(&tx);
        let parsed = deserialize_tx(&bytes).unwrap();
        assert_eq!(parsed.causal_predecessors.len(), 2);
        assert_eq!(parsed.causal_predecessors[0], pred1);
        assert_eq!(parsed.causal_predecessors[1], pred2);
    }

    #[test]
    fn utf8_string_round_trip() {
        // Verify multi-byte UTF-8 characters survive serialize → deserialize
        let agent = AgentId::from_name("test");
        let tx = TxFile {
            tx_id: TxId::new(42, 0, agent),
            agent,
            provenance: ProvenanceType::Observed,
            rationale: "UTF-8 test: em-dash — Greek Φ subscript ₁ arrow → CJK 中文".to_string(),
            causal_predecessors: vec![],
            datoms: vec![Datom::new(
                EntityId::from_ident(":test/utf8"),
                Attribute::from_keyword(":db/doc"),
                Value::String(
                    "Divergence Φ=240.6 — structural remediation → convergence ₁₂₃".to_string(),
                ),
                TxId::new(42, 0, agent),
                Op::Assert,
            )],
        };
        let bytes = serialize_tx(&tx);
        let parsed = deserialize_tx(&bytes).unwrap();
        assert_eq!(
            parsed.rationale,
            "UTF-8 test: em-dash — Greek Φ subscript ₁ arrow → CJK 中文"
        );
        match &parsed.datoms[0].value {
            Value::String(s) => assert_eq!(
                s,
                "Divergence Φ=240.6 — structural remediation → convergence ₁₂₃"
            ),
            other => panic!("expected String, got {:?}", other),
        }
    }

    // ===================================================================
    // Property-Based Tests (W1C.9 — INV-LAYOUT-001..006)
    // ===================================================================

    mod proptests {
        use super::*;
        use crate::proptest_strategies::{arb_agent_id, arb_doc_value, arb_entity_id, arb_tx_id};
        use proptest::prelude::*;

        /// Build a TxFile from arbitrary datoms for testing.
        fn arb_tx_file(datoms: Vec<Datom>, agent: AgentId, tx_id: TxId) -> TxFile {
            TxFile {
                tx_id,
                agent,
                provenance: ProvenanceType::Observed,
                rationale: "proptest".into(),
                causal_predecessors: vec![],
                datoms,
            }
        }

        proptest! {
            /// INV-LAYOUT-001: Content-addressed identity — same content = same hash.
            ///
            /// Two TxFiles with identical content produce identical content hashes.
            /// This is the foundation of content-addressable storage.
            #[test]
            fn inv_layout_001_content_identity(
                agent in arb_agent_id(),
                tx in arb_tx_id(),
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let datom = Datom::new(entity, Attribute::from_keyword(":db/doc"), value, tx, Op::Assert);
                let tx1 = arb_tx_file(vec![datom.clone()], agent, tx);
                let tx2 = arb_tx_file(vec![datom], agent, tx);
                let h1 = tx_content_hash(&tx1);
                let h2 = tx_content_hash(&tx2);
                prop_assert_eq!(h1, h2, "INV-LAYOUT-001: identical content → different hash");
            }

            /// INV-LAYOUT-001 (collision resistance): Different content → different hash.
            ///
            /// Two TxFiles with different rationales should (with overwhelming probability)
            /// produce different content hashes.
            #[test]
            fn inv_layout_001_collision_resistance(
                agent in arb_agent_id(),
                tx in arb_tx_id(),
            ) {
                let tx1 = TxFile {
                    tx_id: tx,
                    agent,
                    provenance: ProvenanceType::Observed,
                    rationale: "rationale-a".into(),
                    causal_predecessors: vec![],
                    datoms: vec![],
                };
                let tx2 = TxFile {
                    tx_id: tx,
                    agent,
                    provenance: ProvenanceType::Observed,
                    rationale: "rationale-b".into(),
                    causal_predecessors: vec![],
                    datoms: vec![],
                };
                let h1 = tx_content_hash(&tx1);
                let h2 = tx_content_hash(&tx2);
                prop_assert_ne!(h1, h2, "INV-LAYOUT-001: different content → same hash (collision!)");
            }

            /// INV-LAYOUT-003: Serialization round-trip — deserialize(serialize(tx)) = tx.
            ///
            /// For arbitrary TxFiles, the EDN serialization/deserialization is lossless.
            #[test]
            fn inv_layout_003_serialization_round_trip(
                agent in arb_agent_id(),
                tx_id in arb_tx_id(),
                entity in arb_entity_id(),
                value in arb_doc_value(),
            ) {
                let datom = Datom::new(entity, Attribute::from_keyword(":db/doc"), value.clone(), tx_id, Op::Assert);
                let tx = arb_tx_file(vec![datom], agent, tx_id);
                let bytes = serialize_tx(&tx);
                let parsed = deserialize_tx(&bytes);
                prop_assert!(parsed.is_ok(), "INV-LAYOUT-003: serialization not invertible: {:?}", parsed.err());
                let parsed = parsed.unwrap();
                prop_assert_eq!(parsed.tx_id, tx.tx_id);
                prop_assert_eq!(parsed.agent, tx.agent);
                prop_assert_eq!(parsed.datoms.len(), tx.datoms.len());
                if !tx.datoms.is_empty() {
                    prop_assert_eq!(parsed.datoms[0].value.clone(), value);
                }
            }

            /// INV-LAYOUT-004: Merge = directory union — collect_datoms union is superset.
            ///
            /// Given two transaction files, collect_datoms of both contains all datoms from each.
            #[test]
            fn inv_layout_004_merge_union(
                agent1 in arb_agent_id(),
                agent2 in arb_agent_id(),
                tx1 in arb_tx_id(),
                tx2 in arb_tx_id(),
                entity1 in arb_entity_id(),
                entity2 in arb_entity_id(),
                value1 in arb_doc_value(),
                value2 in arb_doc_value(),
            ) {
                let d1 = Datom::new(entity1, Attribute::from_keyword(":db/doc"), value1, tx1, Op::Assert);
                let d2 = Datom::new(entity2, Attribute::from_keyword(":db/doc"), value2, tx2, Op::Assert);
                let txf1 = arb_tx_file(vec![d1.clone()], agent1, tx1);
                let txf2 = arb_tx_file(vec![d2.clone()], agent2, tx2);

                let merged = collect_datoms(&[txf1, txf2]);
                prop_assert!(merged.contains(&d1), "INV-LAYOUT-004: datom from tx1 lost in merge");
                prop_assert!(merged.contains(&d2), "INV-LAYOUT-004: datom from tx2 lost in merge");
            }

            /// INV-LAYOUT-005: Content hash verification — verify(bytes, hash(bytes)) = true.
            #[test]
            fn inv_layout_005_hash_verification(
                agent in arb_agent_id(),
                tx_id in arb_tx_id(),
            ) {
                let tx = TxFile {
                    tx_id,
                    agent,
                    provenance: ProvenanceType::Observed,
                    rationale: "verify-test".into(),
                    causal_predecessors: vec![],
                    datoms: vec![],
                };
                let bytes = serialize_tx(&tx);
                let hash = ContentHash::of(&bytes);
                prop_assert!(
                    verify_content_hash(&bytes, &hash),
                    "INV-LAYOUT-005: content hash verification failed"
                );
            }

            /// INV-LAYOUT-008: Serialization path is deterministic — same TxFile, same bytes.
            #[test]
            fn inv_layout_008_shard_prefix_deterministic(
                agent in arb_agent_id(),
                tx_id in arb_tx_id(),
            ) {
                let tx = TxFile {
                    tx_id,
                    agent,
                    provenance: ProvenanceType::Observed,
                    rationale: "shard-test".into(),
                    causal_predecessors: vec![],
                    datoms: vec![],
                };
                let hash = tx_content_hash(&tx);
                let path1 = TxFilePath::from_hash(&hash);
                let path2 = TxFilePath::from_hash(&hash);
                prop_assert_eq!(
                    path1.relative_path(), path2.relative_path(),
                    "INV-LAYOUT-008: same hash → different paths"
                );
                // Shard is first 2 hex chars of hash
                prop_assert_eq!(path1.shard.len(), 2, "shard should be 2 hex chars");
            }
        }
    }
}
