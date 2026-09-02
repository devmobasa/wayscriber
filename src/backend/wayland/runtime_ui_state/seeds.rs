use super::*;

pub(super) fn runtime_seeds_from_config(
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
        InteractionSeedTarget::TopMinimized,
        InteractionSeedValue::Bool(config.ui.toolbar.top_minimized),
    )?;
    insert(
        InteractionSeedTarget::StatusBar,
        InteractionSeedValue::Bool(config.ui.show_status_bar),
    )?;
    insert(
        InteractionSeedTarget::StatusBoardBadge,
        InteractionSeedValue::Bool(config.ui.show_status_board_badge),
    )?;
    insert(
        InteractionSeedTarget::StatusPageBadge,
        InteractionSeedValue::Bool(config.ui.show_status_page_badge),
    )?;
    insert(
        InteractionSeedTarget::FloatingBadgeAlways,
        InteractionSeedValue::Bool(config.ui.show_floating_badge_always),
    )?;
    insert(
        InteractionSeedTarget::ToolbarIcons,
        InteractionSeedValue::Bool(config.ui.toolbar.use_icons),
    )?;
    insert(
        InteractionSeedTarget::ToolbarMoreColors,
        InteractionSeedValue::Bool(config.ui.toolbar.show_more_colors),
    )?;
    insert(
        InteractionSeedTarget::ToolbarContextAwareUi,
        InteractionSeedValue::Bool(config.ui.toolbar.context_aware_ui),
    )?;
    insert(
        InteractionSeedTarget::ToolbarPresetToasts,
        InteractionSeedValue::Bool(config.ui.toolbar.show_preset_toasts),
    )?;
    insert(
        InteractionSeedTarget::ToolbarIdleFade,
        InteractionSeedValue::Bool(config.ui.toolbar.idle_fade),
    )?;
    insert(
        InteractionSeedTarget::ToolbarToolPreview,
        InteractionSeedValue::Bool(config.ui.toolbar.show_tool_preview),
    )?;
    insert(
        InteractionSeedTarget::ToolbarDelaySliders,
        InteractionSeedValue::Bool(config.ui.toolbar.show_delay_sliders),
    )?;
    insert(
        InteractionSeedTarget::HistoryCustomSection,
        InteractionSeedValue::Bool(config.history.custom_section_enabled),
    )?;
    insert(
        InteractionSeedTarget::InputHud,
        InteractionSeedValue::Bool(config.ui.input_hud.enabled),
    )?;
    insert(
        InteractionSeedTarget::StatusBarInteractive,
        InteractionSeedValue::Bool(config.ui.status_bar_interactive),
    )?;
    for item in crate::config::StatusBarItem::ALL {
        insert(
            InteractionSeedTarget::StatusBarItem(item),
            InteractionSeedValue::Bool(config.ui.status_bar_item_visible(item)),
        )?;
    }
    let resolved_items = resolved_toolbar_item_seeds(config);
    insert(
        InteractionSeedTarget::FloatingBadge,
        InteractionSeedValue::Bool(config.ui.show_floating_badge),
    )?;
    insert(
        InteractionSeedTarget::ZoomChip,
        InteractionSeedValue::Bool(config.ui.toolbar.show_zoom_chip),
    )?;
    insert(
        InteractionSeedTarget::ClickHighlight,
        InteractionSeedValue::Bool(config.ui.click_highlight.enabled),
    )?;
    insert(
        InteractionSeedTarget::ClickHighlightToolRing,
        InteractionSeedValue::Bool(config.ui.click_highlight.show_on_highlight_tool),
    )?;
    insert(
        InteractionSeedTarget::ToolbarLayoutMode,
        InteractionSeedValue::LayoutMode(config.ui.toolbar.layout_mode),
    )?;
    for flag in crate::config::ToolbarSectionFlag::ALL {
        insert(
            InteractionSeedTarget::SectionVisibility(flag),
            InteractionSeedValue::Visibility(item_visibility_setting(
                &resolved_items,
                flag.item_id(),
            )),
        )?;
    }
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

pub(super) fn board_pin_seeds_from_input(input: &InputState) -> BTreeMap<String, bool> {
    input
        .boards
        .pin_seed_entries()
        .map(|(id, pinned)| (id.to_string(), pinned))
        .collect()
}

pub(super) fn retain_stored_board_pin_seeds_for_session_restore(
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

pub(super) fn toolbar_values(
    target: ToolbarRuntimeUiPersistenceTarget,
    input: &InputState,
) -> std::result::Result<RuntimeUiMutationValues, MutationShapeError> {
    use ToolbarRuntimeUiPersistenceTarget as Target;
    match target {
        Target::TopPinned => RuntimeUiMutationValues::one(
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(input.toolbar_top_pinned),
        ),
        Target::TopMinimized => RuntimeUiMutationValues::one(
            InteractionSeedTarget::TopMinimized,
            InteractionSeedValue::Bool(input.toolbar_top_minimized),
        ),
        Target::TopDisplayMode => top_display_mode_values(input.toolbar_top_display_mode, input),
        Target::StatusBar => RuntimeUiMutationValues::one(
            InteractionSeedTarget::StatusBar,
            InteractionSeedValue::Bool(input.ui_visibility.show_status_bar),
        ),
        Target::StatusBoardBadge => RuntimeUiMutationValues::one(
            InteractionSeedTarget::StatusBoardBadge,
            InteractionSeedValue::Bool(input.ui_visibility.show_status_board_badge),
        ),
        Target::StatusPageBadge => RuntimeUiMutationValues::one(
            InteractionSeedTarget::StatusPageBadge,
            InteractionSeedValue::Bool(input.ui_visibility.show_status_page_badge),
        ),
        Target::FloatingBadgeAlways => RuntimeUiMutationValues::one(
            InteractionSeedTarget::FloatingBadgeAlways,
            InteractionSeedValue::Bool(input.ui_visibility.show_floating_badge_always),
        ),
        Target::ToolbarIcons => RuntimeUiMutationValues::one(
            InteractionSeedTarget::ToolbarIcons,
            InteractionSeedValue::Bool(input.toolbar_use_icons),
        ),
        Target::ToolbarMoreColors => RuntimeUiMutationValues::one(
            InteractionSeedTarget::ToolbarMoreColors,
            InteractionSeedValue::Bool(input.ui_visibility.show_more_colors),
        ),
        Target::ToolbarContextAwareUi => RuntimeUiMutationValues::one(
            InteractionSeedTarget::ToolbarContextAwareUi,
            InteractionSeedValue::Bool(input.ui_visibility.context_aware_ui),
        ),
        Target::ToolbarPresetToasts => RuntimeUiMutationValues::one(
            InteractionSeedTarget::ToolbarPresetToasts,
            InteractionSeedValue::Bool(input.ui_visibility.show_preset_toasts),
        ),
        Target::ToolbarIdleFade => RuntimeUiMutationValues::one(
            InteractionSeedTarget::ToolbarIdleFade,
            InteractionSeedValue::Bool(input.ui_visibility.idle_fade),
        ),
        Target::ToolbarToolPreview => RuntimeUiMutationValues::one(
            InteractionSeedTarget::ToolbarToolPreview,
            InteractionSeedValue::Bool(user_tool_preview(input)),
        ),
        Target::ToolbarDelaySliders => RuntimeUiMutationValues::one(
            InteractionSeedTarget::ToolbarDelaySliders,
            InteractionSeedValue::Bool(input.ui_visibility.show_delay_sliders),
        ),
        Target::HistoryCustomSection => RuntimeUiMutationValues::one(
            InteractionSeedTarget::HistoryCustomSection,
            InteractionSeedValue::Bool(input.custom_section_enabled),
        ),
        Target::InputHud => RuntimeUiMutationValues::one(
            InteractionSeedTarget::InputHud,
            InteractionSeedValue::Bool(input.input_hud_enabled()),
        ),
        Target::FloatingBadge => RuntimeUiMutationValues::one(
            InteractionSeedTarget::FloatingBadge,
            InteractionSeedValue::Bool(input.ui_visibility.show_floating_badge),
        ),
        Target::ZoomChip => RuntimeUiMutationValues::one(
            InteractionSeedTarget::ZoomChip,
            InteractionSeedValue::Bool(input.ui_visibility.show_zoom_chip),
        ),
        Target::ClickHighlight => click_highlight_values(
            user_click_highlight_enabled(input),
            input.highlight_tool_ring_enabled(),
        ),
        Target::LayoutMode => RuntimeUiMutationValues::one(
            InteractionSeedTarget::ToolbarLayoutMode,
            InteractionSeedValue::LayoutMode(input.toolbar_layout_mode),
        ),
        Target::NamedSection(flag) => RuntimeUiMutationValues::one(
            InteractionSeedTarget::SectionVisibility(flag),
            InteractionSeedValue::Visibility(item_visibility_setting(
                &input.resolved_toolbar_items,
                flag.item_id(),
            )),
        ),
        Target::StatusBarInteractive => RuntimeUiMutationValues::one(
            InteractionSeedTarget::StatusBarInteractive,
            InteractionSeedValue::Bool(input.ui_visibility.status_bar_interactive),
        ),
        Target::StatusBarItem(item) => RuntimeUiMutationValues::one(
            InteractionSeedTarget::StatusBarItem(item),
            InteractionSeedValue::Bool(input.status_bar_item_visible(item)),
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
        // The keyboard toggle drives the pin through the same wire key the pin
        // button writes, settled through one accepted revision.
        Target::ToolbarVisibility => RuntimeUiMutationValues::one(
            InteractionSeedTarget::TopPinned,
            InteractionSeedValue::Bool(input.toolbar_top_pinned),
        ),
    }
}

/// The persisted form of a live top-display mode.
///
/// The seed a single-boolean chrome target persists under, if it is one.
///
/// Keyboard-driven toggles build their own rollback and so need the seed the
/// target maps to without a live `InputState` to read it from.
pub(in crate::backend::wayland) fn single_bool_seed_target(
    target: ToolbarRuntimeUiPersistenceTarget,
) -> Option<InteractionSeedTarget> {
    use ToolbarRuntimeUiPersistenceTarget as Target;
    match target {
        Target::StatusBar => Some(InteractionSeedTarget::StatusBar),
        Target::FloatingBadge => Some(InteractionSeedTarget::FloatingBadge),
        Target::ZoomChip => Some(InteractionSeedTarget::ZoomChip),
        Target::InputHud => Some(InteractionSeedTarget::InputHud),
        _ => None,
    }
}

/// The click-highlight values as one batch: `ToggleAllHighlight` can move the
/// ring's companion, and the keyboard path persists both from a single
/// pre-change snapshot.
pub(in crate::backend::wayland) fn click_highlight_values(
    enabled: bool,
    tool_ring: bool,
) -> std::result::Result<RuntimeUiMutationValues, MutationShapeError> {
    RuntimeUiMutationValues::batch([
        (
            InteractionSeedTarget::ClickHighlight,
            InteractionSeedValue::Bool(enabled),
        ),
        (
            InteractionSeedTarget::ClickHighlightToolRing,
            InteractionSeedValue::Bool(tool_ring),
        ),
    ])
}
