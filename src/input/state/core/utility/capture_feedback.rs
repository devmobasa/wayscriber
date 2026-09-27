//! Compact capture-outcome toasts with Open folder and Copy path actions.

use super::super::base::{InputState, TextClipboardRequest, Toast, ToastCommand, ToastPriority};
use crate::domain::Action;
use std::path::Path;

/// One key for every capture outcome, so a new capture replaces the last one.
const CAPTURE_FEEDBACK_KEY: &str = "capture.feedback";
/// Toasts that offer buttons stay up long enough to reach them.
const CAPTURE_ACTIONS_DURATION_MS: u64 = 8000;
const OPEN_FOLDER_LABEL: &str = "Open folder";
const COPY_PATH_LABEL: &str = "Copy path";
/// Shown once the capture path reaches the clipboard.
const PATH_COPIED_MESSAGE: &str = "Path copied";

impl InputState {
    /// Announce a finished capture: the saved file's name (never its full
    /// path) and whether it reached the clipboard. A saved file adds "Open
    /// folder" (with its shortcut) and "Copy path" buttons.
    pub(crate) fn set_capture_feedback(
        &mut self,
        saved_path: Option<&Path>,
        copied_to_clipboard: bool,
    ) {
        self.set_last_capture_path(saved_path.map(Path::to_path_buf));

        let message = capture_feedback_message(
            saved_path,
            copied_to_clipboard,
            crate::paths::home_dir().as_deref(),
        );
        let (priority, toast) = if saved_path.is_some() {
            let open_folder = match self.action_binding_primary_label(Action::OpenCaptureFolder) {
                Some(binding) => format!("{OPEN_FOLDER_LABEL} · {binding}"),
                None => OPEN_FOLDER_LABEL.to_string(),
            };
            let toast = Toast::info(message)
                .duration_ms(CAPTURE_ACTIONS_DURATION_MS)
                .action(open_folder, Action::OpenCaptureFolder)
                .secondary_command(COPY_PATH_LABEL, ToastCommand::CopyLastCapturePath);
            (ToastPriority::Action, toast)
        } else {
            (ToastPriority::Info, Toast::info(message))
        };

        self.push_toast(priority, CAPTURE_FEEDBACK_KEY, toast);
    }

    /// Clipboard text request for the last saved capture's full path, which
    /// confirms itself once the copy lands. `None` when nothing was saved.
    pub(crate) fn last_capture_path_copy_request(&self) -> Option<TextClipboardRequest> {
        let path = self.last_capture_path()?;
        Some(TextClipboardRequest {
            text: path.display().to_string(),
            cut: None,
            confirmation: Some(PATH_COPIED_MESSAGE),
        })
    }
}

/// Toast text for a capture outcome, e.g.
/// `Saved screenshot_2026-09-25_213231.png · Copied to clipboard`.
pub(crate) fn capture_feedback_message(
    saved_path: Option<&Path>,
    copied_to_clipboard: bool,
    home: Option<&Path>,
) -> String {
    match (saved_path, copied_to_clipboard) {
        (Some(path), true) => format!(
            "Saved {} · Copied to clipboard",
            capture_display_name(path, home)
        ),
        (Some(path), false) => format!("Saved {}", capture_display_name(path, home)),
        (None, true) => "Copied to clipboard".to_string(),
        (None, false) => "Screenshot captured".to_string(),
    }
}

/// The file name alone; "Open folder" reveals where it lives. A path without
/// a usable file name is shown relative to the home directory instead.
fn capture_display_name(path: &Path, home: Option<&Path>) -> String {
    if let Some(name) = path.file_name() {
        return name.to_string_lossy().into_owned();
    }

    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::state::test_support::make_test_input_state;
    use std::path::PathBuf;

    const SAVED: &str =
        "/very/long/absolute/path/Pictures/Wayscriber/screenshot_2026-09-25_213231.png";

    #[test]
    fn messages_name_the_file_instead_of_its_full_path() {
        let saved = Path::new(SAVED);

        assert_eq!(
            capture_feedback_message(Some(saved), true, None),
            "Saved screenshot_2026-09-25_213231.png · Copied to clipboard"
        );
        assert_eq!(
            capture_feedback_message(Some(saved), false, None),
            "Saved screenshot_2026-09-25_213231.png"
        );
        assert_eq!(
            capture_feedback_message(None, true, None),
            "Copied to clipboard"
        );
        assert_eq!(
            capture_feedback_message(None, false, None),
            "Screenshot captured"
        );
    }

    #[test]
    fn a_path_without_a_file_name_falls_back_to_a_home_relative_path() {
        let home = Path::new("/home/user");

        assert_eq!(
            capture_display_name(Path::new("/home/user/Pictures/.."), Some(home)),
            "~/Pictures/.."
        );
        assert_eq!(
            capture_display_name(Path::new("/home/user"), Some(home)),
            "user"
        );
        assert_eq!(capture_display_name(Path::new("/"), Some(home)), "/");
    }

    #[test]
    fn saved_capture_offers_open_folder_with_its_shortcut_and_copy_path() {
        let mut state = make_test_input_state();
        state.set_action_bindings(std::collections::HashMap::from([(
            Action::OpenCaptureFolder,
            vec![crate::config::Shortcut::parse("Ctrl+Alt+O").unwrap()],
        )]));

        state.set_capture_feedback(Some(Path::new(SAVED)), true);

        let toast = state.active_toast().expect("capture toast");
        assert_eq!(
            toast.message,
            "Saved screenshot_2026-09-25_213231.png · Copied to clipboard"
        );
        assert!(!toast.message.contains("/very/long"));
        let open = toast.action.as_ref().expect("open folder button");
        assert_eq!(open.label, "Open folder · Ctrl+Alt+O");
        assert_eq!(open.dispatch_action(), Some(Action::OpenCaptureFolder));
        let copy = toast.secondary_action.as_ref().expect("copy path button");
        assert_eq!(copy.label, "Copy path");
        assert_eq!(copy.command, ToastCommand::CopyLastCapturePath);
        assert_eq!(toast.duration_ms, CAPTURE_ACTIONS_DURATION_MS);
        assert_eq!(state.last_capture_path(), Some(Path::new(SAVED)));
    }

    #[test]
    fn copy_path_requests_the_full_path_with_a_confirmation() {
        let mut state = make_test_input_state();
        assert!(state.last_capture_path_copy_request().is_none());

        state.set_capture_feedback(Some(Path::new(SAVED)), false);

        let request = state
            .last_capture_path_copy_request()
            .expect("copy request");
        assert_eq!(request.text, SAVED);
        assert!(request.cut.is_none());
        assert_eq!(request.confirmation, Some(PATH_COPIED_MESSAGE));
    }

    #[test]
    fn clipboard_only_capture_has_no_file_actions() {
        let mut state = make_test_input_state();
        state.set_last_capture_path(Some(PathBuf::from(SAVED)));

        state.set_capture_feedback(None, true);

        let toast = state.active_toast().expect("capture toast");
        assert_eq!(toast.message, "Copied to clipboard");
        assert!(toast.action.is_none());
        assert!(toast.secondary_action.is_none());
        assert!(state.last_capture_path().is_none());
    }

    #[test]
    fn unbound_open_folder_keeps_a_plain_button_label() {
        let mut state = make_test_input_state();
        state.set_action_bindings(std::collections::HashMap::from([(
            Action::OpenCaptureFolder,
            Vec::new(),
        )]));

        state.set_capture_feedback(Some(Path::new(SAVED)), false);

        let toast = state.active_toast().expect("capture toast");
        assert_eq!(toast.action.as_ref().unwrap().label, "Open folder");
    }
}
