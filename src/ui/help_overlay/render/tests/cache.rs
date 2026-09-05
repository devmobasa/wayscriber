use super::*;
use crate::config::HelpOverlayStyle;
use crate::ui::HelpOverlayBindings;

struct Inputs {
    style: HelpOverlayStyle,
    width: u32,
    height: u32,
    frozen: bool,
    page: usize,
    bindings: HelpOverlayBindings,
    query: String,
    context_filter: bool,
    board: bool,
    capture: bool,
    quick: bool,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            style: HelpOverlayStyle::default(),
            width: 800,
            height: 400,
            frozen: true,
            page: 0,
            bindings: HelpOverlayBindings::default(),
            query: String::new(),
            context_filter: false,
            board: true,
            capture: true,
            quick: false,
        }
    }
}

fn layout(cache: &mut HelpLayoutCache, inputs: &Inputs, scroll: f64) -> OverlayLayout {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    cache.get_or_build_overlay_layout(
        &crate::ui_text::UiTextEngine::default(),
        &ctx,
        &inputs.style,
        inputs.width,
        inputs.height,
        inputs.page,
        &HelpContentSnapshot::from_bindings(
            &inputs.bindings,
            inputs.frozen,
            inputs.context_filter,
            inputs.board,
            inputs.capture,
        ),
        &inputs.query,
        scroll,
        "Wayscriber Controls",
        &super::super::header::HeaderContent {
            version: "test",
            intro: None,
            hints: &[],
        },
        "Note",
        "Esc to close",
        inputs.quick,
    )
}

#[test]
fn scrolling_clamps_a_returned_clone_without_rebuilding_or_mutating_the_entry() {
    let inputs = Inputs::default();
    let mut cache = HelpLayoutCache::default();
    let initial = layout(&mut cache, &inputs, 0.0);
    assert!(initial.scroll_max > 0.0, "fixture needs scrollable content");
    let middle = layout(&mut cache, &inputs, initial.scroll_max * 0.5);
    assert_eq!(middle.scroll_offset, initial.scroll_max * 0.5);
    assert_eq!(layout(&mut cache, &inputs, -10.0).scroll_offset, 0.0);
    assert_eq!(
        layout(&mut cache, &inputs, f64::MAX).scroll_offset,
        initial.scroll_max
    );
    assert_eq!(cache.builds, 1);
    assert_eq!(cache.entry.as_ref().unwrap().layout.scroll_offset, 0.0);
}

fn assert_rebuilds_from_baseline(changed: Inputs, label: &str) {
    let baseline = Inputs::default();
    let mut cache = HelpLayoutCache::default();
    layout(&mut cache, &baseline, 0.0);
    let changed_layout = layout(&mut cache, &changed, 0.0);
    assert_eq!(cache.builds, 2, "{label} must invalidate independently");
    layout(&mut cache, &changed, 1.0);
    assert_eq!(cache.builds, 2, "{label} replacement must be reusable");
    let fresh = layout(&mut HelpLayoutCache::default(), &changed, 0.0);
    assert_eq!(changed_layout.box_width, fresh.box_width, "{label}");
    assert_eq!(changed_layout.box_height, fresh.box_height, "{label}");
    assert_eq!(changed_layout.scroll_max, fresh.scroll_max, "{label}");
    assert_eq!(changed_layout.search_lower, fresh.search_lower, "{label}");
    assert_eq!(changed_layout.note_text, fresh.note_text, "{label}");
    layout(&mut cache, &baseline, 0.0);
    assert_eq!(cache.builds, 3, "{label}: only one entry is retained");
}

#[test]
fn every_layout_key_dimension_invalidates_independently() {
    type Change = (&'static str, fn(&mut Inputs));
    let changes: &[Change] = &[
        ("width", |v| v.width += 1),
        ("height", |v| v.height += 1),
        ("frozen", |v| v.frozen = false),
        ("page", |v| v.page = 1),
        ("bindings", |v| {
            let input = crate::input::state::test_support::make_test_input_state();
            v.bindings = HelpOverlayBindings::from_input_state(&input);
        }),
        ("query", |v| v.query = "draw".into()),
        ("raw whitespace query", |v| v.query = " ".into()),
        ("context filter", |v| v.context_filter = true),
        ("board", |v| v.board = false),
        ("capture", |v| v.capture = false),
        ("quick", |v| v.quick = true),
        ("font size", |v| v.style.font_size += 1.0),
        ("font family", |v| v.style.font_family = "Monospace".into()),
        ("line height", |v| v.style.line_height += 1.0),
        ("padding", |v| v.style.padding += 1.0),
        ("border width", |v| v.style.border_width += 1.0),
    ];
    for (label, change) in changes {
        let mut changed = Inputs::default();
        change(&mut changed);
        assert_rebuilds_from_baseline(changed, label);
    }
    for channel in 0..4 {
        for color in 0..3 {
            let mut changed = Inputs::default();
            let channels = match color {
                0 => &mut changed.style.bg_color,
                1 => &mut changed.style.border_color,
                _ => &mut changed.style.text_color,
            };
            channels[channel] += 0.1;
            assert_rebuilds_from_baseline(changed, &format!("color {color}, channel {channel}"));
        }
    }
}

#[test]
fn style_quantization_retains_existing_hundredths_policy() {
    let mut inputs = Inputs::default();
    let mut cache = HelpLayoutCache::default();
    layout(&mut cache, &inputs, 0.0);
    inputs.style.font_size += 0.001;
    inputs.style.border_color[0] += 0.001;
    layout(&mut cache, &inputs, 0.0);
    assert_eq!(cache.builds, 1);
    inputs.style.font_size += 0.01;
    layout(&mut cache, &inputs, 0.0);
    assert_eq!(cache.builds, 2);
}

fn paint(
    engine: &crate::ui_text::UiTextEngine,
    caches: &mut crate::ui::UiRenderCaches,
    inputs: &Inputs,
    scroll: f64,
) -> (Vec<u8>, crate::help_overlay_interaction::HelpRenderResult) {
    let mut surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        inputs.width as i32,
        inputs.height as i32,
    )
    .unwrap();
    let extent;
    {
        let cairo = cairo::Context::new(&surface).unwrap();
        let theme = crate::ui::theme::Theme::dark();
        let mut render = crate::ui::UiRenderCtx {
            cairo: &cairo,
            theme: &theme,
            caches,
        };
        extent = super::super::render_help_overlay_result_with_context(
            engine,
            &mut render,
            &inputs.style,
            inputs.width,
            inputs.height,
            inputs.frozen,
            inputs.page,
            &inputs.bindings,
            &inputs.query,
            inputs.context_filter,
            inputs.board,
            inputs.capture,
            scroll,
            inputs.quick,
        );
    }
    surface.flush();
    (surface.data().unwrap().to_vec(), extent)
}

fn assert_frame_matches(
    actual: &(Vec<u8>, crate::help_overlay_interaction::HelpRenderResult),
    expected: &(Vec<u8>, crate::help_overlay_interaction::HelpRenderResult),
) {
    assert_eq!(
        actual.1, expected.1,
        "scroll and every hit rectangle must match"
    );
    assert!(
        actual.0 == expected.0,
        "help pixels differ at byte {:?}",
        actual.0.iter().zip(&expected.0).position(|(a, b)| a != b)
    );
}

#[test]
fn owners_keep_independent_layouts_and_reused_rendering_matches_fresh_pixels() {
    let engine = crate::ui_text::UiTextEngine::default();
    let second_engine = crate::ui_text::UiTextEngine::default();
    let mut caches = crate::ui::UiRenderCaches::default();
    let mut other_caches = crate::ui::UiRenderCaches::default();
    let inputs = Inputs::default();
    let initial = paint(&engine, &mut caches, &inputs, 0.0);
    assert!(initial.0.iter().any(|&byte| byte != 0));
    for scroll in [0.0, 30.0, 0.0] {
        let actual = paint(&engine, &mut caches, &inputs, scroll);
        let expected = paint(&second_engine, &mut other_caches, &inputs, scroll);
        assert_frame_matches(&actual, &expected);
    }
    assert_eq!(
        caches.help_mut().builds,
        1,
        "scroll keeps the cached layout"
    );
    assert_eq!(other_caches.help_mut().builds, 1);
    for input in [
        Inputs {
            width: 320,
            height: 180,
            ..Inputs::default()
        },
        Inputs {
            quick: true,
            ..Inputs::default()
        },
        Inputs {
            query: "draw".into(),
            ..Inputs::default()
        },
        Inputs {
            query: "你好".into(),
            ..Inputs::default()
        },
        Inputs {
            page: 1,
            ..Inputs::default()
        },
    ] {
        for scroll in [0.0, 40.0] {
            let actual = paint(&engine, &mut caches, &input, scroll);
            let expected = paint(
                &crate::ui_text::UiTextEngine::default(),
                &mut crate::ui::UiRenderCaches::default(),
                &input,
                scroll,
            );
            assert_frame_matches(&actual, &expected);
            assert_frame_matches(&paint(&engine, &mut caches, &input, scroll), &expected);
        }
    }
    assert_frame_matches(&paint(&engine, &mut caches, &inputs, 0.0), &initial);
    assert_frame_matches(
        &paint(&second_engine, &mut other_caches, &inputs, 0.0),
        &initial,
    );
    assert_eq!(
        other_caches.help_mut().builds,
        1,
        "another owner's changes cannot evict this layout"
    );
}
