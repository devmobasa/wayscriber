use crate::draw::Color;

const PIXELS: i32 = 11;
const CELL: f64 = 8.0;
const GAP: f64 = 18.0;
const LABEL_HEIGHT: f64 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EyedropperLoupeLayout {
    pub panel_x: f64,
    pub panel_y: f64,
    pub panel_width: f64,
    pub panel_height: f64,
    pub grid_x: f64,
    pub grid_y: f64,
    pub grid_size: f64,
}

pub(crate) fn compute_eyedropper_loupe_layout(
    pointer: (f64, f64),
    screen: (u32, u32),
) -> EyedropperLoupeLayout {
    let grid_size = f64::from(PIXELS) * CELL;
    let panel_width = grid_size + 12.0;
    let panel_height = grid_size + LABEL_HEIGHT + 12.0;
    let mut panel_x = pointer.0 + GAP;
    let mut panel_y = pointer.1 + GAP;
    if panel_x + panel_width > f64::from(screen.0) {
        panel_x = pointer.0 - GAP - panel_width;
    }
    if panel_y + panel_height > f64::from(screen.1) {
        panel_y = pointer.1 - GAP - panel_height;
    }
    panel_x = panel_x.clamp(4.0, (f64::from(screen.0) - panel_width - 4.0).max(4.0));
    panel_y = panel_y.clamp(4.0, (f64::from(screen.1) - panel_height - 4.0).max(4.0));
    EyedropperLoupeLayout {
        panel_x,
        panel_y,
        panel_width,
        panel_height,
        grid_x: panel_x + 6.0,
        grid_y: panel_y + 6.0,
        grid_size,
    }
}

pub(crate) fn render_eyedropper_loupe(
    ctx: &cairo::Context,
    layout: EyedropperLoupeLayout,
    image_center: (f64, f64),
    mut sample: impl FnMut(f64, f64) -> Option<Color>,
) {
    let _ = ctx.save();
    ctx.set_source_rgba(0.04, 0.05, 0.07, 0.96);
    ctx.rectangle(
        layout.panel_x,
        layout.panel_y,
        layout.panel_width,
        layout.panel_height,
    );
    let _ = ctx.fill();

    let center_x = image_center.0.floor();
    let center_y = image_center.1.floor();
    let half = PIXELS / 2;
    for row in 0..PIXELS {
        for col in 0..PIXELS {
            if let Some(color) = sample(
                center_x + f64::from(col - half),
                center_y + f64::from(row - half),
            ) {
                ctx.set_source_rgb(color.r, color.g, color.b);
                ctx.rectangle(
                    layout.grid_x + f64::from(col) * CELL,
                    layout.grid_y + f64::from(row) * CELL,
                    CELL,
                    CELL,
                );
                let _ = ctx.fill();
            }
        }
    }

    let center = f64::from(half) * CELL;
    ctx.set_source_rgb(1.0, 1.0, 1.0);
    ctx.set_line_width(2.0);
    ctx.rectangle(layout.grid_x + center, layout.grid_y + center, CELL, CELL);
    let _ = ctx.stroke();
    ctx.set_source_rgb(0.0, 0.0, 0.0);
    ctx.set_line_width(1.0);
    ctx.rectangle(
        layout.grid_x + center + 2.0,
        layout.grid_y + center + 2.0,
        CELL - 4.0,
        CELL - 4.0,
    );
    let _ = ctx.stroke();

    if let Some(color) = sample(center_x, center_y) {
        let hex = format!(
            "#{:02X}{:02X}{:02X}",
            (color.r * 255.0).round() as u8,
            (color.g * 255.0).round() as u8,
            (color.b * 255.0).round() as u8
        );
        let swatch = 14.0;
        let label_y = layout.grid_y + layout.grid_size + 5.0;
        ctx.set_source_rgb(color.r, color.g, color.b);
        ctx.rectangle(layout.grid_x, label_y, swatch, swatch);
        let _ = ctx.fill();
        ctx.set_source_rgb(1.0, 1.0, 1.0);
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(12.0);
        ctx.move_to(layout.grid_x + swatch + 7.0, label_y + 12.0);
        let _ = ctx.show_text(&hex);
    }

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.65);
    ctx.set_line_width(1.0);
    ctx.rectangle(
        layout.panel_x + 0.5,
        layout.panel_y + 0.5,
        layout.panel_width - 1.0,
        layout.panel_height - 1.0,
    );
    let _ = ctx.stroke();
    let _ = ctx.restore();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_prefers_below_right_then_flips_and_clamps() {
        assert_eq!(
            compute_eyedropper_loupe_layout((20.0, 30.0), (400, 400)),
            EyedropperLoupeLayout {
                panel_x: 38.0,
                panel_y: 48.0,
                panel_width: 100.0,
                panel_height: 124.0,
                grid_x: 44.0,
                grid_y: 54.0,
                grid_size: 88.0,
            }
        );
        assert_eq!(
            compute_eyedropper_loupe_layout((390.0, 390.0), (400, 400)),
            EyedropperLoupeLayout {
                panel_x: 272.0,
                panel_y: 248.0,
                panel_width: 100.0,
                panel_height: 124.0,
                grid_x: 278.0,
                grid_y: 254.0,
                grid_size: 88.0,
            }
        );
        assert_eq!(
            compute_eyedropper_loupe_layout((40.0, 40.0), (80, 80)).panel_x,
            4.0
        );
    }
}
