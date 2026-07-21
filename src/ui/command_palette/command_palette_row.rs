use crate::config::KeybindingsConfig;
use crate::config::action_meta::ActionMeta;
use crate::input::InputState;
use crate::input::state::COMMAND_PALETTE_ITEM_HEIGHT;
use crate::input::state::{
    COMMAND_PALETTE_ROW_ACTION_COUNT, COMMAND_PALETTE_ROW_ACTION_GAP,
    COMMAND_PALETTE_ROW_ACTION_SIZE, COMMAND_PALETTE_ROW_ICON_GAP, COMMAND_PALETTE_ROW_ICON_SIZE,
};
use crate::ui_text::{UiTextStyle, draw_text_baseline};

use super::super::constants::{self, BG_INPUT_SELECTION, RADIUS_SM, TEXT_DESCRIPTION, TEXT_WHITE};
use super::super::primitives::{draw_rounded_rect, text_extents_for};
use super::{
    COMMAND_PALETTE_FONT_FAMILY, COMMAND_PALETTE_SHORTCUT_BADGE_GAP,
    COMMAND_PALETTE_SHORTCUT_BADGE_HEIGHT, COMMAND_PALETTE_SHORTCUT_BADGE_PADDING_X,
    COMMAND_PALETTE_SHORTCUT_BADGE_RADIUS, COMMAND_PALETTE_SHORTCUT_MIN_DESC_WIDTH,
    ellipsize_to_width,
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
            RADIUS_SM,
        );
        constants::set_color(ctx, BG_INPUT_SELECTION);
        let _ = ctx.fill();
    }

    let text_alpha = if is_selected { 1.0 } else { 0.85 };
    let label_y = item_y + COMMAND_PALETTE_ITEM_HEIGHT / 2.0 + styles.label.size / 3.0;

    // Leading icon gutter: every row reserves the slot so labels align
    // whether or not the action has a glyph.
    if let Some(icon) = cmd.icon {
        let icon_alpha = if is_selected { 0.95 } else { 0.7 };
        constants::set_color(ctx, constants::with_alpha(TEXT_WHITE, icon_alpha));
        icon(
            ctx,
            inner_x + 10.0,
            item_y + (COMMAND_PALETTE_ITEM_HEIGHT - COMMAND_PALETTE_ROW_ICON_SIZE) / 2.0,
            COMMAND_PALETTE_ROW_ICON_SIZE,
        );
    }
    let label_x = inner_x + 10.0 + COMMAND_PALETTE_ROW_ICON_SIZE + COMMAND_PALETTE_ROW_ICON_GAP;

    constants::set_color(ctx, constants::with_alpha(TEXT_WHITE, text_alpha));
    render_command_row_label(ctx, cmd.label, label_x, label_y, styles);

    let label_extents = text_extents_for(
        ctx,
        COMMAND_PALETTE_FONT_FAMILY,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
        styles.label.size,
        cmd.label,
    );
    let desc_x = label_x + label_extents.width() + 12.0;
    let configurable = KeybindingsConfig::default()
        .bindings_for_action(cmd.action)
        .is_some();
    let actions_width = if configurable {
        (COMMAND_PALETTE_ROW_ACTION_SIZE + COMMAND_PALETTE_ROW_ACTION_GAP)
            * COMMAND_PALETTE_ROW_ACTION_COUNT as f64
    } else {
        0.0
    };
    let content_right = inner_x + inner_width - 8.0 - actions_width;

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
    if configurable {
        render_command_row_actions(ctx, inner_x + inner_width, item_y, is_selected);
    }
}

fn render_command_row_actions(
    ctx: &cairo::Context,
    content_right: f64,
    item_y: f64,
    selected: bool,
) {
    let stride = COMMAND_PALETTE_ROW_ACTION_SIZE + COMMAND_PALETTE_ROW_ACTION_GAP;
    let left = content_right - stride * COMMAND_PALETTE_ROW_ACTION_COUNT as f64;
    let icon_y = item_y + (COMMAND_PALETTE_ITEM_HEIGHT - COMMAND_PALETTE_ROW_ACTION_SIZE) / 2.0;
    let alpha = if selected { 0.95 } else { 0.62 };
    constants::set_color(ctx, constants::with_alpha(TEXT_WHITE, alpha));
    crate::toolbar_icons::draw_icon_pencil(
        ctx,
        left + 3.0,
        icon_y + 3.0,
        COMMAND_PALETTE_ROW_ACTION_SIZE - 6.0,
    );
    crate::toolbar_icons::draw_icon_clear(
        ctx,
        left + stride + 3.0,
        icon_y + 3.0,
        COMMAND_PALETTE_ROW_ACTION_SIZE - 6.0,
    );
    crate::toolbar_icons::draw_icon_refresh(
        ctx,
        left + stride * 2.0 + 3.0,
        icon_y + 3.0,
        COMMAND_PALETTE_ROW_ACTION_SIZE - 6.0,
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

                // White-alpha ladder: badge fill and text both derive from
                // the white root, brighter when the row is selected.
                let badge_alpha = if is_selected { 0.35 } else { 0.25 };
                constants::set_color(ctx, constants::with_alpha(TEXT_WHITE, badge_alpha));
                draw_rounded_rect(
                    ctx,
                    badge_x,
                    badge_y,
                    badge_w,
                    COMMAND_PALETTE_SHORTCUT_BADGE_HEIGHT,
                    COMMAND_PALETTE_SHORTCUT_BADGE_RADIUS,
                );
                let _ = ctx.fill();

                let shortcut_alpha = if is_selected { 0.95 } else { 0.8 };
                constants::set_color(ctx, constants::with_alpha(TEXT_WHITE, shortcut_alpha));
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
    constants::set_color(ctx, constants::with_alpha(TEXT_DESCRIPTION, desc_alpha));
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
