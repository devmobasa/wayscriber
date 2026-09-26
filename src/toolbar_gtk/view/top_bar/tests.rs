//! GTK top-strip unit tests.

mod builtin_contract;
mod builtin_style_nodes;
mod expectations;
mod gtk_contract;
mod gtk_level_widgets;
mod gtk_style_widgets;
mod interactions;
mod pane_popovers;
mod pen_feel;
mod structure;
mod toolbar_popovers;
mod widget_support;

#[test]
fn actual_gtk_widgets_match_the_shared_contract_without_presenting_a_window() {
    const CHILD_ENV: &str = "WAYSCRIBER_GTK_WIDGET_CONTRACT_CHILD";
    const TEST_NAME: &str = "toolbar_gtk::view::top_bar::tests::actual_gtk_widgets_match_the_shared_contract_without_presenting_a_window";

    if std::env::var_os(CHILD_ENV).is_none() {
        let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .arg(TEST_NAME)
            .arg("--exact")
            .arg("--test-threads=1")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .status()
            .expect("run isolated GTK widget contract test");
        assert!(status.success(), "isolated GTK widget contract test failed");
        return;
    }

    if let Err(error) = gtk4::init() {
        assert!(
            std::env::var_os("WAYSCRIBER_REQUIRE_GTK_TESTS").is_none(),
            "Required GTK widget coverage could not initialize: {error}"
        );
        eprintln!("skipping GTK widget contract test: {error}");
        return;
    }

    gtk_contract::install_gtk_contract_metrics();
    let (regular, highlighted, scenarios) = gtk_contract::gtk_widget_contract_scenarios();
    let font_button_widths = gtk_contract::assert_gtk_widget_scenarios(scenarios);

    gtk_contract::assert_font_button_width_stable(&font_button_widths);

    // Compact plans normally drop quick colors before reaching the last
    // degradation step. Keep a direct adapter case so the presentation
    // contract cannot silently diverge if that planner policy changes.
    // Colors left the strip (M7-C1) and the presets island yields under the
    // compact plan (M7-C2): assert neither renders in a compact build.
    gtk_contract::assert_compact_gtk_widget_contract(&regular);

    gtk_contract::assert_preset_slot_faces(&regular);

    toolbar_popovers::assert_shapes_and_overflow_contract(&regular);

    interactions::assert_gtk_toggle_events(&regular, &highlighted);

    interactions::assert_style_pill_interactions(&regular);

    pane_popovers::assert_menu_popover_contracts(&regular);

    toolbar_popovers::assert_key_relay_contract(&regular);

    toolbar_popovers::assert_layout_menu_contract(&regular);
    toolbar_popovers::assert_arrow_style_menu_contract(&regular);

    pen_feel::assert_pen_feel_contract(&regular);
    pen_feel::assert_meter_scroll_survives_delayed_snapshots();
    eprintln!("EXECUTED: GTK widget contract assertions");
}
