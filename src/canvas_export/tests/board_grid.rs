use super::*;
use crate::domain::{BoardGrid, BoardGridKind};
use crate::draw::{EraserBrush, EraserKind, Frame, RED, Shape, WHITE};

fn page(kind: BoardGridKind) -> CanvasExportSnapshot {
    CanvasExportSnapshot {
        viewport: CanvasExportViewport {
            logical_width: 160,
            logical_height: 120,
            scale: 1,
            origin_x: -71,
            origin_y: -53,
        },
        backdrop: CanvasExportBackdropSnapshot::board_paper(WHITE, BoardGrid::new(kind, 20)),
        board: BoardExportSnapshot {
            frame: Frame::new(),
        },
        render_profile: None,
        text_halo_enabled: true,
        spotlight: Default::default(),
    }
}

#[test]
fn board_grid_png_erasers_restore_pattern_and_snapshot_is_independent() {
    for kind in BoardGridKind::ALL.into_iter().skip(1) {
        let original = page(kind);
        let baseline = render_canvas_png(&original).unwrap();
        for brush_kind in [EraserKind::Circle, EraserKind::Rect] {
            let mut edited = original.clone();
            edited.board.frame.add_shape(Shape::Rect {
                x: -40,
                y: -20,
                w: 20,
                h: 20,
                fill: true,
                color: RED,
                thick: 1.0,
            });
            edited.board.frame.add_shape(Shape::EraserStroke {
                points: vec![(-55, -10), (-5, -10)],
                brush: EraserBrush {
                    kind: brush_kind,
                    size: 60.0,
                },
            });
            let png = render_canvas_png(&edited).unwrap();
            let mut before =
                cairo::ImageSurface::create_from_png(&mut std::io::Cursor::new(&baseline.bytes))
                    .unwrap();
            let mut after =
                cairo::ImageSurface::create_from_png(&mut std::io::Cursor::new(&png.bytes))
                    .unwrap();
            let before = before.data().unwrap();
            let after = after.data().unwrap();
            for y in 33..53 {
                for x in 31..51 {
                    let index = (y * 160 + x) * 4;
                    assert!(
                        before[index..index + 4]
                            .iter()
                            .zip(&after[index..index + 4])
                            .all(|(a, b)| a.abs_diff(*b) <= 2)
                    );
                }
            }
            assert_eq!(render_canvas_png(&original).unwrap().bytes, baseline.bytes);
        }
    }
}

#[test]
fn board_grid_pdf_stays_vector_without_erasers_and_leaves_margins_plain() {
    use super::page::{ExportBackdrop, draw_canvas_page_region, paint_pdf_page_background};
    use crate::draw::{RenderCaches, RenderCtx, TextMeasurer};
    for kind in BoardGridKind::ALL.into_iter().skip(1) {
        let snapshot = page(kind);
        let page = CanvasPageExportSnapshot {
            frame: Frame::new(),
            backdrop: snapshot.backdrop,
            viewport_width: 160,
            viewport_height: 120,
            origin_x: -71,
            origin_y: -53,
            text_halo_enabled: true,
            spotlight: Default::default(),
        };
        let source = CanvasExportRect::new(-71.0, -53.0, 160.0, 120.0).unwrap();
        let destination = CanvasExportRect::new(20.0, 20.0, 160.0, 120.0).unwrap();
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 160).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        paint_pdf_page_background(&ctx, &page, 200.0, 160.0);
        let backdrop = ExportBackdrop::new(&page.backdrop).unwrap();
        draw_canvas_page_region(
            &TextMeasurer::default(),
            &mut RenderCtx::new(&ctx, &mut RenderCaches::default()),
            &page,
            &backdrop,
            source,
            destination,
            false,
            None,
        )
        .unwrap();
        drop(ctx);
        let pixels = surface.data().unwrap();
        for y in 0..160 {
            for x in 0..200 {
                if !(20..180).contains(&x) || !(20..140).contains(&y) {
                    assert_eq!(&pixels[(y * 200 + x) * 4..(y * 200 + x) * 4 + 4], &[255; 4]);
                }
            }
        }
        assert!(pixels.iter().any(|v| *v < 250));
        drop(pixels);
        let document = BoardPdfExportSnapshot {
            pages: vec![PdfPageExportSnapshot {
                page,
                layout: PdfPageLayout {
                    page_width: 200.0,
                    page_height: 160.0,
                    source_rect: source,
                    destination_rect: destination,
                },
                metadata: PdfPageMetadata::new(0, 1, 0, 1, 0, 1, 0, 1, "Paper".into(), None),
            }],
            labels: Default::default(),
        };
        let bytes = render_board_pdf(&document).unwrap();
        assert!(
            !bytes
                .windows(b"/Subtype /Image".len())
                .any(|v| v == b"/Subtype /Image"),
            "{kind:?} paper should be a vector pattern"
        );
        assert!(bytes.windows(b"/Pattern".len()).any(|v| v == b"/Pattern"));
        check_pdf_pixels(&bytes, &surface);
    }
}

fn check_pdf_pixels(pdf: &[u8], expected: &cairo::ImageSurface) {
    use std::process::Command;
    if Command::new("pdftoppm").arg("-v").output().is_err() {
        return;
    }
    let folder = crate::test_temp::tempdir().unwrap();
    let path = folder.path().join("paper.pdf");
    let prefix = folder.path().join("paper");
    std::fs::write(&path, pdf).unwrap();
    let output = Command::new("pdftoppm")
        .args(["-png", "-r", "72", "-singlefile"])
        .arg(&path)
        .arg(&prefix)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut image = cairo::ImageSurface::create_from_png(
        &mut std::fs::File::open(prefix.with_extension("png")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        (image.width(), image.height()),
        (expected.width(), expected.height())
    );
    let actual = image.data().unwrap();
    expected
        .with_data(|expected| {
            let error: u64 = expected
                .iter()
                .zip(actual.iter())
                .map(|(a, b)| u64::from(a.abs_diff(*b)))
                .sum();
            assert!(
                error as f64 / (expected.len() as f64) < 4.0,
                "PDF paper phase differs from raster: mean error {}",
                error as f64 / expected.len() as f64
            );
            assert!(actual.chunks_exact(4).all(|pixel| pixel[3] == 255));
        })
        .unwrap();
}
