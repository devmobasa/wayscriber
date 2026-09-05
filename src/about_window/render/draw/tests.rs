use super::*;
use crate::about_window::layout;
use crate::update_check::{AvailableUpdate, DEFAULT_NOTES_URL, DEFAULT_UPDATE_URL};

fn frame_for<'a>(plan: &'a Plan, content: &'a AboutContent, update: &'a UpdateState) -> Frame<'a> {
    Frame {
        plan,
        content,
        update,
        icon: None,
        hover: None,
        focus: None,
        notice: None,
    }
}

fn context(plan: &Plan) -> cairo::Context {
    let surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, plan.width as i32, plan.height as i32)
            .unwrap();
    cairo::Context::new(&surface).unwrap()
}

#[test]
fn about_paints_each_explicit_theme_without_changing_plan() {
    let engine = &UiTextEngine::default();
    let content = AboutContent::build();
    let plan = layout::plan(&content);
    let update = UpdateState::Checking;
    let frame = frame_for(&plan, &content, &update);
    let paint = |theme: &Theme| {
        let mut surface = cairo::ImageSurface::create(
            cairo::Format::ARgb32,
            plan.width.ceil() as i32,
            plan.height.ceil() as i32,
        )
        .unwrap();
        {
            let ctx = cairo::Context::new(&surface).unwrap();
            draw_about(engine, &ctx, theme, &frame);
            assert_eq!(ctx.status(), Ok(()));
        }
        surface.flush();
        surface.data().unwrap().to_vec()
    };
    let dark = paint(&Theme::dark());
    let light = paint(&Theme::light());
    assert!(
        dark != light,
        "explicit About themes must change chrome colors"
    );
    assert!(
        dark == paint(&Theme::dark()),
        "another theme must not replace the first owner's theme"
    );
}

/// The dialog is a fixed width, so row wording has to be chosen to fit it.
/// An ellipsis here means a row's text was written without checking.
#[test]
fn link_row_wording_fits_without_being_ellipsized() {
    let engine = &UiTextEngine::default();
    let content = AboutContent::build();
    let plan = layout::plan(&content);
    let ctx = context(&plan);

    let title_style = style(ROW_TITLE_SIZE, cairo::FontWeight::Normal);
    let detail_style = style(DETAIL_SIZE, cairo::FontWeight::Normal);

    for (rect, link) in plan.link_rows.iter().zip(content.links.iter()) {
        let (_, max_width) = row_text_bounds(*rect);

        assert_eq!(
            fit(engine, &ctx, link.title, title_style, max_width),
            link.title,
            "row title does not fit"
        );
        assert_eq!(
            fit(engine, &ctx, &link.detail, detail_style, max_width),
            link.detail,
            "detail of the {:?} row does not fit",
            link.title
        );
    }
}

#[test]
fn every_update_state_paints_cleanly() {
    let engine = &UiTextEngine::default();
    let content = AboutContent::build();
    let plan = layout::plan(&content);
    let ctx = context(&plan);

    let states = [
        UpdateState::Unavailable,
        UpdateState::Unknown(crate::update_check::Freshness::default()),
        UpdateState::Checking,
        UpdateState::UpToDate(crate::update_check::Freshness {
            checked_seconds_ago: Some(3_600),
            last_attempt_failed: false,
        }),
        UpdateState::UpToDate(crate::update_check::Freshness {
            checked_seconds_ago: Some(3_600),
            last_attempt_failed: true,
        }),
        UpdateState::Available {
            update: Box::new(AvailableUpdate {
                version: "0.9.23".to_string(),
                released: Some("2026-07-20".to_string()),
                update_url: DEFAULT_UPDATE_URL.to_string(),
                notes_url: DEFAULT_NOTES_URL.to_string(),
            }),
            freshness: crate::update_check::Freshness {
                checked_seconds_ago: Some(0),
                last_attempt_failed: false,
            },
        },
        UpdateState::Failed("Network unreachable".to_string()),
    ];

    for state in &states {
        draw_about(
            engine,
            &ctx,
            &Theme::dark(),
            &frame_for(&plan, &content, state),
        );
        assert_eq!(ctx.status(), Ok(()), "state {state:?} failed to paint");
    }
}

#[test]
fn hover_focus_and_notice_paint_cleanly() {
    let engine = &UiTextEngine::default();
    let content = AboutContent::build();
    let plan = layout::plan(&content);
    let ctx = context(&plan);
    let update = UpdateState::Unknown(crate::update_check::Freshness::default());

    let mut frame = frame_for(&plan, &content, &update);
    frame.hover = Some(Element::Link(0));
    frame.focus = Some(Element::Close);
    frame.notice = Some("Copied to clipboard");
    draw_about(engine, &ctx, &Theme::dark(), &frame);

    frame.hover = Some(Element::UpdateCard);
    frame.focus = Some(Element::Button(0));
    frame.notice = None;
    draw_about(engine, &ctx, &Theme::dark(), &frame);

    assert_eq!(ctx.status(), Ok(()));
}

#[test]
fn text_is_trimmed_to_the_width_it_is_given() {
    let engine = &UiTextEngine::default();
    let content = AboutContent::build();
    let plan = layout::plan(&content);
    let ctx = context(&plan);
    let narrow = style(ROW_TITLE_SIZE, cairo::FontWeight::Normal);

    let trimmed = fit(engine, &ctx, "Setup, config, troubleshooting", narrow, 40.0);

    assert!(trimmed.len() < "Setup, config, troubleshooting".len());
    assert!(advance(engine, narrow, &trimmed) <= 40.0);
}
