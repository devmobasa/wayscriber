use anyhow::{Context, Result};
use log::warn;

pub(super) fn open_url(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "xdg-open"
    };

    let mut cmd = std::process::Command::new(opener);
    if cfg!(target_os = "windows") {
        cmd.args(["/C", "start", ""]).arg(url);
    } else {
        cmd.arg(url);
    }
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    if let Err(err) = cmd.spawn() {
        warn!("Failed to open URL {}: {}", url, err);
    }
}

pub(super) fn copy_text_to_clipboard(text: &str) {
    if text.is_empty() {
        return;
    }
    let text = text.to_string();
    std::thread::spawn(move || {
        if copy_text_via_command(&text).is_ok() {
            return;
        }
        if let Err(err) = copy_text_via_library(&text) {
            warn!("Failed to copy commit id to clipboard: {}", err);
        }
    });
}

fn copy_text_via_library(text: &str) -> Result<()> {
    use wl_clipboard_rs::copy::{MimeType, Options, ServeRequests, Source};

    let mut opts = Options::new();
    opts.serve_requests(ServeRequests::Only(1));
    opts.copy(Source::Bytes(text.as_bytes().into()), MimeType::Text)
        .context("wl-clipboard-rs text copy failed")?;
    Ok(())
}

fn copy_text_via_command(text: &str) -> Result<()> {
    use std::io::Write;

    let mut child = std::process::Command::new("wl-copy")
        .arg("--type")
        .arg("text/plain")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to spawn wl-copy")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .context("Failed to write to wl-copy stdin")?;
    }

    let output = child
        .wait_with_output()
        .context("Failed to wait for wl-copy")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("wl-copy failed: {}", stderr.trim()));
    }

    Ok(())
}
