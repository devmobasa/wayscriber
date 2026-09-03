use super::super::super::{
    CanvasIndex, Keymap, PointerTracking, ViewState,
    selection::{SelectionClipboard, SelectionInteraction},
};
use super::super::types::{CompositorCapabilities, DrawingState, PendingOnboardingUsage};
use super::structs::InputState;
use crate::config::BoardsConfig;
use crate::draw::DirtyTracker;
use crate::input::state::highlight::{ClickHighlightSettings, ClickHighlightState};
use crate::input::state::input_hud::{InputHudSettings, InputHudState};
use crate::input::{BoardManager, modifiers::Modifiers};

/// Runtime values needed to construct an [`InputState`].
///
/// The backend translates configuration into this value. Input state does not
/// need to know which config sections supplied each setting, and call sites
/// cannot accidentally transpose same-typed positional arguments.
#[derive(Clone)]
pub(in crate::input::state) struct InputStateSeed {
    pub(in crate::input::state) style: crate::input::state::core::DrawingStyle,
    pub(in crate::input::state) ui_visibility: crate::input::state::UiVisibility,
    pub(in crate::input::state) boards_config: BoardsConfig,
    pub(in crate::input::state) keymap: Keymap,
    pub(in crate::input::state) canvas_index: CanvasIndex,
    pub(in crate::input::state) click_highlight_settings: ClickHighlightSettings,
    pub(in crate::input::state) history_limits: crate::input::state::core::HistoryLimits,
    pub(in crate::input::state) presenter_mode_config: crate::config::PresenterModeConfig,
}

impl InputState {
    /// Creates input state from an explicit runtime seed.
    ///
    /// Screen dimensions default to zero and the backend updates them after
    /// surface configuration.
    pub(in crate::input::state) fn from_seed(seed: InputStateSeed) -> Self {
        let InputStateSeed {
            style,
            ui_visibility,
            boards_config,
            keymap,
            canvas_index,
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
            view: ViewState::default(),
            pointer: PointerTracking::default(),
            canvas_index,
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
            feedback: Default::default(),
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
            toolbar: crate::input::state::core::ToolbarInteraction::default(),
            precision_entry: None,
            board_previous_color: None,
            board_recent: Vec::new(),
            pending_board_delete: None,
            pending_page_delete: None,
            deleted_pages: Vec::new(),
            dirty_tracker: DirtyTracker::new(),
            spotlight_wheel: Default::default(),
            pending_onboarding_usage: PendingOnboardingUsage::default(),
            click_highlight: ClickHighlightState::new(click_highlight_settings),
            input_hud: InputHudState::new(InputHudSettings::default()),
            selection_interaction: SelectionInteraction::default(),
            context_menu: Default::default(),

            color_picker_popup: Default::default(),
            radial_menu: Default::default(),
            history_limits,
            ocr_scan: None,
            selection_clipboard: SelectionClipboard::default(),
            last_capture_path: None,
            properties: Default::default(),
            eyedropper_ui_state: crate::input::state::core::EyedropperUiState::Inactive,
            region_select_ui_state: crate::input::state::core::RegionSelectUiState::Inactive,
            preset_slots: Default::default(),
            tour: Default::default(),
            compositor_capabilities: CompositorCapabilities::default(),
            deleted_boards: Vec::new(),
        };

        if state.click_highlight.uses_pen_color() {
            state.sync_highlight_color();
        }

        state
    }
}
