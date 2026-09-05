use smithay_client_toolkit::seat::pointer::CursorIcon;
use wayland_client::Connection;

use super::*;
use crate::backend::wayland::toolbar::ToolbarCursorHint;
use crate::input::{
    BoardPickerCursorHint, ColorPickerCursorHint, CommandPaletteCursorHint, ContextMenuCursorHint,
    DrawingState, HelpOverlayCursorHint, IdleHandle, SelectionHandle,
};

/// What the pointer is over on the screen-modal surfaces (the eyedropper and
/// the region picker), resolved by [`WaylandState::screen_modal_cursor_context`]
/// so that [`screen_modal_cursor`] stays a pure decision table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ScreenModalCursorContext {
    window_snap_active: bool,
    review: bool,
    /// A device owns a Review move-drag right now.
    review_dragging: bool,
    over_action: bool,
    /// Inside the Review bar, whether or not over one of its controls.
    over_bar: bool,
    over_selection: bool,
    /// The grip being dragged, or hovered when nothing is being dragged.
    resize_handle: Option<SelectionHandle>,
    /// Cut mode is armed. The pointer aims a band, not a crop move.
    cut_armed: bool,
    /// Cuts exist, so the source crop cannot move or resize.
    crop_locked: bool,
}

/// Cursor for the screen-modal surfaces. Targeting keeps the crosshair; a
/// finished selection in Review swaps to ordinary chrome cursors, because
/// nothing is being aimed any more — the pointer now drags the rectangle or
/// presses a button.
fn screen_modal_cursor(context: ScreenModalCursorContext) -> CursorIcon {
    if context.window_snap_active {
        return CursorIcon::Pointer;
    }
    if !context.review {
        return CursorIcon::Crosshair;
    }
    // A held grip outranks everything: the pointer can leave both the grip and
    // the rectangle mid-drag and the cursor must keep describing the drag.
    if let Some(handle) = context.resize_handle.filter(|_| context.review_dragging) {
        return resize_cursor(handle);
    }
    if context.cut_armed && context.review_dragging {
        return CursorIcon::Crosshair;
    }
    if context.review_dragging {
        return CursorIcon::Grabbing;
    }
    if context.over_action {
        return CursorIcon::Pointer;
    }
    if context.over_bar {
        return CursorIcon::Default;
    }
    if let Some(handle) = context.resize_handle {
        return resize_cursor(handle);
    }
    if context.cut_armed && context.over_selection {
        return CursorIcon::Crosshair;
    }
    if context.crop_locked {
        return CursorIcon::Default;
    }
    if context.over_selection {
        return CursorIcon::Grab;
    }
    CursorIcon::Default
}

/// The one place a resize grip becomes a cursor. Shared by the Review grips
/// and the canvas selection handles so the two can never disagree.
const fn resize_cursor(handle: SelectionHandle) -> CursorIcon {
    match handle {
        SelectionHandle::TopLeft | SelectionHandle::BottomRight => CursorIcon::NwseResize,
        SelectionHandle::TopRight | SelectionHandle::BottomLeft => CursorIcon::NeswResize,
        SelectionHandle::Top | SelectionHandle::Bottom => CursorIcon::NsResize,
        SelectionHandle::Left | SelectionHandle::Right => CursorIcon::EwResize,
    }
}

trait CursorHint {
    fn icon(self) -> CursorIcon;
}

impl CursorHint for ColorPickerCursorHint {
    fn icon(self) -> CursorIcon {
        match self {
            Self::Text => CursorIcon::Text,
            Self::Crosshair => CursorIcon::Crosshair,
            Self::Pointer => CursorIcon::Pointer,
            Self::Default => CursorIcon::Default,
        }
    }
}

impl CursorHint for BoardPickerCursorHint {
    fn icon(self) -> CursorIcon {
        match self {
            Self::Text => CursorIcon::Text,
            Self::Pointer => CursorIcon::Pointer,
            Self::Grab => CursorIcon::Grab,
            Self::Grabbing => CursorIcon::Grabbing,
            Self::Default => CursorIcon::Default,
        }
    }
}

impl CursorHint for ContextMenuCursorHint {
    fn icon(self) -> CursorIcon {
        match self {
            Self::Pointer => CursorIcon::Pointer,
            Self::Default => CursorIcon::Default,
        }
    }
}

impl CursorHint for CommandPaletteCursorHint {
    fn icon(self) -> CursorIcon {
        match self {
            Self::Text => CursorIcon::Text,
            Self::Pointer => CursorIcon::Pointer,
            Self::Default => CursorIcon::Default,
        }
    }
}

impl CursorHint for HelpOverlayCursorHint {
    fn icon(self) -> CursorIcon {
        match self {
            Self::Text => CursorIcon::Text,
            Self::Pointer => CursorIcon::Pointer,
            Self::Default => CursorIcon::Default,
        }
    }
}

impl CursorHint for ToolbarCursorHint {
    fn icon(self) -> CursorIcon {
        match self {
            Self::Pointer => CursorIcon::Pointer,
            Self::Grab => CursorIcon::Grab,
            Self::Default => CursorIcon::Default,
        }
    }
}

fn drawing_state_cursor(state: &DrawingState) -> Option<CursorIcon> {
    match state {
        DrawingState::TextInput { .. } => Some(CursorIcon::Text),
        DrawingState::MovingSelection { .. } | DrawingState::BendingArrow { .. } => {
            Some(CursorIcon::Grabbing)
        }
        DrawingState::ResizingText { .. } => Some(CursorIcon::SeResize),
        DrawingState::Drawing { .. }
        | DrawingState::BuildingPolygon { .. }
        | DrawingState::Selecting { .. } => Some(CursorIcon::Crosshair),
        DrawingState::PendingTextClick { .. } => Some(CursorIcon::Default),
        DrawingState::ResizingSelection { handle, .. } => Some(resize_cursor(*handle)),
        DrawingState::AdjustingSpotlightMagnification { .. } => Some(CursorIcon::EwResize),
        DrawingState::Idle => None,
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn update_pointer_cursor(
        &mut self,
        toolbar_hover: bool,
        conn: &Connection,
    ) {
        if self.toolbar_drag.item_dragging() && self.pointer_lock_active() {
            self.hide_pointer_cursor();
            return;
        }

        let icon = self.compute_cursor_icon(toolbar_hover);
        self.pointer.apply_cursor_icon(conn, icon);
    }

    /// Refresh the cursor around a pointer press or release that touched the
    /// screen-modal surfaces. Both dispatches can change the pointer's role
    /// with no motion following: a press starts a Review move-drag, submits an
    /// action that closes the picker, or right-click-cancels it; a release
    /// ends a drag or moves targeting into Review. Scoped to the modal
    /// surfaces so ordinary drawing, toolbar and pointer-lock paths keep their
    /// existing behaviour.
    pub(super) fn refresh_screen_modal_cursor(
        &mut self,
        modal_before: bool,
        routed: RoutedInput,
        conn: &Connection,
    ) {
        if modal_before || self.input_state.screen_modal_is_active() {
            self.update_pointer_cursor(
                routed.surface == InputSurface::Toolbar
                    || self.toolbar_chrome.pointer_over_toolbar(),
                conn,
            );
        }
    }

    /// Gather what the screen-modal cursor decision needs. The hit tests each
    /// re-place the Review bar, so they run only once Review is actually
    /// engaged.
    fn screen_modal_cursor_context(&self) -> ScreenModalCursorContext {
        let window_snap_active = self.region_window_snap_active();
        let region_state = self.input_state.region_state();
        if window_snap_active || !region_state.is_review() {
            return ScreenModalCursorContext {
                window_snap_active,
                ..ScreenModalCursorContext::default()
            };
        }
        let (mouse_x, mouse_y) = self.pointer.position();
        let point = (f64::from(mouse_x), f64::from(mouse_y));
        let dragging = region_state.selection_owner().is_some();
        // While a grip is held its identity comes from the drag, not from
        // whatever the pointer currently happens to be over.
        let resize_handle = if dragging {
            self.region_review_resize_handle()
        } else {
            self.region_review_handle_at(point)
        };
        ScreenModalCursorContext {
            window_snap_active,
            review: true,
            review_dragging: dragging,
            over_action: self.region_review_action_at(point).is_some(),
            over_bar: self.region_review_bar_contains(point),
            over_selection: self.region_review_selection_contains(point),
            resize_handle,
            cut_armed: self.region_cut_mode_armed(),
            crop_locked: self.region_review_crop_locked(),
        }
    }

    /// Computes the appropriate cursor icon based on current context.
    fn compute_cursor_icon(&mut self, toolbar_hover: bool) -> CursorIcon {
        if self.input_state.screen_modal_is_active() && !toolbar_hover {
            return screen_modal_cursor(self.screen_modal_cursor_context());
        }
        if let Some(icon) = self.popup_cursor() {
            return icon;
        }
        if let Some(icon) = self.toolbar_cursor(toolbar_hover) {
            return icon;
        }
        if let Some(icon) = drawing_state_cursor(&self.input_state.state) {
            return icon;
        }
        self.idle_canvas_cursor()
    }

    fn popup_cursor(&self) -> Option<CursorIcon> {
        let (mx, my) = self.pointer.position();
        if self.input_state.is_color_picker_popup_open()
            && let Some(layout) = self.input_state.color_picker_popup_layout()
        {
            if self.input_state.color_picker_popup_is_dragging() {
                return Some(CursorIcon::Crosshair);
            }
            return Some(
                layout
                    .cursor_hint_at(mx as f64, my as f64, self.input_state.recent_colors().len())
                    .icon(),
            );
        }
        if self.input_state.is_board_picker_open()
            && let Some(hint) = self.input_state.board_picker_cursor_hint_at(mx, my)
        {
            return Some(hint.icon());
        }
        if self.input_state.is_context_menu_open()
            && let Some(hint) = self.input_state.context_menu_cursor_hint_at(mx, my)
        {
            return Some(hint.icon());
        }
        if self.input_state.command_palette.open
            && let Some(hint) = self.input_state.command_palette_cursor_hint_at(
                mx,
                my,
                self.surface.width(),
                self.surface.height(),
            )
        {
            return Some(hint.icon());
        }
        if self.input_state.help_overlay.is_visible()
            && let Some(hint) = self.input_state.help_overlay_cursor_hint_at(mx, my)
        {
            return Some(hint.icon());
        }
        None
    }

    fn toolbar_cursor(&self, toolbar_hover: bool) -> Option<CursorIcon> {
        if self.toolbar_drag.item_dragging() || self.pointer.board_pan_active() {
            return Some(CursorIcon::Grabbing);
        }
        if self.pointer.board_pan_key_held() && self.can_start_board_pan() {
            return Some(CursorIcon::Grab);
        }
        if self.toolbar_chrome.inline_toolbars()
            && self.toolbar_chrome.pointer_over_toolbar()
            && let Some(hint) = self.inline_toolbar_cursor_hint()
        {
            return Some(hint.icon());
        }
        if toolbar_hover {
            return Some(
                self.toolbar
                    .cursor_hint()
                    .map_or(CursorIcon::Default, CursorHint::icon),
            );
        }
        None
    }

    fn idle_canvas_cursor(&mut self) -> CursorIcon {
        if self.input_state.status_hud.hover().is_some()
            || self.input_state.zoom_chip.hover().is_some()
        {
            return CursorIcon::Pointer;
        }
        let (pointer_x, pointer_y) = self.input_state.pointer_position();
        if self.input_state.status_hud_contains(pointer_x, pointer_y)
            || self.input_state.zoom_chip_contains(pointer_x, pointer_y)
        {
            return CursorIcon::Default;
        }

        let (canvas_x, canvas_y) = self.input_state.canvas_pointer_position();
        match self
            .input_state
            .hit_idle_handle_with(self.render.text_measurer(), canvas_x, canvas_y)
        {
            Some(IdleHandle::SpotlightMagnification(_)) => return CursorIcon::EwResize,
            Some(IdleHandle::ArrowBend(_)) => return CursorIcon::Grab,
            Some(IdleHandle::TextResize(_)) => return CursorIcon::SeResize,
            Some(IdleHandle::SelectionResize(handle)) => return resize_cursor(handle),
            None => {}
        }
        if let Some(hit_id) =
            self.input_state
                .hit_test_at_with(self.render.text_measurer(), canvas_x, canvas_y)
            && self
                .input_state
                .selected_shape_ids_set()
                .is_some_and(|set| set.contains(&hit_id))
        {
            return CursorIcon::Grab;
        }
        CursorIcon::Crosshair
    }

    pub(in crate::backend::wayland) fn hide_pointer_cursor(&mut self) {
        self.pointer.hide_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::{CursorHint, ScreenModalCursorContext, drawing_state_cursor, screen_modal_cursor};
    use crate::{
        backend::wayland::toolbar::ToolbarCursorHint,
        draw::{Shape, color::BLACK, frame::ShapeSnapshot},
        input::{
            BoardPickerCursorHint, ColorPickerCursorHint, CommandPaletteCursorHint,
            ContextMenuCursorHint, DrawingState, HelpOverlayCursorHint, SelectionHandle, Tool,
        },
        util::Rect,
    };
    use smithay_client_toolkit::seat::pointer::CursorIcon;
    use std::sync::Arc;

    fn review(context: ScreenModalCursorContext) -> ScreenModalCursorContext {
        ScreenModalCursorContext {
            review: true,
            ..context
        }
    }

    fn snapshot() -> ShapeSnapshot {
        ShapeSnapshot {
            shape: Shape::Line {
                x1: 0,
                y1: 0,
                x2: 1,
                y2: 1,
                color: BLACK,
                thick: 1.0,
            },
            locked: false,
        }
    }

    #[test]
    fn every_popup_and_toolbar_hint_has_one_cursor_mapping() {
        for (hint, expected) in [
            (ColorPickerCursorHint::Default, CursorIcon::Default),
            (ColorPickerCursorHint::Text, CursorIcon::Text),
            (ColorPickerCursorHint::Crosshair, CursorIcon::Crosshair),
            (ColorPickerCursorHint::Pointer, CursorIcon::Pointer),
        ] {
            assert_eq!(hint.icon(), expected);
        }
        for (hint, expected) in [
            (BoardPickerCursorHint::Default, CursorIcon::Default),
            (BoardPickerCursorHint::Text, CursorIcon::Text),
            (BoardPickerCursorHint::Pointer, CursorIcon::Pointer),
            (BoardPickerCursorHint::Grab, CursorIcon::Grab),
            (BoardPickerCursorHint::Grabbing, CursorIcon::Grabbing),
        ] {
            assert_eq!(hint.icon(), expected);
        }
        for (hint, expected) in [
            (ContextMenuCursorHint::Default, CursorIcon::Default),
            (ContextMenuCursorHint::Pointer, CursorIcon::Pointer),
        ] {
            assert_eq!(hint.icon(), expected);
        }
        for (hint, expected) in [
            (CommandPaletteCursorHint::Default, CursorIcon::Default),
            (CommandPaletteCursorHint::Text, CursorIcon::Text),
            (CommandPaletteCursorHint::Pointer, CursorIcon::Pointer),
        ] {
            assert_eq!(hint.icon(), expected);
        }
        for (hint, expected) in [
            (HelpOverlayCursorHint::Default, CursorIcon::Default),
            (HelpOverlayCursorHint::Text, CursorIcon::Text),
            (HelpOverlayCursorHint::Pointer, CursorIcon::Pointer),
        ] {
            assert_eq!(hint.icon(), expected);
        }
        for (hint, expected) in [
            (ToolbarCursorHint::Default, CursorIcon::Default),
            (ToolbarCursorHint::Pointer, CursorIcon::Pointer),
            (ToolbarCursorHint::Grab, CursorIcon::Grab),
        ] {
            assert_eq!(hint.icon(), expected);
        }
    }

    #[test]
    fn every_drawing_state_arm_selects_its_cursor() {
        let cases = [
            (
                DrawingState::TextInput {
                    x: 0,
                    y: 0,
                    buffer: String::new(),
                    caret: 0,
                    selection_anchor: None,
                },
                Some(CursorIcon::Text),
            ),
            (
                DrawingState::MovingSelection {
                    last_x: 0,
                    last_y: 0,
                    snapshots: Vec::new(),
                    moved: false,
                },
                Some(CursorIcon::Grabbing),
            ),
            (
                DrawingState::ResizingText {
                    shape_id: 1,
                    snapshot: snapshot(),
                    base_x: 0,
                    size: 12.0,
                },
                Some(CursorIcon::SeResize),
            ),
            (
                DrawingState::Drawing {
                    tool: Tool::Pen,
                    start_x: 0,
                    start_y: 0,
                    points: Vec::new(),
                    point_thicknesses: Vec::new(),
                },
                Some(CursorIcon::Crosshair),
            ),
            (
                DrawingState::BuildingPolygon {
                    points: Vec::new(),
                    preview: None,
                    fill: false,
                    color: BLACK,
                    thick: 1.0,
                },
                Some(CursorIcon::Crosshair),
            ),
            (
                DrawingState::Selecting {
                    start_x: 0,
                    start_y: 0,
                    additive: false,
                },
                Some(CursorIcon::Crosshair),
            ),
            (
                DrawingState::PendingTextClick {
                    x: 0,
                    y: 0,
                    tool: Tool::Select,
                    shape_id: 1,
                },
                Some(CursorIcon::Default),
            ),
            (
                DrawingState::ResizingSelection {
                    handle: SelectionHandle::Right,
                    original_bounds: Rect::new(0, 0, 1, 1).unwrap(),
                    start_x: 0,
                    start_y: 0,
                    snapshots: Arc::new(Vec::new()),
                },
                Some(CursorIcon::EwResize),
            ),
            (
                DrawingState::AdjustingSpotlightMagnification {
                    shape_id: 1,
                    snapshot: snapshot(),
                },
                Some(CursorIcon::EwResize),
            ),
            (
                DrawingState::BendingArrow {
                    shape_id: 1,
                    snapshot: snapshot(),
                },
                Some(CursorIcon::Grabbing),
            ),
            (DrawingState::Idle, None),
        ];
        for (state, expected) in cases {
            assert_eq!(drawing_state_cursor(&state), expected, "{state:?}");
        }
    }

    #[test]
    fn targeting_keeps_the_crosshair_and_window_mode_takes_the_hand() {
        assert_eq!(
            screen_modal_cursor(ScreenModalCursorContext::default()),
            CursorIcon::Crosshair,
            "armed, selecting, measuring and the eyedropper all aim"
        );
        assert_eq!(
            screen_modal_cursor(ScreenModalCursorContext {
                window_snap_active: true,
                ..ScreenModalCursorContext::default()
            }),
            CursorIcon::Pointer,
            "window mode picks a target rather than aiming"
        );
    }

    #[test]
    fn window_mode_outranks_review_state() {
        // Window mode replaces the selection wholesale, so any Review hover
        // flags left over from a previous rectangle must not leak through.
        assert_eq!(
            screen_modal_cursor(review(ScreenModalCursorContext {
                window_snap_active: true,
                over_selection: true,
                ..ScreenModalCursorContext::default()
            })),
            CursorIcon::Pointer
        );
    }

    #[test]
    fn review_resolves_dragging_then_controls_then_the_rectangle() {
        let cases = [
            (
                ScreenModalCursorContext {
                    review_dragging: true,
                    over_selection: true,
                    ..ScreenModalCursorContext::default()
                },
                CursorIcon::Grabbing,
                "a live move-drag outranks every hover",
            ),
            (
                ScreenModalCursorContext {
                    over_action: true,
                    over_bar: true,
                    ..ScreenModalCursorContext::default()
                },
                CursorIcon::Pointer,
                "an action button",
            ),
            (
                ScreenModalCursorContext {
                    over_bar: true,
                    ..ScreenModalCursorContext::default()
                },
                CursorIcon::Default,
                "a gap inside the bar stays modal-owned but inert",
            ),
            (
                ScreenModalCursorContext {
                    over_selection: true,
                    ..ScreenModalCursorContext::default()
                },
                CursorIcon::Grab,
                "the rectangle can still be dragged",
            ),
            (
                ScreenModalCursorContext::default(),
                CursorIcon::Default,
                "the scrim outside both",
            ),
        ];
        for (context, expected, what) in cases {
            assert_eq!(screen_modal_cursor(review(context)), expected, "{what}");
        }
    }

    #[test]
    fn each_grip_takes_the_resize_cursor_that_matches_its_axis() {
        for (handle, expected) in [
            (SelectionHandle::TopLeft, CursorIcon::NwseResize),
            (SelectionHandle::BottomRight, CursorIcon::NwseResize),
            (SelectionHandle::TopRight, CursorIcon::NeswResize),
            (SelectionHandle::BottomLeft, CursorIcon::NeswResize),
            (SelectionHandle::Top, CursorIcon::NsResize),
            (SelectionHandle::Bottom, CursorIcon::NsResize),
            (SelectionHandle::Left, CursorIcon::EwResize),
            (SelectionHandle::Right, CursorIcon::EwResize),
        ] {
            assert_eq!(
                screen_modal_cursor(review(ScreenModalCursorContext {
                    resize_handle: Some(handle),
                    // A grip sits on the rectangle's own edge, so hovering one
                    // always also hovers the rectangle.
                    over_selection: true,
                    ..ScreenModalCursorContext::default()
                })),
                expected,
                "{handle:?}"
            );
        }
    }

    #[test]
    fn a_held_grip_keeps_its_resize_cursor_instead_of_the_move_hand() {
        // Mid-drag the pointer can be anywhere, including off the rectangle
        // entirely, and the cursor must keep describing the resize.
        assert_eq!(
            screen_modal_cursor(review(ScreenModalCursorContext {
                review_dragging: true,
                resize_handle: Some(SelectionHandle::Right),
                ..ScreenModalCursorContext::default()
            })),
            CursorIcon::EwResize
        );
        assert_eq!(
            screen_modal_cursor(review(ScreenModalCursorContext {
                review_dragging: true,
                resize_handle: None,
                over_selection: true,
                ..ScreenModalCursorContext::default()
            })),
            CursorIcon::Grabbing,
            "a move-drag still reads as grabbing"
        );
    }

    #[test]
    fn cut_mode_uses_a_crosshair_and_locked_crops_drop_the_move_hand() {
        assert_eq!(
            screen_modal_cursor(review(ScreenModalCursorContext {
                cut_armed: true,
                over_selection: true,
                ..ScreenModalCursorContext::default()
            })),
            CursorIcon::Crosshair
        );
        assert_eq!(
            screen_modal_cursor(review(ScreenModalCursorContext {
                cut_armed: true,
                review_dragging: true,
                over_selection: true,
                ..ScreenModalCursorContext::default()
            })),
            CursorIcon::Crosshair
        );
        assert_eq!(
            screen_modal_cursor(review(ScreenModalCursorContext {
                crop_locked: true,
                over_selection: true,
                ..ScreenModalCursorContext::default()
            })),
            CursorIcon::Default
        );
        assert_eq!(
            screen_modal_cursor(review(ScreenModalCursorContext {
                cut_armed: true,
                over_action: true,
                over_bar: true,
                over_selection: true,
                ..ScreenModalCursorContext::default()
            })),
            CursorIcon::Pointer
        );
    }

    #[test]
    fn the_action_bar_outranks_a_grip_it_is_painted_over() {
        // A bar clamped over the selection hides the grips underneath it, so
        // the cursor must describe what is actually visible.
        for context in [
            ScreenModalCursorContext {
                over_action: true,
                over_bar: true,
                resize_handle: Some(SelectionHandle::Bottom),
                ..ScreenModalCursorContext::default()
            },
            ScreenModalCursorContext {
                over_bar: true,
                resize_handle: Some(SelectionHandle::Bottom),
                ..ScreenModalCursorContext::default()
            },
        ] {
            let expected = if context.over_action {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            };
            assert_eq!(screen_modal_cursor(review(context)), expected);
        }
    }

    #[test]
    fn a_button_that_overlaps_the_rectangle_still_reads_as_a_button() {
        // The bar can be clamped over a full-screen selection, so the two
        // hover flags are not mutually exclusive.
        assert_eq!(
            screen_modal_cursor(review(ScreenModalCursorContext {
                over_action: true,
                over_bar: true,
                over_selection: true,
                ..ScreenModalCursorContext::default()
            })),
            CursorIcon::Pointer
        );
    }
}
