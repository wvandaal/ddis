/// Braid binary error type — wraps kernel errors with IO context.
#[derive(Debug)]
pub enum BraidError {
    /// Error from the kernel (pure computation).
    Kernel(braid_kernel::KernelError),
    /// IO error (filesystem, network).
    Io(std::io::Error),
}

impl std::fmt::Display for BraidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BraidError::Kernel(e) => write!(f, "{e}"),
            BraidError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for BraidError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BraidError::Kernel(e) => Some(e),
            BraidError::Io(e) => Some(e),
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
