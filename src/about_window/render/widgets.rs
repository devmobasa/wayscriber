//! Painting primitives for the About dialog: rounded surfaces, the close
//! affordance, and the focus ring that makes keyboard navigation visible.

use crate::ui::theme::Rgba;

/// Visual state shared by every interactive element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct InteractionState {
    pub(super) hovered: bool,
    pub(super) focused: bool,
}

impl InteractionState {
    pub(super) fn is_highlighted(self) -> bool {
        self.hovered || self.focused
    }
}

pub(super) fn set_color(ctx: &cairo::Context, color: Rgba) {
    ctx.set_source_rgba(color.0, color.1, color.2, color.3);
}

/// Fill a rounded rectangle.
pub(super) fn fill_rounded_rect(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    radius: f64,
    color: Rgba,
) {
    set_color(ctx, color);
    rounded_rect_path(ctx, rect, radius);
    let _ = ctx.fill();
}

/// Stroke a rounded rectangle on the pixel grid.
pub(super) fn stroke_rounded_rect(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    radius: f64,
    color: Rgba,
    line_width: f64,
) {
    set_color(ctx, color);
    ctx.set_line_width(line_width);
    let inset = line_width / 2.0;
    rounded_rect_path(
        ctx,
        (
            rect.0 + inset,
            rect.1 + inset,
            rect.2 - line_width,
            rect.3 - line_width,
        ),
        (radius - inset).max(0.0),
    );
    let _ = ctx.stroke();
}

/// Interactive row/card background: a tint on hover, plus a focus ring when
/// the element is keyboard-selected.
pub(super) fn draw_surface(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    radius: f64,
    state: InteractionState,
    background: Rgba,
    hover_background: Rgba,
    accent: Rgba,
) {
    let fill = if state.hovered {
        hover_background
    } else {
        background
    };
    fill_rounded_rect(ctx, rect, radius, fill);
    if state.focused {
        stroke_rounded_rect(ctx, rect, radius, accent, 1.5);
    }
}

/// The close affordance: a hairline square with an X, tinted red on hover.
pub(super) fn draw_close_button(
    ctx: &cairo::Context,
    rect: (f64, f64, f64, f64),
    state: InteractionState,
    foreground: Rgba,
    destructive: Rgba,
) {
    let (x, y, size, _) = rect;
    let color = if state.is_highlighted() {
        destructive
    } else {
        foreground
    };

    if state.hovered {
        fill_rounded_rect(
            ctx,
            rect,
            4.0,
            (destructive.0, destructive.1, destructive.2, 0.18),
        );
    }
    if state.focused {
        stroke_rounded_rect(ctx, rect, 4.0, color, 1.5);
    }

    set_color(ctx, color);
    ctx.set_line_width(1.6);
    let inset = size * 0.3;
    ctx.move_to(x + inset, y + inset);
    ctx.line_to(x + size - inset, y + size - inset);
    let _ = ctx.stroke();
    ctx.move_to(x + size - inset, y + inset);
    ctx.line_to(x + inset, y + size - inset);
    let _ = ctx.stroke();
}

/// A right-pointing chevron marking "this row opens something".
pub(super) fn draw_chevron(ctx: &cairo::Context, x: f64, center_y: f64, size: f64, color: Rgba) {
    set_color(ctx, color);
    ctx.set_line_width(1.4);
    ctx.move_to(x, center_y - size / 2.0);
    ctx.line_to(x + size / 2.0, center_y);
    ctx.line_to(x, center_y + size / 2.0);
    let _ = ctx.stroke();
}

/// A filled status dot; the update card uses it as an at-a-glance signal.
pub(super) fn draw_status_dot(ctx: &cairo::Context, center: (f64, f64), radius: f64, color: Rgba) {
    set_color(ctx, color);
    ctx.arc(center.0, center.1, radius, 0.0, std::f64::consts::TAU);
    let _ = ctx.fill();
}

/// Paint an icon surface scaled into `rect`.
pub(super) fn draw_icon(
    ctx: &cairo::Context,
    icon: &cairo::ImageSurface,
    rect: (f64, f64, f64, f64),
) {
    let (width, height) = (icon.width() as f64, icon.height() as f64);
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    let _ = ctx.save();
    ctx.translate(rect.0, rect.1);
    ctx.scale(rect.2 / width, rect.3 / height);
    let _ = ctx.set_source_surface(icon, 0.0, 0.0);
    let _ = ctx.paint();
    let _ = ctx.restore();
}

fn rounded_rect_path(ctx: &cairo::Context, rect: (f64, f64, f64, f64), radius: f64) {
    let (x, y, width, height) = rect;
    let r = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    ctx.new_sub_path();
    ctx.arc(x + width - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.arc(
        x + width - r,
        y + height - r,
        r,
        0.0,
        std::f64::consts::FRAC_PI_2,
    );
    ctx.arc(
        x + r,
        y + height - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    ctx.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    ctx.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> cairo::Context {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 64, 64).unwrap();
        cairo::Context::new(&surface).unwrap()
    }

    #[test]
    fn highlight_covers_hover_and_focus() {
        assert!(!InteractionState::default().is_highlighted());
        assert!(
            InteractionState {
                hovered: true,
                focused: false
            }
            .is_highlighted()
        );
        assert!(
            InteractionState {
                hovered: false,
                focused: true
            }
            .is_highlighted()
        );
    }

    #[test]
    fn degenerate_rectangles_do_not_panic() {
        let ctx = context();

        fill_rounded_rect(&ctx, (0.0, 0.0, 0.0, 0.0), 8.0, (1.0, 1.0, 1.0, 1.0));
        stroke_rounded_rect(&ctx, (0.0, 0.0, 1.0, 1.0), 8.0, (1.0, 1.0, 1.0, 1.0), 2.0);
        draw_chevron(&ctx, 0.0, 0.0, 0.0, (1.0, 1.0, 1.0, 1.0));

        assert_eq!(ctx.status(), Ok(()));
    }

    #[test]
    fn interactive_surface_paints_in_every_state() {
        let ctx = context();
        let states = [
            InteractionState::default(),
            InteractionState {
                hovered: true,
                focused: false,
            },
            InteractionState {
                hovered: false,
                focused: true,
            },
            InteractionState {
                hovered: true,
                focused: true,
            },
        ];

        for state in states {
            draw_surface(
                &ctx,
                (4.0, 4.0, 40.0, 20.0),
                8.0,
                state,
                (0.1, 0.1, 0.1, 1.0),
                (0.2, 0.2, 0.2, 1.0),
                (0.2, 0.5, 0.9, 1.0),
            );
            draw_close_button(
                &ctx,
                (40.0, 4.0, 16.0, 16.0),
                state,
                (1.0, 1.0, 1.0, 0.9),
                (0.9, 0.2, 0.2, 1.0),
            );
        }

        assert_eq!(ctx.status(), Ok(()));
    }

    #[test]
    fn icon_painting_tolerates_scaling() {
        let ctx = context();
        let icon = cairo::ImageSurface::create(cairo::Format::ARgb32, 32, 32).unwrap();

        draw_icon(&ctx, &icon, (2.0, 2.0, 44.0, 44.0));

        assert_eq!(ctx.status(), Ok(()));
    }
}
