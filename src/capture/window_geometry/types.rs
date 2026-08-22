use crate::util::Rect;

use super::WindowGeometryBackend;

/// Frozen-source output identity and compositor-global logical bounds for one query.
///
/// Wayland exposes no portable workspace identity at the freeze boundary. The
/// provider therefore returns windows visible when its compositor query runs;
/// output identity and geometry remain correlated fail-closed to the frozen
/// source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowQueryContext {
    pub(crate) output_name: String,
    pub(crate) output_logical_rect: Rect,
}

/// A visible window clipped to the active output and normalized to its origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowTarget {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) logical_rect: Rect,
}

/// One complete provider query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowQueryResult {
    pub(crate) backend: WindowGeometryBackend,
    pub(crate) targets: Vec<WindowTarget>,
}

/// Window-provider failure classified for the picker worker boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WindowGeometryError {
    Unsupported,
    Broker(String),
    CommandFailed {
        backend: WindowGeometryBackend,
        message: String,
    },
    InvalidResponse {
        backend: WindowGeometryBackend,
        message: String,
    },
}

impl std::fmt::Display for WindowGeometryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => formatter.write_str("window snapping is unavailable"),
            Self::Broker(message) => write!(formatter, "window geometry broker failed: {message}"),
            Self::CommandFailed { backend, message } => {
                write!(formatter, "{backend:?} window query failed: {message}")
            }
            Self::InvalidResponse { backend, message } => {
                write!(formatter, "invalid {backend:?} window response: {message}")
            }
        }
    }
}

impl std::error::Error for WindowGeometryError {}
