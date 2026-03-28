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

/// Default allowed element types (bootstrap fallback — C9: INV-FOUNDATION-015).
pub const DEFAULT_ELEMENT_TYPES: &[&str] = &["INV", "ADR", "NEG"];

impl SpecId {
    /// Parse a human-readable spec ID like "INV-GUIDANCE-022".
    ///
    /// Case-insensitive: "inv-store-001" and "INV-STORE-001" both parse.
    /// Returns `None` if the input doesn't match the expected pattern.
    /// Uses the hardcoded default element types `["INV", "ADR", "NEG"]`.
    /// For policy-configurable parsing, use [`parse_with_types`].
    pub fn parse(input: &str) -> Option<Self> {
        Self::parse_with_types(input, DEFAULT_ELEMENT_TYPES)
    }

    /// Parse a human-readable spec ID with configurable element types
    /// (C9: AUDIT-W1-005, INV-FOUNDATION-015).
    ///
    /// `allowed_types` contains the accepted element type prefixes (e.g., `["INV", "ADR", "NEG"]`).
    /// Case-insensitive: both input and allowed types are compared in uppercase.
    /// Returns `None` if the input doesn't match the expected pattern or uses
    /// an element type not in `allowed_types`.
    pub fn parse_with_types(input: &str, allowed_types: &[&str]) -> Option<Self> {
        let upper = input.trim().to_uppercase();
        let parts: Vec<&str> = upper.splitn(3, '-').collect();
        if parts.len() != 3 {
            return None;
        }

        let element_type = parts[0];
        if !allowed_types
            .iter()
            .any(|t| t.eq_ignore_ascii_case(element_type))
        {
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

    /// Parse a human-readable spec ID using policy-loaded element types.
    ///
    /// If `policy_types` is non-empty, uses those as the allowed set;
    /// otherwise falls back to the hardcoded defaults.
    pub fn parse_with_policy(input: &str, policy_types: &[String]) -> Option<Self> {
        if policy_types.is_empty() {
            Self::parse(input)
        } else {
            let types_as_strs: Vec<&str> = policy_types.iter().map(|s| s.as_str()).collect();
            Self::parse_with_types(input, &types_as_strs)
        }
    }

    /// Parse from a store ident like ":spec/inv-guidance-022".
    pub fn from_store_ident(ident: &str) -> Option<Self> {
        let stripped = ident.strip_prefix(":spec/")?;
        // Store idents are lowercase; convert to uppercase for parsing
        Self::parse(&stripped.to_uppercase())
    }

    /// Parse from a store ident using policy-loaded element types.
    pub fn from_store_ident_with_policy(ident: &str, policy_types: &[String]) -> Option<Self> {
        let stripped = ident.strip_prefix(":spec/")?;
        Self::parse_with_policy(&stripped.to_uppercase(), policy_types)
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

    /// Try to parse from either form using policy-loaded element types.
    pub fn from_any_with_policy(input: &str, policy_types: &[String]) -> Option<Self> {
        if input.starts_with(":spec/") {
            Self::from_store_ident_with_policy(input, policy_types)
        } else {
            Self::parse_with_policy(input, policy_types)
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

    // ── AUDIT-W1-005: Configurable element types ──

    #[test]
    fn parse_with_custom_types_accepts_custom() {
        // Custom types: REQ (requirement), CTRL (control)
        let id = SpecId::parse_with_types("REQ-AUTH-001", &["REQ", "CTRL"]).unwrap();
        assert_eq!(id.element_type, "REQ");
        assert_eq!(id.namespace, "AUTH");
        assert_eq!(id.number, "001");

        let id2 = SpecId::parse_with_types("CTRL-ACCESS-042", &["REQ", "CTRL"]).unwrap();
        assert_eq!(id2.element_type, "CTRL");
    }

    #[test]
    fn parse_with_custom_types_rejects_default() {
        // When custom types are specified, default types are NOT accepted
        assert!(SpecId::parse_with_types("INV-STORE-001", &["REQ", "CTRL"]).is_none());
        assert!(SpecId::parse_with_types("ADR-STORE-001", &["REQ", "CTRL"]).is_none());
        assert!(SpecId::parse_with_types("NEG-STORE-001", &["REQ", "CTRL"]).is_none());
    }

    #[test]
    fn parse_with_default_types_matches_parse() {
        // Using DEFAULT_ELEMENT_TYPES should produce identical results to parse()
        let default_result = SpecId::parse("INV-STORE-001");
        let explicit_result = SpecId::parse_with_types("INV-STORE-001", DEFAULT_ELEMENT_TYPES);
        assert_eq!(default_result, explicit_result);
    }

    #[test]
    fn parse_with_policy_empty_uses_defaults() {
        // Empty policy types = fall back to defaults
        let empty: Vec<String> = vec![];
        let id = SpecId::parse_with_policy("INV-STORE-001", &empty).unwrap();
        assert_eq!(id.element_type, "INV");
    }

    #[test]
    fn parse_with_policy_non_empty_uses_custom() {
        let policy = vec!["REQ".to_string(), "CTRL".to_string()];
        let id = SpecId::parse_with_policy("REQ-AUTH-001", &policy).unwrap();
        assert_eq!(id.element_type, "REQ");
        // Default types should be rejected when policy is non-empty
        assert!(SpecId::parse_with_policy("INV-STORE-001", &policy).is_none());
    }

    #[test]
    fn from_store_ident_with_policy_works() {
        let policy = vec!["REQ".to_string(), "CTRL".to_string()];
        let id = SpecId::from_store_ident_with_policy(":spec/req-auth-001", &policy).unwrap();
        assert_eq!(id.human_form(), "REQ-AUTH-001");
    }

    #[test]
    fn from_any_with_policy_works() {
        let policy = vec!["REQ".to_string()];
        let from_human = SpecId::from_any_with_policy("REQ-AUTH-001", &policy).unwrap();
        let from_store = SpecId::from_any_with_policy(":spec/req-auth-001", &policy).unwrap();
        assert_eq!(from_human, from_store);
    }

    #[test]
    fn parse_with_types_case_insensitive() {
        // Allowed types comparison is case-insensitive
        let id = SpecId::parse_with_types("req-auth-001", &["REQ"]).unwrap();
        assert_eq!(id.element_type, "REQ");
    }
}
