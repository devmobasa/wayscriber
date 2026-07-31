//! Composited-frame cache for animated GIF images.
//!
//! Animated GIFs decode forward only (disposal methods make frame N depend on
//! frames 0..N), so playback steps through this cache: the first playthrough
//! decodes at most one new frame per call, and every later loop is pure cache
//! hits. Entries are keyed by content, shared by shapes with identical bytes;
//! playback clocks stay per-shape in the input-state registry.

use crate::draw::shape::EmbeddedImage;
use crate::image_decode::{GifStreamDecoder, MAX_ANIMATION_FRAMES};
use cairo::ImageSurface;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::time::Duration;

/// Decoded-frame budget for one animation; a breach degrades that GIF to the
/// static first-frame path. Matches the 32 Mpx paste-time verdict ceiling.
const ANIMATION_MAX_BYTES_PER_IMAGE: usize = 128 * 1024 * 1024;
/// Total budget across cached animations before LRU eviction, mirroring the
/// canvas layer cache precedent.
const ANIMATION_CACHE_MAX_BYTES: usize = 256 * 1024 * 1024;

/// Cheap animated-candidate check: canonical GIF mime or GIF magic bytes.
/// True does not guarantee the payload animates (it may be single-frame,
/// malformed, or over budget) — `step_to`/`first_frame_delay` settle that.
pub(crate) fn is_gif(data: &EmbeddedImage) -> bool {
    data.mime_type == "image/gif"
        || data.bytes.starts_with(b"GIF87a")
        || data.bytes.starts_with(b"GIF89a")
}

/// Outcome of stepping a playback clock forward.
pub(crate) enum FrameStep {
    /// The next frame; `index` is authoritative (it can restart at 0 after an
    /// eviction rebuilt the entry).
    Frame { index: usize, delay: Duration },
    /// End of stream: playback wraps to frame 0.
    Wrapped {
        delay: Duration,
        /// NETSCAPE loop count of the finished animation (`None` = forever).
        loop_count: Option<u32>,
    },
    /// Not animatable: single frame, over budget, or failed to decode.
    Static,
}

/// Returns the already-composited frame at `index` (clamped to the newest
/// decoded frame), or `None` when the payload should render via the static
/// image path instead.
pub(crate) fn peek_frame(data: &EmbeddedImage, index: usize) -> Option<Rc<ImageSurface>> {
    if !is_gif(data) {
        return None;
    }
    ANIMATION_CACHE.with(|cache| cache.borrow_mut().peek(data, index))
}

/// Delay of frame 0, decoding it if needed. `None` when the payload is not an
/// animatable GIF. This is how a playback clock arms its first deadline.
pub(crate) fn first_frame_delay(data: &EmbeddedImage) -> Option<Duration> {
    if !is_gif(data) {
        return None;
    }
    ANIMATION_CACHE.with(|cache| cache.borrow_mut().first_delay(data))
}

/// Steps playback toward `next_index`, decoding at most one new frame. The
/// returned index is authoritative; callers adopt it rather than trusting
/// their own counter.
pub(crate) fn step_to(data: &EmbeddedImage, next_index: usize) -> FrameStep {
    if !is_gif(data) {
        return FrameStep::Static;
    }
    ANIMATION_CACHE.with(|cache| cache.borrow_mut().step(data, next_index))
}

thread_local! {
    static ANIMATION_CACHE: RefCell<AnimationCache> = RefCell::new(AnimationCache::new());
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct AnimationKey {
    len: usize,
    hash: u64,
}

impl AnimationKey {
    fn for_image(data: &EmbeddedImage) -> Self {
        Self {
            len: data.bytes.len(),
            hash: super::image::content_hash(&data.bytes),
        }
    }
}

struct AnimFrame {
    surface: Rc<ImageSurface>,
    delay: Duration,
}

struct AnimatedImage {
    frames: Vec<AnimFrame>,
    /// Present until end of stream or a budget breach; dropped afterwards to
    /// free its copy of the encoded bytes.
    decoder: Option<Box<GifStreamDecoder>>,
    /// NETSCAPE loop count (`None` = forever). Accurate once EOF is reached.
    loop_count: Option<u32>,
    bytes_used: usize,
}

enum AnimationEntry {
    Animated(AnimatedImage),
    /// Exceeded a per-animation budget: kept as a cheap negative entry so the
    /// render path settles on the static fallback without re-decoding.
    TooLarge,
    Failed,
}

impl AnimationEntry {
    fn bytes_used(&self) -> usize {
        match self {
            AnimationEntry::Animated(animated) => animated.bytes_used,
            AnimationEntry::TooLarge | AnimationEntry::Failed => 0,
        }
    }
}

/// Entry-level result collected while the entry is mutably borrowed; cache
/// totals and negative-entry replacement happen afterwards.
enum StepOutcome {
    Ready(FrameStep),
    Decoded { added: usize, step: FrameStep },
    MarkTooLarge,
    MarkFailed,
}

struct AnimationCache {
    entries: HashMap<AnimationKey, AnimationEntry>,
    order: VecDeque<AnimationKey>,
    total_bytes: usize,
}

impl AnimationCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            total_bytes: 0,
        }
    }

    fn ensure_entry(&mut self, key: &AnimationKey, data: &EmbeddedImage) {
        if self.entries.contains_key(key) {
            if let Some(position) = self.order.iter().position(|entry| entry == key) {
                self.order.remove(position);
                self.order.push_back(key.clone());
            }
            return;
        }
        let entry = match GifStreamDecoder::new(&data.bytes) {
            Ok(decoder) => AnimationEntry::Animated(AnimatedImage {
                frames: Vec::new(),
                decoder: Some(Box::new(decoder)),
                loop_count: None,
                bytes_used: 0,
            }),
            Err(_) => AnimationEntry::Failed,
        };
        self.order.push_back(key.clone());
        self.entries.insert(key.clone(), entry);
    }

    fn step(&mut self, data: &EmbeddedImage, next_index: usize) -> FrameStep {
        let key = AnimationKey::for_image(data);
        self.ensure_entry(&key, data);
        let outcome = {
            let entry = self.entries.get_mut(&key).expect("entry just ensured");
            let AnimationEntry::Animated(animated) = entry else {
                return FrameStep::Static;
            };
            if let Some(frame) = animated.frames.get(next_index) {
                StepOutcome::Ready(FrameStep::Frame {
                    index: next_index,
                    delay: frame.delay,
                })
            } else if animated.decoder.is_none() {
                StepOutcome::Ready(wrap_step(animated))
            } else if animated.frames.len() >= MAX_ANIMATION_FRAMES {
                StepOutcome::MarkTooLarge
            } else {
                match decode_next_frame(animated) {
                    DecodeStep::Frame { added, delay } => StepOutcome::Decoded {
                        added,
                        step: FrameStep::Frame {
                            index: animated.frames.len() - 1,
                            delay,
                        },
                    },
                    DecodeStep::EndOfStream => StepOutcome::Ready(wrap_step(animated)),
                    DecodeStep::OverBudget => StepOutcome::MarkTooLarge,
                    DecodeStep::Failed => StepOutcome::MarkFailed,
                }
            }
        };
        self.apply(&key, outcome)
    }

    fn peek(&mut self, data: &EmbeddedImage, index: usize) -> Option<Rc<ImageSurface>> {
        let key = AnimationKey::for_image(data);
        self.ensure_entry(&key, data);
        let (surface, outcome) = {
            let entry = self.entries.get_mut(&key).expect("entry just ensured");
            let AnimationEntry::Animated(animated) = entry else {
                return None;
            };
            let mut added = 0;
            if animated.frames.is_empty() {
                match decode_next_frame(animated) {
                    DecodeStep::Frame { added: bytes, .. } => added = bytes,
                    DecodeStep::EndOfStream => return None,
                    DecodeStep::OverBudget => {
                        self.apply(&key, StepOutcome::MarkTooLarge);
                        return None;
                    }
                    DecodeStep::Failed => {
                        self.apply(&key, StepOutcome::MarkFailed);
                        return None;
                    }
                }
            }
            let frame = animated
                .frames
                .get(index)
                .or_else(|| animated.frames.last())?;
            (
                frame.surface.clone(),
                StepOutcome::Decoded {
                    added,
                    step: FrameStep::Static,
                },
            )
        };
        self.apply(&key, outcome);
        Some(surface)
    }

    fn first_delay(&mut self, data: &EmbeddedImage) -> Option<Duration> {
        let key = AnimationKey::for_image(data);
        self.ensure_entry(&key, data);
        let (delay, outcome) = {
            let entry = self.entries.get_mut(&key).expect("entry just ensured");
            let AnimationEntry::Animated(animated) = entry else {
                return None;
            };
            let mut added = 0;
            if animated.frames.is_empty() {
                match decode_next_frame(animated) {
                    DecodeStep::Frame { added: bytes, .. } => added = bytes,
                    DecodeStep::EndOfStream => return None,
                    DecodeStep::OverBudget => {
                        self.apply(&key, StepOutcome::MarkTooLarge);
                        return None;
                    }
                    DecodeStep::Failed => {
                        self.apply(&key, StepOutcome::MarkFailed);
                        return None;
                    }
                }
            }
            let delay = animated.frames.first()?.delay;
            (
                delay,
                StepOutcome::Decoded {
                    added,
                    step: FrameStep::Static,
                },
            )
        };
        self.apply(&key, outcome);
        Some(delay)
    }

    /// Applies cache-level bookkeeping once the entry borrow has ended.
    fn apply(&mut self, key: &AnimationKey, outcome: StepOutcome) -> FrameStep {
        match outcome {
            StepOutcome::Ready(step) => step,
            StepOutcome::Decoded { added, step } => {
                self.total_bytes = self.total_bytes.saturating_add(added);
                self.evict_over_budget(key);
                step
            }
            StepOutcome::MarkTooLarge => {
                self.replace_entry(key, AnimationEntry::TooLarge);
                FrameStep::Static
            }
            StepOutcome::MarkFailed => {
                self.replace_entry(key, AnimationEntry::Failed);
                FrameStep::Static
            }
        }
    }

    fn replace_entry(&mut self, key: &AnimationKey, replacement: AnimationEntry) {
        if let Some(previous) = self.entries.insert(key.clone(), replacement) {
            self.total_bytes = self.total_bytes.saturating_sub(previous.bytes_used());
        }
    }

    fn evict_over_budget(&mut self, keep: &AnimationKey) {
        while self.total_bytes > ANIMATION_CACHE_MAX_BYTES && self.order.len() > 1 {
            let Some(position) = self.order.iter().position(|key| key != keep) else {
                break;
            };
            let Some(oldest) = self.order.remove(position) else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(entry.bytes_used());
            }
        }
    }
}

fn wrap_step(animated: &AnimatedImage) -> FrameStep {
    if animated.frames.len() <= 1 {
        return FrameStep::Static;
    }
    FrameStep::Wrapped {
        delay: animated.frames[0].delay,
        loop_count: animated.loop_count,
    }
}

enum DecodeStep {
    Frame { added: usize, delay: Duration },
    EndOfStream,
    OverBudget,
    Failed,
}

fn decode_next_frame(animated: &mut AnimatedImage) -> DecodeStep {
    let Some(decoder) = animated.decoder.as_mut() else {
        return DecodeStep::EndOfStream;
    };
    match decoder.next_frame() {
        Ok(Some(frame)) => {
            let (width, height) = decoder.dimensions();
            let Some(surface) = super::image::rgba_to_cairo_surface(width, height, &frame.rgba)
            else {
                return DecodeStep::Failed;
            };
            let added = surface.stride().max(0) as usize * surface.height().max(0) as usize;
            if animated.bytes_used.saturating_add(added) > ANIMATION_MAX_BYTES_PER_IMAGE {
                return DecodeStep::OverBudget;
            }
            let delay = frame.delay;
            animated.bytes_used += added;
            animated.frames.push(AnimFrame {
                surface: Rc::new(surface),
                delay,
            });
            DecodeStep::Frame { added, delay }
        }
        Ok(None) => {
            animated.loop_count = animated
                .decoder
                .as_ref()
                .and_then(|decoder| decoder.loop_count());
            animated.decoder = None;
            DecodeStep::EndOfStream
        }
        Err(_) => DecodeStep::Failed,
    }
}
