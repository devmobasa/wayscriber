use std::sync::Arc;

use crate::canvas_export::{CanvasRegionExportSnapshot, render_canvas_region_pixels};
use crate::capture::{CaptureError, CutBand, apply_band_cuts, output_size as band_cut_output_size};
use crate::screen_pixels::{ImagePixelRect, PackedArgb32, ScreenImage};

/// Immutable pixels a region render job may flatten, then cut.
#[derive(Debug, Clone)]
pub(super) enum RegionPixelSource {
    Raw {
        image: Arc<ScreenImage>,
        selection: ImagePixelRect,
    },
    Annotated(Box<CanvasRegionExportSnapshot>),
}

#[derive(Debug, Clone)]
pub(super) struct RegionRenderRequest {
    pub source: RegionPixelSource,
    pub cuts: Vec<CutBand>,
}

impl RegionPixelSource {
    pub(super) fn selection(&self) -> ImagePixelRect {
        match self {
            Self::Raw { selection, .. } => *selection,
            Self::Annotated(snapshot) => snapshot.selection,
        }
    }
}

impl RegionRenderRequest {
    pub(super) fn output_size(&self) -> Result<(u32, u32), CaptureError> {
        let selection = self.source.selection();
        band_cut_output_size((selection.width(), selection.height()), &self.cuts)
            .map_err(band_cut_error)
    }
}

pub(super) fn render_region_base_pixels(
    source: RegionPixelSource,
) -> Result<PackedArgb32, CaptureError> {
    match source {
        RegionPixelSource::Raw { image, selection } => {
            image.copy_rect(selection).ok_or_else(|| {
                CaptureError::ImageError("Could not copy the selected screen pixels.".to_string())
            })
        }
        RegionPixelSource::Annotated(snapshot) => render_canvas_region_pixels(*snapshot),
    }
}

pub(super) fn compose_region_pixels(
    base: &PackedArgb32,
    cuts: &[CutBand],
) -> Result<PackedArgb32, CaptureError> {
    apply_band_cuts(base, cuts).map_err(band_cut_error)
}

pub(super) fn render_region_pixels(
    request: RegionRenderRequest,
) -> Result<PackedArgb32, CaptureError> {
    let _ = request.output_size()?;
    let base = render_region_base_pixels(request.source)?;
    compose_region_pixels(&base, &request.cuts)
}

/// The PNG job for a submission. Shared by every destination, so what Copy
/// writes and what Board pastes can only differ by the Review toggle and cuts.
pub(super) fn region_render_job(request: RegionRenderRequest) -> crate::capture::ImageRenderJob {
    Box::new(move || crate::capture::png::encode_packed_argb32_png(&render_region_pixels(request)?))
}

fn band_cut_error(error: crate::capture::BandCutError) -> CaptureError {
    CaptureError::ImageError(format!("Could not apply the requested cut: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_export::{CanvasExportRect, CanvasRegionSource, SpotlightPassSnapshot};
    use crate::capture::CutAxis;
    use crate::draw::{Frame, RED, Shape};

    fn swatch() -> ([u32; 6], Vec<u8>) {
        let ids = [
            0xFF11_2233,
            0xFF44_5566,
            0xFF77_8899,
            0xFFAA_BBCC,
            0xFFDD_EEFF,
            0xFF01_0203,
        ];
        let bytes = ids
            .iter()
            .copied()
            .flat_map(u32::to_ne_bytes)
            .collect::<Vec<_>>();
        (ids, bytes)
    }

    fn decode_png_pixels(bytes: &[u8]) -> Vec<u32> {
        let png = bytes.to_vec();
        let mut surface = cairo::ImageSurface::create_from_png(&mut png.as_slice()).expect("png");
        surface.flush();
        let width = surface.width() as usize;
        let height = surface.height() as usize;
        let stride = surface.stride() as usize;
        let data = surface.data().expect("pixels");
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let offset = y * stride + x * 4;
                pixels.push(u32::from_ne_bytes(
                    data[offset..offset + 4].try_into().expect("pixel"),
                ));
            }
        }
        pixels
    }

    #[test]
    fn raw_and_annotated_jobs_share_the_cut_compose_path() {
        let (ids, bytes) = swatch();
        let image = Arc::new(ScreenImage {
            data: bytes.clone(),
            width: 3,
            height: 2,
            stride: 12,
        });
        let selection = ImagePixelRect::new(0, 0, 3, 2, (3, 2)).unwrap();
        let cut = CutBand::new(CutAxis::Columns, 1, 2).unwrap();
        let rendered = region_render_job(RegionRenderRequest {
            source: RegionPixelSource::Raw {
                image: Arc::clone(&image),
                selection,
            },
            cuts: vec![cut],
        })()
        .expect("raw cut");
        assert_eq!((rendered.width, rendered.height), (2, 2));
        assert_eq!(
            decode_png_pixels(&rendered.bytes),
            vec![ids[0], ids[2], ids[3], ids[5]]
        );

        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 10,
            y: 20,
            w: 3,
            h: 2,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        let composed = region_render_job(RegionRenderRequest {
            source: RegionPixelSource::Annotated(Box::new(CanvasRegionExportSnapshot {
                source: CanvasRegionSource {
                    image,
                    logical_bounds: CanvasExportRect::new(10.0, 20.0, 3.0, 2.0).unwrap(),
                },
                selection,
                frame,
                text_halo_enabled: true,
                spotlight: SpotlightPassSnapshot {
                    dim_opacity: 0.0,
                    feather: 0.0,
                },
            })),
            cuts: vec![cut],
        })()
        .expect("annotated cut");
        assert_eq!((composed.width, composed.height), (2, 2));
        assert_ne!(composed.bytes, rendered.bytes);
    }

    #[test]
    fn empty_cuts_keep_the_source_crop() {
        let (_, bytes) = swatch();
        let request = RegionRenderRequest {
            source: RegionPixelSource::Raw {
                image: Arc::new(ScreenImage {
                    data: bytes,
                    width: 3,
                    height: 2,
                    stride: 12,
                }),
                selection: ImagePixelRect::new(0, 0, 3, 2, (3, 2)).unwrap(),
            },
            cuts: Vec::new(),
        };
        let rendered = region_render_job(request)().expect("raw");
        assert_eq!((rendered.width, rendered.height), (3, 2));
    }
}
