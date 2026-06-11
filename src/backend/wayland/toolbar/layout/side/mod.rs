mod actions;
mod arrow;
mod boards;
mod colors;
mod delay;
mod drawer;
mod header;
mod pages;
mod presets;
mod section_header;
mod session;
mod settings;
mod sliders;

pub(super) use super::super::events::HitKind;
pub(super) use super::super::format_binding_label;
pub(super) use super::super::hit::HitRegion;
pub(super) use super::spec::ToolbarLayoutSpec;
pub(super) use crate::ui::toolbar::model::{delay_secs_from_t, delay_t_from_ms};
pub(super) use crate::ui::toolbar::snapshot::ToolContext;
pub(super) use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection, ToolbarSnapshot};

/// Populate hit regions for the side toolbar.
#[allow(dead_code)]
pub fn build_side_hits(
    width: f64,
    _height: f64,
    snapshot: &ToolbarSnapshot,
    hits: &mut Vec<HitRegion>,
) {
    let ctx = SideLayoutContext::new(width, snapshot);
    let tool_context = ToolContext::from_snapshot(snapshot);

    header::push_header_hits(&ctx, hits);

    let mut y = ctx.spec.side_content_start_y();

    if snapshot.drawer_open
        && (snapshot.customize_items_open
            || snapshot.drawer_tab == crate::input::ToolbarDrawerTab::Customize
            || snapshot.drawer_tab == crate::input::ToolbarDrawerTab::Session)
    {
        y = drawer::push_drawer_tabs_hits(&ctx, y, hits);
        if snapshot.drawer_tab == crate::input::ToolbarDrawerTab::Session {
            session::push_session_hits(&ctx, y, hits);
        } else {
            settings::push_settings_hits(&ctx, y, hits);
        }
        return;
    }

    let mut drawer_tabs_pushed = false;
    let mut thickness_block_pushed = false;
    let mut text_block_pushed = false;
    for section in crate::ui::toolbar::model::ordered_side_sections(snapshot) {
        match section {
            ToolbarSideSection::Colors
                if tool_context.needs_color
                    && !snapshot.side_section_hidden(ToolbarSideSection::Colors) =>
            {
                y = colors::push_color_picker_hits(&ctx, y, hits);
            }
            ToolbarSideSection::Presets
                if !snapshot.side_section_hidden(ToolbarSideSection::Presets) =>
            {
                y = presets::push_preset_hits(&ctx, y, hits);
            }
            ToolbarSideSection::Thickness
            | ToolbarSideSection::EraserMode
            | ToolbarSideSection::PolygonSides
                if tool_context.needs_thickness && !thickness_block_pushed =>
            {
                thickness_block_pushed = true;
                y = sliders::push_thickness_hits(&ctx, y, hits);
                if tool_context.show_eraser_mode
                    && !snapshot.side_section_hidden(ToolbarSideSection::EraserMode)
                {
                    y = sliders::push_eraser_mode_hits(&ctx, y, hits);
                }
                if tool_context.show_polygon_sides_control
                    && !snapshot.side_section_hidden(ToolbarSideSection::PolygonSides)
                {
                    y = sliders::push_polygon_sides_hits(&ctx, y, hits);
                }
            }
            ToolbarSideSection::ArrowLabels
                if tool_context.show_arrow_labels
                    && !snapshot.side_section_hidden(ToolbarSideSection::ArrowLabels) =>
            {
                y = arrow::push_arrow_section_hits(&ctx, y, hits);
            }
            ToolbarSideSection::StepMarkers
                if tool_context.show_step_counter
                    && !snapshot.side_section_hidden(ToolbarSideSection::StepMarkers) =>
            {
                y = arrow::push_step_marker_hits(&ctx, y, hits);
            }
            ToolbarSideSection::MarkerOpacity
                if tool_context.show_marker_opacity
                    && !snapshot.side_section_hidden(ToolbarSideSection::MarkerOpacity) =>
            {
                y = sliders::push_marker_opacity_hits(&ctx, y, hits);
            }
            ToolbarSideSection::TextSize | ToolbarSideSection::Font
                if tool_context.show_font_controls && !text_block_pushed =>
            {
                text_block_pushed = true;
                y = sliders::push_text_size_hits(&ctx, y, hits);
                y = sliders::push_font_hits(&ctx, y, hits);
            }
            ToolbarSideSection::Actions => {
                y = push_drawer_tabs_hits_once(&ctx, y, hits, &mut drawer_tabs_pushed);
                y = actions::push_actions_hits(&ctx, y, hits);
            }
            ToolbarSideSection::Boards => {
                y = push_drawer_tabs_hits_once(&ctx, y, hits, &mut drawer_tabs_pushed);
                y = boards::push_boards_hits(&ctx, y, hits);
            }
            ToolbarSideSection::Pages => {
                y = push_drawer_tabs_hits_once(&ctx, y, hits, &mut drawer_tabs_pushed);
                y = pages::push_pages_hits(&ctx, y, hits);
            }
            ToolbarSideSection::StepUndo => {
                y = push_drawer_tabs_hits_once(&ctx, y, hits, &mut drawer_tabs_pushed);
                y = delay::push_delay_hits(&ctx, y, hits);
            }
            ToolbarSideSection::Session => {
                y = push_drawer_tabs_hits_once(&ctx, y, hits, &mut drawer_tabs_pushed);
                y = session::push_session_hits(&ctx, y, hits);
            }
            ToolbarSideSection::Settings => {
                y = push_drawer_tabs_hits_once(&ctx, y, hits, &mut drawer_tabs_pushed);
                settings::push_settings_hits(&ctx, y, hits);
            }
            _ => {}
        }
    }
    let _ = push_drawer_tabs_hits_once(&ctx, y, hits, &mut drawer_tabs_pushed);
}

fn push_drawer_tabs_hits_once(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
    pushed: &mut bool,
) -> f64 {
    if *pushed {
        return y;
    }
    *pushed = true;
    drawer::push_drawer_tabs_hits(ctx, y, hits)
}

pub(super) struct SideLayoutContext<'a> {
    pub(super) width: f64,
    pub(super) snapshot: &'a ToolbarSnapshot,
    pub(super) spec: ToolbarLayoutSpec,
    pub(super) x: f64,
    pub(super) content_width: f64,
    pub(super) use_icons: bool,
    pub(super) section_gap: f64,
}

impl<'a> SideLayoutContext<'a> {
    fn new(width: f64, snapshot: &'a ToolbarSnapshot) -> Self {
        let spec = ToolbarLayoutSpec::new(snapshot);
        let use_icons = spec.use_icons();
        let x = ToolbarLayoutSpec::SIDE_START_X;
        let content_width = spec.side_content_width(width);
        let section_gap = ToolbarLayoutSpec::SIDE_SECTION_GAP;
        Self {
            width,
            snapshot,
            spec,
            x,
            content_width,
            use_icons,
            section_gap,
        }
    }
}
