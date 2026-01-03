use super::core::{ContextMenuKind, ContextMenuState, MenuCommand};
use super::*;
use crate::config::{Action, BoardsConfig, ColorSpec, ToolPresetConfig};
use crate::draw::{Color, EraserKind, FontDescriptor, Shape, frame::UndoAction};
use crate::input::{ClickHighlightSettings, EraserMode, Key, MouseButton, Tool};
use crate::util;

mod helpers;
use helpers::create_test_input_state;

mod basics;
mod board_picker;
mod drawing;
mod erase;
mod menus;
mod selection;
mod text_edit;
mod text_input;
mod transform;
