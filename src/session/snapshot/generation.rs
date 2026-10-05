//! Write order of session artifacts.
//!
//! Every "which artifact was written later" decision of the session loader and
//! of the post-save cleanup goes through this module. An artifact carries a
//! save generation once a generation-aware build writes it. When either side of
//! a comparison has none, the decision falls back to the exact mtime rule the
//! loader used before generations existed.

use std::fs;
use std::time::SystemTime;

mod marker;
mod probe;
mod view;

pub(super) use marker::{MarkerKind, marker_record_bytes};
pub(super) use probe::payload_prefix_generation;
pub(super) use view::{ArtifactSetView, UnreadablePolicy};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Generation {
    /// Written by a generation-aware save of this artifact set.
    Known(u64),
    /// Written by an older build, re-saved by one, out of range, or unreadable.
    Unknown,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ArtifactStamp {
    pub(super) generation: Generation,
    /// Last-modified time. Only the legacy fallback reads it.
    pub(super) modified: Option<SystemTime>,
}

impl ArtifactStamp {
    /// A stamp for an artifact whose generation has not been read.
    pub(super) fn without_generation(metadata: &fs::Metadata) -> Self {
        Self {
            generation: Generation::Unknown,
            modified: metadata.modified().ok(),
        }
    }
}

/// How a comparison behaves when either side has no generation. Each variant
/// reproduces one rule from before generations existed exactly.
#[derive(Clone, Copy, Debug)]
pub(super) enum LegacyOrder {
    /// `a > b`; an unreadable mtime answers false.
    Strict,
    /// `a >= b`; an unreadable mtime answers true.
    NonStrictTrue,
}

/// Whether `a` holds bytes from a later save than `b`.
pub(super) fn written_after(a: ArtifactStamp, b: ArtifactStamp, legacy: LegacyOrder) -> bool {
    match (a.generation, b.generation) {
        (Generation::Known(a), Generation::Known(b)) => a > b,
        _ => match (a.modified, b.modified, legacy) {
            (Some(a), Some(b), LegacyOrder::Strict) => a > b,
            (Some(a), Some(b), LegacyOrder::NonStrictTrue) => a >= b,
            (_, _, LegacyOrder::Strict) => false,
            (_, _, LegacyOrder::NonStrictTrue) => true,
        },
    }
}

/// Whether the clear marker `clear` hides `artifact`.
pub(super) fn cleared_by(artifact: ArtifactStamp, clear: ArtifactStamp) -> bool {
    match (artifact.generation, clear.generation) {
        (Generation::Known(artifact), Generation::Known(clear)) => artifact < clear,
        _ => !written_after(artifact, clear, LegacyOrder::Strict),
    }
}

pub(super) const MAX_GENERATION: u64 = (1 << 53) - 1;

impl Generation {
    pub(super) fn from_raw(value: u64) -> Self {
        if (1..=MAX_GENERATION).contains(&value) {
            Self::Known(value)
        } else {
            Self::Unknown
        }
    }

    pub(super) fn known(self) -> Option<u64> {
        match self {
            Self::Known(value) => Some(value),
            Self::Unknown => None,
        }
    }
}

#[cfg(test)]
mod tests;
