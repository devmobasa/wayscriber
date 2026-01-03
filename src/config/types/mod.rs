//! Configuration type definitions.

mod arrow;
mod board;
mod capture;
mod click_highlight;
mod context_menu;
mod drawing;
mod help_overlay;
mod history;
mod performance;
mod presenter_mode;
mod presets;
mod session;
mod status_bar;
#[cfg(tablet)]
mod tablet;
mod toolbar;
mod ui;

pub use arrow::ArrowConfig;
pub use board::BoardConfig;
pub use capture::CaptureConfig;
pub use click_highlight::ClickHighlightConfig;
pub use context_menu::ContextMenuUiConfig;
pub use drawing::DrawingConfig;
pub use help_overlay::HelpOverlayStyle;
pub use history::HistoryConfig;
pub use performance::PerformanceConfig;
pub use presenter_mode::{PresenterModeConfig, PresenterToolBehavior};
pub use presets::{PRESET_SLOTS_MAX, PRESET_SLOTS_MIN, PresetSlotsConfig, ToolPresetConfig};
pub use session::{SessionCompression, SessionConfig, SessionStorageMode};
pub use status_bar::StatusBarStyle;
#[cfg(tablet)]
pub use tablet::TabletInputConfig;
#[allow(unused_imports)]
pub use toolbar::{
    ToolbarConfig, ToolbarLayoutMode, ToolbarModeOverride, ToolbarModeOverrides,
    ToolbarSectionDefaults,
};
pub use ui::UiConfig;
