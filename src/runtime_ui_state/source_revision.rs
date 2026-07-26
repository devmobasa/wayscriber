use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeStateFileIdentity {
    pub(crate) device: u64,
    pub(crate) inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeStateResolvedParent {
    path: PathBuf,
    identity: RuntimeStateFileIdentity,
}

impl RuntimeStateResolvedParent {
    pub(crate) fn new(path: PathBuf, identity: RuntimeStateFileIdentity) -> Self {
        Self { path, identity }
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub(crate) fn identity(&self) -> RuntimeStateFileIdentity {
        self.identity
    }
}

/// Exact identity of the managed runtime-state path after resolving links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeStatePathIdentity {
    source_path: PathBuf,
    followed_links: Box<[(PathBuf, PathBuf)]>,
    resolved_parent: Option<RuntimeStateResolvedParent>,
}

impl RuntimeStatePathIdentity {
    pub(crate) fn new(
        source_path: PathBuf,
        followed_links: impl Into<Box<[(PathBuf, PathBuf)]>>,
    ) -> Self {
        Self {
            source_path,
            followed_links: followed_links.into(),
            resolved_parent: None,
        }
    }

    pub(crate) fn direct(source_path: impl Into<PathBuf>) -> Self {
        Self::new(source_path.into(), Vec::new().into_boxed_slice())
    }

    pub(crate) fn observed(
        source_path: impl Into<PathBuf>,
        resolved_parent: RuntimeStateResolvedParent,
    ) -> Self {
        Self {
            source_path: source_path.into(),
            followed_links: Vec::new().into_boxed_slice(),
            resolved_parent: Some(resolved_parent),
        }
    }

    pub(crate) fn source_path(&self) -> &std::path::Path {
        &self.source_path
    }

    pub(crate) fn followed_links(&self) -> &[(PathBuf, PathBuf)] {
        &self.followed_links
    }

    pub(crate) fn resolved_parent(&self) -> Option<&RuntimeStateResolvedParent> {
        self.resolved_parent.as_ref()
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
        bytes: Box<[u8]>,
        file_identity: Option<RuntimeStateFileIdentity>,
    },
}

impl RuntimeStateSourceRevision {
    pub(crate) fn missing(path: RuntimeStatePathIdentity) -> Self {
        Self::Missing { path }
    }

    pub(crate) fn present(path: RuntimeStatePathIdentity, bytes: impl Into<Box<[u8]>>) -> Self {
        Self::Present {
            path,
            bytes: bytes.into(),
            file_identity: None,
        }
    }

    pub(crate) fn present_observed(
        path: RuntimeStatePathIdentity,
        bytes: impl Into<Box<[u8]>>,
        file_identity: RuntimeStateFileIdentity,
    ) -> Self {
        Self::Present {
            path,
            bytes: bytes.into(),
            file_identity: Some(file_identity),
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

    pub(crate) fn file_identity(&self) -> Option<RuntimeStateFileIdentity> {
        match self {
            Self::Missing { .. } => None,
            Self::Present { file_identity, .. } => *file_identity,
        }
    }

    pub(crate) fn with_path_identity(&self, path: RuntimeStatePathIdentity) -> Self {
        match self {
            Self::Missing { .. } => Self::Missing { path },
            Self::Present {
                bytes,
                file_identity,
                ..
            } => Self::Present {
                path,
                bytes: bytes.clone(),
                file_identity: *file_identity,
            },
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
            Self::Present {
                path,
                bytes,
                file_identity,
            } => formatter
                .debug_struct("Present")
                .field("path", path)
                .field("byte_len", &bytes.len())
                .field("file_identity", file_identity)
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

#[cfg(test)]
mod ownership_tests {
    use super::*;

    #[test]
    fn cloned_source_revision_owns_independent_bytes_and_link_storage() {
        let path = RuntimeStatePathIdentity::new(
            PathBuf::from("/tmp/runtime-ui.toml"),
            vec![(
                PathBuf::from("/tmp/runtime-ui.toml"),
                PathBuf::from("/tmp/runtime-ui-target.toml"),
            )]
            .into_boxed_slice(),
        );
        let original = RuntimeStateSourceRevision::present(path, vec![1, 2, 3].into_boxed_slice());
        let cloned = original.clone();

        let (original_path, original_bytes, cloned_path, cloned_bytes) = match (&original, &cloned)
        {
            (
                RuntimeStateSourceRevision::Present {
                    path: original_path,
                    bytes: original_bytes,
                    ..
                },
                RuntimeStateSourceRevision::Present {
                    path: cloned_path,
                    bytes: cloned_bytes,
                    ..
                },
            ) => Some((original_path, original_bytes, cloned_path, cloned_bytes)),
            _ => None,
        }
        .expect("fixture explicitly constructs and clones a present source revision");

        assert_eq!(original_bytes, cloned_bytes);
        assert!(!std::ptr::eq(
            original_bytes.as_ptr(),
            cloned_bytes.as_ptr()
        ));
        assert_eq!(original_path.followed_links(), cloned_path.followed_links());
        assert!(!std::ptr::eq(
            original_path.followed_links().as_ptr(),
            cloned_path.followed_links().as_ptr()
        ));
    }
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
