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
    Tour,
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
    RightClickSuppressedByZoom,
    RightClickContextMenuDisabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyboardSideEffect {
    ModifierUpdated,
    ReturnEditSelectedTextMiss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NoRouteReason {
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
