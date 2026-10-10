#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RoutingOutcome {
    Consumed(ConsumedBy),
    Started(ActiveInteractionKind),
    Continued(ActiveInteractionKind),
    Finished(ActiveInteractionKind),
    Canceled(CancelTarget),
    SideEffect(InteractionSideEffect),
    DispatchedAction(ActionRoute),
    NoRoute(NoRouteReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConsumedBy {
    EscapeDismissalGuard,
    CommandPalette,
    HelpOverlay,
    RadialMenu,
    ColorPickerPopup,
    FontPicker,
    PrecisionEntry,
    ContextMenu,
    BoardPicker,
    PropertiesPanel,
    TextInput,
    ToolButton,
    RightClickContextMenu,
    RadialMenuToggle,
    StatusHud,
    ZoomChip,
    SequencePrefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveInteractionKind {
    Drawing,
    BuildingPolygon,
    TextInput,
    PendingTextClick,
    MovingSelection,
    BoxSelecting,
    ResizingText,
    ResizingSelection,
    BendingArrow,
    AdjustingSpotlightMagnification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CancelTarget {
    ActiveInteraction(ActiveInteractionKind),
    PendingBoardDelete,
    PendingPageDelete,
    Selection,
    /// An open top-strip menu or popover (Escape dismissal).
    TopMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InteractionSideEffect {
    Pointer(PointerSideEffect),
    Keyboard(KeyboardSideEffect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PointerSideEffect {
    IdleEraserHover,
    RightClickContextMenuDisabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyboardSideEffect {
    ModifierUpdated,
    ReturnEditSelectedTextMiss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoRouteReason {
    InvalidPointerPosition,
    NoPointerBinding,
    NoActiveInteraction,
    NonLeftReleaseWithoutActiveDrag,
    ReleaseButtonMismatch,
    UnsupportedKey,
    NoKeyBinding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionRoute {
    Core,
    History,
    Selection,
    Tool,
    BoardPages,
    Ui,
    Color,
    CaptureZoom,
    Preset,
}

impl RoutingOutcome {
    pub(crate) fn owns_pointer_motion(self, state: &crate::input::InputState) -> bool {
        match self {
            Self::Started(_) | Self::Continued(_) => state.has_active_pointer_interaction(),
            Self::Consumed(ConsumedBy::RadialMenu | ConsumedBy::RadialMenuToggle) => {
                state.is_radial_menu_open()
            }
            Self::Consumed(ConsumedBy::ColorPickerPopup) => state.color_picker_popup_is_dragging(),
            Self::Consumed(ConsumedBy::BoardPicker) => {
                state.board_picker_is_dragging()
                    || state.board_picker_is_page_dragging()
                    || state.board_appearance_is_size_dragging()
            }
            Self::Consumed(ConsumedBy::PropertiesPanel) => state.is_properties_slider_dragging(),
            _ => false,
        }
    }

    /// A popup button can own a release without owning motion. Its ownership
    /// ends if that popup closes, so a late Up cannot finish a later stroke.
    pub(crate) fn owns_pointer_release(self, state: &crate::input::InputState) -> bool {
        match self {
            Self::Started(_) | Self::Continued(_) => state.has_active_pointer_interaction(),
            Self::Consumed(ConsumedBy::RadialMenu | ConsumedBy::RadialMenuToggle) => {
                state.is_radial_menu_open()
            }
            Self::Consumed(ConsumedBy::ColorPickerPopup) => state.is_color_picker_popup_open(),
            Self::Consumed(ConsumedBy::BoardPicker) => state.is_board_picker_open(),
            Self::Consumed(ConsumedBy::PropertiesPanel) => state.is_properties_panel_open(),
            Self::Consumed(ConsumedBy::FontPicker) => state.is_font_picker_open(),
            Self::Consumed(ConsumedBy::ContextMenu | ConsumedBy::RightClickContextMenu) => {
                state.is_context_menu_open()
            }
            Self::Consumed(ConsumedBy::StatusHud) => state.status_hud.press_pending,
            Self::Consumed(ConsumedBy::ZoomChip) => state.zoom_chip.press_pending.is_pending(),
            _ => false,
        }
    }
}
