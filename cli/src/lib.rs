//! Public library surface of the `deal` CLI crate.
//!
//! Exposes internal modules (resolver, etc.) so that integration tests
//! in `cli/tests/` can call them directly via `deal::resolver::resolve_all`.
//! The binary entry point (`main.rs`) is a separate compilation unit.

pub mod closure;
pub mod evidence;
pub mod model_values;
pub mod reporter;
pub mod reqif;
pub mod reqif_schema;
pub mod resolver;
pub mod sims_protocol;
pub mod simulate;
pub mod verify;

// ─── Shared CliError (used by simulate.rs, sims_protocol.rs, evidence.rs, etc.) ──

/// Typed CLI error: User errors exit 1; Internal errors exit 2 (D-34).
///
/// Defined in lib.rs so modules compiled for both the `deal` binary and the
/// `deal` library (integration tests) can share the same type.
#[derive(Debug)]
pub enum CliError {
    /// A user-visible error (e.g. invalid argument, blocking diagnostic).
    /// CLI prints the message to stderr and exits 1.
    User(String),
    /// An internal error (e.g. not-yet-implemented, FFI failure, OOM).
    /// CLI prints the error chain to stderr and exits 2.
    Internal(anyhow::Error),
}

impl CliError {
    /// Returns true if this is a user-visible error (exit 1).
    pub fn is_user(&self) -> bool {
        matches!(self, CliError::User(_))
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::User(msg) => write!(f, "{}", msg),
            CliError::Internal(e) => write!(f, "internal error: {:#}", e),
        }
    }
}
