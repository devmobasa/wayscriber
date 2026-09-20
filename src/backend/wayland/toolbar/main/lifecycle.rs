use crate::ui_text::UiTextEngine;
use log::info;
use smithay_client_toolkit::{
    compositor::CompositorState,
    shell::wlr_layer::{LayerShell, LayerSurfaceConfigure},
};
use wayland_client::{QueueHandle, protocol::wl_output};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{
        wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        wp_fractional_scale_v1::WpFractionalScaleV1,
    },
    viewporter::client::wp_viewporter::WpViewporter,
};

use super::structs::ToolbarSurfaceManager;
use crate::backend::wayland::state::WaylandState;
use crate::ui::toolbar::ToolbarSnapshot;

impl ToolbarSurfaceManager {
    #[allow(clippy::too_many_arguments)]
    pub fn ensure_created(
        &mut self,
        engine: &UiTextEngine,
        qh: &QueueHandle<WaylandState>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        scale: i32,
        output: Option<&wl_output::WlOutput>,
        snapshot: &ToolbarSnapshot,
        scaling: Option<(&WpFractionalScaleManagerV1, &WpViewporter)>,
    ) {
        let top_size = crate::backend::wayland::toolbar::top_size(engine, snapshot);

        if self.is_top_visible() {
            if self.top.layer_surface.is_none() {
                info!(
                    "Ensuring top toolbar surface exists at logical size {:?}, scale {}",
                    top_size, scale
                );
                self.top.set_logical_size(top_size);
            } else if self.top.logical_size != top_size {
                // Resize the mapped surface in place — destroying and
                // recreating it made every size change flicker.
                self.top.resize(top_size);
            }
            self.top
                .ensure_created(qh, compositor, layer_shell, scale, output, scaling);
        }

        if self.suppressed {
            self.top.set_suppressed(compositor, true);
        }
    }

    pub fn handle_configure(
        &mut self,
        configure: &LayerSurfaceConfigure,
        layer: &smithay_client_toolkit::shell::wlr_layer::LayerSurface,
    ) -> bool {
        if self.top.is_layer(layer) {
            return self.top.handle_configure(configure);
        }
        false
    }

    pub fn maybe_update_scale(&mut self, output: Option<&wl_output::WlOutput>, scale: i32) {
        self.top.maybe_update_scale(output, scale);
    }

    pub(in crate::backend::wayland) fn set_preferred_scale(
        &mut self,
        source: &WpFractionalScaleV1,
        preferred: u32,
    ) -> anyhow::Result<bool> {
        self.top.set_preferred_scale(source, preferred)
    }
}
