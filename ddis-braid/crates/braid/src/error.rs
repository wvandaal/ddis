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
        match self {
            BraidError::Kernel(e) => write!(f, "{e}"),
            BraidError::Io(e) => write!(f, "io: {e}"),
            BraidError::Parse(e) => write!(f, "parse: {e}"),
            BraidError::DatalogParse(e) => write!(f, "datalog: {e}"),
            BraidError::Validation(e) => write!(f, "validation: {e}"),
            BraidError::EmptyResult(e) => write!(f, "no results: {e}"),
        }
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
    /// Returns a human-readable recovery suggestion for this error.
    ///
    /// Every variant produces an actionable hint. Kernel errors delegate
    /// to `KernelError::recovery_hint()`. IO and parse errors inspect
    /// their context to provide targeted advice.
    pub fn recovery_hint(&self) -> &'static str {
        match self {
            BraidError::Kernel(e) => e.recovery_hint(),
            BraidError::Io(e) => match e.kind() {
                std::io::ErrorKind::NotFound => {
                    "The store directory was not found. \
                     Run `braid init` to create a new store, \
                     or pass `--path` to point at an existing one."
                }
                std::io::ErrorKind::PermissionDenied => {
                    "Permission denied accessing the store. \
                     Check filesystem permissions on the .braid directory."
                }
                std::io::ErrorKind::AlreadyExists => {
                    "The target already exists. \
                     If re-initializing, remove the existing .braid directory first \
                     (with the user's explicit permission)."
                }
                _ => {
                    "An IO error occurred. \
                     Check that the .braid directory is accessible \
                     and the filesystem has sufficient space."
                }
            },
            BraidError::Parse(_) => {
                "The input could not be parsed. \
                 Check the EDN syntax: keywords use colons (:ns/name), \
                 strings use double-quotes, and maps use curly braces."
            }
            BraidError::DatalogParse(_) => {
                "Datalog syntax: [:find ?var :where [?var :attribute value]]. \
                 Variables start with ?, attributes are keywords (:ns/name), \
                 strings need double-quotes, numbers are bare. \
                 Example: [:find ?e ?v :where [?e :db/doc ?v]]"
            }
            BraidError::Validation(_) => {
                "The input failed validation. \
                 Check the allowed ranges and formats in `braid <command> --help`."
            }
            BraidError::EmptyResult(_) => {
                "No datoms matched your query. Try: \
                 `braid query -a :db/ident` to list all entities, \
                 `braid status` to see store contents, or \
                 `braid find :db/doc` in the shell to browse."
            }
        }
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
