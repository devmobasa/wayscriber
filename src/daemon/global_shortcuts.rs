use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

#[cfg(feature = "portal")]
use anyhow::{Context, Result, anyhow};
#[cfg(feature = "portal")]
use futures::StreamExt;
#[cfg(feature = "portal")]
use log::{debug, info, warn};
#[cfg(feature = "portal")]
use std::collections::HashMap;
#[cfg(feature = "portal")]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(feature = "portal")]
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
#[cfg(feature = "portal")]
use zbus::{Connection, proxy};

const DEFAULT_PREFERRED_TRIGGER: &str = "<Ctrl><Shift>g";
const DEFAULT_PORTAL_APP_ID: &str = "wayscriber";

#[cfg(feature = "portal")]
const TOGGLE_SHORTCUT_ID: &str = "toggle-overlay";
#[cfg(feature = "portal")]
const TOGGLE_SHORTCUT_DESCRIPTION: &str = "Toggle Wayscriber overlay";
#[cfg(feature = "portal")]
const PORTAL_REQUEST_POLL_INTERVAL_MS: u64 = 100;

pub(super) fn start_global_shortcuts_listener(
    toggle_flag: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
    activation_token_slot: Arc<Mutex<Option<String>>>,
) -> Option<JoinHandle<()>> {
    #[cfg(feature = "portal")]
    {
        let listener_quit_flag = quit_flag.clone();
        let preferred_trigger = std::env::var("WAYSCRIBER_PORTAL_SHORTCUT")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_PREFERRED_TRIGGER.to_string());
        let portal_app_id = std::env::var("WAYSCRIBER_PORTAL_APP_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_PORTAL_APP_ID.to_string());

        Some(std::thread::spawn(move || {
            let runtime = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(err) => {
                    warn!(
                        "Global shortcuts portal listener disabled: failed to create Tokio runtime: {}",
                        err
                    );
                    return;
                }
            };

            runtime.block_on(async move {
                if let Err(err) = run_listener(
                    toggle_flag,
                    quit_flag,
                    activation_token_slot,
                    preferred_trigger,
                    portal_app_id,
                )
                .await
                {
                    if listener_quit_flag.load(Ordering::Acquire) {
                        info!(
                            "Global shortcuts portal listener stopped during shutdown: {}",
                            err
                        );
                    } else {
                        warn!("Global shortcuts portal listener disabled: {}", err);
                    }
                }
            });
        }))
    }
    #[cfg(not(feature = "portal"))]
    {
        let _ = (toggle_flag, quit_flag, activation_token_slot);
        None
    }
}

#[cfg(feature = "portal")]
#[proxy(
    interface = "org.freedesktop.portal.GlobalShortcuts",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait GlobalShortcuts {
    async fn create_session(
        &self,
        options: HashMap<String, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;

    async fn bind_shortcuts(
        &self,
        session_handle: zbus::zvariant::ObjectPath<'_>,
        shortcuts: Vec<(String, HashMap<String, zbus::zvariant::Value<'_>>)>,
        parent_window: &str,
        options: HashMap<String, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;

    #[zbus(signal)]
    fn activated(
        &self,
        session_handle: zbus::zvariant::ObjectPath<'_>,
        shortcut_id: &str,
        timestamp: u64,
        options: HashMap<String, OwnedValue>,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    fn version(&self) -> zbus::Result<u32>;
}

#[cfg(feature = "portal")]
#[proxy(
    interface = "org.freedesktop.host.portal.Registry",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait HostPortalRegistry {
    async fn register(
        &self,
        app_id: &str,
        options: HashMap<String, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<()>;
}

#[cfg(feature = "portal")]
#[proxy(
    interface = "org.freedesktop.portal.Request",
    default_service = "org.freedesktop.portal.Desktop"
)]
trait Request {
    #[zbus(signal)]
    fn response(&self, response: u32, results: HashMap<String, OwnedValue>) -> zbus::Result<()>;
}

#[cfg(feature = "portal")]
#[proxy(
    interface = "org.freedesktop.portal.Session",
    default_service = "org.freedesktop.portal.Desktop"
)]
trait Session {
    async fn close(&self) -> zbus::Result<()>;
}

#[cfg(feature = "portal")]
async fn run_listener(
    toggle_flag: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
    activation_token_slot: Arc<Mutex<Option<String>>>,
    preferred_trigger: String,
    portal_app_id: String,
) -> Result<()> {
    let connection = Connection::session()
        .await
        .context("failed to connect to session D-Bus")?;
    register_host_portal_app_id(&connection, &portal_app_id).await?;
    let proxy = GlobalShortcutsProxy::new(&connection)
        .await
        .context("org.freedesktop.portal.GlobalShortcuts unavailable")?;
    match proxy.version().await {
        Ok(version) => {
            debug!("GlobalShortcuts portal interface version {}", version);
            if version < 2 {
                warn!(
                    "GlobalShortcuts portal version {} lacks reliable activation token support; overlay focus may require notification click",
                    version
                );
            }
        }
        Err(err) => {
            warn!(
                "Failed to read GlobalShortcuts portal interface version: {}",
                err
            );
        }
    }

    let session_handle =
        create_global_shortcuts_session(&connection, &proxy, &portal_app_id, quit_flag.as_ref())
            .await?;
    bind_toggle_shortcut(
        &connection,
        &proxy,
        &session_handle,
        &preferred_trigger,
        quit_flag.as_ref(),
    )
    .await?;

    info!(
        "Global shortcuts portal listener ready (app_id '{}', shortcut id '{}', preferred trigger '{}')",
        portal_app_id, TOGGLE_SHORTCUT_ID, preferred_trigger
    );

    let mut activated_stream = proxy
        .receive_activated()
        .await
        .context("failed to subscribe to GlobalShortcuts.Activated")?;

    loop {
        tokio::select! {
            maybe_signal = activated_stream.next() => {
                let Some(signal) = maybe_signal else {
                    return Err(anyhow!("GlobalShortcuts.Activated stream ended unexpectedly"));
                };
                let args = signal.args().context("failed to parse Activated signal args")?;
                if args.shortcut_id != TOGGLE_SHORTCUT_ID {
                    continue;
                }

                let activation_token = extract_activation_token(&args.options);
                if let Some(token) = activation_token {
                    info!("Global shortcut activated; activation_token received");
                    let mut slot = lock_token_slot(&activation_token_slot);
                    *slot = Some(token);
                } else {
                    let option_keys: Vec<&str> =
                        args.options.keys().map(|key| key.as_str()).collect();
                    warn!(
                        "Global shortcut activated without activation_token; focus may require manual click (options keys: {:?})",
                        option_keys
                    );
                }
                toggle_flag.store(true, Ordering::Release);
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                if quit_flag.load(Ordering::Acquire) {
                    break;
                }
            }
        }
    }

    if let Err(err) = close_global_shortcuts_session(&connection, &session_handle).await {
        warn!("Failed to close GlobalShortcuts session cleanly: {}", err);
    }

    Ok(())
}

#[cfg(feature = "portal")]
async fn register_host_portal_app_id(connection: &Connection, app_id: &str) -> Result<()> {
    let registry = HostPortalRegistryProxy::new(connection)
        .await
        .context("org.freedesktop.host.portal.Registry unavailable")?;
    registry
        .register(app_id, HashMap::new())
        .await
        .with_context(|| format!("host portal app-id registration failed for '{}'", app_id))?;
    debug!("Registered host portal app-id '{}'", app_id);
    Ok(())
}

#[cfg(feature = "portal")]
async fn create_global_shortcuts_session(
    connection: &Connection,
    proxy: &GlobalShortcutsProxy<'_>,
    portal_app_id: &str,
    quit_flag: &AtomicBool,
) -> Result<OwnedObjectPath> {
    let mut options: HashMap<String, Value<'static>> = HashMap::new();
    options.insert(
        "handle_token".to_string(),
        Value::from(make_handle_token("wayscribergsreq")),
    );
    options.insert(
        "session_handle_token".to_string(),
        Value::from(make_handle_token("wayscribergssess")),
    );
    options.insert("app_id".to_string(), Value::from(portal_app_id.to_string()));

    let request_path = proxy
        .create_session(options)
        .await
        .context("GlobalShortcuts.CreateSession call failed")?;

    let (response, results) =
        wait_for_request_response(connection, request_path, quit_flag).await?;
    if response != 0 {
        return Err(anyhow!(
            "GlobalShortcuts.CreateSession denied by portal (response code {})",
            response
        ));
    }

    let session_handle_value = results
        .get("session_handle")
        .ok_or_else(|| anyhow!("CreateSession response missing session_handle"))?;
    parse_object_path(session_handle_value)
        .context("failed to parse session_handle from CreateSession response")
}

#[cfg(feature = "portal")]
async fn bind_toggle_shortcut(
    connection: &Connection,
    proxy: &GlobalShortcutsProxy<'_>,
    session_handle: &OwnedObjectPath,
    preferred_trigger: &str,
    quit_flag: &AtomicBool,
) -> Result<()> {
    let mut shortcut_options: HashMap<String, Value<'static>> = HashMap::new();
    shortcut_options.insert(
        "description".to_string(),
        Value::from(TOGGLE_SHORTCUT_DESCRIPTION.to_string()),
    );
    shortcut_options.insert(
        "preferred_trigger".to_string(),
        Value::from(preferred_trigger.to_string()),
    );

    let shortcuts = vec![(TOGGLE_SHORTCUT_ID.to_string(), shortcut_options)];

    let mut bind_options: HashMap<String, Value<'static>> = HashMap::new();
    bind_options.insert(
        "handle_token".to_string(),
        Value::from(make_handle_token("wayscribergsbind")),
    );

    let session_path = zbus::zvariant::ObjectPath::try_from(session_handle.as_str())
        .map_err(|err| anyhow!("invalid GlobalShortcuts session path: {}", err))?;

    let request_path = proxy
        .bind_shortcuts(session_path, shortcuts, "", bind_options)
        .await
        .context("GlobalShortcuts.BindShortcuts call failed")?;

    let (response, _) = wait_for_request_response(connection, request_path, quit_flag).await?;
    if response != 0 {
        return Err(anyhow!(
            "GlobalShortcuts.BindShortcuts denied by portal (response code {})",
            response
        ));
    }

    Ok(())
}

#[cfg(feature = "portal")]
async fn wait_for_request_response(
    connection: &Connection,
    request_path: OwnedObjectPath,
    quit_flag: &AtomicBool,
) -> Result<(u32, HashMap<String, OwnedValue>)> {
    let request_proxy = RequestProxy::builder(connection)
        .path(request_path)
        .context("invalid portal request path")?
        .build()
        .await
        .context("failed to build Request proxy")?;

    let mut response_stream = request_proxy
        .receive_response()
        .await
        .context("failed to subscribe to Request.Response")?;
    loop {
        tokio::select! {
            maybe_signal = response_stream.next() => {
                let response_signal = maybe_signal
                    .ok_or_else(|| anyhow!("portal request completed without Response signal"))?;
                let args = response_signal
                    .args()
                    .context("failed to parse Request.Response signal arguments")?;
                return Ok((args.response, args.results.clone()));
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(PORTAL_REQUEST_POLL_INTERVAL_MS)) => {
                if quit_flag.load(Ordering::Acquire) {
                    return Err(anyhow!(
                        "shutdown requested while waiting for portal Request.Response"
                    ));
                }
            }
        }
    }
}

#[cfg(feature = "portal")]
async fn close_global_shortcuts_session(
    connection: &Connection,
    session_handle: &OwnedObjectPath,
) -> Result<()> {
    let proxy = SessionProxy::builder(connection)
        .path(session_handle.clone())
        .context("invalid GlobalShortcuts session path")?
        .build()
        .await
        .context("failed to build Session proxy")?;
    proxy
        .close()
        .await
        .context("GlobalShortcuts session close failed")
}

#[cfg(feature = "portal")]
fn parse_object_path(value: &OwnedValue) -> Result<OwnedObjectPath> {
    let path: &str = value
        .downcast_ref()
        .map_err(|err| anyhow!("session_handle is not a string/object-path: {}", err))?;
    OwnedObjectPath::try_from(path.to_string())
        .map_err(|err| anyhow!("invalid object path string '{}': {}", path, err))
}

#[cfg(feature = "portal")]
fn extract_activation_token(options: &HashMap<String, OwnedValue>) -> Option<String> {
    const TOKEN_KEYS: [&str; 5] = [
        "activation_token",
        "activation-token",
        "activationToken",
        "startup_id",
        "desktop-startup-id",
    ];

    for key in TOKEN_KEYS {
        if let Some(token) = options.get(key).and_then(owned_value_as_string) {
            return Some(token);
        }
    }

    options.iter().find_map(|(key, value)| {
        if !key.contains("token") {
            return None;
        }
        owned_value_as_string(value)
    })
}

#[cfg(feature = "portal")]
fn owned_value_as_string(value: &OwnedValue) -> Option<String> {
    let token: &str = value.downcast_ref().ok()?;
    Some(token.to_string())
}

#[cfg(feature = "portal")]
fn make_handle_token(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}_{}_{}", prefix, std::process::id(), nanos)
}

#[cfg(feature = "portal")]
fn lock_token_slot(slot: &Arc<Mutex<Option<String>>>) -> std::sync::MutexGuard<'_, Option<String>> {
    slot.lock().unwrap_or_else(|poisoned| {
        warn!("portal activation token slot mutex poisoned; recovering");
        poisoned.into_inner()
    })
}
