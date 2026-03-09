#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::all)]

//! `braid-kernel` — Pure computation library for the Braid datom store.
//!
//! This crate contains all domain logic for Braid. It has no IO, no async,
//! no filesystem access, and no network. Every function is deterministic:
//! same inputs produce same outputs. This is the verification surface for
//! all property-based testing and bounded model checking.

pub mod datom;
pub mod error;
pub mod resolution;
pub mod schema;
pub mod store;

// Re-export core types at crate root for ergonomic access.
pub use datom::{AgentId, Attribute, Datom, EntityId, Op, ProvenanceType, TxId, Value};
pub use error::KernelError;
pub use resolution::{resolve, ConflictSet, ResolvedValue};
pub use schema::{
    AttributeDef, AttributeSpec, Cardinality, ResolutionMode, Schema, Uniqueness, ValueType,
};
pub use store::{Frontier, MergeReceipt, Store, TxData, TxReceipt};
