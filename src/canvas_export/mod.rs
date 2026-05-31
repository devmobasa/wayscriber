mod page;
mod pdf;
mod pdf_labels;
mod png;

pub use page::{CanvasExportBackdropSnapshot, CanvasExportRect, CanvasPageExportSnapshot};
#[allow(unused_imports)]
pub use pdf::{
    BoardPdfExportSnapshot, PdfPageExportSnapshot, PdfPageLayout, PdfPageMetadata,
    render_board_pdf, resolve_pdf_page_layout,
};
pub use png::{BoardExportSnapshot, CanvasExportSnapshot, CanvasExportViewport, render_canvas_png};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::canvas_export::page::draw_canvas_page;
    use crate::canvas_export::png::render_canvas_surface;
    use crate::config::{PdfExportConfig, RenderColorMappingConfig, RenderProfileConfig};
    use crate::draw::{BLACK, Frame, RED, Shape, WHITE};
    use crate::render_profiles::RenderColorProfile;

    fn snapshot(frame: Frame, viewport: CanvasExportViewport) -> CanvasExportSnapshot {
        CanvasExportSnapshot {
            viewport,
            backdrop: CanvasExportBackdropSnapshot::Transparent,
            board: BoardExportSnapshot { frame },
            render_profile: None,
        }
    }

    fn page_snapshot(frame: Frame) -> CanvasPageExportSnapshot {
        CanvasPageExportSnapshot {
            frame,
            backdrop: CanvasExportBackdropSnapshot::Transparent,
            viewport_width: 20,
            viewport_height: 20,
            origin_x: 0,
            origin_y: 0,
        }
    }

    fn pdf_snapshot(page: CanvasPageExportSnapshot) -> BoardPdfExportSnapshot {
        let layout = resolve_pdf_page_layout(64, 48, 0, 0, None, &PdfExportConfig::default())
            .expect("layout");
        BoardPdfExportSnapshot {
            pages: vec![PdfPageExportSnapshot {
                page,
                metadata: PdfPageMetadata::new(0, 1, 0, 1, 0, 1, 0, 1, "Board".to_string(), None),
                layout,
            }],
            labels: Default::default(),
        }
    }

    fn pixel(surface: &mut cairo::ImageSurface, x: i32, y: i32) -> u32 {
        surface.flush();
        let stride = surface.stride() as usize;
        let data = surface.data().expect("surface data");
        let offset = y as usize * stride + x as usize * 4;
        u32::from_ne_bytes(data[offset..offset + 4].try_into().expect("pixel"))
    }

    #[test]
    fn export_uses_current_viewport_origin() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 20,
            y: 10,
            w: 8,
            h: 8,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        let mut surface = render_canvas_surface(&snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 20,
                logical_height: 20,
                scale: 1,
                origin_x: 20,
                origin_y: 10,
            },
        ))
        .expect("surface");

        assert_ne!(pixel(&mut surface, 3, 3), 0);
    }

    #[test]
    fn export_scale_creates_physical_surface_and_scales_geometry() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 1,
            y: 1,
            w: 4,
            h: 4,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        let mut surface = render_canvas_surface(&snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 10,
                logical_height: 10,
                scale: 2,
                origin_x: 0,
                origin_y: 0,
            },
        ))
        .expect("surface");

        assert_eq!(surface.width(), 20);
        assert_eq!(surface.height(), 20);
        assert_ne!(pixel(&mut surface, 4, 4), 0);
        assert_eq!(pixel(&mut surface, 0, 0), 0);
    }

    #[test]
    fn draw_canvas_page_uses_explicit_output_scale() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 4,
            y: 4,
            w: 2,
            h: 2,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        let mut surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, 20, 20).expect("surface");
        {
            let ctx = cairo::Context::new(&surface).expect("context");
            draw_canvas_page(&ctx, &page_snapshot(frame), 2.0).expect("draw");
        }

        assert_ne!(pixel(&mut surface, 9, 9), 0);
        assert_eq!(pixel(&mut surface, 1, 1), 0);
    }

    #[test]
    fn render_board_pdf_returns_pdf_bytes() {
        let pdf = render_board_pdf(&pdf_snapshot(page_snapshot(Frame::new()))).expect("pdf");

        assert!(pdf.starts_with(b"%PDF-"));
    }

    #[test]
    fn render_board_pdf_rejects_zero_dimensions() {
        let mut snapshot = pdf_snapshot(page_snapshot(Frame::new()));
        snapshot.pages[0].layout.page_width = 0.0;
        let err = render_board_pdf(&snapshot).expect_err("zero width should fail");

        assert!(
            err.to_string().contains("non-empty surface"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn render_board_pdf_rejects_empty_pages() {
        let err = render_board_pdf(&BoardPdfExportSnapshot {
            pages: Vec::new(),
            labels: Default::default(),
        })
        .expect_err("empty pages should fail");

        assert!(
            err.to_string().contains("at least one page"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn export_applies_cloned_profile_to_pixels() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 6,
            h: 6,
            fill: true,
            color: BLACK,
            thick: 1.0,
        });
        let profile = RenderColorProfile::from_config(&RenderProfileConfig {
            id: "print".to_string(),
            name: "Print".to_string(),
            mappings: vec![RenderColorMappingConfig {
                from: "#000000".to_string(),
                to: "#FFFFFF".to_string(),
            }],
        })
        .expect("profile");
        let mut export = snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 8,
                logical_height: 8,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        );
        export.render_profile = Some(profile);

        let mut surface = render_canvas_surface(&export).expect("surface");

        assert_eq!(pixel(&mut surface, 2, 2), 0xffffffff);
    }

    #[test]
    fn export_replays_eraser_on_solid_background() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 12,
            h: 12,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        frame.add_shape(Shape::EraserStroke {
            points: vec![(6, 6)],
            brush: crate::draw::EraserBrush {
                size: 6.0,
                kind: crate::draw::EraserKind::Circle,
            },
        });
        let mut export = snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 14,
                logical_height: 14,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        );
        export.backdrop = CanvasExportBackdropSnapshot::Solid(WHITE);
        let mut surface = render_canvas_surface(&export).expect("surface");

        assert_eq!(pixel(&mut surface, 6, 6), 0xffffffff);
    }

    #[test]
    fn export_replays_eraser_on_transparent_background() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 12,
            h: 12,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        frame.add_shape(Shape::EraserStroke {
            points: vec![(6, 6)],
            brush: crate::draw::EraserBrush {
                size: 6.0,
                kind: crate::draw::EraserKind::Circle,
            },
        });
        let mut surface = render_canvas_surface(&snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 14,
                logical_height: 14,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        ))
        .expect("surface");

        assert_eq!(pixel(&mut surface, 6, 6), 0);
    }

    #[test]
    fn export_blur_uses_placeholder_without_persisted_backdrop() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::BlurRect {
            x: 2,
            y: 2,
            w: 8,
            h: 8,
            strength: 12.0,
        });
        let mut surface = render_canvas_surface(&snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 14,
                logical_height: 14,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        ))
        .expect("surface");

        assert_ne!(pixel(&mut surface, 5, 5), 0);
    }

    #[test]
    fn export_blur_replays_against_persisted_image_backdrop() {
        let width = 16;
        let height = 16;
        let stride = width * 4;
        let mut data = vec![0u8; (stride * height) as usize];
        for y in 0..height {
            for x in 0..width {
                let offset = (y * stride + x * 4) as usize;
                let red = if x < 8 { 255 } else { 0 };
                let blue = if x < 8 { 0 } else { 255 };
                data[offset..offset + 4].copy_from_slice(
                    &(0xff000000u32 | ((red as u32) << 16) | blue as u32).to_ne_bytes(),
                );
            }
        }
        let mut frame = Frame::new();
        frame.add_shape(Shape::BlurRect {
            x: 4,
            y: 4,
            w: 8,
            h: 8,
            strength: 12.0,
        });
        let mut export = snapshot(
            frame,
            CanvasExportViewport {
                logical_width: width as u32,
                logical_height: height as u32,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        );
        export.backdrop = CanvasExportBackdropSnapshot::PersistedImage {
            data: Arc::from(data),
            width,
            height,
            stride,
            logical_to_image_scale_x: 1.0,
            logical_to_image_scale_y: 1.0,
        };
        let mut surface = render_canvas_surface(&export).expect("surface");

        assert_ne!(pixel(&mut surface, 6, 6), 0);
        assert_ne!(pixel(&mut surface, 6, 6), pixel(&mut surface, 1, 1));
    }

    #[test]
    fn export_rejects_invalid_persisted_image_backdrop_buffer() {
        let mut export = snapshot(
            Frame::new(),
            CanvasExportViewport {
                logical_width: 4,
                logical_height: 4,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        );
        export.backdrop = CanvasExportBackdropSnapshot::PersistedImage {
            data: Arc::from(vec![0u8; 8]),
            width: 4,
            height: 4,
            stride: 16,
            logical_to_image_scale_x: 1.0,
            logical_to_image_scale_y: 1.0,
        };

        let err = match render_canvas_surface(&export) {
            Ok(_) => panic!("short backdrop must fail"),
            Err(err) => err,
        };

        assert!(
            err.to_string().contains("buffer is too small"),
            "unexpected error: {err}"
        );
    }
}
