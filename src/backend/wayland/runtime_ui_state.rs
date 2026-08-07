//! Wayland-side adapter for seed-guarded runtime UI persistence.
//!
//! The controller owns authority and persistence ordering. This adapter owns
//! toolbar target conversion, preview lifetimes, and the writer transport; UI
//! models and `InputState` never see storage or controller details.

use std::collections::BTreeMap;
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

mod board;
mod coordinator;
mod lifecycle;
mod live_state;
mod positions;
mod rollback;
mod seeds;
mod wayland;

use live_state::{
    apply_live_board_state, apply_live_toolbar_positions, apply_live_toolbar_state,
    apply_persisted_top_display_mode, runtime_preview_authority, top_display_mode_values,
};
pub(in crate::backend::wayland) use live_state::{user_click_highlight_enabled, user_tool_preview};
use positions::{
    position_rollback, position_seed_targets, position_values, rejected_source_mutation,
};
pub(in crate::backend::wayland) use rollback::apply_toolbar_runtime_rollback;
use seeds::{
    board_pin_seeds_from_input, retain_stored_board_pin_seeds_for_session_restore,
    runtime_seeds_from_config, toolbar_values,
};
pub(in crate::backend::wayland) use seeds::{click_highlight_values, single_bool_seed_target};

use lifecycle::RuntimeUiLifecycleState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::backend::wayland) struct ToolbarPositionSnapshot {
    pub top: (f64, f64),
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
            derive_toolbar_visibility_from_pins: false,
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

#[cfg(test)]
mod tests;
