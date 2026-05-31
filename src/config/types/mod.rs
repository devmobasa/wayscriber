//! Configuration type definitions.

mod arrow;
mod board;
mod boards;
mod capture;
mod click_highlight;
mod context_menu;
mod drawing;
mod export;
mod help_overlay;
mod history;
mod performance;
mod presenter_mode;
mod presets;
mod render_profiles;
mod session;
mod status_bar;
#[cfg(tablet)]
mod tablet;
mod toolbar;
mod ui;

pub use arrow::ArrowConfig;
pub use board::BoardConfig;
pub use boards::{BoardBackgroundConfig, BoardColorConfig, BoardItemConfig, BoardsConfig};
pub use capture::CaptureConfig;
pub use click_highlight::ClickHighlightConfig;
pub use context_menu::ContextMenuUiConfig;
pub use drawing::{DragButtonConfig, DrawingConfig, MouseDragToolsConfig};
pub use export::{
    ExportConfig, PDF_LABEL_DEFAULT_TEMPLATE, PDF_LABEL_PLACEHOLDERS, PdfExportConfig, PdfFitMode,
    PdfLabelConfig, PdfLabelContentMode, PdfLabelPosition, PdfOrientation, PdfPageSize,
    validate_pdf_label_template,
};
pub use help_overlay::HelpOverlayStyle;
pub use history::HistoryConfig;
pub use performance::PerformanceConfig;
pub use presenter_mode::{PresenterModeConfig, PresenterToolBehavior};
pub use presets::{
    PRESET_SLOTS_MAX, PRESET_SLOTS_MIN, PresetSlotsConfig, PresetToolSettingConfig,
    PresetToolStatesConfig, ToolPresetConfig,
};
pub use render_profiles::{
    RenderColorMappingConfig, RenderProfileConfig, RenderProfileExportMode, RenderProfilesConfig,
};
pub use session::{SessionCompression, SessionConfig, SessionStorageMode};
pub use status_bar::StatusBarStyle;
#[cfg(tablet)]
pub use tablet::{StylusButtonBinding, TabletInputConfig};
#[allow(unused_imports)]
pub use toolbar::{
    ToolbarConfig, ToolbarLayoutMode, ToolbarModeOverride, ToolbarModeOverrides,
    ToolbarSectionDefaults,
};
pub use ui::UiConfig;
