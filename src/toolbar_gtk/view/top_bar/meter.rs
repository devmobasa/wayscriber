//! GTK style-pill level meters: a caption naming the setting, then one bar
//! button per level. Mirrors the builtin `view/top/meter.rs` slot for slot;
//! the Pen feel panel lays the same bars across its wider column.

use super::*;

/// Current click target of every bar, refreshed from each snapshot. The
/// handlers read these rather than the build-time meter because clicking
/// the top filled bar lowers the level, so
/// what a bar does changes with the level.
#[derive(Default)]
struct MeterTargets {
    bars: Vec<ToolbarEvent>,
}

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
/// that keeps fills, tooltips, sensitivity, and targets on the live level.
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

    let targets = Rc::new(RefCell::new(MeterTargets::default()));
    let bar_w = row_w / meter.segments.len().max(1) as f64;
    let mut bars = Vec::with_capacity(meter.segments.len());
    for (index, segment) in meter.segments.iter().enumerate() {
        let bar = sized_button(bar_w * scale, row_h * scale);
        bar.add_css_class("meter-bar");
        set_semantic_widget_id(&bar, &segment.id);
        // The bar takes the slot less the gap between bars, like the
        // builtin painter; a bare box would shrink to a dot.
        let fill = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        fill.add_css_class("meter-fill");
        fill.set_valign(gtk4::Align::Center);
        fill.set_size_request(px((bar_w - METER_BAR_GAP).max(1.0)), -1);
        bar.set_child(Some(&fill));

        let sender = feedback.clone();
        let targets = targets.clone();
        bar.connect_clicked(move |_| {
            let event = targets.borrow().bars.get(index).cloned();
            if let Some(event) = event {
                send_event(&sender, event);
            }
        });
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
        *targets.borrow_mut() = MeterTargets {
            bars: meter
                .segments
                .into_iter()
                .map(|segment| segment.event)
                .collect(),
        };
    };
    refresh(snapshot);
    Box::new(refresh)
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
