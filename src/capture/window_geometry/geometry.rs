use crate::util::Rect;

pub(super) fn intersect(left: Rect, right: Rect) -> Option<Rect> {
    let min_x = i64::from(left.x).max(i64::from(right.x));
    let min_y = i64::from(left.y).max(i64::from(right.y));
    let max_x = (i64::from(left.x) + i64::from(left.width))
        .min(i64::from(right.x) + i64::from(right.width));
    let max_y = (i64::from(left.y) + i64::from(left.height))
        .min(i64::from(right.y) + i64::from(right.height));
    Rect::from_min_max(
        i32::try_from(min_x).ok()?,
        i32::try_from(min_y).ok()?,
        i32::try_from(max_x).ok()?,
        i32::try_from(max_y).ok()?,
    )
}

pub(super) fn localize(rect: Rect, output: Rect) -> Option<Rect> {
    Rect::new(
        rect.x.checked_sub(output.x)?,
        rect.y.checked_sub(output.y)?,
        rect.width,
        rect.height,
    )
}

/// Correlate independently reported logical output bounds.
///
/// Wayland and compositor control APIs can integerize the same fractional
/// scale one logical pixel differently. Origins must match exactly; size may
/// differ by one pixel per axis. Only their overlap is safe for targets.
pub(super) fn correlated_output_overlap(provider: Rect, source: Rect) -> Option<Rect> {
    if provider.x != source.x
        || provider.y != source.y
        || provider.width.abs_diff(source.width) > 1
        || provider.height.abs_diff(source.height) > 1
    {
        return None;
    }
    intersect(provider, source)
}
