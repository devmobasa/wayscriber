#![cfg(feature = "portal")]
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
            // Modal = false means non-interactive (capture immediately)
            options.insert("modal".to_string(), false.into());
            options.insert("interactive".to_string(), false.into());
        }
        CaptureType::ActiveWindow => {
            // Interactive = true lets user select window
            // TODO: Try to get active window first, fall back to interactive
            options.insert("modal".to_string(), false.into());
            options.insert("interactive".to_string(), true.into());
        }
        CaptureType::Selection { .. } => {
            // Interactive mode for selection
            options.insert("modal".to_string(), false.into());
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
