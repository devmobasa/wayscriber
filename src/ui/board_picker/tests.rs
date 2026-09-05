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
            &mut crate::draw::RenderCtx::new(&ctx, caches),
            state,
            size.0 as u32,
            size.1 as u32,
            true,
        );
    }
    surface.data().unwrap().to_vec()
}

#[test]
fn retained_board_text_owner_matches_fresh_during_unicode_rename_and_small_layouts() {
    let engine = UiTextEngine::default();
    let measurer = crate::draw::TextMeasurer::default();
    let mut caches = crate::draw::RenderCaches::default();
    let mut state = crate::input::state::test_support::make_test_input_state();
    state.open_board_picker();
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
