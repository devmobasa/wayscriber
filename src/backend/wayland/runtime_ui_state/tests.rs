use super::*;

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::config::{ToolbarItemsConfig, toolbar_item_ids as ids};
use crate::input::state::test_support::make_test_input_state;
use crate::ui::toolbar::{RuntimeUiPersistenceMode, RuntimeUiPersistenceSnapshot, ToolbarEvent};

mod board_pin_resets;
mod board_pins;
mod drag_previews;
mod layout_state;
mod preference_actions;
mod preferences;
mod recovery;
mod visibility;
mod visibility_recovery;

fn input_from_config(config: &Config) -> InputState {
    let mut input = make_test_input_state();
    input.boards = crate::input::boards::BoardManager::from_config(config.resolved_boards());
    input.init_toolbar_from_config(&config.ui.toolbar);
    input
}

fn test_runtime(config: &Config, path: &Path) -> ToolbarRuntimeState {
    let runtime = test_runtime_allow_startup_incident(config, path);
    assert!(!matches!(
        runtime.persistence_snapshot().mode,
        RuntimeUiPersistenceMode::Unhealthy
    ));
    runtime
}

fn test_runtime_allow_startup_incident(config: &Config, path: &Path) -> ToolbarRuntimeState {
    fs::create_dir_all(path.parent().expect("runtime parent")).unwrap();
    let store = RuntimeUiStateStore::new(path);
    let mut board_pin_seeds = board_pin_seeds_from_input(&input_from_config(config));
    let inspection = store.inspect().unwrap();
    retain_stored_board_pin_seeds_for_session_restore(&mut board_pin_seeds, &inspection);
    let bootstrap = inspection
        .into_controller_bootstrap(runtime_seeds_from_config(config, &board_pin_seeds).unwrap());
    let mut runtime = ToolbarRuntimeState {
        controller: bootstrap.controller,
        runtime_path: path.to_path_buf(),
        lifecycle: RuntimeUiLifecycleState::startup(bootstrap.startup_incident),
        board_pin_seeds,
        deferred_board_pin_restores: BTreeMap::new(),
        writer: Some(RuntimeUiStateWriter::spawn(store).unwrap()),
        pending_writer_command: None,
        live_rebuild_pending: false,
        item_drag: None,
        position_drag: None,
    };
    runtime.dispatch_writer_command();
    runtime
}

fn controller_only_runtime(config: &Config, path: &Path) -> ToolbarRuntimeState {
    let mut board_pin_seeds = board_pin_seeds_from_input(&input_from_config(config));
    let inspection = RuntimeUiStateStore::new(path).inspect().unwrap();
    retain_stored_board_pin_seeds_for_session_restore(&mut board_pin_seeds, &inspection);
    let bootstrap = inspection
        .into_controller_bootstrap(runtime_seeds_from_config(config, &board_pin_seeds).unwrap());
    ToolbarRuntimeState {
        controller: bootstrap.controller,
        runtime_path: path.to_path_buf(),
        lifecycle: RuntimeUiLifecycleState::startup(bootstrap.startup_incident),
        board_pin_seeds,
        deferred_board_pin_restores: BTreeMap::new(),
        writer: None,
        pending_writer_command: None,
        live_rebuild_pending: false,
        item_drag: None,
        position_drag: None,
    }
}

fn settle_runtime(runtime: &mut ToolbarRuntimeState) -> ToolbarRuntimeDrain {
    let mut combined = ToolbarRuntimeDrain::default();
    for _ in 0..400 {
        let drain = runtime.drain_writer_completions();
        combined.rollbacks.extend(drain.rollbacks);
        combined.rebuild_live |= drain.rebuild_live;
        combined.lifecycle_changed |= drain.lifecycle_changed;
        let pipeline = runtime.controller.pipeline();
        if pipeline.settled_through() == pipeline.latest_accepted()
            && !pipeline.has_source_mutation_in_flight()
            && runtime.pending_writer_command.is_none()
        {
            return combined;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("runtime writer did not settle");
}

fn wait_for_runtime_mode(
    runtime: &mut ToolbarRuntimeState,
    expected: RuntimeUiPersistenceMode,
) -> RuntimeUiPersistenceSnapshot {
    for _ in 0..800 {
        runtime.drain_writer_completions();
        let snapshot = runtime.persistence_snapshot();
        if snapshot.mode == expected {
            return snapshot;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!(
        "runtime UI lifecycle did not reach {expected:?}; last state: {:?}",
        runtime.persistence_snapshot()
    );
}

fn apply_finish(
    input: &mut InputState,
    positions: &mut ToolbarPositionSnapshot,
    finish: ToolbarRuntimeFinish,
) {
    if let ToolbarRuntimeFinish::Rollback(rollback) = finish {
        apply_toolbar_runtime_rollback(
            &crate::ui_text::UiTextEngine::default(),
            input,
            positions,
            &rollback,
        );
    }
}

fn board_pinned(input: &InputState, board_id: &str) -> bool {
    input
        .boards
        .board_states()
        .iter()
        .find(|board| board.spec.id == board_id)
        .unwrap_or_else(|| panic!("missing test board {board_id}"))
        .spec
        .pinned
}

fn commit_board_pin_toggle(
    runtime: &mut ToolbarRuntimeState,
    config: &Config,
    input: &mut InputState,
    board_id: &str,
) -> ToolbarRuntimeFinish {
    let current = board_pinned(input, board_id);
    let seed = input.boards.pin_seed(board_id).expect("board pin seed");
    let prepared = runtime
        .begin_board_pin_toggle(config, board_id.to_string(), seed, current)
        .expect("board pin permit");
    assert!(input.apply_board_pinned_runtime(board_id, prepared.desired));
    runtime.finish_board_pin_toggle(prepared, true)
}

fn config_positions(config: &Config) -> ToolbarPositionSnapshot {
    ToolbarPositionSnapshot {
        top: (config.ui.toolbar.top_offset, config.ui.toolbar.top_offset_y),
    }
}

fn stored_position(
    runtime: &ToolbarRuntimeState,
    target: InteractionSeedTarget,
) -> Option<(f64, f64)> {
    match runtime
        .controller
        .model()
        .get(&target)
        .map(|entry| &entry.value)
    {
        Some(InteractionSeedValue::Position(position)) => {
            Some((position.x.get(), position.y.get()))
        }
        _ => None,
    }
}

fn stored_bool(runtime: &ToolbarRuntimeState, target: InteractionSeedTarget) -> Option<bool> {
    match runtime
        .controller
        .model()
        .get(&target)
        .map(|entry| &entry.value)
    {
        Some(InteractionSeedValue::Bool(value)) => Some(*value),
        _ => None,
    }
}

fn stored_display_mode(runtime: &ToolbarRuntimeState) -> Option<PersistedTopDisplayMode> {
    match runtime
        .controller
        .model()
        .get(&InteractionSeedTarget::TopDisplayMode)
        .map(|entry| &entry.value)
    {
        Some(InteractionSeedValue::TopDisplayMode(mode)) => Some(*mode),
        _ => None,
    }
}

fn commit_display_mode(
    runtime: &mut ToolbarRuntimeState,
    input: &mut InputState,
    mode: crate::config::TopDisplayMode,
) -> ToolbarRuntimeFinish {
    let target = ToolbarRuntimeUiPersistenceTarget::TopDisplayMode;
    let prepared = runtime
        .begin_toolbar_mutation(target, input)
        .expect("display mode permit");
    input.set_top_display_mode_with_engine(&crate::ui_text::UiTextEngine::default(), mode);
    runtime.finish_toolbar_mutation(prepared, true, input)
}

/// The pre-toggle pins as the visibility toggle's write path batches them.
fn pins_rollback_values(top: bool) -> RuntimeUiMutationValues {
    RuntimeUiMutationValues::batch([(
        InteractionSeedTarget::TopPinned,
        InteractionSeedValue::Bool(top),
    )])
    .expect("distinct pin targets batch")
}

/// The pre-toggle pins as the visibility toggle's rollback snapshot carries
/// them (visibility itself is never persisted, so it is not in there).
fn pins_rollback(top: bool) -> PreviewRollbackSnapshot {
    PreviewRollbackSnapshot {
        values: BTreeMap::from([(
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(top),
        )]),
        derive_toolbar_visibility_from_pins: true,
    }
}

fn section_toggle(flag: crate::config::ToolbarSectionFlag, show: bool) -> ToolbarEvent {
    use crate::config::ToolbarSectionFlag as Flag;
    match flag {
        Flag::Actions => ToolbarEvent::ToggleActionsSection(show),
        Flag::ActionsAdvanced => ToolbarEvent::ToggleActionsAdvanced(show),
        Flag::ZoomActions => ToolbarEvent::ToggleZoomActions(show),
        Flag::Pages => ToolbarEvent::TogglePagesSection(show),
        Flag::Boards => ToolbarEvent::ToggleBoardsSection(show),
        Flag::Presets => ToolbarEvent::TogglePresets(show),
        Flag::StepSection => ToolbarEvent::ToggleStepSection(show),
        Flag::TextControls => ToolbarEvent::ToggleTextControls(show),
    }
}

mod text_geometry;
