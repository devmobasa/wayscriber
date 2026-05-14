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
        if let Err(err) = copy_text_with_command(&text, copy_text_via_command) {
            warn!("Failed to copy commit id to clipboard: {}", err);
        }
    });
}

fn copy_text_with_command<C>(text: &str, command_copy: C) -> Result<()>
where
    C: Fn(&str) -> Result<()>,
{
    if text.is_empty() {
        return Ok(());
    }
    command_copy(text)
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn copy_text_with_command_short_circuits_for_empty_text() {
        let command_calls = AtomicUsize::new(0);

        copy_text_with_command("", |_| {
            command_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .unwrap();

        assert_eq!(command_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn copy_text_with_command_uses_command_when_available() {
        let command_calls = AtomicUsize::new(0);

        copy_text_with_command("abc123", |_| {
            command_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .unwrap();

        assert_eq!(command_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn copy_text_with_command_returns_command_error() {
        let err = copy_text_with_command("abc123", |_| Err(anyhow::anyhow!("command failed")))
            .unwrap_err();

        assert!(err.to_string().contains("command failed"));
    }
}
