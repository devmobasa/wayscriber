use anyhow::Result;
use log::info;
use smithay_client_toolkit::shell::{
    WaylandSurface,
    wlr_layer::{Anchor, Layer},
    xdg::window::WindowDecorations,
};

use super::super::state::WaylandState;

pub(super) fn create_overlay_surface(
    state: &mut WaylandState,
    qh: &wayland_client::QueueHandle<WaylandState>,
) -> Result<()> {
    // Create surface using layer-shell when available, otherwise fall back to xdg-shell
    let wl_surface = state.compositor_state.create_surface(qh);
    if let Some(layer_shell) = state.layer_shell.as_ref() {
        info!("Creating layer shell surface");
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            wl_surface,
            Layer::Top,
            Some("wayscriber"),
            None, // Default output
        );

        // Configure the layer surface for fullscreen overlay
        layer_surface.set_anchor(Anchor::all());
        let desired_keyboard_mode = state.desired_keyboard_interactivity();
        layer_surface.set_keyboard_interactivity(desired_keyboard_mode);
        layer_surface.set_size(0, 0); // Use full screen size
        layer_surface.set_exclusive_zone(-1);

        // Commit the surface
        layer_surface.commit();

        state.surface.set_layer_surface(layer_surface);
        state.set_current_keyboard_interactivity(Some(desired_keyboard_mode));
        info!("Layer shell surface created");
    } else if let Some(xdg_shell) = state.xdg_shell.as_ref() {
        info!("Layer shell missing; creating xdg-shell window");
        let window = xdg_shell.create_window(wl_surface, WindowDecorations::None, qh);
        window.set_title("wayscriber overlay");
        window.set_app_id("com.devmobasa.wayscriber");
        if state.xdg_fullscreen() {
            if let Some(output) = state.preferred_fullscreen_output() {
                info!("Requesting fullscreen on preferred output");
                window.set_fullscreen(Some(&output));
            } else {
                info!("Preferred output unknown; requesting compositor-chosen fullscreen");
                window.set_fullscreen(None);
            }
        } else {
            window.set_maximized();
        }
        window.commit();
        state.surface.set_xdg_window(window);
        state.request_xdg_activation(qh);
        info!("xdg-shell window created");
    } else {
        return Err(anyhow::anyhow!(
            "No supported Wayland shell protocol available"
        ));
    }

    Ok(())
}
