use super::super::super::{
    Keymap,
    selection::{SelectionClipboard, SelectionInteraction},
};
use super::super::types::{CompositorCapabilities, DrawingState, PendingOnboardingUsage};
use super::structs::InputState;
use crate::config::BoardsConfig;
use crate::draw::DirtyTracker;
use crate::input::state::highlight::{ClickHighlightSettings, ClickHighlightState};
use crate::input::state::input_hud::{InputHudSettings, InputHudState};
use crate::input::{BoardManager, modifiers::Modifiers};
use std::collections::HashMap;

/// Runtime values needed to construct an [`InputState`].
///
/// The backend translates configuration into this value. Input state does not
/// need to know which config sections supplied each setting, and call sites
/// cannot accidentally transpose same-typed positional arguments.
#[derive(Clone)]
pub(crate) struct InputStateSeed {
    pub(crate) style: crate::input::state::core::DrawingStyle,
    pub(crate) ui_visibility: crate::input::state::UiVisibility,
    pub(crate) boards_config: BoardsConfig,
    pub(crate) keymap: Keymap,
    pub(crate) max_shapes_per_frame: usize,
    pub(crate) click_highlight_settings: ClickHighlightSettings,
    pub(crate) history_limits: crate::input::state::core::HistoryLimits,
    pub(crate) presenter_mode_config: crate::config::PresenterModeConfig,
}

impl InputState {
    /// Creates input state from an explicit runtime seed.
    ///
    /// Screen dimensions default to zero and the backend updates them after
    /// surface configuration.
    pub(crate) fn from_seed(seed: InputStateSeed) -> Self {
        let InputStateSeed {
            style,
            ui_visibility,
            boards_config,
            keymap,
            max_shapes_per_frame,
            click_highlight_settings,
            history_limits,
            presenter_mode_config,
        } = seed;
        let mut state = Self {
            input_effects: Default::default(),
            boards: BoardManager::from_config(boards_config),
            style,
            font_picker: Default::default(),
            text_editing: Default::default(),
            modifiers: Modifiers::new(),
            keymap,
            state: DrawingState::Idle,
            should_exit: false,
            explicit_exit_requested: false,
            needs_redraw: true,
            session_dirty: false,
            session_preflight_options: None,
            pending_save_as_overwrite: None,
            help_overlay: Default::default(),
            board_picker: Default::default(),
            command_palette: Default::default(),
            command_palette_toast_duration_ms: 1500,
            ui_visibility,
            zoom_chip: Default::default(),
            presenter_mode: false,
            presenter_mode_config,
            render_profiles: crate::render_profiles::RenderProfileSet::default(),
            presenter_restore: None,
            focus_mode_restore: None,
            status_hud: Default::default(),
            light_mode: false,
            light_mode_drawing: false,
            light_mode_restore: None,
            toolbar_visible: true,
            toolbar_top_visible: true,
            toolbar_top_pinned: true,
            toolbar_use_icons: true, // Default to icon mode
            toolbar_scale: 1.0,
            toolbar_layout_mode: crate::config::ToolbarLayoutMode::Regular,
            toolbar_mode_overrides: crate::config::ToolbarModeOverrides::default(),
            toolbar_items: crate::config::ToolbarItemsConfig::default(),
            resolved_toolbar_items: crate::config::ToolbarItemsConfig::default().resolved(),
            toolbar_customize_drag: None,
            toolbar_top_menu: crate::input::state::TopMenuState::Closed,
            toolbar_top_popover_scroll: 0.0,
            toolbar_top_minimized: false,
            toolbar_top_display_mode: crate::config::TopDisplayMode::Full,
            last_draw_activity: std::time::Instant::now(),
            precision_entry: None,
            toolbar_rebind_modifier: crate::config::ToolbarRebindModifier::default(),
            toolbar_customize_items_open: false,
            toolbar_customize_items_group: None,
            toolbar_status_bar_contents_open: false,
            screen_width: 0,
            screen_height: 0,
            active_output_label: None,
            board_previous_color: None,
            board_recent: Vec::new(),
            pending_board_delete: None,
            pending_page_delete: None,
            deleted_pages: Vec::new(),
            dirty_tracker: DirtyTracker::new(),
            last_provisional_bounds: None,
            spotlight_magnification_gesture: None,
            spotlight_wheel_value120_remainder: None,
            pending_onboarding_usage: PendingOnboardingUsage::default(),
            max_shapes_per_frame,
            click_highlight: ClickHighlightState::new(click_highlight_settings),
            input_hud: InputHudState::new(InputHudSettings::default()),
            selection_interaction: SelectionInteraction::default(),
            context_menu: Default::default(),

            color_picker_popup: Default::default(),
            radial_menu: Default::default(),
            hit_test_cache: HashMap::new(),
            canvas_content_generation: 0,
            hit_test_tolerance: 6.0,
            max_linear_hit_test: 400,
            history_limits,
            ocr_scan: None,
            ui_toast: None,
            toast_queue: super::super::toast_queue::ToastQueue::default(),
            ui_toast_bounds: None,
            ui_toast_action_bounds: [None, None],
            selection_clipboard: SelectionClipboard::default(),
            last_capture_path: None,
            spatial_index: None,
            last_pointer_position: (0, 0),
            last_canvas_pointer_position: (0, 0),
            pointer_seen: false,
            pending_menu_hover_recalc: false,
            properties: Default::default(),
            frozen_active: false,
            eyedropper_ui_state: crate::input::state::core::EyedropperUiState::Inactive,
            region_select_ui_state: crate::input::state::core::RegionSelectUiState::Inactive,
            zoom_active: false,
            zoom_locked: false,
            zoom_scale: 1.0,
            zoom_view_offset: (0.0, 0.0),
            preset_slots: Default::default(),
            tour: Default::default(),
            compositor_capabilities: CompositorCapabilities::default(),
            capability_toast_caps: None,
            blocked_action_feedback: None,
            deleted_boards: Vec::new(),
            status_change_highlight: None,
        };

        if state.click_highlight.uses_pen_color() {
            state.sync_highlight_color();
        }

        state
    }
}
