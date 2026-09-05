use crate::draw::ShapeId;
use crate::draw::frame::ShapeSnapshot;
use crate::input::InputState;
use crate::input::state::core::editing::CanvasEdit;

impl InputState {
    pub(crate) fn push_translation_undo(
        &mut self,
        measurer: &crate::draw::TextMeasurer,
        before: Vec<(ShapeId, ShapeSnapshot)>,
    ) -> bool {
        let effects = CanvasEdit::from_snapshots(before).commit(
            self.boards.active_frame_mut(),
            self.history_limits.undo_stack_limit(),
        );
        self.apply_edit_effects(measurer, effects)
    }
}
