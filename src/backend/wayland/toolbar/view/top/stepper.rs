//! Style-pill steppers: an optional caption, the − half, the live readout,
//! and the + half, laid out abutting.

use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::toolbar::{ToolbarSnapshot, model};

use super::super::node::{ButtonStyle, Interaction, LabelSpec, WidgetKind, WidgetNode};
use super::TOP_LABEL_FONT_SIZE;

/// Caption text size. The GTK `.caption` label reads the same token
/// (`font_tooltip`), so both toolbars draw the word at one size.
const CAPTION_FONT_SIZE: f64 = crate::ui::theme::toolbar::FONT_SIZE_TOOLTIP;

/// Width one stepper occupies, caption included, without the trailing gap.
///
/// The width planner walks the built tree, so this is what it budgets; the
/// GTK row asks for the same three or four slots.
pub(super) fn style_stepper_width(control: model::StylePillControl) -> f64 {
    let caption = if control.caption().is_some() {
        ToolbarLayoutSpec::TOP_STYLE_CAPTION_W
    } else {
        0.0
    };
    caption + ToolbarLayoutSpec::TOP_STYLE_STEP_W * 2.0 + ToolbarLayoutSpec::TOP_STYLE_SEL_VALUE_W
}

/// Push one stepper's nodes starting at `x` on the row whose top is `y`,
/// returning the advance it consumed (see [`style_stepper_width`]).
pub(super) fn push_style_stepper(
    nodes: &mut Vec<WidgetNode>,
    control: model::StylePillControl,
    snapshot: &ToolbarSnapshot,
    x: f64,
    y: f64,
) -> f64 {
    let row_h = ToolbarLayoutSpec::TOP_STYLE_ROW_H;
    let step_w = ToolbarLayoutSpec::TOP_STYLE_STEP_W;
    let value_w = ToolbarLayoutSpec::TOP_STYLE_SEL_VALUE_W;
    let id = control.id();
    let enabled = control.enabled(snapshot);
    let steps = control.required_steps(snapshot);
    let step_style = if enabled {
        ButtonStyle::plain()
    } else {
        ButtonStyle::disabled()
    };

    let mut left = x;
    if let Some(caption) = control.caption() {
        let caption_w = ToolbarLayoutSpec::TOP_STYLE_CAPTION_W;
        nodes.push(WidgetNode::decor(
            format!("{id}.caption"),
            (left, y, caption_w, row_h),
            WidgetKind::Label(LabelSpec::new(caption, CAPTION_FONT_SIZE, false).caption()),
        ));
        left += caption_w;
    }

    nodes.push(WidgetNode::new(
        steps[0].id,
        (left, y, step_w, row_h),
        WidgetKind::TextButton {
            label: LabelSpec::new(steps[0].label, TOP_LABEL_FONT_SIZE, true),
            style: step_style,
        },
        enabled.then(|| Interaction::click(steps[0].event.clone(), Some(steps[0].tooltip.clone()))),
    ));
    // The readout is the value the user is changing, so it takes the primary
    // foreground, bold and centered between the halves, like the numeral
    // buttons beside it.
    nodes.push(WidgetNode::decor(
        format!("{id}.value"),
        (left + step_w, y, value_w, row_h),
        WidgetKind::Label(
            LabelSpec::new(
                control.required_value_text(snapshot),
                TOP_LABEL_FONT_SIZE,
                true,
            )
            .centered(),
        ),
    ));
    nodes.push(WidgetNode::new(
        steps[1].id,
        (left + step_w + value_w, y, step_w, row_h),
        WidgetKind::TextButton {
            label: LabelSpec::new(steps[1].label, TOP_LABEL_FONT_SIZE, true),
            style: step_style,
        },
        enabled.then(|| Interaction::click(steps[1].event.clone(), Some(steps[1].tooltip.clone()))),
    ));

    style_stepper_width(control)
}
