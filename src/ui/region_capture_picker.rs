//! Region selector painting and its numeric layout facade.

mod cut;
mod layout;
mod legend;
mod readout;
mod selection;
mod types;

pub(crate) use layout::{capture_size_text, measure_picker_damage};
pub(crate) use legend::{OCR_LEGEND_TEXT, render_region_legend};
pub(crate) use types::{
    RegionCaptureCutVisual, RegionCaptureLoupeVisual, RegionCapturePickerVisual,
    RegionCaptureWindowVisual, RegionCutDragVisual, RegionCutPreviewVisual,
};

use crate::ui::region_action_bar::{
    RegionActionBar, RegionActionBarVisual, render_region_action_bar,
};
use crate::ui::region_resize_handles::render_region_resize_handles;
use crate::ui_text::UiTextEngine;
use cut::{draw_cut_drag, paint_cut_preview};
use layout::normalized_rect;
use legend::picker_legend_text;
use readout::{READOUT_FONT_SIZE, draw_readout_panel};
use selection::{draw_crosshair, draw_scrim, draw_selection_frame, draw_window_target_frames};

const PANEL_FILL: (f64, f64, f64, f64) = (12.0 / 255.0, 12.0 / 255.0, 15.0 / 255.0, 0.92);
const PANEL_RADIUS: f64 = 6.0;

pub(crate) fn render_region_capture_loupe(
    ctx: &cairo::Context,
    screen: (u32, u32),
    visual: RegionCaptureLoupeVisual,
    sample: impl FnMut(f64, f64) -> Option<crate::draw::Color>,
) {
    let layout = crate::ui::compute_eyedropper_loupe_layout(visual.pointer, screen);
    crate::ui::render_eyedropper_loupe(ctx, layout, visual.image_center, sample);
}

pub(crate) fn render_region_capture_picker(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    screen_width: u32,
    screen_height: u32,
    visual: &RegionCapturePickerVisual<'_>,
    mut sample_loupe: impl FnMut(f64, f64) -> Option<crate::draw::Color>,
) {
    let width = f64::from(screen_width);
    let height = f64::from(screen_height);

    let highlighted_window = visual.window.highlighted_selection();
    let effective_selection = if visual.window.active {
        highlighted_window
    } else {
        visual.selection
    };

    let _ = ctx.save();
    if let Some(preview) = visual.cut.preview {
        paint_cut_preview(ctx, preview);
    }
    if visual.show_scrim {
        draw_scrim(ctx, width, height, effective_selection);
    }

    if visual.window.active {
        draw_window_target_frames(ctx, visual.window);
    }
    if let Some(selection) = effective_selection {
        let (x, y, w, h) = normalized_rect(selection);
        // Corner arms and corner grips would occupy the same pixels, so the
        // frame drops its arms wherever grips are offered.
        draw_selection_frame(ctx, x, y, w, h, visual.resize_handles.is_none());
    }
    if let Some(handles) = visual.resize_handles.as_ref() {
        render_region_resize_handles(ctx, handles, visual.hovered_handle);
    }
    if let Some(drag) = visual.cut.drag {
        draw_cut_drag(ctx, drag);
    }
    if !visual.window.active && !visual.review {
        draw_crosshair(ctx, visual.pointer, (width, height));
    }

    if let Some(measurement) = visual.measurement {
        let anchor = effective_selection
            .filter(|_| visual.review)
            .map(normalized_rect);
        draw_readout_panel(
            engine,
            ctx,
            measurement,
            READOUT_FONT_SIZE,
            visual.pointer,
            anchor,
            visual.action_bar.as_ref().map(RegionActionBar::bounds),
            (screen_width, screen_height),
            cairo::FontWeight::Bold,
        );
    }
    if visual.show_legend && (visual.window.active || visual.selection.is_none()) {
        render_region_legend(
            engine,
            ctx,
            (screen_width, screen_height),
            picker_legend_text(visual.window),
        );
    }
    if let Some(loupe) = visual.loupe {
        render_region_capture_loupe(ctx, (screen_width, screen_height), loupe, &mut sample_loupe);
    }
    if let Some(action_bar) = visual.action_bar.as_ref() {
        render_region_action_bar(
            engine,
            ctx,
            action_bar,
            RegionActionBarVisual {
                hovered: visual.hovered_action,
                include_drawings: visual.include_drawings,
                availability: visual.cut.availability,
                cut_armed: visual.cut.cut_armed,
                status: visual.cut.status,
            },
        );
    }
    let _ = ctx.restore();
}

#[cfg(test)]
mod tests;
