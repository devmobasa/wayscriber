use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarControlId {
    Item(ToolbarItemId),
    Preset(usize),
    Restore,
    MicroChip,
    CanvasMenu,
    SessionMenu,
    SettingsMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarUtility {
    Text,
    StickyNote,
    Screenshot,
    Ocr,
    Highlight,
}

impl TopToolbarUtility {
    pub(super) fn from_model(utility: TopUtilityButton) -> Option<Self> {
        match utility {
            TopUtilityButton::Text => Some(Self::Text),
            TopUtilityButton::StickyNote => Some(Self::StickyNote),
            TopUtilityButton::Screenshot => Some(Self::Screenshot),
            TopUtilityButton::Ocr => Some(Self::Ocr),
            TopUtilityButton::Highlight => Some(Self::Highlight),
            TopUtilityButton::ClearCanvas | TopUtilityButton::IconMode => None,
        }
    }
}

impl TopToolbarControlId {
    pub(crate) fn render_id(self) -> Cow<'static, str> {
        match self {
            Self::Item(id) => Cow::Borrowed(id.as_str()),
            Self::Preset(index) => Cow::Owned(format!("top.preset.{index}")),
            Self::Restore => Cow::Borrowed("top.chrome.restore"),
            Self::MicroChip => Cow::Borrowed("top.chrome.micro"),
            Self::CanvasMenu => Cow::Borrowed("top.menu.canvas"),
            Self::SessionMenu => Cow::Borrowed("top.menu.session"),
            Self::SettingsMenu => Cow::Borrowed("top.menu.settings"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarControlRole {
    Button,
    Toggle,
    Destructive,
    Chrome,
    DragHandle,
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopToolbarIcon {
    Restore,
    Drag,
    Tool(SemanticToolIcon),
    ShapePicker,
    Text,
    StickyNote,
    Screenshot,
    /// Screen text recognition: a scan frame around three text lines.
    Ocr,
    Highlight,
    ClearCanvas,
    Undo,
    Redo,
    Pin,
    Unpin,
    Overflow,
    Minimize,
    /// Canvas popover entry (stacked-boards/layers glyph).
    Canvas,
    /// Session popover entry (save-with-clock glyph).
    Session,
    /// Settings popover entry (sliders glyph).
    Settings,
    /// About entry (circled information glyph).
    About,
    /// Exit entry (a cross).
    Exit,
    /// Layout button showing Simple (one density bar).
    LayoutSimple,
    /// Layout button showing Regular (two density bars).
    LayoutRegular,
    /// Layout button showing Advanced (three density bars).
    LayoutAdvanced,
}

impl TopToolbarIcon {
    /// The density glyph for a layout preset. Total on purpose: the GTK
    /// factory renders the layout button from the mode alone, and this
    /// mapping spares it unwrapping the generic `icon()` Option.
    pub(crate) fn for_layout_mode(mode: crate::config::ToolbarLayoutMode) -> Self {
        match mode {
            crate::config::ToolbarLayoutMode::Simple => Self::LayoutSimple,
            crate::config::ToolbarLayoutMode::Regular => Self::LayoutRegular,
            crate::config::ToolbarLayoutMode::Advanced => Self::LayoutAdvanced,
        }
    }
}

pub(super) fn utility_event(
    utility: TopToolbarUtility,
    snapshot: &ToolbarSnapshot,
) -> ToolbarEvent {
    match utility {
        TopToolbarUtility::Text => ToolbarEvent::EnterTextMode,
        TopToolbarUtility::StickyNote => ToolbarEvent::EnterStickyNoteMode,
        TopToolbarUtility::Screenshot => ToolbarEvent::CaptureScreenshot,
        TopToolbarUtility::Ocr => ToolbarEvent::CopyTextFromScreen,
        TopToolbarUtility::Highlight => {
            ToolbarEvent::ToggleAllHighlight(!snapshot.any_highlight_active)
        }
    }
}

pub(super) fn utility_action(utility: TopToolbarUtility) -> Action {
    match utility {
        TopToolbarUtility::Text => Action::EnterTextMode,
        TopToolbarUtility::StickyNote => Action::EnterStickyNoteMode,
        TopToolbarUtility::Screenshot => Action::CaptureRegionInteractive,
        TopToolbarUtility::Ocr => Action::CopyTextFromScreen,
        TopToolbarUtility::Highlight => Action::ToggleHighlightTool,
    }
}

pub(super) fn utility_short_label(utility: TopToolbarUtility) -> &'static str {
    match utility {
        TopToolbarUtility::Screenshot => "Capture",
        TopToolbarUtility::Ocr => "Copy text",
        TopToolbarUtility::Highlight => "Highlight",
        _ => action_short_label(utility_action(utility)),
    }
}

pub(super) fn utility_accessible_label(utility: TopToolbarUtility) -> &'static str {
    action_label(utility_action(utility))
}

/// The filled preset saved in slot `index` (0-based), if any. Both frontends
/// read the same accessor so their per-slot rendering cannot drift.
pub(crate) fn preset_slot(
    snapshot: &ToolbarSnapshot,
    index: usize,
) -> Option<&crate::ui::toolbar::PresetSlotSnapshot> {
    snapshot.presets.get(index).and_then(Option::as_ref)
}

/// Trimmed non-empty preset name for slot `index`, if the slot is filled and
/// carries a name.
fn preset_name(snapshot: &ToolbarSnapshot, index: usize) -> Option<&str> {
    preset_slot(snapshot, index)
        .and_then(|preset| preset.name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

/// What a filled slot will apply, as "Pen, Red, 4px": the tool, then its color
/// and size where the tool has them. A name the user gave the preset leads.
///
/// The color reads as the quick-color label it matches, so it uses the words
/// the swatch tooltips use; anything else reads as hex.
fn preset_summary(snapshot: &ToolbarSnapshot, index: usize) -> Option<String> {
    let preset = preset_slot(snapshot, index)?;
    let profile = preset.tool.profile();
    // "Pen", not "Pen Tool": the summary is a list, and "Preset 1:" already
    // says what kind of thing it describes.
    let tool = tool_tooltip_label(preset.tool);
    let mut parts = vec![tool.strip_suffix(" Tool").unwrap_or(tool).to_string()];
    if profile.needs_color {
        parts.push(preset_color_label(snapshot, preset.color));
    }
    if profile.needs_thickness_control() {
        parts.push(format!("{:.0}px", preset.size));
    }

    let details = parts.join(", ");
    Some(match preset_name(snapshot, index) {
        Some(name) => format!("{name} \u{2014} {details}"),
        None => details,
    })
}

fn preset_color_label(snapshot: &ToolbarSnapshot, color: crate::draw::Color) -> String {
    const TOLERANCE: f64 = 0.5 / 255.0;
    let matches = |entry: &crate::draw::Color| {
        (entry.r - color.r).abs() <= TOLERANCE
            && (entry.g - color.g).abs() <= TOLERANCE
            && (entry.b - color.b).abs() <= TOLERANCE
    };
    if let Some(entry) = snapshot
        .quick_colors
        .rendered_entries()
        .iter()
        .find(|entry| matches(&entry.color))
    {
        return entry.label.clone();
    }

    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}",
        channel(color.r),
        channel(color.g),
        channel(color.b)
    )
}

/// Accessible label for a preset slot: what a filled slot applies, and an
/// "(empty)" note otherwise. The 1-based slot number leads either way so the
/// slots read distinctly under a screen reader.
pub(super) fn preset_accessible_label(snapshot: &ToolbarSnapshot, index: usize) -> String {
    let slot = index + 1;
    match preset_summary(snapshot, index) {
        Some(summary) => format!("Preset {slot}: {summary}"),
        None => format!("Preset {slot} (empty)"),
    }
}

/// Tooltip for a preset slot: filled slots summarize what they apply, with
/// the apply binding; empty slots say so and how to fill them, naming the
/// configured save binding when there is one.
pub(super) fn preset_tooltip(snapshot: &ToolbarSnapshot, index: usize) -> String {
    let slot = index + 1;
    match preset_summary(snapshot, index) {
        Some(summary) => format_binding_label(
            &format!("Preset {slot}: {summary}"),
            snapshot.binding_hints.apply_preset(slot),
        ),
        None => match snapshot.binding_hints.save_preset(slot) {
            Some(binding) => format!(
                "Preset {slot} (empty) \u{2014} click or press {binding} to save the current tool"
            ),
            None => format!("Preset {slot} (empty) \u{2014} click to save the current tool"),
        },
    }
}

/// Size of the minimized strip's restore tab, in spec units, shared by both
/// frontends. Large enough to hit comfortably and to carry the restore glyph
/// plus its "Tools" caption.
pub(crate) const RESTORE_TAB_SIZE: (f64, f64) = (104.0, 32.0);

/// Ring stroke width of the micro chip for a given stroke thickness.
///
/// Perceptual mapping shared by both frontends: thickness 1px → 1.5px ring,
/// growing linearly to a 5px ring at 20px thickness and clamped there — the
/// upper stroke range (to 50px) would otherwise swallow the chip.
pub(crate) fn micro_ring_width(thickness: f64) -> f64 {
    const MIN_RING: f64 = 1.5;
    const MAX_RING: f64 = 5.0;
    const THICKNESS_AT_MAX: f64 = 20.0;
    let normalized = ((thickness - 1.0) / (THICKNESS_AT_MAX - 1.0)).clamp(0.0, 1.0);
    MIN_RING + normalized * (MAX_RING - MIN_RING)
}

pub(crate) fn action_tooltip(snapshot: &ToolbarSnapshot, action: Action) -> String {
    format_binding_label(
        action_label(action),
        snapshot.binding_hints.binding_for_action(action),
    )
}

pub(super) fn tool_tooltip(snapshot: &ToolbarSnapshot, tool: Tool) -> String {
    let label = tool_tooltip_label(tool);
    let default_hint = default_drag_hint(tool);
    let binding = match (snapshot.binding_hints.for_tool(tool), default_hint) {
        (Some(binding), Some(fallback)) => Some(format!("{binding}, {fallback}")),
        (Some(binding), None) => Some(binding.to_string()),
        (None, Some(fallback)) => Some(fallback.to_string()),
        (None, None) => None,
    };
    format_binding_label(label, binding.as_deref())
}
