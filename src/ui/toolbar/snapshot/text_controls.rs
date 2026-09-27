//! Which text controls reach what a tool draws next.

use crate::input::Tool;

use super::ToolbarSnapshot;

/// Text controls the style pill offers for `tool` under context-aware UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DrawnTextControls {
    /// Font family and Bold change what the tool draws.
    pub(super) face: bool,
    /// The text-size slider changes what the tool draws.
    pub(super) size: bool,
}

impl DrawnTextControls {
    /// Text and sticky notes take the Text state before this is consulted, so
    /// only tools whose shapes carry a label appear here. An arrow's
    /// auto-number label uses the text font and scales with the text size, so
    /// it takes both while numbering is on. A step marker's number uses the
    /// family and weight but is sized by the marker's own Size slider, so the
    /// text-size slider would change nothing there.
    pub(super) fn for_tool(tool: Tool, snapshot: &ToolbarSnapshot) -> Self {
        match tool {
            Tool::Arrow => Self {
                face: snapshot.arrow_label_enabled,
                size: snapshot.arrow_label_enabled,
            },
            Tool::StepMarker => Self {
                face: true,
                size: false,
            },
            _ => Self {
                face: false,
                size: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use crate::ui::toolbar::{ToolContext, ToolbarBindingHints};

    fn context_for(tool: Tool, arrow_labels: bool) -> ToolContext {
        let state = make_test_input_state();
        let mut snapshot =
            ToolbarSnapshot::from_input_with_bindings(&state, ToolbarBindingHints::default());
        snapshot.active_tool = tool;
        snapshot.tool_override = None;
        snapshot.context_aware_ui = true;
        // The default config pins text controls on; context-aware UI must
        // still keep them off tools that draw no text.
        snapshot.show_text_controls = true;
        snapshot.arrow_label_enabled = arrow_labels;
        ToolContext::from_snapshot(&snapshot)
    }

    #[test]
    fn tools_that_draw_no_text_get_no_text_controls() {
        for tool in [
            Tool::Pen,
            Tool::LiveShape,
            Tool::Marker,
            Tool::Line,
            Tool::Rect,
            Tool::Ellipse,
            Tool::Blur,
            Tool::Spotlight,
            Tool::Eraser,
        ] {
            let context = context_for(tool, true);
            assert!(!context.show_font_controls, "{tool:?} font controls");
            assert!(!context.show_font_size, "{tool:?} text size");
        }
    }

    #[test]
    fn arrows_get_text_controls_only_while_they_draw_labels() {
        let unlabeled = context_for(Tool::Arrow, false);
        assert!(!unlabeled.show_font_controls && !unlabeled.show_font_size);

        let labeled = context_for(Tool::Arrow, true);
        assert!(labeled.show_font_controls && labeled.show_font_size);
    }

    #[test]
    fn step_markers_get_the_face_but_not_the_text_size() {
        let context = context_for(Tool::StepMarker, false);
        assert!(context.show_font_controls, "the number uses the font");
        assert!(
            !context.show_font_size,
            "the number is sized by the marker's own slider"
        );
    }
}
