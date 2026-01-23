use super::super::KeybindingsConfig;
use super::BindingInserter;
use crate::config::Action;

impl KeybindingsConfig {
    pub(super) fn insert_tool_bindings(
        &self,
        inserter: &mut BindingInserter,
    ) -> Result<(), String> {
        inserter.insert_all(&self.tools.increase_thickness, Action::IncreaseThickness)?;
        inserter.insert_all(&self.tools.decrease_thickness, Action::DecreaseThickness)?;
        inserter.insert_all(
            &self.tools.increase_marker_opacity,
            Action::IncreaseMarkerOpacity,
        )?;
        inserter.insert_all(
            &self.tools.decrease_marker_opacity,
            Action::DecreaseMarkerOpacity,
        )?;
        inserter.insert_all(
            &self.tools.select_selection_tool,
            Action::SelectSelectionTool,
        )?;
        inserter.insert_all(&self.tools.select_marker_tool, Action::SelectMarkerTool)?;
        inserter.insert_all(&self.tools.select_eraser_tool, Action::SelectEraserTool)?;
        inserter.insert_all(&self.tools.toggle_eraser_mode, Action::ToggleEraserMode)?;
        inserter.insert_all(&self.tools.select_pen_tool, Action::SelectPenTool)?;
        inserter.insert_all(&self.tools.select_line_tool, Action::SelectLineTool)?;
        inserter.insert_all(&self.tools.select_rect_tool, Action::SelectRectTool)?;
        inserter.insert_all(&self.tools.select_ellipse_tool, Action::SelectEllipseTool)?;
        inserter.insert_all(&self.tools.select_arrow_tool, Action::SelectArrowTool)?;
        inserter.insert_all(
            &self.tools.select_highlight_tool,
            Action::SelectHighlightTool,
        )?;
        inserter.insert_all(
            &self.tools.toggle_highlight_tool,
            Action::ToggleHighlightTool,
        )?;
        inserter.insert_all(&self.tools.increase_font_size, Action::IncreaseFontSize)?;
        inserter.insert_all(&self.tools.decrease_font_size, Action::DecreaseFontSize)?;
        inserter.insert_all(
            &self.tools.reset_arrow_labels,
            Action::ResetArrowLabelCounter,
        )?;
        Ok(())
    }
}
