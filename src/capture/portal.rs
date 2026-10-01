//! xdg-desktop-portal integration for screenshot capture.

use super::types::{CaptureError, CaptureType};
use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, proxy};

const PORTAL_DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_REQUEST_PATH_PREFIX: &str = "/org/freedesktop/portal/desktop/request";
const PORTAL_OPTION_HANDLE_TOKEN_KEY: &str = "handle_token";
const PORTAL_HANDLE_RANDOM_BYTES: usize = 16;
const LOWERCASE_HEX: &[u8; 16] = b"0123456789abcdef";

/// How long a non-interactive request may wait for the portal to answer. It
/// matches the limit the freeze and zoom portal fallbacks use.
const PORTAL_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
/// How long an interactive request may wait while the user picks a window or
/// region. It matches the limit for an interactive `slurp` selection.
const PORTAL_INTERACTIVE_RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);
/// How long closing an unanswered request may take before it is abandoned.
const PORTAL_CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

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

    /// Close the request, dismissing any dialog it still shows.
    fn close(&self) -> zbus::Result<()>;
}

struct PortalAttempt {
    label: &'static str,
    options: HashMap<String, zbus::zvariant::Value<'static>>,
}

const PORTAL_RESULT_URI_KEY: &str = "uri";
const PORTAL_OPTION_INTERACTIVE_KEY: &str = "interactive";
const PORTAL_ERROR_CANCELLED: &str = "org.freedesktop.portal.Error.Cancelled";
const PORTAL_ERROR_NOT_ALLOWED: &str = "org.freedesktop.portal.Error.NotAllowed";
const DBUS_ERROR_ACCESS_DENIED: &str = "org.freedesktop.DBus.Error.AccessDenied";

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
    // The overlay stays hidden until this returns, so a portal that accepts
    // the request and never answers must not keep it hidden for good.
    let response_timeout = response_timeout(&options);
    let deadline = tokio::time::Instant::now() + response_timeout;

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
    let returned_path = within_deadline(
        deadline,
        response_timeout,
        proxy.screenshot("", options),
        close_request(connection, request_path.clone()),
    )
    .await?
    .map_err(map_portal_call_error)?;

    log::info!("Screenshot request created: {:?}", returned_path);

    log::debug!("Waiting for Response signal...");

    // Most portals honor handle_token, which lets us install the signal match
    // before calling Screenshot. Older implementations may return a different
    // path; switch to that path as required by the Request compatibility
    // contract instead of rejecting an otherwise valid request.
    let close = close_request(connection, returned_path.clone());
    let response_signal = if returned_path == request_path {
        within_deadline(
            deadline,
            response_timeout,
            crate::zbus_stream::next(&mut response_stream),
            close,
        )
        .await?
    } else {
        log::warn!(
            "Screenshot portal returned a different request path; updating Response subscription"
        );
        let returned_request_proxy = RequestProxy::builder(connection)
            .destination(PORTAL_DESTINATION)
            .map_err(CaptureError::DBusError)?
            .path(returned_path.clone())
            .map_err(CaptureError::DBusError)?
            .build()
            .await
            .map_err(CaptureError::DBusError)?;
        let mut returned_response_stream = returned_request_proxy
            .receive_response()
            .await
            .map_err(CaptureError::DBusError)?;
        within_deadline(
            deadline,
            response_timeout,
            crate::zbus_stream::next(&mut returned_response_stream),
            close,
        )
        .await?
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

/// The time a request may wait for the portal's answer.
fn response_timeout(options: &HashMap<String, zbus::zvariant::Value<'static>>) -> Duration {
    let interactive =
        options.get(PORTAL_OPTION_INTERACTIVE_KEY) == Some(&zbus::zvariant::Value::from(true));
    if interactive {
        PORTAL_INTERACTIVE_RESPONSE_TIMEOUT
    } else {
        PORTAL_RESPONSE_TIMEOUT
    }
}

/// Run one step of a portal request until `deadline`.
///
/// On expiry the request is closed, so its dialog does not outlive the
/// capture, and the capture fails instead of waiting forever.
async fn within_deadline<T>(
    deadline: tokio::time::Instant,
    limit: Duration,
    step: impl Future<Output = T>,
    close: impl Future<Output = ()>,
) -> Result<T, CaptureError> {
    match tokio::time::timeout_at(deadline, step).await {
        Ok(value) => Ok(value),
        Err(_) => {
            log::warn!(
                "Screenshot portal did not answer within {}s; closing the request",
                limit.as_secs()
            );
            close.await;
            Err(CaptureError::PortalTimeout(limit))
        }
    }
}

/// Ask the portal to close an unanswered request, without waiting long for it.
async fn close_request(connection: &Connection, path: OwnedObjectPath) {
    let close = async {
        RequestProxy::builder(connection)
            .destination(PORTAL_DESTINATION)?
            .path(path)?
            .build()
            .await?
            .close()
            .await
    };

    match tokio::time::timeout(PORTAL_CLOSE_TIMEOUT, close).await {
        Ok(Ok(())) => log::debug!("Closed the unanswered screenshot portal request"),
        Ok(Err(error)) => log::warn!("Could not close the screenshot portal request: {error}"),
        Err(_) => log::warn!("Closing the screenshot portal request timed out"),
    }
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
            Err(CaptureError::PortalResponse(code))
        }
    }
}

/// Map a failed `Screenshot` call by its D-Bus error name, never its text.
fn map_portal_call_error(err: zbus::Error) -> CaptureError {
    let error_name = match &err {
        zbus::Error::MethodError(name, _, _) => Some(name.as_str()),
        zbus::Error::FDO(fdo_error) if matches!(**fdo_error, zbus::fdo::Error::AccessDenied(_)) => {
            Some(DBUS_ERROR_ACCESS_DENIED)
        }
        _ => None,
    };

    match error_name {
        Some(PORTAL_ERROR_CANCELLED) => {
            log::info!("Portal screenshot call was cancelled");
            CaptureError::Cancelled("portal screenshot request was cancelled".to_string())
        }
        Some(PORTAL_ERROR_NOT_ALLOWED | DBUS_ERROR_ACCESS_DENIED) => {
            log::warn!("Portal screenshot permission was denied: {err}");
            CaptureError::PermissionDenied
        }
        _ => {
            log::error!("Portal screenshot call failed: {err}");
            CaptureError::DBusError(err)
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

    #[test]
    fn interactive_portal_requests_wait_longer_than_non_interactive_ones() {
        assert_eq!(
            response_timeout(&build_portal_options(CaptureType::FullScreen)),
            PORTAL_RESPONSE_TIMEOUT
        );
        assert_eq!(
            response_timeout(&build_portal_options(CaptureType::Selection {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            })),
            PORTAL_INTERACTIVE_RESPONSE_TIMEOUT
        );
        assert_eq!(
            response_timeout(&build_active_window_interactive_options()),
            PORTAL_INTERACTIVE_RESPONSE_TIMEOUT
        );
    }

    #[tokio::test]
    async fn an_unanswered_portal_request_is_closed_and_fails() {
        let limit = Duration::from_millis(20);
        let closed = std::sync::atomic::AtomicBool::new(false);

        let result = within_deadline(
            tokio::time::Instant::now() + limit,
            limit,
            std::future::pending::<()>(),
            async { closed.store(true, std::sync::atomic::Ordering::SeqCst) },
        )
        .await;

        let error = result.expect_err("a silent portal must not be awaited forever");
        assert!(matches!(error, CaptureError::PortalTimeout(_)), "{error}");
        assert_eq!(
            error.failure_kind(),
            crate::capture::CaptureFailureKind::PortalError
        );
        assert!(closed.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn an_answered_portal_request_is_left_open() {
        let limit = Duration::from_secs(5);
        let closed = std::sync::atomic::AtomicBool::new(false);

        let result = within_deadline(
            tokio::time::Instant::now() + limit,
            limit,
            std::future::ready(7),
            async { closed.store(true, std::sync::atomic::Ordering::SeqCst) },
        )
        .await;

        assert_eq!(result.expect("answered in time"), 7);
        assert!(!closed.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn portal_response_error_code_is_a_portal_error() {
        let error = parse_response(2, &HashMap::new()).expect_err("response 2 must fail");

        assert!(matches!(error, CaptureError::PortalResponse(2)));
        assert_eq!(
            error.failure_kind(),
            crate::capture::CaptureFailureKind::PortalError
        );
    }

    fn method_error(name: &str, detail: &str) -> zbus::Error {
        let call = zbus::Message::method_call("/org/freedesktop/portal/desktop", "Screenshot")
            .expect("method call builder")
            .build(&())
            .expect("method call");
        let reply = zbus::Message::error(&call.header(), name)
            .expect("error builder")
            .build(&(detail,))
            .expect("error reply");
        zbus::Error::from(reply)
    }

    #[test]
    fn portal_call_errors_are_mapped_by_dbus_error_name() {
        assert!(matches!(
            map_portal_call_error(method_error(PORTAL_ERROR_NOT_ALLOWED, "not allowed")),
            CaptureError::PermissionDenied
        ));
        assert!(matches!(
            map_portal_call_error(method_error(DBUS_ERROR_ACCESS_DENIED, "no")),
            CaptureError::PermissionDenied
        ));
        assert!(matches!(
            map_portal_call_error(method_error(PORTAL_ERROR_CANCELLED, "closed")),
            CaptureError::Cancelled(_)
        ));
    }

    #[test]
    fn portal_call_error_text_does_not_decide_the_mapping() {
        let error = map_portal_call_error(method_error(
            "org.freedesktop.portal.Error.Failed",
            "screenshot cancelled: access denied",
        ));

        assert!(matches!(error, CaptureError::DBusError(_)), "{error}");
    }
}

#[cfg(test)]
mod transport_tests;
