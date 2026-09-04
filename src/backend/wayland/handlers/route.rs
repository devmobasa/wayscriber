use wayland_client::protocol::wl_surface;

use super::super::state::{MoveDragKind, WaylandState, surface_id};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) enum InputSurface {
    Canvas,
    Toolbar,
    Foreign,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::backend::wayland) struct RoutedInput {
    pub(in crate::backend::wayland) surface: InputSurface,
    /// Built-in inline strips are drawn on the canvas and hit-tested there.
    pub(in crate::backend::wayland) inline_toolbars: bool,
    /// Overlay screen coordinates. Toolbar-local positions are converted;
    /// foreign surfaces have no overlay coordinates.
    pub(in crate::backend::wayland) screen: Option<(f64, f64)>,
}

/// Snapshot of the protocol surfaces that can own an input callback.
pub(in crate::backend::wayland) struct SurfaceRouter {
    canvas: Option<u32>,
    toolbar: Option<u32>,
    inline_toolbars: bool,
}

impl SurfaceRouter {
    fn new(canvas: Option<u32>, toolbar: Option<u32>, inline_toolbars: bool) -> Self {
        Self {
            canvas,
            toolbar,
            inline_toolbars,
        }
    }

    fn classify(&self, surface: u32) -> InputSurface {
        if self.canvas == Some(surface) {
            InputSurface::Canvas
        } else if self.toolbar == Some(surface) {
            InputSurface::Toolbar
        } else {
            InputSurface::Foreign
        }
    }

    fn route(
        &self,
        surface: u32,
        position: (f64, f64),
        toolbar_screen_position: (f64, f64),
    ) -> RoutedInput {
        let surface = self.classify(surface);
        let screen = match surface {
            InputSurface::Canvas => Some(position),
            InputSurface::Toolbar => Some(toolbar_screen_position),
            InputSurface::Foreign => None,
        };
        RoutedInput {
            surface,
            inline_toolbars: self.inline_toolbars,
            screen,
        }
    }
}

impl WaylandState {
    pub(in crate::backend::wayland) fn route_input(
        &self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> RoutedInput {
        SurfaceRouter::new(
            self.surface.wl_surface().map(surface_id),
            self.toolbar.wl_surface().map(surface_id),
            self.toolbar_chrome.inline_toolbars() && self.toolbar.is_visible(),
        )
        .route(
            surface_id(surface),
            position,
            self.local_to_screen_coords(MoveDragKind::Top, position),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{InputSurface, SurfaceRouter};

    #[test]
    fn classifies_canvas_toolbar_and_foreign_surface_ids() {
        for (surface, canvas, toolbar, expected) in [
            (10, Some(10), Some(20), InputSurface::Canvas),
            (20, Some(10), Some(20), InputSurface::Toolbar),
            (30, Some(10), Some(20), InputSurface::Foreign),
            (20, Some(10), None, InputSurface::Foreign),
            (10, None, Some(20), InputSurface::Foreign),
        ] {
            let router = SurfaceRouter::new(canvas, toolbar, false);
            assert_eq!(router.classify(surface), expected);
        }
    }

    #[test]
    fn routes_only_owned_surfaces_into_overlay_coordinates() {
        let router = SurfaceRouter::new(Some(10), Some(20), true);
        let canvas = router.route(10, (4.0, 5.0), (104.0, 205.0));
        assert_eq!(canvas.surface, InputSurface::Canvas);
        assert_eq!(canvas.screen, Some((4.0, 5.0)));
        assert!(canvas.inline_toolbars);

        let toolbar = router.route(20, (4.0, 5.0), (104.0, 205.0));
        assert_eq!(toolbar.surface, InputSurface::Toolbar);
        assert_eq!(toolbar.screen, Some((104.0, 205.0)));

        let foreign = router.route(30, (4.0, 5.0), (104.0, 205.0));
        assert_eq!(foreign.surface, InputSurface::Foreign);
        assert_eq!(foreign.screen, None);
    }
}
