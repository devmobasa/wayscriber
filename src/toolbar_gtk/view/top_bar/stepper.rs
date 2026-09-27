//! GTK style-pill steppers: an optional caption, the − half, the live
//! readout, and the + half. Mirrors the builtin `view/top/stepper.rs` slot
//! for slot; serves the docked selection properties and the pen smoothing
//! and Shape Pen detection steppers of `stroke_controls = "stepper"`.

use super::style_pill::pill_button;
use super::*;

impl TopBar {
    /// Appends one stepper to `pill`, followed by `gap_px` of margin, and
    /// registers its updater.
    pub(super) fn append_style_stepper(
        &mut self,
        pill: &gtk4::Box,
        control: model::StylePillControl,
        snapshot: &ToolbarSnapshot,
        scale: f64,
        gap_px: i32,
    ) {
        let sz = |value: f64| value * scale;
        let px = |value: f64| (value * scale).round() as i32;

        // No spacing between the parts: the builtin lays caption, −, value,
        // and + out abutting, and the width planner budgets exactly that.
        // Child gaps here would make this widget wider than the arrangement
        // the planner declared fits.
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        set_semantic_widget_id(&row, control.id().as_ref());
        // A row of "−  3  +" says nothing about what it steps: the caption
        // names it on screen, the label for assistive tech.
        let accessible_label = control.label(snapshot);
        row.update_property(&[gtk4::accessible::Property::Label(&accessible_label)]);
        row.set_valign(gtk4::Align::Center);
        if let Some(caption) = control.caption() {
            let caption_label = gtk4::Label::new(Some(caption));
            caption_label.add_css_class("stepper-caption");
            set_semantic_widget_id(&caption_label, &format!("{}.caption", control.id()));
            caption_label.set_xalign(0.0);
            caption_label.set_width_request(px(STYLE_CAPTION_W));
            row.append(&caption_label);
        }

        let steps = control.required_steps(snapshot);
        let mut handles: Vec<gtk4::Button> = Vec::new();
        let minus = pill_button(steps[0].label, sz(STYLE_STEP_W), sz(STYLE_ROW_H));
        set_semantic_widget_id(&minus, steps[0].id);
        minus.set_tooltip_text(Some(&steps[0].tooltip));
        minus.update_property(&[gtk4::accessible::Property::Label(&steps[0].tooltip)]);
        row.append(&minus);
        handles.push(minus.clone());
        let value = gtk4::Label::new(Some(&control.required_value_text(snapshot)));
        value.add_css_class("stepper-value");
        set_semantic_widget_id(&value, &format!("{}.value", control.id()));
        value.set_width_request(px(STYLE_SEL_VALUE_W));
        row.append(&value);
        let plus = pill_button(steps[1].label, sz(STYLE_STEP_W), sz(STYLE_ROW_H));
        set_semantic_widget_id(&plus, steps[1].id);
        plus.set_tooltip_text(Some(&steps[1].tooltip));
        plus.update_property(&[gtk4::accessible::Property::Label(&steps[1].tooltip)]);
        row.append(&plus);
        handles.push(plus.clone());

        // The halves carry the level they land on, which moves with the
        // level, so their clicks read the live step rather than the one
        // captured at build time.
        for (index, button) in handles.iter().enumerate() {
            button.set_sensitive(control.enabled(snapshot));
            let sender = self.feedback.clone();
            let targets = Rc::new(RefCell::new(steps[index].event.clone()));
            let click_target = targets.clone();
            button.connect_clicked(move |_| {
                send_event(&sender, click_target.borrow().clone());
            });
            self.updaters.borrow_mut().push(Box::new(move |snapshot| {
                if let Some(steps) = control.steps(snapshot) {
                    *targets.borrow_mut() = steps[index].event.clone();
                }
            }));
        }
        row.set_margin_end(gap_px.max(0));
        pill.append(&row);
        self.updaters.borrow_mut().push(Box::new(move |snapshot| {
            value.set_label(&control.required_value_text(snapshot));
            let enabled = control.enabled(snapshot);
            for button in &handles {
                button.set_sensitive(enabled);
            }
        }));
        // The docked control reports on the selected shape's own factor, so
        // it needs the same unavailable state the slider has.
        self.append_style_status_label(pill, control, snapshot, gap_px);
    }
}
