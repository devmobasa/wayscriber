use std::path::PathBuf;
use std::sync::Arc;

use wayscriber::config::Config;

use crate::models::{
    BoardModeOption, ColorMode, FontStyleOption, FontWeightOption, KeybindingField,
    NamedColorOption, QuadField, SessionCompressionOption, SessionStorageModeOption,
    StatusPositionOption, TabId, TextField, ToggleField, TripletField,
};

#[derive(Debug, Clone)]
pub enum Message {
    ConfigLoaded(Result<Arc<Config>, String>),
    ReloadRequested,
    ResetToDefaults,
    SaveRequested,
    ConfigSaved(Result<(Option<PathBuf>, Arc<Config>), String>),
    TabSelected(TabId),
    ToggleChanged(ToggleField, bool),
    TextChanged(TextField, String),
    TripletChanged(TripletField, usize, String),
    QuadChanged(QuadField, usize, String),
    ColorModeChanged(ColorMode),
    NamedColorSelected(NamedColorOption),
    StatusPositionChanged(StatusPositionOption),
    BoardModeChanged(BoardModeOption),
    SessionStorageModeChanged(SessionStorageModeOption),
    SessionCompressionChanged(SessionCompressionOption),
    BufferCountChanged(u32),
    KeybindingChanged(KeybindingField, String),
    FontStyleOptionSelected(FontStyleOption),
    FontWeightOptionSelected(FontWeightOption),
}
