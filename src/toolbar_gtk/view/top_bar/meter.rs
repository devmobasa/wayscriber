//! GTK style-pill level meters: a caption naming the setting, then one bar
//! button per level. Mirrors the builtin `view/top/meter.rs` slot for slot;
//! the Pen feel panel lays the same bars across its wider column.

use super::*;

/// Gap between neighbouring bars; matches the builtin painter's `BAR_GAP`.
const METER_BAR_GAP: f64 = 3.0;

/// Where a row of meter bars comes from and how big it is.
pub(super) struct MeterBarsSpec {
    pub(super) setting: model::StrokeSetting,
    /// Bars are named `<id_prefix>.level-<n>`, as in the builtin tree.
    pub(super) id_prefix: String,
    /// The row the bars divide evenly, in spec units.
    pub(super) size: (f64, f64),
}

/// Appends one bar button per level of `spec.setting` to `row` and installs
/// the wheel on it: one level per notch, like the builtin. Returns the updater
/// that keeps fills, tooltips and sensitivity on the live level.
pub(super) fn append_meter_bars(
    feedback: &FeedbackSender,
    row: &gtk4::Box,
    spec: MeterBarsSpec,
    enabled: impl Fn(&ToolbarSnapshot) -> bool + 'static,
    snapshot: &ToolbarSnapshot,
    scale: f64,
) -> Updater {
    let MeterBarsSpec {
        setting,
        id_prefix,
        size: (row_w, row_h),
    } = spec;
    let px = |value: f64| (value * scale).round() as i32;
    let meter = setting.meter(snapshot, &id_prefix);

    let dot_refresh = append_meter_zero(feedback, row, setting, &id_prefix, row_h, snapshot, scale);
    let bar_w = (row_w - model::METER_ZERO_SLOT_W) / meter.segments.len().max(1) as f64;
    let mut bars = Vec::with_capacity(meter.segments.len());
    for (index, segment) in meter.segments.iter().enumerate() {
        let bar = sized_button(bar_w * scale, row_h * scale);
        bar.add_css_class("meter-bar");
        set_semantic_widget_id(&bar, &segment.id);
        // The bar takes the slot less the gap between bars, like the
        // builtin painter; a bare box would shrink to a dot.
        let fill = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        fill.add_css_class("meter-fill");
        fill.set_halign(gtk4::Align::Center);
        fill.set_valign(gtk4::Align::Center);
        fill.set_size_request(px((bar_w - METER_BAR_GAP).max(1.0)), -1);
        bar.set_child(Some(&fill));

        let sender = feedback.clone();
        bar.connect_clicked(move |_| send_event(&sender, setting.event(index as u8 + 1)));
        row.append(&bar);
        bars.push(bar);
    }

    // One level per wheel notch; `DISCRETE` folds touchpad scrolling into
    // notches so a small swipe does not jump the whole range, and reports a
    // coalesced frame as several notches at once.
    let wheel = gtk4::EventControllerScroll::new(
        gtk4::EventControllerScrollFlags::VERTICAL | gtk4::EventControllerScrollFlags::DISCRETE,
    );
    let sender = feedback.clone();
    wheel.connect_scroll(move |_, _, dy| {
        // Positive dy scrolls down; the meter rises when the user scrolls up.
        let steps = (dy.round() as i32).saturating_neg();
        if steps != 0 {
            send_event(&sender, setting.nudge_event(steps));
        }
        gtk4::glib::Propagation::Stop
    });
    row.add_controller(wheel);

    // Updaters run on every toolbar update, many per second while the strip
    // fades. Touch the bars only when their level or sensitivity changed:
    // rewriting unchanged tooltips, classes, and labels makes an open popover
    // commit every frame, which is enough to cost it its keyboard grab.
    let applied: Cell<Option<(u8, bool)>> = Cell::new(None);
    let refresh = move |snapshot: &ToolbarSnapshot| {
        let enabled = enabled(snapshot);
        dot_refresh(snapshot, enabled);
        let state = (setting.level(snapshot), enabled);
        if applied.replace(Some(state)) == Some(state) {
            return;
        }

        let meter = setting.meter(snapshot, &id_prefix);
        for (bar, segment) in bars.iter().zip(&meter.segments) {
            if segment.filled {
                bar.add_css_class("filled");
            } else {
                bar.remove_css_class("filled");
            }
            bar.set_tooltip_text(Some(&segment.tooltip));
            bar.update_property(&[gtk4::accessible::Property::Label(&segment.tooltip)]);
            bar.set_sensitive(enabled);
        }
    };
    refresh(snapshot);
    Box::new(refresh)
}

/// Exact shared Cairo dot geometry inside a GTK button, including its hover ring.
pub(super) fn append_meter_zero(
    feedback: &FeedbackSender,
    row: &gtk4::Box,
    setting: model::StrokeSetting,
    id_prefix: &str,
    row_h: f64,
    snapshot: &ToolbarSnapshot,
    scale: f64,
) -> impl Fn(&ToolbarSnapshot, bool) + 'static {
    let dot = sized_button(model::METER_ZERO_SLOT_W * scale, row_h * scale);
    dot.add_css_class("meter-bar");
    dot.add_css_class("meter-dot");
    let zero = setting.zero_segment(snapshot, id_prefix);
    set_semantic_widget_id(&dot, &zero.id);
    let area = gtk4::DrawingArea::new();
    area.set_content_width((model::METER_ZERO_SLOT_W * scale).round() as i32);
    area.set_content_height((row_h * scale).round() as i32);
    let paint_state = Rc::new(Cell::new((setting.level(snapshot) == 0, false, true)));
    let state = paint_state.clone();
    area.set_draw_func(move |_, ctx, _, _| {
        let (active, hover, enabled) = state.get();
        let _ = ctx.save();
        ctx.scale(scale, scale);
        crate::toolbar_icons::draw_meter_dot(
            ctx,
            (3.0, 0.0, model::METER_ZERO_SLOT_W, row_h),
            active,
            hover,
            enabled,
        );
        let _ = ctx.restore();
    });
    // The hover ring extends left of the 22px slot. Paint with a 3px
    // overhang so GTK does not clip that edge of the shared circle.
    let overhang = gtk4::Fixed::new();
    overhang.set_size_request(
        (model::METER_ZERO_SLOT_W * scale).round() as i32,
        (row_h * scale).round() as i32,
    );
    overhang.set_overflow(gtk4::Overflow::Visible);
    overhang.put(&area, -3.0 * scale, 0.0);
    dot.set_child(Some(&overhang));
    let motion = gtk4::EventControllerMotion::new();
    let state = paint_state.clone();
    let draw = area.clone();
    motion.connect_enter(move |_, _, _| {
        let (active, _, enabled) = state.get();
        state.set((active, true, enabled));
        draw.queue_draw();
    });
    let state = paint_state.clone();
    let draw = area.clone();
    motion.connect_leave(move |_| {
        let (active, _, enabled) = state.get();
        state.set((active, false, enabled));
        draw.queue_draw();
    });
    dot.add_controller(motion);
    let sender = feedback.clone();
    dot.connect_clicked(move |_| send_event(&sender, setting.event(0)));
    row.append(&dot);

    // Same rule as the bars: touch the dot only when its level or
    // sensitivity changed, never on every toolbar update.
    let id_prefix = id_prefix.to_owned();
    let applied: Cell<Option<(u8, bool)>> = Cell::new(None);
    move |snapshot, enabled| {
        let level = setting.level(snapshot);
        if applied.replace(Some((level, enabled))) == Some((level, enabled)) {
            return;
        }

        let (_, hover, _) = paint_state.get();
        paint_state.set((level == 0, hover, enabled));
        area.queue_draw();
        let zero = setting.zero_segment(snapshot, &id_prefix);
        dot.set_tooltip_text(Some(&zero.tooltip));
        dot.update_property(&[gtk4::accessible::Property::Label(&zero.tooltip)]);
        dot.set_sensitive(enabled);
    }
}

impl TopBar {
    /// Appends one level meter to `pill`, followed by `gap_px` of margin, and
    /// registers its updater.
    pub(super) fn append_style_meter(
        &mut self,
        pill: &gtk4::Box,
        control: model::StylePillControl,
        snapshot: &ToolbarSnapshot,
        scale: f64,
        gap_px: i32,
    ) {
        let px = |value: f64| (value * scale).round() as i32;
        let setting = control
            .meter_setting()
            .expect("this style-pill control is a level meter");

        // No spacing between the parts: the builtin lays the caption and the
        // bars out abutting, and the width planner budgets exactly that.
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        set_semantic_widget_id(&row, control.id().as_ref());
        row.update_property(&[gtk4::accessible::Property::Label(&control.label(snapshot))]);
        row.set_valign(gtk4::Align::Center);
        if let Some(caption) = control.caption() {
            let caption_label = gtk4::Label::new(Some(caption));
            caption_label.add_css_class("meter-caption");
            set_semantic_widget_id(&caption_label, &format!("{}.caption", control.id()));
            caption_label.set_xalign(0.0);
            caption_label.set_width_request(px(STYLE_CAPTION_W));
            row.append(&caption_label);
        }

        let refresh = append_meter_bars(
            &self.feedback,
            &row,
            MeterBarsSpec {
                setting,
                id_prefix: control.id().into_owned(),
                size: (STYLE_METER_W, STYLE_ROW_H),
            },
            move |snapshot| control.enabled(snapshot),
            snapshot,
            scale,
        );
        self.updaters.borrow_mut().push(refresh);

        row.set_margin_end(gap_px.max(0));
        pill.append(&row);
    }
}
