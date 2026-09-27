//! Dragging a properties-panel slider: the shapes follow the thumb live, and
//! the whole drag lands as one undo entry when it ends.

use super::super::base::InputState;
use super::apply_selection::{level_applies, set_level};
use super::types::{PropertiesRowControl, SelectionPropertyKind};
use crate::draw::frame::ShapeSnapshot;
use crate::draw::{ShapeId, TextMeasurer};
use crate::input::state::core::editing::CanvasEdit;

/// A slider drag in progress: the shapes as they were when it began, so every
/// step previews from them and the release commits against them.
#[derive(Debug)]
pub(crate) struct SliderDrag {
    row: usize,
    kind: SelectionPropertyKind,
    snapshots: Vec<(ShapeId, ShapeSnapshot)>,
    value: Option<f64>,
}

impl InputState {
    pub(crate) fn is_properties_slider_dragging(&self) -> bool {
        self.properties.slider_drag.is_some()
    }

    /// Starts dragging the slider on `row`, with the pointer at `x`, which
    /// moves the value there at once. Returns false when the row has no
    /// editable slider.
    pub(crate) fn begin_properties_slider_drag_with(
        &mut self,
        measurer: &TextMeasurer,
        row: usize,
        x: i32,
    ) -> bool {
        let Some(entry) = self
            .properties
            .panel
            .as_ref()
            .and_then(|panel| panel.entries.get(row))
            .filter(|entry| !entry.disabled && entry.kind.level_range().is_some())
        else {
            return false;
        };
        let kind = entry.kind;

        self.finish_active_arrow_bend();
        let frame = self.boards.active_frame();
        let editable: Vec<ShapeId> = self
            .selected_shape_ids()
            .iter()
            .copied()
            .filter(|id| {
                frame
                    .shape(*id)
                    .is_some_and(|drawn| !drawn.locked && level_applies(kind, &drawn.shape))
            })
            .collect();
        let snapshots = CanvasEdit::capture(frame, &editable).into_snapshots();
        self.properties.slider_drag = Some(SliderDrag {
            row,
            kind,
            snapshots,
            value: None,
        });

        self.drag_properties_slider_with(measurer, x);
        true
    }

    /// Moves the dragged slider's value to the pointer at `x`, previewing it
    /// on the shapes without recording history.
    pub(crate) fn drag_properties_slider_with(&mut self, measurer: &TextMeasurer, x: i32) {
        let Some(mut drag) = self.properties.slider_drag.take() else {
            return;
        };
        let track = self.properties_slider_track(drag.row);
        let value = track
            .zip(drag.kind.level_range())
            .map(|(track, range)| range.value_at_x(track, f64::from(x)));

        if let Some(value) = value
            && drag.value != Some(value)
        {
            drag.value = Some(value);
            let kind = drag.kind;
            let effects = CanvasEdit::borrow_snapshots(&drag.snapshots).preview(
                self.boards.active_frame_mut(),
                measurer,
                |shape, _| set_level(kind, shape, value),
            );
            self.apply_edit_effects(measurer, effects);
            self.properties.slider_drag = Some(drag);
            self.refresh_properties_panel_with(measurer);
        } else {
            self.properties.slider_drag = Some(drag);
        }
    }

    /// Ends a slider drag, recording everything it changed as one undo entry.
    pub(crate) fn finish_properties_slider_drag_with(&mut self, measurer: &TextMeasurer) {
        let Some(drag) = self.properties.slider_drag.take() else {
            return;
        };

        let effects = CanvasEdit::from_snapshots(drag.snapshots).commit(
            self.boards.active_frame_mut(),
            self.history_limits.undo_stack_limit(),
        );
        self.apply_edit_effects(measurer, effects);

        if self.is_properties_panel_open() {
            self.refresh_properties_panel_with(measurer);
        }
    }

    fn properties_slider_track(&self, row: usize) -> Option<crate::input::state::PanelRect> {
        let panel = self.properties.panel.as_ref()?;
        let layout = self.properties.layout.as_ref()?;
        match layout.rows(panel).into_iter().nth(row)?.control {
            PropertiesRowControl::Slider { track, .. } => Some(track),
            _ => None,
        }
    }
}
