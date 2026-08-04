use super::*;
use crate::models::SearchQuery;

fn app_with_a_profile() -> ConfiguratorApp {
    let (mut app, _effects) = ConfiguratorApp::new_app();
    let profile = app.draft.render_profiles.new_profile();
    app.draft.render_profiles.profiles.push(profile);
    app
}

/// The law the binding rests on: one layout per profile, and a row's
/// values carry exactly the mappings that layout built rows for.
#[test]
fn a_row_carries_one_value_per_mapping_row_the_layout_built() {
    let app = app_with_a_profile();
    let layouts = section_layouts(&app, &app.search_summary());

    assert_eq!(layouts.len(), app.draft.render_profiles.profiles.len());
    for (profile, layout) in app
        .draft
        .render_profiles
        .profiles
        .iter()
        .zip(layouts.iter())
    {
        assert_eq!(profile.mappings.len(), layout.mappings.len());
    }
}

/// The caret guarantee, stated as the layout law it rests on: no keystroke
/// in a profile's text may rebuild the row it lands in.
#[test]
fn typing_into_a_profile_field_leaves_the_layout_alone() {
    let mut app = app_with_a_profile();
    let before = section_layouts(&app, &app.search_summary());

    let Some(profile) = app.draft.render_profiles.profiles.first_mut() else {
        return;
    };
    profile.name = "Half typ".to_string();
    profile.id = "half-typ".to_string();
    if let Some(mapping) = profile.mappings.first_mut() {
        mapping.from = "#00FF0".to_string();
    }

    assert_eq!(before, section_layouts(&app, &app.search_summary()));
    assert_eq!(app.draft.render_profiles.profiles[0].id, "half-typ");
}

#[test]
fn adding_a_mapping_changes_the_layout() {
    let mut app = app_with_a_profile();
    let before = section_layouts(&app, &app.search_summary());

    let Some(profile) = app.draft.render_profiles.profiles.first_mut() else {
        return;
    };
    let Some(mapping) = profile.mappings.first().cloned() else {
        return;
    };
    profile.mappings.push(mapping);

    assert_ne!(before, section_layouts(&app, &app.search_summary()));
}

/// Search visibility belongs to the layout too: a rebuild is what applies
/// it now that nothing refreshes a section in place.
#[test]
fn a_search_that_hides_a_profile_changes_the_layout() {
    let mut app = app_with_a_profile();
    let second = app.draft.render_profiles.new_profile();
    app.draft.render_profiles.profiles.push(second);
    let Some(profile) = app.draft.render_profiles.profiles.first_mut() else {
        return;
    };
    profile.name = "zqxwvu".to_string();
    let before = section_layouts(&app, &app.search_summary());

    app.search_query = SearchQuery::new("zqxwvu");
    let layouts = section_layouts(&app, &app.search_summary());

    assert_ne!(before, layouts);
    let visible: Vec<bool> = layouts.iter().map(|layout| layout.visible).collect();
    assert_eq!(visible.first(), Some(&true));
    assert!(visible.iter().skip(1).all(|visible| !visible));
}
