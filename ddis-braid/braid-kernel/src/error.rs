//! Error types for the braid-kernel crate.
//!
//! Each namespace has its own error variants. `KernelError` is the top-level
//! enum that wraps them all. The binary crate wraps `KernelError` in `BraidError`
//! which adds IO context.

use crate::datom::Attribute;

/// Top-level kernel error. Pure — no IO errors here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KernelError {
    /// Error during store operations.
    Store(StoreError),
    /// Error during schema validation.
    Schema(SchemaError),
}

impl std::fmt::Display for KernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KernelError::Store(e) => write!(f, "store: {e}"),
            KernelError::Schema(e) => write!(f, "schema: {e}"),
        }
    }
}

impl std::error::Error for KernelError {}

impl From<StoreError> for KernelError {
    fn from(e: StoreError) -> Self {
        KernelError::Store(e)
    }
}

impl From<SchemaError> for KernelError {
    fn from(e: SchemaError) -> Self {
        KernelError::Schema(e)
    }
}

/// Error from store operations (transact, merge).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// Transaction validation failed — attribute not in schema.
    UnknownAttribute(Attribute),
    /// Transaction validation failed — value type mismatch.
    SchemaViolation {
        /// The attribute that was violated.
        attr: Attribute,
        /// Human-readable description of the expected type.
        expected: String,
        /// Human-readable description of the actual type.
        got: String,
    },
    /// Duplicate transaction ID.
    DuplicateTransaction(String),
    /// Empty transaction.
    EmptyTransaction,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::UnknownAttribute(a) => write!(f, "unknown attribute: {}", a.as_str()),
            StoreError::SchemaViolation {
                attr,
                expected,
                got,
            } => write!(
                f,
                "schema violation on {}: expected {expected}, got {got}",
                attr.as_str()
            ),
            StoreError::DuplicateTransaction(id) => write!(f, "duplicate transaction: {id}"),
            StoreError::EmptyTransaction => write!(f, "empty transaction"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Error from schema operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaError {
    /// Invalid attribute keyword format.
    InvalidAttribute(String),
    /// Schema consistency violation.
    Inconsistency(String),
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::InvalidAttribute(msg) => write!(f, "invalid attribute: {msg}"),
            SchemaError::Inconsistency(msg) => write!(f, "schema inconsistency: {msg}"),
        }
    }
}

impl std::error::Error for SchemaError {}
