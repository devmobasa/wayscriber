//! xdg-desktop-portal integration for screenshot capture.

use super::types::{CaptureError, CaptureType};
use std::collections::HashMap;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, proxy};

const PORTAL_DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_REQUEST_PATH_PREFIX: &str = "/org/freedesktop/portal/desktop/request";
const PORTAL_OPTION_HANDLE_TOKEN_KEY: &str = "handle_token";
const PORTAL_HANDLE_RANDOM_BYTES: usize = 16;
const LOWERCASE_HEX: &[u8; 16] = b"0123456789abcdef";

/// D-Bus proxy for the xdg-desktop-portal Screenshot interface.
#[proxy(
    interface = "org.freedesktop.portal.Screenshot",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait Screenshot {
    /// Maximum Screenshot interface version supported by the selected portal backend.
    #[zbus(property, name = "version")]
    fn version(&self) -> zbus::Result<u32>;

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
    /// * `results` - Dictionary containing the screenshot URI result key
    #[zbus(signal)]
    fn response(&self, response: u32, results: HashMap<String, OwnedValue>) -> zbus::Result<()>;
}

struct PortalAttempt {
    label: &'static str,
    options: HashMap<String, zbus::zvariant::Value<'static>>,
}

const PORTAL_RESULT_URI_KEY: &str = "uri";
const PORTAL_OPTION_INTERACTIVE_KEY: &str = "interactive";

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
            Err(err @ CaptureError::Cancelled(_)) => return Err(err),
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
    mut options: HashMap<String, zbus::zvariant::Value<'static>>,
) -> Result<String, CaptureError> {
    let handle_token = next_handle_token()?;
    let request_path = portal_request_path(connection, &handle_token)?;
    options.insert(
        PORTAL_OPTION_HANDLE_TOKEN_KEY.to_string(),
        handle_token.into(),
    );
    log::debug!("Calling portal screenshot with options: {:?}", options);

    // Portal backends may complete non-interactive screenshots before the
    // Screenshot method reply reaches us. Subscribe at the predicted request
    // path first so that fast Response signals cannot be lost.
    let request_proxy = RequestProxy::builder(connection)
        .destination(PORTAL_DESTINATION)
        .map_err(CaptureError::DBusError)?
        .path(request_path.clone())
        .map_err(CaptureError::DBusError)?
        .build()
        .await
        .map_err(CaptureError::DBusError)?;
    let mut response_stream = request_proxy
        .receive_response()
        .await
        .map_err(CaptureError::DBusError)?;
    let returned_path = proxy
        .screenshot("", options)
        .await
        .map_err(map_portal_call_error)?;

    log::info!("Screenshot request created: {:?}", returned_path);

    log::debug!("Waiting for Response signal...");

    // Most portals honor handle_token, which lets us install the signal match
    // before calling Screenshot. Older implementations may return a different
    // path; switch to that path as required by the Request compatibility
    // contract instead of rejecting an otherwise valid request.
    let response_signal = if returned_path == request_path {
        crate::zbus_stream::next(&mut response_stream).await
    } else {
        log::warn!(
            "Screenshot portal returned a different request path; updating Response subscription"
        );
        let returned_request_proxy = RequestProxy::builder(connection)
            .destination(PORTAL_DESTINATION)
            .map_err(CaptureError::DBusError)?
            .path(returned_path)
            .map_err(CaptureError::DBusError)?
            .build()
            .await
            .map_err(CaptureError::DBusError)?;
        let mut returned_response_stream = returned_request_proxy
            .receive_response()
            .await
            .map_err(CaptureError::DBusError)?;
        crate::zbus_stream::next(&mut returned_response_stream).await
    }
    .ok_or_else(|| CaptureError::InvalidResponse("No Response signal received".to_string()))?;

    let args = response_signal.args().map_err(|e| {
        CaptureError::InvalidResponse(format!("Failed to parse response args: {}", e))
    })?;

    log::debug!(
        "Response signal received: code={}, result_keys={:?}",
        args.response,
        args.results.keys().collect::<Vec<_>>()
    );

    parse_response(args.response, &args.results)
}

fn next_handle_token() -> Result<String, CaptureError> {
    let mut random = [0_u8; PORTAL_HANDLE_RANDOM_BYTES];
    getrandom::fill(&mut random).map_err(|error| {
        CaptureError::InvalidResponse(format!(
            "Failed to generate a secure portal handle token: {error}"
        ))
    })?;
    Ok(handle_token_from_random(&random))
}

fn handle_token_from_random(random: &[u8; PORTAL_HANDLE_RANDOM_BYTES]) -> String {
    let mut token = String::with_capacity("wayscriber_".len() + random.len() * 2);
    token.push_str("wayscriber_");
    for byte in random {
        token.push(char::from(LOWERCASE_HEX[usize::from(byte >> 4)]));
        token.push(char::from(LOWERCASE_HEX[usize::from(byte & 0x0f)]));
    }
    token
}

fn portal_request_path(
    connection: &Connection,
    handle_token: &str,
) -> Result<OwnedObjectPath, CaptureError> {
    let unique_name = connection.unique_name().ok_or_else(|| {
        CaptureError::InvalidResponse("Session bus connection has no unique D-Bus name".to_string())
    })?;
    portal_request_path_for_unique_name(unique_name.as_str(), handle_token)
}

fn portal_request_path_for_unique_name(
    unique_name: &str,
    handle_token: &str,
) -> Result<OwnedObjectPath, CaptureError> {
    if handle_token.is_empty()
        || !handle_token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(CaptureError::InvalidResponse(
            "Portal handle token is not a valid D-Bus object-path element".to_string(),
        ));
    }
    let sender = unique_name.trim_start_matches(':').replace('.', "_");
    OwnedObjectPath::try_from(format!(
        "{PORTAL_REQUEST_PATH_PREFIX}/{sender}/{handle_token}"
    ))
    .map_err(|error| CaptureError::InvalidResponse(format!("Invalid portal request path: {error}")))
}

fn parse_response(
    response_code: u32,
    results: &HashMap<String, OwnedValue>,
) -> Result<String, CaptureError> {
    // Check response code (0 = success, 1 = cancelled, 2 = other error).
    match response_code {
        0 => {
            // Success - extract URI from results.
            let uri_value = results.get(PORTAL_RESULT_URI_KEY).ok_or_else(|| {
                CaptureError::InvalidResponse(format!(
                    "No '{PORTAL_RESULT_URI_KEY}' field in response"
                ))
            })?;

            // Extract string from OwnedValue.
            let uri_str: &str = uri_value.downcast_ref().map_err(|e| {
                CaptureError::InvalidResponse(format!("URI is not a string: {}", e))
            })?;

            log::info!("Screenshot captured successfully");
            Ok(uri_str.to_string())
        }
        1 => {
            log::info!("Screenshot cancelled by user");
            Err(CaptureError::Cancelled(
                "portal screenshot request was cancelled by the user".to_string(),
            ))
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
    let message = err.to_string();
    let lowercase_message = message.to_ascii_lowercase();
    if lowercase_message.contains("cancelled") || lowercase_message.contains("canceled") {
        log::info!("Portal screenshot call was cancelled");
        CaptureError::Cancelled("portal screenshot request was cancelled".to_string())
    } else if lowercase_message.contains("denied") {
        log::warn!("Portal screenshot permission was denied");
        CaptureError::PermissionDenied
    } else {
        log::error!("Portal screenshot call failed: {err}");
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
            options.insert(PORTAL_OPTION_INTERACTIVE_KEY.to_string(), false.into());
        }
        CaptureType::ActiveWindow => {
            options.insert(PORTAL_OPTION_INTERACTIVE_KEY.to_string(), true.into());
        }
        CaptureType::Selection { .. } => {
            // Interactive mode for selection.
            options.insert(PORTAL_OPTION_INTERACTIVE_KEY.to_string(), true.into());
        }
    }

    options
}

/// Active-window capture options (user picks window interactively).
fn build_active_window_interactive_options() -> HashMap<String, zbus::zvariant::Value<'static>> {
    let mut options = HashMap::new();
    options.insert(PORTAL_OPTION_INTERACTIVE_KEY.to_string(), true.into());
    options
}

/// Check if xdg-desktop-portal is available on the system.
pub async fn is_portal_available() -> bool {
    match Connection::session().await {
        Ok(connection) => {
            let Ok(proxy) = ScreenshotProxy::new(&connection).await else {
                return false;
            };
            proxy.version().await.is_ok()
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
            options.get(PORTAL_OPTION_INTERACTIVE_KEY),
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
            options.get(PORTAL_OPTION_INTERACTIVE_KEY),
            Some(&zbus::zvariant::Value::from(true))
        );
    }

    #[test]
    fn test_portal_attempts_active_window_uses_interactive_only() {
        let attempts = portal_attempts(CaptureType::ActiveWindow);
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].label, "active-window-interactive");
        assert_eq!(
            attempts[0].options.get(PORTAL_OPTION_INTERACTIVE_KEY),
            Some(&zbus::zvariant::Value::from(true))
        );
        assert_eq!(attempts[0].options.len(), 1);
    }

    #[test]
    fn test_build_active_window_interactive_options() {
        let options = build_active_window_interactive_options();

        assert_eq!(
            options.get(PORTAL_OPTION_INTERACTIVE_KEY),
            Some(&zbus::zvariant::Value::from(true))
        );
    }

    #[test]
    fn portal_request_path_uses_the_dbus_unique_name_and_handle_token() {
        assert_eq!(
            portal_request_path_for_unique_name(":1.42", "wayscriber_7_9")
                .expect("valid request path")
                .as_str(),
            "/org/freedesktop/portal/desktop/request/1_42/wayscriber_7_9"
        );
    }

    #[test]
    fn portal_request_path_rejects_an_invalid_handle_token() {
        assert!(
            portal_request_path_for_unique_name(":1.42", "invalid/token").is_err(),
            "a handle token must remain one D-Bus object-path element"
        );
    }

    #[test]
    fn portal_handle_token_is_a_128_bit_random_object_path_element() {
        let token = handle_token_from_random(&[
            0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76,
            0x54, 0x32,
        ]);

        assert_eq!(token, "wayscriber_000123456789abcdeffedcba98765432");
        assert!(portal_request_path_for_unique_name(":1.42", &token).is_ok());
    }

    #[test]
    fn generated_portal_handle_tokens_have_independent_random_suffixes() -> Result<(), CaptureError>
    {
        let first = next_handle_token()?;
        let second = next_handle_token()?;

        assert_ne!(first, second);
        assert_eq!(
            first.len(),
            "wayscriber_".len() + PORTAL_HANDLE_RANDOM_BYTES * 2
        );
        assert_eq!(second.len(), first.len());
        assert!(portal_request_path_for_unique_name(":1.42", &first).is_ok());
        assert!(portal_request_path_for_unique_name(":1.42", &second).is_ok());
        Ok(())
    }

    #[test]
    fn portal_response_code_one_preserves_user_cancellation() {
        let error = parse_response(1, &HashMap::new()).expect_err("response 1 must cancel");
        assert!(matches!(error, CaptureError::Cancelled(_)));
    }
}
