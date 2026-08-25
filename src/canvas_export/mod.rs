mod page;
mod pdf;
mod pdf_labels;
mod png;
mod region;

pub use page::{
    CanvasExportBackdropSnapshot, CanvasExportRect, CanvasPageExportSnapshot, SpotlightPassSnapshot,
};
#[allow(unused_imports)]
pub use pdf::{
    BoardPdfExportSnapshot, PdfPageExportSnapshot, PdfPageLayout, PdfPageMetadata,
    render_board_pdf, resolve_pdf_page_layout,
};
pub use png::{BoardExportSnapshot, CanvasExportSnapshot, CanvasExportViewport, render_canvas_png};
pub(crate) use region::{CanvasRegionExportSnapshot, CanvasRegionSource, render_canvas_region_png};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::canvas_export::page::draw_canvas_page;
    use crate::canvas_export::png::render_canvas_surface;
    use crate::config::{PdfExportConfig, RenderColorMappingConfig, RenderProfileConfig};
    use crate::draw::{BLACK, BlurStyle, Frame, RED, Shape, WHITE};
    use crate::render_profiles::RenderColorProfile;

    fn snapshot(frame: Frame, viewport: CanvasExportViewport) -> CanvasExportSnapshot {
        CanvasExportSnapshot {
            viewport,
            backdrop: CanvasExportBackdropSnapshot::Transparent,
            board: BoardExportSnapshot { frame },
            render_profile: None,
            spotlight: Default::default(),
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
            spotlight: Default::default(),
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
    fn transparent_export_rejects_magnified_spotlight_without_source_pixels() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Spotlight {
            cx: 8,
            cy: 8,
            rx: 6,
            ry: 6,
            magnification: 2.0,
        });
        let export = snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 20,
                logical_height: 20,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        );

        let error = render_canvas_surface(&export).expect_err("missing source must fail");
        assert!(
            error.to_string().contains("Freeze screen to magnify"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn canvas_preflight_rejects_missing_spotlight_source_before_rendering() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Spotlight {
            cx: 8,
            cy: 8,
            rx: 6,
            ry: 6,
            magnification: 2.0,
        });
        let export = snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 20,
                logical_height: 20,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        );

        let error = export
            .validate_spotlight_source("Canvas 'Demo'")
            .expect_err("preflight must reject an incomplete source");
        let message = error.to_string();
        assert!(message.contains("Canvas 'Demo'"), "unexpected: {message}");
        // Freezing cannot rescue this export, so the message must not say to.
        assert!(
            !message.to_lowercase().contains("freeze screen"),
            "canvas PNG excludes frozen pixels, so advising Freeze is a dead end: {message}"
        );
        assert!(
            message.contains("solid background"),
            "unexpected: {message}"
        );
    }

    #[test]
    fn the_export_preflight_accepts_every_backdrop_that_can_feed_a_loupe() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Spotlight {
            cx: 8,
            cy: 8,
            rx: 6,
            ry: 6,
            magnification: 2.0,
        });
        let viewport = CanvasExportViewport {
            logical_width: 20,
            logical_height: 20,
            scale: 1,
            origin_x: 0,
            origin_y: 0,
        };

        // A solid board fills every pixel itself, and a persisted image is a
        // frozen raster: both are complete sources, so neither may be refused.
        for backdrop in [
            CanvasExportBackdropSnapshot::Solid(WHITE),
            CanvasExportBackdropSnapshot::PersistedImage {
                data: Arc::from(vec![0u8; 20 * 20 * 4].into_boxed_slice()),
                width: 20,
                height: 20,
                stride: 20 * 4,
                logical_to_image_scale_x: 1.0,
                logical_to_image_scale_y: 1.0,
            },
        ] {
            let mut export = snapshot(frame.clone(), viewport);
            export.backdrop = backdrop;
            assert!(
                export.validate_spotlight_source("Canvas 'Demo'").is_ok(),
                "a complete backdrop must not be refused"
            );
        }
    }

    #[test]
    fn pdf_preflight_names_the_failing_board_and_page() {
        let mut page = page_snapshot(Frame::new());
        page.frame.add_shape(Shape::Spotlight {
            cx: 8,
            cy: 8,
            rx: 6,
            ry: 6,
            magnification: 2.0,
        });
        let export = pdf_snapshot(page);

        let error = export
            .validate_spotlight_sources()
            .expect_err("preflight must reject an incomplete source");
        let message = error.to_string();
        assert!(
            message.contains("Board 'Board'"),
            "unexpected error: {message}"
        );
        assert!(message.contains("Page 1"), "unexpected error: {message}");
        // A PDF page does have a real way to gain a source; name it.
        assert!(
            message.contains("transparent_background = \"desktop\""),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn solid_export_magnifies_the_completed_canvas_before_dimming() {
        let mut frame = Frame::new();
        frame.add_shape(Shape::Rect {
            x: 5,
            y: 8,
            w: 2,
            h: 4,
            fill: true,
            color: RED,
            thick: 1.0,
        });
        frame.add_shape(Shape::Spotlight {
            cx: 10,
            cy: 10,
            rx: 10,
            ry: 10,
            magnification: 2.0,
        });
        let mut export = snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 20,
                logical_height: 20,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        );
        export.backdrop = CanvasExportBackdropSnapshot::Solid(BLACK);

        let mut surface = render_canvas_surface(&export).expect("magnified solid export");
        assert_ne!(pixel(&mut surface, 2, 10), pixel(&mut surface, 0, 0));
    }

    #[test]
    fn magnified_pdf_page_uses_the_raster_fallback() {
        let mut page = page_snapshot(Frame::new());
        page.backdrop = CanvasExportBackdropSnapshot::Solid(WHITE);
        page.frame.add_shape(Shape::Spotlight {
            cx: 10,
            cy: 10,
            rx: 8,
            ry: 8,
            magnification: 2.0,
        });

        let pdf = render_board_pdf(&pdf_snapshot(page)).expect("magnified PDF");
        assert!(pdf.starts_with(b"%PDF-"));
    }

    #[test]
    fn a_magnified_page_still_draws_its_label_over_the_raster() {
        fn pdf_with_labels(magnification: f64, labels_enabled: bool) -> Vec<u8> {
            let mut page = page_snapshot(Frame::new());
            page.backdrop = CanvasExportBackdropSnapshot::Solid(WHITE);
            page.frame.add_shape(Shape::Spotlight {
                cx: 10,
                cy: 10,
                rx: 8,
                ry: 8,
                magnification,
            });
            let mut snapshot = pdf_snapshot(page);
            snapshot.labels.enabled = labels_enabled;
            render_board_pdf(&snapshot).expect("pdf")
        }

        // The magnified page takes the raster fallback, which replaces the
        // whole page's vector content. The label is emitted after that content
        // and before `show_page`, so it must still reach the document rather
        // than being covered by — or dropped with — the raster.
        let magnified_plain = pdf_with_labels(2.0, false);
        let magnified_labelled = pdf_with_labels(2.0, true);
        assert!(magnified_labelled.starts_with(b"%PDF-"));
        assert_ne!(
            magnified_plain, magnified_labelled,
            "a raster page must still carry its label"
        );

        // And the vector path is unaffected by the same switch.
        assert_ne!(pdf_with_labels(1.0, false), pdf_with_labels(1.0, true));
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

    /// The spotlight pass has to run after the shapes. Eraser strokes clear their
    /// path and replay the original backdrop into it, so a dim layer painted
    /// before them would be punched away and every past erasure would show as a
    /// bright trail outside the openings.
    #[test]
    fn erased_regions_outside_a_spotlight_stay_dimmed() {
        let mut frame = Frame::new();
        // Opening on the left; the eraser sweeps across the dimmed right half.
        frame.add_shape(Shape::Spotlight {
            cx: 20,
            cy: 60,
            rx: 14,
            ry: 14,
            magnification: crate::draw::DEFAULT_SPOTLIGHT_MAGNIFICATION,
        });
        frame.add_shape(Shape::EraserStroke {
            points: vec![(40, 60), (110, 60)],
            brush: crate::draw::EraserBrush {
                size: 12.0,
                kind: crate::draw::EraserKind::Circle,
            },
        });

        let mut surface = render_canvas_surface(&snapshot(
            frame,
            CanvasExportViewport {
                logical_width: 120,
                logical_height: 120,
                scale: 1,
                origin_x: 0,
                origin_y: 0,
            },
        ))
        .expect("render export surface");

        let alpha_at = |surface: &mut cairo::ImageSurface, x: usize, y: usize| {
            surface.flush();
            let stride = surface.stride() as usize;
            surface.data().expect("surface data")[y * stride + x * 4 + 3]
        };

        let on_erased_path = alpha_at(&mut surface, 80, 60);
        let untouched_surround = alpha_at(&mut surface, 80, 15);
        assert!(
            on_erased_path > 0,
            "the erased path must still carry the dim layer, got alpha {on_erased_path}"
        );
        assert!(
            on_erased_path.abs_diff(untouched_surround) <= 8,
            "erased path ({on_erased_path}) should be dimmed like its surroundings \
             ({untouched_surround}), not punched bright"
        );
        assert_eq!(
            alpha_at(&mut surface, 20, 60),
            0,
            "the spotlight opening itself stays clear"
        );
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
            style: BlurStyle::Gaussian,
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
            style: BlurStyle::Gaussian,
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
