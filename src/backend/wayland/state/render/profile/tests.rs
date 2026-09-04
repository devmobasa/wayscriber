use super::*;
use crate::config::{RenderColorMappingConfig, RenderProfileConfig};

const RED: u32 = 0xffff0000;
const GREEN: u32 = 0xff00ff00;
const BLUE: u32 = 0xff0000ff;

fn selected_profile() -> RenderColorProfile {
    RenderColorProfile::from_config(&RenderProfileConfig {
        id: "test".into(),
        name: "Test".into(),
        mappings: vec![
            RenderColorMappingConfig {
                from: "#ff0000".into(),
                to: "#00ff00".into(),
            },
            RenderColorMappingConfig {
                from: "#00ff00".into(),
                to: "#0000ff".into(),
            },
        ],
    })
    .expect("valid test profile")
}

fn canvas() -> cairo::ImageSurface {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 4, 1).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    ctx.set_source_rgb(1.0, 0.0, 0.0);
    ctx.paint().unwrap();
    ctx.set_source_rgb(0.0, 0.0, 1.0);
    ctx.rectangle(1.0, 0.0, 1.0, 1.0);
    ctx.fill().unwrap();
    surface
}

fn pixels<'a>(data: &'a mut [u8], damage: &'a [Rect]) -> PixelBuffer<'a> {
    PixelBuffer {
        data,
        width: 4,
        height: 1,
        stride: 16,
        damage,
    }
}

fn read(surface: &mut cairo::ImageSurface) -> Vec<u32> {
    surface
        .data()
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| u32::from_ne_bytes(*bytes))
        .collect()
}

#[test]
fn profile_passes_preserve_canvas_and_ui_separation_with_partial_damage() {
    let damage = [Rect::new(0, 0, 3, 1).unwrap()];
    for (canvas_enabled, ui_enabled, before, after) in [
        (false, false, [RED, BLUE, RED, RED], [RED, RED, RED, RED]),
        (
            true,
            false,
            [GREEN, BLUE, GREEN, RED],
            [GREEN, RED, GREEN, RED],
        ),
        (false, true, [RED, BLUE, RED, RED], [RED, GREEN, RED, RED]),
        (
            true,
            true,
            [RED, BLUE, RED, RED],
            [GREEN, GREEN, GREEN, RED],
        ),
    ] {
        let profile = FrameProfile::new(Some(selected_profile()), canvas_enabled, ui_enabled);
        let mut surface = canvas();
        let mut baseline = Vec::new();
        let rewritten = profile.needs_before_ui(true)
            && profile.before_ui(
                pixels(&mut surface.data().unwrap(), &damage),
                &mut baseline,
                true,
            );
        assert_eq!(rewritten, canvas_enabled && !ui_enabled);
        assert_eq!(read(&mut surface), before);
        // UI paints an opaque red pixel over the blue canvas pixel.
        {
            let ctx = cairo::Context::new(&surface).unwrap();
            ctx.set_source_rgb(1.0, 0.0, 0.0);
            ctx.rectangle(1.0, 0.0, 1.0, 1.0);
            ctx.fill().unwrap();
        }
        profile.after_ui(
            pixels(&mut surface.data().unwrap(), &damage),
            &baseline,
            true,
        );
        assert_eq!(read(&mut surface), after);
    }
}

#[test]
fn suppressed_ui_does_not_copy_or_remap_a_stale_baseline() {
    let profile = FrameProfile::new(Some(selected_profile()), false, true);
    let damage = [Rect::new(0, 0, 4, 1).unwrap()];
    let mut surface = canvas();
    let mut baseline = vec![0; 16];
    assert!(!profile.needs_before_ui(false));
    assert!(!profile.before_ui(
        pixels(&mut surface.data().unwrap(), &damage),
        &mut baseline,
        false,
    ));
    assert_eq!(baseline, vec![0; 16]);
    profile.after_ui(
        pixels(&mut surface.data().unwrap(), &damage),
        &baseline,
        false,
    );
    assert_eq!(read(&mut surface), [RED, BLUE, RED, RED]);
}

#[test]
fn absent_profile_does_not_remap_even_when_both_targets_are_enabled() {
    let profile = FrameProfile::new(None, true, true);
    let damage = [Rect::new(0, 0, 4, 1).unwrap()];
    let mut surface = canvas();
    let mut baseline = vec![7];
    assert_eq!(profile.mode(), ProfileMode::Off);
    assert!(!profile.needs_before_ui(true));
    assert!(!profile.before_ui(
        pixels(&mut surface.data().unwrap(), &damage),
        &mut baseline,
        true,
    ));
    profile.after_ui(
        pixels(&mut surface.data().unwrap(), &damage),
        &baseline,
        true,
    );
    assert_eq!(baseline, [7]);
    assert_eq!(read(&mut surface), [RED, BLUE, RED, RED]);
}
