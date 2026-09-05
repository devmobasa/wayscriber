use crate::toolbar_icons::ToolbarIconPainter;
use crate::ui::primitives::{draw_keycap_with_engine, keycap_size_with_engine};
use crate::ui::theme::{self, Rgba, overlay, toolbar};
use crate::ui_text::{UiTextEngine, UiTextStyle};

/// Vertical lift of the wedge label when it has no glyph but a keycap hint
/// is shown below it.
const HINT_LABEL_LIFT: f64 = 6.0;
/// Drop of the keycap hint's top edge below the wedge midpoint when the
/// wedge has no glyph.
const HINT_LABEL_DROP: f64 = 8.0;

/// Draw a wedge's content stack centered on the wedge midpoint: glyph above
/// a short label with the primary bound shortcut as a keycap below, falling
/// back to label-only layouts when the glyph or hint is missing. With
/// `paint_hint` false the keycap is left to the layer below (hover repaints
/// only the color-dependent glyph/label); the hint still shapes the layout
/// so both layers agree on positions.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_wedge_content(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    label: &str,
    icon: Option<ToolbarIconPainter>,
    hint: Option<&str>,
    color: Rgba,
    label_size: f64,
    paint_hint: bool,
) {
    match icon {
        Some(paint) => {
            let size = overlay::RADIAL_WEDGE_ICON_SIZE;
            theme::set_color(ctx, color);
            paint(
                ctx,
                x - size / 2.0,
                y - overlay::RADIAL_WEDGE_ICON_LIFT - size / 2.0,
                size,
            );
            draw_centered_label(
                engine,
                ctx,
                x,
                y + overlay::RADIAL_WEDGE_LABEL_DROP,
                label,
                label_size,
                color,
            );
            if paint_hint && let Some(hint) = hint {
                draw_hint_keycap(engine, ctx, x, y + overlay::RADIAL_WEDGE_HINT_DROP, hint);
            }
        }
        None => match hint {
            Some(hint) => {
                draw_centered_label(
                    engine,
                    ctx,
                    x,
                    y - HINT_LABEL_LIFT,
                    label,
                    label_size,
                    color,
                );
                if paint_hint {
                    draw_hint_keycap(engine, ctx, x, y + HINT_LABEL_DROP, hint);
                }
            }
            None => draw_centered_label(engine, ctx, x, y, label, label_size, color),
        },
    }
}

/// Draw a keycap hint horizontally centered on `center_x` with its top edge
/// at `top_y`, in the shared keycap language.
fn draw_hint_keycap(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    center_x: f64,
    top_y: f64,
    label: &str,
) {
    let (width, _height) =
        keycap_size_with_engine(engine, ctx, label, toolbar::FONT_SIZE_SWATCH_KEY);
    draw_keycap_with_engine(
        engine,
        ctx,
        center_x - width / 2.0,
        top_y,
        label,
        toolbar::FONT_SIZE_SWATCH_KEY,
        toolbar::COLOR_BADGE_BACKGROUND,
        toolbar::COLOR_BADGE_TEXT,
    );
}

/// Draw a centered text label at the given position.
pub(super) fn draw_centered_label(
    engine: &UiTextEngine,
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    text: &str,
    size: f64,
    color: Rgba,
) {
    let style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size,
    };
    let layout = engine.layout(ctx, style, text, None);
    let extents = layout.ink_extents();
    let tx = x - extents.width() / 2.0 - extents.x_bearing();
    let ty = y - extents.height() / 2.0 - extents.y_bearing();
    theme::set_color(ctx, color);
    layout.show_at_baseline(ctx, tx, ty);
}
