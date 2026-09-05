use crate::draw::ShapeId;
use crate::draw::TextMeasurer;
use crate::draw::frame::ShapeSnapshot;
use crate::input::InputState;

impl InputState {
    pub(crate) fn restore_selection_from_snapshots_with(
        &mut self,
        measurer: &TextMeasurer,
        snapshots: Vec<(ShapeId, ShapeSnapshot)>,
    ) {
        let effects = crate::input::state::core::editing::CanvasEdit::from_snapshots(snapshots)
            .rollback(self.boards.active_frame_mut(), measurer);
        self.apply_edit_effects(measurer, effects);
    }
}
