//! Applying a font family to text, shared by the two ways of choosing one.
//!
//! `Shift+T` steps through a short configured list; the font picker offers every
//! installed family. They differ only in how the family is chosen — what happens
//! to the text afterwards is one behaviour, and lives here so the two cannot
//! drift into disagreeing about which shapes a font reaches or what a partly
//! applied change reports.

use super::InputState;
use crate::draw::{FontDescriptor, Shape, families_match};
use crate::draw::{TextMeasurer, with_legacy_measurer};

fn text_font_descriptor(shape: &Shape) -> Option<&FontDescriptor> {
    match shape {
        Shape::Text {
            font_descriptor, ..
        }
        | Shape::StickyNote {
            font_descriptor, ..
        } => Some(font_descriptor),
        _ => None,
    }
}

fn text_font_descriptor_mut(shape: &mut Shape) -> Option<&mut FontDescriptor> {
    match shape {
        Shape::Text {
            font_descriptor, ..
        }
        | Shape::StickyNote {
            font_descriptor, ..
        } => Some(font_descriptor),
        _ => None,
    }
}

impl InputState {
    /// Whether the selection holds anything a font applies to.
    pub(crate) fn selection_has_text(&self) -> bool {
        let frame = self.boards.active_frame();
        self.selected_shape_ids().iter().any(|id| {
            frame
                .shape(*id)
                .and_then(|drawn| text_font_descriptor(&drawn.shape))
                .is_some()
        })
    }

    fn first_selected_text_descriptor(&self) -> Option<&FontDescriptor> {
        let frame = self.boards.active_frame();
        self.selected_shape_ids().iter().find_map(|id| {
            frame
                .shape(*id)
                .and_then(|drawn| text_font_descriptor(&drawn.shape))
        })
    }

    fn first_editable_selected_text_descriptor(&self) -> Option<&FontDescriptor> {
        let frame = self.boards.active_frame();
        self.selected_shape_ids().iter().find_map(|id| {
            frame.shape(*id).and_then(|drawn| {
                if drawn.locked {
                    None
                } else {
                    text_font_descriptor(&drawn.shape)
                }
            })
        })
    }

    /// The family of the first selected text shape, if any.
    ///
    /// One shape decides for the whole selection. A mixed selection then
    /// converges on a single family rather than each shape stepping away from
    /// wherever it happened to be.
    pub(in crate::input::state::core) fn first_selected_text_family(&self) -> Option<String> {
        self.first_selected_text_descriptor()
            .map(|descriptor| descriptor.family.clone())
    }

    /// Bold state of the first editable selected text target, if there is one.
    ///
    /// A mixed selection converges when the user clicks rather than borrowing
    /// state from either a locked shape or the unrelated tool default.
    pub(crate) fn first_editable_selected_text_is_bold(&self) -> Option<bool> {
        self.first_editable_selected_text_descriptor()
            .map(FontDescriptor::is_bold)
    }

    /// Turn bold on or off, on selected text when there is any and on the tool
    /// otherwise — the same target rule the font picker uses for a family.
    ///
    /// Writes the words `bold` and `normal`. A configuration asking for a
    /// numeric weight is asking for something a two-state control cannot say,
    /// so turning bold off from here lands on `normal` rather than restoring
    /// whatever number was there.
    pub(crate) fn set_font_bold(&mut self, bold: bool) -> bool {
        with_legacy_measurer(|measurer| self.set_font_bold_with(measurer, bold))
    }

    pub(crate) fn set_font_bold_with(&mut self, measurer: &TextMeasurer, bold: bool) -> bool {
        let weight = if bold { "bold" } else { "normal" };
        if self.selection_has_text() {
            return self.apply_weight_to_selected_text(measurer, weight);
        }
        let descriptor = crate::draw::FontDescriptor::new(
            self.style.font_descriptor.family.clone(),
            weight.to_string(),
            self.style.font_descriptor.style.clone(),
        );
        self.set_font_descriptor(descriptor)
    }

    /// Restyle every selected text shape to `weight`.
    fn apply_weight_to_selected_text(&mut self, measurer: &TextMeasurer, weight: &str) -> bool {
        let target = weight.to_string();
        self.apply_descriptor_to_selected_text(measurer, "weight", move |descriptor| {
            if descriptor.weight.eq_ignore_ascii_case(&target) {
                return false;
            }
            descriptor.weight = target.clone();
            true
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
        with_legacy_measurer(|measurer| self.apply_family_to_selected_text_with(measurer, family))
    }

    pub(in crate::input::state::core) fn apply_family_to_selected_text_with(
        &mut self,
        measurer: &TextMeasurer,
        family: &str,
    ) -> bool {
        let target = family.to_string();
        self.apply_descriptor_to_selected_text(measurer, "font", move |descriptor| {
            if families_match(&descriptor.family, &target) {
                return false;
            }
            descriptor.family = target.clone();
            true
        })
    }

    /// Apply one font-descriptor mutation to every editable selected text
    /// shape, preserving shared lock, undo, damage, and partial-result reporting.
    fn apply_descriptor_to_selected_text(
        &mut self,
        measurer: &TextMeasurer,
        property: &'static str,
        mut apply: impl FnMut(&mut FontDescriptor) -> bool,
    ) -> bool {
        let result = self.apply_selection_change_with(
            measurer,
            |shape| text_font_descriptor(shape).is_some(),
            move |shape| text_font_descriptor_mut(shape).is_some_and(&mut apply),
        );
        self.report_selection_apply_result(result, property)
    }
}
