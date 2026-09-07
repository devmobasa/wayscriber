//! Pure normalization at configuration and persistence boundaries.
use std::collections::HashSet;

pub fn clamp_board_rgb(mut rgb: [f64; 3]) -> ([f64; 3], bool) {
    let mut clamped = false;
    for component in &mut rgb {
        if !(0.0..=1.0).contains(component) {
            *component = (*component).clamp(0.0, 1.0);
            clamped = true;
        }
    }
    (rgb, clamped)
}

pub struct BoundaryBoardId {
    pub value: String,
    pub changes: BoardIdChangeSet,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardIdChangeSet {
    pub trimmed: bool,
    pub lowercased: bool,
    pub defaulted_empty: bool,
    pub deduplicated: bool,
}

#[derive(Default)]
pub struct BoundaryBoardIdSet {
    seen: HashSet<String>,
}

impl BoundaryBoardIdSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn normalize_unique(&mut self, raw: &str, index: usize) -> BoundaryBoardId {
        let trimmed = raw.trim();
        let lowercased = trimmed.to_lowercase();
        let mut changes = BoardIdChangeSet {
            trimmed: trimmed != raw,
            lowercased: lowercased != trimmed,
            ..BoardIdChangeSet::default()
        };

        let mut candidate = lowercased;
        if candidate.is_empty() {
            candidate = format!("board-{}", index + 1);
            changes.defaulted_empty = true;
        }

        let base = candidate.clone();
        let mut suffix = 2;
        while self.seen.contains(&candidate) {
            candidate = format!("{base}-{suffix}");
            suffix += 1;
            changes.deduplicated = true;
        }

        self.seen.insert(candidate.clone());
        BoundaryBoardId {
            value: candidate,
            changes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_board_ids_trim_lowercase_default_and_dedupe() {
        let mut ids = BoundaryBoardIdSet::new();

        let first = ids.normalize_unique("  Board-A  ", 0);
        assert_eq!(first.value, "board-a");
        assert_eq!(
            first.changes,
            BoardIdChangeSet {
                trimmed: true,
                lowercased: true,
                defaulted_empty: false,
                deduplicated: false,
            }
        );

        let second = ids.normalize_unique("board-a", 1);
        assert_eq!(second.value, "board-a-2");
        assert!(second.changes.deduplicated);

        let third = ids.normalize_unique("   ", 2);
        assert_eq!(third.value, "board-3");
        assert!(third.changes.defaulted_empty);
    }
}
