//! The error vocabulary shared by every protocol. Callers match on this instead of on strings.

use std::time::Duration;

/// Anything a protocol client can fail with.
///
/// The variants are deliberately coarse: a caller that only cares whether the device answered can
/// match `Io` and `Timeout` and ignore the rest, and a caller that builds a report can render the whole
/// enum without knowing which protocol produced it.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The socket failed, was reset, or could not be opened.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// The peer did not answer inside the budget the caller set.
    #[error("timeout after {}s", .0.as_secs())]
    Timeout(Duration),

    /// The peer answered, and the answer was wrong for this protocol.
    #[error("protocol {protocol}: {detail}")]
    Protocol {
        /// Slug of the protocol that rejected the answer, for example `modbus`.
        protocol: &'static str,
        /// What was wrong, in the words the parser would use.
        detail: String,
    },

    /// The peer rejected the credentials.
    #[error("authentication failed for {user}")]
    Auth {
        /// The account that was refused.
        user: String,
    },

    /// This protocol has no such operation, today.
    #[error("unsupported: {0}")]
    Unsupported(String),
}

/// Short alias, so signatures read `Result<T>` inside the crate.
pub type Result<T> = std::result::Result<T, Error>;
