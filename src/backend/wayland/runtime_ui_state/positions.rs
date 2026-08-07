use super::*;

/// The toolbar position a move drag can write.
///
/// Naming it as its own type keeps the seed target and the snapshot field that
/// feeds it in lockstep, so neither has to be recovered from the other at
/// runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionDragTarget {
    Top,
}

impl PositionDragTarget {
    fn offsets(self, positions: ToolbarPositionSnapshot) -> (f64, f64) {
        match self {
            Self::Top => positions.top,
        }
    }
}

impl From<PositionDragTarget> for InteractionSeedTarget {
    fn from(target: PositionDragTarget) -> Self {
        match target {
            PositionDragTarget::Top => Self::TopPosition,
        }
    }
}

/// The override targets a toolbar drag of `kind` may write.
fn position_drag_targets(kind: MoveDragKind) -> &'static [PositionDragTarget] {
    match kind {
        MoveDragKind::Top => &[PositionDragTarget::Top],
    }
}

pub(super) fn position_seed_targets(
    kind: MoveDragKind,
) -> impl Iterator<Item = InteractionSeedTarget> {
    position_drag_targets(kind)
        .iter()
        .copied()
        .map(InteractionSeedTarget::from)
}

pub(super) fn position_rollback(
    kind: MoveDragKind,
    positions: ToolbarPositionSnapshot,
) -> PreviewRollbackSnapshot {
    let mut values = std::collections::BTreeMap::new();
    for target in position_drag_targets(kind) {
        let (x, y) = target.offsets(positions);
        if let Some(position) = ToolbarPositionSeed::new(x, y) {
            values.insert(
                InteractionSeedTarget::from(*target),
                InteractionSeedValue::Position(position),
            );
        }
    }
    PreviewRollbackSnapshot {
        values,
        derive_toolbar_visibility_from_pins: false,
    }
}

/// The committed values for a finished drag, or `None` when any guarded offset
/// is not finite and therefore cannot be stored as an override.
pub(super) fn position_values(
    kind: MoveDragKind,
    positions: ToolbarPositionSnapshot,
) -> Option<RuntimeUiMutationValues> {
    let mut values = Vec::new();
    for target in position_drag_targets(kind) {
        let (x, y) = target.offsets(positions);
        let position = ToolbarPositionSeed::new(x, y)?;
        values.push((
            InteractionSeedTarget::from(*target),
            InteractionSeedValue::Position(position),
        ));
    }
    RuntimeUiMutationValues::batch(values).ok()
}

pub(super) fn rejected_source_mutation(
    id: SourceMutationId,
    error: RuntimeStateIoError,
) -> SourceMutationResult {
    SourceMutationResult::Failed {
        id,
        error,
        active: None,
        recovery_artifacts: Vec::new(),
        path_effect: RuntimeStateFailurePathEffect::Known(
            RuntimeStateObservedPathEffect::Untouched,
        ),
    }
}
