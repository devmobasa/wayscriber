//! Per-shape playback clocks for animated GIF images.
//!
//! Runtime-only state: nothing here is serialized, and nothing lives inside
//! `Shape`, so session round-trips and the `ModifyImageBounds` undo byte-dedup
//! are unaffected. Entries are keyed by [`ShapeId`], which is unique only
//! within one frame — the registry therefore tracks the *active* frame
//! exclusively, created and garbage-collected by the same mark-and-sweep
//! advance pass. That one pass covers paste, undo/redo, delete, duplicate,
//! page/board switches, and session loads without dedicated hooks; the
//! accepted consequence is that playback (and any pause) restarts when a page
//! is revisited.

use super::base::InputState;
use crate::draw::Shape;
use crate::draw::ShapeId;
use crate::draw::render::animation::{self, FrameStep};
use crate::util::Rect;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::time::{Duration, Instant};

pub(crate) struct GifPlaybackEntry {
    frame_index: usize,
    next_due: Instant,
    loops_done: u32,
    playing: bool,
    /// Finite loop count exhausted (holding the last frame), or the payload
    /// turned out not to animate after all.
    finished: bool,
    /// Display bbox at the last advance pass, for visibility-filtered
    /// deadlines without re-walking the frame.
    last_bbox: Option<Rect>,
    /// Mark bit for the sweep half of the advance pass.
    seen: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PresenceKey {
    generation: u64,
    board: usize,
    page: usize,
}

#[derive(Default)]
pub(crate) struct GifPlaybackRegistry {
    entries: HashMap<ShapeId, GifPlaybackEntry>,
    /// Kept in sync with `entries` on every advance; borrowed by render paths
    /// to pick the frame per shape without touching entries.
    frame_indices: HashMap<ShapeId, usize>,
    /// Memoized "active frame contains a GIF image" scan.
    presence_memo: Option<(PresenceKey, bool)>,
}

/// Clamps a GIF frame delay to the configured UI animation budget.
fn effective_delay(delay: Duration, interval_floor: Option<Duration>) -> Duration {
    match interval_floor {
        Some(floor) => delay.max(floor),
        None => delay,
    }
}

fn visible(view: Option<Rect>, bbox: Option<Rect>) -> bool {
    match (view, bbox) {
        (Some(view), Some(bbox)) => view.intersects(&bbox),
        // Without both rects there is nothing to cull against; keep playing.
        _ => true,
    }
}

impl InputState {
    /// Advances every due GIF on the active frame by one composited frame,
    /// marking each advanced shape's bbox dirty. Also creates missing entries
    /// (autoplay) and sweeps entries whose shapes are gone. Returns whether
    /// anything advanced.
    ///
    /// `interval_floor` is the UI animation tick interval; GIF delays are
    /// clamped to it so a fast GIF cannot outrun the configured budget.
    pub fn advance_gif_animations(
        &mut self,
        now: Instant,
        view: Option<Rect>,
        interval_floor: Option<Duration>,
    ) -> bool {
        if !crate::ui::anim::motion_enabled() {
            // Reduced motion: clocks freeze (new entries hold frame 0).
            return false;
        }
        let frame = self.boards.active_frame();
        let registry = &mut self.gif_playback;
        let dirty = &mut self.dirty_tracker;
        let mut advanced = false;

        for entry in registry.entries.values_mut() {
            entry.seen = false;
        }

        for shape in &frame.shapes {
            let Shape::Image { data, .. } = &shape.shape else {
                continue;
            };
            if !animation::is_gif(data) {
                continue;
            }
            let bbox = shape.bounding_box();
            let entry = match registry.entries.entry(shape.id) {
                Entry::Occupied(occupied) => occupied.into_mut(),
                Entry::Vacant(vacant) => {
                    // Not animatable (failed, over budget): no entry. The
                    // animation cache's negative entry keeps retries O(1).
                    let Some(delay) = animation::first_frame_delay(data) else {
                        continue;
                    };
                    registry.frame_indices.insert(shape.id, 0);
                    vacant.insert(GifPlaybackEntry {
                        frame_index: 0,
                        next_due: now + effective_delay(delay, interval_floor),
                        loops_done: 0,
                        playing: true,
                        finished: false,
                        last_bbox: bbox,
                        seen: true,
                    })
                }
            };
            entry.seen = true;
            entry.last_bbox = bbox;
            if !entry.playing || entry.finished {
                continue;
            }
            if !visible(view, bbox) {
                // Clock freezes offscreen; resumes where it left off.
                continue;
            }
            if now < entry.next_due {
                continue;
            }
            match animation::step_to(data, entry.frame_index + 1) {
                FrameStep::Frame { index, delay } => {
                    entry.frame_index = index;
                    entry.next_due = now + effective_delay(delay, interval_floor);
                    registry.frame_indices.insert(shape.id, index);
                    if let Some(bbox) = bbox {
                        dirty.mark_rect(bbox);
                    }
                    advanced = true;
                }
                FrameStep::Wrapped { delay, loop_count } => {
                    entry.loops_done += 1;
                    if loop_count.is_some_and(|limit| entry.loops_done >= limit) {
                        // Hold the last frame, like browsers do.
                        entry.finished = true;
                        continue;
                    }
                    entry.frame_index = 0;
                    entry.next_due = now + effective_delay(delay, interval_floor);
                    registry.frame_indices.insert(shape.id, 0);
                    if let Some(bbox) = bbox {
                        dirty.mark_rect(bbox);
                    }
                    advanced = true;
                }
                FrameStep::Static => {
                    // Damage once so the wake that discovered this cannot
                    // escalate into an empty-damage full repaint.
                    entry.finished = true;
                    if let Some(bbox) = bbox {
                        dirty.mark_rect(bbox);
                    }
                }
            }
        }

        let GifPlaybackRegistry {
            entries,
            frame_indices,
            ..
        } = registry;
        entries.retain(|id, entry| {
            if entry.seen {
                true
            } else {
                frame_indices.remove(id);
                false
            }
        });
        advanced
    }

    fn gif_earliest_due(&self, view: Option<Rect>) -> Option<Instant> {
        if !crate::ui::anim::motion_enabled() {
            return None;
        }
        self.gif_playback
            .entries
            .values()
            .filter(|entry| entry.playing && !entry.finished)
            .filter(|entry| visible(view, entry.last_bbox))
            .map(|entry| entry.next_due)
            .min()
    }

    /// Event-loop deadline provider. Mirrors the radial-menu guard: once the
    /// earliest deadline has passed and a redraw is already pending, returns
    /// `None` so a zero timeout cannot spin the loop — the pending render's
    /// advance pass takes it from there.
    pub fn gif_frame_timeout(&self, now: Instant, view: Option<Rect>) -> Option<Duration> {
        let due = self.gif_earliest_due(view)?;
        if due <= now {
            if self.needs_redraw {
                return None;
            }
            return Some(Duration::ZERO);
        }
        Some(due - now)
    }

    /// The wake-check twin of [`Self::advance_gif_animations`]'s gate: true
    /// when at least one visible, playing entry is due.
    pub fn gif_frames_due(&self, now: Instant, view: Option<Rect>) -> bool {
        self.gif_earliest_due(view).is_some_and(|due| due <= now)
    }

    /// Per-shape frame selection for render paths (absent id = frame 0).
    pub fn gif_frame_indices(&self) -> &HashMap<ShapeId, usize> {
        &self.gif_playback.frame_indices
    }

    /// `Some(true)` while the shape's animation is running. `None` when the
    /// shape has no playback entry (not an animatable GIF, or not yet seen by
    /// an advance pass).
    pub fn gif_playback_running(&self, id: ShapeId) -> Option<bool> {
        self.gif_playback
            .entries
            .get(&id)
            .map(|entry| entry.playing && !entry.finished)
    }

    /// Toggles play/pause; resuming a finished finite-loop GIF restarts its
    /// loop budget. Returns the new running state.
    pub fn toggle_gif_playback(&mut self, id: ShapeId, now: Instant) -> Option<bool> {
        let entry = self.gif_playback.entries.get_mut(&id)?;
        if entry.playing && !entry.finished {
            entry.playing = false;
        } else {
            entry.playing = true;
            if entry.finished {
                entry.finished = false;
                entry.loops_done = 0;
            }
            entry.next_due = now;
        }
        Some(entry.playing)
    }

    /// O(1)-amortized "does the active frame contain any GIF image", used to
    /// bypass the canvas layer cache (which is content-keyed, not time-keyed).
    pub fn active_frame_has_gif_image(&mut self) -> bool {
        let key = PresenceKey {
            generation: self.canvas_content_generation(),
            board: self.boards.active_index(),
            page: self.boards.active_page_index(),
        };
        if let Some((memo_key, has_gif)) = self.gif_playback.presence_memo
            && memo_key == key
        {
            return has_gif;
        }
        let has_gif = self.boards.active_frame().shapes.iter().any(
            |shape| matches!(&shape.shape, Shape::Image { data, .. } if animation::is_gif(data)),
        );
        self.gif_playback.presence_memo = Some((key, has_gif));
        has_gif
    }
}
