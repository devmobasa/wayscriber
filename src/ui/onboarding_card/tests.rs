use super::*;

fn button(
    label: &str,
    key: &str,
    action: OnboardingCardAction,
    primary: bool,
) -> OnboardingCardButton {
    OnboardingCardButton {
        label: label.to_string(),
        key_hint: Some(key.to_string()),
        action,
        primary,
    }
}

fn card() -> OnboardingCard {
    OnboardingCard {
        eyebrow: "Step 6 / 6".to_string(),
        title: "Keep Wayscriber ready?".to_string(),
        body: "Background mode keeps Wayscriber running.".to_string(),
        items: vec![OnboardingChecklistItem {
            label: "Draw a stroke".to_string(),
            done: false,
        }],
        buttons: vec![
            button(
                "Set up",
                "Y",
                OnboardingCardAction::SetUpBackgroundMode,
                true,
            ),
            button(
                "Not now",
                "N",
                OnboardingCardAction::SkipBackgroundMode,
                false,
            ),
        ],
        footer: String::new(),
    }
}

fn paint(
    card: &OnboardingCard,
    (width, height): (u32, u32),
    hovered: Option<OnboardingCardAction>,
) -> (cairo::ImageSurface, OnboardingCardLayout) {
    let engine = UiTextEngine::default();
    let surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, width as i32, height as i32).unwrap();
    let layout = {
        let ctx = cairo::Context::new(&surface).unwrap();
        render_onboarding_card_with_engine(&engine, &ctx, width, height, card, hovered)
    };
    (surface, layout)
}

/// Bounding box of every painted pixel.
fn painted_bounds(surface: &mut cairo::ImageSurface) -> (i32, i32, i32, i32) {
    let width = surface.width();
    let height = surface.height();
    let stride = surface.stride() as usize;
    let data = surface.data().expect("surface data");
    let mut bounds = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for y in 0..height {
        for x in 0..width {
            if data[y as usize * stride + x as usize * 4 + 3] != 0 {
                bounds.0 = bounds.0.min(x);
                bounds.1 = bounds.1.min(y);
                bounds.2 = bounds.2.max(x);
                bounds.3 = bounds.3.max(y);
            }
        }
    }
    bounds
}

#[test]
fn the_returned_layout_is_the_painted_card() {
    let (mut surface, layout) = paint(&card(), (1280, 720), None);

    let (min_x, min_y, max_x, max_y) = painted_bounds(&mut surface);
    // The hairline border straddles the edge and antialiases into the
    // neighbouring pixel row.
    assert!(
        f64::from(min_x) >= layout.x - 2.0,
        "{layout:?} vs x={min_x}"
    );
    assert!(
        f64::from(min_y) >= layout.y - 2.0,
        "{layout:?} vs y={min_y}"
    );
    assert!(f64::from(max_x) <= layout.x + layout.width + 2.0);
    assert!(f64::from(max_y) <= layout.y + layout.height + 2.0);
    assert!(layout.contains(f64::from(min_x + 4), f64::from(min_y + 4)));
    assert!(layout.contains(f64::from(max_x - 4), f64::from(max_y - 4)));
}

#[test]
fn buttons_sit_inside_the_card_in_order_and_resolve_presses() {
    let (_, layout) = paint(&card(), (1280, 720), None);

    let actions: Vec<_> = layout.buttons.iter().map(|hit| hit.action).collect();
    assert_eq!(
        actions,
        vec![
            OnboardingCardAction::SetUpBackgroundMode,
            OnboardingCardAction::SkipBackgroundMode
        ]
    );
    let first = layout.buttons[0];
    let second = layout.buttons[1];
    assert!(first.x + first.width < second.x, "one row, left to right");
    for hit in &layout.buttons {
        assert!(layout.contains(hit.x, hit.y));
        assert!(layout.contains(hit.x + hit.width, hit.y + hit.height));
        assert_eq!(
            layout.press_at(hit.x + hit.width / 2.0, hit.y + hit.height / 2.0),
            Some(OnboardingCardPress::Button(hit.action))
        );
    }

    assert_eq!(
        layout.press_at(layout.x + 4.0, layout.y + 4.0),
        Some(OnboardingCardPress::Body)
    );
    assert_eq!(layout.press_at(layout.x - 4.0, layout.y + 4.0), None);
}

#[test]
fn buttons_wrap_instead_of_leaving_a_narrow_card() {
    let mut card = card();
    card.buttons.push(button(
        "Skip the whole tour now",
        "Shift+Esc",
        OnboardingCardAction::SkipTour,
        false,
    ));
    let (_, layout) = paint(&card, (420, 700), None);

    for hit in &layout.buttons {
        assert!(hit.x + hit.width <= layout.x + layout.width, "{hit:?}");
    }
    let rows: std::collections::BTreeSet<i64> = layout
        .buttons
        .iter()
        .map(|hit| hit.y.round() as i64)
        .collect();
    assert!(rows.len() > 1, "the third button moves to its own row");
    let last = layout.buttons.last().unwrap();
    assert!(last.y + last.height <= layout.y + layout.height);
}

#[test]
fn hovering_a_button_changes_only_the_card_pixels() {
    let (mut idle, layout) = paint(&card(), (1280, 720), None);
    let (mut hovered, hovered_layout) = paint(
        &card(),
        (1280, 720),
        Some(OnboardingCardAction::SkipBackgroundMode),
    );

    assert_eq!(layout, hovered_layout, "hover never moves anything");
    assert!(idle.data().unwrap().to_vec() != hovered.data().unwrap().to_vec());
}

#[test]
fn a_card_without_buttons_or_footer_stays_compact() {
    let mut bare = card();
    bare.buttons.clear();
    let (_, bare_layout) = paint(&bare, (1280, 720), None);
    let (_, full_layout) = paint(&card(), (1280, 720), None);

    assert!(bare_layout.buttons.is_empty());
    assert!(bare_layout.height < full_layout.height);
}
