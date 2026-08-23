mod capture;
mod eraser;
mod export;
mod font;
mod input_hud;
mod presenter;
#[cfg(feature = "tablet-input")]
mod pressure;
mod session;
mod status;
mod theme;
mod toggles;
mod tool;
mod toolbar;

pub use capture::RegionPickerOption;
pub use eraser::{EraserModeOption, PresetEraserKindOption, PresetEraserModeOption};
pub use export::{
    PdfFitModeOption, PdfLabelContentModeOption, PdfLabelPositionOption, PdfOrientationOption,
    PdfPageSizeOption, PdfTransparentBackgroundOption,
};
pub use font::{FontStyleOption, FontWeightOption};
pub use input_hud::{InputHudModeOption, InputHudPositionOption};
pub use presenter::{PresenterToolBehaviorOption, PresenterToolbarModeOption};
#[cfg(feature = "tablet-input")]
pub use pressure::{PressureThicknessEditModeOption, PressureThicknessEntryModeOption};
pub use session::{SessionCompressionOption, SessionStorageModeOption};
pub use status::StatusPositionOption;
pub use theme::{ReducedMotionOption, UiThemeOption};
pub use toggles::{PresetTextField, PresetToggleField, QuadField, TextField, ToggleField};
pub use tool::{DragColorOption, DragMouseButton, DragToolField, DragToolOption, ToolOption};
pub use toolbar::{
    OverrideOption, ToolbarLayoutModeOption, ToolbarOverrideField, ToolbarRebindModifierOption,
    ZoomChipDisplayOption,
};

#[cfg(test)]
mod tests;
