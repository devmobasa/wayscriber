//! Stable hit geometry shared by input routing and painting.

use crate::pin::PinFrame;

pub(crate) const CONTROL_SIZE: f64 = 28.0;
pub(crate) const CONTROL_GAP: f64 = 4.0;
pub(crate) const CONTROL_INSET: f64 = 6.0;
pub(crate) const CONTROL_STRIP_HEIGHT: f64 = CONTROL_SIZE + CONTROL_INSET * 2.0;
pub(crate) const CHROME_PADDING: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Control {
    Copy,
    Close,
}

pub(crate) fn control_at(frame: PinFrame, position: (f64, f64)) -> Option<Control> {
    let (x, y) = position;
    if x < 0.0 || !(CONTROL_INSET..CONTROL_INSET + CONTROL_SIZE).contains(&y) {
        return None;
    }
    let right = f64::from(frame.width) - CONTROL_INSET;
    let close_left = right - CONTROL_SIZE;
    let copy_left = close_left - CONTROL_GAP - CONTROL_SIZE;
    if (close_left..right).contains(&x) {
        Some(Control::Close)
    } else if (copy_left..copy_left + CONTROL_SIZE).contains(&x) {
        Some(Control::Copy)
    } else {
        None
    }
}

pub(crate) fn content_position(position: (f64, f64)) -> (f64, f64) {
    (
        position.0 - f64::from(CHROME_PADDING),
        position.1 - f64::from(CHROME_PADDING),
    )
}

pub(crate) fn surface_size(frame: PinFrame) -> Option<(u32, u32)> {
    let chrome = CHROME_PADDING.checked_mul(2)?;
    Some((
        frame.width.checked_add(chrome)?,
        frame.height.checked_add(chrome)?,
    ))
}

pub(crate) fn surface_origin(frame: PinFrame) -> (i32, i32) {
    let padding = i32::try_from(CHROME_PADDING).unwrap_or(i32::MAX);
    (
        frame.x.saturating_sub(padding),
        frame.y.saturating_sub(padding),
    )
}

pub(crate) fn control_strip(frame: PinFrame) -> (i32, i32, u32, u32) {
    let width = (CONTROL_SIZE * 2.0 + CONTROL_GAP + CONTROL_INSET * 2.0)
        .ceil()
        .min(f64::from(frame.width)) as u32;
    (
        i32::try_from(frame.width.saturating_sub(width)).unwrap_or(i32::MAX),
        0,
        width,
        CONTROL_STRIP_HEIGHT.ceil().min(f64::from(frame.height)) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_are_distinct_and_stay_at_top_right() {
        let frame = PinFrame::new(0, 0, 300, 200).unwrap();
        assert_eq!(control_at(frame, (288.0, 20.0)), Some(Control::Close));
        assert_eq!(control_at(frame, (254.0, 20.0)), Some(Control::Copy));
        assert_eq!(control_at(frame, (220.0, 20.0)), None);
        assert_eq!(control_at(frame, (288.0, 50.0)), None);
    }

    #[test]
    fn content_surface_transform_accounts_for_outer_chrome() {
        let frame = PinFrame::new(40, 30, 300, 200).unwrap();
        assert_eq!(surface_size(frame), Some((316, 216)));
        assert_eq!(surface_origin(frame), (32, 22));
        assert_eq!(content_position((18.5, 13.0)), (10.5, 5.0));
    }
}
