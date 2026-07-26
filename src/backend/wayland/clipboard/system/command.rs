use super::super::WAYSCRIBER_SELECTION_MIME;
use crate::process_broker::{BrokerOutput, HelperKind, ProcessBrokerHandle};
use std::ffi::OsStr;
use std::time::Duration;

pub(super) trait ClipboardCommandRunner {
    fn list_types(&self) -> anyhow::Result<BrokerOutput>;
    fn paste_mime(
        &self,
        mime_type: &str,
        timeout: Duration,
        output_cap: usize,
    ) -> anyhow::Result<BrokerOutput>;
    fn copy_selection(&self, payload: &[u8], timeout: Duration) -> anyhow::Result<BrokerOutput>;
}

pub(super) struct WlClipboardCommandRunner<'a> {
    process_broker: &'a ProcessBrokerHandle,
}

impl<'a> WlClipboardCommandRunner<'a> {
    pub(super) fn new(process_broker: &'a ProcessBrokerHandle) -> Self {
        Self { process_broker }
    }
}

impl ClipboardCommandRunner for WlClipboardCommandRunner<'_> {
    fn list_types(&self) -> anyhow::Result<BrokerOutput> {
        self.process_broker.run(
            HelperKind::WlPaste,
            OsStr::new("wl-paste"),
            [OsStr::new("--list-types")],
            Vec::new(),
            Duration::from_secs(5),
            64 * 1024,
        )
    }

    fn paste_mime(
        &self,
        mime_type: &str,
        timeout: Duration,
        output_cap: usize,
    ) -> anyhow::Result<BrokerOutput> {
        self.process_broker.run_prefix(
            HelperKind::WlPaste,
            OsStr::new("wl-paste"),
            [OsStr::new("--type"), OsStr::new(mime_type)],
            Vec::new(),
            timeout,
            output_cap,
        )
    }

    fn copy_selection(&self, payload: &[u8], timeout: Duration) -> anyhow::Result<BrokerOutput> {
        self.process_broker.publish(
            HelperKind::WlCopy,
            OsStr::new("wl-copy"),
            [OsStr::new("--type"), OsStr::new(WAYSCRIBER_SELECTION_MIME)],
            payload.to_vec(),
            timeout,
        )
    }
}
