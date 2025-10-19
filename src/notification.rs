//! System notifications via freedesktop D-Bus.

use std::collections::HashMap;
use zbus::{Connection, proxy};

/// D-Bus interface for freedesktop Notifications.
#[proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    /// Send a notification.
    ///
    /// # Arguments
    /// * `app_name` - Application name
    /// * `replaces_id` - ID of notification to replace (0 for new)
    /// * `app_icon` - Icon name or path
    /// * `summary` - Notification title
    /// * `body` - Notification body text
    /// * `actions` - List of action identifiers and labels
    /// * `hints` - Additional metadata
    /// * `expire_timeout` - Timeout in milliseconds (-1 for default)
    ///
    /// # Returns
    /// Notification ID
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<&str>,
        hints: HashMap<&str, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
}

/// Send a system notification.
///
/// # Arguments
/// * `summary` - Notification title
/// * `body` - Notification body text
/// * `icon` - Optional icon name (defaults to "camera-photo")
pub async fn send_notification(
    summary: &str,
    body: &str,
    icon: Option<&str>,
) -> Result<(), String> {
    let connection = Connection::session()
        .await
        .map_err(|e| format!("Failed to connect to session bus: {}", e))?;

    let proxy = NotificationsProxy::new(&connection)
        .await
        .map_err(|e| format!("Failed to create notifications proxy: {}", e))?;

    let icon = icon.unwrap_or("camera-photo");
    let hints = HashMap::new();

    proxy
        .notify(
            "Hyprmarker",
            0,
            icon,
            summary,
            body,
            vec![],
            hints,
            3000, // 3 second timeout
        )
        .await
        .map_err(|e| format!("Failed to send notification: {}", e))?;

    Ok(())
}

/// Send a notification in the background (non-blocking).
///
/// Spawns a tokio task to send the notification and logs errors.
///
/// # Arguments
/// * `runtime_handle` - Handle to the tokio runtime
/// * `summary` - Notification title
/// * `body` - Notification body text
/// * `icon` - Optional icon name
pub fn send_notification_async(
    runtime_handle: &tokio::runtime::Handle,
    summary: String,
    body: String,
    icon: Option<String>,
) {
    runtime_handle.spawn(async move {
        let icon_ref = icon.as_deref();
        if let Err(e) = send_notification(&summary, &body, icon_ref).await {
            log::warn!("Failed to send notification: {}", e);
        }
    });
}
