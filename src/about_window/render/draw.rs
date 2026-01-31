use super::super::{GITHUB_URL, LinkAction, LinkRegion, WEBSITE_URL};
use super::text::draw_text;
use super::widgets::{draw_close_button, draw_copy_button};
use crate::ui_text::UiTextStyle;

pub(super) fn draw_about(
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

    let title_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Bold,
        size: 18.0,
    };
    let body_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 13.0,
    };
    let hint_style = UiTextStyle {
        family: "Sans",
        slant: cairo::FontSlant::Normal,
        weight: cairo::FontWeight::Normal,
        size: 11.0,
    };
    let title = format!("Wayscriber version {}", version());
    draw_text(ctx, title_style, margin, y, &title);

    y += 28.0;

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
        body_style,
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
        body_style,
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
    let commit_rect = draw_text(ctx, body_style, margin, y, &commit_line);
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
    draw_text(
        ctx,
        hint_style,
        margin,
        height - 16.0,
        "Press Esc or click X to close",
    );
}

struct LinkRenderState<'a> {
    hover_index: Option<usize>,
    links: &'a mut Vec<LinkRegion>,
}

#[allow(clippy::too_many_arguments)]
fn add_link_line(
    ctx: &cairo::Context,
    style: UiTextStyle<'_>,
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
    let rect = draw_text(ctx, style, x, y, text);
    ctx.set_line_width(1.0);
    ctx.move_to(rect.0, rect.1 + rect.3 + 2.0);
    ctx.line_to(rect.0 + rect.2, rect.1 + rect.3 + 2.0);
    let _ = ctx.stroke();
    state.links.push(LinkRegion { rect, action });
    y + 22.0
}

fn version() -> &'static str {
    crate::build_info::version()
}

fn commit_hash() -> &'static str {
    env!("WAYSCRIBER_GIT_HASH")
}
