//! Applying a font family to text, shared by the two ways of choosing one.
//!
//! `Shift+T` steps through a short configured list; the font picker offers every
//! installed family. They differ only in how the family is chosen — what happens
//! to the text afterwards is one behaviour, and lives here so the two cannot
//! drift into disagreeing about which shapes a font reaches or what a partly
//! applied change reports.

use super::InputState;
use crate::draw::{Shape, families_match};

impl InputState {
    /// Whether the selection holds anything a font applies to.
    pub(in crate::input::state::core) fn selection_has_text(&self) -> bool {
        let frame = self.boards.active_frame();
        self.selected_shape_ids().iter().any(|id| {
            matches!(
                frame.shape(*id).map(|drawn| &drawn.shape),
                Some(Shape::Text { .. } | Shape::StickyNote { .. })
            )
        })
    }

    /// The family of the first selected text shape, if any.
    ///
    /// One shape decides for the whole selection. A mixed selection then
    /// converges on a single family rather than each shape stepping away from
    /// wherever it happened to be.
    pub(in crate::input::state::core) fn first_selected_text_family(&self) -> Option<String> {
        let frame = self.boards.active_frame();
        self.selected_shape_ids().iter().find_map(|id| {
            match frame.shape(*id).map(|drawn| &drawn.shape) {
                Some(
                    Shape::Text {
                        font_descriptor, ..
                    }
                    | Shape::StickyNote {
                        font_descriptor, ..
                    },
                ) => Some(font_descriptor.family.clone()),
                _ => None,
            }
        })
    }

    /// Restyle every selected text shape to `family`, and report the result the
    /// way the properties panel does.
    ///
    /// Returns whether anything changed. A shape already in that family is left
    /// alone — matched without case, because fontconfig resolves names that way
    /// and rewriting `Sans` as `sans` is not an edit.
    pub(in crate::input::state::core) fn apply_family_to_selected_text(
        &mut self,
        family: &str,
    ) -> bool {
        let target = family.to_string();
        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Text { .. } | Shape::StickyNote { .. }),
            move |shape| match shape {
                Shape::Text {
                    font_descriptor, ..
                }
                | Shape::StickyNote {
                    font_descriptor, ..
                } if !families_match(&font_descriptor.family, &target) => {
                    font_descriptor.family = target.clone();
                    true
                }
                _ => false,
            },
        );
        self.report_selection_apply_result(result, "font")
    }
}
