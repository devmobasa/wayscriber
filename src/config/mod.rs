//! Configuration file support for wayscriber.
//!
//! This module handles loading and validating user settings from the configuration file
//! located at `~/.config/wayscriber/config.toml`. Settings include drawing defaults,
//! arrow appearance, performance tuning, and UI preferences.
//!
//! If no config file exists, sensible defaults are used automatically.

pub mod action_meta;
pub mod enums;
pub mod keybindings;
pub mod types;

mod core;
mod io;
mod paths;
#[cfg(feature = "config-schema")]
mod schema;
mod validate;

#[cfg(test)]
pub(crate) mod test_helpers;
#[cfg(test)]
mod tests;

// Re-export commonly used types at module level
#[allow(unused_imports)]
pub use action_meta::{
    ActionCategory, ActionMeta, action_description, action_display_label, action_label,
    action_meta, action_meta_iter, action_short_label,
};
pub use core::Config;
pub use enums::{RadialMenuMouseBinding, StatusPosition, XdgFocusLossBehavior};
#[allow(unused_imports)]
pub use io::{ConfigSource, LoadedConfig};
pub use keybindings::{Action, KeyBinding, KeybindingsConfig};
#[allow(unused_imports)]
pub use types::{
    ArrowConfig, BoardBackgroundConfig, BoardColorConfig, BoardConfig, BoardItemConfig,
    BoardsConfig, CaptureConfig, ClickHighlightConfig, DragButtonConfig, DrawingConfig,
    ExportConfig, HelpOverlayStyle, HistoryConfig, MouseDragToolsConfig, PDF_LABEL_APP_BOARD,
    PDF_LABEL_APP_BOARDS, PDF_LABEL_BOARD_NAME, PDF_LABEL_DEFAULT_TEMPLATE,
    PDF_LABEL_DOCUMENT_PAGE, PDF_LABEL_DOCUMENT_PAGES, PDF_LABEL_EXPORT_BOARD,
    PDF_LABEL_EXPORT_BOARDS, PDF_LABEL_PAGE, PDF_LABEL_PAGE_NAME, PDF_LABEL_PAGES,
    PDF_LABEL_PLACEHOLDERS, PRESET_SLOTS_MAX, PRESET_SLOTS_MIN, PdfExportConfig, PdfFitMode,
    PdfLabelConfig, PdfLabelContentMode, PdfLabelPosition, PdfOrientation, PdfPageSize,
    PdfTransparentBackground, PerformanceConfig, PresenterModeConfig, PresenterToolBehavior,
    PresetSlotsConfig, PresetToolSettingConfig, PresetToolStatesConfig, QuickColorConfig,
    QuickColorPalette, QuickColorPaletteEntry, QuickColorSlot, QuickColorsConfig,
    RenderColorMappingConfig, RenderProfileConfig, RenderProfileExportMode, RenderProfilesConfig,
    ResolvedToolbarItems, SessionCompression, SessionConfig, SessionStorageMode, StatusBarStyle,
    ToolPresetConfig, ToolbarConfig, ToolbarGroupId, ToolbarItemCategory, ToolbarItemDefinition,
    ToolbarItemId, ToolbarItemOrderConfig, ToolbarItemOrderGroup, ToolbarItemSurface,
    ToolbarItemsConfig, ToolbarLayoutMode, ToolbarModeOverride, ToolbarModeOverrides, UiConfig,
    toolbar_item_definitions, toolbar_item_ids, toolbar_item_order_group,
    validate_pdf_label_template,
};
#[cfg(tablet)]
#[allow(unused_imports)]
pub use types::{StylusButtonBinding, TabletInputConfig};

// Re-export for public API (unused internally but part of public interface)
#[allow(unused_imports)]
pub use enums::ColorSpec;

#[cfg(test)]
pub(crate) use paths::PRIMARY_CONFIG_DIR;
