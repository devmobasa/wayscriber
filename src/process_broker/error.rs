use std::fmt;

use serde::{Deserialize, Serialize};

/// Stable broker policy categories; diagnostic wording is never a protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BrokerErrorKind {
    Busy,
    MissingExecutable,
    Denied,
    Transport,
    #[default]
    Rejected,
}

#[derive(Debug)]
pub(crate) struct BrokerError {
    pub(crate) kind: BrokerErrorKind,
    message: String,
}

impl BrokerError {
    pub(super) fn new(kind: BrokerErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub(super) fn spawn(error: std::io::Error) -> anyhow::Error {
        let kind = match error.kind() {
            std::io::ErrorKind::NotFound => BrokerErrorKind::MissingExecutable,
            std::io::ErrorKind::PermissionDenied => BrokerErrorKind::Denied,
            _ => BrokerErrorKind::Rejected,
        };
        anyhow::Error::new(error).context(Self::new(kind, "broker helper spawn failed"))
    }
}

impl fmt::Display for BrokerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BrokerError {}

pub(crate) fn error_kind(error: &anyhow::Error) -> Option<BrokerErrorKind> {
    error.downcast_ref::<BrokerError>().map(|error| error.kind)
}
