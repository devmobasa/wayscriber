//! Live output inventory and deterministic pin migration.

use std::collections::HashMap;

use smithay_client_toolkit::output::OutputInfo;
use wayland_client::{Proxy, backend::ObjectId, protocol::wl_output};

use crate::pin::{PinFrame, PinMemoryCharge, PinOutputTransform, PinRefusal, geometry};

#[derive(Debug, Clone)]
pub(crate) struct HostOutput {
    pub(crate) proxy: wl_output::WlOutput,
    pub(crate) connector_name: String,
    pub(crate) logical_position: (i32, i32),
    pub(crate) logical_size: (u32, u32),
    pub(crate) scale: i32,
    pub(crate) transform: wl_output::Transform,
}

#[derive(Default)]
pub(crate) struct OutputInventory {
    outputs: HashMap<ObjectId, HostOutput>,
}

impl OutputInventory {
    pub(crate) fn update(
        &mut self,
        proxy: wl_output::WlOutput,
        info: &OutputInfo,
    ) -> Option<&HostOutput> {
        let name = info.name.clone()?;
        let logical_size = info.logical_size?;
        let width = u32::try_from(logical_size.0)
            .ok()
            .filter(|value| *value > 0)?;
        let height = u32::try_from(logical_size.1)
            .ok()
            .filter(|value| *value > 0)?;
        let record = HostOutput {
            connector_name: name,
            logical_position: info.logical_position.unwrap_or(info.location),
            logical_size: (width, height),
            scale: info.scale_factor.max(1),
            transform: info.transform,
            proxy: proxy.clone(),
        };
        self.outputs.insert(proxy.id(), record);
        self.outputs.get(&proxy.id())
    }

    pub(crate) fn remove(&mut self, proxy: &wl_output::WlOutput) -> Option<HostOutput> {
        self.outputs.remove(&proxy.id())
    }

    pub(crate) fn by_connector(&self, connector: &str) -> Option<&HostOutput> {
        self.outputs
            .values()
            .find(|output| output.connector_name == connector)
    }

    pub(crate) fn by_proxy(&self, proxy: &wl_output::WlOutput) -> Option<&HostOutput> {
        self.outputs.get(&proxy.id())
    }

    pub(crate) fn deterministic_first(&self) -> Option<&HostOutput> {
        self.outputs.values().min_by_key(|output| {
            (
                output.logical_position.0,
                output.logical_position.1,
                output.connector_name.as_str(),
            )
        })
    }
}

/// Preserve normalized centre and aspect-preserving size on another output.
pub(crate) fn migrate_frame(
    frame: PinFrame,
    image_size: (u32, u32),
    previous_output_size: (u32, u32),
    next_output_size: (u32, u32),
    next_scale: u32,
) -> Result<PinFrame, crate::pin::PinRefusal> {
    let frame = geometry::migrated_frame(
        frame,
        image_size,
        previous_output_size,
        next_output_size,
        next_scale,
    )?;
    fit_frame_to_surface_limit(frame, image_size, next_output_size, next_scale)
}

pub(crate) fn output_snapshot_changed(
    hint_size: (u32, u32),
    hint_scale: u32,
    hint_transform: PinOutputTransform,
    actual_size: (u32, u32),
    actual_scale: u32,
    actual_transform: wl_output::Transform,
) -> bool {
    hint_size != actual_size
        || hint_scale != actual_scale
        || pin_transform(hint_transform) != actual_transform
}

const fn pin_transform(transform: PinOutputTransform) -> wl_output::Transform {
    match transform {
        PinOutputTransform::Normal => wl_output::Transform::Normal,
        PinOutputTransform::Rotate90 => wl_output::Transform::_90,
        PinOutputTransform::Rotate180 => wl_output::Transform::_180,
        PinOutputTransform::Rotate270 => wl_output::Transform::_270,
        PinOutputTransform::Flipped => wl_output::Transform::Flipped,
        PinOutputTransform::Flipped90 => wl_output::Transform::Flipped90,
        PinOutputTransform::Flipped180 => wl_output::Transform::Flipped180,
        PinOutputTransform::Flipped270 => wl_output::Transform::Flipped270,
    }
}

/// Apply the actual decorated surface pixel limit after domain geometry.
///
/// Domain frames describe image content. The host owns an additional border
/// and shadow on every side, so the admission predicate must include that
/// chrome before a layer surface, raster, or SHM pool is allocated.
pub(crate) fn fit_frame_to_surface_limit(
    frame: PinFrame,
    image_size: (u32, u32),
    output_size: (u32, u32),
    scale: u32,
) -> Result<PinFrame, PinRefusal> {
    let fits = |width, height| {
        let candidate = PinFrame::new(0, 0, width, height).ok_or(PinRefusal::InvalidPlacement)?;
        let surface =
            crate::pin::surface::surface_size(candidate).ok_or(PinRefusal::LimitExceeded)?;
        PinMemoryCharge::for_surface(surface.0, surface.1, scale).map(|_| ())
    };
    if fits(frame.width, frame.height).is_ok() {
        return Ok(frame);
    }
    let (image_width, image_height) = image_size;
    if image_width == 0 || image_height == 0 {
        return Err(PinRefusal::InvalidImage);
    }
    let height_for = |width: u32| {
        u64::from(width)
            .checked_mul(u64::from(image_height))
            .and_then(|value| value.checked_add(u64::from(image_width) / 2))
            .map(|value| value / u64::from(image_width))
            .and_then(|height| u32::try_from(height.max(1)).ok())
    };
    let mut low = 1_u32;
    let mut high = frame.width;
    let mut best = None;
    while low <= high {
        let width = low + (high - low) / 2;
        let Some(height) = height_for(width) else {
            return Err(PinRefusal::LimitExceeded);
        };
        if height <= frame.height && fits(width, height).is_ok() {
            best = Some((width, height));
            low = width.saturating_add(1);
        } else {
            high = width.saturating_sub(1);
        }
    }
    let (width, height) = best.ok_or(PinRefusal::LimitExceeded)?;
    let centered = (
        f64::from(frame.x) + (f64::from(frame.width) - f64::from(width)) / 2.0,
        f64::from(frame.y) + (f64::from(frame.height) - f64::from(height)) / 2.0,
    );
    let shrunk =
        PinFrame::new(frame.x, frame.y, width, height).ok_or(PinRefusal::InvalidPlacement)?;
    Ok(geometry::dragged_frame(shrunk, centered, output_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_keeps_pin_identity_geometry_visible_and_aspect() {
        let old = PinFrame::new(700, 300, 800, 400).unwrap();
        let migrated = migrate_frame(old, (800, 400), (2000, 1000), (1000, 600), 1).unwrap();
        assert_eq!(migrated.width * old.height, migrated.height * old.width);
        assert!(migrated.right() >= 32);
        assert!(migrated.bottom() >= 32);
        assert!(migrated.x <= 968);
        assert!(migrated.y <= 568);
    }

    #[test]
    fn migration_to_tiny_output_never_panics_or_loses_all_visibility() {
        let old = PinFrame::new(0, 0, 320, 160).unwrap();
        let migrated = migrate_frame(old, (320, 160), (1920, 1080), (40, 30), 1).unwrap();
        assert!(migrated.right() >= 32);
        assert!(migrated.bottom() >= 30);
    }

    #[test]
    fn migration_reapplies_scale_dependent_surface_pixel_cap() {
        let old = PinFrame::new(0, 0, 5000, 4000).unwrap();
        let migrated = migrate_frame(old, (5000, 4000), (8000, 8000), (8000, 8000), 2).unwrap();
        let surface = crate::pin::surface::surface_size(migrated).unwrap();
        assert!(u64::from(surface.0) * u64::from(surface.1) * 4 <= 16_000_000);
    }

    #[test]
    fn chrome_is_included_at_the_surface_pixel_boundary() {
        let content_only = PinFrame::new(0, 0, 2000, 2000).unwrap();
        let limited = fit_frame_to_surface_limit(content_only, (1, 1), (8000, 8000), 2).unwrap();
        let surface = crate::pin::surface::surface_size(limited).unwrap();
        assert!(u64::from(surface.0) * u64::from(surface.1) * 4 <= 16_000_000);
        assert!(limited.width < content_only.width);
    }

    #[test]
    fn same_connector_snapshot_drift_includes_size_scale_and_transform() {
        assert!(output_snapshot_changed(
            (1920, 1080),
            1,
            PinOutputTransform::Normal,
            (800, 600),
            2,
            wl_output::Transform::_90,
        ));
        assert!(output_snapshot_changed(
            (800, 600),
            1,
            PinOutputTransform::Normal,
            (800, 600),
            2,
            wl_output::Transform::Normal,
        ));
        assert!(output_snapshot_changed(
            (800, 600),
            2,
            PinOutputTransform::Normal,
            (800, 600),
            2,
            wl_output::Transform::_90,
        ));
        assert!(!output_snapshot_changed(
            (800, 600),
            2,
            PinOutputTransform::Rotate90,
            (800, 600),
            2,
            wl_output::Transform::_90,
        ));
        let migrated = migrate_frame(
            PinFrame::new(1600, 800, 600, 300).unwrap(),
            (600, 300),
            (1920, 1080),
            (800, 600),
            2,
        )
        .unwrap();
        assert!(migrated.right() >= 32 && migrated.x <= 768);
        assert!(migrated.bottom() >= 32 && migrated.y <= 568);
    }
}
