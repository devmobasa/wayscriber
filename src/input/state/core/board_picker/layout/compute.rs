use cairo::Context as CairoContext;

use super::super::super::base::InputState;
use super::super::{
    BOARD_PICKER_RECENT_LINE_HEIGHT, BOARD_PICKER_RECENT_LINE_HEIGHT_COMPACT, BODY_FONT_SIZE,
    BoardPickerLayout, COMPACT_BODY_FONT_SIZE, COMPACT_FOOTER_FONT_SIZE, COMPACT_FOOTER_HEIGHT,
    COMPACT_HEADER_HEIGHT, COMPACT_PADDING_X, COMPACT_PADDING_Y, COMPACT_ROW_HEIGHT,
    COMPACT_SWATCH_PADDING, COMPACT_SWATCH_SIZE, COMPACT_TITLE_FONT_SIZE, FOOTER_FONT_SIZE,
    FOOTER_HEIGHT, HANDLE_GAP, HANDLE_WIDTH, HEADER_HEIGHT, OPEN_ICON_GAP, OPEN_ICON_SIZE,
    PADDING_X, PADDING_Y, PAGE_PANEL_MAX_ROWS, PAGE_THUMB_GAP, PAGE_THUMB_HEIGHT, PALETTE_TOP_GAP,
    ROW_HEIGHT, SWATCH_PADDING, SWATCH_SIZE, TITLE_FONT_SIZE,
};
mod content_metrics;
mod layout_geometry;
mod page_panel;
mod palette_metrics;

#[derive(Clone, Copy)]
struct BoardPickerLayoutConfig {
    title_font_size: f64,
    body_font_size: f64,
    footer_font_size: f64,
    row_height: f64,
    header_height: f64,
    base_footer_height: f64,
    padding_x: f64,
    padding_y: f64,
    swatch_size: f64,
    swatch_padding: f64,
    recent_line_height: f64,
    handle_width: f64,
    handle_gap: f64,
    open_icon_size: f64,
    open_icon_gap: f64,
}

impl BoardPickerLayoutConfig {
    fn for_mode(is_quick: bool) -> Self {
        if is_quick {
            Self {
                title_font_size: COMPACT_TITLE_FONT_SIZE,
                body_font_size: COMPACT_BODY_FONT_SIZE,
                footer_font_size: COMPACT_FOOTER_FONT_SIZE,
                row_height: COMPACT_ROW_HEIGHT,
                header_height: COMPACT_HEADER_HEIGHT,
                base_footer_height: COMPACT_FOOTER_HEIGHT,
                padding_x: COMPACT_PADDING_X,
                padding_y: COMPACT_PADDING_Y,
                swatch_size: COMPACT_SWATCH_SIZE,
                swatch_padding: COMPACT_SWATCH_PADDING,
                recent_line_height: BOARD_PICKER_RECENT_LINE_HEIGHT_COMPACT,
                handle_width: 0.0,
                handle_gap: 0.0,
                open_icon_size: 0.0,
                open_icon_gap: 0.0,
            }
        } else {
            Self {
                title_font_size: TITLE_FONT_SIZE,
                body_font_size: BODY_FONT_SIZE,
                footer_font_size: FOOTER_FONT_SIZE,
                row_height: ROW_HEIGHT,
                header_height: HEADER_HEIGHT,
                base_footer_height: FOOTER_HEIGHT,
                padding_x: PADDING_X,
                padding_y: PADDING_Y,
                swatch_size: SWATCH_SIZE,
                swatch_padding: SWATCH_PADDING,
                recent_line_height: BOARD_PICKER_RECENT_LINE_HEIGHT,
                handle_width: HANDLE_WIDTH,
                handle_gap: HANDLE_GAP,
                open_icon_size: OPEN_ICON_SIZE,
                open_icon_gap: OPEN_ICON_GAP,
            }
        }
    }
}

#[derive(Clone, Copy)]
struct BoardPickerContentMetrics {
    list_width: f64,
    max_hint_width: f64,
    footer_height: f64,
    recent_height: f64,
}

#[derive(Clone, Copy)]
struct BoardPickerPaletteMetrics {
    rows: usize,
    cols: usize,
    extra_height: f64,
}

#[derive(Clone, Copy)]
struct BoardPickerPagePanelMetrics {
    enabled: bool,
    width: f64,
    height: f64,
    thumb_width: f64,
    cols: usize,
    rows: usize,
    count: usize,
    visible_count: usize,
    board_index: Option<usize>,
}

#[derive(Clone, Copy)]
struct BoardPickerLayoutGeometry {
    origin_x: f64,
    origin_y: f64,
    width: f64,
    list_width: f64,
    page_panel_x: f64,
    page_panel_y: f64,
}

impl InputState {
    pub(crate) fn board_picker_layout(&self) -> Option<&BoardPickerLayout> {
        self.board_picker_layout.as_ref()
    }

    pub(crate) fn clear_board_picker_layout(&mut self) {
        self.board_picker_layout = None;
    }

    pub(crate) fn update_board_picker_layout(
        &mut self,
        ctx: &CairoContext,
        screen_width: u32,
        screen_height: u32,
    ) {
        if !self.is_board_picker_open() {
            self.board_picker_layout = None;
            return;
        }

        let row_count = self.board_picker_row_count();
        if row_count == 0 {
            self.board_picker_layout = None;
            return;
        }

        let board_count = self.boards.board_count();
        let max_count = self.boards.max_count();

        let config = BoardPickerLayoutConfig::for_mode(self.board_picker_is_quick());
        let edit_state = self.board_picker_edit_state();
        let content = self.compute_board_picker_content_metrics(
            ctx,
            row_count,
            board_count,
            max_count,
            &config,
            edit_state,
        );
        let palette = self.compute_board_picker_palette_metrics(
            edit_state,
            content.list_width,
            config.padding_x,
        );

        let panel_height =
            self.derive_board_picker_panel_height(&config, row_count, &content, &palette);
        let (page_panel, panel_height) = self.compute_board_picker_page_panel_metrics(
            screen_width,
            screen_height,
            content.footer_height,
            panel_height,
        );
        let geometry = self.compute_board_picker_layout_geometry(
            screen_width,
            screen_height,
            content.list_width,
            panel_height,
            &page_panel,
        );

        self.board_picker_layout = Some(self.build_board_picker_layout(
            &config,
            row_count,
            &content,
            &palette,
            &page_panel,
            &geometry,
            panel_height,
        ));
    }

    fn derive_board_picker_panel_height(
        &self,
        config: &BoardPickerLayoutConfig,
        row_count: usize,
        content: &BoardPickerContentMetrics,
        palette: &BoardPickerPaletteMetrics,
    ) -> f64 {
        config.padding_y * 2.0
            + config.header_height
            + config.row_height * row_count as f64
            + palette.extra_height
            + content.footer_height
    }

    #[allow(clippy::too_many_arguments)]
    fn build_board_picker_layout(
        &self,
        config: &BoardPickerLayoutConfig,
        row_count: usize,
        content: &BoardPickerContentMetrics,
        palette: &BoardPickerPaletteMetrics,
        page_panel: &BoardPickerPagePanelMetrics,
        geometry: &BoardPickerLayoutGeometry,
        panel_height: f64,
    ) -> BoardPickerLayout {
        BoardPickerLayout {
            origin_x: geometry.origin_x,
            origin_y: geometry.origin_y,
            width: geometry.width,
            height: panel_height,
            list_width: geometry.list_width,
            title_font_size: config.title_font_size,
            body_font_size: config.body_font_size,
            footer_font_size: config.footer_font_size,
            row_height: config.row_height,
            header_height: config.header_height,
            footer_height: content.footer_height,
            padding_x: config.padding_x,
            padding_y: config.padding_y,
            swatch_size: config.swatch_size,
            swatch_padding: config.swatch_padding,
            hint_width: content.max_hint_width,
            row_count,
            palette_top: geometry.origin_y
                + config.padding_y
                + config.header_height
                + config.row_height * row_count as f64
                + PALETTE_TOP_GAP,
            palette_rows: palette.rows,
            palette_cols: palette.cols,
            recent_height: content.recent_height,
            handle_width: config.handle_width,
            handle_gap: config.handle_gap,
            open_icon_size: config.open_icon_size,
            open_icon_gap: config.open_icon_gap,
            page_panel_enabled: page_panel.enabled,
            page_panel_x: geometry.page_panel_x,
            page_panel_y: geometry.page_panel_y,
            page_panel_width: page_panel.width,
            page_panel_height: page_panel.height,
            page_thumb_width: page_panel.thumb_width,
            page_thumb_height: PAGE_THUMB_HEIGHT,
            page_thumb_gap: PAGE_THUMB_GAP,
            page_cols: page_panel.cols,
            page_rows: page_panel.rows,
            page_max_rows: PAGE_PANEL_MAX_ROWS,
            page_count: page_panel.count,
            page_visible_count: page_panel.visible_count,
            page_board_index: page_panel.board_index,
        }
    }
}
