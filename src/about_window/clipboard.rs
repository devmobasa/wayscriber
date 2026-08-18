use anyhow::Result;
use log::warn;

pub(super) fn open_url(url: &str) -> Result<()> {
    open_url_with(url, |invocation| {
        crate::desktop_open::open_in_background(invocation.clone())
    })
}

fn open_url_with(
    url: &str,
    open: impl FnOnce(&crate::desktop_open::DesktopOpenInvocation) -> Result<()>,
) -> Result<()> {
    let invocation = crate::desktop_open::about_url(url)?;
    open(&invocation)
}

pub(super) fn copy_text_to_clipboard(text: &str) {
    if text.is_empty() {
        return;
    }

    let text = text.to_string();
    std::thread::spawn(move || {
        if let Err(err) = copy_text_with_command(&text, copy_text_via_command) {
            warn!("Failed to copy About text to clipboard: {err:#}");
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
    crate::clipboard_text::copy_text_via_command(text).map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn open_url_builds_the_broker_ready_desktop_open_argv() {
        let mut observed = None;

        open_url_with("https://wayscriber.com/report#d=abc", |invocation| {
            observed = Some((
                invocation.program().to_owned(),
                invocation.arguments().to_vec(),
            ));
            Ok(())
        })
        .unwrap();

        let (program, arguments) = observed.expect("trusted URL reaches the open adapter");
        assert!(!matches!(program.to_str(), Some("sh" | "bash" | "cmd")));
        assert_eq!(
            arguments,
            [std::ffi::OsString::from(
                "https://wayscriber.com/report#d=abc"
            )]
        );
    }

    #[test]
    fn open_url_refuses_untrusted_hosts_before_spawning() {
        let spawn_calls = AtomicUsize::new(0);

        for url in [
            "http://wayscriber.com/report",
            "https://wayscriber.com.example/report",
            "https://www.wayscriber.com/report",
            "https://example.com/report",
        ] {
            let result = open_url_with(url, |_| {
                spawn_calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            });
            assert!(result.is_err(), "unexpectedly accepted {url}");
        }

        assert_eq!(spawn_calls.load(Ordering::SeqCst), 0);
    }

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
