use std::path::PathBuf;
use std::sync::Arc;

use wayscriber::config::Config;

use crate::models::{
    BoardModeOption, ColorMode, EraserModeOption, FontStyleOption, FontWeightOption,
    KeybindingField, NamedColorOption, OverrideOption, PresetEraserKindOption,
    PresetEraserModeOption, PresetTextField, PresetToggleField, QuadField,
    SessionCompressionOption, SessionStorageModeOption, StatusPositionOption, TabId, TextField,
    ToggleField, ToolOption, ToolbarLayoutModeOption, ToolbarOverrideField, TripletField, UiTabId,
};

#[derive(Debug, Clone)]
pub enum Message {
    ConfigLoaded(Result<Arc<Config>, String>),
    ReloadRequested,
    ResetToDefaults,
    SaveRequested,
    ConfigSaved(Result<(Option<PathBuf>, Arc<Config>), String>),
    TabSelected(TabId),
    UiTabSelected(UiTabId),
    ToggleChanged(ToggleField, bool),
    TextChanged(TextField, String),
    TripletChanged(TripletField, usize, String),
    QuadChanged(QuadField, usize, String),
    ColorModeChanged(ColorMode),
    NamedColorSelected(NamedColorOption),
    EraserModeChanged(EraserModeOption),
    StatusPositionChanged(StatusPositionOption),
    ToolbarLayoutModeChanged(ToolbarLayoutModeOption),
    ToolbarOverrideModeChanged(ToolbarLayoutModeOption),
    ToolbarOverrideChanged(ToolbarOverrideField, OverrideOption),
    BoardModeChanged(BoardModeOption),
    SessionStorageModeChanged(SessionStorageModeOption),
    SessionCompressionChanged(SessionCompressionOption),
    BufferCountChanged(u32),
    KeybindingChanged(KeybindingField, String),
    FontStyleOptionSelected(FontStyleOption),
    FontWeightOptionSelected(FontWeightOption),
    PresetSlotCountChanged(usize),
    PresetSlotEnabledChanged(usize, bool),
    PresetToolChanged(usize, ToolOption),
    PresetColorModeChanged(usize, ColorMode),
    PresetNamedColorSelected(usize, NamedColorOption),
    PresetColorComponentChanged(usize, usize, String),
    PresetTextChanged(usize, PresetTextField, String),
    PresetToggleOptionChanged(usize, PresetToggleField, OverrideOption),
    PresetEraserKindChanged(usize, PresetEraserKindOption),
    PresetEraserModeChanged(usize, PresetEraserModeOption),
}
