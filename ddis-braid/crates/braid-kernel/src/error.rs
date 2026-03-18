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

impl KernelError {
    /// Returns a human-readable recovery suggestion for this error.
    ///
    /// Every variant maps to an actionable hint. This is the kernel-level
    /// half of the error protocol; the binary crate adds IO-level hints.
    pub fn recovery_hint(&self) -> &'static str {
        match self {
            KernelError::Store(e) => e.recovery_hint(),
            KernelError::Schema(e) => e.recovery_hint(),
        }
    }
}

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
    /// Causal predecessor not found in the store (INV-STORE-010).
    InvalidCausalPredecessor(String),
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
            StoreError::InvalidCausalPredecessor(id) => {
                write!(f, "causal predecessor not found: {id}")
            }
        }
    }
}

impl std::error::Error for StoreError {}

impl StoreError {
    /// Returns a human-readable recovery suggestion for this error.
    pub fn recovery_hint(&self) -> &'static str {
        match self {
            StoreError::UnknownAttribute(_) => {
                "Register the attribute in the schema before transacting. \
                 Use `braid transact` with a :db/ident datom to define it."
            }
            StoreError::SchemaViolation { .. } => {
                "Check the attribute's declared value type in the schema \
                 and ensure your datom value matches. \
                 Use `braid query -a db/valueType` to inspect the schema."
            }
            StoreError::DuplicateTransaction(_) => {
                "This transaction ID was already committed. \
                 Each transaction must carry a unique ID. \
                 Verify you are not replaying an already-applied transaction file."
            }
            StoreError::EmptyTransaction => {
                "Supply at least one datom assertion. \
                 Use `braid transact -d entity attribute value` to add datoms."
            }
            StoreError::InvalidCausalPredecessor(_) => {
                "A causal predecessor transaction was not found in the store. \
                 Ensure the predecessor transaction has been committed before \
                 creating a transaction that depends on it. \
                 Use `braid log` to list committed transactions."
            }
        }
    }
}

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

impl SchemaError {
    /// Returns a human-readable recovery suggestion for this error.
    pub fn recovery_hint(&self) -> &'static str {
        match self {
            SchemaError::InvalidAttribute(_) => {
                "Attribute keywords must be namespaced (e.g., :db/ident, :spec/title). \
                 Check for typos, missing namespace prefix, or invalid characters."
            }
            SchemaError::Inconsistency(_) => {
                "The schema has conflicting definitions. \
                 Use `braid query -a db/ident` to list all schema attributes \
                 and look for duplicate or contradictory declarations."
            }
        }
    }
}
