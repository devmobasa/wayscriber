use super::*;

impl ActiveScreenRegion {
    pub(super) fn begin_selection(&mut self, logical: (f64, f64)) -> bool {
        if let Self::Measure {
            bounds,
            anchor,
            edge,
            ..
        } = self
        {
            let Some(point) = measure_anchor(logical, *bounds) else {
                return false;
            };
            *anchor = Some(point);
            *edge = Some(point);
            return true;
        }
        let Self::Ready {
            purpose,
            source,
            anchor,
            raw_edge,
            logical_anchor,
            logical_edge,
            legend_dismissed,
            ..
        } = self
        else {
            return false;
        };
        if anchor.is_some()
            || raw_edge.is_some()
            || logical_anchor.is_some()
            || logical_edge.is_some()
        {
            return false;
        }
        let Some(mapped) = clamp_edge(
            image_point_for_screen_point(source, logical),
            source.image_size,
        ) else {
            return false;
        };
        let Some(anchor_point) = geometry::selection_anchor(*purpose, mapped, source.image_size)
        else {
            return false;
        };
        *anchor = Some(anchor_point);
        *raw_edge = Some(if purpose.is_capture() {
            anchor_point
        } else {
            mapped
        });
        *logical_anchor = Some(logical);
        *logical_edge = Some(logical);
        *legend_dismissed = true;
        true
    }

    pub(super) fn update_endpoint(&mut self, logical: (f64, f64)) -> bool {
        if let Self::Measure {
            bounds,
            anchor: Some(anchor),
            edge,
            ..
        } = self
        {
            let Some(point) = measure_edge(*anchor, logical, *bounds) else {
                return false;
            };
            *edge = Some(point);
            return true;
        }
        let Self::Ready {
            source,
            raw_edge,
            logical_edge,
            ..
        } = self
        else {
            return false;
        };
        let Some(mapped) = clamp_edge(
            image_point_for_screen_point(source, logical),
            source.image_size,
        ) else {
            return false;
        };
        *raw_edge = Some(mapped);
        *logical_edge = Some(logical);
        true
    }
}
