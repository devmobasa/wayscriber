use crate::input::BoardBackground;

pub(in crate::ui::board_picker::page_panel) const PREVIEW_SCALE: f64 = 1.6;

pub(in crate::ui::board_picker::page_panel) struct PageThumbnailArgs<'a> {
    pub(in crate::ui::board_picker::page_panel) ctx: &'a cairo::Context,
    pub(in crate::ui::board_picker::page_panel) frame: &'a crate::draw::Frame,
    pub(in crate::ui::board_picker::page_panel) background: &'a BoardBackground,
    pub(in crate::ui::board_picker::page_panel) x: f64,
    pub(in crate::ui::board_picker::page_panel) y: f64,
    pub(in crate::ui::board_picker::page_panel) width: f64,
    pub(in crate::ui::board_picker::page_panel) height: f64,
    pub(in crate::ui::board_picker::page_panel) screen_width: u32,
    pub(in crate::ui::board_picker::page_panel) screen_height: u32,
    pub(in crate::ui::board_picker::page_panel) page_number: usize,
    pub(in crate::ui::board_picker::page_panel) page_name: Option<&'a str>,
    pub(in crate::ui::board_picker::page_panel) is_active: bool,
    pub(in crate::ui::board_picker::page_panel) is_drop_target: bool,
    pub(in crate::ui::board_picker::page_panel) is_hovered: bool,
    pub(in crate::ui::board_picker::page_panel) is_keyboard_focused: bool,
    pub(in crate::ui::board_picker::page_panel) delete_hovered: bool,
    pub(in crate::ui::board_picker::page_panel) duplicate_hovered: bool,
    pub(in crate::ui::board_picker::page_panel) rename_hovered: bool,
}

pub(in crate::ui::board_picker::page_panel) struct PagePreviewArgs<'a> {
    pub(in crate::ui::board_picker::page_panel) ctx: &'a cairo::Context,
    pub(in crate::ui::board_picker::page_panel) frame: &'a crate::draw::Frame,
    pub(in crate::ui::board_picker::page_panel) background: &'a BoardBackground,
    pub(in crate::ui::board_picker::page_panel) thumb_x: f64,
    pub(in crate::ui::board_picker::page_panel) thumb_y: f64,
    pub(in crate::ui::board_picker::page_panel) thumb_w: f64,
    pub(in crate::ui::board_picker::page_panel) thumb_h: f64,
    pub(in crate::ui::board_picker::page_panel) screen_width: u32,
    pub(in crate::ui::board_picker::page_panel) screen_height: u32,
    pub(in crate::ui::board_picker::page_panel) page_number: usize,
}

pub(in crate::ui::board_picker::page_panel) struct PageContentArgs<'a> {
    pub(in crate::ui::board_picker::page_panel) ctx: &'a cairo::Context,
    pub(in crate::ui::board_picker::page_panel) frame: &'a crate::draw::Frame,
    pub(in crate::ui::board_picker::page_panel) background: &'a BoardBackground,
    pub(in crate::ui::board_picker::page_panel) x: f64,
    pub(in crate::ui::board_picker::page_panel) y: f64,
    pub(in crate::ui::board_picker::page_panel) width: f64,
    pub(in crate::ui::board_picker::page_panel) height: f64,
    pub(in crate::ui::board_picker::page_panel) screen_width: u32,
    pub(in crate::ui::board_picker::page_panel) screen_height: u32,
}
