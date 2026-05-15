use super::super::WAYSCRIBER_SELECTION_MIME;
use std::process::{Child, Command, Output, Stdio};

pub(super) trait ClipboardCommandRunner {
    fn list_types(&self) -> std::io::Result<Output>;
    fn spawn_paste_mime(&self, mime_type: &str) -> std::io::Result<Child>;
    fn spawn_copy_selection(&self) -> std::io::Result<Child>;
}

pub(super) struct WlClipboardCommandRunner;

impl ClipboardCommandRunner for WlClipboardCommandRunner {
    fn list_types(&self) -> std::io::Result<Output> {
        Command::new("wl-paste")
            .arg("--list-types")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    }

    fn spawn_paste_mime(&self, mime_type: &str) -> std::io::Result<Child> {
        Command::new("wl-paste")
            .arg("--type")
            .arg(mime_type)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn spawn_copy_selection(&self) -> std::io::Result<Child> {
        Command::new("wl-copy")
            .arg("--type")
            .arg(WAYSCRIBER_SELECTION_MIME)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
    }
}
