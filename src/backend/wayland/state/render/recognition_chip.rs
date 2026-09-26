use super::super::*;

impl WaylandState {
    /// Where the Shape Pen recognition chip sits this frame, beside the shape
    /// it names. Shared by damage collection and painting so they agree.
    pub(super) fn recognition_chip_visual(
        &self,
        width: u32,
        height: u32,
    ) -> Option<crate::ui::RecognitionChipVisual> {
        let chip = self.input_state.recognition_chip()?;
        let anchor = self.input_state.screen_rect_for_canvas(chip.anchor())?;
        crate::ui::recognition_chip_layout(
            self.render.ui_text(),
            chip.label(),
            (
                f64::from(anchor.x),
                f64::from(anchor.y),
                f64::from(anchor.width),
                f64::from(anchor.height),
            ),
            chip.opacity(Instant::now()),
            width,
            height,
        )
    }

    pub(super) fn render_recognition_chip(&self, ctx: &cairo::Context, width: u32, height: u32) {
        if let Some(visual) = self.recognition_chip_visual(width, height) {
            crate::ui::render_recognition_chip(self.render.ui_text(), ctx, &visual);
        }
    }
}
