/// Four-part error information (INV-INTERFACE-009).
///
/// Every error provides: what happened, why it happened,
/// how to fix it, and which spec element governs it.
pub struct ErrorInfo {
    /// What went wrong (one line).
    pub what: &'static str,
    /// Why it happened (root cause).
    pub why: &'static str,
    /// How to fix it (actionable command or instruction).
    pub fix: &'static str,
    /// Governing spec reference.
    pub spec_ref: &'static str,
}

/// Braid binary error type — wraps kernel errors with IO context.
#[derive(Debug)]
pub enum BraidError {
    /// Error from the kernel (pure computation).
    Kernel(braid_kernel::KernelError),
    /// IO error (filesystem, network).
    Io(std::io::Error),
    /// EDN parse error.
    Parse(String),
    /// Datalog query parse error (more specific hint than generic Parse).
    DatalogParse(String),
    /// Input validation error (bad arguments, out-of-range values).
    Validation(String),
    /// Query returned no results (not an error per se, but needs guidance).
    EmptyResult(String),
}

impl std::fmt::Display for BraidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let info = self.error_info();
        write!(f, "error: {}\n  why: ", info.what)?;
        // Include the specific error detail
        match self {
            BraidError::Kernel(e) => write!(f, "{e}")?,
            BraidError::Io(e) => write!(f, "{e}")?,
            BraidError::Parse(e) => write!(f, "{e}")?,
            BraidError::DatalogParse(e) => write!(f, "{e}")?,
            BraidError::Validation(e) => write!(f, "{e}")?,
            BraidError::EmptyResult(e) => write!(f, "{e}")?,
        }
        write!(f, "\n  fix: {}\n  ref: {}", info.fix, info.spec_ref)
    }
}

impl std::error::Error for BraidError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BraidError::Kernel(e) => Some(e),
            BraidError::Io(e) => Some(e),
            BraidError::Parse(_)
            | BraidError::DatalogParse(_)
            | BraidError::Validation(_)
            | BraidError::EmptyResult(_) => None,
        }
    }
}

impl BraidError {
    /// Returns structured four-part error information (INV-INTERFACE-009).
    ///
    /// Every variant produces: what happened, why, how to fix it,
    /// and the governing spec reference.
    pub fn error_info(&self) -> ErrorInfo {
        match self {
            BraidError::Kernel(e) => ErrorInfo {
                what: "kernel computation error",
                why: e.recovery_hint(),
                fix: "Check inputs and retry. Run `braid status` for store state.",
                spec_ref: "INV-STORE-001",
            },
            BraidError::Io(e) => match e.kind() {
                std::io::ErrorKind::NotFound => ErrorInfo {
                    what: "store not found",
                    why: "the .braid directory does not exist at the specified path",
                    fix: "braid init",
                    spec_ref: "INV-STORE-001",
                },
                std::io::ErrorKind::PermissionDenied => ErrorInfo {
                    what: "permission denied",
                    why: "insufficient filesystem permissions on .braid directory",
                    fix: "Check permissions: ls -la .braid/",
                    spec_ref: "INV-STORE-001",
                },
                std::io::ErrorKind::AlreadyExists => ErrorInfo {
                    what: "store already exists",
                    why: "a .braid directory already exists at this location",
                    fix: "Use existing store or choose a different --path",
                    spec_ref: "INV-STORE-001",
                },
                _ => ErrorInfo {
                    what: "IO error",
                    why: "filesystem operation failed",
                    fix: "Check disk space and permissions on .braid/",
                    spec_ref: "INV-STORE-001",
                },
            },
            BraidError::Parse(_) => ErrorInfo {
                what: "parse error",
                why: "input is not valid EDN syntax",
                fix: "Check EDN: keywords use :ns/name, strings use \"quotes\", maps use {}",
                spec_ref: "INV-INTERFACE-009",
            },
            BraidError::DatalogParse(_) => ErrorInfo {
                what: "Datalog parse error",
                why: "query expression is not valid Datalog syntax",
                fix: "Syntax: [:find ?var :where [?var :attribute value]]. Example: [:find ?e ?v :where [?e :db/doc ?v]]",
                spec_ref: "INV-QUERY-001",
            },
            BraidError::Validation(_) => ErrorInfo {
                what: "validation error",
                why: "input values are outside allowed ranges",
                fix: "Run `braid <command> --help` for valid argument formats",
                spec_ref: "INV-INTERFACE-009",
            },
            BraidError::EmptyResult(_) => ErrorInfo {
                what: "no results",
                why: "no datoms matched the query criteria",
                fix: "braid query --attribute :db/ident  # list all entities",
                spec_ref: "INV-INTERFACE-012",
            },
        }
    }

    /// Returns a human-readable recovery suggestion for this error.
    ///
    /// Delegates to `error_info().fix`. Kept for backward compatibility
    /// with code that calls `recovery_hint()` directly.
    pub fn recovery_hint(&self) -> &'static str {
        self.error_info().fix
    }
}

impl From<braid_kernel::KernelError> for BraidError {
    fn from(e: braid_kernel::KernelError) -> Self {
        BraidError::Kernel(e)
    }
}

impl From<std::io::Error> for BraidError {
    fn from(e: std::io::Error) -> Self {
        BraidError::Io(e)
    }
}

impl From<braid_kernel::EdnParseError> for BraidError {
    fn from(e: braid_kernel::EdnParseError) -> Self {
        BraidError::Parse(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_has_four_parts() {
        let err = BraidError::Parse("unexpected token".into());
        let display = err.to_string();
        assert!(display.contains("error:"), "must have 'error:' prefix");
        assert!(display.contains("why:"), "must have 'why:' section");
        assert!(display.contains("fix:"), "must have 'fix:' section");
        assert!(display.contains("ref:"), "must have 'ref:' section");
    }

    #[test]
    fn io_not_found_has_init_hint() {
        let err = BraidError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        let info = err.error_info();
        assert_eq!(info.what, "store not found");
        assert!(info.fix.contains("braid init"));
    }

    #[test]
    fn empty_result_references_diagnostics() {
        let err = BraidError::EmptyResult("no matches".into());
        let info = err.error_info();
        assert_eq!(info.spec_ref, "INV-INTERFACE-012");
    }

    #[test]
    fn all_variants_have_spec_ref() {
        let variants: Vec<BraidError> = vec![
            BraidError::Parse("test".into()),
            BraidError::DatalogParse("test".into()),
            BraidError::Validation("test".into()),
            BraidError::EmptyResult("test".into()),
            BraidError::Io(std::io::Error::other("test")),
        ];
        for v in &variants {
            let info = v.error_info();
            assert!(
                !info.spec_ref.is_empty(),
                "spec_ref must not be empty for {:?}",
                v
            );
            assert!(!info.fix.is_empty(), "fix must not be empty for {:?}", v);
        }
    }

    #[test]
    fn recovery_hint_delegates_to_error_info() {
        let err = BraidError::Parse("test".into());
        assert_eq!(err.recovery_hint(), err.error_info().fix);
    }
}
