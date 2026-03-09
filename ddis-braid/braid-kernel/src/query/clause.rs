//! Query clause types for Datalog expressions.
//!
//! A query is a set of clauses that pattern-match against the datom store.
//! Variables are bound by unification, and the result is the set of all
//! satisfying bindings.

use crate::datom::{Attribute, EntityId, Value};

/// A Datalog query expression.
#[derive(Clone, Debug)]
pub struct QueryExpr {
    /// What to return (find specification).
    pub find: FindSpec,
    /// The clauses to match.
    pub where_clauses: Vec<Clause>,
}

/// What the query should return.
#[derive(Clone, Debug)]
pub enum FindSpec {
    /// Return bindings for the named variables.
    Rel(Vec<String>),
    /// Return a single scalar value.
    Scalar(String),
}

/// A single query clause.
#[derive(Clone, Debug)]
pub enum Clause {
    /// A data pattern: match datoms where (entity, attribute, value) match.
    Pattern(Pattern),
    /// A predicate filter: only include bindings where the predicate holds.
    Predicate {
        /// Function name (e.g., `>`, `<`, `=`).
        op: String,
        /// Arguments (variable names or constants).
        args: Vec<Term>,
    },
}

/// A single term in a pattern or predicate.
#[derive(Clone, Debug)]
pub enum Term {
    /// A logic variable (e.g., `?e`, `?name`).
    Variable(String),
    /// A constant value.
    Constant(Value),
    /// A constant entity ID.
    Entity(EntityId),
    /// A constant attribute.
    Attr(Attribute),
}

/// A data pattern that matches against datoms.
///
/// Each position can be a variable (to bind) or a constant (to match).
#[derive(Clone, Debug)]
pub struct Pattern {
    /// Entity position.
    pub entity: Term,
    /// Attribute position.
    pub attribute: Term,
    /// Value position.
    pub value: Term,
}

/// A binding environment: variable name → value.
pub type Binding = std::collections::HashMap<String, Value>;

impl Pattern {
    /// Create a pattern from entity, attribute, value terms.
    pub fn new(entity: Term, attribute: Term, value: Term) -> Self {
        Pattern {
            entity,
            attribute,
            value,
        }
    }
}

impl QueryExpr {
    /// Create a new query.
    pub fn new(find: FindSpec, where_clauses: Vec<Clause>) -> Self {
        QueryExpr {
            find,
            where_clauses,
        }
    }
}
