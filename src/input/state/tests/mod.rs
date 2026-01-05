use super::core::{ContextMenuKind, ContextMenuState, MenuCommand};
use super::*;
use crate::config::{Action, BoardConfig, ColorSpec, ToolPresetConfig};
use crate::draw::{Color, EraserKind, FontDescriptor, Shape, frame::UndoAction};
use crate::input::{BoardMode, ClickHighlightSettings, EraserMode, Key, MouseButton, Tool};
use crate::util;

mod helpers;
use helpers::create_test_input_state;

mod arrow_labels;
mod basics;
mod drawing;
mod erase;
mod menus;
mod presenter_mode;
mod selection;
mod text_edit;
mod text_input;
mod transform;
