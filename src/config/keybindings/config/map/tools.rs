use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_tool_bindings(
        &self,
        inserter: &mut BindingInserter,
    ) -> Result<(), String> {
        inserter.insert_all(&self.increase_thickness, Action::IncreaseThickness)?;
        inserter.insert_all(&self.decrease_thickness, Action::DecreaseThickness)?;
        inserter.insert_all(&self.increase_marker_opacity, Action::IncreaseMarkerOpacity)?;
        inserter.insert_all(&self.decrease_marker_opacity, Action::DecreaseMarkerOpacity)?;
        inserter.insert_all(&self.select_marker_tool, Action::SelectMarkerTool)?;
        inserter.insert_all(&self.select_eraser_tool, Action::SelectEraserTool)?;
        inserter.insert_all(&self.toggle_eraser_mode, Action::ToggleEraserMode)?;
        inserter.insert_all(&self.select_pen_tool, Action::SelectPenTool)?;
        inserter.insert_all(&self.select_line_tool, Action::SelectLineTool)?;
        inserter.insert_all(&self.select_rect_tool, Action::SelectRectTool)?;
        inserter.insert_all(&self.select_ellipse_tool, Action::SelectEllipseTool)?;
        inserter.insert_all(&self.select_arrow_tool, Action::SelectArrowTool)?;
        inserter.insert_all(&self.select_highlight_tool, Action::SelectHighlightTool)?;
        inserter.insert_all(&self.toggle_highlight_tool, Action::ToggleHighlightTool)?;
        inserter.insert_all(&self.increase_font_size, Action::IncreaseFontSize)?;
        inserter.insert_all(&self.decrease_font_size, Action::DecreaseFontSize)?;
        Ok(())
    }
}
