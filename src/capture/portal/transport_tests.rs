//! Real subscription transactions on an owned private session bus.

use super::*;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use zbus::message::Header;

struct PrivateBus {
    child: Child,
    address: String,
}

impl PrivateBus {
    fn start() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("portal transport tests require dbus-daemon");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        assert!(
            !address.trim().is_empty(),
            "private bus must publish its address"
        );
        Self {
            child,
            address: address.trim().to_owned(),
        }
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Clone, Copy)]
enum ResponseMode {
    Immediate,
    Deferred,
    Cancelled,
    Compatibility,
    Silent,
}

struct ScreenshotFixture {
    mode: ResponseMode,
    requested: tokio::sync::mpsc::UnboundedSender<OwnedObjectPath>,
    closed: Arc<AtomicUsize>,
}

struct RequestFixture {
    closed: Arc<AtomicUsize>,
}

#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl RequestFixture {
    fn close(&self) {
        self.closed.fetch_add(1, Ordering::SeqCst);
    }
}

#[zbus::interface(name = "org.freedesktop.portal.Screenshot")]
impl ScreenshotFixture {
    async fn screenshot(
        &self,
        _parent_window: &str,
        mut options: HashMap<String, OwnedValue>,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let token =
            String::try_from(options.remove(PORTAL_OPTION_HANDLE_TOKEN_KEY).unwrap()).unwrap();
        let sender = header
            .sender()
            .unwrap()
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let path = if matches!(self.mode, ResponseMode::Compatibility) {
            format!("{PORTAL_REQUEST_PATH_PREFIX}/{sender}/compatibility")
        } else {
            format!("{PORTAL_REQUEST_PATH_PREFIX}/{sender}/{token}")
        };
        let path = OwnedObjectPath::try_from(path).unwrap();
        connection
            .object_server()
            .at(
                path.clone(),
                RequestFixture {
                    closed: self.closed.clone(),
                },
            )
            .await
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        if matches!(self.mode, ResponseMode::Immediate) {
            emit_response(connection, &path, 0).await;
        }
        self.requested.send(path.clone()).unwrap();
        Ok(path)
    }
}

async fn emit_response(connection: &Connection, path: &OwnedObjectPath, code: u32) {
    let results: HashMap<String, OwnedValue> = if code == 0 {
        HashMap::from([(
            PORTAL_RESULT_URI_KEY.into(),
            zbus::zvariant::Str::from("file:///tmp/private-portal.png").into(),
        )])
    } else {
        HashMap::new()
    };
    connection
        .emit_signal(
            None::<&str>,
            path.clone(),
            "org.freedesktop.portal.Request",
            "Response",
            &(code, results),
        )
        .await
        .unwrap();
}

async fn fixture(
    bus: &PrivateBus,
    mode: ResponseMode,
) -> (
    Connection,
    Connection,
    tokio::sync::mpsc::UnboundedReceiver<OwnedObjectPath>,
    Arc<AtomicUsize>,
) {
    let (requested, receiver) = tokio::sync::mpsc::unbounded_channel();
    let closed = Arc::new(AtomicUsize::new(0));
    let service = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name(PORTAL_DESTINATION)
        .unwrap()
        .serve_at(
            "/org/freedesktop/portal/desktop",
            ScreenshotFixture {
                mode,
                requested,
                closed: closed.clone(),
            },
        )
        .unwrap()
        .build()
        .await
        .unwrap();
    let client = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    (service, client, receiver, closed)
}

async fn capture(client: Connection) -> Result<String, CaptureError> {
    let proxy = ScreenshotProxy::new(&client).await.unwrap();
    capture_once(
        &client,
        &proxy,
        build_portal_options(CaptureType::FullScreen),
    )
    .await
}

/// Inspect the private bus's actual match rules before a deferred signal.
/// This avoids timing guesses, especially after a compatibility path reply.
async fn wait_for_response_subscription(
    service: &Connection,
    client: &Connection,
    path: &OwnedObjectPath,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let reply = service
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus.Debug.Stats"),
                "GetAllMatchRules",
                &(),
            )
            .await
            .unwrap();
        let rules: HashMap<String, Vec<String>> = reply.body().deserialize().unwrap();
        let matched = rules
            .get(client.unique_name().unwrap().as_str())
            .is_some_and(|rules| {
                rules
                    .iter()
                    .any(|rule| rule.contains(path.as_str()) && rule.contains("Response"))
            });
        if matched {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "client did not subscribe at {path}"
        );
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn portal_transport_immediate_response_before_method_reply_is_not_lost() {
    let bus = PrivateBus::start();
    let (_service, client, _requests, closed) = fixture(&bus, ResponseMode::Immediate).await;
    let result = tokio::time::timeout(Duration::from_secs(2), capture(client))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result, "file:///tmp/private-portal.png");
    assert_eq!(closed.load(Ordering::SeqCst), 0);
}

async fn deferred_case(mode: ResponseMode, code: u32) -> Result<String, CaptureError> {
    let bus = PrivateBus::start();
    let (service, client, mut requests, closed) = fixture(&bus, mode).await;
    let operation = tokio::spawn(capture(client.clone()));
    let path = requests.recv().await.unwrap();
    wait_for_response_subscription(&service, &client, &path).await;
    emit_response(&service, &path, code).await;
    let result = tokio::time::timeout(Duration::from_secs(2), operation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(closed.load(Ordering::SeqCst), 0);
    result
}

#[tokio::test]
async fn portal_transport_delayed_response_reaches_the_original_subscription() {
    assert_eq!(
        deferred_case(ResponseMode::Deferred, 0).await.unwrap(),
        "file:///tmp/private-portal.png"
    );
}

#[tokio::test]
async fn portal_transport_compatibility_path_installs_a_new_subscription() {
    assert_eq!(
        deferred_case(ResponseMode::Compatibility, 0).await.unwrap(),
        "file:///tmp/private-portal.png"
    );
}

#[tokio::test]
async fn portal_transport_cancellation_remains_distinct_from_failure() {
    assert!(matches!(
        deferred_case(ResponseMode::Cancelled, 1).await,
        Err(CaptureError::Cancelled(_))
    ));
}

#[tokio::test]
async fn portal_transport_silent_request_times_out_and_closes_once() {
    let bus = PrivateBus::start();
    let (_service, client, _requests, closed) = fixture(&bus, ResponseMode::Silent).await;
    assert!(
        matches!(capture(client).await, Err(CaptureError::PortalTimeout(limit)) if limit == PORTAL_RESPONSE_TIMEOUT)
    );
    assert_eq!(closed.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn portal_transport_bus_disconnect_is_a_terminal_failure() {
    let mut bus = PrivateBus::start();
    let (_service, client, mut requests, _closed) = fixture(&bus, ResponseMode::Silent).await;
    let operation = tokio::spawn(capture(client));
    requests.recv().await.unwrap();
    bus.child.kill().unwrap();
    bus.child.wait().unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(2), operation)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        outcome,
        Err(CaptureError::DBusError(_) | CaptureError::InvalidResponse(_))
    ));
}
