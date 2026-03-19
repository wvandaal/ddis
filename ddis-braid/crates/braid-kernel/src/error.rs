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
    /// Error during topology compilation (INV-TOPOLOGY-001..005).
    Topology(TopologyError),
}

impl std::fmt::Display for KernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KernelError::Store(e) => write!(f, "store: {e}"),
            KernelError::Schema(e) => write!(f, "schema: {e}"),
            KernelError::Topology(e) => write!(f, "topology: {e}"),
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
            KernelError::Topology(e) => e.recovery_hint(),
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

impl From<TopologyError> for KernelError {
    fn from(e: TopologyError) -> Self {
        KernelError::Topology(e)
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

/// Error from topology compilation operations (INV-TOPOLOGY-001..005).
///
/// Each variant maps to a specific phase of the topology pipeline
/// (ADR-TOPOLOGY-004: Topology as Compilation) and carries a recovery
/// hint pointing to the concrete `braid` command that resolves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TopologyError {
    /// Fewer than 2 ready tasks — no partition is meaningful.
    InsufficientTasks {
        /// Number of ready tasks found.
        found: usize,
    },
    /// No coupling data available — all tasks have disjoint or empty file sets.
    NoCouplingData,
    /// Zero agents requested — agent_count must be >= 1.
    AgentCountZero,
    /// Coupling weight vector is empty or all-zero.
    NoCouplingWeights,
    /// Partition imbalance exceeds threshold (max_group / min_group > bound).
    PartitionImbalance {
        /// Ratio formatted as "X.XX" for error display.
        ratio: String,
        /// Threshold formatted as "X.XX" for error display.
        threshold: String,
    },
    /// Disjointness invariant violated — a file appears in multiple agent assignments.
    DisjointnessViolation {
        /// The file that appears in multiple assignments.
        file: String,
    },
    /// No spec dependency edges in store — topology front-end has no input.
    NoSpecDependencies,
}

impl std::fmt::Display for TopologyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TopologyError::InsufficientTasks { found } => write!(
                f,
                "topology requires >= 2 ready tasks, found {found} (INV-TOPOLOGY-001)"
            ),
            TopologyError::NoCouplingData => write!(
                f,
                "no file coupling data — tasks have no FILE: markers or shared files \
                 (INV-TOPOLOGY-004)"
            ),
            TopologyError::AgentCountZero => {
                write!(f, "agent count must be >= 1 (INV-TOPOLOGY-005)")
            }
            TopologyError::NoCouplingWeights => write!(
                f,
                "coupling weight vector is empty or all-zero (ADR-TOPOLOGY-001)"
            ),
            TopologyError::PartitionImbalance { ratio, threshold } => write!(
                f,
                "partition imbalance {ratio} exceeds threshold {threshold} \
                 (INV-TOPOLOGY-005)"
            ),
            TopologyError::DisjointnessViolation { file } => write!(
                f,
                "file {file} assigned to multiple agents — disjointness violated \
                 (INV-TOPOLOGY-003)"
            ),
            TopologyError::NoSpecDependencies => write!(
                f,
                "no :spec/traces-to edges in store — run braid spec to populate \
                 (ADR-TOPOLOGY-004)"
            ),
        }
    }
}

impl std::error::Error for TopologyError {}

impl TopologyError {
    /// Returns a human-readable recovery suggestion for this error.
    pub fn recovery_hint(&self) -> &'static str {
        match self {
            TopologyError::InsufficientTasks { .. } => {
                "Create more tasks before computing topology. \
                 Use `braid task create \"...\"` to add tasks, \
                 or `braid task list` to check current ready set."
            }
            TopologyError::NoCouplingData => {
                "Add FILE: markers to task titles so topology can detect coupling. \
                 Example: `braid task set <id> title \"Fix X. FILE: crates/a/src/b.rs\"`"
            }
            TopologyError::AgentCountZero => {
                "Specify at least 1 agent. \
                 Use `braid topology plan --agents 2` (or more)."
            }
            TopologyError::NoCouplingWeights => {
                "Coupling weights are unconfigured or all zero. \
                 Check `:topology/coupling-weights` config via `braid query`."
            }
            TopologyError::PartitionImbalance { .. } => {
                "Partition is too uneven. Consider adding FILE: markers to \
                 abstract tasks, or increase agent count to spread the load. \
                 Use `braid topology plan --agents N` with a higher N."
            }
            TopologyError::DisjointnessViolation { .. } => {
                "A file was assigned to multiple agents. This indicates a bug \
                 in the partition algorithm. Report via `braid observe` with \
                 the details and re-run with `--agents 1` as a workaround."
            }
            TopologyError::NoSpecDependencies => {
                "The store has no spec dependency edges. \
                 Run `braid spec` to parse and transact :spec/traces-to datoms \
                 from spec/*.md files."
            }
        }
    }
}
