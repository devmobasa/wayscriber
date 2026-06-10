use crate::config::{ResolvedToolbarItems, ToolbarGroupId, ToolbarItemId, ToolbarLayoutMode};
use crate::draw::{Color, EraserKind, FontDescriptor};
use crate::input::state::PresetFeedbackKind;
use crate::input::tool::{ToolControlGroup, ToolProfile};
use crate::input::{EraserMode, Tool, ToolbarDrawerTab};
use std::path::PathBuf;

use super::super::bindings::ToolbarBindingHints;
use super::super::events::ToolbarSideSection;
use std::collections::BTreeSet;

/// The kind of tool-specific options to display in the side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOptionsKind {
    /// No tool-specific options (e.g., Select tool)
    None,
    /// Pen/Line tools: thickness only
    Stroke,
    /// Marker tool: thickness + opacity
    Marker,
    /// Eraser tool: eraser size + mode toggle
    Eraser,
    /// Shape tools (Rect/Ellipse): thickness + fill toggle
    Shape,
    /// Arrow tool: thickness + labels toggle + counter
    Arrow,
    /// StepMarker tool: size + counter
    StepMarker,
    /// Text mode: font size + font family
    Text,
}

fn tool_options_kind_from_group(group: ToolControlGroup) -> ToolOptionsKind {
    match group {
        ToolControlGroup::None => ToolOptionsKind::None,
        ToolControlGroup::Stroke => ToolOptionsKind::Stroke,
        ToolControlGroup::Marker => ToolOptionsKind::Marker,
        ToolControlGroup::Eraser => ToolOptionsKind::Eraser,
        ToolControlGroup::Shape => ToolOptionsKind::Shape,
        ToolControlGroup::Arrow => ToolOptionsKind::Arrow,
        ToolControlGroup::StepMarker => ToolOptionsKind::StepMarker,
    }
}

/// Context that determines which UI sections to show based on the active tool.
///
/// This enables contextual/adaptive UI - showing only relevant controls for the
/// current tool rather than all options at once.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolContext {
    /// Whether to show the color section (compact swatch + popover)
    pub needs_color: bool,
    /// Whether to show a thickness/size slider
    pub needs_thickness: bool,
    /// The kind of tool-specific options to display
    pub tool_options_kind: ToolOptionsKind,
    /// Label for the thickness/size slider
    pub thickness_label: &'static str,
    /// Whether the fill toggle should be shown (for shapes)
    pub show_fill_toggle: bool,
    /// Whether the arrow labels section should be shown
    pub show_arrow_labels: bool,
    /// Whether the step marker counter should be shown
    pub show_step_counter: bool,
    /// Whether the eraser mode toggle should be shown
    pub show_eraser_mode: bool,
    /// Whether the marker opacity slider should be shown
    pub show_marker_opacity: bool,
    /// Whether the regular polygon side-count control should be shown
    pub show_polygon_sides_control: bool,
    /// Whether font controls should be shown
    pub show_font_controls: bool,
}

impl ToolContext {
    /// Compute the tool context from the current toolbar state.
    pub fn from_snapshot(snapshot: &ToolbarSnapshot) -> Self {
        // If context-aware UI is disabled, show all sections (classic behavior)
        if !snapshot.context_aware_ui {
            return Self::all_visible(snapshot);
        }

        let effective_tool = snapshot.active_tool;
        let text_or_note_active = snapshot.text_active || snapshot.note_active;

        // If text/note mode is active, show text controls
        if text_or_note_active {
            return Self {
                needs_color: true,
                needs_thickness: false,
                tool_options_kind: ToolOptionsKind::Text,
                thickness_label: "",
                show_fill_toggle: false,
                show_arrow_labels: false,
                show_step_counter: false,
                show_eraser_mode: false,
                // Honor show_marker_opacity_section setting
                show_marker_opacity: snapshot.show_marker_opacity_section,
                show_polygon_sides_control: false,
                show_font_controls: true,
            };
        }

        // Compute base context from the tool catalog, then apply snapshot settings.
        let mut ctx = Self::from_profile(effective_tool.profile());

        // Honor snapshot settings that override tool-based visibility.
        // Temporary drag bindings can make the active size target differ from
        // the selected top-toolbar override, so keep size-specific controls
        // aligned with the snapshot target flags.
        if snapshot.thickness_targets_eraser {
            ctx.needs_thickness = true;
            ctx.tool_options_kind = ToolOptionsKind::Eraser;
            ctx.thickness_label = "Eraser size";
            ctx.show_eraser_mode = true;
        }
        if snapshot.thickness_targets_marker {
            ctx.show_marker_opacity = true;
        }
        // show_text_controls: keep font controls visible even when text mode is inactive
        if snapshot.show_text_controls {
            ctx.show_font_controls = true;
        }
        // show_marker_opacity_section: keep opacity slider visible for all tools
        if snapshot.show_marker_opacity_section {
            ctx.show_marker_opacity = true;
        }
        if effective_tool == Tool::RegularPolygon {
            ctx.show_polygon_sides_control = true;
        }

        ctx
    }

    fn from_profile(profile: ToolProfile) -> Self {
        Self {
            needs_color: profile.needs_color,
            needs_thickness: profile.needs_thickness_control(),
            tool_options_kind: tool_options_kind_from_group(profile.control_group),
            thickness_label: profile.thickness_label,
            show_fill_toggle: profile.show_fill_toggle(),
            show_arrow_labels: profile.show_arrow_labels(),
            show_step_counter: profile.show_step_counter(),
            show_eraser_mode: profile.show_eraser_mode(),
            show_marker_opacity: profile.show_marker_opacity(),
            show_polygon_sides_control: false,
            show_font_controls: false,
        }
    }

    /// Returns a context where all sections are visible (classic/non-contextual behavior).
    fn all_visible(snapshot: &ToolbarSnapshot) -> Self {
        let effective_tool = snapshot.active_tool;
        let profile = effective_tool.profile();
        let text_or_note_active = snapshot.text_active || snapshot.note_active;
        let show_arrow_labels = profile.show_arrow_labels() || snapshot.arrow_label_enabled;
        let show_step_counter = profile.show_step_counter();
        let show_marker_opacity =
            snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;
        let show_font_controls = text_or_note_active || snapshot.show_text_controls;
        let show_polygon_sides_control = snapshot.active_tool == Tool::RegularPolygon;

        Self {
            needs_color: true,
            needs_thickness: true,
            tool_options_kind: ToolOptionsKind::Stroke, // Generic
            thickness_label: if snapshot.thickness_targets_eraser {
                "Eraser size"
            } else {
                "Thickness"
            },
            show_fill_toggle: false, // Only shown contextually for shapes
            show_arrow_labels,
            show_step_counter,
            show_eraser_mode: snapshot.thickness_targets_eraser,
            show_marker_opacity,
            show_polygon_sides_control,
            show_font_controls,
        }
    }
}

/// Snapshot of a single preset slot for toolbar display.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetSlotSnapshot {
    pub name: Option<String>,
    pub tool: Tool,
    pub color: Color,
    pub size: f64,
    pub eraser_kind: Option<EraserKind>,
    pub eraser_mode: Option<EraserMode>,
    pub marker_opacity: Option<f64>,
    pub fill_enabled: Option<bool>,
    pub font_size: Option<f64>,
    pub text_background_enabled: Option<bool>,
    pub arrow_length: Option<f64>,
    pub arrow_angle: Option<f64>,
    pub arrow_head_at_end: Option<bool>,
    pub show_status_bar: Option<bool>,
}

/// Snapshot of an in-progress preset feedback animation.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetFeedbackSnapshot {
    pub kind: PresetFeedbackKind,
    pub progress: f32,
}

/// Snapshot of a recent session entry for toolbar display.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecentSnapshot {
    pub display_name: String,
    pub path: PathBuf,
}

/// Snapshot of state mirrored to the toolbar UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarSnapshot {
    pub active_tool: Tool,
    pub tool_override: Option<Tool>,
    pub color: Color,
    pub thickness: f64,
    pub eraser_size: f64,
    pub thickness_targets_eraser: bool,
    pub thickness_targets_marker: bool,
    pub eraser_kind: EraserKind,
    pub eraser_mode: EraserMode,
    pub marker_opacity: f64,
    pub font: FontDescriptor,
    pub font_size: f64,
    pub text_active: bool,
    pub note_active: bool,
    pub frozen_active: bool,
    pub zoom_active: bool,
    pub zoom_locked: bool,
    pub fill_enabled: bool,
    pub polygon_sides: u8,
    pub arrow_label_enabled: bool,
    pub arrow_label_next: u32,
    pub step_marker_next: u32,
    pub undo_available: bool,
    pub redo_available: bool,
    pub board_index: usize,
    pub board_count: usize,
    pub board_name: String,
    pub board_color: Option<Color>,
    pub page_index: usize,
    pub page_count: usize,
    pub click_highlight_enabled: bool,
    pub highlight_tool_active: bool,
    pub highlight_tool_ring_enabled: bool,
    /// Whether any highlight feature is active (tool or click)
    pub any_highlight_active: bool,
    pub undo_all_delay_ms: u64,
    pub redo_all_delay_ms: u64,
    pub custom_section_enabled: bool,
    pub show_delay_sliders: bool,
    /// Whether to show delayed undo/redo actions in the toolbar
    pub delay_actions_enabled: bool,
    pub custom_undo_delay_ms: u64,
    pub custom_redo_delay_ms: u64,
    pub custom_undo_steps: usize,
    pub custom_redo_steps: usize,
    /// Whether the top toolbar is pinned (opens at startup)
    pub top_pinned: bool,
    /// Whether the side toolbar is pinned (opens at startup)
    pub side_pinned: bool,
    /// Whether to use icons instead of text labels
    pub use_icons: bool,
    /// Scale factor for toolbar UI (icons + layout)
    pub toolbar_scale: f64,
    /// Current toolbar layout mode
    pub layout_mode: ToolbarLayoutMode,
    /// Resolved known item-level toolbar visibility config.
    pub resolved_toolbar_items: ResolvedToolbarItems,
    /// Whether to show extended color palette
    pub show_more_colors: bool,
    /// Whether to show the Actions section
    pub show_actions_section: bool,
    /// Whether to show advanced action buttons
    pub show_actions_advanced: bool,
    /// Whether to show zoom actions
    pub show_zoom_actions: bool,
    /// Whether to show the Pages section
    pub show_pages_section: bool,
    /// Whether to show the Boards section
    pub show_boards_section: bool,
    /// Whether to show the marker opacity slider section
    pub show_marker_opacity_section: bool,
    /// Whether to show preset action toasts
    pub show_preset_toasts: bool,
    /// Whether to show presets in the side toolbar
    pub show_presets: bool,
    /// Whether to show the Step Undo/Redo section
    pub show_step_section: bool,
    /// Whether to keep text controls visible when text is inactive
    pub show_text_controls: bool,
    /// Whether to enable context-aware UI that shows/hides controls based on active tool
    pub context_aware_ui: bool,
    /// Whether to show the Settings section
    pub show_settings_section: bool,
    /// Side drawer sections whose body content is collapsed.
    pub collapsed_side_sections: BTreeSet<ToolbarSideSection>,
    pub show_tool_preview: bool,
    pub show_status_bar: bool,
    pub show_status_board_badge: bool,
    pub show_status_page_badge: bool,
    pub show_floating_badge_always: bool,
    /// Whether the simple-mode shape picker is expanded
    pub shape_picker_open: bool,
    /// Whether the drawer is open
    pub drawer_open: bool,
    /// Active drawer tab
    pub drawer_tab: ToolbarDrawerTab,
    /// Whether the Settings drawer is showing the toolbar item customization sub-panel.
    pub customize_items_open: bool,
    /// Selected toolbar item customization group in the Settings drawer sub-panel.
    pub customize_items_group: Option<super::super::events::ToolbarItemCustomizeGroup>,
    /// Number of preset slots to display
    pub preset_slot_count: usize,
    /// Preset slot previews
    pub presets: Vec<Option<PresetSlotSnapshot>>,
    /// Currently active preset slot
    pub active_preset_slot: Option<usize>,
    /// Transient preset feedback animations
    pub preset_feedback: Vec<Option<PresetFeedbackSnapshot>>,
    /// Binding hints for tooltips
    pub binding_hints: ToolbarBindingHints,
    /// Whether to show the drawer onboarding hint (first-time users)
    pub show_drawer_hint: bool,
    /// Whether the current board is the transparent overlay
    pub is_transparent: bool,
    /// Changes whenever final-render color profile preview changes.
    pub render_profile_generation: u64,
    /// Active persisted session name, if persistence is active.
    pub active_session_name: Option<String>,
    /// Active persisted session path, if persistence is active.
    pub active_session_path: Option<PathBuf>,
    /// Recent persisted sessions from the catalog.
    pub recent_sessions: Vec<SessionRecentSnapshot>,
    /// Save Session As target waiting for explicit overwrite confirmation.
    pub pending_save_as_overwrite_path: Option<PathBuf>,
}

impl ToolbarSnapshot {
    pub fn toolbar_item_hidden(&self, item: ToolbarItemId) -> bool {
        self.resolved_toolbar_items.is_hidden(item)
    }

    pub fn toolbar_group_hidden(&self, group: ToolbarGroupId) -> bool {
        self.toolbar_item_hidden(group.toolbar_item_id())
    }

    pub fn side_section_hidden(&self, section: ToolbarSideSection) -> bool {
        let group = match section {
            ToolbarSideSection::Colors => ToolbarGroupId::Colors,
            ToolbarSideSection::Presets => ToolbarGroupId::Presets,
            ToolbarSideSection::Thickness => ToolbarGroupId::Thickness,
            ToolbarSideSection::EraserMode => ToolbarGroupId::EraserMode,
            ToolbarSideSection::PolygonSides => ToolbarGroupId::PolygonSides,
            ToolbarSideSection::ArrowLabels => ToolbarGroupId::ArrowLabels,
            ToolbarSideSection::StepMarkers => ToolbarGroupId::StepMarkers,
            ToolbarSideSection::MarkerOpacity => ToolbarGroupId::MarkerOpacity,
            ToolbarSideSection::TextSize => ToolbarGroupId::TextSize,
            ToolbarSideSection::Font => ToolbarGroupId::Font,
            ToolbarSideSection::Actions => ToolbarGroupId::Actions,
            ToolbarSideSection::Boards => ToolbarGroupId::Boards,
            ToolbarSideSection::Pages => ToolbarGroupId::Pages,
            ToolbarSideSection::StepUndo => ToolbarGroupId::StepUndo,
            ToolbarSideSection::Session => ToolbarGroupId::Session,
            ToolbarSideSection::Settings => ToolbarGroupId::Settings,
        };
        let legacy_item = match section {
            ToolbarSideSection::Colors => Some("side.tool-options.color"),
            ToolbarSideSection::Thickness => Some("side.tool-options.thickness"),
            ToolbarSideSection::EraserMode => Some("side.tool-options.eraser-mode"),
            ToolbarSideSection::PolygonSides => Some("side.tool-options.polygon-sides"),
            ToolbarSideSection::ArrowLabels => Some("side.tool-options.arrow-labels"),
            ToolbarSideSection::StepMarkers => Some("side.tool-options.step-marker-reset"),
            ToolbarSideSection::MarkerOpacity => Some("side.tool-options.marker-opacity"),
            ToolbarSideSection::TextSize => Some("side.tool-options.font-size"),
            ToolbarSideSection::Font => Some("side.tool-options.font-family"),
            ToolbarSideSection::Presets
            | ToolbarSideSection::Actions
            | ToolbarSideSection::Boards
            | ToolbarSideSection::Pages
            | ToolbarSideSection::StepUndo
            | ToolbarSideSection::Session
            | ToolbarSideSection::Settings => None,
        };

        self.toolbar_group_hidden(group)
            || legacy_item.is_some_and(|item| {
                self.toolbar_item_hidden(ToolbarItemId::from_known(item))
            })
    }

    pub fn side_section_collapsed(&self, section: ToolbarSideSection) -> bool {
        self.collapsed_side_sections.contains(&section)
    }
}
