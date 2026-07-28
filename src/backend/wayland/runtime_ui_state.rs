//! Wayland-side adapter for seed-guarded runtime UI persistence.
//!
//! The controller owns authority and persistence ordering. This adapter owns
//! toolbar target conversion, preview lifetimes, and the writer transport; UI
//! models and `InputState` never see storage or controller details.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::backend::wayland::state::MoveDragKind;
use crate::config::{
    Config, ToolbarItemOrderGroup, ToolbarSectionVisibility, TopDisplayMode,
    fold_legacy_section_flags, item_visibility_setting, resettable_individual_toolbar_item_ids,
};
use crate::input::InputState;
use crate::runtime_ui_state::*;
use crate::ui::toolbar::model::ToolbarRuntimeUiPersistenceTarget;
use crate::ui::toolbar::{SidePane, ToolbarSideSection};

mod board;
mod coordinator;
mod lifecycle;
mod wayland;

use lifecycle::RuntimeUiLifecycleState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::backend::wayland) struct ToolbarPositionSnapshot {
    pub top: (f64, f64),
    pub side: (f64, f64),
}

#[derive(Debug)]
pub(in crate::backend::wayland) enum ToolbarRuntimeFinish {
    KeepPreview,
    Rollback(PreviewRollbackSnapshot),
    DeferredBehindBarrier,
}

#[derive(Debug, Default)]
pub(in crate::backend::wayland) struct ToolbarSeedRefresh {
    pub item_drag_aborted: bool,
    pub position_drag_aborted: bool,
    pub applied: bool,
}

#[derive(Debug, Default)]
struct ToolbarRuntimeDrain {
    rollbacks: Vec<PreviewRollbackSnapshot>,
    rebuild_live: bool,
    lifecycle_changed: bool,
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct PreparedToolbarMutation {
    target: ToolbarRuntimeUiPersistenceTarget,
    session: RuntimeUiPreviewSession,
}

impl PreparedToolbarMutation {
    pub(in crate::backend::wayland) fn is_persistent_preview(&self) -> bool {
        matches!(&self.session, RuntimeUiPreviewSession::Persistent(_))
    }
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct PreparedBoardPinMutation {
    board_id: String,
    desired: bool,
    session: RuntimeUiPreviewSession,
}

#[derive(Debug)]
struct DeferredBoardPinRestore {
    board_id: String,
    board_identity_generation: crate::input::boards::BoardIdentityGeneration,
    pin_seed: bool,
    pinned: bool,
    authority_epoch: u64,
}

#[derive(Debug)]
struct ActiveItemDrag {
    group: ToolbarItemOrderGroup,
    session: RuntimeUiPreviewSession,
}

#[derive(Debug)]
struct ActivePositionDrag {
    kind: MoveDragKind,
    session: RuntimeUiPreviewSession,
}

#[derive(Debug, Default)]
pub(in crate::backend::wayland) struct UnavailablePersistencePreviews {
    item_drag: Option<PreviewRollbackSnapshot>,
    position_drag: Option<(MoveDragKind, PreviewRollbackSnapshot)>,
}

impl UnavailablePersistencePreviews {
    fn begin_item_drag(&mut self, group: ToolbarItemOrderGroup, input: &InputState) -> bool {
        if self.item_drag.is_some() {
            return false;
        }
        let target = ToolbarRuntimeUiPersistenceTarget::ItemOrder(group);
        let values = match toolbar_values(target, input) {
            Ok(values) => values,
            Err(error) => {
                log::error!(
                    "Unavailable-persistence item drag has invalid rollback values: {error:?}"
                );
                return false;
            }
        };
        self.item_drag = Some(PreviewRollbackSnapshot {
            values: values.values().clone(),
        });
        true
    }

    fn item_drag_update_allowed(&self) -> bool {
        self.item_drag.is_some()
    }

    fn finish_item_drag(&mut self, commit: bool) -> ToolbarRuntimeFinish {
        let Some(rollback) = self.item_drag.take() else {
            return ToolbarRuntimeFinish::KeepPreview;
        };
        if commit {
            ToolbarRuntimeFinish::KeepPreview
        } else {
            ToolbarRuntimeFinish::Rollback(rollback)
        }
    }

    fn begin_position_drag(
        &mut self,
        kind: MoveDragKind,
        positions: ToolbarPositionSnapshot,
    ) -> bool {
        if let Some((active_kind, _)) = &self.position_drag {
            return *active_kind == kind;
        }
        self.position_drag = Some((kind, position_rollback(kind, positions)));
        true
    }

    fn position_drag_update_allowed(&self, kind: MoveDragKind) -> bool {
        self.position_drag
            .as_ref()
            .is_some_and(|(active_kind, _)| *active_kind == kind)
    }

    /// Without a runtime store there is nowhere to persist the drag, so a
    /// committed drag stays process-only and `config.toml` is never touched —
    /// the same contract every other runtime-owned target already has.
    fn finish_position_drag(&mut self, commit: bool) -> ToolbarRuntimeFinish {
        let Some((_, rollback)) = self.position_drag.take() else {
            return ToolbarRuntimeFinish::KeepPreview;
        };
        if commit {
            ToolbarRuntimeFinish::KeepPreview
        } else {
            ToolbarRuntimeFinish::Rollback(rollback)
        }
    }
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct ToolbarRuntimeState {
    controller: RuntimeUiStateController,
    runtime_path: PathBuf,
    lifecycle: RuntimeUiLifecycleState,
    board_pin_seeds: BTreeMap<String, bool>,
    deferred_board_pin_restores: BTreeMap<String, DeferredBoardPinRestore>,
    writer: Option<RuntimeUiStateWriter>,
    pending_writer_command: Option<RuntimeStateWriterCommand>,
    live_rebuild_pending: bool,
    item_drag: Option<ActiveItemDrag>,
    position_drag: Option<ActivePositionDrag>,
}

pub(in crate::backend::wayland) fn apply_toolbar_runtime_rollback(
    input: &mut InputState,
    positions: &mut ToolbarPositionSnapshot,
    rollback: &PreviewRollbackSnapshot,
) {
    for (target, value) in &rollback.values {
        match (target, value) {
            (InteractionSeedTarget::TopPinned, InteractionSeedValue::Bool(value)) => {
                input.toolbar_top_pinned = *value;
            }
            (InteractionSeedTarget::SidePinned, InteractionSeedValue::Bool(value)) => {
                input.toolbar_side_pinned = *value;
            }
            (InteractionSeedTarget::TopMinimized, InteractionSeedValue::Bool(value)) => {
                input.apply_toolbar_set_top_minimized(*value);
            }
            (InteractionSeedTarget::SideMinimized, InteractionSeedValue::Bool(value)) => {
                input.toolbar_side_minimized = *value;
            }
            (InteractionSeedTarget::SidePane, InteractionSeedValue::SidePane(value)) => {
                input.apply_toolbar_set_side_pane(*value);
            }
            (
                InteractionSeedTarget::CollapsedSection(section),
                InteractionSeedValue::Bool(collapsed),
            ) => {
                if *collapsed {
                    input.toolbar_collapsed_side_sections.insert(*section);
                } else {
                    input.toolbar_collapsed_side_sections.remove(section);
                }
            }
            (
                InteractionSeedTarget::ItemVisibility(id),
                InteractionSeedValue::Visibility(setting),
            ) => {
                input.set_toolbar_item_visibility_setting(*id, *setting);
            }
            (InteractionSeedTarget::ItemOrder(group), InteractionSeedValue::ItemOrder(order)) => {
                input.set_toolbar_item_order(*group, order);
            }
            (InteractionSeedTarget::TopPosition, InteractionSeedValue::Position(position)) => {
                positions.top = (position.x.get(), position.y.get());
            }
            (InteractionSeedTarget::SidePosition, InteractionSeedValue::Position(position)) => {
                positions.side = (position.x.get(), position.y.get());
            }
            (InteractionSeedTarget::TopDisplayMode, InteractionSeedValue::TopDisplayMode(mode)) => {
                apply_persisted_top_display_mode(input, *mode);
            }
            (InteractionSeedTarget::BoardPin(board_id), InteractionSeedValue::Bool(pinned)) => {
                input.apply_board_pinned_runtime(board_id, *pinned);
            }
            _ => {}
        }
    }
    input.needs_redraw = true;
}

fn runtime_seeds_from_config(
    config: &Config,
    board_pin_seeds: &BTreeMap<String, bool>,
) -> Result<ValidatedInteractionSeeds> {
    let mut seeds = ValidatedInteractionSeeds::new();
    let mut insert = |target, value| {
        seeds
            .insert(target, value)
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!("invalid runtime UI seed: {error:?}"))
    };
    insert(
        InteractionSeedTarget::TopPinned,
        InteractionSeedValue::Bool(config.ui.toolbar.top_pinned),
    )?;
    insert(
        InteractionSeedTarget::SidePinned,
        InteractionSeedValue::Bool(config.ui.toolbar.side_pinned),
    )?;
    insert(
        InteractionSeedTarget::TopMinimized,
        InteractionSeedValue::Bool(config.ui.toolbar.top_minimized),
    )?;
    insert(
        InteractionSeedTarget::SideMinimized,
        InteractionSeedValue::Bool(config.ui.toolbar.side_minimized),
    )?;
    insert(
        InteractionSeedTarget::SidePane,
        InteractionSeedValue::SidePane(
            SidePane::from_config_id(&config.ui.toolbar.side_active_pane).unwrap_or_default(),
        ),
    )?;
    let collapsed = config
        .ui
        .toolbar
        .collapsed_sections
        .iter()
        .filter_map(|raw| ToolbarSideSection::from_config_id(raw))
        .collect::<BTreeSet<_>>();
    for section in ToolbarSideSection::ALL {
        insert(
            InteractionSeedTarget::CollapsedSection(section),
            InteractionSeedValue::Bool(collapsed.contains(&section)),
        )?;
    }
    let resolved_items = resolved_toolbar_item_seeds(config);
    for id in resettable_individual_toolbar_item_ids() {
        insert(
            InteractionSeedTarget::ItemVisibility(id),
            InteractionSeedValue::Visibility(item_visibility_setting(&resolved_items, id)),
        )?;
    }
    for group in ToolbarItemOrderGroup::ALL {
        insert(
            InteractionSeedTarget::ItemOrder(group),
            InteractionSeedValue::ItemOrder(resolved_items.order.ordered_ids(group).to_vec()),
        )?;
    }
    insert(
        InteractionSeedTarget::TopPosition,
        InteractionSeedValue::Position(
            ToolbarPositionSeed::new(config.ui.toolbar.top_offset, config.ui.toolbar.top_offset_y)
                .context("top toolbar position seed is not finite")?,
        ),
    )?;
    insert(
        InteractionSeedTarget::SidePosition,
        InteractionSeedValue::Position(
            ToolbarPositionSeed::new(
                config.ui.toolbar.side_offset_x,
                config.ui.toolbar.side_offset,
            )
            .context("side toolbar position seed is not finite")?,
        ),
    )?;
    insert(
        InteractionSeedTarget::TopDisplayMode,
        InteractionSeedValue::TopDisplayMode(PersistedTopDisplayMode::from_display_mode(
            config.ui.toolbar.top_display_mode,
        )),
    )?;
    for (board_id, pinned) in board_pin_seeds {
        insert(
            InteractionSeedTarget::BoardPin(board_id.clone()),
            InteractionSeedValue::Bool(*pinned),
        )?;
    }
    Ok(seeds)
}

fn board_pin_seeds_from_input(input: &InputState) -> BTreeMap<String, bool> {
    input
        .boards
        .pin_seed_entries()
        .map(|(id, pinned)| (id.to_string(), pinned))
        .collect()
}

fn retain_stored_board_pin_seeds_for_session_restore(
    board_pin_seeds: &mut BTreeMap<String, bool>,
    inspection: &RuntimeUiStateInspection,
) {
    let Some(wire) = inspection.supported_wire.as_ref() else {
        return;
    };
    for (target, runtime_override) in wire.model.iter() {
        let (InteractionSeedTarget::BoardPin(board_id), InteractionSeedValue::Bool(stored_seed)) =
            (target, &runtime_override.seed)
        else {
            continue;
        };
        board_pin_seeds
            .entry(board_id.clone())
            .or_insert(*stored_seed);
    }
}

fn resolved_toolbar_item_seeds(config: &Config) -> crate::config::ResolvedToolbarItems {
    let toolbar = &config.ui.toolbar;
    let mut items = toolbar.items.clone();
    let mut legacy = ToolbarSectionVisibility {
        show_actions_section: toolbar.show_actions_section,
        show_actions_advanced: toolbar.show_actions_advanced,
        show_zoom_actions: toolbar.show_zoom_actions,
        show_pages_section: toolbar.show_pages_section,
        show_boards_section: toolbar.show_boards_section,
        show_presets: toolbar.show_presets,
        show_step_section: toolbar.show_step_section,
        show_text_controls: toolbar.show_text_controls,
        show_settings_section: toolbar.show_settings_section,
    };
    legacy.apply_mode_override(toolbar.mode_overrides.for_mode(toolbar.layout_mode));
    fold_legacy_section_flags(
        &legacy,
        toolbar.layout_mode,
        &toolbar.mode_overrides,
        &mut items,
    );
    items.resolved()
}

fn toolbar_values(
    target: ToolbarRuntimeUiPersistenceTarget,
    input: &InputState,
) -> std::result::Result<RuntimeUiMutationValues, MutationShapeError> {
    use ToolbarRuntimeUiPersistenceTarget as Target;
    match target {
        Target::TopPinned => RuntimeUiMutationValues::one(
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(input.toolbar_top_pinned),
        ),
        Target::SidePinned => RuntimeUiMutationValues::one(
            InteractionSeedTarget::SidePinned,
            InteractionSeedValue::Bool(input.toolbar_side_pinned),
        ),
        Target::TopMinimized => RuntimeUiMutationValues::one(
            InteractionSeedTarget::TopMinimized,
            InteractionSeedValue::Bool(input.toolbar_top_minimized),
        ),
        Target::SideMinimized => RuntimeUiMutationValues::one(
            InteractionSeedTarget::SideMinimized,
            InteractionSeedValue::Bool(input.toolbar_side_minimized),
        ),
        Target::SidePane => RuntimeUiMutationValues::one(
            InteractionSeedTarget::SidePane,
            InteractionSeedValue::SidePane(input.toolbar_side_pane),
        ),
        Target::TopDisplayMode => top_display_mode_values(input.toolbar_top_display_mode, input),
        Target::CollapsedSection(section) => RuntimeUiMutationValues::one(
            InteractionSeedTarget::CollapsedSection(section),
            InteractionSeedValue::Bool(input.toolbar_collapsed_side_sections.contains(&section)),
        ),
        Target::ItemVisibility { id, .. } => RuntimeUiMutationValues::one(
            InteractionSeedTarget::ItemVisibility(id),
            InteractionSeedValue::Visibility(item_visibility_setting(
                &input.resolved_toolbar_items,
                id,
            )),
        ),
        Target::ItemOrder(group) => RuntimeUiMutationValues::one(
            InteractionSeedTarget::ItemOrder(group),
            InteractionSeedValue::ItemOrder(
                input
                    .resolved_toolbar_items
                    .order
                    .ordered_ids(group)
                    .to_vec(),
            ),
        ),
        Target::ResetItemVisibility => {
            RuntimeUiMutationValues::batch(resettable_individual_toolbar_item_ids().map(|id| {
                (
                    InteractionSeedTarget::ItemVisibility(id),
                    InteractionSeedValue::Visibility(item_visibility_setting(
                        &input.resolved_toolbar_items,
                        id,
                    )),
                )
            }))
        }
    }
}

/// The persisted form of a live top-display mode.
///
/// While presenter mode owns the top strip, the saved pre-presenter mode wins
/// over the temporary live mapping; `Hidden` always folds to `Full` because a
/// hidden strip is runtime-only and `top_pinned` governs startup.
fn persisted_top_display_mode(
    current: TopDisplayMode,
    presenter_restore: Option<TopDisplayMode>,
) -> PersistedTopDisplayMode {
    PersistedTopDisplayMode::from_display_mode(presenter_restore.unwrap_or(current))
}

fn top_display_mode_values(
    mode: TopDisplayMode,
    input: &InputState,
) -> std::result::Result<RuntimeUiMutationValues, MutationShapeError> {
    let presenter_restore = input
        .presenter_restore
        .as_ref()
        .and_then(|restore| restore.toolbar_top_display_mode);
    RuntimeUiMutationValues::one(
        InteractionSeedTarget::TopDisplayMode,
        InteractionSeedValue::TopDisplayMode(persisted_top_display_mode(mode, presenter_restore)),
    )
}

/// Apply a persisted display mode to the live UI.
///
/// While presenter mode holds the strip in its own mapping, the runtime value
/// belongs to what presenter will restore, not to the live strip; anywhere
/// else it is the live strip's mode.
fn apply_persisted_top_display_mode(input: &mut InputState, mode: PersistedTopDisplayMode) {
    let mode = mode.display_mode();
    if let Some(restore) = input.presenter_restore.as_mut()
        && restore.toolbar_top_display_mode.is_some()
    {
        restore.toolbar_top_display_mode = Some(mode);
        return;
    }
    if input.toolbar_top_display_mode != mode {
        input.set_top_display_mode(mode);
    }
}

fn apply_live_toolbar_state(
    input: &mut InputState,
    live: &RuntimeUiLiveState,
    include: impl Fn(&InteractionSeedTarget) -> bool,
) {
    let bool_value = |target| match live.get(&target) {
        Some(InteractionSeedValue::Bool(value)) => Some(*value),
        _ => None,
    };
    // Applied before `TopMinimized` so that entering micro (which clears the
    // minimized flag) cannot drop a minimized state that is also being
    // restored in the same pass.
    if include(&InteractionSeedTarget::TopDisplayMode)
        && let Some(InteractionSeedValue::TopDisplayMode(mode)) =
            live.get(&InteractionSeedTarget::TopDisplayMode)
    {
        apply_persisted_top_display_mode(input, *mode);
    }
    if include(&InteractionSeedTarget::TopPinned)
        && let Some(value) = bool_value(InteractionSeedTarget::TopPinned)
    {
        input.toolbar_top_pinned = value;
    }
    if include(&InteractionSeedTarget::SidePinned)
        && let Some(value) = bool_value(InteractionSeedTarget::SidePinned)
    {
        input.toolbar_side_pinned = value;
    }
    if include(&InteractionSeedTarget::TopMinimized)
        && let Some(value) = bool_value(InteractionSeedTarget::TopMinimized)
    {
        input.apply_toolbar_set_top_minimized(value);
    }
    if include(&InteractionSeedTarget::SideMinimized)
        && let Some(value) = bool_value(InteractionSeedTarget::SideMinimized)
    {
        input.toolbar_side_minimized = value;
    }
    if include(&InteractionSeedTarget::SidePane)
        && let Some(InteractionSeedValue::SidePane(pane)) =
            live.get(&InteractionSeedTarget::SidePane)
    {
        input.apply_toolbar_set_side_pane(*pane);
    }
    for section in ToolbarSideSection::ALL {
        let target = InteractionSeedTarget::CollapsedSection(section);
        if !include(&target) {
            continue;
        }
        if bool_value(target) == Some(true) {
            input.toolbar_collapsed_side_sections.insert(section);
        } else {
            input.toolbar_collapsed_side_sections.remove(&section);
        }
    }
    for id in resettable_individual_toolbar_item_ids() {
        let target = InteractionSeedTarget::ItemVisibility(id);
        if !include(&target) {
            continue;
        }
        if let Some(InteractionSeedValue::Visibility(setting)) = live.get(&target) {
            input.set_toolbar_item_visibility_setting(id, *setting);
        }
    }
    for group in ToolbarItemOrderGroup::ALL {
        let target = InteractionSeedTarget::ItemOrder(group);
        if include(&target)
            && let Some(InteractionSeedValue::ItemOrder(order)) = live.get(&target)
        {
            input.set_toolbar_item_order(group, order);
        }
    }
}

fn apply_live_toolbar_positions(
    positions: &mut ToolbarPositionSnapshot,
    live: &RuntimeUiLiveState,
    include: impl Fn(&InteractionSeedTarget) -> bool,
) {
    if include(&InteractionSeedTarget::TopPosition)
        && let Some(InteractionSeedValue::Position(position)) =
            live.get(&InteractionSeedTarget::TopPosition)
    {
        positions.top = (position.x.get(), position.y.get());
    }
    if include(&InteractionSeedTarget::SidePosition)
        && let Some(InteractionSeedValue::Position(position)) =
            live.get(&InteractionSeedTarget::SidePosition)
    {
        positions.side = (position.x.get(), position.y.get());
    }
}

fn apply_live_board_state(
    input: &mut InputState,
    live: &RuntimeUiLiveState,
    include: impl Fn(&InteractionSeedTarget) -> bool,
) {
    let board_ids = input
        .boards
        .board_states()
        .iter()
        .map(|board| board.spec.id.clone())
        .collect::<Vec<_>>();
    for board_id in board_ids {
        let target = InteractionSeedTarget::BoardPin(board_id.clone());
        if !include(&target) {
            continue;
        }
        if let Some(InteractionSeedValue::Bool(pinned)) = live.get(&target) {
            input.apply_board_pinned_runtime(&board_id, *pinned);
        }
    }
}

fn runtime_preview_authority(
    session: &RuntimeUiPreviewSession,
) -> (ControllerId, u64, &[SeedGuard]) {
    match session {
        RuntimeUiPreviewSession::Persistent(session) => (
            session.permit.controller_id,
            session.permit.authority_epoch,
            &session.permit.guards,
        ),
        RuntimeUiPreviewSession::LiveOnly(session) => (
            session.guard.controller_id,
            session.guard.authority_epoch,
            &session.guard.guards,
        ),
    }
}

/// One of the two toolbar positions a move drag can write.
///
/// Position drags only ever touch these two overrides. Naming them as their own
/// type keeps the seed target and the snapshot field that feeds it in lockstep,
/// so neither has to be recovered from the other at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionDragTarget {
    Top,
    Side,
}

impl PositionDragTarget {
    fn offsets(self, positions: ToolbarPositionSnapshot) -> (f64, f64) {
        match self {
            Self::Top => positions.top,
            Self::Side => positions.side,
        }
    }
}

impl From<PositionDragTarget> for InteractionSeedTarget {
    fn from(target: PositionDragTarget) -> Self {
        match target {
            PositionDragTarget::Top => Self::TopPosition,
            PositionDragTarget::Side => Self::SidePosition,
        }
    }
}

/// The override targets a toolbar drag of `kind` may write.
///
/// A side drag can change whether the side palette overlaps the top strip, and
/// drag completion reconciles the top strip's X offset against that new base,
/// so it owns both position targets in one mutation scope.
fn position_drag_targets(kind: MoveDragKind) -> &'static [PositionDragTarget] {
    match kind {
        MoveDragKind::Top => &[PositionDragTarget::Top],
        MoveDragKind::Side => &[PositionDragTarget::Top, PositionDragTarget::Side],
    }
}

fn position_seed_targets(kind: MoveDragKind) -> impl Iterator<Item = InteractionSeedTarget> {
    position_drag_targets(kind)
        .iter()
        .copied()
        .map(InteractionSeedTarget::from)
}

fn position_rollback(
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
    PreviewRollbackSnapshot { values }
}

/// The committed values for a finished drag, or `None` when any guarded offset
/// is not finite and therefore cannot be stored as an override.
fn position_values(
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

fn rejected_source_mutation(
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

#[cfg(test)]
mod tests;
