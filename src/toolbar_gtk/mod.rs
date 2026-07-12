//! GTK4 toolbar frontend.
//!
//! Renders the top strip and side palette as GTK4 windows on their own
//! layer-shell surfaces, replacing the built-in Cairo toolbars on
//! compositors where those would have used separate layer surfaces anyway.
//! The built-in toolbars stay compiled in as the fallback everywhere else:
//! GNOME's xdg fallback, forced-inline mode, builds without the
//! `toolbar-gtk` feature, and GTK runtime failures.
//!
//! Threading: GTK runs on a dedicated thread with its own Wayland
//! connection and GLib main loop. The Wayland backend never calls GTK; the
//! two sides talk exclusively through the [`GtkToolbarBridge`] channels.
//! [`GtkToolbarUpdate`]s flow to the GTK thread whenever toolbar-relevant
//! state changes, and [`GtkToolbarFeedback`] flows back and is drained
//! once per event-loop iteration. Toolbar events feed the same
//! `handle_toolbar_event` path the built-in bars use, so persistence and
//! popover policy stay shared instead of duplicated.

pub mod select;

#[cfg(feature = "toolbar-gtk")]
mod css;
#[cfg(feature = "toolbar-gtk")]
mod icons;
#[cfg(feature = "toolbar-gtk")]
mod runtime;
#[cfg(feature = "toolbar-gtk")]
mod view;
#[cfg(feature = "toolbar-gtk")]
mod widgets;

use crate::config::ToolbarRebindModifier;
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};

/// State pushed to the GTK thread; sent only when it differs from the
/// previously delivered update.
#[derive(Debug, Clone, PartialEq)]
pub struct GtkToolbarUpdate {
    pub snapshot: ToolbarSnapshot,
    pub top_visible: bool,
    pub side_visible: bool,
    /// Drag offsets relative to each bar's base margins, already clamped
    /// by the backend.
    pub top_offset: (f64, f64),
    pub side_offset: (f64, f64),
    /// Highest drag sequence number the backend has drained per bar; the
    /// GTK side ignores offset echoes older than its own counter so a
    /// mid-drag mirror can never snap a just-released bar backwards.
    pub top_offset_seq: u64,
    pub side_offset_seq: u64,
    /// Base X for the top strip in spec units (the side palette pushes it
    /// right when they would overlap), mirroring the backend clamp math.
    pub top_base_x: f64,
    /// Connector name of the output hosting the overlay (e.g. "DP-1"),
    /// used to pin the GTK bars to the same monitor.
    pub output_name: Option<String>,
    /// Modifier chord that turns a GTK toolbar click into shortcut rebinding.
    pub rebind_modifier: ToolbarRebindModifier,
    /// Backend-observed chord state. Wayland pointer events do not always
    /// expose keyboard modifiers to GTK when its surface lacks keyboard focus.
    pub rebind_modifier_active: bool,
}

/// Messages from the GTK thread back to the backend.
// Without the feature the stub bridge never constructs these; the backend
// match arms still compile against them.
#[cfg_attr(not(feature = "toolbar-gtk"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub enum GtkToolbarFeedback {
    /// A toolbar control fired; routed through `handle_toolbar_event`.
    Event {
        event: ToolbarEvent,
        rebind_requested: bool,
    },
    /// Drag-to-move progress for the top bar. `done` marks the drag end,
    /// which is when the offsets get clamped and persisted; `seq` is the
    /// bar's monotonically increasing drag counter (see
    /// [`GtkToolbarUpdate::top_offset_seq`]).
    SetTopOffset {
        x: f64,
        y: f64,
        seq: u64,
        done: bool,
    },
    /// Drag-to-move progress for the side palette.
    SetSideOffset {
        x: f64,
        y: f64,
        seq: u64,
        done: bool,
    },
}

#[cfg(feature = "toolbar-gtk")]
pub use enabled::GtkToolbarBridge;

#[cfg(feature = "toolbar-gtk")]
mod enabled {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU8, Ordering};

    use super::{GtkToolbarFeedback, GtkToolbarUpdate};

    pub(super) const STATUS_STARTING: u8 = 0;
    pub(super) const STATUS_READY: u8 = 1;
    pub(super) const STATUS_FAILED: u8 = 2;

    /// Main-thread handle to the GTK toolbar thread.
    pub struct GtkToolbarBridge {
        update_tx: async_channel::Sender<GtkToolbarUpdate>,
        feedback_rx: std::sync::mpsc::Receiver<GtkToolbarFeedback>,
        status: Arc<AtomicU8>,
        last_sent: Option<GtkToolbarUpdate>,
    }

    impl GtkToolbarBridge {
        /// Spawns the GTK thread. Returns `None` only when the OS thread
        /// cannot be created; GTK-level failures are reported
        /// asynchronously through [`Self::failed`].
        pub fn spawn() -> Option<Self> {
            let (update_tx, update_rx) = async_channel::unbounded();
            let (feedback_tx, feedback_rx) = std::sync::mpsc::channel();
            let status = Arc::new(AtomicU8::new(STATUS_STARTING));
            let thread_status = status.clone();
            let spawned = std::thread::Builder::new()
                .name("gtk-toolbar".into())
                .spawn(move || super::runtime::run(update_rx, feedback_tx, thread_status));
            match spawned {
                Ok(_join_handle) => Some(Self {
                    update_tx,
                    feedback_rx,
                    status,
                    last_sent: None,
                }),
                Err(err) => {
                    log::error!("Failed to spawn GTK toolbar thread: {err}");
                    None
                }
            }
        }

        /// True once the GTK thread reported an unrecoverable failure.
        pub fn failed(&self) -> bool {
            self.status.load(Ordering::Acquire) == STATUS_FAILED
        }

        /// Non-blocking receive of one feedback message from the GTK bars.
        pub fn try_recv_feedback(&self) -> Option<GtkToolbarFeedback> {
            self.feedback_rx.try_recv().ok()
        }

        /// Sends the update if it differs from the previously sent one.
        pub fn maybe_send(&mut self, update: GtkToolbarUpdate) {
            if self.last_sent.as_ref() == Some(&update) {
                return;
            }
            // try_send on an unbounded channel only fails when the GTK
            // thread is gone; the failed() flag covers that case.
            if self.update_tx.try_send(update.clone()).is_ok() {
                self.last_sent = Some(update);
            }
        }
    }
}

#[cfg(not(feature = "toolbar-gtk"))]
pub use disabled::GtkToolbarBridge;

#[cfg(not(feature = "toolbar-gtk"))]
mod disabled {
    use super::{GtkToolbarFeedback, GtkToolbarUpdate};

    /// Stub bridge: without the `toolbar-gtk` feature `spawn` never
    /// succeeds, so the other methods are unreachable but keep call sites
    /// free of cfg noise.
    pub struct GtkToolbarBridge {}

    impl GtkToolbarBridge {
        pub fn spawn() -> Option<Self> {
            None
        }

        pub fn failed(&self) -> bool {
            false
        }

        pub fn try_recv_feedback(&self) -> Option<GtkToolbarFeedback> {
            None
        }

        pub fn maybe_send(&mut self, _update: GtkToolbarUpdate) {}
    }
}
