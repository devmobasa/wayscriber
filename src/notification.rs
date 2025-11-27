#![allow(clippy::too_many_arguments)]
//! System notifications via freedesktop D-Bus.

#[cfg(feature = "dbus")]
mod real {
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
                "Wayscriber",
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
}

#[cfg(not(feature = "dbus"))]
mod real {
    #[cfg_attr(not(feature = "dbus"), allow(dead_code))]
    pub async fn send_notification(
        _summary: &str,
        _body: &str,
        _icon: Option<&str>,
    ) -> Result<(), String> {
        Ok(())
    }

    #[cfg_attr(not(feature = "dbus"), allow(dead_code))]
    pub fn send_notification_async(
        _runtime_handle: &tokio::runtime::Handle,
        _summary: String,
        _body: String,
        _icon: Option<String>,
    ) {
        // no-op without D-Bus
    }
}

#[allow(unused_imports)]
pub use real::{send_notification, send_notification_async};
