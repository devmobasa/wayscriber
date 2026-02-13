use log::debug;
use smithay_client_toolkit::seat::pointer::CursorIcon;
use wayland_client::Connection;

use super::{ABOUT_HEIGHT, ABOUT_WIDTH, AboutWindowState};

fn link_index_at_impl(links: &[super::LinkRegion], pos: (f64, f64)) -> Option<usize> {
    links.iter().position(|link| link.contains(pos))
}

fn update_hover_index(
    links: &[super::LinkRegion],
    current: Option<usize>,
    pos: (f64, f64),
) -> (Option<usize>, bool) {
    let next = link_index_at_impl(links, pos);
    (next, next != current)
}

impl AboutWindowState {
    pub(super) fn new(
        registry_state: super::RegistryState,
        compositor_state: super::CompositorState,
        shm: super::Shm,
        output_state: super::OutputState,
        seat_state: super::SeatState,
        xdg_shell: super::XdgShell,
        window: super::Window,
    ) -> Self {
        Self {
            registry_state,
            compositor_state,
            shm,
            output_state,
            seat_state,
            xdg_shell,
            window,
            pool: None,
            width: ABOUT_WIDTH,
            height: ABOUT_HEIGHT,
            scale: 1,
            configured: false,
            should_exit: false,
            needs_redraw: true,
            link_regions: Vec::new(),
            hover_index: None,
            themed_pointer: None,
        }
    }

    pub(super) fn link_index_at(&self, pos: (f64, f64)) -> Option<usize> {
        link_index_at_impl(&self.link_regions, pos)
    }

    pub(super) fn update_hover(&mut self, pos: (f64, f64)) {
        let (next, changed) = update_hover_index(&self.link_regions, self.hover_index, pos);
        if changed {
            self.hover_index = next;
            self.needs_redraw = true;
        }
    }

    pub(super) fn update_cursor(&self, conn: &Connection) {
        if let Some(pointer) = self.themed_pointer.as_ref() {
            let icon = if self.hover_index.is_some() {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            };
            if let Err(err) = pointer.set_cursor(conn, icon) {
                debug!("Failed to set cursor icon: {}", err);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_links() -> Vec<super::super::LinkRegion> {
        vec![
            super::super::LinkRegion {
                rect: (10.0, 10.0, 40.0, 20.0),
                action: super::super::LinkAction::Close,
            },
            super::super::LinkRegion {
                rect: (70.0, 12.0, 30.0, 30.0),
                action: super::super::LinkAction::OpenUrl("https://example.com".to_string()),
            },
        ]
    }

    #[test]
    fn link_index_at_finds_matching_region() {
        let links = sample_links();

        assert_eq!(link_index_at_impl(&links, (15.0, 15.0)), Some(0));
        assert_eq!(link_index_at_impl(&links, (90.0, 30.0)), Some(1));
        assert_eq!(link_index_at_impl(&links, (0.0, 0.0)), None);
    }

    #[test]
    fn update_hover_index_reports_when_hover_changed() {
        let links = sample_links();

        let (next, changed) = update_hover_index(&links, None, (15.0, 15.0));
        assert_eq!(next, Some(0));
        assert!(changed);

        let (next, changed) = update_hover_index(&links, Some(0), (16.0, 16.0));
        assert_eq!(next, Some(0));
        assert!(!changed);

        let (next, changed) = update_hover_index(&links, Some(0), (90.0, 30.0));
        assert_eq!(next, Some(1));
        assert!(changed);

        let (next, changed) = update_hover_index(&links, Some(1), (1.0, 1.0));
        assert_eq!(next, None);
        assert!(changed);
    }
}
