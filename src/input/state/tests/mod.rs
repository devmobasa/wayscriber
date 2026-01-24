use super::core::{ContextMenuKind, ContextMenuState, MenuCommand};
use super::*;
use crate::config::{Action, BoardsConfig, ColorSpec, ToolPresetConfig};
use crate::draw::{Color, EraserKind, FontDescriptor, Shape, frame::UndoAction};
use crate::input::{ClickHighlightSettings, EraserMode, Key, MouseButton, Tool};
use crate::util;

mod helpers;
use helpers::create_test_input_state;

mod arrow_labels;
mod basics;
mod board_picker;
mod drawing;
mod erase;
mod menus;
mod presenter_mode;
mod pressure_modes;
mod selection;
mod step_markers;
mod text_edit;
mod text_input;
mod transform;
