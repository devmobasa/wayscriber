use crate::ui::toolbar::ToolbarSnapshot;

use super::ToolbarLayoutSpec;

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) const TOP_SIZE_ICONS: (u32, u32) = (735, 58);
    /// Minimized top strip: the edge restore tab.
    pub(in crate::backend::wayland::toolbar) const TOP_MINIMIZED_SIZE: (u32, u32) = (64, 24);
    /// Micro-mode top strip: one round tool/color chip.
    pub(in crate::backend::wayland::toolbar) const TOP_MICRO_SIZE: (u32, u32) = (44, 44);
    pub(in crate::backend::wayland::toolbar) const TOP_SIZE_TEXT: (u32, u32) = (875, 60);

    pub(in crate::backend::wayland::toolbar) const TOP_GAP: f64 = 5.0;
    /// Clear space between the detached top-strip islands (pill edges).
    pub(in crate::backend::wayland::toolbar) const TOP_ISLAND_GAP: f64 = 10.0;
    /// Inner horizontal padding between an island edge and its content.
    /// The number lives in `theme::toolbar::ISLAND_PAD` so the GTK
    /// stylesheet interpolates the same island padding — one source for
    /// both frontends (theme must not depend on this backend module).
    pub(in crate::backend::wayland::toolbar) const TOP_ISLAND_PAD: f64 =
        crate::ui::theme::toolbar::ISLAND_PAD;
    pub(in crate::backend::wayland::toolbar) const TOP_START_X: f64 = 19.0;
    pub(in crate::backend::wayland::toolbar) const TOP_HANDLE_SIZE: f64 = 18.0;
    pub(in crate::backend::wayland::toolbar) const TOP_HANDLE_Y: f64 = 20.0;
    pub(in crate::backend::wayland::toolbar) const TOP_ICON_BUTTON: f64 = 46.0;
    pub(in crate::backend::wayland::toolbar) const TOP_ICON_SIZE: f64 = 28.0;
    pub(in crate::backend::wayland::toolbar) const TOP_ICON_FILL_HEIGHT: f64 = 18.0;
    pub(in crate::backend::wayland::toolbar) const TOP_ICON_FILL_OFFSET: f64 = 2.0;
    pub(in crate::backend::wayland::toolbar) const TOP_TEXT_BUTTON_W: f64 = 60.0;
    pub(in crate::backend::wayland::toolbar) const TOP_TEXT_BUTTON_H: f64 = 36.0;
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_SIZE: f64 = 24.0;
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_GAP: f64 = 6.0;
    pub(in crate::backend::wayland::toolbar) const TOP_PIN_BUTTON_MARGIN_RIGHT: f64 = 15.0;
    pub(in crate::backend::wayland::toolbar) const TOP_SHAPE_ROW_GAP: f64 = 6.0;
    /// Inner padding of the shapes/options popover panel.
    pub(in crate::backend::wayland::toolbar) const TOP_POPOVER_PAD: f64 = 8.0;
    /// Height of one option row (Fill, polygon sides) inside the popover.
    pub(in crate::backend::wayland::toolbar) const TOP_OPTION_ROW_H: f64 = 24.0;

    // ---- Style pill (island D): the contextual row under the islands ----
    /// Vertical gap between the island band and the style pill.
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_PILL_GAP: f64 = 6.0;
    /// Style pill height (one control row plus vertical padding).
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_PILL_H: f64 = 40.0;
    /// Slider track width inside the pill.
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_SLIDER_W: f64 = 110.0;
    /// Standard control-row height inside the pill (sliders, buttons,
    /// segments, numerals).
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_ROW_H: f64 = 24.0;
    /// Live numeral button width.
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_VALUE_W: f64 = 44.0;
    /// Mini-toggle height (Fill, Auto-number).
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_TOGGLE_H: f64 = 18.0;
    /// Fill toggle width.
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_FILL_W: f64 = 64.0;
    /// Auto-number toggle width.
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_AUTO_NUMBER_W: f64 = 108.0;
    /// Counter reset button width.
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_RESET_W: f64 = 56.0;
    /// Two-segment control width.
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_SEGMENT_W: f64 = 120.0;
    /// Docked selection-property value button width (cycle buttons and
    /// the readout between stepper halves).
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_SEL_VALUE_W: f64 = 64.0;
    /// Stepper half (−/+) width for docked numeric selection properties.
    pub(in crate::backend::wayland::toolbar) const TOP_STYLE_STEP_W: f64 = 20.0;

    pub(in crate::backend::wayland::toolbar) fn top_size(
        &self,
        snapshot: &ToolbarSnapshot,
    ) -> (u32, u32) {
        if snapshot.top_minimized {
            return Self::TOP_MINIMIZED_SIZE;
        }
        if snapshot.top_micro_active() {
            return Self::TOP_MICRO_SIZE;
        }
        let base_height = if self.use_icons {
            Self::TOP_SIZE_ICONS.1
        } else {
            Self::TOP_SIZE_TEXT.1
        };
        let mut height = base_height as f64;
        // Popovers (shapes grid + options, overflow) and the contextual
        // highlight-ring row grow the surface below the bar.
        height += crate::backend::wayland::toolbar::view::top::top_extra_height(snapshot);

        // Width comes from the same tree walk the builder performs, so the
        // size math and the builder cannot drift apart.
        let width =
            crate::backend::wayland::toolbar::view::top::top_natural_width(snapshot, height);

        (width.ceil() as u32, height.ceil() as u32)
    }

    pub(in crate::backend::wayland::toolbar) fn top_button_size(&self) -> (f64, f64) {
        if self.use_icons {
            (Self::TOP_ICON_BUTTON, Self::TOP_ICON_BUTTON)
        } else {
            (Self::TOP_TEXT_BUTTON_W, Self::TOP_TEXT_BUTTON_H)
        }
    }
}
