/// A point in source-image pixel-edge coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImagePoint {
    pub x: f64,
    pub y: f64,
}

impl ImagePoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

fn valid_point_and_bounds(point: ImagePoint, bounds: (u32, u32)) -> bool {
    point.x.is_finite() && point.y.is_finite() && bounds.0 != 0 && bounds.1 != 0
}

/// Snap an image point to the top-left index of an existing pixel.
pub fn snap_anchor(point: ImagePoint, bounds: (u32, u32)) -> Option<ImagePoint> {
    if !valid_point_and_bounds(point, bounds) {
        return None;
    }
    Some(ImagePoint::new(
        point.x.floor().clamp(0.0, f64::from(bounds.0 - 1)),
        point.y.floor().clamp(0.0, f64::from(bounds.1 - 1)),
    ))
}

/// Clamp an image point to the inclusive outer pixel-edge domain.
pub fn clamp_edge(point: ImagePoint, bounds: (u32, u32)) -> Option<ImagePoint> {
    if !valid_point_and_bounds(point, bounds) {
        return None;
    }
    Some(ImagePoint::new(
        point.x.clamp(0.0, f64::from(bounds.0)),
        point.y.clamp(0.0, f64::from(bounds.1)),
    ))
}

/// Constrain an edge to a dominant-axis square rooted at `anchor`.
pub fn squared_cursor(
    anchor: ImagePoint,
    raw_edge: ImagePoint,
    bounds: (u32, u32),
) -> Option<ImagePoint> {
    if !valid_point_and_bounds(anchor, bounds)
        || !raw_edge.x.is_finite()
        || !raw_edge.y.is_finite()
        || anchor.x < 0.0
        || anchor.y < 0.0
        || anchor.x > f64::from(bounds.0 - 1)
        || anchor.y > f64::from(bounds.1 - 1)
    {
        return None;
    }

    let delta_x = raw_edge.x - anchor.x;
    let delta_y = raw_edge.y - anchor.y;
    let mut side = delta_x.abs().max(delta_y.abs());
    if !side.is_finite() {
        return None;
    }

    let (direction_x, room_x) = square_direction_and_room(delta_x, anchor.x, bounds.0);
    let (direction_y, room_y) = square_direction_and_room(delta_y, anchor.y, bounds.1);
    side = side.min(room_x).min(room_y);
    Some(ImagePoint::new(
        anchor.x + direction_x * side,
        anchor.y + direction_y * side,
    ))
}

fn square_direction_and_room(delta: f64, anchor: f64, bound: u32) -> (f64, f64) {
    let mut direction = if delta < 0.0 { -1.0 } else { 1.0 };
    let mut room = if direction > 0.0 {
        f64::from(bound) - anchor
    } else {
        anchor
    };
    if room == 0.0 {
        direction = -direction;
        room = if direction > 0.0 {
            f64::from(bound) - anchor
        } else {
            anchor
        };
    }
    (direction, room)
}

/// A quantized pixel span. It may be empty while a selection is in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSpan {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl PixelSpan {
    #[allow(dead_code)] // Geometry diagnostics and tests inspect the quantized origin.
    pub const fn x(self) -> u32 {
        self.x
    }

    #[allow(dead_code)] // Geometry diagnostics and tests inspect the quantized origin.
    pub const fn y(self) -> u32 {
        self.y
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }

    #[allow(dead_code)] // Tests and Phase 2 review use the paired dimensions.
    pub const fn size(self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Quantize two unordered image edges using floor-minimum/ceil-maximum coverage.
pub fn pixel_span(first: ImagePoint, second: ImagePoint, bounds: (u32, u32)) -> Option<PixelSpan> {
    let first = clamp_edge(first, bounds)?;
    let second = clamp_edge(second, bounds)?;
    let left = first.x.min(second.x).floor() as u32;
    let top = first.y.min(second.y).floor() as u32;
    let right = first.x.max(second.x).ceil() as u32;
    let bottom = first.y.max(second.y).ceil() as u32;
    Some(PixelSpan {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

/// A non-empty rectangle of source-image pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImagePixelRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl ImagePixelRect {
    /// Construct a non-empty rectangle fully contained by `bounds`.
    pub fn new(x: u32, y: u32, width: u32, height: u32, bounds: (u32, u32)) -> Option<Self> {
        if width == 0
            || height == 0
            || x.checked_add(width)? > bounds.0
            || y.checked_add(height)? > bounds.1
        {
            return None;
        }
        Some(Self {
            x,
            y,
            width,
            height,
        })
    }

    #[allow(dead_code)] // Tests and Phase 2 review use the paired dimensions.
    pub const fn size(self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub const fn x(self) -> u32 {
        self.x
    }

    pub const fn y(self) -> u32 {
        self.y
    }

    pub const fn width(self) -> u32 {
        self.width
    }

    pub const fn height(self) -> u32 {
        self.height
    }

    /// Select every pixel in non-empty image bounds.
    pub fn whole(bounds: (u32, u32)) -> Option<Self> {
        Self::new(0, 0, bounds.0, bounds.1, bounds)
    }

    /// Quantize two unordered image edges into a non-empty in-bounds rectangle.
    #[allow(dead_code)] // Phase 2 review constructs rectangles from edited endpoints.
    pub fn from_points(first: ImagePoint, second: ImagePoint, bounds: (u32, u32)) -> Option<Self> {
        pixel_span(first, second, bounds)?.try_into().ok()
    }

    /// Translate the rectangle and clamp its origin while preserving its size.
    #[allow(dead_code)] // Phase 2 review uses this for keyboard nudging.
    pub fn translated_clamped(
        self,
        delta_x: i64,
        delta_y: i64,
        bounds: (u32, u32),
    ) -> Option<Self> {
        let max_x = bounds.0.checked_sub(self.width)?;
        let max_y = bounds.1.checked_sub(self.height)?;
        let x = (i128::from(self.x) + i128::from(delta_x)).clamp(0, i128::from(max_x)) as u32;
        let y = (i128::from(self.y) + i128::from(delta_y)).clamp(0, i128::from(max_y)) as u32;
        Self::new(x, y, self.width, self.height, bounds)
    }
}

impl TryFrom<PixelSpan> for ImagePixelRect {
    type Error = ();

    fn try_from(span: PixelSpan) -> Result<Self, Self::Error> {
        if span.width == 0 || span.height == 0 {
            return Err(());
        }
        Ok(Self {
            x: span.x,
            y: span.y,
            width: span.width,
            height: span.height,
        })
    }
}

/// Tightly packed premultiplied ARGB32 pixels in Cairo's native byte order.
#[derive(Clone, PartialEq, Eq)]
pub struct PackedArgb32 {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) stride: i32,
    pub(crate) data: Vec<u8>,
}

impl PackedArgb32 {
    /// Construct a buffer whose stride and byte length exactly match its dimensions.
    pub fn new(width: u32, height: u32, stride: i32, data: Vec<u8>) -> Option<Self> {
        let row_bytes = width.checked_mul(4)?;
        if stride != i32::try_from(row_bytes).ok()? {
            return None;
        }
        let byte_len = usize::try_from(row_bytes)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?;
        if data.len() != byte_len {
            return None;
        }
        Some(Self {
            width,
            height,
            stride,
            data,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn stride(&self) -> i32 {
        self.stride
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl std::fmt::Debug for PackedArgb32 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PackedArgb32")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .field("bytes", &self.data.len())
            .finish()
    }
}

/// Resource limits shared by persisted embedded images and clipboard images.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedImageLimits {
    max_bytes: usize,
    max_pixels: u64,
}

impl EmbeddedImageLimits {
    const DEFAULT_MAX_BYTES: usize = 3 * 1024 * 1024;
    const DEFAULT_MAX_PIXELS: u64 = 48_000_000;

    pub const fn max_bytes(self) -> usize {
        self.max_bytes
    }

    pub const fn max_pixels(self) -> u64 {
        self.max_pixels
    }

    pub const fn allows_bytes(self, byte_len: usize) -> bool {
        byte_len <= self.max_bytes
    }

    pub fn allows_pixels(self, width: u32, height: u32) -> bool {
        u64::from(width)
            .checked_mul(u64::from(height))
            .is_some_and(|pixels| pixels <= self.max_pixels)
    }
}

impl Default for EmbeddedImageLimits {
    fn default() -> Self {
        Self {
            max_bytes: Self::DEFAULT_MAX_BYTES,
            max_pixels: Self::DEFAULT_MAX_PIXELS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_image_limits_keep_the_existing_clipboard_boundaries() {
        let limits = EmbeddedImageLimits::default();

        assert_eq!(limits.max_bytes(), 3 * 1024 * 1024);
        assert_eq!(limits.max_pixels(), 48_000_000);
        assert!(limits.allows_bytes(3 * 1024 * 1024));
        assert!(!limits.allows_bytes(3 * 1024 * 1024 + 1));
        assert!(limits.allows_pixels(8_000, 6_000));
        assert!(!limits.allows_pixels(8_000, 6_001));
    }

    #[test]
    fn image_pixel_rect_constructor_requires_a_non_empty_in_bounds_rectangle() {
        assert_eq!(
            ImagePixelRect::new(2, 3, 4, 5, (10, 10)).map(ImagePixelRect::size),
            Some((4, 5))
        );
        assert_eq!(ImagePixelRect::new(2, 3, 0, 5, (10, 10)), None);
        assert_eq!(ImagePixelRect::new(2, 3, 4, 0, (10, 10)), None);
        assert_eq!(ImagePixelRect::new(8, 3, 4, 5, (10, 10)), None);
        assert_eq!(ImagePixelRect::new(2, 8, 4, 5, (10, 10)), None);
        assert_eq!(ImagePixelRect::new(u32::MAX, 0, 2, 1, (10, 10)), None);
    }

    #[test]
    fn whole_and_translation_preserve_the_non_empty_in_bounds_invariant() {
        let whole = ImagePixelRect::whole((1, 4)).expect("non-zero bounds");
        assert_eq!((whole.x(), whole.y(), whole.size()), (0, 0, (1, 4)));
        assert_eq!(ImagePixelRect::whole((0, 4)), None);
        assert_eq!(ImagePixelRect::whole((4, 0)), None);

        let rect = ImagePixelRect::new(2, 3, 4, 2, (10, 10)).unwrap();
        let lower_right = rect.translated_clamped(20, 20, (10, 10)).unwrap();
        assert_eq!(
            (lower_right.x(), lower_right.y(), lower_right.size()),
            (6, 8, (4, 2))
        );
        let upper_left = rect
            .translated_clamped(i64::MIN, i64::MIN, (10, 10))
            .unwrap();
        assert_eq!((upper_left.x(), upper_left.y()), (0, 0));
        assert_eq!(rect.translated_clamped(0, 0, (3, 10)), None);
    }

    #[test]
    fn anchor_snapping_floors_and_clamps_to_existing_pixel_indices() {
        assert_eq!(
            snap_anchor(ImagePoint::new(2.9, 3.1), (5, 6)),
            Some(ImagePoint::new(2.0, 3.0))
        );
        assert_eq!(
            snap_anchor(ImagePoint::new(-4.0, 99.0), (5, 6)),
            Some(ImagePoint::new(0.0, 5.0))
        );
        assert_eq!(
            snap_anchor(ImagePoint::new(99.0, 2.8), (1, 5)),
            Some(ImagePoint::new(0.0, 2.0))
        );
        assert_eq!(
            snap_anchor(ImagePoint::new(4.0, 8.0), (1, 1)),
            Some(ImagePoint::new(0.0, 0.0))
        );
    }

    #[test]
    fn edge_clamping_keeps_the_far_pixel_edge_inclusive() {
        assert_eq!(
            clamp_edge(ImagePoint::new(-0.2, 8.0), (5, 6)),
            Some(ImagePoint::new(0.0, 6.0))
        );
        assert_eq!(
            clamp_edge(ImagePoint::new(1.0, 5.0), (1, 5)),
            Some(ImagePoint::new(1.0, 5.0))
        );
        assert_eq!(
            clamp_edge(ImagePoint::new(0.5, 0.75), (1, 1)),
            Some(ImagePoint::new(0.5, 0.75))
        );
    }

    #[test]
    fn point_mapping_rejects_non_finite_coordinates_and_zero_bounds() {
        for point in [
            ImagePoint::new(f64::NAN, 0.0),
            ImagePoint::new(0.0, f64::NAN),
            ImagePoint::new(f64::INFINITY, 0.0),
            ImagePoint::new(0.0, f64::NEG_INFINITY),
        ] {
            assert_eq!(snap_anchor(point, (5, 5)), None);
            assert_eq!(clamp_edge(point, (5, 5)), None);
        }
        for bounds in [(0, 0), (0, 5), (5, 0)] {
            assert_eq!(snap_anchor(ImagePoint::new(1.0, 1.0), bounds), None);
            assert_eq!(clamp_edge(ImagePoint::new(1.0, 1.0), bounds), None);
        }
    }

    #[test]
    fn square_cursor_uses_the_dominant_axis_and_preserves_direction() {
        assert_eq!(
            squared_cursor(
                ImagePoint::new(4.0, 4.0),
                ImagePoint::new(8.0, 6.0),
                (10, 10)
            ),
            Some(ImagePoint::new(8.0, 8.0))
        );
        assert_eq!(
            squared_cursor(
                ImagePoint::new(4.0, 4.0),
                ImagePoint::new(2.0, 9.0),
                (10, 10)
            ),
            Some(ImagePoint::new(0.0, 8.0))
        );
        assert_eq!(
            squared_cursor(
                ImagePoint::new(6.0, 6.0),
                ImagePoint::new(1.0, 2.0),
                (10, 10)
            ),
            Some(ImagePoint::new(1.0, 1.0))
        );
    }

    #[test]
    fn square_cursor_treats_zero_as_positive_and_flips_a_blocked_edge() {
        assert_eq!(
            squared_cursor(
                ImagePoint::new(2.0, 2.0),
                ImagePoint::new(2.0, 5.0),
                (10, 10)
            ),
            Some(ImagePoint::new(5.0, 5.0))
        );
        assert_eq!(
            squared_cursor(
                ImagePoint::new(2.0, 2.0),
                ImagePoint::new(5.0, 2.0),
                (10, 10)
            ),
            Some(ImagePoint::new(5.0, 5.0))
        );
        assert_eq!(
            squared_cursor(
                ImagePoint::new(0.0, 2.0),
                ImagePoint::new(-3.0, 5.0),
                (5, 5)
            ),
            Some(ImagePoint::new(3.0, 5.0))
        );
        assert_eq!(
            squared_cursor(ImagePoint::new(2.0, 2.0), ImagePoint::new(2.0, 2.0), (5, 5)),
            Some(ImagePoint::new(2.0, 2.0))
        );
    }

    #[test]
    fn square_cursor_selects_the_only_pixel_and_caps_one_pixel_wide_images() {
        assert_eq!(
            squared_cursor(ImagePoint::new(0.0, 0.0), ImagePoint::new(9.0, 9.0), (1, 1)),
            Some(ImagePoint::new(1.0, 1.0))
        );
        assert_eq!(
            squared_cursor(ImagePoint::new(0.0, 2.0), ImagePoint::new(0.0, 8.0), (1, 9)),
            Some(ImagePoint::new(1.0, 3.0))
        );
    }

    #[test]
    fn square_cursor_rejects_invalid_inputs_and_caps_finite_extremes() {
        assert_eq!(
            squared_cursor(
                ImagePoint::new(-1.0, 0.0),
                ImagePoint::new(1.0, 1.0),
                (5, 5)
            ),
            None
        );
        assert_eq!(
            squared_cursor(
                ImagePoint::new(0.0, 0.0),
                ImagePoint::new(f64::INFINITY, 1.0),
                (5, 5)
            ),
            None
        );
        assert_eq!(
            squared_cursor(ImagePoint::new(0.0, 0.0), ImagePoint::new(1.0, 1.0), (0, 5)),
            None
        );
        assert_eq!(
            squared_cursor(
                ImagePoint::new(0.0, 0.0),
                ImagePoint::new(f64::MAX, -f64::MAX),
                (5, 5)
            ),
            Some(ImagePoint::new(5.0, 5.0))
        );
    }

    #[test]
    fn pixel_span_floors_minimums_ceils_maximums_and_supports_reversed_drags() {
        let forward =
            pixel_span(ImagePoint::new(0.2, 1.8), ImagePoint::new(3.1, 4.0), (8, 8)).unwrap();
        let reversed =
            pixel_span(ImagePoint::new(3.1, 4.0), ImagePoint::new(0.2, 1.8), (8, 8)).unwrap();
        assert_eq!(forward, reversed);
        assert_eq!((forward.x(), forward.y(), forward.size()), (0, 1, (4, 3)));
        assert_eq!(
            ImagePixelRect::try_from(forward).map(ImagePixelRect::size),
            Ok((4, 3))
        );
        assert_eq!(
            ImagePixelRect::from_points(
                ImagePoint::new(3.1, 4.0),
                ImagePoint::new(0.2, 1.8),
                (8, 8)
            ),
            ImagePixelRect::new(0, 1, 4, 3, (8, 8))
        );
    }

    #[test]
    fn pixel_span_clamps_to_bounds_and_can_represent_an_empty_drag() {
        let clipped = pixel_span(
            ImagePoint::new(-3.5, 2.2),
            ImagePoint::new(12.0, 99.0),
            (5, 6),
        )
        .unwrap();
        assert_eq!((clipped.x(), clipped.y(), clipped.size()), (0, 2, (5, 4)));

        let empty =
            pixel_span(ImagePoint::new(2.0, 3.0), ImagePoint::new(2.0, 3.0), (5, 6)).unwrap();
        assert_eq!((empty.x(), empty.y(), empty.size()), (2, 3, (0, 0)));
        assert_eq!(ImagePixelRect::try_from(empty), Err(()));

        let zero_width =
            pixel_span(ImagePoint::new(2.0, 1.0), ImagePoint::new(2.0, 4.0), (5, 6)).unwrap();
        assert_eq!(zero_width.size(), (0, 3));
        assert_eq!(ImagePixelRect::try_from(zero_width), Err(()));
    }

    #[test]
    fn pixel_span_covers_one_pixel_and_one_pixel_wide_images() {
        let sole_pixel =
            pixel_span(ImagePoint::new(0.0, 0.0), ImagePoint::new(1.0, 1.0), (1, 1)).unwrap();
        assert_eq!(sole_pixel.size(), (1, 1));
        assert_eq!(
            ImagePixelRect::try_from(sole_pixel),
            ImagePixelRect::whole((1, 1)).ok_or(())
        );

        let vertical =
            pixel_span(ImagePoint::new(1.0, 8.8), ImagePoint::new(0.0, 1.2), (1, 9)).unwrap();
        assert_eq!(
            (vertical.x(), vertical.y(), vertical.size()),
            (0, 1, (1, 8))
        );
    }

    #[test]
    fn pixel_span_rejects_non_finite_points_and_zero_bounds() {
        for point in [
            ImagePoint::new(f64::NAN, 1.0),
            ImagePoint::new(1.0, f64::INFINITY),
        ] {
            assert_eq!(pixel_span(ImagePoint::new(0.0, 0.0), point, (5, 5)), None);
            assert_eq!(pixel_span(point, ImagePoint::new(0.0, 0.0), (5, 5)), None);
            assert_eq!(ImagePixelRect::from_points(point, point, (5, 5)), None);
        }
        for bounds in [(0, 0), (0, 5), (5, 0)] {
            assert_eq!(
                pixel_span(ImagePoint::new(0.0, 0.0), ImagePoint::new(1.0, 1.0), bounds),
                None
            );
            assert_eq!(
                ImagePixelRect::from_points(
                    ImagePoint::new(0.0, 0.0),
                    ImagePoint::new(1.0, 1.0),
                    bounds
                ),
                None
            );
        }
    }

    #[test]
    fn packed_argb32_accepts_only_exact_tightly_packed_storage() {
        let pixels = PackedArgb32::new(2, 3, 8, vec![0xA5; 24]).expect("valid packed pixels");
        assert_eq!(
            (pixels.width(), pixels.height(), pixels.stride()),
            (2, 3, 8)
        );
        assert_eq!(pixels.data(), &[0xA5; 24]);
        assert_eq!(PackedArgb32::new(2, 3, 12, vec![0; 36]), None);
        assert_eq!(PackedArgb32::new(2, 3, 8, vec![0; 23]), None);
        assert_eq!(PackedArgb32::new(u32::MAX, 1, -4, Vec::new()), None);
    }

    #[test]
    fn packed_argb32_debug_redacts_captured_bytes() {
        let pixels = PackedArgb32::new(3, 4, 12, vec![0xFF; 48]).unwrap();
        let rendered = format!("{pixels:?}");
        assert!(rendered.contains("width: 3"));
        assert!(rendered.contains("bytes: 48"));
        assert!(!rendered.contains("255"));
    }
}
