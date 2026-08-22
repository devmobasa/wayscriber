use super::*;

impl ActiveScreenRegion {
    pub(super) fn enter_review(&mut self, rect: ImagePixelRect) -> Option<RegionSelection> {
        let Self::Ready {
            purpose,
            anchor,
            raw_edge,
            logical_anchor,
            logical_edge,
            ..
        } = self
        else {
            return None;
        };
        if *purpose != RegionPurposeTag::CaptureInteractive {
            return None;
        }
        *anchor = None;
        *raw_edge = None;
        *logical_anchor = None;
        *logical_edge = None;
        *anchor = Some(ImagePoint::new(f64::from(rect.x()), f64::from(rect.y())));
        *raw_edge = Some(ImagePoint::new(
            f64::from(rect.x() + rect.width()),
            f64::from(rect.y() + rect.height()),
        ));
        self.review_geometry()
            .map(|geometry| geometry.display_selection())
    }

    pub(super) fn begin_review_move(&mut self, logical: (f64, f64)) -> bool {
        let Some(rect) = (*self).stored_review_rect() else {
            return false;
        };
        let Self::Ready {
            source,
            logical_anchor,
            ..
        } = self
        else {
            return false;
        };
        if logical_anchor.is_some() {
            return false;
        }
        let display =
            crate::backend::wayland::state::screen_image::screen_rect_for_image_rect(source, rect);
        let x = logical.0.floor() as i32;
        let y = logical.1.floor() as i32;
        if x < display.x
            || y < display.y
            || x >= display.x.saturating_add(display.width)
            || y >= display.y.saturating_add(display.height)
        {
            return false;
        }
        let origin = image_point_for_screen_point(source, logical);
        *logical_anchor = Some((origin.x, origin.y));
        true
    }

    pub(super) fn update_review_move(&mut self, logical: (f64, f64)) -> bool {
        let Some(image_rect) = (*self).stored_review_rect() else {
            return false;
        };
        let Self::Ready {
            source,
            logical_anchor: Some(origin),
            ..
        } = self
        else {
            return false;
        };
        let current = image_point_for_screen_point(source, logical);
        let delta_x = (current.x - origin.0).round() as i64;
        let delta_y = (current.y - origin.1).round() as i64;
        if delta_x == 0 && delta_y == 0 {
            return false;
        }
        let Some(next) = image_rect.translated_clamped(delta_x, delta_y, source.image_size) else {
            return false;
        };
        if image_rect == next {
            return false;
        }
        let applied_x = i64::from(next.x()) - i64::from(image_rect.x());
        let applied_y = i64::from(next.y()) - i64::from(image_rect.y());
        let next_origin = (origin.0 + applied_x as f64, origin.1 + applied_y as f64);
        if !self.store_review_rect(next) {
            return false;
        }
        if let Self::Ready { logical_anchor, .. } = self {
            *logical_anchor = Some(next_origin);
        }
        true
    }

    pub(super) fn finish_review_move(&mut self) -> bool {
        let Self::Ready { logical_anchor, .. } = self else {
            return false;
        };
        if logical_anchor.is_none() {
            return false;
        }
        *logical_anchor = None;
        true
    }

    pub(super) fn reset_review_for_selection(&mut self) {
        if let Self::Ready {
            anchor,
            raw_edge,
            logical_anchor,
            ..
        } = self
        {
            *anchor = None;
            *raw_edge = None;
            *logical_anchor = None;
        }
    }

    pub(super) fn nudge_review(&mut self, delta_x: i64, delta_y: i64) -> Option<RegionSelection> {
        let review_rect = (*self).stored_review_rect()?;
        let Self::Ready {
            source,
            logical_anchor,
            ..
        } = self
        else {
            return None;
        };
        if logical_anchor.is_some() {
            return None;
        }
        let rect = review_rect.translated_clamped(delta_x, delta_y, source.image_size)?;
        self.store_review_rect(rect);
        self.review_geometry()
            .map(|geometry| geometry.display_selection())
    }
}
