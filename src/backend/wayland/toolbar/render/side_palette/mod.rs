mod actions;
mod arrow;
mod boards;
mod colors;
mod header;
mod marker;
mod pages;
mod presets;
mod section_header;
pub(in crate::backend::wayland::toolbar) mod session;
mod settings;
mod step;
mod step_marker;
mod text;
mod thickness;

use anyhow::Result;

use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::toolbar::model;
use crate::ui::toolbar::snapshot::ToolContext;
use crate::ui::toolbar::{SidePane, ToolbarEvent, ToolbarSideSection, ToolbarSnapshot};

use std::time::Instant;

use super::widgets::constants::set_color;
use super::widgets::{draw_panel_background, draw_round_rect, draw_tooltip_with_delay};
use crate::ui::theme::Rgba;

/// Scrollbar track tint: a white wash with no theme token (value-coincides
/// with COLOR_DIVIDER but is a control surface, not a separator). The thumb
/// shares the GTK scrollbar slider token. Shared with the top strip's
/// Canvas/Session/Settings popover scrollbar so both scroll surfaces match.
pub(in crate::backend::wayland::toolbar) const COLOR_SCROLLBAR_TRACK: Rgba = (1.0, 1.0, 1.0, 0.08);
use crate::ui::theme::toolbar::COLOR_SCROLLBAR_SLIDER as COLOR_SCROLLBAR_THUMB;

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

/// Button rects for a Boards/Pages-style command row. Icon mode packs
/// plain buttons left as squares and right-aligns destructive ones with
/// the leftover width as a guard gap; text mode keeps the equal-width
/// grid so labels stay readable.
pub(super) struct SideRowLayout {
    pub(super) x: f64,
    pub(super) row_y: f64,
    pub(super) content_width: f64,
    pub(super) btn_h: f64,
    pub(super) btn_gap: f64,
    pub(super) use_icons: bool,
    pub(super) text_columns: usize,
}

pub(super) fn side_row_button_rects(
    layout: SideRowLayout,
    buttons: &[model::ToolbarButtonModel],
) -> Vec<(f64, f64, f64, f64)> {
    use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
    let SideRowLayout {
        x,
        row_y,
        content_width,
        btn_h,
        btn_gap,
        use_icons,
        text_columns,
    } = layout;
    if use_icons {
        let mut leading = 0usize;
        let mut trailing = 0usize;
        buttons
            .iter()
            .map(|button| {
                if button.event.is_destructive() {
                    trailing += 1;
                    let bx = x + content_width
                        - trailing as f64 * btn_h
                        - (trailing as f64 - 1.0) * btn_gap;
                    (bx, row_y, btn_h, btn_h)
                } else {
                    let bx = x + leading as f64 * (btn_h + btn_gap);
                    leading += 1;
                    (bx, row_y, btn_h, btn_h)
                }
            })
            .collect()
    } else {
        let btn_w = row_item_width(content_width, text_columns, btn_gap);
        grid_layout(
            x,
            row_y,
            btn_w,
            btn_h,
            btn_gap,
            btn_gap,
            buttons.len(),
            text_columns,
        )
        .items
        .iter()
        .map(|item| (item.x, item.y, item.w, item.h))
        .collect()
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
    draw_panel_background(ctx, 0.0, 0.0, width, height);

    if snapshot.side_minimized {
        draw_side_minimized_tab(ctx, width, height, hits, hover, hover_start);
        return Ok(());
    }

    let mut layout = SidePaletteLayout::new(ctx, width, snapshot, hits, hover);

    let content_top = header::draw_header(&mut layout);

    // Scroll bookkeeping: when the pane's natural height exceeds the
    // surface, content below the fixed chrome is clipped and translated.
    let natural = layout.spec.side_natural_height(snapshot);
    let max_scroll = (natural - height).max(0.0);
    let scroll = if max_scroll > 0.0 {
        snapshot.side_scroll.clamp(0.0, max_scroll)
    } else {
        0.0
    };
    let scrolling = max_scroll > 0.0;

    let chrome_hits = layout.hits.len();
    if scrolling {
        let _ = ctx.save();
        ctx.rectangle(0.0, content_top, width, height - content_top);
        ctx.clip();
        ctx.translate(0.0, -scroll);
        // Content is laid out in unscrolled coordinates; hover must be
        // compared in the same space.
        layout.hover = layout.hover.map(|(hx, hy)| {
            (
                hx,
                if hy >= content_top {
                    hy + scroll
                } else {
                    f64::MIN
                },
            )
        });
    }

    let mut y = content_top;
    draw_active_pane(&mut layout, &mut y);

    if scrolling {
        let _ = ctx.restore();
        // Shift pane-content hits into visible coordinates, then clip every
        // region to the scroll viewport so partial rows cannot remain active
        // through the fixed header or below the surface.
        for hit in &mut layout.hits[chrome_hits..] {
            hit.rect.1 -= scroll;
        }
        crate::backend::wayland::toolbar::hit::clip_hit_regions_to_bounds(
            layout.hits,
            chrome_hits,
            (0.0, content_top, width, (height - content_top).max(0.0)),
        );
        layout.hover = hover;
        draw_scrollbar(&mut layout, content_top, height, natural, scroll);
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

fn draw_active_pane(layout: &mut SidePaletteLayout<'_>, y: &mut f64) {
    let snapshot = layout.snapshot;
    match snapshot.active_side_pane {
        SidePane::Session => {
            session::draw_session_section(layout, y);
        }
        SidePane::Settings => {
            settings::draw_settings_section(layout, y);
        }
        SidePane::Draw | SidePane::Canvas => {
            draw_pane_sections(layout, y);
        }
    }
}

fn draw_pane_sections(layout: &mut SidePaletteLayout<'_>, y: &mut f64) {
    let snapshot = layout.snapshot;
    let tool_context = ToolContext::from_snapshot(snapshot);
    let mut colors_info = None;
    let mut hover_preset_color = None;
    let mut thickness_block_drawn = false;
    let mut text_block_drawn = false;
    for section in model::ordered_pane_sections(snapshot) {
        match section {
            ToolbarSideSection::Colors
                if tool_context.needs_color
                    && !snapshot.side_section_hidden(ToolbarSideSection::Colors) =>
            {
                colors_info = colors::draw_colors_section(layout, y);
            }
            ToolbarSideSection::Presets
                if !snapshot.side_section_hidden(ToolbarSideSection::Presets) =>
            {
                hover_preset_color = presets::draw_presets_section(layout, y);
            }
            ToolbarSideSection::Thickness
            | ToolbarSideSection::EraserMode
            | ToolbarSideSection::PolygonSides
                if tool_context.needs_thickness && !thickness_block_drawn =>
            {
                thickness_block_drawn = true;
                thickness::draw_thickness_section(layout, y);
            }
            ToolbarSideSection::ArrowLabels
                if tool_context.show_arrow_labels
                    && !snapshot.side_section_hidden(ToolbarSideSection::ArrowLabels) =>
            {
                arrow::draw_arrow_section(layout, y);
            }
            ToolbarSideSection::StepMarkers
                if tool_context.show_step_counter
                    && !snapshot.side_section_hidden(ToolbarSideSection::StepMarkers) =>
            {
                step_marker::draw_step_marker_section(layout, y);
            }
            ToolbarSideSection::MarkerOpacity
                if tool_context.show_marker_opacity
                    && !snapshot.side_section_hidden(ToolbarSideSection::MarkerOpacity) =>
            {
                marker::draw_marker_opacity_section(layout, y);
            }
            ToolbarSideSection::TextSize | ToolbarSideSection::Font
                if tool_context.show_font_controls && !text_block_drawn =>
            {
                text_block_drawn = true;
                text::draw_text_controls_section(layout, y);
            }
            ToolbarSideSection::Actions => {
                actions::draw_actions_section(layout, y);
            }
            ToolbarSideSection::Boards => {
                boards::draw_boards_section(layout, y);
            }
            ToolbarSideSection::Pages => {
                pages::draw_pages_section(layout, y);
            }
            ToolbarSideSection::StepUndo => {
                step::draw_step_section(layout, y);
            }
            _ => {}
        }
    }
    if let (Some(color), Some(info)) = (hover_preset_color, &colors_info) {
        colors::draw_preset_hover_highlight(layout, info, color);
    }
}

/// Minimized side palette: the whole tab is one restore button. Not an
/// item id on purpose — the way back must not be hideable.
fn draw_side_minimized_tab(
    ctx: &cairo::Context,
    width: f64,
    height: f64,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    hover_start: Option<Instant>,
) {
    let is_hover = hover
        .map(|(hx, hy)| super::widgets::point_in_rect(hx, hy, 0.0, 0.0, width, height))
        .unwrap_or(false);
    super::widgets::set_icon_color(ctx, is_hover);
    let icon = (width * 0.75).min(18.0);
    crate::toolbar_icons::draw_icon_chevron_right(
        ctx,
        (width - icon) / 2.0,
        (height - icon) / 2.0,
        icon,
    );
    hits.push(HitRegion {
        focus_id: None,
        rect: (0.0, 0.0, width, height),
        event: ToolbarEvent::SetSideMinimized(false),
        kind: HitKind::Click,
        tooltip: Some("Show toolbar".to_string()),
    });
    draw_tooltip_with_delay(ctx, hits, hover, width, height, false, hover_start);
}

/// Minimal scrollbar: proportional thumb on the right edge; the whole track
/// is draggable (stylus and touch have no wheel).
fn draw_scrollbar(
    layout: &mut SidePaletteLayout<'_>,
    content_top: f64,
    height: f64,
    natural: f64,
    scroll: f64,
) {
    let ctx = layout.ctx;
    let track_w = 4.0;
    let track_x = layout.width - track_w - 3.0;
    let track_y = content_top + 2.0;
    let track_h = height - content_top - 4.0;
    if track_h <= 20.0 {
        return;
    }

    set_color(ctx, COLOR_SCROLLBAR_TRACK);
    draw_round_rect(ctx, track_x, track_y, track_w, track_h, 2.0);
    let _ = ctx.fill();

    let viewport = height - content_top;
    let thumb_h = (track_h * (viewport / natural)).max(20.0);
    let max_scroll = natural - height;
    let thumb_y = track_y + (track_h - thumb_h) * (scroll / max_scroll).clamp(0.0, 1.0);
    set_color(ctx, COLOR_SCROLLBAR_THUMB);
    draw_round_rect(ctx, track_x, thumb_y, track_w, thumb_h, 2.0);
    let _ = ctx.fill();

    layout.hits.push(HitRegion {
        focus_id: None,
        rect: (track_x - 8.0, track_y, track_w + 11.0, track_h),
        event: ToolbarEvent::ScrollSidePane(scroll),
        kind: HitKind::DragScrollSide { max_scroll },
        tooltip: None,
    });
}
