use smithay_client_toolkit::seat::pointer::{
    BTN_LEFT, PointerEvent, PointerEventKind, PointerHandler,
};
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::{Connection, QueueHandle, protocol::wl_pointer};

use super::super::clipboard::{copy_text_to_clipboard, open_url};
use super::super::{AboutWindowState, LinkAction};

impl PointerHandler for AboutWindowState {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if &event.surface != self.window.wl_surface() {
                continue;
            }
            match event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.update_hover(event.position);
                    self.update_cursor(conn);
                }
                PointerEventKind::Leave { .. } => {
                    if self.hover_index.is_some() {
                        self.hover_index = None;
                        self.needs_redraw = true;
                    }
                    self.update_cursor(conn);
                }
                PointerEventKind::Press { button, .. } => {
                    if button == BTN_LEFT
                        && let Some(index) = self.link_index_at(event.position)
                        && let Some(link) = self.link_regions.get(index)
                    {
                        match &link.action {
                            LinkAction::OpenUrl(url) => open_url(url),
                            LinkAction::CopyText(text) => copy_text_to_clipboard(text),
                            LinkAction::Close => self.should_exit = true,
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
