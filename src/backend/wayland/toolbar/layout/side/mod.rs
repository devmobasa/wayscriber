mod actions;
mod colors;
mod delay;
mod drawer;
mod header;
mod pages;
mod presets;
mod settings;
mod sliders;

pub(super) use super::super::events::{HitKind, delay_secs_from_t, delay_t_from_ms};
pub(super) use super::super::format_binding_label;
pub(super) use super::super::hit::HitRegion;
pub(super) use super::spec::ToolbarLayoutSpec;
pub(super) use crate::config::ToolbarLayoutMode;
pub(super) use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

/// Populate hit regions for the side toolbar.
#[allow(dead_code)]
pub fn build_side_hits(
    width: f64,
    _height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
) {
    let ctx = SideLayoutContext::new(width, snapshot);

    header::push_header_hits(&ctx, hits);

    let mut y = ctx.spec.side_content_start_y();

    y = colors::push_color_picker_hits(&ctx, y, hits);
    y = presets::push_preset_hits(&ctx, y, hits);
    y = sliders::push_thickness_hits(&ctx, y, hits);

    if snapshot.thickness_targets_eraser {
        y += ToolbarLayoutSpec::SIDE_ERASER_MODE_CARD_HEIGHT + ctx.section_gap;
    }

    let show_marker_opacity =
        snapshot.show_marker_opacity_section || snapshot.thickness_targets_marker;
    if show_marker_opacity {
        y += ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT + ctx.section_gap;
    }

    y = sliders::push_text_hits(&ctx, y, hits);
    y = drawer::push_drawer_tabs_hits(&ctx, y, hits);
    y = actions::push_actions_hits(&ctx, y, hits);
    y = pages::push_pages_hits(&ctx, y, hits);
    y = delay::push_delay_hits(&ctx, y, hits);

    settings::push_settings_hits(&ctx, y, hits);
}

pub(super) struct SideLayoutContext<'a> {
    pub(super) width: f64,
    pub(super) snapshot: &'a ToolbarSnapshot,
    pub(super) spec: ToolbarLayoutSpec,
    pub(super) x: f64,
    pub(super) content_width: f64,
    pub(super) use_icons: bool,
    pub(super) section_gap: f64,
    pub(super) show_text_controls: bool,
}

impl<'a> SideLayoutContext<'a> {
    fn new(width: f64, snapshot: &'a ToolbarSnapshot) -> Self {
        let spec = ToolbarLayoutSpec::new(snapshot);
        let use_icons = spec.use_icons();
        let x = ToolbarLayoutSpec::SIDE_START_X;
        let content_width = spec.side_content_width(width);
        let section_gap = ToolbarLayoutSpec::SIDE_SECTION_GAP;
        let show_text_controls =
            snapshot.text_active || snapshot.note_active || snapshot.show_text_controls;
        Self {
            width,
            snapshot,
            spec,
            x,
            content_width,
            use_icons,
            section_gap,
            show_text_controls,
        }
    }
}
