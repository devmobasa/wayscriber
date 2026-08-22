use super::*;
use crate::backend::ExitAfterCaptureMode;
use crate::backend::wayland::state::region_capture::delivery::{
    RegionSubmit, include_drawings_for_submit, region_delivery_request, review_delivery_destination,
};
use crate::backend::wayland::state::region_capture::picker::{
    RegionPickerEntry, legacy_region_request, region_destination, region_picker_entry,
};
use crate::capture::{
    CaptureDestination, CaptureType, ImageFormatMetadata, ImageOperationKind, file::FileSaveConfig,
};
use crate::config::{Action, RegionPicker};
use crate::input::state::{BoardPasteTarget, RegionPurposeTag};
use crate::screen_pixels::PackedArgb32;
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
}

#[test]
fn include_drawings_applies_to_exports_but_board_keeps_the_raw_crop() {
    assert!(include_drawings_for_submit(
        true,
        &RegionSubmit::Deliver(CaptureDestination::ClipboardAndFile)
    ));
    assert!(!include_drawings_for_submit(
        false,
        &RegionSubmit::Deliver(CaptureDestination::ClipboardAndFile)
    ));
    assert!(!include_drawings_for_submit(
        true,
        &RegionSubmit::Board(BoardPasteTarget {
            board_id: "board".to_string(),
            page_index: 0,
            page_generation: 1,
            world_bounds: crate::util::Rect::new(0, 0, 1, 1).unwrap(),
        })
    ));
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
