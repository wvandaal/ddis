//! Canonical Spec ID Algebra — provable bijection between human and machine identifiers.
//!
//! DDIS spec elements have two representations:
//! - Human-readable: `INV-GUIDANCE-022`, `ADR-STORE-012`, `NEG-MERGE-001`
//! - Machine-readable (store ident): `:spec/inv-guidance-022`, `:spec/adr-store-012`
//!
//! This module provides a single `SpecId` type that handles both forms,
//! with a provable bijection: `denormalize(normalize(h)) = h` for all valid `h`.
//!
//! INV-REFLEXIVE-003: Spec ID Bijection.
//! Traces to: C5 (Traceability), INV-STORE-003 (Content-Addressable Identity).

/// A canonical spec element identifier.
///
/// Stores the normalized components (type, namespace, number) and can
/// render in either human-readable or store-ident form.
///
/// # Examples
///
/// ```
/// use braid_kernel::spec_id::SpecId;
///
/// let id = SpecId::parse("INV-GUIDANCE-022").unwrap();
/// assert_eq!(id.human_form(), "INV-GUIDANCE-022");
/// assert_eq!(id.store_ident(), ":spec/inv-guidance-022");
///
/// let id2 = SpecId::from_store_ident(":spec/adr-store-012").unwrap();
/// assert_eq!(id2.human_form(), "ADR-STORE-012");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpecId {
    /// Element type: "INV", "ADR", or "NEG".
    pub element_type: String,
    /// Namespace: "STORE", "GUIDANCE", "MERGE", etc.
    pub namespace: String,
    /// Numeric suffix: "001", "022", etc. (preserved as string for leading zeros).
    pub number: String,
}

impl SpecId {
    /// Parse a human-readable spec ID like "INV-GUIDANCE-022".
    ///
    /// Case-insensitive: "inv-store-001" and "INV-STORE-001" both parse.
    /// Returns `None` if the input doesn't match the expected pattern.
    pub fn parse(input: &str) -> Option<Self> {
        let upper = input.trim().to_uppercase();
        let parts: Vec<&str> = upper.splitn(3, '-').collect();
        if parts.len() != 3 {
            return None;
        }

        let element_type = parts[0];
        if !matches!(element_type, "INV" | "ADR" | "NEG") {
            return None;
        }

        let namespace = parts[1];
        if namespace.is_empty() || !namespace.chars().all(|c| c.is_ascii_uppercase()) {
            return None;
        }

        let number = parts[2];
        if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }

        Some(SpecId {
            element_type: element_type.to_string(),
            namespace: namespace.to_string(),
            number: number.to_string(),
        })
    }

    /// Parse from a store ident like ":spec/inv-guidance-022".
    pub fn from_store_ident(ident: &str) -> Option<Self> {
        let stripped = ident.strip_prefix(":spec/")?;
        // Store idents are lowercase; convert to uppercase for parsing
        Self::parse(&stripped.to_uppercase())
    }

    /// Render as human-readable form: "INV-GUIDANCE-022".
    pub fn human_form(&self) -> String {
        format!("{}-{}-{}", self.element_type, self.namespace, self.number)
    }

    /// Render as store ident: ":spec/inv-guidance-022".
    pub fn store_ident(&self) -> String {
        format!(
            ":spec/{}-{}-{}",
            self.element_type.to_lowercase(),
            self.namespace.to_lowercase(),
            self.number
        )
    }

    /// Try to parse from either form (auto-detect).
    pub fn from_any(input: &str) -> Option<Self> {
        if input.starts_with(":spec/") {
            Self::from_store_ident(input)
        } else {
            Self::parse(input)
        }
    }
}

impl std::fmt::Display for SpecId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.human_form())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_inv() {
        let id = SpecId::parse("INV-GUIDANCE-022").unwrap();
        assert_eq!(id.element_type, "INV");
        assert_eq!(id.namespace, "GUIDANCE");
        assert_eq!(id.number, "022");
    }

    #[test]
    fn parse_adr() {
        let id = SpecId::parse("ADR-STORE-012").unwrap();
        assert_eq!(id.element_type, "ADR");
        assert_eq!(id.namespace, "STORE");
        assert_eq!(id.number, "012");
    }

    #[test]
    fn parse_neg() {
        let id = SpecId::parse("NEG-MERGE-001").unwrap();
        assert_eq!(id.element_type, "NEG");
        assert_eq!(id.namespace, "MERGE");
    }

    #[test]
    fn parse_case_insensitive() {
        let id1 = SpecId::parse("INV-STORE-001").unwrap();
        let id2 = SpecId::parse("inv-store-001").unwrap();
        let id3 = SpecId::parse("Inv-Store-001").unwrap();
        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(SpecId::parse("not-a-spec-id").is_none());
        assert!(SpecId::parse("INV-").is_none());
        assert!(SpecId::parse("INV-STORE").is_none());
        assert!(SpecId::parse("FOO-STORE-001").is_none());
        assert!(SpecId::parse("").is_none());
    }

    #[test]
    fn human_form_roundtrip() {
        let id = SpecId::parse("INV-GUIDANCE-022").unwrap();
        assert_eq!(id.human_form(), "INV-GUIDANCE-022");
    }

    #[test]
    fn store_ident_format() {
        let id = SpecId::parse("INV-GUIDANCE-022").unwrap();
        assert_eq!(id.store_ident(), ":spec/inv-guidance-022");
    }

    #[test]
    fn from_store_ident_roundtrip() {
        let id = SpecId::from_store_ident(":spec/inv-guidance-022").unwrap();
        assert_eq!(id.human_form(), "INV-GUIDANCE-022");
        assert_eq!(id.store_ident(), ":spec/inv-guidance-022");
    }

    #[test]
    fn bijection_normalize_denormalize() {
        // denormalize(normalize(h)) = h for all valid h
        let cases = [
            "INV-STORE-001",
            "ADR-GUIDANCE-022",
            "NEG-MERGE-003",
            "INV-REFLEXIVE-005",
            "ADR-WITNESS-004",
        ];
        for human in &cases {
            let id = SpecId::parse(human).unwrap();
            let store_form = id.store_ident();
            let roundtrip = SpecId::from_store_ident(&store_form).unwrap();
            assert_eq!(
                roundtrip.human_form(),
                *human,
                "bijection failed for {human}"
            );
        }
    }

    #[test]
    fn from_any_detects_format() {
        let from_human = SpecId::from_any("INV-STORE-001").unwrap();
        let from_store = SpecId::from_any(":spec/inv-store-001").unwrap();
        assert_eq!(from_human, from_store);
    }

    #[test]
    fn display_shows_human_form() {
        let id = SpecId::parse("ADR-STORE-012").unwrap();
        assert_eq!(format!("{id}"), "ADR-STORE-012");
    }
}
