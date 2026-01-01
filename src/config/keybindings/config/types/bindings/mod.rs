mod board;
mod capture;
mod colors;
mod core;
mod presets;
mod selection;
mod tools;
mod ui;
mod zoom;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct KeybindingsConfig {
    #[serde(flatten, default)]
    pub core: CoreKeybindingsConfig,

    #[serde(flatten, default)]
    pub selection: SelectionKeybindingsConfig,

    #[serde(flatten, default)]
    pub tools: ToolKeybindingsConfig,

    #[serde(flatten, default)]
    pub board: BoardKeybindingsConfig,

    #[serde(flatten, default)]
    pub ui: UiKeybindingsConfig,

    #[serde(flatten, default)]
    pub colors: ColorKeybindingsConfig,

    #[serde(flatten, default)]
    pub capture: CaptureKeybindingsConfig,

    #[serde(flatten, default)]
    pub zoom: ZoomKeybindingsConfig,

    #[serde(flatten, default)]
    pub presets: PresetKeybindingsConfig,
}

pub use self::board::BoardKeybindingsConfig;
pub use self::capture::CaptureKeybindingsConfig;
pub use self::colors::ColorKeybindingsConfig;
pub use self::core::CoreKeybindingsConfig;
pub use self::presets::PresetKeybindingsConfig;
pub use self::selection::SelectionKeybindingsConfig;
pub use self::tools::ToolKeybindingsConfig;
pub use self::ui::UiKeybindingsConfig;
pub use self::zoom::ZoomKeybindingsConfig;
