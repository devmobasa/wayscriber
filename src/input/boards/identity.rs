use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_BOARD_IDENTITY_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardIdentityGeneration(pub u64);

impl BoardIdentityGeneration {
    pub(crate) fn fresh() -> Self {
        Self(NEXT_BOARD_IDENTITY_GENERATION.fetch_add(1, Ordering::Relaxed))
    }
}

pub use crate::domain::{BoardIdChangeSet, BoundaryBoardId, BoundaryBoardIdSet};
