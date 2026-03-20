//! Capability Census — runtime self-knowledge for the braid system.
//!
//! Each subsystem reports its implementation status via the `Censusable` trait.
//! Census results are asserted as `:capability/*` datoms at session start,
//! making the system's self-knowledge queryable and traceable.
//!
//! Traces to: INV-REFLEXIVE-001 (Capability Census Completeness),
//! C7 (Self-Bootstrap), C3 (Schema-as-Data).

/// The implementation status of a subsystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CensusStatus {
    /// Subsystem is compiled in and functional.
    Implemented,
    /// Subsystem is specified but not yet implemented.
    SpecOnly,
    /// Subsystem is partially implemented (with description of what's missing).
    Partial(String),
}

/// A census result for a single subsystem.
#[derive(Clone, Debug)]
pub struct CensusResult {
    /// Machine-readable subsystem name (e.g., "live-index", "witness", "mcp").
    pub name: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Implementation status.
    pub status: CensusStatus,
}

impl CensusResult {
    /// Create a new census result.
    pub fn new(name: &str, display_name: &str, status: CensusStatus) -> Self {
        CensusResult {
            name: name.to_string(),
            display_name: display_name.to_string(),
            status,
        }
    }

    /// Whether this subsystem is fully implemented.
    pub fn is_implemented(&self) -> bool {
        self.status == CensusStatus::Implemented
    }

    /// The store ident for this capability (e.g., ":capability/live-index").
    pub fn ident(&self) -> String {
        format!(":capability/{}", self.name)
    }

    /// The status keyword for store assertion.
    pub fn status_keyword(&self) -> String {
        match &self.status {
            CensusStatus::Implemented => ":capability.status/implemented".to_string(),
            CensusStatus::SpecOnly => ":capability.status/spec-only".to_string(),
            CensusStatus::Partial(desc) => format!(":capability.status/partial:{desc}"),
        }
    }
}

/// Run the full capability census for all known subsystems.
///
/// This is the authoritative list of subsystems. Each is probed for
/// implementation evidence. The results can be asserted as store datoms
/// at session start.
///
/// INV-REFLEXIVE-001: Census is total — covers every known subsystem.
pub fn run_census(store: &crate::store::Store) -> Vec<CensusResult> {
    use crate::datom::Op;

    let has_attr_prefix = |prefix: &str| -> bool {
        store
            .datoms()
            .any(|d| d.attribute.as_str().starts_with(prefix) && d.op == Op::Assert)
    };

    // T-UX-4: All modules are compiled into the binary and always available.
    // Availability is a compile-time property, not a runtime one.
    // Data presence (has_attr_prefix) is shown as Partial("dormant") when no
    // datoms have been produced yet, distinguishing "available" from "active."
    vec![
        CensusResult::new("live-index", "LIVE index", CensusStatus::Implemented),
        CensusResult::new(
            "cache-persistence",
            ".cache/ persistence",
            CensusStatus::Implemented,
        ),
        CensusResult::new(
            "witness",
            "WITNESS system",
            if has_attr_prefix(":witness/") {
                CensusStatus::Implemented
            } else {
                CensusStatus::Partial("dormant: no witness data yet".to_string())
            },
        ),
        CensusResult::new("agp", "Adaptive guidance (AGP)", CensusStatus::Implemented),
        CensusResult::new(
            "harvest-seed",
            "Harvest/Seed lifecycle",
            if has_attr_prefix(":harvest/") {
                CensusStatus::Implemented
            } else {
                CensusStatus::Partial("dormant: no harvest data yet".to_string())
            },
        ),
        CensusResult::new(
            "task-routing",
            "R(t) task routing",
            if has_attr_prefix(":task/") {
                CensusStatus::Implemented
            } else {
                CensusStatus::Partial("dormant: no task data yet".to_string())
            },
        ),
        // Datalog query: always available (compiled in)
        CensusResult::new("datalog-query", "Datalog query", CensusStatus::Implemented),
        // MCP interface: always available (compiled in)
        CensusResult::new("mcp", "MCP interface", CensusStatus::Implemented),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    #[test]
    fn census_returns_all_subsystems() {
        let store = Store::genesis();
        let results = run_census(&store);
        assert_eq!(results.len(), 8, "should have 8 subsystems");
    }

    #[test]
    fn census_genesis_store_has_implemented_subsystems() {
        let store = Store::genesis();
        let results = run_census(&store);
        let agp = results.iter().find(|r| r.name == "agp").unwrap();
        assert!(agp.is_implemented());
        let datalog = results.iter().find(|r| r.name == "datalog-query").unwrap();
        assert!(datalog.is_implemented());
    }

    #[test]
    fn census_witness_dormant_without_data() {
        // T-UX-4: WITNESS is always compiled-in. Without data, status is Partial("dormant").
        let store = Store::from_datoms(std::collections::BTreeSet::new());
        let results = run_census(&store);
        let witness = results.iter().find(|r| r.name == "witness").unwrap();
        assert!(
            matches!(&witness.status, CensusStatus::Partial(s) if s.contains("dormant")),
            "witness without data should be Partial(dormant), got {:?}",
            witness.status
        );
    }

    #[test]
    fn census_result_ident_format() {
        let r = CensusResult::new("live-index", "LIVE", CensusStatus::Implemented);
        assert_eq!(r.ident(), ":capability/live-index");
    }

    #[test]
    fn census_result_status_keyword() {
        let r1 = CensusResult::new("x", "X", CensusStatus::Implemented);
        assert_eq!(r1.status_keyword(), ":capability.status/implemented");

        let r2 = CensusResult::new("x", "X", CensusStatus::SpecOnly);
        assert_eq!(r2.status_keyword(), ":capability.status/spec-only");
    }
}
