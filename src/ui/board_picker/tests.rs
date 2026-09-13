use super::*;

fn pixels(
    engine: &UiTextEngine,
    measurer: &crate::draw::TextMeasurer,
    caches: &mut crate::draw::RenderCaches,
    state: &InputState,
    size: (i32, i32),
    density: i32,
) -> Vec<u8> {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, size.0 * density, size.1 * density)
            .unwrap();
    {
        let ctx = cairo::Context::new(&surface).unwrap();
        ctx.scale(f64::from(density), f64::from(density));
        render_board_picker_with_halo(
            engine,
            measurer,
            &mut ThumbnailCache::default(),
            &mut crate::draw::RenderCtx::new(&ctx, caches),
            state,
            size.0 as u32,
            size.1 as u32,
            true,
        );
    }
    surface.data().unwrap().to_vec()
}

// A parallel-suite core dump reached Cairo/FreeType glyph loading through this
// test. tools/lint-and-test.sh runs it alone under both feature configurations;
// process isolation preserves assertions but does not fix cairo/cairo!81.
#[test]
#[ignore = "isolated by tools/lint-and-test.sh; Cairo race: https://gitlab.freedesktop.org/cairo/cairo/-/merge_requests/81"]
fn retained_board_text_owner_matches_fresh_during_unicode_rename_and_small_layouts() {
    check_appearance_sheet_on_small_surfaces();
    let engine = UiTextEngine::default();
    let measurer = crate::draw::TextMeasurer::default();
    let mut caches = crate::draw::RenderCaches::default();
    let mut state = crate::input::state::test_support::make_test_input_state();
    state.open_board_picker_with_measurer(&crate::draw::TextMeasurer::default());
    for (width, height, density) in [(900, 700, 1), (420, 300, 2), (900, 700, 1)] {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        state.update_board_picker_layout(&ctx, width as u32, height as u32);
        let before = pixels(
            &engine,
            &measurer,
            &mut caches,
            &state,
            (width, height),
            density,
        );
        let board_index = state
            .board_picker_layout()
            .unwrap()
            .page_board_index
            .unwrap();
        state.board_picker_start_page_rename(board_index, 0);
        for ch in "你好 Καλημέρα long page name".chars() {
            state.board_picker_page_edit_append(ch);
        }
        let actual = pixels(
            &engine,
            &measurer,
            &mut caches,
            &state,
            (width, height),
            density,
        );
        let expected = pixels(
            &UiTextEngine::default(),
            &crate::draw::TextMeasurer::default(),
            &mut crate::draw::RenderCaches::default(),
            &state,
            (width, height),
            density,
        );
        assert!(actual.iter().any(|&byte| byte != 0));
        assert!(actual == expected, "retained board UI pixels differ");
        assert!(
            actual != before,
            "rename overlay must paint the edited label"
        );
        state.board_picker_cancel_page_edit();
        assert!(
            pixels(
                &engine,
                &measurer,
                &mut caches,
                &state,
                (width, height),
                density
            ) == before
        );
    }
}

fn check_appearance_sheet_on_small_surfaces() {
    let engine = UiTextEngine::default();
    let measurer = crate::draw::TextMeasurer::default();
    let mut state = crate::input::state::test_support::make_test_input_state();
    state.switch_board_force("whiteboard");
    state.open_board_picker_with_measurer(&measurer);
    state.board_picker_edit_color_selected_with_measurer(&measurer);
    state.board_appearance_key(crate::input::events::Key::Tab);
    state.board_appearance_key(crate::input::events::Key::Right);
    state.board_appearance_key(crate::input::events::Key::Right);
    for (width, height) in [(900, 700), (420, 300)] {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
        let ctx = cairo::Context::new(&surface).unwrap();
        state.update_board_picker_layout(&ctx, width as u32, height as u32);
        let (x, y, w) = state.board_appearance_rect().unwrap();
        assert!(x >= 12.0 && x + w + 12.0 <= f64::from(width));
        assert!(y >= 70.0 && y + 222.0 <= f64::from(height));
        let data = pixels(
            &engine,
            &measurer,
            &mut crate::draw::RenderCaches::default(),
            &state,
            (width, height),
            1,
        );
        if let Ok(folder) = std::env::var("WAYSCRIBER_GRID_UI_ARTIFACTS") {
            let surface = cairo::ImageSurface::create_for_data(
                data,
                cairo::Format::ARgb32,
                width,
                height,
                width * 4,
            )
            .unwrap();
            let mut file = std::fs::File::create(
                std::path::Path::new(&folder).join(format!("paper-editor-{width}x{height}.png")),
            )
            .unwrap();
            surface.write_to_png(&mut file).unwrap();
        }
    }
}
