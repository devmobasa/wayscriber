use crate::config::action_meta::ActionMeta;
use crate::input::InputState;
use crate::input::state::COMMAND_PALETTE_ITEM_HEIGHT;
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::super::constants::TEXT_DESCRIPTION;
use super::super::primitives::{draw_rounded_rect, text_extents_for};
use super::{
    COMMAND_PALETTE_FONT_FAMILY, COMMAND_PALETTE_SHORTCUT_BADGE_GAP,
    COMMAND_PALETTE_SHORTCUT_BADGE_HEIGHT, COMMAND_PALETTE_SHORTCUT_BADGE_PADDING_X,
    COMMAND_PALETTE_SHORTCUT_MIN_DESC_WIDTH, ellipsize_to_width,
};

pub(super) struct CommandPaletteRowStyle {
    pub(super) label: UiTextStyle<'static>,
    pub(super) desc: UiTextStyle<'static>,
    pub(super) shortcut: UiTextStyle<'static>,
}

pub(super) fn command_palette_row_styles() -> CommandPaletteRowStyle {
    CommandPaletteRowStyle {
        label: super::command_palette_text_style(
            super::COMMAND_PALETTE_LABEL_TEXT_SIZE,
            cairo::FontWeight::Normal,
            cairo::FontSlant::Normal,
        ),
        desc: super::command_palette_text_style(
            super::COMMAND_PALETTE_DESC_TEXT_SIZE,
            cairo::FontWeight::Normal,
            cairo::FontSlant::Normal,
        ),
        shortcut: super::command_palette_text_style(
            super::COMMAND_PALETTE_SHORTCUT_TEXT_SIZE,
            cairo::FontWeight::Normal,
            cairo::FontSlant::Normal,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_command_row(
    ctx: &cairo::Context,
    input_state: &InputState,
    cmd: &ActionMeta,
    styles: &CommandPaletteRowStyle,
    inner_x: f64,
    inner_width: f64,
    item_y: f64,
    is_selected: bool,
) {
    if is_selected {
        draw_rounded_rect(
            ctx,
            inner_x,
            item_y,
            inner_width,
            COMMAND_PALETTE_ITEM_HEIGHT - 2.0,
            super::super::constants::RADIUS_SM,
        );
        super::super::constants::set_color(ctx, super::super::constants::BG_INPUT_SELECTION);
        let _ = ctx.fill();
    }

    let text_alpha = if is_selected { 1.0 } else { 0.85 };
    let label_y = item_y + COMMAND_PALETTE_ITEM_HEIGHT / 2.0 + styles.label.size / 3.0;
    ctx.set_source_rgba(
        super::super::constants::TEXT_WHITE.0,
        super::super::constants::TEXT_WHITE.1,
        super::super::constants::TEXT_WHITE.2,
        text_alpha,
    );
    render_command_row_label(ctx, cmd.label, inner_x + 10.0, label_y, styles);

    let label_extents = text_extents_for(
        ctx,
        COMMAND_PALETTE_FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        styles.label.size,
        cmd.label,
    );
    let desc_x = inner_x + 10.0 + label_extents.width() + 12.0;
    let content_right = inner_x + inner_width - 8.0;

    let shortcut_labels = input_state.action_binding_labels(cmd.action);
    let badge_left_edge = render_command_row_shortcut_badge(
        ctx,
        item_y,
        content_right,
        desc_x,
        is_selected,
        &shortcut_labels,
        &styles.shortcut,
    );

    let max_desc_width = (badge_left_edge - 12.0 - desc_x).max(0.0);
    let desc_alpha = if is_selected { 0.9 } else { 0.75 };
    render_command_row_description(
        ctx,
        &styles.desc,
        cmd.description,
        desc_x,
        label_y,
        max_desc_width,
        desc_alpha,
    );
}

fn render_command_row_label(
    ctx: &cairo::Context,
    label: &str,
    x: f64,
    y: f64,
    styles: &CommandPaletteRowStyle,
) {
    draw_text_baseline(ctx, styles.label, label, x, y, None);
}

pub(super) fn render_command_row_shortcut_badge(
    ctx: &cairo::Context,
    item_y: f64,
    content_right: f64,
    desc_x: f64,
    is_selected: bool,
    shortcut_labels: &[String],
    shortcut_style: &UiTextStyle,
) -> f64 {
    let mut badge_left_edge = content_right;
    if let Some(shortcut) = shortcut_labels.first() {
        let max_badge_w = (content_right
            - desc_x
            - COMMAND_PALETTE_SHORTCUT_MIN_DESC_WIDTH
            - COMMAND_PALETTE_SHORTCUT_BADGE_GAP)
            .max(0.0);

        if max_badge_w > COMMAND_PALETTE_SHORTCUT_BADGE_PADDING_X * 2.0 {
            let max_shortcut_text_w = max_badge_w - COMMAND_PALETTE_SHORTCUT_BADGE_PADDING_X * 2.0;
            let shortcut_display = ellipsize_to_width(
                ctx,
                shortcut,
                COMMAND_PALETTE_FONT_FAMILY,
                shortcut_style.slant,
                shortcut_style.weight,
                shortcut_style.size,
                max_shortcut_text_w,
            );
            if !shortcut_display.is_empty() {
                let shortcut_extents = text_extents_for(
                    ctx,
                    COMMAND_PALETTE_FONT_FAMILY,
                    shortcut_style.slant,
                    shortcut_style.weight,
                    shortcut_style.size,
                    &shortcut_display,
                );
                let badge_w = (shortcut_extents.width()
                    + COMMAND_PALETTE_SHORTCUT_BADGE_PADDING_X * 2.0)
                    .min(max_badge_w);
                let badge_x = content_right - badge_w;
                let badge_y = item_y
                    + (COMMAND_PALETTE_ITEM_HEIGHT - COMMAND_PALETTE_SHORTCUT_BADGE_HEIGHT) / 2.0
                    - 1.0;
                badge_left_edge = badge_x;

                let badge_alpha = if is_selected { 0.35 } else { 0.25 };
                ctx.set_source_rgba(1.0, 1.0, 1.0, badge_alpha);
                draw_rounded_rect(
                    ctx,
                    badge_x,
                    badge_y,
                    badge_w,
                    COMMAND_PALETTE_SHORTCUT_BADGE_HEIGHT,
                    3.0,
                );
                let _ = ctx.fill();

                let shortcut_alpha = if is_selected { 0.95 } else { 0.8 };
                ctx.set_source_rgba(1.0, 1.0, 1.0, shortcut_alpha);
                draw_text_baseline(
                    ctx,
                    *shortcut_style,
                    &shortcut_display,
                    badge_x + COMMAND_PALETTE_SHORTCUT_BADGE_PADDING_X,
                    badge_y + COMMAND_PALETTE_SHORTCUT_BADGE_HEIGHT / 2.0 + 3.0,
                    None,
                );
            }
        }
    }
    badge_left_edge
}

pub(super) fn render_command_row_description(
    ctx: &cairo::Context,
    desc_style: &UiTextStyle,
    description: &str,
    desc_x: f64,
    label_y: f64,
    max_desc_width: f64,
    desc_alpha: f64,
) {
    ctx.set_source_rgba(
        TEXT_DESCRIPTION.0,
        TEXT_DESCRIPTION.1,
        TEXT_DESCRIPTION.2,
        desc_alpha,
    );
    if max_desc_width > 6.0 {
        let desc_display = ellipsize_to_width(
            ctx,
            description,
            COMMAND_PALETTE_FONT_FAMILY,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
            desc_style.size,
            max_desc_width,
        );
        if !desc_display.is_empty() {
            draw_text_baseline(ctx, *desc_style, &desc_display, desc_x, label_y, None);
        }
    }
}
