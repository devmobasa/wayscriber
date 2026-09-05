use super::*;
use crate::draw::TextMeasurer;
use crate::input::state::InputTextResources;
use crate::ui::onboarding_card::{
    OnboardingCard, OnboardingChecklistItem, render_onboarding_card_with_engine,
};
use crate::ui::tour::render_tour_with_engine;

fn pixels(density: i32, paint: impl FnOnce(&cairo::Context)) -> Vec<u8> {
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, 420 * density, 360 * density).unwrap();
    {
        let ctx = cairo::Context::new(&surface).unwrap();
        ctx.scale(f64::from(density), f64::from(density));
        paint(&ctx);
    }
    surface.data().unwrap().to_vec()
}

fn assert_owner_parity(
    engine: &UiTextEngine,
    density: i32,
    paint: impl Fn(&UiTextEngine, &cairo::Context),
) -> Vec<u8> {
    let actual = pixels(density, |ctx| paint(engine, ctx));
    let fresh = pixels(density, |ctx| paint(&UiTextEngine::default(), ctx));
    assert!(actual.iter().any(|&byte| byte != 0));
    assert!(actual == fresh, "retained overlay text pixels differ");
    actual
}

#[test]
fn retained_overlay_owner_matches_fresh_across_density_and_visible_state_changes() {
    let engine = UiTextEngine::default();
    let measurer = TextMeasurer::default();
    let mut state = crate::input::state::test_support::make_test_input_state();
    let mut card = OnboardingCard {
        eyebrow: "你好 Καλημέρα onboarding".into(),
        title: "A long title that needs fitting on a small output".into(),
        body: "Wrapped body text with café and שלום repeated across the narrow card. More words to occupy another line.".into(),
        items: vec![OnboardingChecklistItem { label: "A long Unicode checklist label 你好 café".into(), done: false }],
        footer: "Long footer explaining the next action without changing layout policy".into(),
    };
    for density in [1, 2, 1] {
        state.open_precision_entry(crate::ui::toolbar::PrecisionEntryTarget::Thickness);
        assert_owner_parity(&engine, density, |engine, ctx| {
            render_precision_entry_popup_with_engine(engine, ctx, &state, 420, 360, (400.0, 350.0))
        });
        let before = assert_owner_parity(&engine, density, |engine, ctx| {
            render_onboarding_card_with_engine(engine, ctx, 420, 360, &card)
        });
        card.items[0].done = !card.items[0].done;
        let after = assert_owner_parity(&engine, density, |engine, ctx| {
            render_onboarding_card_with_engine(engine, ctx, 420, 360, &card)
        });
        assert!(before != after, "checklist completion must remain visible");
        state.start_tour_with_resources(InputTextResources {
            measurer: &measurer,
            ui_engine: &engine,
        });
        let first = assert_owner_parity(&engine, density, |engine, ctx| {
            render_tour_with_engine(engine, ctx, &state, 420, 360)
        });
        state.tour_next();
        let next = assert_owner_parity(&engine, density, |engine, ctx| {
            render_tour_with_engine(engine, ctx, &state, 420, 360)
        });
        assert!(
            first != next,
            "tour navigation must update the painted step"
        );
        state.end_tour();
    }
}
