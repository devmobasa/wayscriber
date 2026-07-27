use super::*;
use crate::config::ToolbarSectionVisibility;
use crate::input::boards::{BoardConfigChange, PendingBoardConfigUpdate};
use std::fs;
use std::sync::mpsc;
use std::time::Instant;

#[test]
fn request_does_not_wait_for_an_in_flight_write() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut block_first_write = true;
    let persist = Box::new(move |_mutations: &[ConfigMutation]| {
        if block_first_write {
            block_first_write = false;
            let _ = entered_tx.send(());
            release_rx
                .recv()
                .map_err(|error| anyhow::anyhow!("release signal missing: {error}"))?;
        }
        Ok(())
    });
    let mut writer = ConfigWriter::spawn(persist);

    assert!(writer.request(&ConfigMutation::ToolbarUseIcons(false)));
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the test writer should enter its first persistence call");

    let started = Instant::now();
    let queued = writer.request(&ConfigMutation::ToolbarShowMoreColors(true));
    let elapsed = started.elapsed();

    let _ = release_tx.send(());
    writer.shutdown();

    assert!(queued);
    assert!(
        elapsed < Duration::from_millis(50),
        "queueing must not wait for the blocked persistence call"
    );
}

#[test]
fn shutdown_coalesces_rapid_mutations_into_one_write() {
    let (batches_tx, batches_rx) = mpsc::channel();
    let persist = Box::new(move |mutations: &[ConfigMutation]| {
        batches_tx
            .send(mutations.len())
            .map_err(|error| anyhow::anyhow!("batch observer disconnected: {error}"))?;
        Ok(())
    });
    let mut writer = ConfigWriter::spawn(persist);

    assert!(writer.request(&ConfigMutation::ToolbarUseIcons(false)));
    assert!(writer.request(&ConfigMutation::ToolbarShowMoreColors(true)));
    writer.shutdown();

    assert_eq!(
        batches_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("shutdown should flush one batch"),
        2
    );
    assert!(
        batches_rx.try_recv().is_err(),
        "the two rapid edits should share one durable write"
    );
}

#[test]
fn every_wayland_config_family_shares_the_writer_batch() {
    let (batches_tx, batches_rx) = mpsc::channel();
    let persist = Box::new(move |mutations: &[ConfigMutation]| {
        let kinds: Vec<_> = mutations
            .iter()
            .map(|mutation| match mutation {
                ConfigMutation::BoardConfig(_) => "board",
                ConfigMutation::PresetSlot { .. } => "preset",
                ConfigMutation::QuickColor { .. } => "quick-color",
                _ => "toolbar",
            })
            .collect();
        batches_tx
            .send(kinds)
            .map_err(|error| anyhow::anyhow!("batch observer disconnected: {error}"))?;
        Ok(())
    });
    let mut writer = ConfigWriter::spawn(persist);
    let boards = Config::default().resolved_boards();

    assert!(writer.request(&ConfigMutation::ToolbarUseIcons(false)));
    assert!(writer.request(&ConfigMutation::BoardConfig(Box::new(
        PendingBoardConfigUpdate::new(boards, BoardConfigChange::Structure),
    ))));
    assert!(writer.request(&ConfigMutation::PresetSlot {
        slot: 1,
        preset: None,
    }));
    assert!(writer.request(&ConfigMutation::QuickColor {
        index: 0,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    }));
    writer.shutdown();

    assert_eq!(
        batches_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("shutdown should flush one shared config batch"),
        ["toolbar", "board", "preset", "quick-color"]
    );
}

#[test]
fn repeated_edits_to_one_preference_keep_only_the_latest_value() {
    let (batches_tx, batches_rx) = mpsc::channel();
    let persist = Box::new(move |mutations: &[ConfigMutation]| {
        let mut config = Config::default();
        for mutation in mutations {
            let _ = mutation.apply(&mut config);
        }
        batches_tx
            .send((mutations.len(), config.ui.toolbar.use_icons))
            .map_err(|error| anyhow::anyhow!("batch observer disconnected: {error}"))?;
        Ok(())
    });
    let mut writer = ConfigWriter::spawn(persist);

    assert!(writer.request(&ConfigMutation::ToolbarUseIcons(false)));
    assert!(writer.request(&ConfigMutation::ToolbarUseIcons(true)));
    writer.shutdown();

    assert_eq!(
        batches_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("shutdown should flush the coalesced edit"),
        (1, true)
    );
}

#[test]
fn board_edits_remain_ordered_instead_of_being_coalesced() {
    let (batches_tx, batches_rx) = mpsc::channel();
    let persist = Box::new(move |mutations: &[ConfigMutation]| {
        batches_tx
            .send(mutations.len())
            .map_err(|error| anyhow::anyhow!("batch observer disconnected: {error}"))?;
        Ok(())
    });
    let mut writer = ConfigWriter::spawn(persist);
    let boards = Config::default().resolved_boards();

    assert!(writer.request(&ConfigMutation::BoardConfig(Box::new(
        PendingBoardConfigUpdate::new(boards.clone(), BoardConfigChange::Structure),
    ))));
    assert!(writer.request(&ConfigMutation::BoardConfig(Box::new(
        PendingBoardConfigUpdate::new(boards, BoardConfigChange::Structure),
    ))));
    writer.shutdown();

    assert_eq!(
        batches_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("shutdown should flush both ordered board edits"),
        2
    );
}

#[test]
fn partial_click_highlight_edits_remain_ordered() {
    let (batches_tx, batches_rx) = mpsc::channel();
    let persist = Box::new(move |mutations: &[ConfigMutation]| {
        let mut config = Config::default();
        for mutation in mutations {
            let _ = mutation.apply(&mut config);
        }
        batches_tx
            .send((
                mutations.len(),
                config.ui.click_highlight.enabled,
                config.ui.click_highlight.show_on_highlight_tool,
            ))
            .map_err(|error| anyhow::anyhow!("batch observer disconnected: {error}"))?;
        Ok(())
    });
    let mut writer = ConfigWriter::spawn(persist);

    assert!(writer.request(&ConfigMutation::ClickHighlight {
        enabled: Some(true),
        show_on_highlight_tool: false,
    }));
    assert!(writer.request(&ConfigMutation::ClickHighlight {
        enabled: None,
        show_on_highlight_tool: true,
    }));
    writer.shutdown();

    assert_eq!(
        batches_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("shutdown should preserve both partial edits"),
        (2, true, true)
    );
}

/// The HUD toggle is a single boolean, so repeated flips coalesce onto the
/// latest value instead of queueing one write per keypress.
#[test]
fn repeated_input_hud_toggles_keep_only_the_latest_value() {
    let (batches_tx, batches_rx) = mpsc::channel();
    let persist = Box::new(move |mutations: &[ConfigMutation]| {
        let mut config = Config::default();
        for mutation in mutations {
            let _ = mutation.apply(&mut config);
        }
        batches_tx
            .send((mutations.len(), config.ui.input_hud.enabled))
            .map_err(|error| anyhow::anyhow!("batch observer disconnected: {error}"))?;
        Ok(())
    });
    let mut writer = ConfigWriter::spawn(persist);

    assert!(writer.request(&ConfigMutation::InputHud(true)));
    assert!(writer.request(&ConfigMutation::InputHud(false)));
    assert!(writer.request(&ConfigMutation::InputHud(true)));
    writer.shutdown();

    assert_eq!(
        batches_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("shutdown should flush the coalesced edit"),
        (1, true)
    );
}

#[test]
fn a_failed_write_is_retained_for_the_shutdown_retry() {
    let (attempts_tx, attempts_rx) = mpsc::channel();
    let mut fail_first = true;
    let persist = Box::new(move |_mutations: &[ConfigMutation]| {
        let _ = attempts_tx.send(());
        if fail_first {
            fail_first = false;
            return Err(anyhow::anyhow!("injected write failure"));
        }
        Ok(())
    });
    let mut writer = ConfigWriter::spawn(persist);

    assert!(writer.request(&ConfigMutation::ToolbarUseIcons(false)));
    attempts_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the first write should be attempted");
    writer.shutdown();

    attempts_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("shutdown should retry the retained mutation");
}

/// The section visibility a reload derives from `path`, folding the legacy
/// `show_*` flags into explicit overrides exactly as loading does.
fn reloaded_section_visibility(path: &Path) -> ToolbarSectionVisibility {
    let reloaded = ConfigDocument::load_from_path(path).expect("saved config should parse");
    let toolbar = &reloaded.config().ui.toolbar;
    let mut items = toolbar.items.clone();
    let mut legacy = ToolbarSectionVisibility {
        show_actions_section: toolbar.show_actions_section,
        show_actions_advanced: toolbar.show_actions_advanced,
        show_zoom_actions: toolbar.show_zoom_actions,
        show_pages_section: toolbar.show_pages_section,
        show_boards_section: toolbar.show_boards_section,
        show_presets: toolbar.show_presets,
        show_step_section: toolbar.show_step_section,
        show_text_controls: toolbar.show_text_controls,
        show_settings_section: toolbar.show_settings_section,
    };
    legacy.apply_mode_override(toolbar.mode_overrides.for_mode(toolbar.layout_mode));
    crate::config::fold_legacy_section_flags(
        &legacy,
        toolbar.layout_mode,
        &toolbar.mode_overrides,
        &mut items,
    );
    crate::config::resolve_section_visibility(
        toolbar.layout_mode,
        &toolbar.mode_overrides,
        &items.resolved(),
    )
}

/// A layout switch owns `layout_mode`; it must not materialize section flags
/// the user left out. The only mirrors it still writes are the ones loading
/// reads back — without them the pre-switch values fold into explicit
/// overrides and the mode change partly undoes itself on the next start.
#[test]
fn layout_switch_writes_the_mode_and_only_the_mirrors_loading_reads_back() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    fs::write(&path, "[ui.toolbar]\ntop_pinned = true\n").expect("test config should be written");

    persist_mutations_to_path(
        &path,
        &[ConfigMutation::ToolbarLayout(ToolbarLayoutMode::Simple)],
    )
    .expect("the layout switch should persist");

    let written = fs::read_to_string(&path).expect("persisted config should be readable");
    assert!(written.contains("layout_mode = \"simple\""));
    for key in [
        "show_actions_section",
        "show_actions_advanced",
        "show_zoom_actions",
        "show_pages_section",
        "show_boards_section",
        "show_step_section",
        "show_text_controls",
        "show_settings_section",
    ] {
        assert!(
            !written.contains(key),
            "a layout switch must not materialize {key}"
        );
    }
    // Presets is the one section Simple re-baselines away from the value the
    // file already implied, so its mirror has to travel with the mode.
    assert!(written.contains("show_presets = false"));

    let sections = reloaded_section_visibility(&path);
    assert!(!sections.show_presets, "Simple hides presets after reload");
    assert!(sections.show_zoom_actions);
    assert!(!sections.show_step_section);
}

/// The mirrors are scoped to sections the load fold reads: an explicit item
/// override already wins there, so a layout switch leaves both the override
/// and the authored flag alone.
#[test]
fn layout_switch_leaves_explicitly_overridden_sections_untouched() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        "[ui.toolbar]\nshow_settings_section = false\n\n[ui.toolbar.items]\nshown = [\"side.group.presets\"]\n",
    )
    .expect("test config should be written");

    persist_mutations_to_path(
        &path,
        &[ConfigMutation::ToolbarLayout(ToolbarLayoutMode::Simple)],
    )
    .expect("the layout switch should persist");

    let written = fs::read_to_string(&path).expect("persisted config should be readable");
    assert!(!written.contains("show_presets"));
    assert!(written.contains("show_settings_section = false"));
    assert!(written.contains("shown = [\"side.group.presets\"]"));
    assert!(reloaded_section_visibility(&path).show_presets);
}

#[test]
fn each_batch_reloads_and_preserves_the_latest_document() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        "# external heading\n[ui.toolbar]\nuse_icons = true\nfuture_setting = \"keep\"\n",
    )
    .expect("test config should be written");

    persist_mutations_to_path(&path, &[ConfigMutation::ToolbarUseIcons(false)])
        .expect("mutation should persist");

    let written = fs::read_to_string(&path).expect("persisted config should be readable");
    assert!(written.contains("# external heading"));
    assert!(written.contains("future_setting = \"keep\""));
    let reloaded = ConfigDocument::load_from_path(&path).expect("saved config should parse");
    assert!(!reloaded.config().ui.toolbar.use_icons);
}

#[test]
fn mixed_runtime_mutations_persist_through_one_document_revision() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    fs::write(&path, "[ui.toolbar]\nuse_icons = true\n").expect("test config should be written");

    let baseline = Config::default();
    let mut boards = baseline.resolved_boards();
    let board_id = boards.items[0].id.clone();
    boards.items[0].name = "Writer board".to_string();
    let board_update =
        PendingBoardConfigUpdate::new(boards, BoardConfigChange::Name(board_id.clone()));
    let color = Color {
        r: 0.13,
        g: 0.27,
        b: 0.41,
        a: 1.0,
    };
    let preset = ToolPresetConfig {
        name: Some("Writer preset".to_string()),
        tool: crate::input::Tool::Pen,
        color: crate::config::ColorSpec::from(color),
        size: 7.0,
        tool_settings: None,
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: None,
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        polygon_sides: None,
        show_status_bar: None,
        drag_tools: None,
    };

    persist_mutations_to_path(
        &path,
        &[
            ConfigMutation::ToolbarUseIcons(false),
            ConfigMutation::BoardConfig(Box::new(board_update)),
            ConfigMutation::PresetSlot {
                slot: 1,
                preset: Some(Box::new(preset)),
            },
            ConfigMutation::QuickColor { index: 0, color },
        ],
    )
    .expect("the shared runtime mutation batch should persist");

    let reloaded = ConfigDocument::load_from_path(path).expect("saved config should parse");
    let config = reloaded.config();
    assert!(!config.ui.toolbar.use_icons);
    assert_eq!(
        config
            .boards
            .as_ref()
            .and_then(|saved| saved.items.iter().find(|board| board.id == board_id))
            .map(|board| board.name.as_str()),
        Some("Writer board")
    );
    assert_eq!(
        config
            .presets
            .get_slot(1)
            .and_then(|saved| saved.name.as_deref()),
        Some("Writer preset")
    );
    assert_eq!(
        config.drawing.quick_colors.effective_entries()[0].color,
        crate::config::ColorSpec::from(color)
    );
}

#[test]
fn shutdown_flushes_the_real_document_writer() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    fs::write(&path, "[ui.toolbar]\nuse_icons = true\n").expect("test config should be written");
    let mut writer = ConfigWriter::for_path(path.clone());

    assert!(writer.request(&ConfigMutation::ToolbarUseIcons(false)));
    writer.shutdown();

    let reloaded = ConfigDocument::load_from_path(path).expect("saved config should parse");
    assert!(!reloaded.config().ui.toolbar.use_icons);
}
