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

    // Color section: only when tool needs color
    if tool_context.needs_color && !snapshot.side_section_hidden(ToolbarSideSection::Colors) {
        y = colors::push_color_picker_hits(&ctx, y, hits);
    }

    if !snapshot.side_section_hidden(ToolbarSideSection::Presets) {
        y = presets::push_preset_hits(&ctx, y, hits);
    }

    // Thickness/size: only when tool needs it
    if tool_context.needs_thickness {
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

    // Arrow section: only for arrow tool
    if tool_context.show_arrow_labels
        && !snapshot.side_section_hidden(ToolbarSideSection::ArrowLabels)
    {
        y = arrow::push_arrow_section_hits(&ctx, y, hits);
    }

    // Step marker counter: only for step marker tool
    if tool_context.show_step_counter
        && !snapshot.side_section_hidden(ToolbarSideSection::StepMarkers)
    {
        y = arrow::push_step_marker_hits(&ctx, y, hits);
    }

    // Marker opacity: only for marker tool
    if tool_context.show_marker_opacity
        && !snapshot.side_section_hidden(ToolbarSideSection::MarkerOpacity)
    {
        y = sliders::push_marker_opacity_hits(&ctx, y, hits);
    }

    // Text controls: only when text/note is active
    if tool_context.show_font_controls {
        if !snapshot.side_section_hidden(ToolbarSideSection::TextSize) {
            y = sliders::push_text_size_hits(&ctx, y, hits);
        }
        if !snapshot.side_section_hidden(ToolbarSideSection::Font) {
            y = sliders::push_font_hits(&ctx, y, hits);
        }
    }

    y = drawer::push_drawer_tabs_hits(&ctx, y, hits);
    y = actions::push_actions_hits(&ctx, y, hits);
    y = boards::push_boards_hits(&ctx, y, hits);
    y = pages::push_pages_hits(&ctx, y, hits);
    y = delay::push_delay_hits(&ctx, y, hits);
    y = session::push_session_hits(&ctx, y, hits);

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
