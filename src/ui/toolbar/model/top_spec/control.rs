use super::control_meta::*;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarNode {
    Divider(TopToolbarDivider),
    Control(TopToolbarControl),
}

impl TopToolbarNode {
    /// Island membership of a strip node. Thin dividers only exist inside
    /// the tools island; the island boundaries themselves are gaps, not
    /// divider nodes.
    pub(crate) fn island(&self) -> TopToolbarIsland {
        match self {
            Self::Divider(_) => TopToolbarIsland::Tools,
            Self::Control(control) => control.island(),
        }
    }
}

/// The detached pill islands of the top strip, in reading order: tools
/// (drag grip through annotations), presets (saved tool+color slots),
/// history (undo/redo/overflow), chrome (layout cycle/About/pin/minimize).
/// Both frontends and
/// the contract tests derive island membership from this one accessor; the
/// `Ord` derive fixes the reading order the strip walk relies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TopToolbarIsland {
    Tools,
    Presets,
    History,
    Chrome,
}

impl TopToolbarIsland {
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Tools => "tools",
            Self::Presets => "presets",
            Self::History => "history",
            Self::Chrome => "chrome",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarDivider {
    Tools,
    Annotations,
}

impl TopToolbarDivider {
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Tools => "top.divider.tools",
            Self::Annotations => "top.divider.annotations",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarControl {
    Restore,
    /// The micro-mode chip: the whole strip collapsed to one round chip
    /// showing the active tool inside a ring in the current color. Like
    /// `Restore`, it is the way back and therefore never hideable.
    MicroChip,
    DragHandle,
    Tool(Tool),
    ShapePicker,
    Utility(TopToolbarUtility),
    /// One saved-preset slot (0-based index) in the presets island. Left
    /// click applies the saved preset when the slot is filled, or saves the
    /// current setup into it when the slot is empty. Renders as a compact
    /// button showing the saved tool glyph
    /// in the neutral foreground with the preset color as a separate corner
    /// swatch, or the 1-based slot number when empty.
    Preset(usize),
    Undo,
    Redo,
    ClearCanvas,
    Pin,
    Overflow,
    Minimize,
    /// Chrome-island entry opening the standalone About dialog. Activating it
    /// leaves the overlay, since About is a normal window and the overlay
    /// renders above those.
    About,
    /// Chrome-island entry cycling the layout preset Simple → Regular →
    /// Advanced → Simple. A cycle rather than a toggle, so it never reads
    /// as active; the icon shows the current mode.
    LayoutMode,
    HighlightRing,
    /// Overflow menu entry opening the Canvas popover (boards, pages, zoom,
    /// history/advanced actions, step undo/redo).
    CanvasMenu,
    /// Overflow menu entry opening the Session popover (open/save/recent).
    SessionMenu,
    /// Overflow menu entry opening the Settings popover (toolbar options
    /// and customization).
    SettingsMenu,
}

impl TopToolbarControl {
    pub(crate) fn id(self) -> TopToolbarControlId {
        let id = match self {
            Self::Restore => return TopToolbarControlId::Restore,
            Self::MicroChip => return TopToolbarControlId::MicroChip,
            Self::CanvasMenu => return TopToolbarControlId::CanvasMenu,
            Self::SessionMenu => return TopToolbarControlId::SessionMenu,
            Self::SettingsMenu => return TopToolbarControlId::SettingsMenu,
            Self::DragHandle => ids::TOP_CHROME_DRAG,
            Self::Tool(tool) => toolbar_item_id_for_tool(tool),
            Self::ShapePicker => ids::TOP_UTILITY_SHAPE_PICKER,
            Self::Utility(utility) => match utility {
                TopToolbarUtility::Text => ids::TOP_UTILITY_TEXT,
                TopToolbarUtility::StickyNote => ids::TOP_UTILITY_STICKY_NOTE,
                TopToolbarUtility::Screenshot => ids::TOP_UTILITY_SCREENSHOT,
                TopToolbarUtility::Ocr => ids::TOP_UTILITY_OCR,
                TopToolbarUtility::Highlight => ids::TOP_UTILITY_HIGHLIGHT,
            },
            Self::Preset(index) => return TopToolbarControlId::Preset(index),
            Self::Undo => ids::TOP_UTILITY_UNDO,
            Self::Redo => ids::TOP_UTILITY_REDO,
            Self::ClearCanvas => ids::TOP_UTILITY_CLEAR_CANVAS,
            Self::Pin => ids::TOP_CHROME_PIN,
            Self::Overflow => ids::TOP_CHROME_OVERFLOW,
            Self::Minimize => ids::TOP_CHROME_CLOSE,
            Self::About => ids::TOP_CHROME_ABOUT,
            Self::LayoutMode => ids::TOP_CHROME_LAYOUT,
            Self::HighlightRing => ids::TOP_UTILITY_HIGHLIGHT_RING,
        };
        TopToolbarControlId::Item(id)
    }

    pub(crate) fn event(self, snapshot: &ToolbarSnapshot) -> ToolbarEvent {
        match self {
            Self::Restore => ToolbarEvent::SetTopMinimized(false),
            Self::MicroChip => ToolbarEvent::SetTopDisplayMode(crate::config::TopDisplayMode::Full),
            Self::DragHandle => ToolbarEvent::MoveTopToolbar { x: 0.0, y: 0.0 },
            Self::Tool(tool) => ToolbarEvent::SelectTool(tool),
            Self::ShapePicker => ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            Self::Utility(utility) => utility_event(utility, snapshot),
            // Filled slots apply; empty slots save the current setup (the slot
            // is 1-based).
            Self::Preset(index) => {
                let slot = index + 1;
                if preset_slot(snapshot, index).is_some() {
                    ToolbarEvent::ApplyPreset(slot)
                } else {
                    ToolbarEvent::SavePreset(slot)
                }
            }
            Self::Undo => ToolbarEvent::Undo,
            Self::Redo => ToolbarEvent::Redo,
            // The mouse path clears with an undo toast; the frontends
            // upgrade to the instant variant when Shift is held.
            Self::ClearCanvas => ToolbarEvent::ClearCanvas { instant: false },
            Self::Pin => ToolbarEvent::PinTopToolbar(!snapshot.top_pinned),
            Self::Overflow => ToolbarEvent::ToggleTopOverflow(!snapshot.top_overflow_open),
            Self::CanvasMenu => ToolbarEvent::ToggleCanvasPopover(!snapshot.canvas_popover_open),
            Self::SessionMenu => ToolbarEvent::ToggleSessionPopover(!snapshot.session_popover_open),
            Self::SettingsMenu => {
                ToolbarEvent::ToggleSettingsPopover(!snapshot.settings_popover_open)
            }
            Self::Minimize => ToolbarEvent::SetTopMinimized(true),
            Self::About => ToolbarEvent::OpenAbout,
            Self::LayoutMode => ToolbarEvent::SetToolbarLayoutMode(snapshot.layout_mode.next()),
            Self::HighlightRing => {
                ToolbarEvent::ToggleHighlightToolRing(!snapshot.highlight_tool_ring_enabled)
            }
        }
    }

    pub(crate) fn action(self, snapshot: &ToolbarSnapshot) -> Option<Action> {
        self.event(snapshot).action()
    }

    pub(crate) fn enabled(self, snapshot: &ToolbarSnapshot) -> bool {
        match self {
            Self::Undo => snapshot.undo_available,
            Self::Redo => snapshot.redo_available,
            _ => true,
        }
    }

    pub(crate) fn active(self, snapshot: &ToolbarSnapshot) -> bool {
        match self {
            Self::Tool(tool) => {
                snapshot.active_tool == tool || snapshot.tool_override == Some(tool)
            }
            Self::ShapePicker => {
                snapshot.shape_picker_open
                    || current_shape_tool(snapshot.active_tool, snapshot.tool_override).is_some()
            }
            Self::Utility(TopToolbarUtility::Text) => snapshot.text_active,
            Self::Utility(TopToolbarUtility::StickyNote) => snapshot.note_active,
            Self::Utility(TopToolbarUtility::Highlight) => snapshot.any_highlight_active,
            Self::Utility(TopToolbarUtility::Screenshot | TopToolbarUtility::Ocr) => false,
            // A filled slot reads as active while it is the applied preset.
            Self::Preset(index) => snapshot.active_preset_slot == Some(index + 1),
            Self::Pin => snapshot.top_pinned,
            Self::Overflow => snapshot.top_overflow_open,
            Self::CanvasMenu => snapshot.canvas_popover_open,
            Self::SessionMenu => snapshot.session_popover_open,
            Self::SettingsMenu => snapshot.settings_popover_open,
            Self::HighlightRing => snapshot.highlight_tool_ring_enabled,
            _ => false,
        }
    }

    pub(crate) fn role(self) -> TopToolbarControlRole {
        match self {
            // The micro chip shares the restore role: a single non-hideable
            // control whose click brings the full strip back.
            Self::Restore | Self::MicroChip => TopToolbarControlRole::Restore,
            Self::DragHandle => TopToolbarControlRole::DragHandle,
            Self::ClearCanvas => TopToolbarControlRole::Destructive,
            Self::ShapePicker
            | Self::Utility(TopToolbarUtility::Highlight)
            | Self::Overflow
            | Self::CanvasMenu
            | Self::SessionMenu
            | Self::SettingsMenu
            | Self::HighlightRing => TopToolbarControlRole::Toggle,
            // The chrome island (layout cycle, About, pin, minimize) renders
            // quieter than the content islands; both frontends key that
            // styling off this role.
            Self::Pin | Self::Minimize | Self::About | Self::LayoutMode => {
                TopToolbarControlRole::Chrome
            }
            _ => TopToolbarControlRole::Button,
        }
    }

    /// Which pill island the control belongs to. Total over all controls so
    /// non-strip lanes (chrome, overflow, contextual) answer consistently.
    pub(crate) fn island(self) -> TopToolbarIsland {
        match self {
            Self::Undo
            | Self::Redo
            | Self::Overflow
            | Self::ClearCanvas
            | Self::CanvasMenu
            | Self::SessionMenu
            | Self::SettingsMenu => TopToolbarIsland::History,
            Self::Pin
            | Self::Minimize
            | Self::About
            | Self::LayoutMode
            | Self::Restore
            | Self::MicroChip => TopToolbarIsland::Chrome,
            Self::Preset(_) => TopToolbarIsland::Presets,
            _ => TopToolbarIsland::Tools,
        }
    }

    pub(crate) fn icon(self, snapshot: &ToolbarSnapshot) -> Option<TopToolbarIcon> {
        Some(match self {
            Self::Restore => TopToolbarIcon::Restore,
            // The chip shows the active tool's glyph; the ring around it is
            // frontend paint, not an icon.
            Self::MicroChip => TopToolbarIcon::Tool(semantic_icon_for_tool(snapshot.active_tool)),
            Self::DragHandle => TopToolbarIcon::Drag,
            Self::Tool(tool) => TopToolbarIcon::Tool(semantic_icon_for_tool(tool)),
            Self::ShapePicker => TopToolbarIcon::ShapePicker,
            Self::Utility(TopToolbarUtility::Text) => TopToolbarIcon::Text,
            Self::Utility(TopToolbarUtility::StickyNote) => TopToolbarIcon::StickyNote,
            Self::Utility(TopToolbarUtility::Screenshot) => TopToolbarIcon::Screenshot,
            Self::Utility(TopToolbarUtility::Ocr) => TopToolbarIcon::Ocr,
            Self::ClearCanvas => TopToolbarIcon::ClearCanvas,
            Self::Utility(TopToolbarUtility::Highlight) => TopToolbarIcon::Highlight,
            Self::Undo => TopToolbarIcon::Undo,
            Self::Redo => TopToolbarIcon::Redo,
            Self::Pin if snapshot.top_pinned => TopToolbarIcon::Pin,
            Self::Pin => TopToolbarIcon::Unpin,
            // The glyph shows the CURRENT mode; the click advances to the next.
            Self::LayoutMode => TopToolbarIcon::for_layout_mode(snapshot.layout_mode),
            Self::Overflow => TopToolbarIcon::Overflow,
            Self::CanvasMenu => TopToolbarIcon::Canvas,
            Self::SessionMenu => TopToolbarIcon::Session,
            Self::SettingsMenu => TopToolbarIcon::Settings,
            Self::Minimize => TopToolbarIcon::Minimize,
            Self::About => TopToolbarIcon::About,
            // Filled preset slots carry the saved tool's glyph (the renderers
            // draw it neutral and show the preset color as a corner swatch);
            // empty slots have no glyph.
            Self::Preset(index) => {
                let preset = preset_slot(snapshot, index)?;
                TopToolbarIcon::Tool(semantic_icon_for_tool(preset.tool))
            }
            Self::HighlightRing => return None,
        })
    }

    pub(crate) fn label(self, snapshot: &ToolbarSnapshot) -> Cow<'static, str> {
        match self {
            Self::Restore => Cow::Borrowed("Show toolbar"),
            Self::MicroChip => Cow::Borrowed("Show full toolbar"),
            Self::DragHandle => Cow::Borrowed("Drag toolbar"),
            Self::Tool(tool) => Cow::Borrowed(tool_label(tool)),
            Self::ShapePicker => Cow::Borrowed("Shapes"),
            Self::Utility(utility) => Cow::Borrowed(utility_short_label(utility)),
            // An empty slot's visible label is its 1-based number; once
            // filled the renderers show the tool glyph instead.
            Self::Preset(index) => Cow::Owned((index + 1).to_string()),
            Self::Undo => Cow::Borrowed(action_short_label(Action::Undo)),
            Self::Redo => Cow::Borrowed(action_short_label(Action::Redo)),
            Self::ClearCanvas => Cow::Borrowed(action_short_label(Action::ClearCanvas)),
            Self::Pin if snapshot.top_pinned => Cow::Borrowed("Unpin top toolbar"),
            Self::Pin => Cow::Borrowed("Pin top toolbar"),
            Self::Overflow => Cow::Borrowed("More tools"),
            Self::CanvasMenu => Cow::Borrowed("Canvas..."),
            Self::SessionMenu => Cow::Borrowed("Session..."),
            Self::SettingsMenu => Cow::Borrowed("Settings..."),
            Self::Minimize => Cow::Borrowed("Minimize top toolbar"),
            Self::About => Cow::Borrowed(action_short_label(Action::OpenAbout)),
            Self::LayoutMode => Cow::Borrowed("Cycle toolbar layout"),
            Self::HighlightRing => Cow::Borrowed("Ring"),
        }
    }

    pub(crate) fn accessible_label(self, snapshot: &ToolbarSnapshot) -> Cow<'static, str> {
        match self {
            Self::Tool(tool) => Cow::Borrowed(tool_tooltip_label(tool)),
            Self::Utility(utility) => Cow::Borrowed(utility_accessible_label(utility)),
            Self::Undo => Cow::Borrowed(action_label(Action::Undo)),
            Self::Redo => Cow::Borrowed(action_label(Action::Redo)),
            Self::ClearCanvas => Cow::Borrowed(action_label(Action::ClearCanvas)),
            Self::Preset(index) => Cow::Owned(preset_accessible_label(snapshot, index)),
            Self::CanvasMenu => Cow::Borrowed("Canvas menu"),
            Self::SessionMenu => Cow::Borrowed("Session menu"),
            Self::SettingsMenu => Cow::Borrowed("Settings menu"),
            Self::About => Cow::Borrowed(action_label(Action::OpenAbout)),
            _ => self.label(snapshot),
        }
    }

    pub(crate) fn tooltip(self, snapshot: &ToolbarSnapshot) -> String {
        match self {
            Self::Tool(tool) => tool_tooltip(snapshot, tool),
            Self::Utility(utility) => action_tooltip(snapshot, utility_action(utility)),
            Self::Preset(index) => preset_tooltip(snapshot, index),
            Self::Undo => action_tooltip(snapshot, Action::Undo),
            Self::Redo => action_tooltip(snapshot, Action::Redo),
            Self::ClearCanvas => action_tooltip(snapshot, Action::ClearCanvas),
            Self::Pin if snapshot.top_pinned => {
                "Pinned: opens at startup (click to disable)".to_string()
            }
            Self::Pin => "Pin: click to open at startup".to_string(),
            // Current mode first, then where the click lands, so hovering
            // reads the cycle without pressing it.
            Self::LayoutMode => match snapshot.layout_mode {
                ToolbarLayoutMode::Simple => "Layout: Simple (click for Regular)".to_string(),
                ToolbarLayoutMode::Regular => "Layout: Regular (click for Advanced)".to_string(),
                ToolbarLayoutMode::Advanced => "Layout: Advanced (click for Simple)".to_string(),
            },
            Self::Minimize => "Minimize (leaves a restore tab)".to_string(),
            Self::MicroChip => "Micro toolbar (click to show the full toolbar)".to_string(),
            Self::CanvasMenu => "Canvas: boards, pages, zoom, history, steps".to_string(),
            Self::SessionMenu => "Session: open, save, recent files".to_string(),
            Self::SettingsMenu => "Settings: toolbar options and customization".to_string(),
            Self::HighlightRing => "Highlight ring".to_string(),
            _ => self.accessible_label(snapshot).into_owned(),
        }
    }

    pub(crate) fn overflow_tooltip(self, snapshot: &ToolbarSnapshot) -> String {
        match self {
            Self::Utility(_) => self.accessible_label(snapshot).into_owned(),
            _ => self.tooltip(snapshot),
        }
    }

    pub(crate) fn shortcut_badge(self, snapshot: &ToolbarSnapshot) -> Option<String> {
        match self {
            Self::Tool(tool) => snapshot
                .binding_hints
                .badge_for_tool(tool)
                .map(str::to_owned),
            // Preset slots carry their binding in the tooltip, not a badge
            // (the slots are already numbered, so a keycap would double up).
            Self::Preset(_) => None,
            _ => self
                .action(snapshot)
                .and_then(|action| snapshot.binding_hints.badge_for_action(action))
                .map(str::to_owned),
        }
    }
}
