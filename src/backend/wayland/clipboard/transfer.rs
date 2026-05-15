use super::{
    CLIPBOARD_READ_TIMEOUT, ClipboardPasteCompletion, ClipboardPasteResult,
    ClipboardPublishCompletion, ClipboardReadError, MAX_CLIPBOARD_IMAGE_BYTES,
    MAX_CLIPBOARD_SELECTION_BYTES, WAYSCRIBER_SELECTION_MIME, file_list, image, system,
};
use crate::draw::{EmbeddedImage, Shape};
use crate::input::state::{
    ClipboardFingerprint, ClipboardPasteRequest, WayscriberClipboardSelection,
};

#[derive(Debug, Clone)]
pub(in crate::backend::wayland) struct FailedLocalSelectionProbe {
    pub(in crate::backend::wayland) generation: u64,
    pub(in crate::backend::wayland) expected: Option<ClipboardFingerprint>,
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct TransferPlan<T> {
    pub(in crate::backend::wayland) effects: Vec<TransferEffect>,
    pub(in crate::backend::wayland) action: T,
}

impl<T> TransferPlan<T> {
    fn action(action: T) -> Self {
        Self {
            effects: Vec::new(),
            action,
        }
    }

    fn with_effects(effects: Vec<TransferEffect>, action: T) -> Self {
        Self { effects, action }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum TransferEffect {
    SupersedeLocalGeneration { generation: u64 },
}

#[derive(Debug)]
pub(in crate::backend::wayland) enum PasteAction {
    UseLocalShapes {
        request: ClipboardPasteRequest,
        shapes: Vec<Shape>,
        warning: Option<TransferWarning>,
    },
    ProbeSystemFingerprint {
        request: ClipboardPasteRequest,
        generation: u64,
        expected: Option<ClipboardFingerprint>,
    },
    ReadSystemClipboard {
        request: ClipboardPasteRequest,
    },
    StaleCompletion {
        request_id: u64,
    },
    ApplyPrivateSelection {
        request: ClipboardPasteRequest,
        shapes: Vec<Shape>,
    },
    ApplyExternalImage {
        request: ClipboardPasteRequest,
        image: EmbeddedImage,
    },
    TryFreshLocalFallbackOrWarn {
        request: ClipboardPasteRequest,
        missing_warning: TransferWarning,
        fallback_message: &'static str,
    },
    ShowWarning {
        request: ClipboardPasteRequest,
        warning: TransferWarning,
        block_feedback: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) enum TransferWarning {
    PublishSelectionTooLarge,
    PublishUnavailable,
    NothingPasted,
    NoShapesPasted,
    ClipboardEmpty,
    UnsupportedContent,
    TooLarge { limit: usize },
    TooManyPixels { width: u32, height: u32, limit: u64 },
    DecodeFailed,
    ReadTimedOut,
    ClipboardUnavailable,
    ClipboardError,
    MalformedPrivateSelection,
}

pub(in crate::backend::wayland) fn resolve_selection_clipboard_publish(
    generation: u64,
    payload_json: String,
) -> ClipboardPublishCompletion {
    if payload_json.len() > MAX_CLIPBOARD_SELECTION_BYTES {
        return ClipboardPublishCompletion {
            generation,
            fingerprint: system::clipboard_fingerprint(),
            copied: false,
            warning: Some(TransferWarning::PublishSelectionTooLarge),
        };
    }

    let copied =
        match std::panic::catch_unwind(|| system::publish_selection_clipboard(&payload_json)) {
            Ok(Ok(())) => true,
            Ok(Err(err)) => {
                log::warn!("Selection clipboard publish failed: {}", err);
                false
            }
            Err(_) => {
                log::error!("Selection clipboard publish panicked");
                false
            }
        };

    ClipboardPublishCompletion {
        generation,
        fingerprint: if copied {
            None
        } else {
            system::clipboard_fingerprint()
        },
        copied,
        warning: (!copied).then_some(TransferWarning::PublishUnavailable),
    }
}

pub(in crate::backend::wayland) fn resolve_system_clipboard() -> ClipboardPasteResult {
    let offered = match system::list_mime_types() {
        Ok(types) if types.is_empty() => return ClipboardPasteResult::ClipboardEmpty,
        Ok(types) => types,
        Err(ClipboardReadError::Empty) => return ClipboardPasteResult::ClipboardEmpty,
        Err(ClipboardReadError::Unavailable(err)) => {
            return ClipboardPasteResult::ClipboardUnavailable(err);
        }
        Err(err) => return map_read_error(err),
    };

    let Some(mime_type) = image::choose_supported_mime(&offered) else {
        return ClipboardPasteResult::NoSupportedMime { offered };
    };
    let limit = if mime_type == WAYSCRIBER_SELECTION_MIME {
        MAX_CLIPBOARD_SELECTION_BYTES
    } else {
        MAX_CLIPBOARD_IMAGE_BYTES
    };

    let bytes = match system::read_clipboard_mime(&mime_type, limit, CLIPBOARD_READ_TIMEOUT) {
        Ok(bytes) if bytes.is_empty() => return ClipboardPasteResult::ClipboardEmpty,
        Ok(bytes) => bytes,
        Err(err) => return map_read_error(err),
    };

    if mime_type == WAYSCRIBER_SELECTION_MIME {
        return serde_json::from_slice::<WayscriberClipboardSelection>(&bytes)
            .map(ClipboardPasteResult::PrivateSelection)
            .unwrap_or_else(|err| {
                ClipboardPasteResult::MalformedPrivateSelection(err.to_string())
            });
    }

    if file_list::is_uri_list_mime(&mime_type) {
        return file_list::decode_clipboard_uri_list(&mime_type, bytes, offered);
    }

    image::decode_clipboard_image(&mime_type, bytes)
}

pub(in crate::backend::wayland) fn plan_paste_start(
    request: ClipboardPasteRequest,
    pending_local_shapes: Option<Vec<Shape>>,
    failed_probe: Option<FailedLocalSelectionProbe>,
) -> TransferPlan<PasteAction> {
    if let Some(shapes) = pending_local_shapes {
        return TransferPlan::action(PasteAction::UseLocalShapes {
            request,
            shapes,
            warning: None,
        });
    }

    if let Some(failed_probe) = failed_probe {
        return TransferPlan::action(PasteAction::ProbeSystemFingerprint {
            request,
            generation: failed_probe.generation,
            expected: failed_probe.expected,
        });
    }

    TransferPlan::action(PasteAction::ReadSystemClipboard { request })
}

pub(in crate::backend::wayland) fn plan_after_fingerprint_probe(
    request: ClipboardPasteRequest,
    generation: u64,
    expected: Option<ClipboardFingerprint>,
    current: Option<ClipboardFingerprint>,
    local_shapes: Option<Vec<Shape>>,
) -> TransferPlan<PasteAction> {
    match (expected.as_ref(), current.as_ref()) {
        (Some(previous), Some(current)) if previous == current => {
            if let Some(shapes) = local_shapes {
                TransferPlan::action(PasteAction::UseLocalShapes {
                    request,
                    shapes,
                    warning: None,
                })
            } else {
                TransferPlan::action(PasteAction::ReadSystemClipboard { request })
            }
        }
        (None, None) => TransferPlan::action(PasteAction::ReadSystemClipboard { request }),
        _ => TransferPlan::with_effects(
            vec![TransferEffect::SupersedeLocalGeneration { generation }],
            PasteAction::ReadSystemClipboard { request },
        ),
    }
}

pub(in crate::backend::wayland) fn plan_paste_completion(
    completion: ClipboardPasteCompletion,
    active_request_id: Option<u64>,
    private_payload: Option<PrivateSelectionResolution>,
) -> TransferPlan<PasteAction> {
    let ClipboardPasteCompletion { request, result } = completion;
    if active_request_id != Some(request.id) {
        return TransferPlan::action(PasteAction::StaleCompletion {
            request_id: request.id,
        });
    }

    match result {
        ClipboardPasteResult::PrivateSelection(_) => {
            let private_payload = private_payload.expect("private payload resolution required");
            plan_private_selection_completion(request, private_payload)
        }
        ClipboardPasteResult::Image(image) => TransferPlan::with_effects(
            supersede_request_generation(&request),
            PasteAction::ApplyExternalImage { request, image },
        ),
        ClipboardPasteResult::ClipboardEmpty => TransferPlan::with_effects(
            supersede_request_generation(&request),
            PasteAction::ShowWarning {
                request,
                warning: TransferWarning::ClipboardEmpty,
                block_feedback: true,
            },
        ),
        ClipboardPasteResult::ClipboardUnavailable(err) => {
            log::warn!("Clipboard unavailable: {}", err);
            TransferPlan::action(PasteAction::TryFreshLocalFallbackOrWarn {
                request,
                missing_warning: TransferWarning::ClipboardUnavailable,
                fallback_message: "System clipboard unavailable; pasted local copy.",
            })
        }
        ClipboardPasteResult::NoSupportedMime { offered } => {
            log::debug!("Unsupported clipboard MIME types: {:?}", offered);
            TransferPlan::with_effects(
                supersede_request_generation(&request),
                PasteAction::ShowWarning {
                    request,
                    warning: TransferWarning::UnsupportedContent,
                    block_feedback: true,
                },
            )
        }
        ClipboardPasteResult::TooLarge { limit } => TransferPlan::with_effects(
            supersede_request_generation(&request),
            PasteAction::ShowWarning {
                request,
                warning: TransferWarning::TooLarge { limit },
                block_feedback: true,
            },
        ),
        ClipboardPasteResult::TooManyPixels {
            width,
            height,
            limit,
        } => TransferPlan::with_effects(
            supersede_request_generation(&request),
            PasteAction::ShowWarning {
                request,
                warning: TransferWarning::TooManyPixels {
                    width,
                    height,
                    limit,
                },
                block_feedback: true,
            },
        ),
        ClipboardPasteResult::DecodeFailed(err) => {
            log::warn!("Clipboard image decode failed: {}", err);
            TransferPlan::with_effects(
                supersede_request_generation(&request),
                PasteAction::ShowWarning {
                    request,
                    warning: TransferWarning::DecodeFailed,
                    block_feedback: true,
                },
            )
        }
        ClipboardPasteResult::ReadTimedOut => {
            TransferPlan::action(PasteAction::TryFreshLocalFallbackOrWarn {
                request,
                missing_warning: TransferWarning::ReadTimedOut,
                fallback_message: "Clipboard read timed out; pasted local copy.",
            })
        }
        ClipboardPasteResult::ClipboardError(err) => {
            log::warn!("Clipboard paste error: {}", err);
            TransferPlan::action(PasteAction::TryFreshLocalFallbackOrWarn {
                request,
                missing_warning: TransferWarning::ClipboardError,
                fallback_message: "Failed to read clipboard; pasted local copy.",
            })
        }
        ClipboardPasteResult::MalformedPrivateSelection(err) => {
            log::warn!("Malformed Wayscriber clipboard payload: {}", err);
            TransferPlan::with_effects(
                supersede_request_generation(&request),
                PasteAction::ShowWarning {
                    request,
                    warning: TransferWarning::MalformedPrivateSelection,
                    block_feedback: true,
                },
            )
        }
    }
}

#[derive(Debug)]
pub(in crate::backend::wayland) struct PrivateSelectionResolution {
    pub(in crate::backend::wayland) payload_matches_local: bool,
    pub(in crate::backend::wayland) same_instance: bool,
    pub(in crate::backend::wayland) shapes: Option<Vec<Shape>>,
}

fn plan_private_selection_completion(
    request: ClipboardPasteRequest,
    resolution: PrivateSelectionResolution,
) -> TransferPlan<PasteAction> {
    let effects = if resolution.payload_matches_local {
        Vec::new()
    } else {
        supersede_request_generation(&request)
    };

    if let Some(shapes) = resolution.shapes {
        return TransferPlan::with_effects(
            effects,
            PasteAction::ApplyPrivateSelection { request, shapes },
        );
    }

    if resolution.same_instance {
        log::debug!("Same-instance clipboard payload did not match paste request generation");
    }
    TransferPlan::with_effects(
        effects,
        PasteAction::ShowWarning {
            request,
            warning: TransferWarning::NoShapesPasted,
            block_feedback: true,
        },
    )
}

fn supersede_request_generation(request: &ClipboardPasteRequest) -> Vec<TransferEffect> {
    request
        .local_selection_fallback_generation
        .map(|generation| TransferEffect::SupersedeLocalGeneration { generation })
        .into_iter()
        .collect()
}

fn map_read_error(err: ClipboardReadError) -> ClipboardPasteResult {
    match err {
        ClipboardReadError::Empty => ClipboardPasteResult::ClipboardEmpty,
        ClipboardReadError::TooLarge { limit } => ClipboardPasteResult::TooLarge { limit },
        ClipboardReadError::TimedOut => ClipboardPasteResult::ReadTimedOut,
        ClipboardReadError::Unavailable(err) => ClipboardPasteResult::ClipboardUnavailable(err),
        ClipboardReadError::Other(err) => ClipboardPasteResult::ClipboardError(err),
    }
}

#[cfg(test)]
mod tests;
