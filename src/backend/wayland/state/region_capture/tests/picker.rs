use super::*;
use crate::backend::ExitAfterCaptureMode;
use crate::backend::wayland::state::region_capture::delivery::{
    region_delivery_request, review_delivery_destination,
};
use crate::backend::wayland::state::region_capture::picker::{
    RegionPickerEntry, legacy_region_request, region_destination, region_picker_entry,
};
use crate::backend::wayland::state::region_capture::render::{
    RegionPixelSource, RegionRenderRequest, region_render_job,
};
use crate::canvas_export::{
    CanvasExportRect, CanvasRegionExportSnapshot, CanvasRegionSource, SpotlightPassSnapshot,
};
use crate::capture::{
    CaptureDestination, CaptureType, ImageFormatMetadata, ImageOperationKind, file::FileSaveConfig,
};
use crate::config::{Action, RegionPicker};
use crate::input::state::RegionPurposeTag;
use crate::screen_pixels::{ImagePixelRect, PackedArgb32};
use crate::ui::RegionAction;

#[test]
fn every_bound_region_action_has_the_declared_destination() {
    let default = CaptureDestination::ClipboardAndFile;
    assert_eq!(
        region_destination(Action::CaptureSelection, default),
        Some(default)
    );
    for action in [
        Action::CaptureClipboardSelection,
        Action::CaptureClipboardRegion,
    ] {
        assert_eq!(
            region_destination(action, default),
            Some(CaptureDestination::ClipboardOnly)
        );
    }
    for action in [Action::CaptureFileSelection, Action::CaptureFileRegion] {
        assert_eq!(
            region_destination(action, default),
            Some(CaptureDestination::FileOnly)
        );
    }
    assert_eq!(
        region_destination(Action::CaptureRegionInteractive, default),
        Some(default)
    );
    assert_eq!(region_destination(Action::CaptureFullScreen, default), None);
}

#[test]
fn review_destination_labels_match_the_delivery_they_request() {
    assert_eq!(
        review_delivery_destination(RegionAction::Copy),
        Some(CaptureDestination::ClipboardOnly)
    );
    assert_eq!(
        review_delivery_destination(RegionAction::Save),
        Some(CaptureDestination::FileOnly)
    );
    assert_eq!(
        review_delivery_destination(RegionAction::Both),
        Some(CaptureDestination::ClipboardAndFile),
        "Both must always match its label"
    );
    assert_eq!(review_delivery_destination(RegionAction::Board), None);
    assert_eq!(
        review_delivery_destination(RegionAction::ToggleIncludeDrawings),
        None
    );
    assert_eq!(review_delivery_destination(RegionAction::CutBand), None);
    assert_eq!(review_delivery_destination(RegionAction::UndoCut), None);
    assert_eq!(review_delivery_destination(RegionAction::RedoCut), None);
    assert_eq!(review_delivery_destination(RegionAction::ResetCuts), None);
}

#[test]
fn the_render_job_composes_drawings_when_asked_and_stays_raw_otherwise() {
    // Every destination — Copy, Save, Both and Board alike — takes its PNG
    // from this one job, so what the toggle produces cannot differ between
    // them. Board therefore honours the toggle by construction.
    //
    // A deliberately non-uniform 3x2 crop: a first-pixel check would pass on a
    // flat image even if the encoder transposed or truncated it.
    let swatch: [u32; 6] = [
        0xFF11_2233,
        0xFF44_5566,
        0xFF77_8899,
        0xFFAA_BBCC,
        0xFFDD_EEFF,
        0xFF01_0203,
    ];
    let bytes: Vec<u8> = swatch
        .iter()
        .flat_map(|pixel| pixel.to_ne_bytes())
        .collect();

    let rendered = region_render_job(RegionRenderRequest {
        source: RegionPixelSource::Raw {
            image: std::sync::Arc::new(crate::screen_pixels::ScreenImage {
                data: bytes.clone(),
                width: 3,
                height: 2,
                stride: 12,
            }),
            selection: ImagePixelRect::new(0, 0, 3, 2, (3, 2)).expect("selection"),
        },
        cuts: Vec::new(),
    })()
    .expect("raw render");
    let direct = crate::capture::png::encode_packed_argb32_png(
        &PackedArgb32::new(3, 2, 12, bytes).expect("a 3x2 crop"),
    )
    .expect("direct encode");
    assert_eq!(
        rendered.bytes, direct.bytes,
        "the raw path must hand the crop to the encoder untouched"
    );
    assert_eq!(
        (rendered.width, rendered.height),
        (direct.width, direct.height)
    );
    assert_eq!(rendered.format, direct.format);

    let decoded = decode_png_pixels(&rendered.bytes);
    assert_eq!(decoded, swatch.to_vec(), "every pixel survives in order");

    // The composed path renders the same selection with the board's committed
    // shapes over it, so the crop's pixels are no longer what comes back.
    let mut frame = crate::draw::Frame::new();
    frame.add_shape(crate::draw::Shape::Rect {
        x: 10,
        y: 20,
        w: 3,
        h: 2,
        fill: true,
        color: crate::draw::RED,
        thick: 1.0,
    });
    let snapshot = CanvasRegionExportSnapshot {
        source: CanvasRegionSource {
            image: std::sync::Arc::new(crate::screen_pixels::ScreenImage {
                data: swatch
                    .iter()
                    .flat_map(|pixel| pixel.to_ne_bytes())
                    .collect(),
                width: 3,
                height: 2,
                stride: 12,
            }),
            logical_bounds: CanvasExportRect::new(10.0, 20.0, 3.0, 2.0).expect("bounds"),
        },
        selection: ImagePixelRect::new(0, 0, 3, 2, (3, 2)).expect("selection"),
        frame,
        text_halo_enabled: true,
        spotlight: SpotlightPassSnapshot {
            dim_opacity: 0.0,
            feather: 0.0,
        },
    };
    let composed = region_render_job(RegionRenderRequest {
        source: RegionPixelSource::Annotated(Box::new(snapshot)),
        cuts: Vec::new(),
    })()
    .expect("composed");
    assert_eq!(
        (composed.width, composed.height),
        (3, 2),
        "composition keeps the selection's pixel dimensions"
    );
    assert_ne!(
        decode_png_pixels(&composed.bytes),
        swatch.to_vec(),
        "the board's committed shapes are composited in"
    );
}

#[test]
fn the_render_job_cuts_after_flattening_and_keeps_composed_bytes() {
    let swatch: [u32; 6] = [
        0xFF11_2233,
        0xFF44_5566,
        0xFF77_8899,
        0xFFAA_BBCC,
        0xFFDD_EEFF,
        0xFF01_0203,
    ];
    let bytes: Vec<u8> = swatch
        .iter()
        .flat_map(|pixel| pixel.to_ne_bytes())
        .collect();
    let cut = crate::capture::CutBand::new(crate::capture::CutAxis::Columns, 1, 2).unwrap();
    let request = RegionRenderRequest {
        source: RegionPixelSource::Raw {
            image: std::sync::Arc::new(crate::screen_pixels::ScreenImage {
                data: bytes,
                width: 3,
                height: 2,
                stride: 12,
            }),
            selection: ImagePixelRect::new(0, 0, 3, 2, (3, 2)).expect("selection"),
        },
        cuts: vec![cut],
    };
    let rendered = region_render_job(request)().expect("cut render");
    assert_eq!((rendered.width, rendered.height), (2, 2));
    assert_eq!(
        decode_png_pixels(&rendered.bytes),
        vec![swatch[0], swatch[2], swatch[3], swatch[5]]
    );
}

fn decode_png_pixels(bytes: &[u8]) -> Vec<u32> {
    let mut surface =
        cairo::ImageSurface::create_from_png(&mut { bytes }).expect("the job produced a PNG");
    surface.flush();
    let width = surface.width() as usize;
    let height = surface.height() as usize;
    let stride = surface.stride() as usize;
    let data = surface.data().expect("decoded pixels");
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
fn native_entry_auto_freezes_solid_boards_but_refuses_solid_zoom() {
    assert_eq!(
        region_picker_entry(false, false, false, false, true),
        RegionPickerEntry::AutoFreeze
    );
    assert_eq!(
        region_picker_entry(false, false, true, true, true),
        RegionPickerEntry::RefuseSolidZoom
    );
}

#[test]
fn native_entry_uses_existing_waiting_legacy_and_missing_zoom_paths() {
    assert_eq!(
        region_picker_entry(true, false, true, true, true),
        RegionPickerEntry::Activate
    );
    assert_eq!(
        region_picker_entry(false, true, true, false, true),
        RegionPickerEntry::WaitForZoom
    );
    assert_eq!(
        region_picker_entry(false, true, false, false, false),
        RegionPickerEntry::Legacy
    );
    assert_eq!(
        region_picker_entry(false, true, true, true, true),
        RegionPickerEntry::ZoomImageUnavailable
    );
}

#[test]
fn picker_options_snapshot_is_independent_of_later_config_changes() {
    let mut live = crate::config::CaptureConfig::default();
    let options = RegionPickerOptions::new(
        live.region.show_size_readout,
        live.region.show_loupe,
        live.region.show_legend,
    );
    let intent = RegionCaptureIntent::new(
        Action::CaptureSelection,
        RegionPurposeTag::CaptureDeliver,
        CaptureDestination::ClipboardOnly,
        None,
        ExitAfterCaptureMode::Auto,
        options,
        live.include_drawings,
    );
    live.include_drawings = false;
    live.region.show_size_readout = false;
    live.region.show_loupe = true;
    live.region.show_legend = false;
    live.region.picker = RegionPicker::Slurp;

    assert_eq!(live.region.picker, RegionPicker::Slurp);
    assert!(!live.region.show_size_readout);
    assert!(live.region.show_loupe);
    assert!(!live.region.show_legend);
    assert!(!live.include_drawings);
    assert!(intent.include_drawings());
    assert!(intent.options().show_size_readout());
    assert!(!intent.options().show_loupe());
    assert!(intent.options().show_legend());
}

#[test]
fn native_delivery_forces_png_naming_metadata_and_bytes() {
    let intent = RegionCaptureIntent::new(
        Action::CaptureFileRegion,
        RegionPurposeTag::CaptureDeliver,
        CaptureDestination::FileOnly,
        Some(FileSaveConfig {
            save_directory: std::path::PathBuf::from("/tmp/captures"),
            filename_template: "region-{timestamp}".to_string(),
            format: "jpg".to_string(),
        }),
        ExitAfterCaptureMode::Never,
        RegionPickerOptions::new(true, false, true),
        true,
    );
    let pixels = PackedArgb32::new(1, 1, 4, 0xff33_2211_u32.to_ne_bytes().to_vec())
        .expect("one native ARGB32 pixel");

    let render: crate::capture::ImageRenderJob =
        Box::new(move || crate::capture::png::encode_packed_argb32_png(&pixels));
    let request = region_delivery_request(render, &intent, intent.destination());

    assert_eq!(request.destination, CaptureDestination::FileOnly);
    assert_eq!(request.operation, ImageOperationKind::Screenshot);
    assert_eq!(
        request
            .save_config
            .as_ref()
            .map(|save| save.format.as_str()),
        Some("png")
    );
    assert_eq!(
        request.fallback_format_override,
        Some(ImageFormatMetadata::png())
    );
    let rendered = (request.render)().expect("native region crop encodes");
    assert_eq!(rendered.format, ImageFormatMetadata::png());
    assert_eq!((rendered.width, rendered.height), (1, 1));
    assert!(rendered.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn legacy_handoff_preserves_the_reserved_selection_request_snapshot() {
    let intent = RegionCaptureIntent::new(
        Action::CaptureFileSelection,
        RegionPurposeTag::CaptureDeliver,
        CaptureDestination::FileOnly,
        Some(FileSaveConfig {
            save_directory: std::path::PathBuf::from("/tmp/original"),
            filename_template: "shot-{timestamp}".to_string(),
            format: "jpg".to_string(),
        }),
        ExitAfterCaptureMode::Always,
        RegionPickerOptions::new(false, true, false),
        true,
    );

    let request = legacy_region_request(&intent);

    assert!(matches!(
        request.capture_type,
        CaptureType::Selection {
            x: 0,
            y: 0,
            width: 0,
            height: 0
        }
    ));
    assert_eq!(request.destination, CaptureDestination::FileOnly);
    let save = request
        .save_config
        .expect("file selection keeps save config");
    assert_eq!(
        save.save_directory,
        std::path::PathBuf::from("/tmp/original")
    );
    assert_eq!(save.filename_template, "shot-{timestamp}");
    assert_eq!(
        save.format, "jpg",
        "explicit slurp keeps the legacy configured format"
    );
}
