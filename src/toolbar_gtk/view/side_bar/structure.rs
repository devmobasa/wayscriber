//! Snapshot inputs that determine side-palette widget structure.

use super::*;

/// Discrete inputs that force a rebuild of the pane content.
#[derive(PartialEq)]
pub(super) struct StructureKey {
    pub(super) minimized: bool,
    pub(super) pane: SidePane,
    scale_milli: i64,
    use_icons: bool,
    layout_mode: crate::config::ToolbarLayoutMode,
    items: crate::config::ResolvedToolbarItems,
    /// Shortcut hints shown in button tooltips (e.g. the command-palette
    /// button). Rebuild when a rebind changes them so a tooltip never keeps
    /// showing a stale shortcut, matching the top bar's structure key.
    binding_hints: crate::ui::toolbar::ToolbarBindingHints,
    collapsed: BTreeSet<ToolbarSideSection>,
    tool_flags: (bool, bool, bool, bool, bool, bool),
    show_more_colors: bool,
    quick_colors: crate::config::QuickColorPalette,
    preset_slots: Vec<bool>,
    custom_section_enabled: bool,
    show_delay_sliders: bool,
    delay_actions_enabled: bool,
    show_actions_advanced: bool,
    show_zoom_actions: bool,
    customize_open: bool,
    customize_group: Option<crate::ui::toolbar::ToolbarItemCustomizeGroup>,
    recents: Vec<std::path::PathBuf>,
    pending_save_as: Option<std::path::PathBuf>,
    active_session: Option<std::path::PathBuf>,
    eraser_kind_targets: (bool, bool),
    polygon_active: bool,
    font_mono: bool,
    /// Drives the scoped section titles ("Color — Pen"): tool or text/note
    /// scope changes must rebuild even when nothing else did.
    title_scope: (crate::input::Tool, bool, bool, bool),
}

impl StructureKey {
    pub(super) fn of(snapshot: &ToolbarSnapshot) -> Self {
        let tool_context = ToolContext::from_snapshot(snapshot);
        Self {
            minimized: snapshot.side_minimized,
            pane: snapshot.active_side_pane,
            scale_milli: (effective_scale(snapshot) * 1000.0).round() as i64,
            use_icons: snapshot.use_icons,
            layout_mode: snapshot.layout_mode,
            items: snapshot.resolved_toolbar_items.clone(),
            binding_hints: snapshot.binding_hints.clone(),
            collapsed: snapshot.collapsed_side_sections.clone(),
            tool_flags: (
                tool_context.needs_color,
                tool_context.needs_thickness,
                tool_context.show_arrow_labels,
                tool_context.show_step_counter,
                tool_context.show_marker_opacity,
                tool_context.show_font_controls,
            ),
            show_more_colors: snapshot.show_more_colors,
            quick_colors: snapshot.quick_colors.clone(),
            preset_slots: snapshot.presets.iter().map(|slot| slot.is_some()).collect(),
            custom_section_enabled: snapshot.custom_section_enabled,
            show_delay_sliders: snapshot.show_delay_sliders,
            delay_actions_enabled: snapshot.delay_actions_enabled,
            show_actions_advanced: snapshot.show_actions_advanced,
            show_zoom_actions: snapshot.show_zoom_actions,
            customize_open: snapshot.customize_items_open,
            customize_group: snapshot.customize_items_group,
            recents: snapshot
                .recent_sessions
                .iter()
                .map(|recent| recent.path.clone())
                .collect(),
            pending_save_as: snapshot.pending_save_as_overwrite_path.clone(),
            active_session: snapshot.active_session_path.clone(),
            eraser_kind_targets: (
                snapshot.thickness_targets_eraser,
                snapshot.thickness_targets_marker,
            ),
            polygon_active: snapshot.active_tool == crate::input::Tool::RegularPolygon
                || snapshot.tool_override == Some(crate::input::Tool::RegularPolygon),
            font_mono: snapshot.font.family.eq_ignore_ascii_case("monospace"),
            title_scope: (
                snapshot.active_tool,
                snapshot.text_active,
                snapshot.note_active,
                snapshot.context_aware_ui,
            ),
        }
    }
}

pub(super) fn effective_scale(snapshot: &ToolbarSnapshot) -> f64 {
    if snapshot.toolbar_scale.is_finite() {
        snapshot.toolbar_scale.clamp(0.5, 3.0)
    } else {
        1.0
    }
}
