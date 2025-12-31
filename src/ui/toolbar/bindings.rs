use crate::config::{KeybindingsConfig, PRESET_SLOTS_MAX};
use crate::input::Tool;

use super::events::ToolbarEvent;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ToolbarBindingHints {
    pub pen: Option<String>,
    pub line: Option<String>,
    pub rect: Option<String>,
    pub ellipse: Option<String>,
    pub arrow: Option<String>,
    pub marker: Option<String>,
    pub highlight: Option<String>,
    pub eraser: Option<String>,
    pub toggle_eraser_mode: Option<String>,
    pub text: Option<String>,
    pub note: Option<String>,
    pub clear: Option<String>,
    pub fill: Option<String>,
    pub toggle_highlight: Option<String>,
    pub undo: Option<String>,
    pub redo: Option<String>,
    pub undo_all: Option<String>,
    pub redo_all: Option<String>,
    pub undo_all_delayed: Option<String>,
    pub redo_all_delayed: Option<String>,
    pub toggle_freeze: Option<String>,
    pub zoom_in: Option<String>,
    pub zoom_out: Option<String>,
    pub reset_zoom: Option<String>,
    pub toggle_zoom_lock: Option<String>,
    pub page_prev: Option<String>,
    pub page_next: Option<String>,
    pub page_new: Option<String>,
    pub page_duplicate: Option<String>,
    pub page_delete: Option<String>,
    pub open_configurator: Option<String>,
    pub apply_presets: Vec<Option<String>>,
    pub save_presets: Vec<Option<String>>,
    pub clear_presets: Vec<Option<String>>,
}

impl ToolbarBindingHints {
    pub fn for_tool(&self, tool: Tool) -> Option<&str> {
        match tool {
            Tool::Pen => self.pen.as_deref(),
            Tool::Line => self.line.as_deref(),
            Tool::Rect => self.rect.as_deref(),
            Tool::Ellipse => self.ellipse.as_deref(),
            Tool::Arrow => self.arrow.as_deref(),
            Tool::Marker => self.marker.as_deref(),
            Tool::Highlight => self.highlight.as_deref(),
            Tool::Eraser => self.eraser.as_deref(),
            Tool::Select => None,
        }
    }

    pub fn from_keybindings(kb: &KeybindingsConfig) -> Self {
        let first = |v: &Vec<String>| v.first().cloned();
        let mut apply_presets = vec![None; PRESET_SLOTS_MAX];
        let mut save_presets = vec![None; PRESET_SLOTS_MAX];
        let mut clear_presets = vec![None; PRESET_SLOTS_MAX];
        if PRESET_SLOTS_MAX >= 1 {
            apply_presets[0] = first(&kb.apply_preset_1);
            save_presets[0] = first(&kb.save_preset_1);
            clear_presets[0] = first(&kb.clear_preset_1);
        }
        if PRESET_SLOTS_MAX >= 2 {
            apply_presets[1] = first(&kb.apply_preset_2);
            save_presets[1] = first(&kb.save_preset_2);
            clear_presets[1] = first(&kb.clear_preset_2);
        }
        if PRESET_SLOTS_MAX >= 3 {
            apply_presets[2] = first(&kb.apply_preset_3);
            save_presets[2] = first(&kb.save_preset_3);
            clear_presets[2] = first(&kb.clear_preset_3);
        }
        if PRESET_SLOTS_MAX >= 4 {
            apply_presets[3] = first(&kb.apply_preset_4);
            save_presets[3] = first(&kb.save_preset_4);
            clear_presets[3] = first(&kb.clear_preset_4);
        }
        if PRESET_SLOTS_MAX >= 5 {
            apply_presets[4] = first(&kb.apply_preset_5);
            save_presets[4] = first(&kb.save_preset_5);
            clear_presets[4] = first(&kb.clear_preset_5);
        }
        Self {
            pen: first(&kb.select_pen_tool),
            line: first(&kb.select_line_tool),
            rect: first(&kb.select_rect_tool),
            ellipse: first(&kb.select_ellipse_tool),
            arrow: first(&kb.select_arrow_tool),
            marker: first(&kb.select_marker_tool),
            highlight: first(&kb.select_highlight_tool),
            eraser: first(&kb.select_eraser_tool),
            toggle_eraser_mode: first(&kb.toggle_eraser_mode),
            text: first(&kb.enter_text_mode),
            note: first(&kb.enter_sticky_note_mode),
            clear: first(&kb.clear_canvas),
            fill: first(&kb.toggle_fill),
            toggle_highlight: first(&kb.toggle_highlight_tool),
            undo: first(&kb.undo),
            redo: first(&kb.redo),
            undo_all: first(&kb.undo_all),
            redo_all: first(&kb.redo_all),
            undo_all_delayed: first(&kb.undo_all_delayed),
            redo_all_delayed: first(&kb.redo_all_delayed),
            toggle_freeze: first(&kb.toggle_frozen_mode),
            zoom_in: first(&kb.zoom_in),
            zoom_out: first(&kb.zoom_out),
            reset_zoom: first(&kb.reset_zoom),
            toggle_zoom_lock: first(&kb.toggle_zoom_lock),
            page_prev: first(&kb.page_prev),
            page_next: first(&kb.page_next),
            page_new: first(&kb.page_new),
            page_duplicate: first(&kb.page_duplicate),
            page_delete: first(&kb.page_delete),
            open_configurator: first(&kb.open_configurator),
            apply_presets,
            save_presets,
            clear_presets,
        }
    }

    fn preset_binding(slots: &[Option<String>], slot: usize) -> Option<&str> {
        if slot == 0 {
            return None;
        }
        slots.get(slot - 1).and_then(|binding| binding.as_deref())
    }

    pub fn apply_preset(&self, slot: usize) -> Option<&str> {
        Self::preset_binding(&self.apply_presets, slot)
    }

    pub fn save_preset(&self, slot: usize) -> Option<&str> {
        Self::preset_binding(&self.save_presets, slot)
    }

    pub fn clear_preset(&self, slot: usize) -> Option<&str> {
        Self::preset_binding(&self.clear_presets, slot)
    }

    pub fn binding_for_event(&self, event: &ToolbarEvent) -> Option<&str> {
        match event {
            ToolbarEvent::Undo => self.undo.as_deref(),
            ToolbarEvent::Redo => self.redo.as_deref(),
            ToolbarEvent::UndoAll => self.undo_all.as_deref(),
            ToolbarEvent::RedoAll => self.redo_all.as_deref(),
            ToolbarEvent::UndoAllDelayed => self.undo_all_delayed.as_deref(),
            ToolbarEvent::RedoAllDelayed => self.redo_all_delayed.as_deref(),
            ToolbarEvent::ToggleFreeze => self.toggle_freeze.as_deref(),
            ToolbarEvent::ZoomIn => self.zoom_in.as_deref(),
            ToolbarEvent::ZoomOut => self.zoom_out.as_deref(),
            ToolbarEvent::ResetZoom => self.reset_zoom.as_deref(),
            ToolbarEvent::ToggleZoomLock => self.toggle_zoom_lock.as_deref(),
            ToolbarEvent::PagePrev => self.page_prev.as_deref(),
            ToolbarEvent::PageNext => self.page_next.as_deref(),
            ToolbarEvent::PageNew => self.page_new.as_deref(),
            ToolbarEvent::PageDuplicate => self.page_duplicate.as_deref(),
            ToolbarEvent::PageDelete => self.page_delete.as_deref(),
            ToolbarEvent::OpenConfigurator => self.open_configurator.as_deref(),
            ToolbarEvent::ClearCanvas => self.clear.as_deref(),
            ToolbarEvent::ToggleAllHighlight(_) => self.toggle_highlight.as_deref(),
            _ => None,
        }
    }
}
