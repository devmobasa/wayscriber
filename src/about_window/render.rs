use anyhow::{Context, Result};
use smithay_client_toolkit::shell::WaylandSurface;

use super::{AboutWindowState, GITHUB_URL, LinkAction, LinkRegion, WEBSITE_URL};

impl AboutWindowState {
    pub(super) fn render(&mut self) -> Result<()> {
        if !self.configured {
            return Ok(());
        }

        let (phys_w, phys_h) = (
            self.width.saturating_mul(self.scale as u32),
            self.height.saturating_mul(self.scale as u32),
        );

        if self.pool.is_none() {
            let buffer_size = (phys_w * phys_h * 4) as usize;
            let pool = super::SlotPool::new(buffer_size, &self.shm)
                .context("Failed to create about window buffer pool")?;
            self.pool = Some(pool);
        }

        let pool = match self.pool.as_mut() {
            Some(pool) => pool,
            None => return Ok(()),
        };
        let (buffer, canvas) = pool
            .create_buffer(
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
                wayland_client::protocol::wl_shm::Format::Argb8888,
            )
            .context("Failed to create about window buffer")?;

        let surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                canvas.as_mut_ptr(),
                cairo::Format::ARgb32,
                phys_w as i32,
                phys_h as i32,
                (phys_w * 4) as i32,
            )
        }
        .context("Failed to create Cairo surface")?;
        let ctx = cairo::Context::new(&surface).context("Failed to create Cairo context")?;

        ctx.set_operator(cairo::Operator::Clear);
        let _ = ctx.paint();
        ctx.set_operator(cairo::Operator::Over);
        if self.scale > 1 {
            ctx.scale(self.scale as f64, self.scale as f64);
        }

        self.link_regions.clear();
        draw_about(
            &ctx,
            self.width as f64,
            self.height as f64,
            &mut self.link_regions,
            self.hover_index,
        );

        surface.flush();

        let wl_surface = self.window.wl_surface();
        wl_surface.set_buffer_scale(self.scale);
        wl_surface.attach(Some(buffer.wl_buffer()), 0, 0);
        wl_surface.damage_buffer(0, 0, phys_w as i32, phys_h as i32);
        wl_surface.commit();

        self.needs_redraw = false;
        Ok(())
    }
}

fn draw_about(
    ctx: &cairo::Context,
    width: f64,
    height: f64,
    links: &mut Vec<LinkRegion>,
    hover_index: Option<usize>,
) {
    let margin = 22.0;
    let mut y = margin + 20.0;

    ctx.set_source_rgb(0.96, 0.95, 0.93);
    ctx.rectangle(0.0, 0.0, width, height);
    let _ = ctx.fill();

    ctx.set_source_rgb(0.18, 0.18, 0.18);
    ctx.set_line_width(1.0);
    ctx.rectangle(0.5, 0.5, width - 1.0, height - 1.0);
    let _ = ctx.stroke();

    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    ctx.set_font_size(18.0);
    let title = format!("Wayscriber version {}", version());
    draw_text(ctx, margin, y, &title);

    y += 28.0;
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(13.0);

    let mut link_index = 0usize;
    let close_size = 16.0;
    let close_padding = 10.0;
    let close_x = width - close_padding - close_size;
    let close_y = close_padding;
    let close_rect = draw_close_button(
        ctx,
        close_x,
        close_y,
        close_size,
        hover_index == Some(link_index),
    );
    links.push(LinkRegion {
        rect: close_rect,
        action: LinkAction::Close,
    });
    link_index += 1;

    y = add_link_line(
        ctx,
        margin,
        y,
        &format!("Website: {}", WEBSITE_URL.trim_start_matches("https://")),
        LinkAction::OpenUrl(WEBSITE_URL.to_string()),
        link_index,
        &mut LinkRenderState { hover_index, links },
    );
    link_index += 1;

    y = add_link_line(
        ctx,
        margin,
        y,
        "GitHub: github.com/devmobasa/wayscriber",
        LinkAction::OpenUrl(GITHUB_URL.to_string()),
        link_index,
        &mut LinkRenderState { hover_index, links },
    );
    link_index += 1;

    let commit = commit_hash();
    let commit_line = format!("Commit: {}", commit);
    ctx.set_source_rgb(0.25, 0.25, 0.25);
    let commit_rect = draw_text(ctx, margin, y, &commit_line);
    if commit != "unknown" {
        let button_size = 14.0;
        let text_right = commit_rect.0 + commit_rect.2;
        let button_x = text_right + 8.0;
        let button_y = commit_rect.1 + (commit_rect.3 - button_size) / 2.0;
        let rect = draw_copy_button(
            ctx,
            button_x,
            button_y,
            button_size,
            hover_index == Some(link_index),
        );
        links.push(LinkRegion {
            rect,
            action: LinkAction::CopyText(commit.to_string()),
        });
    }

    ctx.set_source_rgb(0.4, 0.4, 0.4);
    ctx.set_font_size(11.0);
    draw_text(ctx, margin, height - 16.0, "Press Esc or click X to close");
}

struct LinkRenderState<'a> {
    hover_index: Option<usize>,
    links: &'a mut Vec<LinkRegion>,
}

fn add_link_line(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    text: &str,
    action: LinkAction,
    index: usize,
    state: &mut LinkRenderState<'_>,
) -> f64 {
    let is_hover = state.hover_index == Some(index);
    if is_hover {
        ctx.set_source_rgb(0.08, 0.38, 0.75);
    } else {
        ctx.set_source_rgb(0.12, 0.45, 0.84);
    }
    let rect = draw_text(ctx, x, y, text);
    ctx.set_line_width(1.0);
    ctx.move_to(rect.0, rect.1 + rect.3 + 2.0);
    ctx.line_to(rect.0 + rect.2, rect.1 + rect.3 + 2.0);
    let _ = ctx.stroke();
    state.links.push(LinkRegion { rect, action });
    y + 22.0
}

fn draw_text(ctx: &cairo::Context, x: f64, y: f64, text: &str) -> (f64, f64, f64, f64) {
    ctx.move_to(x, y);
    let _ = ctx.show_text(text);
    let extents = match ctx.text_extents(text) {
        Ok(extents) => extents,
        Err(_) => fallback_text_extents(ctx, text),
    };
    (
        x + extents.x_bearing(),
        y + extents.y_bearing(),
        extents.width(),
        extents.height(),
    )
}

fn fallback_text_extents(ctx: &cairo::Context, text: &str) -> cairo::TextExtents {
    let height = ctx
        .font_extents()
        .map(|extents| extents.height())
        .unwrap_or(14.0);
    let width = text.len() as f64 * height * 0.5;
    cairo::TextExtents::new(0.0, -height, width, height, width, 0.0)
}

fn draw_copy_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) -> (f64, f64, f64, f64) {
    let radius = 3.0;
    let (bg_r, bg_g, bg_b) = if hover {
        (0.85, 0.9, 0.98)
    } else {
        (0.92, 0.94, 0.96)
    };
    ctx.set_source_rgb(bg_r, bg_g, bg_b);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.fill();

    ctx.set_source_rgb(0.55, 0.6, 0.68);
    ctx.set_line_width(1.0);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.stroke();

    let pad = 3.0;
    let icon_size = size - pad * 2.0;
    let back = (x + pad + 2.0, y + pad - 1.0);
    let front = (x + pad - 1.0, y + pad + 2.0);
    ctx.set_source_rgb(0.35, 0.4, 0.48);
    draw_rounded_rect(ctx, back.0, back.1, icon_size, icon_size, 2.0);
    let _ = ctx.stroke();
    draw_rounded_rect(ctx, front.0, front.1, icon_size, icon_size, 2.0);
    let _ = ctx.stroke();

    (x, y, size, size)
}

fn draw_close_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) -> (f64, f64, f64, f64) {
    let radius = 3.0;
    let (bg_r, bg_g, bg_b) = if hover {
        (0.98, 0.88, 0.88)
    } else {
        (0.96, 0.92, 0.92)
    };
    ctx.set_source_rgb(bg_r, bg_g, bg_b);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.fill();

    ctx.set_source_rgb(0.7, 0.55, 0.55);
    ctx.set_line_width(1.0);
    draw_rounded_rect(ctx, x, y, size, size, radius);
    let _ = ctx.stroke();

    ctx.set_source_rgb(0.4, 0.25, 0.25);
    ctx.set_line_width(1.6);
    let inset = 4.0;
    ctx.move_to(x + inset, y + inset);
    ctx.line_to(x + size - inset, y + size - inset);
    let _ = ctx.stroke();
    ctx.move_to(x + size - inset, y + inset);
    ctx.line_to(x + inset, y + size - inset);
    let _ = ctx.stroke();

    (x, y, size, size)
}

fn draw_rounded_rect(ctx: &cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let r = radius.min(width / 2.0).min(height / 2.0);
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

fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn commit_hash() -> &'static str {
    env!("WAYSCRIBER_GIT_HASH")
}
