use crate::draw::{Shape, ShapeId};
use crate::input::InputState;
use crate::util::Rect;

/// Knob size in canvas pixels.
const MAGNIFICATION_HANDLE_SIZE: i32 = 12;
/// Gap between the loupe's bounding box and the track it carries.
const MAGNIFICATION_TRACK_OFFSET: i32 = 18;
/// Track length. The whole 1x-4x range maps onto this, so a full sweep is one
/// short drag rather than a screen-wide one.
const MAGNIFICATION_TRACK_LENGTH: i32 = 120;

/// The on-canvas magnification control for the selected loupe: which shape it
/// edits, where it sits, and the factor it currently shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectedSpotlightControl {
    pub(crate) shape_id: ShapeId,
    pub(crate) track: SpotlightMagnificationTrack,
    pub(crate) magnification: f64,
}

/// Geometry of the on-canvas magnification control for one loupe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpotlightMagnificationTrack {
    /// Full track rect, used for rendering and as the coarse hit target.
    pub(crate) track: Rect,
    /// Current knob position along the track.
    pub(crate) knob: Rect,
}

impl SpotlightMagnificationTrack {
    /// Magnification for a pointer at canvas `x`, clamped to the track ends
    /// and snapped to the same 0.25 grid every other control uses.
    ///
    /// Reads the pointer's absolute position rather than a delta from where
    /// the drag began, so the knob lands under the finger instead of drifting
    /// away from it over a long gesture.
    ///
    /// Snapping goes through the toolbar slider's own spec rather than a
    /// second copy of the rule: three controls edit this property, and a drag
    /// that landed between steps would leave the shape on a value the toolbar
    /// could never show and the wheel would carry its offset forever.
    pub(crate) fn magnification_at(self, x: i32) -> f64 {
        // Widened before subtracting: a track near the i32 extremes would
        // otherwise wrap and snap the loupe to the wrong end of its range.
        let span = f64::from(self.track.width - MAGNIFICATION_HANDLE_SIZE).max(1.0);
        let travelled =
            f64::from(x) - f64::from(self.track.x) - f64::from(MAGNIFICATION_HANDLE_SIZE / 2);
        let t = (travelled / span).clamp(0.0, 1.0);
        let range =
            crate::draw::MAX_SPOTLIGHT_MAGNIFICATION - crate::draw::MIN_SPOTLIGHT_MAGNIFICATION;
        crate::ui::toolbar::model::ToolbarSliderSpec::SPOTLIGHT_MAGNIFICATION
            .normalize_value(crate::draw::MIN_SPOTLIGHT_MAGNIFICATION + t * range)
    }
}

/// Room the readout plate needs above the track, so clamping keeps it on
/// screen too rather than only the track itself.
const MAGNIFICATION_READOUT_HEIGHT: i32 = 26;

/// Track geometry for a loupe whose bounds and factor are known.
///
/// Centred above the ellipse, clear of the `Top` selection handle that sits on
/// the bounding box itself, and kept inside `viewport` so the control stays
/// reachable for a loupe at the edge of the screen: it slides horizontally
/// rather than running off, and flips below the loupe when there is no room
/// above. `None` places it without clamping.
///
/// `viewport` is in **canvas** coordinates, like `bounds`, and so must be the
/// visible canvas rectangle rather than the surface size: pan and zoom are
/// applied after this, and clamping against a zero-origin screen rectangle
/// would put the control off-screen on any panned board.
///
/// Free of `InputState` so the geometry can be derived wherever a shape's
/// bounds are known.
pub(crate) fn spotlight_magnification_track(
    bounds: Rect,
    magnification: f64,
    viewport: Option<Rect>,
) -> Option<SpotlightMagnificationTrack> {
    // Placement is computed in i64 and narrowed once. A loupe persisted near
    // the i32 extremes would otherwise overflow while being centred above its
    // own bounds; failing to place the control is correct there, and `Rect::new`
    // already reports that as `None`.
    let mut track_x = i64::from(bounds.x) + i64::from(bounds.width) / 2
        - i64::from(MAGNIFICATION_TRACK_LENGTH) / 2;
    let mut track_y = i64::from(bounds.y)
        - i64::from(MAGNIFICATION_TRACK_OFFSET)
        - i64::from(MAGNIFICATION_HANDLE_SIZE);

    // A viewport too small to hold the control at all — or not yet known, which
    // is what a zero size means — gets no clamping. Inventing a position there
    // would be worse than placing it where the geometry says.
    let clampable = viewport.filter(|visible| {
        i64::from(visible.width) >= i64::from(MAGNIFICATION_TRACK_LENGTH)
            && i64::from(visible.height)
                >= i64::from(MAGNIFICATION_READOUT_HEIGHT) + i64::from(MAGNIFICATION_HANDLE_SIZE)
    });
    if let Some(visible) = clampable {
        let left = i64::from(visible.x);
        let top = i64::from(visible.y);
        let right = left + i64::from(visible.width);
        let bottom = top + i64::from(visible.height);
        // Slide along the edge rather than letting an endpoint knob leave the
        // visible canvas, where it could never be grabbed.
        track_x = track_x.clamp(left, right - i64::from(MAGNIFICATION_TRACK_LENGTH));
        // A loupe at the top edge has no room above it, so the control flips
        // under the opening instead of sitting off-screen.
        if track_y < top + i64::from(MAGNIFICATION_READOUT_HEIGHT) {
            track_y = i64::from(bounds.y)
                + i64::from(bounds.height)
                + i64::from(MAGNIFICATION_TRACK_OFFSET);
        }
        track_y = track_y.clamp(
            top + i64::from(MAGNIFICATION_READOUT_HEIGHT),
            bottom - i64::from(MAGNIFICATION_HANDLE_SIZE),
        );
    }

    let track = Rect::new(
        i32::try_from(track_x).ok()?,
        i32::try_from(track_y).ok()?,
        MAGNIFICATION_TRACK_LENGTH,
        MAGNIFICATION_HANDLE_SIZE,
    )?;
    let range = crate::draw::MAX_SPOTLIGHT_MAGNIFICATION - crate::draw::MIN_SPOTLIGHT_MAGNIFICATION;
    let t = if range > f64::EPSILON {
        ((crate::draw::normalize_spotlight_magnification(magnification)
            - crate::draw::MIN_SPOTLIGHT_MAGNIFICATION)
            / range)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let span = f64::from(track.width - MAGNIFICATION_HANDLE_SIZE).max(0.0);
    let knob_x = i64::from(track.x) + (t * span).round() as i64;
    let knob = Rect::new(
        i32::try_from(knob_x).ok()?,
        track.y,
        MAGNIFICATION_HANDLE_SIZE,
        MAGNIFICATION_HANDLE_SIZE,
    )?;
    Some(SpotlightMagnificationTrack { track, knob })
}

impl InputState {
    /// The on-canvas magnification control, when exactly one unlocked Spotlight
    /// is selected.
    ///
    /// Single selection only, like the text resize handle: the control edits
    /// one shape's factor, and there is no honest knob position for a mixed
    /// selection.
    ///
    /// Carries the factor as well as the geometry: the renderer needs both, and
    /// walking back through boards, frame, and shape to re-read the value it
    /// was just derived from is a message chain waiting to disagree with the
    /// knob position.
    pub(crate) fn selected_spotlight_control(&self) -> Option<SelectedSpotlightControl> {
        let ids = self.selected_shape_ids();
        if ids.len() != 1 {
            return None;
        }
        let shape_id = ids[0];
        let drawn = self.boards.active_frame().shape(shape_id)?;
        if drawn.locked {
            return None;
        }
        let Shape::Spotlight { magnification, .. } = drawn.shape else {
            return None;
        };
        let bounds = drawn.bounding_box()?;
        // Canvas coordinates, so the clamp survives pan and zoom.
        let track =
            spotlight_magnification_track(bounds, magnification, Some(self.visible_canvas_rect()))?;
        Some(SelectedSpotlightControl {
            shape_id,
            track,
            magnification,
        })
    }

    /// Applies a pointer position on the track to the loupe being dragged.
    ///
    /// The track is recomputed rather than frozen at press: it hangs off the
    /// loupe's bounding box, which magnification does not move, so the mapping
    /// is stable for the whole gesture.
    pub(crate) fn drag_spotlight_magnification_to(&mut self, x: i32) -> bool {
        let crate::input::state::DrawingState::AdjustingSpotlightMagnification { shape_id, .. } =
            self.state
        else {
            return false;
        };
        let Some(control) = self.selected_spotlight_control() else {
            return false;
        };
        if control.shape_id != shape_id {
            return false;
        }
        self.set_spotlight_shape_magnification(shape_id, control.track.magnification_at(x))
    }

    /// Whether the pointer is on the magnification control, and which loupe it
    /// belongs to.
    pub(crate) fn hit_spotlight_magnification_track(
        &self,
        x: i32,
        y: i32,
    ) -> Option<SelectedSpotlightControl> {
        let control = self.selected_spotlight_control()?;
        let tolerance = self.hit_test_tolerance.ceil() as i32;
        let hit = control
            .track
            .track
            .inflated(tolerance)
            .unwrap_or(control.track.track);
        hit.contains(x, y).then_some(control)
    }
}
