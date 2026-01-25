//! xdg-desktop-portal integration for screenshot capture.

use super::types::{CaptureError, CaptureType};
use futures::StreamExt;
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;
use zbus::{Connection, proxy};

/// D-Bus proxy for the xdg-desktop-portal Screenshot interface.
#[proxy(
    interface = "org.freedesktop.portal.Screenshot",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait Screenshot {
    /// Take a screenshot.
    ///
    /// # Arguments
    /// * `parent_window` - Identifier for the parent window (empty string for none)
    /// * `options` - Options for the screenshot
    ///
    /// # Returns
    /// Response containing the URI to the screenshot file
    async fn screenshot(
        &self,
        parent_window: &str,
        options: HashMap<String, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    /// Pick a color from the screen.
    ///
    /// # Arguments
    /// * `parent_window` - Identifier for the parent window (empty string for none)
    /// * `options` - Options for the color picker
    ///
    /// # Returns
    /// Response containing the picked color
    async fn pick_color(
        &self,
        parent_window: &str,
        options: HashMap<String, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

/// D-Bus proxy for org.freedesktop.portal.Request interface.
/// This is used to receive the Response signal from the portal.
#[proxy(
    interface = "org.freedesktop.portal.Request",
    default_service = "org.freedesktop.portal.Desktop"
)]
trait Request {
    /// Response signal emitted when the request is completed.
    ///
    /// # Signal Arguments
    /// * `response` - Response code (0 = success, 1 = cancelled, 2 = other error)
    /// * `results` - Dictionary containing the results (e.g., "uri" key)
    #[zbus(signal)]
    fn response(&self, response: u32, results: HashMap<String, OwnedValue>) -> zbus::Result<()>;
}

/// Capture a screenshot using xdg-desktop-portal.
///
/// This function communicates with the desktop portal via D-Bus to capture
/// a screenshot. The portal may prompt the user for permission.
///
/// # Arguments
/// * `capture_type` - Type of screenshot to capture
///
/// # Returns
/// The URI path to the captured screenshot file
pub async fn capture_via_portal(capture_type: CaptureType) -> Result<String, CaptureError> {
    log::debug!("Initiating portal screenshot capture: {:?}", capture_type);

    // Connect to session bus
    let connection = Connection::session()
        .await
        .map_err(CaptureError::DBusError)?;

    // Create proxy for Screenshot portal
    let proxy = ScreenshotProxy::new(&connection)
        .await
        .map_err(CaptureError::DBusError)?;

    // Prepare options based on capture type
    let options = build_portal_options(capture_type);

    log::debug!("Calling portal screenshot with options: {:?}", options);

    // Call screenshot method - this returns a Request object path
    let request_path = proxy.screenshot("", options).await.map_err(|e| {
        log::error!("Portal screenshot call failed: {}", e);
        // Check if it's a permission denial
        if e.to_string().contains("Cancelled") || e.to_string().contains("denied") {
            CaptureError::PermissionDenied
        } else {
            CaptureError::DBusError(e)
        }
    })?;

    log::info!("Screenshot request created: {:?}", request_path);

    // Create a proxy for the Request object to receive Response signal
    let request_proxy = RequestProxy::builder(&connection)
        .path(request_path.clone())
        .map_err(CaptureError::DBusError)?
        .build()
        .await
        .map_err(CaptureError::DBusError)?;

    // Wait for the Response signal
    let mut response_stream = request_proxy
        .receive_response()
        .await
        .map_err(CaptureError::DBusError)?;

    log::debug!("Waiting for Response signal...");

    // Get the first (and only) response
    let response_signal = response_stream
        .next()
        .await
        .ok_or_else(|| CaptureError::InvalidResponse("No Response signal received".to_string()))?;

    let args = response_signal.args().map_err(|e| {
        CaptureError::InvalidResponse(format!("Failed to parse response args: {}", e))
    })?;

    log::debug!(
        "Response signal received: code={}, results={:?}",
        args.response,
        args.results
    );

    // Check response code (0 = success, 1 = cancelled, 2 = other error)
    match args.response {
        0 => {
            // Success - extract URI from results
            let uri_value = args.results.get("uri").ok_or_else(|| {
                CaptureError::InvalidResponse("No 'uri' field in response".to_string())
            })?;

            // Extract string from OwnedValue
            // OwnedValue can be converted to a borrowed Value for downcasting
            let uri_str: &str = uri_value.downcast_ref().map_err(|e| {
                CaptureError::InvalidResponse(format!("URI is not a string: {}", e))
            })?;

            log::info!("Screenshot captured successfully: {}", uri_str);
            Ok(uri_str.to_string())
        }
        1 => {
            log::warn!("Screenshot cancelled by user");
            Err(CaptureError::PermissionDenied)
        }
        code => {
            log::error!("Screenshot failed with code {}", code);
            Err(CaptureError::InvalidResponse(format!(
                "Portal returned error code {}",
                code
            )))
        }
    }
}

/// Build portal options based on capture type.
fn build_portal_options(
    capture_type: CaptureType,
) -> HashMap<String, zbus::zvariant::Value<'static>> {
    let mut options = HashMap::new();

    match capture_type {
        CaptureType::FullScreen => {
            options.insert("interactive".to_string(), false.into());
        }
        CaptureType::ActiveWindow => {
            // Interactive = true lets user select window
            // TODO: Try to get active window first, fall back to interactive
            options.insert("interactive".to_string(), true.into());
        }
        CaptureType::Selection { .. } => {
            // Interactive mode for selection
            options.insert("interactive".to_string(), true.into());
        }
    }

    options
}

/// Check if xdg-desktop-portal is available on the system.
#[allow(dead_code)] // Will be used in Phase 2 for capability detection
pub async fn is_portal_available() -> bool {
    match Connection::session().await {
        Ok(connection) => {
            // Try to create the proxy
            ScreenshotProxy::new(&connection).await.is_ok()
        }
        Err(_) => false,
    }
}

/// Pick a color from the screen using xdg-desktop-portal.
///
/// This uses the portal's PickColor method which shows a color picker UI
/// allowing the user to click anywhere on screen to sample a color.
/// Works on GNOME, KDE, and other desktops that implement the portal.
///
/// # Returns
/// The picked color as (r, g, b) with values in 0.0-1.0 range
pub async fn pick_color_via_portal() -> Result<(f64, f64, f64), CaptureError> {
    log::debug!("Initiating portal color picker");

    // Connect to session bus
    let connection = Connection::session()
        .await
        .map_err(CaptureError::DBusError)?;

    // Create proxy for Screenshot portal
    let proxy = ScreenshotProxy::new(&connection)
        .await
        .map_err(CaptureError::DBusError)?;

    // Empty options for color picker
    let options: HashMap<String, zbus::zvariant::Value<'static>> = HashMap::new();

    log::debug!("Calling portal pick_color");

    // Call pick_color method - this returns a Request object path
    let request_path = proxy.pick_color("", options).await.map_err(|e| {
        log::error!("Portal pick_color call failed: {}", e);
        if e.to_string().contains("Cancelled") || e.to_string().contains("denied") {
            CaptureError::PermissionDenied
        } else {
            CaptureError::DBusError(e)
        }
    })?;

    log::info!("Color picker request created: {:?}", request_path);

    // Create a proxy for the Request object to receive Response signal
    let request_proxy = RequestProxy::builder(&connection)
        .path(request_path.clone())
        .map_err(CaptureError::DBusError)?
        .build()
        .await
        .map_err(CaptureError::DBusError)?;

    // Wait for the Response signal
    let mut response_stream = request_proxy
        .receive_response()
        .await
        .map_err(CaptureError::DBusError)?;

    log::debug!("Waiting for color picker Response signal...");

    // Get the first (and only) response
    let response_signal = response_stream
        .next()
        .await
        .ok_or_else(|| CaptureError::InvalidResponse("No Response signal received".to_string()))?;

    let args = response_signal.args().map_err(|e| {
        CaptureError::InvalidResponse(format!("Failed to parse response args: {}", e))
    })?;

    log::debug!(
        "Color picker response: code={}, results={:?}",
        args.response,
        args.results
    );

    // Check response code (0 = success, 1 = cancelled, 2 = other error)
    match args.response {
        0 => {
            // Success - extract color from results
            // The color is returned as (ddd) - a struct of 3 doubles
            let color_value = args.results.get("color").ok_or_else(|| {
                CaptureError::InvalidResponse("No 'color' field in response".to_string())
            })?;

            // The color is a tuple/struct of 3 f64 values (r, g, b)
            // Try to extract as an array or tuple
            let color_tuple: (f64, f64, f64) = color_value.downcast_ref().map_err(|e| {
                CaptureError::InvalidResponse(format!("Color is not a (ddd) tuple: {}", e))
            })?;

            log::info!(
                "Color picked: ({:.3}, {:.3}, {:.3})",
                color_tuple.0,
                color_tuple.1,
                color_tuple.2
            );
            Ok(color_tuple)
        }
        1 => {
            log::warn!("Color picker cancelled by user");
            Err(CaptureError::PermissionDenied)
        }
        code => {
            log::error!("Color picker failed with code {}", code);
            Err(CaptureError::InvalidResponse(format!(
                "Portal returned error code {}",
                code
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_portal_options_full_screen() {
        let options = build_portal_options(CaptureType::FullScreen);

        // Full screen should be non-interactive
        assert_eq!(
            options.get("interactive"),
            Some(&zbus::zvariant::Value::from(false))
        );
    }

    #[test]
    fn test_build_portal_options_selection() {
        let options = build_portal_options(CaptureType::Selection {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        });

        // Selection should be interactive
        assert_eq!(
            options.get("interactive"),
            Some(&zbus::zvariant::Value::from(true))
        );
    }
}
