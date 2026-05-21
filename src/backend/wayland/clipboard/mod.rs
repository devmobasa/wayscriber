//! Clipboard transfer rules and `wl-copy`/`wl-paste` integration for selections and images.

use crate::draw::EmbeddedImage;
use crate::input::state::{
    ClipboardFingerprint, ClipboardPasteRequest, WayscriberClipboardSelection,
};
use std::time::Duration;

pub(in crate::backend::wayland) use system::clipboard_fingerprint;
pub(in crate::backend::wayland) use transfer::{
    FailedLocalSelectionProbe, PasteAction, TransferEffect, TransferPlan, TransferWarning,
};

mod file_list;
mod image;
mod system;
pub(in crate::backend::wayland) mod transfer;

pub(super) const WAYSCRIBER_SELECTION_MIME: &str = "application/vnd.wayscriber.selection+json";

// A pasted image is persisted in the visible frame and in the Create undo action.
// Keep one accepted image comfortably below the default 50 MiB session JSON budget.
pub(super) const MAX_CLIPBOARD_IMAGE_BYTES: usize = 3 * 1024 * 1024;
pub(super) const MAX_CLIPBOARD_SELECTION_BYTES: usize = 2 * 1024 * 1024;
pub(super) const MAX_CLIPBOARD_IMAGE_PIXELS: u64 = 48_000_000;
pub(super) const CLIPBOARD_READ_TIMEOUT: Duration = Duration::from_millis(1500);
pub(super) const CLIPBOARD_FINGERPRINT_BYTES: usize = 4096;
pub(super) const CLIPBOARD_FINGERPRINT_TIMEOUT: Duration = Duration::from_millis(300);
pub(super) const CLIPBOARD_PUBLISH_COMMAND_TIMEOUT: Duration = Duration::from_millis(750);

#[derive(Debug)]
pub(in crate::backend::wayland) struct ClipboardPasteCompletion {
    pub(in crate::backend::wayland) request: ClipboardPasteRequest,
    pub(in crate::backend::wayland) result: ClipboardPasteResult,
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct ClipboardPublishCompletion {
    pub(in crate::backend::wayland) generation: u64,
    pub(in crate::backend::wayland) fingerprint: Option<ClipboardFingerprint>,
    pub(in crate::backend::wayland) copied: bool,
    pub(in crate::backend::wayland) warning: Option<TransferWarning>,
}

#[derive(Debug)]
pub(in crate::backend::wayland) enum ClipboardPasteResult {
    PrivateSelection(WayscriberClipboardSelection),
    Image(EmbeddedImage),
    ClipboardEmpty,
    NoSupportedMime { offered: Vec<String> },
    TooLarge { limit: usize },
    TooManyPixels { width: u32, height: u32, limit: u64 },
    DecodeFailed(String),
    ReadTimedOut,
    ClipboardUnavailable(String),
    ClipboardError(String),
    MalformedPrivateSelection(String),
}

impl ClipboardPasteResult {
    pub(in crate::backend::wayland) fn summary(&self) -> String {
        match self {
            Self::PrivateSelection(selection) => {
                format!("private-selection shapes={}", selection.shapes.len())
            }
            Self::Image(image) => format!(
                "image mime={} dimensions={}x{} bytes={}",
                image.mime_type,
                image.width,
                image.height,
                image.bytes.len()
            ),
            Self::ClipboardEmpty => "clipboard-empty".to_string(),
            Self::NoSupportedMime { offered } => {
                format!("unsupported-mime offered={offered:?}")
            }
            Self::TooLarge { limit } => format!("too-large limit={limit}"),
            Self::TooManyPixels {
                width,
                height,
                limit,
            } => {
                format!("too-many-pixels dimensions={width}x{height} limit={limit}")
            }
            Self::DecodeFailed(err) => format!("decode-failed error={err}"),
            Self::ReadTimedOut => "read-timed-out".to_string(),
            Self::ClipboardUnavailable(err) => format!("clipboard-unavailable error={err}"),
            Self::ClipboardError(err) => format!("clipboard-error error={err}"),
            Self::MalformedPrivateSelection(err) => {
                format!("malformed-private-selection error={err}")
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ClipboardReadError {
    Empty,
    TooLarge { limit: usize },
    TimedOut,
    Unavailable(String),
    Other(String),
}

#[derive(Debug)]
pub(super) struct ClipboardPrefixRead {
    pub(super) bytes: Vec<u8>,
    pub(super) truncated: bool,
}
