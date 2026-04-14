use crate::draw::Color;

/// Background replay context for tools that need access to the captured backdrop.
pub struct EraserReplayContext<'a> {
    /// Optional pattern representing the current background (e.g., frozen image) in device space.
    pub pattern: Option<&'a cairo::Pattern>,
    /// Optional surface representing the captured background image.
    pub surface: Option<&'a cairo::ImageSurface>,
    /// Stable cache key for the currently active backdrop image generation.
    pub backdrop_cache_key: Option<u64>,
    /// Solid background color (board modes) when no pattern is available.
    pub bg_color: Option<Color>,
    /// Horizontal scale from logical canvas coordinates to captured image pixels.
    pub logical_to_image_scale_x: f64,
    /// Vertical scale from logical canvas coordinates to captured image pixels.
    pub logical_to_image_scale_y: f64,
}
