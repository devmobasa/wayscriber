use crate::config::{
    ToolbarGroupId, ToolbarItemCategory, ToolbarItemDefinition, ToolbarItemId,
    toolbar_item_ids as ids,
};
use crate::input::Tool;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection, ToolbarSnapshot};

use super::activation::ToolbarControlId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopChromeControl {
    Drag,
    Pin,
    Close,
}

impl TopChromeControl {
    pub(crate) const fn item_id(self) -> ToolbarItemId {
        match self {
            Self::Drag => ids::TOP_CHROME_DRAG,
            Self::Pin => ids::TOP_CHROME_PIN,
            Self::Close => ids::TOP_CHROME_CLOSE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopUtilityItem {
    ShapePicker,
    Fill,
    Text,
    StickyNote,
    ClearCanvas,
    Screenshot,
    Highlight,
    HighlightRing,
    IconModeIcons,
    IconModeText,
}

impl TopUtilityItem {
    pub(crate) const fn item_id(self) -> ToolbarItemId {
        match self {
            Self::ShapePicker => ids::TOP_UTILITY_SHAPE_PICKER,
            Self::Fill => ids::TOP_UTILITY_FILL,
            Self::Text => ids::TOP_UTILITY_TEXT,
            Self::StickyNote => ids::TOP_UTILITY_STICKY_NOTE,
            Self::ClearCanvas => ids::TOP_UTILITY_CLEAR_CANVAS,
            Self::Screenshot => ids::TOP_UTILITY_SCREENSHOT,
            Self::Highlight => ids::TOP_UTILITY_HIGHLIGHT,
            Self::HighlightRing => ids::TOP_UTILITY_HIGHLIGHT_RING,
            Self::IconModeIcons => ids::TOP_UTILITY_ICON_MODE_ICONS,
            Self::IconModeText => ids::TOP_UTILITY_ICON_MODE_TEXT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopUtilityButton {
    Text,
    StickyNote,
    Screenshot,
    ClearCanvas,
    Highlight,
    IconMode,
}

impl TopUtilityButton {
    pub(crate) fn id(self, snapshot: &ToolbarSnapshot) -> ToolbarItemId {
        top_utility_button_item_id(self, snapshot.use_icons)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarRuntimeRole {
    TopChrome(TopChromeControl),
    TopTool(Tool),
    TopUtility(TopUtilityItem),
    SideSection(ToolbarSideSection),
    ActionButton,
    PageButton,
    BoardButton,
    SettingControl(ToolbarControlId),
    SessionButton,
    ToolOptionAlias(ToolbarSideSection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolbarCatalogEntry {
    pub(crate) id: ToolbarItemId,
    pub(crate) role: ToolbarRuntimeRole,
}

const CATALOG_ENTRIES: &[ToolbarCatalogEntry] = &[
    entry(
        ids::TOP_CHROME_DRAG,
        ToolbarRuntimeRole::TopChrome(TopChromeControl::Drag),
    ),
    entry(
        ids::TOP_CHROME_PIN,
        ToolbarRuntimeRole::TopChrome(TopChromeControl::Pin),
    ),
    entry(
        ids::TOP_CHROME_CLOSE,
        ToolbarRuntimeRole::TopChrome(TopChromeControl::Close),
    ),
    entry(
        ids::TOP_TOOL_SELECT,
        ToolbarRuntimeRole::TopTool(Tool::Select),
    ),
    entry(ids::TOP_TOOL_PEN, ToolbarRuntimeRole::TopTool(Tool::Pen)),
    entry(
        ids::TOP_TOOL_MARKER,
        ToolbarRuntimeRole::TopTool(Tool::Marker),
    ),
    entry(
        ids::TOP_TOOL_STEP_MARKER,
        ToolbarRuntimeRole::TopTool(Tool::StepMarker),
    ),
    entry(
        ids::TOP_TOOL_ERASER,
        ToolbarRuntimeRole::TopTool(Tool::Eraser),
    ),
    entry(ids::TOP_TOOL_LINE, ToolbarRuntimeRole::TopTool(Tool::Line)),
    entry(ids::TOP_TOOL_RECT, ToolbarRuntimeRole::TopTool(Tool::Rect)),
    entry(
        ids::TOP_TOOL_ELLIPSE,
        ToolbarRuntimeRole::TopTool(Tool::Ellipse),
    ),
    entry(
        ids::TOP_TOOL_ARROW,
        ToolbarRuntimeRole::TopTool(Tool::Arrow),
    ),
    entry(ids::TOP_TOOL_BLUR, ToolbarRuntimeRole::TopTool(Tool::Blur)),
    entry(
        ids::TOP_TOOL_TRIANGLE,
        ToolbarRuntimeRole::TopTool(Tool::Triangle),
    ),
    entry(
        ids::TOP_TOOL_PARALLELOGRAM,
        ToolbarRuntimeRole::TopTool(Tool::Parallelogram),
    ),
    entry(
        ids::TOP_TOOL_RHOMBUS,
        ToolbarRuntimeRole::TopTool(Tool::Rhombus),
    ),
    entry(
        ids::TOP_TOOL_REGULAR_POLYGON,
        ToolbarRuntimeRole::TopTool(Tool::RegularPolygon),
    ),
    entry(
        ids::TOP_TOOL_FREEFORM_POLYGON,
        ToolbarRuntimeRole::TopTool(Tool::FreeformPolygon),
    ),
    entry(
        ids::TOP_UTILITY_SHAPE_PICKER,
        ToolbarRuntimeRole::TopUtility(TopUtilityItem::ShapePicker),
    ),
    entry(
        ids::TOP_UTILITY_FILL,
        ToolbarRuntimeRole::TopUtility(TopUtilityItem::Fill),
    ),
    entry(
        ids::TOP_UTILITY_TEXT,
        ToolbarRuntimeRole::TopUtility(TopUtilityItem::Text),
    ),
    entry(
        ids::TOP_UTILITY_STICKY_NOTE,
        ToolbarRuntimeRole::TopUtility(TopUtilityItem::StickyNote),
    ),
    entry(
        ids::TOP_UTILITY_CLEAR_CANVAS,
        ToolbarRuntimeRole::TopUtility(TopUtilityItem::ClearCanvas),
    ),
    entry(
        ids::TOP_UTILITY_SCREENSHOT,
        ToolbarRuntimeRole::TopUtility(TopUtilityItem::Screenshot),
    ),
    entry(
        ids::TOP_UTILITY_HIGHLIGHT,
        ToolbarRuntimeRole::TopUtility(TopUtilityItem::Highlight),
    ),
    entry(
        ids::TOP_UTILITY_HIGHLIGHT_RING,
        ToolbarRuntimeRole::TopUtility(TopUtilityItem::HighlightRing),
    ),
    entry(
        ids::TOP_UTILITY_ICON_MODE_ICONS,
        ToolbarRuntimeRole::TopUtility(TopUtilityItem::IconModeIcons),
    ),
    entry(
        ids::TOP_UTILITY_ICON_MODE_TEXT,
        ToolbarRuntimeRole::TopUtility(TopUtilityItem::IconModeText),
    ),
    entry(
        ToolbarGroupId::Colors.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::Colors),
    ),
    entry(
        ToolbarGroupId::Presets.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::Presets),
    ),
    entry(
        ToolbarGroupId::Thickness.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::Thickness),
    ),
    entry(
        ToolbarGroupId::EraserMode.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::EraserMode),
    ),
    entry(
        ToolbarGroupId::PolygonSides.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::PolygonSides),
    ),
    entry(
        ToolbarGroupId::ArrowLabels.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::ArrowLabels),
    ),
    entry(
        ToolbarGroupId::StepMarkers.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::StepMarkers),
    ),
    entry(
        ToolbarGroupId::MarkerOpacity.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::MarkerOpacity),
    ),
    entry(
        ToolbarGroupId::TextSize.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::TextSize),
    ),
    entry(
        ToolbarGroupId::Font.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::Font),
    ),
    entry(
        ToolbarGroupId::Actions.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::Actions),
    ),
    entry(
        ToolbarGroupId::Boards.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::Boards),
    ),
    entry(
        ToolbarGroupId::Pages.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::Pages),
    ),
    entry(
        ToolbarGroupId::StepUndo.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::StepUndo),
    ),
    entry(
        ToolbarGroupId::Session.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::Session),
    ),
    entry(
        ToolbarGroupId::Settings.toolbar_item_id(),
        ToolbarRuntimeRole::SideSection(ToolbarSideSection::Settings),
    ),
    entry(ids::SIDE_ACTIONS_UNDO, ToolbarRuntimeRole::ActionButton),
    entry(ids::SIDE_ACTIONS_REDO, ToolbarRuntimeRole::ActionButton),
    entry(
        ids::SIDE_ACTIONS_CLEAR_CANVAS,
        ToolbarRuntimeRole::ActionButton,
    ),
    entry(ids::SIDE_ACTIONS_ZOOM_IN, ToolbarRuntimeRole::ActionButton),
    entry(ids::SIDE_ACTIONS_ZOOM_OUT, ToolbarRuntimeRole::ActionButton),
    entry(
        ids::SIDE_ACTIONS_RESET_ZOOM,
        ToolbarRuntimeRole::ActionButton,
    ),
    entry(
        ids::SIDE_ACTIONS_TOGGLE_ZOOM_LOCK,
        ToolbarRuntimeRole::ActionButton,
    ),
    entry(ids::SIDE_ACTIONS_UNDO_ALL, ToolbarRuntimeRole::ActionButton),
    entry(ids::SIDE_ACTIONS_REDO_ALL, ToolbarRuntimeRole::ActionButton),
    entry(
        ids::SIDE_ACTIONS_UNDO_ALL_DELAYED,
        ToolbarRuntimeRole::ActionButton,
    ),
    entry(
        ids::SIDE_ACTIONS_REDO_ALL_DELAYED,
        ToolbarRuntimeRole::ActionButton,
    ),
    entry(ids::SIDE_ACTIONS_FREEZE, ToolbarRuntimeRole::ActionButton),
    entry(ids::SIDE_PAGES_PREVIOUS, ToolbarRuntimeRole::PageButton),
    entry(ids::SIDE_PAGES_NEXT, ToolbarRuntimeRole::PageButton),
    entry(ids::SIDE_PAGES_NEW, ToolbarRuntimeRole::PageButton),
    entry(ids::SIDE_PAGES_DUPLICATE, ToolbarRuntimeRole::PageButton),
    entry(ids::SIDE_PAGES_DELETE, ToolbarRuntimeRole::PageButton),
    entry(ids::SIDE_BOARDS_PREVIOUS, ToolbarRuntimeRole::BoardButton),
    entry(ids::SIDE_BOARDS_NEXT, ToolbarRuntimeRole::BoardButton),
    entry(ids::SIDE_BOARDS_NEW, ToolbarRuntimeRole::BoardButton),
    entry(ids::SIDE_BOARDS_DUPLICATE, ToolbarRuntimeRole::BoardButton),
    entry(ids::SIDE_BOARDS_DELETE, ToolbarRuntimeRole::BoardButton),
    entry(ids::SIDE_BOARDS_RENAME, ToolbarRuntimeRole::BoardButton),
    entry(
        ids::SIDE_SETTINGS_CONTEXT_AWARE_UI,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsContextAwareUi),
    ),
    entry(
        ids::SIDE_SETTINGS_TEXT_CONTROLS,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsTextControls),
    ),
    entry(
        ids::SIDE_SETTINGS_STATUS_BAR,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsStatusBar),
    ),
    entry(
        ids::SIDE_SETTINGS_STATUS_BOARD_BADGE,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsStatusBoardBadge),
    ),
    entry(
        ids::SIDE_SETTINGS_STATUS_PAGE_BADGE,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsStatusPageBadge),
    ),
    entry(
        ids::SIDE_SETTINGS_FLOATING_BADGE_ALWAYS,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsFloatingBadgeAlways),
    ),
    entry(
        ids::SIDE_SETTINGS_PRESET_TOASTS,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsPresetToasts),
    ),
    entry(
        ids::SIDE_SETTINGS_PRESETS,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsPresets),
    ),
    entry(
        ids::SIDE_SETTINGS_ACTIONS,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsActions),
    ),
    entry(
        ids::SIDE_SETTINGS_ZOOM_ACTIONS,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsZoomActions),
    ),
    entry(
        ids::SIDE_SETTINGS_ADVANCED_ACTIONS,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsAdvancedActions),
    ),
    entry(
        ids::SIDE_SETTINGS_BOARDS,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsBoards),
    ),
    entry(
        ids::SIDE_SETTINGS_PAGES,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsPages),
    ),
    entry(
        ids::SIDE_SETTINGS_STEP_CONTROLS,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::SettingsStepControls),
    ),
    entry(
        ids::SIDE_SETTINGS_CONFIGURATOR,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::OpenConfigurator),
    ),
    entry(
        ids::SIDE_SETTINGS_CONFIG_FILE,
        ToolbarRuntimeRole::SettingControl(ToolbarControlId::OpenConfigFile),
    ),
    entry(ids::SIDE_SESSION_OPEN, ToolbarRuntimeRole::SessionButton),
    entry(ids::SIDE_SESSION_SAVE_AS, ToolbarRuntimeRole::SessionButton),
    entry(ids::SIDE_SESSION_INFO, ToolbarRuntimeRole::SessionButton),
    entry(ids::SIDE_SESSION_CLEAR, ToolbarRuntimeRole::SessionButton),
    entry(ids::SIDE_SESSION_MANAGER, ToolbarRuntimeRole::SessionButton),
    entry(
        ids::SIDE_TOOL_OPTIONS_COLOR,
        ToolbarRuntimeRole::ToolOptionAlias(ToolbarSideSection::Colors),
    ),
    entry(
        ids::SIDE_TOOL_OPTIONS_THICKNESS,
        ToolbarRuntimeRole::ToolOptionAlias(ToolbarSideSection::Thickness),
    ),
    entry(
        ids::SIDE_TOOL_OPTIONS_MARKER_OPACITY,
        ToolbarRuntimeRole::ToolOptionAlias(ToolbarSideSection::MarkerOpacity),
    ),
    entry(
        ids::SIDE_TOOL_OPTIONS_ERASER_MODE,
        ToolbarRuntimeRole::ToolOptionAlias(ToolbarSideSection::EraserMode),
    ),
    entry(
        ids::SIDE_TOOL_OPTIONS_FONT_SIZE,
        ToolbarRuntimeRole::ToolOptionAlias(ToolbarSideSection::TextSize),
    ),
    entry(
        ids::SIDE_TOOL_OPTIONS_FONT_FAMILY,
        ToolbarRuntimeRole::ToolOptionAlias(ToolbarSideSection::Font),
    ),
    entry(
        ids::SIDE_TOOL_OPTIONS_POLYGON_SIDES,
        ToolbarRuntimeRole::ToolOptionAlias(ToolbarSideSection::PolygonSides),
    ),
    entry(
        ids::SIDE_TOOL_OPTIONS_ARROW_LABELS,
        ToolbarRuntimeRole::ToolOptionAlias(ToolbarSideSection::ArrowLabels),
    ),
    entry(
        ids::SIDE_TOOL_OPTIONS_STEP_MARKER_RESET,
        ToolbarRuntimeRole::ToolOptionAlias(ToolbarSideSection::StepMarkers),
    ),
];

const fn entry(id: ToolbarItemId, role: ToolbarRuntimeRole) -> ToolbarCatalogEntry {
    ToolbarCatalogEntry { id, role }
}

pub(crate) fn toolbar_catalog_entries() -> &'static [ToolbarCatalogEntry] {
    CATALOG_ENTRIES
}

pub(crate) fn catalog_entry_for_item_id(id: ToolbarItemId) -> Option<ToolbarCatalogEntry> {
    CATALOG_ENTRIES.iter().copied().find(|entry| entry.id == id)
}

pub(crate) fn catalog_entry_for_definition(
    definition: &ToolbarItemDefinition,
) -> Option<ToolbarCatalogEntry> {
    catalog_entry_for_item_id(definition.id)
}

pub(crate) fn toolbar_item_visible(snapshot: &ToolbarSnapshot, id: ToolbarItemId) -> bool {
    !snapshot.toolbar_item_hidden(id)
}

pub(crate) fn toolbar_item_id_for_tool(tool: Tool) -> ToolbarItemId {
    match tool {
        Tool::Highlight => ids::TOP_UTILITY_HIGHLIGHT,
        _ => CATALOG_ENTRIES
            .iter()
            .find_map(|entry| match entry.role {
                ToolbarRuntimeRole::TopTool(candidate) if candidate == tool => Some(entry.id),
                _ => None,
            })
            .expect("all toolbar tools have catalog items"),
    }
}

pub(crate) fn tool_for_toolbar_item_id(id: ToolbarItemId) -> Option<Tool> {
    match catalog_entry_for_item_id(id)?.role {
        ToolbarRuntimeRole::TopTool(tool) => Some(tool),
        _ => None,
    }
}

pub(crate) fn top_chrome_control_for_item_id(id: ToolbarItemId) -> Option<TopChromeControl> {
    match catalog_entry_for_item_id(id)?.role {
        ToolbarRuntimeRole::TopChrome(control) => Some(control),
        _ => None,
    }
}

pub(crate) fn top_chrome_visible(snapshot: &ToolbarSnapshot, control: TopChromeControl) -> bool {
    toolbar_item_visible(snapshot, control.item_id())
}

pub(crate) fn top_drag_visible(snapshot: &ToolbarSnapshot) -> bool {
    top_chrome_visible(snapshot, TopChromeControl::Drag)
}

pub(crate) fn top_pin_visible(snapshot: &ToolbarSnapshot) -> bool {
    top_chrome_visible(snapshot, TopChromeControl::Pin)
}

pub(crate) fn top_close_visible(snapshot: &ToolbarSnapshot) -> bool {
    top_chrome_visible(snapshot, TopChromeControl::Close)
}

pub(crate) fn top_utility_item_for_id(id: ToolbarItemId) -> Option<TopUtilityItem> {
    match catalog_entry_for_item_id(id)?.role {
        ToolbarRuntimeRole::TopUtility(item) => Some(item),
        _ => None,
    }
}

pub(crate) fn top_utility_item_visible(snapshot: &ToolbarSnapshot, item: TopUtilityItem) -> bool {
    toolbar_item_visible(snapshot, item.item_id())
}

pub(crate) fn top_shape_picker_visible(snapshot: &ToolbarSnapshot) -> bool {
    top_utility_item_visible(snapshot, TopUtilityItem::ShapePicker)
}

pub(crate) fn top_fill_visible(snapshot: &ToolbarSnapshot) -> bool {
    top_utility_item_visible(snapshot, TopUtilityItem::Fill)
}

pub(crate) fn top_text_visible(snapshot: &ToolbarSnapshot) -> bool {
    top_utility_item_visible(snapshot, TopUtilityItem::Text)
}

pub(crate) fn top_sticky_note_visible(snapshot: &ToolbarSnapshot) -> bool {
    top_utility_item_visible(snapshot, TopUtilityItem::StickyNote)
}

pub(crate) fn top_clear_canvas_visible(snapshot: &ToolbarSnapshot) -> bool {
    top_utility_item_visible(snapshot, TopUtilityItem::ClearCanvas)
}

pub(crate) fn top_screenshot_visible(snapshot: &ToolbarSnapshot) -> bool {
    top_utility_item_visible(snapshot, TopUtilityItem::Screenshot)
}

pub(crate) fn top_highlight_visible(snapshot: &ToolbarSnapshot) -> bool {
    top_utility_item_visible(snapshot, TopUtilityItem::Highlight)
}

pub(crate) fn top_highlight_ring_visible(snapshot: &ToolbarSnapshot) -> bool {
    top_utility_item_visible(snapshot, TopUtilityItem::HighlightRing)
}

pub(crate) fn top_icon_mode_toggle_visible(snapshot: &ToolbarSnapshot) -> bool {
    let item = if snapshot.use_icons {
        TopUtilityItem::IconModeText
    } else {
        TopUtilityItem::IconModeIcons
    };
    top_utility_item_visible(snapshot, item)
}

pub(crate) fn top_utility_button_item_id(
    button: TopUtilityButton,
    use_icons: bool,
) -> ToolbarItemId {
    match button {
        TopUtilityButton::Text => TopUtilityItem::Text.item_id(),
        TopUtilityButton::StickyNote => TopUtilityItem::StickyNote.item_id(),
        TopUtilityButton::Screenshot => TopUtilityItem::Screenshot.item_id(),
        TopUtilityButton::ClearCanvas => TopUtilityItem::ClearCanvas.item_id(),
        TopUtilityButton::Highlight => TopUtilityItem::Highlight.item_id(),
        TopUtilityButton::IconMode if use_icons => TopUtilityItem::IconModeText.item_id(),
        TopUtilityButton::IconMode => TopUtilityItem::IconModeIcons.item_id(),
    }
}

pub(crate) fn top_utility_button_for_item_id(id: ToolbarItemId) -> Option<TopUtilityButton> {
    match top_utility_item_for_id(id)? {
        TopUtilityItem::Text => Some(TopUtilityButton::Text),
        TopUtilityItem::StickyNote => Some(TopUtilityButton::StickyNote),
        TopUtilityItem::Screenshot => Some(TopUtilityButton::Screenshot),
        TopUtilityItem::ClearCanvas => Some(TopUtilityButton::ClearCanvas),
        TopUtilityItem::Highlight => Some(TopUtilityButton::Highlight),
        TopUtilityItem::IconModeIcons | TopUtilityItem::IconModeText => {
            Some(TopUtilityButton::IconMode)
        }
        TopUtilityItem::ShapePicker | TopUtilityItem::Fill | TopUtilityItem::HighlightRing => None,
    }
}

pub(crate) fn side_section_item_id(section: ToolbarSideSection) -> Option<ToolbarItemId> {
    CATALOG_ENTRIES.iter().find_map(|entry| match entry.role {
        ToolbarRuntimeRole::SideSection(candidate) if candidate == section => Some(entry.id),
        _ => None,
    })
}

pub(crate) fn side_section_for_toolbar_item_id(id: ToolbarItemId) -> Option<ToolbarSideSection> {
    match catalog_entry_for_item_id(id)?.role {
        ToolbarRuntimeRole::SideSection(section) => Some(section),
        _ => None,
    }
}

pub(crate) fn side_section_legacy_hidden_aliases(
    section: ToolbarSideSection,
) -> impl Iterator<Item = ToolbarItemId> {
    CATALOG_ENTRIES
        .iter()
        .filter_map(move |entry| match entry.role {
            ToolbarRuntimeRole::ToolOptionAlias(candidate) if candidate == section => {
                Some(entry.id)
            }
            _ => None,
        })
}

pub(crate) fn side_section_hidden(snapshot: &ToolbarSnapshot, section: ToolbarSideSection) -> bool {
    side_section_item_id(section).is_some_and(|id| snapshot.toolbar_item_hidden(id))
        || side_section_legacy_hidden_aliases(section)
            .any(|alias| snapshot.toolbar_item_hidden(alias))
}

pub(crate) fn action_button_item_id(event: &ToolbarEvent) -> Option<ToolbarItemId> {
    Some(match event {
        ToolbarEvent::Undo => ids::SIDE_ACTIONS_UNDO,
        ToolbarEvent::Redo => ids::SIDE_ACTIONS_REDO,
        ToolbarEvent::ClearCanvas => ids::SIDE_ACTIONS_CLEAR_CANVAS,
        ToolbarEvent::ZoomIn => ids::SIDE_ACTIONS_ZOOM_IN,
        ToolbarEvent::ZoomOut => ids::SIDE_ACTIONS_ZOOM_OUT,
        ToolbarEvent::ResetZoom => ids::SIDE_ACTIONS_RESET_ZOOM,
        ToolbarEvent::ToggleZoomLock => ids::SIDE_ACTIONS_TOGGLE_ZOOM_LOCK,
        ToolbarEvent::UndoAll => ids::SIDE_ACTIONS_UNDO_ALL,
        ToolbarEvent::RedoAll => ids::SIDE_ACTIONS_REDO_ALL,
        ToolbarEvent::UndoAllDelayed => ids::SIDE_ACTIONS_UNDO_ALL_DELAYED,
        ToolbarEvent::RedoAllDelayed => ids::SIDE_ACTIONS_REDO_ALL_DELAYED,
        ToolbarEvent::ToggleFreeze => ids::SIDE_ACTIONS_FREEZE,
        _ => return None,
    })
}

pub(crate) fn page_button_item_id(event: &ToolbarEvent) -> Option<ToolbarItemId> {
    Some(match event {
        ToolbarEvent::PagePrev => ids::SIDE_PAGES_PREVIOUS,
        ToolbarEvent::PageNext => ids::SIDE_PAGES_NEXT,
        ToolbarEvent::PageNew => ids::SIDE_PAGES_NEW,
        ToolbarEvent::PageDuplicate => ids::SIDE_PAGES_DUPLICATE,
        ToolbarEvent::PageDelete => ids::SIDE_PAGES_DELETE,
        _ => return None,
    })
}

pub(crate) fn board_button_item_id(event: &ToolbarEvent) -> Option<ToolbarItemId> {
    Some(match event {
        ToolbarEvent::BoardPrev => ids::SIDE_BOARDS_PREVIOUS,
        ToolbarEvent::BoardNext => ids::SIDE_BOARDS_NEXT,
        ToolbarEvent::BoardNew => ids::SIDE_BOARDS_NEW,
        ToolbarEvent::BoardDuplicate => ids::SIDE_BOARDS_DUPLICATE,
        ToolbarEvent::BoardDelete => ids::SIDE_BOARDS_DELETE,
        ToolbarEvent::BoardRename => ids::SIDE_BOARDS_RENAME,
        _ => return None,
    })
}

pub(crate) fn command_button_item_id(event: &ToolbarEvent) -> Option<ToolbarItemId> {
    action_button_item_id(event)
        .or_else(|| page_button_item_id(event))
        .or_else(|| board_button_item_id(event))
}

pub(crate) fn session_button_item_id(event: &ToolbarEvent) -> Option<ToolbarItemId> {
    Some(match event {
        ToolbarEvent::OpenSession => ids::SIDE_SESSION_OPEN,
        ToolbarEvent::SaveSessionAs => ids::SIDE_SESSION_SAVE_AS,
        ToolbarEvent::SessionInfo => ids::SIDE_SESSION_INFO,
        ToolbarEvent::ClearSession => ids::SIDE_SESSION_CLEAR,
        ToolbarEvent::OpenConfigurator => ids::SIDE_SESSION_MANAGER,
        _ => return None,
    })
}

pub(crate) fn settings_control_item_id(id: ToolbarControlId) -> Option<ToolbarItemId> {
    CATALOG_ENTRIES.iter().find_map(|entry| match entry.role {
        ToolbarRuntimeRole::SettingControl(candidate) if candidate == id => Some(entry.id),
        _ => None,
    })
}

pub(crate) fn runtime_category_covered(category: ToolbarItemCategory) -> bool {
    matches!(
        category,
        ToolbarItemCategory::Chrome
            | ToolbarItemCategory::Tool
            | ToolbarItemCategory::Utility
            | ToolbarItemCategory::Group
            | ToolbarItemCategory::Action
            | ToolbarItemCategory::Page
            | ToolbarItemCategory::Board
            | ToolbarItemCategory::Setting
            | ToolbarItemCategory::Session
            | ToolbarItemCategory::ToolOption
    )
}

pub(crate) fn toolbar_definition_has_runtime_metadata(definition: &ToolbarItemDefinition) -> bool {
    !runtime_category_covered(definition.category)
        || catalog_entry_for_definition(definition).is_some()
}

pub(crate) fn all_catalog_item_ids() -> impl Iterator<Item = ToolbarItemId> {
    CATALOG_ENTRIES.iter().map(|entry| entry.id)
}

pub(crate) fn all_toolbar_tool_entries() -> impl Iterator<Item = (ToolbarItemId, Tool)> {
    CATALOG_ENTRIES.iter().filter_map(|entry| match entry.role {
        ToolbarRuntimeRole::TopTool(tool) => Some((entry.id, tool)),
        _ => None,
    })
}

pub(crate) fn all_top_chrome_entries() -> impl Iterator<Item = (ToolbarItemId, TopChromeControl)> {
    CATALOG_ENTRIES.iter().filter_map(|entry| match entry.role {
        ToolbarRuntimeRole::TopChrome(control) => Some((entry.id, control)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::config::toolbar_item_definitions;

    #[test]
    fn runtime_catalog_covers_toolbar_definitions() {
        for definition in toolbar_item_definitions() {
            assert!(
                toolbar_definition_has_runtime_metadata(definition),
                "missing runtime catalog metadata for {}",
                definition.id
            );
        }
    }

    #[test]
    fn runtime_catalog_entries_are_unique_and_parseable() {
        let mut seen = BTreeSet::new();

        for id in all_catalog_item_ids() {
            assert!(seen.insert(id), "duplicate catalog item id: {id}");
            assert!(
                toolbar_item_definitions()
                    .iter()
                    .any(|definition| definition.id == id),
                "catalog id has no config definition: {id}"
            );
        }
    }

    #[test]
    fn top_tools_round_trip_through_catalog() {
        for (id, tool) in all_toolbar_tool_entries() {
            assert_eq!(toolbar_item_id_for_tool(tool), id);
            assert_eq!(tool_for_toolbar_item_id(id), Some(tool));
        }
    }

    #[test]
    fn chrome_controls_are_first_class_catalog_items() {
        let expected = [
            (ids::TOP_CHROME_DRAG, TopChromeControl::Drag),
            (ids::TOP_CHROME_PIN, TopChromeControl::Pin),
            (ids::TOP_CHROME_CLOSE, TopChromeControl::Close),
        ];

        for (id, control) in expected {
            assert_eq!(top_chrome_control_for_item_id(id), Some(control));
            assert_eq!(control.item_id(), id);
        }
        assert_eq!(all_top_chrome_entries().count(), expected.len());
    }

    #[test]
    fn command_button_events_resolve_to_stable_items() {
        let expected = [
            (ToolbarEvent::Undo, ids::SIDE_ACTIONS_UNDO),
            (ToolbarEvent::Redo, ids::SIDE_ACTIONS_REDO),
            (ToolbarEvent::ClearCanvas, ids::SIDE_ACTIONS_CLEAR_CANVAS),
            (ToolbarEvent::ZoomIn, ids::SIDE_ACTIONS_ZOOM_IN),
            (ToolbarEvent::ZoomOut, ids::SIDE_ACTIONS_ZOOM_OUT),
            (ToolbarEvent::ResetZoom, ids::SIDE_ACTIONS_RESET_ZOOM),
            (
                ToolbarEvent::ToggleZoomLock,
                ids::SIDE_ACTIONS_TOGGLE_ZOOM_LOCK,
            ),
            (ToolbarEvent::UndoAll, ids::SIDE_ACTIONS_UNDO_ALL),
            (ToolbarEvent::RedoAll, ids::SIDE_ACTIONS_REDO_ALL),
            (
                ToolbarEvent::UndoAllDelayed,
                ids::SIDE_ACTIONS_UNDO_ALL_DELAYED,
            ),
            (
                ToolbarEvent::RedoAllDelayed,
                ids::SIDE_ACTIONS_REDO_ALL_DELAYED,
            ),
            (ToolbarEvent::ToggleFreeze, ids::SIDE_ACTIONS_FREEZE),
            (ToolbarEvent::PagePrev, ids::SIDE_PAGES_PREVIOUS),
            (ToolbarEvent::PageNext, ids::SIDE_PAGES_NEXT),
            (ToolbarEvent::PageNew, ids::SIDE_PAGES_NEW),
            (ToolbarEvent::PageDuplicate, ids::SIDE_PAGES_DUPLICATE),
            (ToolbarEvent::PageDelete, ids::SIDE_PAGES_DELETE),
            (ToolbarEvent::BoardPrev, ids::SIDE_BOARDS_PREVIOUS),
            (ToolbarEvent::BoardNext, ids::SIDE_BOARDS_NEXT),
            (ToolbarEvent::BoardNew, ids::SIDE_BOARDS_NEW),
            (ToolbarEvent::BoardDuplicate, ids::SIDE_BOARDS_DUPLICATE),
            (ToolbarEvent::BoardDelete, ids::SIDE_BOARDS_DELETE),
            (ToolbarEvent::BoardRename, ids::SIDE_BOARDS_RENAME),
        ];

        for (event, id) in expected {
            assert_eq!(command_button_item_id(&event), Some(id));
        }
    }

    #[test]
    fn session_button_events_resolve_to_stable_items() {
        let expected = [
            (ToolbarEvent::OpenSession, ids::SIDE_SESSION_OPEN),
            (ToolbarEvent::SaveSessionAs, ids::SIDE_SESSION_SAVE_AS),
            (ToolbarEvent::SessionInfo, ids::SIDE_SESSION_INFO),
            (ToolbarEvent::ClearSession, ids::SIDE_SESSION_CLEAR),
            (ToolbarEvent::OpenConfigurator, ids::SIDE_SESSION_MANAGER),
        ];

        for (event, id) in expected {
            assert_eq!(session_button_item_id(&event), Some(id));
        }
    }

    #[test]
    fn settings_controls_resolve_to_stable_items() {
        let expected = [
            (
                ToolbarControlId::SettingsContextAwareUi,
                ids::SIDE_SETTINGS_CONTEXT_AWARE_UI,
            ),
            (
                ToolbarControlId::SettingsTextControls,
                ids::SIDE_SETTINGS_TEXT_CONTROLS,
            ),
            (
                ToolbarControlId::SettingsStatusBar,
                ids::SIDE_SETTINGS_STATUS_BAR,
            ),
            (
                ToolbarControlId::SettingsStatusBoardBadge,
                ids::SIDE_SETTINGS_STATUS_BOARD_BADGE,
            ),
            (
                ToolbarControlId::SettingsStatusPageBadge,
                ids::SIDE_SETTINGS_STATUS_PAGE_BADGE,
            ),
            (
                ToolbarControlId::SettingsFloatingBadgeAlways,
                ids::SIDE_SETTINGS_FLOATING_BADGE_ALWAYS,
            ),
            (
                ToolbarControlId::SettingsPresetToasts,
                ids::SIDE_SETTINGS_PRESET_TOASTS,
            ),
            (
                ToolbarControlId::SettingsPresets,
                ids::SIDE_SETTINGS_PRESETS,
            ),
            (
                ToolbarControlId::SettingsActions,
                ids::SIDE_SETTINGS_ACTIONS,
            ),
            (
                ToolbarControlId::SettingsZoomActions,
                ids::SIDE_SETTINGS_ZOOM_ACTIONS,
            ),
            (
                ToolbarControlId::SettingsAdvancedActions,
                ids::SIDE_SETTINGS_ADVANCED_ACTIONS,
            ),
            (ToolbarControlId::SettingsBoards, ids::SIDE_SETTINGS_BOARDS),
            (ToolbarControlId::SettingsPages, ids::SIDE_SETTINGS_PAGES),
            (
                ToolbarControlId::SettingsStepControls,
                ids::SIDE_SETTINGS_STEP_CONTROLS,
            ),
            (
                ToolbarControlId::OpenConfigurator,
                ids::SIDE_SETTINGS_CONFIGURATOR,
            ),
            (
                ToolbarControlId::OpenConfigFile,
                ids::SIDE_SETTINGS_CONFIG_FILE,
            ),
        ];

        for (control, id) in expected {
            assert_eq!(settings_control_item_id(control), Some(id));
        }
    }

    #[test]
    fn legacy_tool_option_aliases_resolve_to_side_sections() {
        let expected = [
            (ToolbarSideSection::Colors, ids::SIDE_TOOL_OPTIONS_COLOR),
            (
                ToolbarSideSection::Thickness,
                ids::SIDE_TOOL_OPTIONS_THICKNESS,
            ),
            (
                ToolbarSideSection::EraserMode,
                ids::SIDE_TOOL_OPTIONS_ERASER_MODE,
            ),
            (
                ToolbarSideSection::PolygonSides,
                ids::SIDE_TOOL_OPTIONS_POLYGON_SIDES,
            ),
            (
                ToolbarSideSection::ArrowLabels,
                ids::SIDE_TOOL_OPTIONS_ARROW_LABELS,
            ),
            (
                ToolbarSideSection::StepMarkers,
                ids::SIDE_TOOL_OPTIONS_STEP_MARKER_RESET,
            ),
            (
                ToolbarSideSection::MarkerOpacity,
                ids::SIDE_TOOL_OPTIONS_MARKER_OPACITY,
            ),
            (
                ToolbarSideSection::TextSize,
                ids::SIDE_TOOL_OPTIONS_FONT_SIZE,
            ),
            (ToolbarSideSection::Font, ids::SIDE_TOOL_OPTIONS_FONT_FAMILY),
        ];

        for (section, id) in expected {
            let aliases: Vec<_> = side_section_legacy_hidden_aliases(section).collect();
            assert_eq!(aliases, vec![id]);
        }
    }
}
