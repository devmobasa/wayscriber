//! Side-palette assembly, pane navigation, and scroll restoration.

use super::*;

impl SideBar {
    pub(super) fn sync_viewport(&self, snapshot: &ToolbarSnapshot) {
        // Resizing a layer surface while its gesture coordinates are being
        // consumed makes the surface catch up after button release. Keep the
        // allocated height stable for the gesture; the final backend echo
        // applies the viewport for the accepted resting position.
        if self.drag_active.get() {
            return;
        }
        let Some(scrolled) = &self.scrolled else {
            return;
        };
        let scale = effective_scale(snapshot);
        if let Some(viewport) = snapshot.side_viewport_max {
            // The viewport budget covers the whole palette; subtract the
            // fixed chrome above the scrolled body.
            let chrome = 76.0;
            let max = ((viewport - chrome).max(120.0) * scale).round() as i32;
            scrolled.set_max_content_height(max);
        }
    }

    pub(super) fn rebuild(&mut self, snapshot: &ToolbarSnapshot) {
        // Preserve the outgoing pane's scroll position, keyed by pane so
        // it is only restored into the same pane.
        if let (Some(scrolled), Some(key)) = (&self.scrolled, &self.structure) {
            let value = scrolled.vadjustment().value();
            let mut saved = self.saved_scroll.borrow_mut();
            saved.retain(|(pane, _)| *pane != key.pane);
            saved.push((key.pane, value));
        }
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
        self.chrome_updaters.clear();
        self.content_updaters.clear();
        self.scrolled = None;

        if snapshot.side_minimized {
            self.build_minimized(snapshot);
        } else {
            self.build_palette(snapshot);
        }
    }

    fn build_minimized(&mut self, snapshot: &ToolbarSnapshot) {
        let scale = effective_scale(snapshot);
        self.root.add_css_class("minimized");
        // Drop the palette's 260px default width so the tab shrinks.
        self.window.set_default_size(
            (MINIMIZED_SIZE.0 * scale).round() as i32,
            (MINIMIZED_SIZE.1 * scale).round() as i32,
        );
        let restore = sized_button(MINIMIZED_SIZE.0 * scale, MINIMIZED_SIZE.1 * scale);
        restore.add_css_class("chrome");
        restore.set_tooltip_text(Some("Show toolbar"));
        let icon = IconWidget::new(
            toolbar_icons::draw_icon_chevron_right,
            (MINIMIZED_SIZE.0 * 0.75 * scale).min(18.0 * scale),
        );
        restore.set_child(Some(&icon.area));
        let sender = self.feedback.clone();
        restore.connect_clicked(move |_| {
            send_event(&sender, ToolbarEvent::SetSideMinimized(false));
        });
        self.root.append(&restore);
    }

    fn build_palette(&mut self, snapshot: &ToolbarSnapshot) {
        self.root.remove_css_class("minimized");
        let scale = effective_scale(snapshot);
        let px = |value: f64| (value * scale).round() as i32;
        self.window
            .set_default_size((SIDE_WIDTH * scale).round() as i32, -1);

        // ===== Header band: grip · board chip · pin · minimize =====
        let band = gtk4::Box::new(gtk4::Orientation::Horizontal, px(6.0));
        band.add_css_class("header-band");

        let grip = IconWidget::new(toolbar_icons::draw_icon_drag, 18.0 * scale);
        grip.area.set_can_target(true);
        grip.area.add_css_class("drag-handle");
        grip.area.set_tooltip_text(Some("Drag toolbar"));
        grip.area.set_valign(gtk4::Align::Center);
        grip.area.set_cursor_from_name(Some("grab"));
        self.attach_move_drag(&grip.area);
        band.append(&grip.area);

        let chip = self.board_chip(snapshot, scale);
        band.append(&chip);

        band.append(&self.pin_button(snapshot, 22.0 * scale));
        band.append(&self.minimize_button(22.0 * scale));
        self.root.append(&band);

        // ===== Pane navigation =====
        let nav = gtk4::Box::new(gtk4::Orientation::Horizontal, px(4.0));
        nav.set_homogeneous(true);
        nav.set_margin_top(px(6.0));
        for pane in SidePane::ALL {
            let tab = gtk4::Button::with_label(pane.label());
            tab.add_css_class("tab");
            tab.set_size_request(-1, px(26.0));
            tab.set_tooltip_text(Some(&format!("{} pane", pane.label())));
            let sender = self.feedback.clone();
            tab.connect_clicked(move |_| {
                send_event(&sender, ToolbarEvent::SetSidePane(pane));
            });
            let handle = tab.clone();
            self.chrome_updaters.push(Box::new(move |snapshot| {
                set_active_class(&handle, snapshot.active_side_pane == pane);
            }));
            nav.append(&tab);
        }
        self.root.append(&nav);

        // ===== Scrolled pane body =====
        let mut content_updaters = Vec::new();
        let content = sections::build_pane_content(
            snapshot,
            self.feedback.clone(),
            scale,
            &mut content_updaters,
        );
        content.set_margin_top(px(8.0));
        self.content_updaters = content_updaters;

        let scrolled = gtk4::ScrolledWindow::new();
        scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scrolled.set_propagate_natural_height(true);
        scrolled.set_child(Some(&content));
        self.root.append(&scrolled);

        // Restore this pane's scroll position once the new content is
        // laid out; one-shot so later size changes never yank the user's
        // scroll.
        let adjustment = scrolled.vadjustment();
        let pane = snapshot.active_side_pane;
        let pending = std::cell::Cell::new(
            self.saved_scroll
                .borrow()
                .iter()
                .find(|(saved_pane, _)| *saved_pane == pane)
                .map(|(_, value)| *value)
                .unwrap_or(0.0),
        );
        adjustment.connect_changed(move |adjustment| {
            let target = pending.get();
            let reachable = adjustment.upper() - adjustment.page_size();
            if target > 0.0 && reachable >= target {
                adjustment.set_value(target);
                pending.set(0.0);
            }
        });
        self.scrolled = Some(scrolled);
    }
}
