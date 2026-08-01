use super::*;

pub(in crate::backend::wayland) fn apply_toolbar_runtime_rollback(
    input: &mut InputState,
    positions: &mut ToolbarPositionSnapshot,
    rollback: &PreviewRollbackSnapshot,
) {
    use InteractionSeedTarget as Target;

    // Matched on the target alone, with no catch-all: a new seed target is a
    // compile error here rather than a value that silently fails to roll
    // back. A value whose shape does not match its target rolls nothing back
    // -- the pairing is validated where the snapshot is built.
    for (target, value) in &rollback.values {
        match target {
            Target::TopPinned => set_bool(value, |v| input.toolbar_top_pinned = v),
            Target::SidePinned => set_bool(value, |v| input.toolbar_side_pinned = v),
            Target::TopMinimized => set_bool(value, |v| {
                input.apply_toolbar_set_top_minimized(v);
            }),
            Target::SideMinimized => set_bool(value, |v| input.toolbar_side_minimized = v),
            Target::SidePane => {
                if let InteractionSeedValue::SidePane(pane) = value {
                    input.apply_toolbar_set_side_pane(*pane);
                }
            }
            Target::CollapsedSection(section) => set_bool(value, |collapsed| {
                if collapsed {
                    input.toolbar_collapsed_side_sections.insert(*section);
                } else {
                    input.toolbar_collapsed_side_sections.remove(section);
                }
            }),
            Target::ItemVisibility(id) => {
                if let InteractionSeedValue::Visibility(setting) = value {
                    input.set_toolbar_item_visibility_setting(*id, *setting);
                }
            }
            Target::ItemOrder(group) => {
                if let InteractionSeedValue::ItemOrder(order) = value {
                    input.set_toolbar_item_order(*group, order);
                }
            }
            Target::BoardPin(board_id) => set_bool(value, |pinned| {
                input.apply_board_pinned_runtime(board_id, pinned);
            }),
            Target::TopPosition => {
                if let InteractionSeedValue::Position(position) = value {
                    positions.top = (position.x.get(), position.y.get());
                }
            }
            Target::SidePosition => {
                if let InteractionSeedValue::Position(position) = value {
                    positions.side = (position.x.get(), position.y.get());
                }
            }
            Target::TopDisplayMode => {
                if let InteractionSeedValue::TopDisplayMode(mode) = value {
                    apply_persisted_top_display_mode(input, *mode);
                }
            }
            Target::StatusBarInteractive => set_bool(value, |v| input.status_bar_interactive = v),
            Target::StatusBarItem(item) => set_bool(value, |v| {
                input.set_status_bar_item_visible(*item, v);
            }),
            Target::StatusBar => set_bool(value, |v| input.show_status_bar = v),
            Target::StatusBoardBadge => set_bool(value, |v| input.show_status_board_badge = v),
            Target::StatusPageBadge => set_bool(value, |v| input.show_status_page_badge = v),
            Target::FloatingBadgeAlways => {
                set_bool(value, |v| input.show_floating_badge_always = v)
            }
            Target::FloatingBadge => set_bool(value, |v| input.show_floating_badge = v),
            Target::ZoomChip => set_bool(value, |v| input.show_zoom_chip = v),
            Target::ToolbarIcons => set_bool(value, |v| input.toolbar_use_icons = v),
            Target::ToolbarMoreColors => set_bool(value, |v| input.show_more_colors = v),
            Target::ToolbarContextAwareUi => set_bool(value, |v| input.context_aware_ui = v),
            Target::ToolbarPresetToasts => set_bool(value, |v| input.show_preset_toasts = v),
            Target::ToolbarToolPreview => set_bool(value, |v| input.show_tool_preview = v),
            Target::ToolbarDelaySliders => set_bool(value, |v| input.show_delay_sliders = v),
            Target::HistoryCustomSection => set_bool(value, |v| input.custom_section_enabled = v),
            Target::InputHud => set_bool(value, |v| {
                input.set_input_hud_enabled(v);
            }),
            Target::ClickHighlight => set_bool(value, |v| {
                input.set_click_highlight_enabled(v);
            }),
            Target::ClickHighlightToolRing => set_bool(value, |v| {
                input.set_highlight_tool_ring_enabled(v);
            }),
            Target::SectionVisibility(flag) => {
                if let InteractionSeedValue::Visibility(setting) = value {
                    input.apply_section_visibility_runtime(*flag, *setting);
                }
            }
            Target::ToolbarLayoutMode => {
                if let InteractionSeedValue::LayoutMode(mode) = value {
                    input.apply_toolbar_layout_mode_runtime(*mode);
                }
            }
        }
    }
    // Today only the visibility toggle batches both pins into one snapshot;
    // the pin buttons persist single-pin scopes and are deliberately
    // decoupled from live visibility, so exactly the both-pins shape must
    // re-derive the live visibility flags — otherwise a rolled-back toggle
    // leaves the screen disagreeing with the restored pins (and with the
    // next start). The toggle arm skips no-op pin writes, so every snapshot
    // that lands here crossed a genuine pin transition and the derived
    // flags are exactly the startup (pin-derived) form of the pre-toggle
    // screen. The display mode is deliberately not restored: the persisted
    // type has no `Hidden`, and none is needed — after a hide-rollback the
    // still-`Hidden` live mode reproduces the pre-toggle screen, after a
    // show-rollback the strip is hidden either way, and any future show
    // unfolds `Hidden` to `Full` regardless.
    if rollback.values.contains_key(&Target::TopPinned)
        && rollback.values.contains_key(&Target::SidePinned)
    {
        input.derive_toolbar_visibility_from_pins();
    }
    input.needs_redraw = true;
}

/// Runs `apply` with the boolean a rollback value carries, if it carries one.
fn set_bool(value: &InteractionSeedValue, apply: impl FnOnce(bool)) {
    if let InteractionSeedValue::Bool(value) = value {
        apply(*value);
    }
}
