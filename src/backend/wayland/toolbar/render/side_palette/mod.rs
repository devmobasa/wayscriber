mod actions;
mod arrow;
mod boards;
mod colors;
mod drawer;
mod header;
mod marker;
mod pages;
mod presets;
mod section_header;
mod session;
mod settings;
mod step;
mod step_marker;
mod text;
mod thickness;

use anyhow::Result;

use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::ToolbarDrawerTab;
use crate::ui::toolbar::model;
use crate::ui::toolbar::snapshot::ToolContext;
use crate::ui::toolbar::{ToolbarSideSection, ToolbarSnapshot};

use std::time::Instant;

use super::widgets::{draw_panel_background, draw_tooltip_with_delay};

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
    hover_start: Option<Instant>,
) -> Result<()> {
    draw_panel_background(ctx, width, height);

    let mut layout = SidePaletteLayout::new(ctx, width, snapshot, hits, hover);
    let tool_context = ToolContext::from_snapshot(snapshot);

    let mut y = header::draw_header(&mut layout);

    if snapshot.drawer_open
        && (snapshot.customize_items_open
            || snapshot.drawer_tab == ToolbarDrawerTab::Customize
            || snapshot.drawer_tab == ToolbarDrawerTab::Session)
    {
        drawer::draw_drawer_tabs(&mut layout, &mut y);
        if snapshot.drawer_tab == ToolbarDrawerTab::Session {
            session::draw_session_section(&mut layout, &mut y);
        } else {
            settings::draw_settings_section(&mut layout, &mut y);
        }
        draw_tooltip_with_delay(
            ctx,
            layout.hits,
            layout.hover,
            width,
            height,
            false,
            hover_start,
        );
        return Ok(());
    }

    let mut colors_info = None;
    let mut hover_preset_color = None;
    let mut drawer_tabs_drawn = false;
    let mut thickness_block_drawn = false;
    let mut text_block_drawn = false;
    for section in model::ordered_side_sections(snapshot) {
        match section {
            ToolbarSideSection::Colors
                if tool_context.needs_color
                    && !snapshot.side_section_hidden(ToolbarSideSection::Colors) =>
            {
                colors_info = colors::draw_colors_section(&mut layout, &mut y);
            }
            ToolbarSideSection::Presets
                if !snapshot.side_section_hidden(ToolbarSideSection::Presets) =>
            {
                hover_preset_color = presets::draw_presets_section(&mut layout, &mut y);
            }
            ToolbarSideSection::Thickness
            | ToolbarSideSection::EraserMode
            | ToolbarSideSection::PolygonSides
                if tool_context.needs_thickness && !thickness_block_drawn =>
            {
                thickness_block_drawn = true;
                thickness::draw_thickness_section(&mut layout, &mut y);
            }
            ToolbarSideSection::ArrowLabels
                if tool_context.show_arrow_labels
                    && !snapshot.side_section_hidden(ToolbarSideSection::ArrowLabels) =>
            {
                arrow::draw_arrow_section(&mut layout, &mut y);
            }
            ToolbarSideSection::StepMarkers
                if tool_context.show_step_counter
                    && !snapshot.side_section_hidden(ToolbarSideSection::StepMarkers) =>
            {
                step_marker::draw_step_marker_section(&mut layout, &mut y);
            }
            ToolbarSideSection::MarkerOpacity
                if tool_context.show_marker_opacity
                    && !snapshot.side_section_hidden(ToolbarSideSection::MarkerOpacity) =>
            {
                marker::draw_marker_opacity_section(&mut layout, &mut y);
            }
            ToolbarSideSection::TextSize | ToolbarSideSection::Font
                if tool_context.show_font_controls && !text_block_drawn =>
            {
                text_block_drawn = true;
                text::draw_text_controls_section(&mut layout, &mut y);
            }
            ToolbarSideSection::Actions => {
                draw_drawer_tabs_once(&mut layout, &mut y, &mut drawer_tabs_drawn);
                actions::draw_actions_section(&mut layout, &mut y);
            }
            ToolbarSideSection::Boards => {
                draw_drawer_tabs_once(&mut layout, &mut y, &mut drawer_tabs_drawn);
                boards::draw_boards_section(&mut layout, &mut y);
            }
            ToolbarSideSection::Pages => {
                draw_drawer_tabs_once(&mut layout, &mut y, &mut drawer_tabs_drawn);
                pages::draw_pages_section(&mut layout, &mut y);
            }
            ToolbarSideSection::StepUndo => {
                draw_drawer_tabs_once(&mut layout, &mut y, &mut drawer_tabs_drawn);
                step::draw_step_section(&mut layout, &mut y);
            }
            ToolbarSideSection::Session => {
                draw_drawer_tabs_once(&mut layout, &mut y, &mut drawer_tabs_drawn);
                session::draw_session_section(&mut layout, &mut y);
            }
            ToolbarSideSection::Settings => {
                draw_drawer_tabs_once(&mut layout, &mut y, &mut drawer_tabs_drawn);
                settings::draw_settings_section(&mut layout, &mut y);
            }
            _ => {}
        }
    }
    draw_drawer_tabs_once(&mut layout, &mut y, &mut drawer_tabs_drawn);
    if let (Some(color), Some(info)) = (hover_preset_color, &colors_info) {
        colors::draw_preset_hover_highlight(&layout, info, color);
    }

    draw_tooltip_with_delay(
        ctx,
        layout.hits,
        layout.hover,
        width,
        height,
        false,
        hover_start,
    );
    Ok(())
}

fn draw_drawer_tabs_once(layout: &mut SidePaletteLayout<'_>, y: &mut f64, drawn: &mut bool) {
    if *drawn {
        return;
    }
    drawer::draw_drawer_tabs(layout, y);
    *drawn = true;
}
