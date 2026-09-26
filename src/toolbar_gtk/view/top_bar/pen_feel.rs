//! The GTK style pill's Pen feel chip and the panel popover it opens
//! (`stroke_controls = "panel"`, the default).
//!
//! Mirrors the builtin `view/top/pen_feel.rs`: the sections, wording, and
//! geometry come from the shared `model::pen_feel_*`, the meters are the same
//! bar rows the inline meters use, and the preview is drawn by the painter
//! both frontends share. The popover follows the layout menu's pattern: no
//! autohide grab (the backend's dismissal policy owns click-away), keys
//! relayed to the overlay, and `closed` echoing a user dismissal.

use super::meter::{MeterBarsSpec, append_meter_bars};
use super::*;

impl TopBar {
    /// Appends the Pen feel chip to `pill`, followed by `gap_px` of margin,
    /// installs the panel popover anchored to it, and registers the chip's
    /// updater.
    pub(super) fn append_pen_feel_chip(
        &mut self,
        pill: &gtk4::Box,
        control: model::StylePillControl,
        snapshot: &ToolbarSnapshot,
        scale: f64,
        gap_px: i32,
    ) {
        let chip = sized_button(STYLE_PEN_FEEL_W * scale, STYLE_ROW_H * scale);
        chip.set_label(control.label(snapshot).as_ref());
        set_semantic_widget_id(&chip, control.id().as_ref());
        let sender = self.feedback.clone();
        let expected = self.feel.expected_open.clone();
        chip.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::TogglePenFeelPanel(!expected.get()));
        });
        let handle = chip.clone();
        let refresh = move |snapshot: &ToolbarSnapshot| {
            handle.set_tooltip_text(control.tooltip(snapshot).as_deref());
            handle.update_property(&[gtk4::accessible::Property::Label(
                &model::StylePillControl::pen_feel_accessible_label(snapshot),
            )]);
            set_active_class(&handle, control.active(snapshot));
        };
        refresh(snapshot);
        self.updaters.borrow_mut().push(Box::new(refresh));

        let popover = gtk4::Popover::new();
        popover.set_parent(&chip);
        popover.set_position(gtk4::PositionType::Bottom);
        popover.set_autohide(false);
        install_key_relay(&popover, &self.feedback);
        let sender = self.feedback.clone();
        let expected = self.feel.expected_open.clone();
        popover.connect_closed(move |_| {
            if expected.get() {
                send_event(&sender, ToolbarEvent::TogglePenFeelPanel(false));
            }
        });
        let capture_surface = CaptureSurfaceContent::empty();
        popover.set_child(Some(capture_surface.widget()));
        self.feel.install(popover, capture_surface);

        chip.set_margin_end(gap_px.max(0));
        pill.append(&chip);
    }

    /// Keep the panel's content and open state in line with the snapshot.
    /// Content rebuilds only when the tool changes which sections it shows;
    /// levels ride the panel's updaters, so a click inside never rebuilds it.
    pub(super) fn sync_pen_feel_panel(&mut self, snapshot: &ToolbarSnapshot, scale: f64) {
        let Some(resources) = self.feel.mounted.clone() else {
            self.feel.expected_open.set(false);
            return;
        };

        let open = snapshot.pen_feel_open;
        if open {
            let content_key = model::pen_feel_settings(snapshot);
            if self.feel.content_key.as_ref() != Some(&content_key) {
                let (content, updaters) = self.build_pen_feel_content(snapshot, scale);
                resources.capture_surface.set_content(&content);
                self.feel.updaters = updaters;
                self.feel.content_key = Some(content_key);
            }
        }
        self.feel.set_open(open);
    }

    /// Panel content: the title, then one section per setting the tool uses.
    /// Returns the content with the updaters that keep its levels live.
    pub(super) fn build_pen_feel_content(
        &self,
        snapshot: &ToolbarSnapshot,
        scale: f64,
    ) -> (gtk4::Box, Vec<Updater>) {
        let px = |value: f64| (value * scale).round() as i32;
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        set_semantic_widget_id(&content, "top.feel.panel");
        content.set_size_request(px(model::PEN_FEEL_CONTENT_W), -1);
        // A popover is its own GTK native: without this, the rebind chord is
        // invisible to the clicks inside it.
        install_click_modifier_capture(&content, &self.feedback);

        let title = gtk4::Label::new(Some(model::PEN_FEEL_TITLE));
        title.add_css_class("section-title");
        title.set_xalign(0.0);
        title.set_size_request(-1, px(model::PEN_FEEL_TITLE_H));
        set_semantic_widget_id(&title, "top.feel.title");
        content.append(&title);

        let mut updaters = Vec::new();
        for section in model::pen_feel_sections(snapshot) {
            self.append_pen_feel_section(&content, &section, snapshot, scale, &mut updaters);
        }
        (content, updaters)
    }

    fn append_pen_feel_section(
        &self,
        content: &gtk4::Box,
        section: &model::PenFeelSection,
        snapshot: &ToolbarSnapshot,
        scale: f64,
        updaters: &mut Vec<Updater>,
    ) {
        let px = |value: f64| (value * scale).round() as i32;
        let setting = section.setting;

        let header = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        header.set_margin_top(px(model::PEN_FEEL_SECTION_GAP));
        header.set_size_request(-1, px(model::PEN_FEEL_HEADER_H));
        let name = gtk4::Label::new(Some(setting.name()));
        name.add_css_class("meter-caption");
        name.set_xalign(0.0);
        name.set_hexpand(true);
        set_semantic_widget_id(&name, &section.id("label"));
        let value = gtk4::Label::new(Some(section.level_name));
        value.add_css_class("pen-feel-value");
        value.set_xalign(1.0);
        set_semantic_widget_id(&value, &section.id("value"));
        header.append(&name);
        header.append(&value);
        content.append(&header);

        let bars = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        bars.set_margin_top(px(model::PEN_FEEL_ROW_GAP));
        set_semantic_widget_id(&bars, &section.id("bars"));
        bars.update_property(&[gtk4::accessible::Property::Label(&format!(
            "{} level",
            setting.name()
        ))]);
        updaters.push(append_meter_bars(
            &self.feedback,
            &bars,
            MeterBarsSpec {
                setting,
                id_prefix: format!("top.feel.{}", setting.panel_key()),
                size: (model::PEN_FEEL_CONTENT_W, model::PEN_FEEL_BARS_H),
            },
            |_| true,
            snapshot,
            scale,
        ));
        content.append(&bars);

        let preview = section.has_preview().then(|| {
            let preview = smoothing_preview_area(section, scale);
            content.append(&preview.area);
            preview
        });

        let hint = gtk4::Label::new(Some(section.hint));
        hint.add_css_class("hint");
        hint.set_xalign(0.0);
        hint.set_margin_top(px(model::PEN_FEEL_ROW_GAP));
        // A hint is one line at the column's width; ellipsizing keeps a wider
        // font from growing the panel past the builtin's footprint.
        hint.set_ellipsize(pango::EllipsizeMode::End);
        hint.set_max_width_chars(1);
        hint.set_width_request(px(model::PEN_FEEL_CONTENT_W));
        set_semantic_widget_id(&hint, &section.id("hint"));
        content.append(&hint);

        let applied: Cell<Option<u8>> = Cell::new(None);
        updaters.push(Box::new(move |snapshot| {
            let level = setting.level(snapshot);
            if applied.replace(Some(level)) == Some(level) {
                return;
            }

            value.set_label(setting.level_name(level));
            hint.set_label(setting.level_hint(level));
            if let Some(preview) = &preview {
                preview.set_level(level);
            }
        }));
    }
}

/// The live smoothing preview: a drawing area painting the shared preview at
/// the level its updater last set.
struct SmoothingPreviewArea {
    area: gtk4::DrawingArea,
    level: Rc<Cell<u8>>,
}

impl SmoothingPreviewArea {
    fn set_level(&self, level: u8) {
        if self.level.replace(level) == level {
            return;
        }

        self.area
            .update_property(&[gtk4::accessible::Property::Label(&preview_label(level))]);
        self.area.queue_draw();
    }
}

fn smoothing_preview_area(section: &model::PenFeelSection, scale: f64) -> SmoothingPreviewArea {
    let px = |value: f64| (value * scale).round() as i32;
    let area = gtk4::DrawingArea::builder()
        .accessible_role(gtk4::AccessibleRole::Img)
        .build();
    area.set_content_width(px(model::PEN_FEEL_CONTENT_W));
    area.set_content_height(px(model::PEN_FEEL_PREVIEW_H));
    area.set_margin_top(px(model::PEN_FEEL_ROW_GAP));
    area.set_can_target(false);
    set_semantic_widget_id(&area, &section.id("preview"));
    area.update_property(&[gtk4::accessible::Property::Label(&preview_label(
        section.level,
    ))]);

    let level = Rc::new(Cell::new(section.level));
    let draw_level = level.clone();
    area.set_draw_func(move |_, ctx, width, height| {
        crate::toolbar_icons::draw_smoothing_preview(
            ctx,
            (0.0, 0.0, f64::from(width), f64::from(height)),
            draw_level.get(),
        );
    });
    SmoothingPreviewArea { area, level }
}

/// What the preview shows, for assistive tech.
fn preview_label(level: u8) -> String {
    format!(
        "Smoothing preview: a sample stroke at {}",
        model::StrokeSetting::Smoothing.level_name(level)
    )
}
