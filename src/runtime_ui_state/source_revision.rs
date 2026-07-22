use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

/// Exact identity of the managed runtime-state path after resolving links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeStatePathIdentity {
    source_path: PathBuf,
    followed_links: Arc<[(PathBuf, PathBuf)]>,
}

impl RuntimeStatePathIdentity {
    pub(crate) fn new(
        source_path: PathBuf,
        followed_links: impl Into<Arc<[(PathBuf, PathBuf)]>>,
    ) -> Self {
        Self {
            source_path,
            followed_links: followed_links.into(),
        }
    }

    pub(crate) fn direct(source_path: impl Into<PathBuf>) -> Self {
        Self::new(source_path.into(), Arc::from([]))
    }

    pub(crate) fn source_path(&self) -> &std::path::Path {
        &self.source_path
    }

    pub(crate) fn followed_links(&self) -> &[(PathBuf, PathBuf)] {
        &self.followed_links
    }
}

/// Exact source revision used by conditional runtime-state mutations.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum RuntimeStateSourceRevision {
    Missing {
        path: RuntimeStatePathIdentity,
    },
    Present {
        path: RuntimeStatePathIdentity,
        bytes: Arc<[u8]>,
    },
}

impl RuntimeStateSourceRevision {
    pub(crate) fn missing(path: RuntimeStatePathIdentity) -> Self {
        Self::Missing { path }
    }

    pub(crate) fn present(path: RuntimeStatePathIdentity, bytes: impl Into<Arc<[u8]>>) -> Self {
        Self::Present {
            path,
            bytes: bytes.into(),
        }
    }

    pub(crate) fn path_identity(&self) -> &RuntimeStatePathIdentity {
        match self {
            Self::Missing { path } | Self::Present { path, .. } => path,
        }
    }

    pub(crate) fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Missing { .. } => None,
            Self::Present { bytes, .. } => Some(bytes),
        }
    }
}

impl fmt::Debug for RuntimeStateSourceRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => formatter
                .debug_struct("Missing")
                .field("path", path)
                .finish(),
            Self::Present { path, bytes } => formatter
                .debug_struct("Present")
                .field("path", path)
                .field("byte_len", &bytes.len())
                .finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeStateObservedEnvelope {
    Missing,
    Version(u64),
    PresentWithoutReadableVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeStateSourceObservation {
    pub(crate) revision: RuntimeStateSourceRevision,
    pub(crate) envelope: RuntimeStateObservedEnvelope,
}

impl RuntimeStateSourceObservation {
    pub(crate) fn missing(revision: RuntimeStateSourceRevision) -> Self {
        debug_assert!(matches!(
            revision,
            RuntimeStateSourceRevision::Missing { .. }
        ));
        Self {
            revision,
            envelope: RuntimeStateObservedEnvelope::Missing,
        }
    }

    pub(crate) fn is_consistent(&self) -> bool {
        matches!(
            (&self.revision, &self.envelope),
            (
                RuntimeStateSourceRevision::Missing { .. },
                RuntimeStateObservedEnvelope::Missing,
            ) | (
                RuntimeStateSourceRevision::Present { .. },
                RuntimeStateObservedEnvelope::Version(_)
                    | RuntimeStateObservedEnvelope::PresentWithoutReadableVersion,
            )
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeStateRecoveryArtifact {
    pub(crate) path: PathBuf,
    pub(crate) observation: RuntimeStateSourceObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeStatePostClaimPathEffect {
    QuarantinedThenRestored {
        restored_source: RuntimeStateSourceRevision,
    },
    QuarantinedAndRetained {
        recovery_path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeStateObservedPathEffect {
    Untouched,
    PostClaim(RuntimeStatePostClaimPathEffect),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeStateFailurePathEffect {
    Known(RuntimeStateObservedPathEffect),
    UnknownAfterMutation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeStateIoError {
    message: String,
}

impl RuntimeStateIoError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeStateInspectionError {
    message: String,
}

impl RuntimeStateInspectionError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}
