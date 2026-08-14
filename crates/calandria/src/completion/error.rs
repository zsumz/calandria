//! Terminal observation failures independent of any async runtime.

use std::{error::Error, fmt};

/// Why a completion could not yield its terminal value.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionError {
    /// The producer closed without publishing a value.
    Closed,
    /// The single terminal outcome was already consumed.
    Consumed,
}

impl fmt::Display for CompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => formatter.write_str("the completion producer closed without a value"),
            Self::Consumed => formatter.write_str("the completion value was already consumed"),
        }
    }
}

impl Error for CompletionError {}
