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

struct PortalAttempt {
    label: &'static str,
    options: HashMap<String, zbus::zvariant::Value<'static>>,
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

    let attempts = portal_attempts(capture_type);
    let mut last_error = None;

    for (index, attempt) in attempts.into_iter().enumerate() {
        if index > 0 {
            log::info!(
                "Retrying portal capture with '{}' options after previous failure",
                attempt.label
            );
        }

        match capture_once(&connection, &proxy, attempt.options).await {
            Ok(uri) => return Ok(uri),
            Err(err) => {
                log::warn!("Portal capture attempt '{}' failed: {}", attempt.label, err);
                last_error = Some(err);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        CaptureError::InvalidResponse("Portal capture failed without an explicit error".to_string())
    }))
}

fn portal_attempts(capture_type: CaptureType) -> Vec<PortalAttempt> {
    match capture_type {
        CaptureType::ActiveWindow => vec![PortalAttempt {
            // Use interactive portal flow for correctness: some compositors accept
            // non-standard `window=true` but ignore it and return fullscreen.
            label: "active-window-interactive",
            options: build_active_window_interactive_options(),
        }],
        _ => vec![PortalAttempt {
            label: "default",
            options: build_portal_options(capture_type),
        }],
    }
}

async fn capture_once(
    connection: &Connection,
    proxy: &ScreenshotProxy<'_>,
    options: HashMap<String, zbus::zvariant::Value<'static>>,
) -> Result<String, CaptureError> {
    log::debug!("Calling portal screenshot with options: {:?}", options);

    // Call screenshot method - this returns a Request object path.
    let request_path = proxy
        .screenshot("", options)
        .await
        .map_err(map_portal_call_error)?;

    log::info!("Screenshot request created: {:?}", request_path);

    // Create a proxy for the Request object to receive Response signal.
    let request_proxy = RequestProxy::builder(connection)
        .path(request_path)
        .map_err(CaptureError::DBusError)?
        .build()
        .await
        .map_err(CaptureError::DBusError)?;

    // Wait for the Response signal.
    let mut response_stream = request_proxy
        .receive_response()
        .await
        .map_err(CaptureError::DBusError)?;

    log::debug!("Waiting for Response signal...");

    // Get the first (and only) response.
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

    parse_response(args.response, &args.results)
}

fn parse_response(
    response_code: u32,
    results: &HashMap<String, OwnedValue>,
) -> Result<String, CaptureError> {
    // Check response code (0 = success, 1 = cancelled, 2 = other error).
    match response_code {
        0 => {
            // Success - extract URI from results.
            let uri_value = results.get("uri").ok_or_else(|| {
                CaptureError::InvalidResponse("No 'uri' field in response".to_string())
            })?;

            // Extract string from OwnedValue.
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

fn map_portal_call_error(err: zbus::Error) -> CaptureError {
    log::error!("Portal screenshot call failed: {}", err);
    let message = err.to_string();
    if message.contains("Cancelled") || message.contains("denied") {
        CaptureError::PermissionDenied
    } else {
        CaptureError::DBusError(err)
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
            options.insert("interactive".to_string(), true.into());
        }
        CaptureType::Selection { .. } => {
            // Interactive mode for selection.
            options.insert("interactive".to_string(), true.into());
        }
    }

    options
}

/// Active-window capture options (user picks window interactively).
fn build_active_window_interactive_options() -> HashMap<String, zbus::zvariant::Value<'static>> {
    let mut options = HashMap::new();
    options.insert("interactive".to_string(), true.into());
    options
}

/// Check if xdg-desktop-portal is available on the system.
#[allow(dead_code)] // Will be used in Phase 2 for capability detection
pub async fn is_portal_available() -> bool {
    match Connection::session().await {
        Ok(connection) => {
            // Try to create the proxy.
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

        // Full screen should be non-interactive.
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

        // Selection should be interactive.
        assert_eq!(
            options.get("interactive"),
            Some(&zbus::zvariant::Value::from(true))
        );
    }

    #[test]
    fn test_portal_attempts_active_window_uses_interactive_only() {
        let attempts = portal_attempts(CaptureType::ActiveWindow);
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].label, "active-window-interactive");
        assert_eq!(
            attempts[0].options.get("interactive"),
            Some(&zbus::zvariant::Value::from(true))
        );
        assert!(!attempts[0].options.contains_key("window"));
    }

    #[test]
    fn test_build_active_window_interactive_options() {
        let options = build_active_window_interactive_options();

        assert_eq!(
            options.get("interactive"),
            Some(&zbus::zvariant::Value::from(true))
        );
    }
}
