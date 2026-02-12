use super::super::super::{
    board_picker::BoardPickerState, color_picker_popup::ColorPickerPopupState,
    menus::ContextMenuState, radial_menu::RadialMenuState, selection::SelectionState,
};
use super::super::types::{
    CompositorCapabilities, DrawingState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS,
    PressureThicknessEditMode, PressureThicknessEntryMode, TextInputMode, ToolbarDrawerTab,
};
use super::structs::InputState;
use crate::config::{Action, BoardsConfig, KeyBinding, PRESET_SLOTS_MAX, RadialMenuMouseBinding};
use crate::draw::{DirtyTracker, EraserKind, FontDescriptor};
use crate::input::state::highlight::{ClickHighlightSettings, ClickHighlightState};
use crate::input::{BoardManager, modifiers::Modifiers, tool::EraserMode};
use std::collections::HashMap;

impl InputState {
    /// Creates a new InputState with specified defaults.
    ///
    /// Screen dimensions default to 0 and should be updated by the backend
    /// after surface configuration (see `update_screen_dimensions`).
    ///
    /// # Arguments
    /// * `color` - Initial drawing color
    /// * `thickness` - Initial pen thickness in pixels
    /// * `eraser_size` - Initial eraser size in pixels
    /// * `eraser_mode` - Initial eraser behavior mode
    /// * `font_size` - Font size for text mode in points
    /// * `font_descriptor` - Font configuration for text rendering
    /// * `text_background_enabled` - Whether to draw background behind text
    /// * `arrow_length` - Arrowhead length in pixels
    /// * `arrow_angle` - Arrowhead angle in degrees
    /// * `arrow_head_at_end` - Whether arrowhead is drawn at the end
    /// * `show_status_bar` - Whether the status bar starts visible
    /// * `boards_config` - Multi-board configuration
    /// * `action_map` - Keybinding action map
    /// * `presenter_mode_config` - Presenter mode behavior configuration
    #[allow(clippy::too_many_arguments)]
    pub fn with_defaults(
        color: crate::draw::Color,
        thickness: f64,
        eraser_size: f64,
        eraser_mode: EraserMode,
        marker_opacity: f64,
        fill_enabled: bool,
        font_size: f64,
        font_descriptor: FontDescriptor,
        text_background_enabled: bool,
        arrow_length: f64,
        arrow_angle: f64,
        arrow_head_at_end: bool,
        show_status_bar: bool,
        boards_config: BoardsConfig,
        action_map: HashMap<KeyBinding, Action>,
        max_shapes_per_frame: usize,
        click_highlight_settings: ClickHighlightSettings,
        undo_all_delay_ms: u64,
        redo_all_delay_ms: u64,
        custom_section_enabled: bool,
        custom_undo_delay_ms: u64,
        custom_redo_delay_ms: u64,
        custom_undo_steps: usize,
        custom_redo_steps: usize,
        presenter_mode_config: crate::config::PresenterModeConfig,
    ) -> Self {
        let clamped_eraser = eraser_size.clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
        let mut state = Self {
            boards: BoardManager::from_config(boards_config),
            current_color: color,
            current_thickness: thickness,
            pressure_variation_threshold: 0.1,
            pressure_thickness_edit_mode: PressureThicknessEditMode::Disabled,
            pressure_thickness_entry_mode: PressureThicknessEntryMode::PressureOnly,
            pressure_thickness_scale_step: 0.1,
            eraser_size: clamped_eraser,
            eraser_kind: EraserKind::Circle,
            eraser_mode,
            marker_opacity,
            current_font_size: font_size,
            font_descriptor,
            text_background_enabled,
            text_wrap_width: None,
            text_input_mode: TextInputMode::Plain,
            arrow_length,
            arrow_angle,
            arrow_head_at_end,
            arrow_label_enabled: false,
            arrow_label_counter: 1,
            step_marker_counter: 1,
            modifiers: Modifiers::new(),
            state: DrawingState::Idle,
            should_exit: false,
            needs_redraw: true,
            session_dirty: false,
            show_help: false,
            help_overlay_page: 0,
            help_overlay_search: String::new(),
            help_overlay_scroll: 0.0,
            help_overlay_scroll_max: 0.0,
            board_picker_search: String::new(),
            board_picker_search_last_input: None,
            command_palette_open: false,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            command_palette_scroll: 0,
            command_palette_recent: Vec::new(),
            command_palette_toast_duration_ms: 1500,
            show_status_bar,
            show_status_board_badge: true,
            show_status_page_badge: true,
            show_floating_badge_always: false,
            presenter_mode: false,
            presenter_mode_config,
            presenter_restore: None,
            toolbar_visible: true,
            toolbar_top_visible: true,
            toolbar_side_visible: true,
            fill_enabled,
            toolbar_top_pinned: true,
            toolbar_side_pinned: true,
            toolbar_use_icons: true, // Default to icon mode
            toolbar_scale: 1.0,
            toolbar_layout_mode: crate::config::ToolbarLayoutMode::Regular,
            toolbar_mode_overrides: crate::config::ToolbarModeOverrides::default(),
            toolbar_shapes_expanded: false,
            toolbar_drawer_open: false,
            toolbar_drawer_tab: ToolbarDrawerTab::View,
            screen_width: 0,
            screen_height: 0,
            show_active_output_badge: false,
            active_output_label: None,
            board_previous_color: None,
            board_recent: Vec::new(),
            pending_board_delete: None,
            pending_page_delete: None,
            deleted_pages: Vec::new(),
            dirty_tracker: DirtyTracker::new(),
            last_provisional_bounds: None,
            last_text_preview_bounds: None,
            action_map,
            action_bindings: HashMap::new(),
            pending_capture_action: None,
            pending_output_focus_action: None,
            pending_zoom_action: None,
            pending_copy_hex: false,
            pending_paste_hex: false,
            max_shapes_per_frame,
            click_highlight: ClickHighlightState::new(click_highlight_settings),
            tool_override: None,
            selection_state: SelectionState::None,
            last_selection_axis: None,
            context_menu_state: ContextMenuState::Hidden,
            context_menu_enabled: true,
            context_menu_page_target: None,
            board_picker_state: BoardPickerState::Hidden,
            board_picker_drag: None,
            board_picker_page_drag: None,
            board_picker_page_edit: None,
            color_picker_popup_state: ColorPickerPopupState::Hidden,
            color_picker_popup_layout: None,
            radial_menu_state: RadialMenuState::Hidden,
            radial_menu_layout: None,
            radial_menu_mouse_binding: RadialMenuMouseBinding::Middle,
            hit_test_cache: HashMap::new(),
            hit_test_tolerance: 6.0,
            max_linear_hit_test: 400,
            undo_stack_limit: 100,
            undo_all_delay_ms,
            redo_all_delay_ms,
            custom_undo_delay_ms,
            custom_redo_delay_ms,
            custom_undo_steps,
            custom_redo_steps,
            custom_section_enabled,
            show_delay_sliders: false, // Default to hidden
            show_marker_opacity_section: false,
            show_preset_toasts: true,
            show_tool_preview: false,
            ui_toast: None,
            ui_toast_bounds: None,
            selection_clipboard: None,
            clipboard_paste_offset: 0,
            last_capture_path: None,
            last_text_click: None,
            last_board_picker_click: None,
            text_edit_target: None,
            text_edit_entry_feedback: None,
            pending_history: None,
            context_menu_layout: None,
            board_picker_layout: None,
            spatial_index: None,
            last_pointer_position: (0, 0),
            pending_menu_hover_recalc: false,
            shape_properties_panel: None,
            properties_panel_layout: None,
            pending_properties_hover_recalc: false,
            properties_panel_needs_refresh: false,
            frozen_active: false,
            pending_frozen_toggle: false,
            zoom_active: false,
            zoom_locked: false,
            zoom_scale: 1.0,
            show_more_colors: false,
            show_actions_section: true, // Show by default
            show_actions_advanced: false,
            show_zoom_actions: true,
            show_pages_section: true,
            show_boards_section: true,
            show_presets: true,
            show_step_section: false,
            show_text_controls: true,
            context_aware_ui: true,
            show_settings_section: true,
            preset_slot_count: PRESET_SLOTS_MAX,
            presets: vec![None; PRESET_SLOTS_MAX],
            active_preset_slot: None,
            preset_feedback: vec![None; PRESET_SLOTS_MAX],
            pending_preset_action: None,
            pending_board_config: None,
            tour_active: false,
            tour_step: 0,
            compositor_capabilities: CompositorCapabilities::default(),
            capability_toast_shown: false,
            blocked_action_feedback: None,
            pending_clipboard_fallback: None,
            deleted_boards: Vec::new(),
            status_change_highlight: None,
            help_overlay_quick_mode: false,
            help_overlay_search_cursor: 0,
        };

        if state.click_highlight.uses_pen_color() {
            state.sync_highlight_color();
        }

        state
    }
}
