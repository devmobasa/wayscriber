use std::sync::Arc;

use crate::canvas_export::page::{
    CanvasExportBackdropSnapshot, CanvasExportRect, CanvasPageExportSnapshot, ExportBackdrop,
    SpotlightPassSnapshot, draw_canvas_page_region,
};
use crate::canvas_export::png::encode_surface_png;
use crate::capture::{CaptureError, RenderedImage};
use crate::draw::Frame;
use crate::screen_pixels::{ImagePixelRect, PackedArgb32, ScreenImage};

#[derive(Debug, Clone)]
pub(crate) struct CanvasRegionSource {
    pub image: std::sync::Arc<ScreenImage>,
    /// World-space bounds occupied by the entire captured image.
    pub logical_bounds: CanvasExportRect,
}

#[derive(Debug, Clone)]
pub(crate) struct CanvasRegionExportSnapshot {
    pub source: CanvasRegionSource,
    pub selection: ImagePixelRect,
    pub frame: Frame,
    pub spotlight: SpotlightPassSnapshot,
}

impl CanvasRegionSource {
    fn selection_source_rect(&self, selection: ImagePixelRect) -> Option<CanvasExportRect> {
        if selection.x().checked_add(selection.width())? > self.image.width
            || selection.y().checked_add(selection.height())? > self.image.height
        {
            return None;
        }
        let scale_x = self.logical_bounds.width / f64::from(self.image.width);
        let scale_y = self.logical_bounds.height / f64::from(self.image.height);
        CanvasExportRect::new(
            self.logical_bounds.x + f64::from(selection.x()) * scale_x,
            self.logical_bounds.y + f64::from(selection.y()) * scale_y,
            f64::from(selection.width()) * scale_x,
            f64::from(selection.height()) * scale_y,
        )
    }

    fn copy_selection(&self, selection: ImagePixelRect) -> Result<PackedArgb32, CaptureError> {
        let source_stride = usize::try_from(self.image.stride)
            .map_err(|_| CaptureError::ImageError("Region source stride is invalid".to_string()))?;
        let row_bytes = usize::try_from(selection.width())
            .ok()
            .and_then(|width| width.checked_mul(4))
            .ok_or_else(|| CaptureError::ImageError("Region row size overflow".to_string()))?;
        let target_stride = i32::try_from(row_bytes)
            .map_err(|_| CaptureError::ImageError("Region stride is too large".to_string()))?;
        let capacity = row_bytes
            .checked_mul(selection.height() as usize)
            .ok_or_else(|| CaptureError::ImageError("Region buffer size overflow".to_string()))?;
        let start_column = usize::try_from(selection.x())
            .ok()
            .and_then(|x| x.checked_mul(4))
            .ok_or_else(|| CaptureError::ImageError("Region offset overflow".to_string()))?;
        let mut data = Vec::with_capacity(capacity);
        for row in 0..selection.height() {
            let start = usize::try_from(selection.y().saturating_add(row))
                .ok()
                .and_then(|row| row.checked_mul(source_stride))
                .and_then(|row| row.checked_add(start_column))
                .ok_or_else(|| CaptureError::ImageError("Region offset overflow".to_string()))?;
            let end = start
                .checked_add(row_bytes)
                .ok_or_else(|| CaptureError::ImageError("Region offset overflow".to_string()))?;
            data.extend_from_slice(self.image.data.get(start..end).ok_or_else(|| {
                CaptureError::ImageError("Region leaves the captured image".to_string())
            })?);
        }
        PackedArgb32::new(selection.width(), selection.height(), target_stride, data)
            .ok_or_else(|| CaptureError::ImageError("Region pixels are invalid".to_string()))
    }

    fn magnifier_working_selection(
        &self,
        selection: ImagePixelRect,
        frame: &Frame,
    ) -> Option<ImagePixelRect> {
        let scale_x = f64::from(self.image.width) / self.logical_bounds.width;
        let scale_y = f64::from(self.image.height) / self.logical_bounds.height;
        let selection_left = selection.x();
        let selection_top = selection.y();
        let selection_right = selection.x().checked_add(selection.width())?;
        let selection_bottom = selection.y().checked_add(selection.height())?;
        let mut left = selection_left;
        let mut top = selection_top;
        let mut right = selection_right;
        let mut bottom = selection_bottom;

        for region in crate::draw::spotlight_regions_for_frame(frame) {
            if !crate::draw::spotlight_magnification_is_active(region.magnification) {
                continue;
            }
            let region_left = (((region.cx - region.rx.abs() - self.logical_bounds.x) * scale_x)
                .floor() as i64
                - 1)
            .clamp(0, i64::from(self.image.width)) as u32;
            let region_top = (((region.cy - region.ry.abs() - self.logical_bounds.y) * scale_y)
                .floor() as i64
                - 1)
            .clamp(0, i64::from(self.image.height)) as u32;
            let region_right = (((region.cx + region.rx.abs() - self.logical_bounds.x) * scale_x)
                .ceil() as i64
                + 1)
            .clamp(0, i64::from(self.image.width)) as u32;
            let region_bottom = (((region.cy + region.ry.abs() - self.logical_bounds.y) * scale_y)
                .ceil() as i64
                + 1)
            .clamp(0, i64::from(self.image.height)) as u32;

            if region_left < selection_right
                && region_right > selection_left
                && region_top < selection_bottom
                && region_bottom > selection_top
            {
                left = left.min(region_left);
                top = top.min(region_top);
                right = right.max(region_right);
                bottom = bottom.max(region_bottom);
            }
        }

        ImagePixelRect::new(
            left,
            top,
            right.checked_sub(left)?,
            bottom.checked_sub(top)?,
            (self.image.width, self.image.height),
        )
    }
}

pub(crate) fn render_canvas_region_png(
    snapshot: CanvasRegionExportSnapshot,
) -> Result<RenderedImage, CaptureError> {
    let working_selection = snapshot
        .source
        .magnifier_working_selection(snapshot.selection, &snapshot.frame)
        .ok_or_else(|| CaptureError::ImageError("Region working area is invalid".to_string()))?;
    let working_source_rect = snapshot
        .source
        .selection_source_rect(working_selection)
        .ok_or_else(|| CaptureError::ImageError("Region source mapping is invalid".to_string()))?;
    let pixels = snapshot.source.copy_selection(working_selection)?;
    let working_width = pixels.width();
    let working_height = pixels.height();
    let width = i32::try_from(working_width)
        .map_err(|_| CaptureError::ImageError("Region width is too large".to_string()))?;
    let height = i32::try_from(working_height)
        .map_err(|_| CaptureError::ImageError("Region height is too large".to_string()))?;
    let stride = pixels.stride();
    let surface = cairo::ImageSurface::create_for_data(
        pixels.into_data(),
        cairo::Format::ARgb32,
        width,
        height,
        stride,
    )
    .map_err(|err| CaptureError::ImageError(format!("Failed to create region surface: {err}")))?;
    let ctx = cairo::Context::new(&surface).map_err(|err| {
        CaptureError::ImageError(format!("Failed to create region context: {err}"))
    })?;
    let backdrop = ExportBackdrop::from_region_source(
        Arc::clone(&snapshot.source.image),
        snapshot.source.logical_bounds,
    )?;
    let page = CanvasPageExportSnapshot {
        frame: snapshot.frame,
        backdrop: CanvasExportBackdropSnapshot::Transparent,
        viewport_width: working_width,
        viewport_height: working_height,
        origin_x: working_source_rect.x.floor() as i32,
        origin_y: working_source_rect.y.floor() as i32,
        spotlight: snapshot.spotlight,
    };
    let destination = CanvasExportRect::new(
        0.0,
        0.0,
        f64::from(working_width),
        f64::from(working_height),
    )
    .expect("validated non-empty destination");
    draw_canvas_page_region(
        &ctx,
        &page,
        &backdrop,
        working_source_rect,
        destination,
        false,
        Some((working_width, working_height)),
    )?;
    drop(ctx);

    if working_selection == snapshot.selection {
        return encode_surface_png(&surface, "region");
    }

    let output_width = snapshot.selection.width();
    let output_height = snapshot.selection.height();
    let output = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        i32::try_from(output_width)
            .map_err(|_| CaptureError::ImageError("Region width is too large".to_string()))?,
        i32::try_from(output_height)
            .map_err(|_| CaptureError::ImageError("Region height is too large".to_string()))?,
    )
    .map_err(|err| CaptureError::ImageError(format!("Failed to create region crop: {err}")))?;
    let crop = cairo::Context::new(&output).map_err(|err| {
        CaptureError::ImageError(format!("Failed to create region crop context: {err}"))
    })?;
    crop.set_source_surface(
        &surface,
        -f64::from(snapshot.selection.x() - working_selection.x()),
        -f64::from(snapshot.selection.y() - working_selection.y()),
    )
    .map_err(|err| CaptureError::ImageError(format!("Failed to position region crop: {err}")))?;
    crop.paint()
        .map_err(|err| CaptureError::ImageError(format!("Failed to paint region crop: {err}")))?;
    drop(crop);

    encode_surface_png(&output, "region")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{BlurStyle, EraserBrush, EraserKind, Frame, RED, Shape};

    fn solid_source(width: u32, height: u32, pixel: u32) -> CanvasRegionSource {
        CanvasRegionSource {
            image: std::sync::Arc::new(ScreenImage {
                data: (0..width.saturating_mul(height))
                    .flat_map(|_| pixel.to_ne_bytes())
                    .collect(),
                width,
                height,
                stride: (width * 4) as i32,
            }),
            logical_bounds: CanvasExportRect::new(10.0, 20.0, 4.0, 4.0).unwrap(),
        }
    }

    fn decoded_pixel(rendered: &RenderedImage, x: i32, y: i32) -> u32 {
        let mut surface = cairo::ImageSurface::create_from_png(&mut rendered.bytes.as_slice())
            .expect("region PNG decodes");
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("decoded pixels");
        let offset = y as usize * stride + x as usize * 4;
        u32::from_ne_bytes(data[offset..offset + 4].try_into().expect("pixel"))
    }

    #[test]
    fn region_renderer_maps_world_shapes_into_native_crop_pixels_and_clips() {
        let backdrop = 0xFF20_3040;
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 11,
            y: 21,
            w: 1,
            h: 1,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        frame.add_shape(Shape::Rect {
            x: 100,
            y: 100,
            w: 10,
            h: 10,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        let rendered = render_canvas_region_png(CanvasRegionExportSnapshot {
            source: solid_source(8, 8, backdrop),
            selection: ImagePixelRect::new(0, 0, 8, 8, (8, 8)).unwrap(),
            frame,
            spotlight: SpotlightPassSnapshot::default(),
        })
        .expect("region renders");

        assert_eq!((rendered.width, rendered.height), (8, 8));
        assert_eq!(decoded_pixel(&rendered, 0, 0), backdrop);
        assert_eq!(decoded_pixel(&rendered, 3, 3), 0xFFFF_0000);
        assert_eq!(decoded_pixel(&rendered, 7, 7), backdrop);
    }

    #[test]
    fn selected_output_is_cropped_while_shape_mapping_uses_the_full_source() {
        let backdrop = 0xFF20_3040;
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 11,
            y: 21,
            w: 1,
            h: 1,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        let rendered = render_canvas_region_png(CanvasRegionExportSnapshot {
            source: solid_source(8, 8, backdrop),
            selection: ImagePixelRect::new(2, 2, 4, 4, (8, 8)).unwrap(),
            frame,
            spotlight: SpotlightPassSnapshot::default(),
        })
        .expect("region renders");

        assert_eq!((rendered.width, rendered.height), (4, 4));
        assert_eq!(decoded_pixel(&rendered, 1, 1), 0xFFFF_0000);
        assert_eq!(decoded_pixel(&rendered, 3, 3), backdrop);
    }

    #[test]
    fn empty_frame_preserves_every_selected_screen_pixel() {
        let source = CanvasRegionSource {
            image: std::sync::Arc::new(ScreenImage {
                data: (0_u32..16)
                    .flat_map(|pixel| (0xFF00_0000 | pixel).to_ne_bytes())
                    .collect(),
                width: 4,
                height: 4,
                stride: 16,
            }),
            logical_bounds: CanvasExportRect::new(-2.5, 7.25, 8.0, 8.0).unwrap(),
        };
        let selection = ImagePixelRect::new(1, 1, 2, 2, (4, 4)).unwrap();
        let rendered = render_canvas_region_png(CanvasRegionExportSnapshot {
            source,
            selection,
            frame: Frame::new(),
            spotlight: SpotlightPassSnapshot::default(),
        })
        .expect("raw crop renders");

        assert_eq!(decoded_pixel(&rendered, 0, 0), 0xFF00_0005);
        assert_eq!(decoded_pixel(&rendered, 1, 0), 0xFF00_0006);
        assert_eq!(decoded_pixel(&rendered, 0, 1), 0xFF00_0009);
        assert_eq!(decoded_pixel(&rendered, 1, 1), 0xFF00_000A);
    }

    #[test]
    fn region_capture_magnifies_its_immutable_screen_source() {
        let black = 0xFF00_0000u32;
        let red = 0xFFFF_0000u32;
        let mut pixels = vec![black; 8 * 8];
        for y in 0..8 {
            pixels[y * 8 + 2] = red;
            pixels[y * 8 + 3] = red;
        }
        let source = CanvasRegionSource {
            image: Arc::new(ScreenImage {
                data: pixels.into_iter().flat_map(u32::to_ne_bytes).collect(),
                width: 8,
                height: 8,
                stride: 32,
            }),
            logical_bounds: CanvasExportRect::new(10.0, 20.0, 4.0, 4.0).unwrap(),
        };
        let mut frame = Frame::new();
        frame.add_shape(Shape::Spotlight {
            cx: 12,
            cy: 22,
            rx: 2,
            ry: 2,
            magnification: 2.0,
        });

        let rendered = render_canvas_region_png(CanvasRegionExportSnapshot {
            source,
            selection: ImagePixelRect::new(0, 0, 8, 8, (8, 8)).unwrap(),
            frame,
            spotlight: SpotlightPassSnapshot {
                dim_opacity: 0.6,
                feather: 0.0,
            },
        })
        .expect("magnified region");

        assert_eq!(decoded_pixel(&rendered, 1, 4), red);
    }

    #[test]
    fn region_capture_samples_toward_a_spotlight_center_outside_the_crop() {
        let black = 0xFF00_0000u32;
        let red = 0xFFFF_0000u32;
        let mut pixels = vec![black; 8 * 8];
        for y in 0..8 {
            pixels[y * 8 + 3] = red;
        }
        let source = CanvasRegionSource {
            image: Arc::new(ScreenImage {
                data: pixels.into_iter().flat_map(u32::to_ne_bytes).collect(),
                width: 8,
                height: 8,
                stride: 32,
            }),
            logical_bounds: CanvasExportRect::new(10.0, 20.0, 4.0, 4.0).unwrap(),
        };
        let mut frame = Frame::new();
        frame.add_shape(Shape::Spotlight {
            cx: 11,
            cy: 22,
            rx: 2,
            ry: 2,
            magnification: 2.0,
        });

        let rendered = render_canvas_region_png(CanvasRegionExportSnapshot {
            source,
            selection: ImagePixelRect::new(4, 0, 4, 8, (8, 8)).unwrap(),
            frame,
            spotlight: SpotlightPassSnapshot {
                dim_opacity: 0.6,
                feather: 0.0,
            },
        })
        .expect("magnified crop renders");

        // The sampled coordinate lands three quarters across the red source
        // texel, so Cairo's bilinear filter produces 75% red over black.
        assert_eq!(decoded_pixel(&rendered, 0, 4), 0xFFBF_0000);
    }

    #[test]
    fn malformed_full_source_fails_closed() {
        let mut source = solid_source(8, 8, 0xFFFF_FFFF);
        std::sync::Arc::get_mut(&mut source.image)
            .expect("test owns source")
            .data
            .pop();
        let result = render_canvas_region_png(CanvasRegionExportSnapshot {
            source,
            selection: ImagePixelRect::new(0, 0, 1, 1, (8, 8)).unwrap(),
            frame: Frame::new(),
            spotlight: SpotlightPassSnapshot::default(),
        });
        assert!(matches!(result, Err(CaptureError::ImageError(_))));
    }

    #[test]
    fn eraser_replays_the_original_screen_pixel_at_a_nonzero_world_origin() {
        let backdrop = 0xFF20_3040;
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 10,
            y: 20,
            w: 4,
            h: 4,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        frame.add_shape(Shape::EraserStroke {
            points: vec![(12, 22)],
            brush: EraserBrush {
                size: 1.0,
                kind: EraserKind::Rect,
            },
        });
        let rendered = render_canvas_region_png(CanvasRegionExportSnapshot {
            source: solid_source(8, 8, backdrop),
            selection: ImagePixelRect::new(0, 0, 8, 8, (8, 8)).unwrap(),
            frame,
            spotlight: SpotlightPassSnapshot::default(),
        })
        .expect("region renders");

        assert_eq!(decoded_pixel(&rendered, 4, 4), backdrop);
        assert_eq!(decoded_pixel(&rendered, 1, 1), 0xFFFF_0000);
    }

    #[test]
    fn blur_at_a_crop_edge_matches_full_image_render_then_crop() {
        let width = 32;
        let height = 16;
        let mut data = Vec::with_capacity(width * height * 4);
        for _y in 0..height {
            for x in 0..width {
                let channel = if (x / 2) % 2 == 0 { 0x10 } else { 0xE0 };
                data.extend_from_slice(
                    &u32::from_be_bytes([0xFF, channel, channel, channel]).to_ne_bytes(),
                );
            }
        }
        let source = CanvasRegionSource {
            image: std::sync::Arc::new(ScreenImage {
                data,
                width: width as u32,
                height: height as u32,
                stride: (width * 4) as i32,
            }),
            logical_bounds: CanvasExportRect::new(10.0, 20.0, width as f64, height as f64).unwrap(),
        };
        let mut frame = Frame::new();
        frame.add_shape(Shape::BlurRect {
            x: 14,
            y: 22,
            w: 14,
            h: 10,
            strength: 20.0,
            style: BlurStyle::Gaussian,
        });
        let full = render_canvas_region_png(CanvasRegionExportSnapshot {
            source: source.clone(),
            selection: ImagePixelRect::new(
                0,
                0,
                width as u32,
                height as u32,
                (width as u32, height as u32),
            )
            .unwrap(),
            frame: frame.clone_without_history(),
            spotlight: SpotlightPassSnapshot::default(),
        })
        .expect("full region renders");
        let selection = ImagePixelRect::new(8, 4, 10, 8, (width as u32, height as u32)).unwrap();
        let cropped = render_canvas_region_png(CanvasRegionExportSnapshot {
            source,
            selection,
            frame,
            spotlight: SpotlightPassSnapshot::default(),
        })
        .expect("cropped region renders");

        for y in 0..selection.height() as i32 {
            for x in 0..selection.width() as i32 {
                assert_eq!(
                    decoded_pixel(&cropped, x, y),
                    decoded_pixel(&full, x + selection.x() as i32, y + selection.y() as i32),
                    "pixel ({x}, {y})",
                );
            }
        }
    }
}
