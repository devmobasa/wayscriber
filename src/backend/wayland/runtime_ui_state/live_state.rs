use super::*;

/// Presenter mode can hold the tool preview hidden while it runs, so what
/// persists is the value presenter will restore -- the user's own.
pub(in crate::backend::wayland) fn user_tool_preview(input: &InputState) -> bool {
    input
        .presenter_restore
        .as_ref()
        .and_then(|restore| restore.show_tool_preview)
        .unwrap_or(input.show_tool_preview)
}

/// Presenter mode forces the click highlight on while it runs, so what
/// persists is the value presenter will restore -- the user's own.
pub(in crate::backend::wayland) fn user_click_highlight_enabled(input: &InputState) -> bool {
    input
        .presenter_restore
        .as_ref()
        .and_then(|restore| restore.click_highlight_enabled)
        .unwrap_or_else(|| input.click_highlight_enabled())
}

/// While presenter mode owns the top strip, the saved pre-presenter mode wins
/// over the temporary live mapping; `Hidden` always folds to `Full` because a
/// hidden strip is runtime-only and `top_pinned` governs startup.
fn persisted_top_display_mode(
    current: TopDisplayMode,
    presenter_restore: Option<TopDisplayMode>,
) -> PersistedTopDisplayMode {
    PersistedTopDisplayMode::from_display_mode(presenter_restore.unwrap_or(current))
}

pub(super) fn top_display_mode_values(
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
pub(super) fn apply_persisted_top_display_mode(
    input: &mut InputState,
    mode: PersistedTopDisplayMode,
) {
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

pub(super) fn apply_live_toolbar_state(
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
    if include(&InteractionSeedTarget::StatusBar)
        && let Some(value) = bool_value(InteractionSeedTarget::StatusBar)
    {
        input.show_status_bar = value;
    }
    if include(&InteractionSeedTarget::StatusBoardBadge)
        && let Some(value) = bool_value(InteractionSeedTarget::StatusBoardBadge)
    {
        input.show_status_board_badge = value;
    }
    if include(&InteractionSeedTarget::StatusPageBadge)
        && let Some(value) = bool_value(InteractionSeedTarget::StatusPageBadge)
    {
        input.show_status_page_badge = value;
    }
    if include(&InteractionSeedTarget::FloatingBadgeAlways)
        && let Some(value) = bool_value(InteractionSeedTarget::FloatingBadgeAlways)
    {
        input.show_floating_badge_always = value;
    }
    if include(&InteractionSeedTarget::ToolbarIcons)
        && let Some(value) = bool_value(InteractionSeedTarget::ToolbarIcons)
    {
        input.toolbar_use_icons = value;
    }
    if include(&InteractionSeedTarget::ToolbarMoreColors)
        && let Some(value) = bool_value(InteractionSeedTarget::ToolbarMoreColors)
    {
        input.show_more_colors = value;
    }
    if include(&InteractionSeedTarget::ToolbarContextAwareUi)
        && let Some(value) = bool_value(InteractionSeedTarget::ToolbarContextAwareUi)
    {
        input.context_aware_ui = value;
    }
    if include(&InteractionSeedTarget::ToolbarPresetToasts)
        && let Some(value) = bool_value(InteractionSeedTarget::ToolbarPresetToasts)
    {
        input.show_preset_toasts = value;
    }
    if include(&InteractionSeedTarget::ToolbarIdleFade)
        && let Some(value) = bool_value(InteractionSeedTarget::ToolbarIdleFade)
    {
        input.idle_fade = value;
    }
    if include(&InteractionSeedTarget::ToolbarToolPreview)
        && let Some(value) = bool_value(InteractionSeedTarget::ToolbarToolPreview)
    {
        input.show_tool_preview = value;
    }
    if include(&InteractionSeedTarget::ToolbarDelaySliders)
        && let Some(value) = bool_value(InteractionSeedTarget::ToolbarDelaySliders)
    {
        input.show_delay_sliders = value;
    }
    if include(&InteractionSeedTarget::HistoryCustomSection)
        && let Some(value) = bool_value(InteractionSeedTarget::HistoryCustomSection)
    {
        input.custom_section_enabled = value;
    }
    if include(&InteractionSeedTarget::InputHud)
        && let Some(value) = bool_value(InteractionSeedTarget::InputHud)
    {
        input.set_input_hud_enabled(value);
    }
    if include(&InteractionSeedTarget::FloatingBadge)
        && let Some(value) = bool_value(InteractionSeedTarget::FloatingBadge)
    {
        input.show_floating_badge = value;
    }
    if include(&InteractionSeedTarget::ZoomChip)
        && let Some(value) = bool_value(InteractionSeedTarget::ZoomChip)
    {
        input.show_zoom_chip = value;
    }
    if include(&InteractionSeedTarget::ClickHighlight)
        && let Some(value) = bool_value(InteractionSeedTarget::ClickHighlight)
    {
        input.set_click_highlight_enabled(value);
    }
    if include(&InteractionSeedTarget::ClickHighlightToolRing)
        && let Some(value) = bool_value(InteractionSeedTarget::ClickHighlightToolRing)
    {
        input.set_highlight_tool_ring_enabled(value);
    }
    if include(&InteractionSeedTarget::ToolbarLayoutMode)
        && let Some(InteractionSeedValue::LayoutMode(mode)) =
            live.get(&InteractionSeedTarget::ToolbarLayoutMode)
    {
        input.apply_toolbar_layout_mode_runtime(*mode);
    }
    for flag in crate::config::ToolbarSectionFlag::ALL {
        let target = InteractionSeedTarget::SectionVisibility(flag);
        if include(&target)
            && let Some(InteractionSeedValue::Visibility(setting)) = live.get(&target)
        {
            input.apply_section_visibility_runtime(flag, *setting);
        }
    }
    if include(&InteractionSeedTarget::StatusBarInteractive)
        && let Some(value) = bool_value(InteractionSeedTarget::StatusBarInteractive)
    {
        input.status_bar_interactive = value;
    }
    for item in crate::config::StatusBarItem::ALL {
        if include(&InteractionSeedTarget::StatusBarItem(item))
            && let Some(value) = bool_value(InteractionSeedTarget::StatusBarItem(item))
        {
            input.set_status_bar_item_visible(item, value);
        }
    }
    if include(&InteractionSeedTarget::TopPinned)
        && let Some(value) = bool_value(InteractionSeedTarget::TopPinned)
    {
        input.toolbar_top_pinned = value;
    }
    if include(&InteractionSeedTarget::TopMinimized)
        && let Some(value) = bool_value(InteractionSeedTarget::TopMinimized)
    {
        input.apply_toolbar_set_top_minimized(value);
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

pub(super) fn apply_live_toolbar_positions(
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
}

pub(super) fn apply_live_board_state(
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

pub(super) fn runtime_preview_authority(
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
