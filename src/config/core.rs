use super::keybindings::{Action, KeybindingAuthorship, KeybindingsConfig};
#[cfg(feature = "tablet-input")]
use super::types::TabletInputConfig;
use super::types::{
    ArrowConfig, BoardConfig, BoardsConfig, CaptureConfig, ClipboardConfig, DrawingConfig,
    ExportConfig, HistoryConfig, PerformanceConfig, PresenterModeConfig, PresetSlotsConfig,
    RenderProfilesConfig, SessionConfig, SpotlightConfig, TrayConfig, UiConfig, UpdatesConfig,
};
use serde::{Deserialize, Serialize};

/// Current revision for reviewed configuration migrations.
///
/// Revision history:
/// - 1: split the command-palette / full-screen-capture default shortcuts
///   (`Ctrl+K` + `Ctrl+Shift+P` vs `Ctrl+Alt+F`).
/// - 2: moved `F2` from the `toggle_toolbar` default pair (`["F2", "F9"]`)
///   to the new `cycle_toolbar_display` action default (`["F2"]`).
/// - 3: gave the new `toggle_input_hud` action a `Ctrl+Shift+K` default,
///   which collides with files that already bound that shortcut elsewhere.
///
/// Loading never advances this: the value in memory is the one the file
/// carries, and the migration recipes in `Config::apply_keybinding_migrations`
/// are preview material for an explicit configurator flow rather than
/// something a process start applies. A new or moved default no longer needs
/// a migration to stay out of an authored shortcut's way either — omitted
/// actions are resolved from source presence, so a default the file never
/// mentioned cannot outrank a binding it did.
/// `default_bindings_match_the_checked_in_snapshot` still fails until the
/// snapshot records the change deliberately.
pub const CURRENT_CONFIG_REVISION: u32 = 3;

/// Main configuration structure containing all user settings.
///
/// This is the root configuration type that gets deserialized from the TOML file.
/// All fields have sensible defaults and will use those if not specified in the config file.
///
/// # Example TOML
/// ```toml
/// [drawing]
/// default_color = "red"
/// default_thickness = 3.0
/// default_font_size = 32.0
///
/// [arrow]
/// length = 20.0
/// angle_degrees = 30.0
/// head_at_end = false
///
/// [performance]
/// buffer_count = 3
/// enable_vsync = false
/// max_fps_no_vsync = 120
/// ui_animation_fps = 30
///
/// [ui]
/// show_status_bar = true
/// status_bar_position = "bottom-left"
///
/// [keybindings]
/// exit = ["Escape", "Ctrl+Q"]
/// undo = ["Ctrl+Z"]
/// ```
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Persisted migration provenance. Missing in legacy files, which
    /// deserialize as revision 0 and keep that value: only an explicit
    /// configurator migration advances it.
    #[serde(default)]
    pub config_revision: u32,

    /// Which `[keybindings]` fields the source this value came from spelled
    /// out.
    ///
    /// Never read from or written to a file — it describes the document, not a
    /// setting — so it is skipped by serde in both directions. A `Config` built
    /// in code carries the `AllExplicit` default, which is what keeps a
    /// fixture, a draft, or the shipped defaults from having their own lists
    /// treated as droppable compiled defaults.
    #[serde(skip)]
    pub(crate) keybinding_authorship: KeybindingAuthorship,

    /// Drawing tool defaults (color, thickness, font size)
    #[serde(default)]
    pub drawing: DrawingConfig,

    /// Preset slots for quick tool switching
    #[serde(default)]
    pub presets: PresetSlotsConfig,

    /// History playback settings
    #[serde(default)]
    pub history: HistoryConfig,

    /// Arrow appearance settings
    #[serde(default)]
    pub arrow: ArrowConfig,

    /// Spotlight tool settings
    #[serde(default)]
    pub spotlight: SpotlightConfig,

    /// Performance tuning options
    #[serde(default)]
    pub performance: PerformanceConfig,

    /// UI display preferences
    #[serde(default)]
    pub ui: UiConfig,

    /// System tray appearance preferences
    #[serde(default)]
    pub tray: TrayConfig,

    /// Update notification preferences (checks only; nothing is installed)
    #[serde(default)]
    pub updates: UpdatesConfig,

    /// Clipboard paste behavior
    #[serde(default)]
    pub clipboard: ClipboardConfig,

    /// Presenter mode behavior overrides
    #[serde(default)]
    pub presenter_mode: PresenterModeConfig,

    /// Final-render color profile mappings.
    #[serde(default)]
    pub render_profiles: RenderProfilesConfig,

    /// Multi-board settings (preferred over legacy [board] section)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boards: Option<BoardsConfig>,

    /// Board mode settings (whiteboard/blackboard)
    #[serde(default)]
    pub board: BoardConfig,

    /// Keybinding customization
    #[serde(default)]
    pub keybindings: KeybindingsConfig,

    /// Screenshot capture settings
    #[serde(default)]
    pub capture: CaptureConfig,

    /// Explicit file export settings
    #[serde(default)]
    pub export: ExportConfig,

    /// Tablet/stylus input settings (feature-gated)
    #[cfg(feature = "tablet-input")]
    #[serde(default)]
    pub tablet: TabletInputConfig,

    /// Session persistence settings
    #[serde(default)]
    pub session: SessionConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_revision: CURRENT_CONFIG_REVISION,
            keybinding_authorship: KeybindingAuthorship::default(),
            drawing: DrawingConfig::default(),
            presets: PresetSlotsConfig::default(),
            history: HistoryConfig::default(),
            arrow: ArrowConfig::default(),
            spotlight: SpotlightConfig::default(),
            performance: PerformanceConfig::default(),
            ui: UiConfig::default(),
            tray: TrayConfig::default(),
            updates: UpdatesConfig::default(),
            clipboard: ClipboardConfig::default(),
            presenter_mode: PresenterModeConfig::default(),
            render_profiles: RenderProfilesConfig::default(),
            boards: Some(BoardsConfig::default()),
            board: BoardConfig::default(),
            keybindings: KeybindingsConfig::default(),
            capture: CaptureConfig::default(),
            export: ExportConfig::default(),
            #[cfg(feature = "tablet-input")]
            tablet: TabletInputConfig::default(),
            session: SessionConfig::default(),
        }
    }
}

impl Config {
    /// Declares every `[keybindings]` list in this configuration authored.
    ///
    /// Any editor that rebuilds the section from its own UI must call this
    /// before validation: the lists no longer come from the document the
    /// authorship was recorded from, so file presence has stopped describing
    /// them. Without it, a shortcut the user just typed for an action their
    /// file omits is still classified as a compiled-in offer, and the
    /// omitted-default pass filters it out instead of arbitrating the
    /// collision it causes — silently, and with the emptied list on its way to
    /// disk.
    pub fn mark_keybindings_explicit(&mut self) {
        self.keybinding_authorship = KeybindingAuthorship::AllExplicit;
    }

    /// Declares one action's `[keybindings]` list authored.
    ///
    /// The narrow shortcut editor's form of the above: it rewrites exactly one
    /// key, so exactly one list has stopped coming from the file, and the
    /// omitted-default pass must keep judging every other one by what the file
    /// actually spells out.
    pub(crate) fn mark_keybinding_explicit(&mut self, action: Action) {
        if let Some(key) = KeybindingsConfig::config_key_for_action(action) {
            self.keybinding_authorship.mark_explicit(key);
        }
    }

    pub fn resolved_boards(&self) -> BoardsConfig {
        match &self.boards {
            Some(boards) if !boards.items.is_empty() => boards.clone(),
            Some(boards) => BoardsConfig {
                max_count: boards.max_count,
                auto_create: boards.auto_create,
                show_board_badge: boards.show_board_badge,
                pan_enabled: boards.pan_enabled,
                show_pan_badge: boards.show_pan_badge,
                persist_customizations: boards.persist_customizations,
                default_board: boards.default_board.clone(),
                ..BoardsConfig::default()
            },
            None => BoardsConfig::from_legacy(&self.board),
        }
    }
}
