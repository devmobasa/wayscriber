mod actions;
mod arrow;
mod boards;
mod colors;
mod drawer;
mod header;
mod marker;
mod pages;
mod presets;
mod settings;
mod step;
mod text;
mod thickness;

use anyhow::Result;

use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::toolbar::ToolbarSnapshot;

use super::widgets::{draw_panel_background, draw_tooltip};

pub(super) struct SidePaletteLayout<'a> {
    pub(super) ctx: &'a cairo::Context,
    pub(super) width: f64,
    pub(super) snapshot: &'a ToolbarSnapshot,
    pub(super) hits: &'a mut Vec<HitRegion>,
    pub(super) hover: Option<(f64, f64)>,
    pub(super) spec: ToolbarLayoutSpec,
    pub(super) x: f64,
    pub(super) card_x: f64,
    pub(super) card_w: f64,
    pub(super) content_width: f64,
    pub(super) section_gap: f64,
}

impl<'a> SidePaletteLayout<'a> {
    fn new(
        ctx: &'a cairo::Context,
        width: f64,
        snapshot: &'a ToolbarSnapshot,
        hits: &'a mut Vec<HitRegion>,
        hover: Option<(f64, f64)>,
    ) -> Self {
        let spec = ToolbarLayoutSpec::new(snapshot);
        let x = ToolbarLayoutSpec::SIDE_START_X;
        let card_x = spec.side_card_x();
        let card_w = spec.side_card_width(width);
        let content_width = spec.side_content_width(width);
        let section_gap = ToolbarLayoutSpec::SIDE_SECTION_GAP;
        Self {
            ctx,
            width,
            snapshot,
            hits,
            hover,
            spec,
            x,
            card_x,
            card_w,
            content_width,
            section_gap,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ColorSectionInfo {
    pub(super) picker_y: f64,
    pub(super) picker_w: f64,
    pub(super) picker_h: f64,
}

pub fn render_side_palette(
    ctx: &cairo::Context,
    width: f64,
    height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
) -> Result<()> {
    draw_panel_background(ctx, width, height);

    let mut layout = SidePaletteLayout::new(ctx, width, snapshot, hits, hover);

    let mut y = header::draw_header(&mut layout);

    let colors_info = colors::draw_colors_section(&mut layout, &mut y);
    let hover_preset_color = presets::draw_presets_section(&mut layout, &mut y);
    if let Some(color) = hover_preset_color {
        colors::draw_preset_hover_highlight(&layout, &colors_info, color);
    }

    thickness::draw_thickness_section(&mut layout, &mut y);
    arrow::draw_arrow_section(&mut layout, &mut y);
    marker::draw_marker_opacity_section(&mut layout, &mut y);
    text::draw_text_controls_section(&mut layout, &mut y);
    drawer::draw_drawer_tabs(&mut layout, &mut y);
    actions::draw_actions_section(&mut layout, &mut y);
    boards::draw_boards_section(&mut layout, &mut y);
    pages::draw_pages_section(&mut layout, &mut y);
    step::draw_step_section(&mut layout, &mut y);
    settings::draw_settings_section(&mut layout, &mut y);

    draw_tooltip(ctx, layout.hits, layout.hover, width, false);
    Ok(())
}
