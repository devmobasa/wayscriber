use super::*;
use crate::input::boards::{BoardConfigChange, PendingBoardConfigUpdate};
use std::fs;
use std::sync::mpsc;
use std::time::Instant;

/// Keeps every test's safety-net copy inside its own temp directory instead of
/// the developer's real XDG state directory.
fn test_backup(path: &Path) -> RuntimeConfigBackup {
    RuntimeConfigBackup::with_directory(backup_dir(path))
}

fn backup_dir(path: &Path) -> PathBuf {
    path.parent()
        .expect("a test config path has a parent")
        .join("config-backups")
}

fn backup_contents(path: &Path) -> Vec<String> {
    let directory = backup_dir(path);
    if !directory.exists() {
        return Vec::new();
    }
    let mut entries = fs::read_dir(&directory)
        .expect("backup directory should be listable")
        .filter_map(Result::ok)
        .map(|entry| fs::read_to_string(entry.path()).expect("snapshot should be readable"))
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

/// One-shot persistence for tests that are not about the backup guard.
fn persist_mutations(path: &Path, mutations: &[ConfigMutation]) -> Result<()> {
    persist_mutations_to_path(path, mutations, &mut test_backup(path))
}

/// A cheap, distinct edit for tests about the queue rather than the payload.
fn quick_color(index: usize, red: f64) -> ConfigMutation {
    ConfigMutation::QuickColor {
        index,
        color: Color {
            r: red,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    }
}

fn quick_color_at(config: &Config, index: usize) -> crate::config::ColorSpec {
    config.drawing.quick_colors.effective_entries()[index]
        .color
        .clone()
}

fn pen_binding(bindings: &[&str], receipt: ConfigWriteReceipt) -> ConfigMutation {
    ConfigMutation::Keybinding {
        action: Action::SelectPenTool,
        bindings: bindings.iter().map(|value| value.to_string()).collect(),
        receipt,
    }
}

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

    assert!(writer.request(&quick_color(0, 0.25)));
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the test writer should enter its first persistence call");

    let started = Instant::now();
    let queued = writer.request(&quick_color(1, 0.5));
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

    assert!(writer.request(&quick_color(0, 0.25)));
    assert!(writer.request(&quick_color(1, 0.5)));
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
                ConfigMutation::Keybinding { .. } => "keybinding",
            })
            .collect();
        batches_tx
            .send(kinds)
            .map_err(|error| anyhow::anyhow!("batch observer disconnected: {error}"))?;
        Ok(())
    });
    let mut writer = ConfigWriter::spawn(persist);
    let boards = Config::default().resolved_boards();

    assert!(writer.request(&ConfigMutation::BoardConfig(Box::new(
        PendingBoardConfigUpdate::new(boards, BoardConfigChange::Structure),
    ))));
    assert!(writer.request(&ConfigMutation::PresetSlot {
        slot: 1,
        preset: None,
    }));
    assert!(writer.request(&quick_color(0, 0.0)));
    assert!(writer.request(&pen_binding(&["Ctrl+Alt+P"], ConfigWriteReceipt::initial())));
    writer.shutdown();

    assert_eq!(
        batches_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("shutdown should flush one shared config batch"),
        ["board", "preset", "quick-color", "keybinding"]
    );
}

#[test]
fn repeated_edits_to_one_slot_keep_only_the_latest_value() {
    let (batches_tx, batches_rx) = mpsc::channel();
    let persist = Box::new(move |mutations: &[ConfigMutation]| {
        let mut config = Config::default();
        for mutation in mutations {
            let _ = mutation.apply(&mut config);
        }
        batches_tx
            .send((mutations.len(), quick_color_at(&config, 0)))
            .map_err(|error| anyhow::anyhow!("batch observer disconnected: {error}"))?;
        Ok(())
    });
    let mut writer = ConfigWriter::spawn(persist);

    assert!(writer.request(&quick_color(0, 0.25)));
    assert!(writer.request(&quick_color(0, 0.75)));
    writer.shutdown();

    let (batched, color) = batches_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("shutdown should flush the coalesced edit");
    assert_eq!(batched, 1);
    assert_eq!(
        color,
        crate::config::ColorSpec::from(Color {
            r: 0.75,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        })
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
    let receipt = ConfigWriteReceipt::initial();

    assert!(writer.request(&pen_binding(&["Ctrl+Alt+P"], receipt)));
    attempts_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the first write should be attempted");
    assert!(
        writer.take_completed_keybinding_writes().is_empty(),
        "a failed write must keep its shortcut pending"
    );
    writer.shutdown();

    attempts_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("shutdown should retry the retained mutation");
    assert_eq!(writer.take_completed_keybinding_writes(), vec![receipt]);
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

    persist_mutations(&path, &[quick_color(0, 0.5)]).expect("mutation should persist");

    let written = fs::read_to_string(&path).expect("persisted config should be readable");
    assert!(written.contains("# external heading"));
    assert!(written.contains("future_setting = \"keep\""));
    let reloaded = ConfigDocument::load_from_path(&path).expect("saved config should parse");
    assert!(
        reloaded.config().ui.toolbar.use_icons,
        "an unrelated authored preference stays exactly as written"
    );
    assert_eq!(
        quick_color_at(reloaded.config(), 0),
        crate::config::ColorSpec::from(Color {
            r: 0.5,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        })
    );
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

    persist_mutations(
        &path,
        &[
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
    assert!(
        config.ui.toolbar.use_icons,
        "authored preferences are not the writer's to touch"
    );
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

/// A shortcut edit is the one runtime write that used to save the whole
/// config. Through the writer it must touch its own `[keybindings]` key only:
/// comments, unrelated sections, and every other binding stay byte-identical.
#[test]
fn a_keybinding_edit_rewrites_only_the_edited_action() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    fs::write(
        &path,
        "# authored by hand\n[keybindings]\n# the pen lives here\nselect_pen_tool = ['F']\nundo = ['Ctrl+Alt+U']\n\n[ui]\nshow_status_bar = false\n",
    )
    .expect("test config should be written");

    persist_mutations(
        &path,
        &[ConfigMutation::Keybinding {
            action: Action::SelectPenTool,
            bindings: vec!["Ctrl+Alt+Shift+K".to_string()],
            receipt: ConfigWriteReceipt::initial(),
        }],
    )
    .expect("the keybinding edit should persist");

    let written = fs::read_to_string(&path).expect("persisted config should be readable");
    assert!(written.contains("# authored by hand"));
    assert!(written.contains("# the pen lives here"));
    assert!(written.contains("undo = ['Ctrl+Alt+U']"));
    assert!(written.contains("show_status_bar = false"));
    assert!(!written.contains("['F']"));

    let reloaded = ConfigDocument::load_from_path(&path).expect("saved config should parse");
    assert_eq!(
        reloaded
            .config()
            .keybindings
            .bindings_for_action(Action::SelectPenTool),
        Some(&["Ctrl+Alt+Shift+K".to_string()][..])
    );
    // Nothing the edit did not name is materialized, so the rest of the
    // shipped defaults stay absent from the file.
    assert!(!written.contains("clear_canvas"));
    assert!(!written.contains("toggle_help"));
}

/// Unbinding an action writes the empty list rather than dropping the key,
/// which is what keeps the default from reappearing on the next load.
#[test]
fn deleting_a_keybinding_persists_an_empty_list() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    fs::write(&path, "[keybindings]\nselect_pen_tool = ['F']\n")
        .expect("test config should be written");

    persist_mutations(
        &path,
        &[ConfigMutation::Keybinding {
            action: Action::SelectPenTool,
            bindings: Vec::new(),
            receipt: ConfigWriteReceipt::initial(),
        }],
    )
    .expect("the unbind should persist");

    let reloaded = ConfigDocument::load_from_path(&path).expect("saved config should parse");
    assert_eq!(
        reloaded
            .config()
            .keybindings
            .bindings_for_action(Action::SelectPenTool),
        Some(&[][..])
    );
}

/// Rapid re-edits of one shortcut collapse onto the latest value, while a
/// second action keeps its own queued entry.
#[test]
fn keybinding_edits_coalesce_per_action() {
    let (batches_tx, batches_rx) = mpsc::channel();
    let persist = Box::new(move |mutations: &[ConfigMutation]| {
        let mut config = Config::default();
        for mutation in mutations {
            let _ = mutation.apply(&mut config);
        }
        batches_tx
            .send((
                mutations.len(),
                config
                    .keybindings
                    .bindings_for_action(Action::SelectPenTool)
                    .map(<[String]>::to_vec)
                    .unwrap_or_default(),
                config
                    .keybindings
                    .bindings_for_action(Action::ClearCanvas)
                    .map(<[String]>::to_vec)
                    .unwrap_or_default(),
            ))
            .map_err(|error| anyhow::anyhow!("batch observer disconnected: {error}"))?;
        Ok(())
    });
    let mut writer = ConfigWriter::spawn(persist);
    let first = ConfigWriteReceipt::initial();
    let second = first
        .successor()
        .expect("the test receipt should have a successor");
    let third = second
        .successor()
        .expect("the test receipt should have a successor");

    assert!(writer.request(&ConfigMutation::Keybinding {
        action: Action::SelectPenTool,
        bindings: vec!["Ctrl+P".to_string()],
        receipt: first,
    }));
    assert!(writer.request(&ConfigMutation::Keybinding {
        action: Action::ClearCanvas,
        bindings: vec!["Ctrl+L".to_string()],
        receipt: second,
    }));
    assert!(writer.request(&ConfigMutation::Keybinding {
        action: Action::SelectPenTool,
        bindings: vec!["Ctrl+Alt+P".to_string()],
        receipt: third,
    }));
    writer.shutdown();

    assert_eq!(
        batches_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("shutdown should flush the coalesced edits"),
        (
            2,
            vec!["Ctrl+Alt+P".to_string()],
            vec!["Ctrl+L".to_string()]
        )
    );
    assert_eq!(
        writer.take_completed_keybinding_writes(),
        vec![second, third],
        "only the coalesced mutations in the durable batch are acknowledged"
    );
}

/// A shortcut edit shares the batch with the other editor flows queued around
/// it, and all of them land in one document revision.
#[test]
fn a_keybinding_edit_shares_the_batch_with_other_editor_flows() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    fs::write(&path, "[ui.toolbar]\nuse_icons = true\n").expect("test config should be written");

    persist_mutations(
        &path,
        &[
            quick_color(0, 0.5),
            ConfigMutation::Keybinding {
                action: Action::Undo,
                bindings: vec!["Ctrl+Alt+U".to_string()],
                receipt: ConfigWriteReceipt::initial(),
            },
        ],
    )
    .expect("the mixed batch should persist");

    let reloaded = ConfigDocument::load_from_path(&path).expect("saved config should parse");
    let config = reloaded.config();
    assert_eq!(
        quick_color_at(config, 0),
        crate::config::ColorSpec::from(Color {
            r: 0.5,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        })
    );
    assert_eq!(
        config.keybindings.bindings_for_action(Action::Undo),
        Some(&["Ctrl+Alt+U".to_string()][..])
    );
}

/// Runtime-only actions have no `[keybindings]` field. The editor refuses them
/// long before the writer sees one, and the writer refuses to invent a key.
#[test]
fn a_runtime_only_action_is_not_written() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    let original = "[keybindings]\nselect_pen_tool = ['F']\n";
    fs::write(&path, original).expect("test config should be written");

    persist_mutations(
        &path,
        &[ConfigMutation::Keybinding {
            action: Action::ReplayTour,
            bindings: vec!["R".to_string()],
            receipt: ConfigWriteReceipt::initial(),
        }],
    )
    .expect("an unwritable action should not fail the batch");

    assert_eq!(
        fs::read_to_string(&path).expect("config should still be readable"),
        original
    );
}

#[test]
fn shutdown_flushes_the_real_document_writer() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    fs::write(&path, "[keybindings]\nselect_pen_tool = ['F']\n")
        .expect("test config should be written");
    let mut writer = ConfigWriter::for_path(path.clone(), test_backup(&path));

    assert!(writer.request(&pen_binding(&["Ctrl+Alt+P"], ConfigWriteReceipt::initial())));
    writer.shutdown();

    let reloaded = ConfigDocument::load_from_path(&path).expect("saved config should parse");
    assert_eq!(
        reloaded
            .config()
            .keybindings
            .bindings_for_action(Action::SelectPenTool),
        Some(&["Ctrl+Alt+P".to_string()][..])
    );
}

/// Every runtime save used to overwrite `config.toml` with no copy anywhere.
/// The writer now takes one snapshot of the file as the session found it,
/// before the first batch that actually changes something.
#[test]
fn the_writers_first_batch_snapshots_the_config_it_is_about_to_change() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    let original = "# authored by hand\n[keybindings]\nselect_pen_tool = ['F']\n";
    fs::write(&path, original).expect("test config should be written");
    let mut backup = test_backup(&path);

    persist_mutations_to_path(
        &path,
        &[pen_binding(&["Ctrl+Alt+P"], ConfigWriteReceipt::initial())],
        &mut backup,
    )
    .expect("the first batch should persist");

    assert_eq!(backup_contents(&path), vec![original.to_string()]);
    assert!(fs::read_to_string(&path).unwrap().contains("Ctrl+Alt+P"));
}

/// One copy per session, not per write: the snapshot has to keep the file the
/// user authored, not the one the previous batch left behind.
#[test]
fn later_batches_reuse_the_processs_single_snapshot() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    let original = "[keybindings]\nselect_pen_tool = ['F']\n";
    fs::write(&path, original).expect("test config should be written");
    let mut backup = test_backup(&path);

    persist_mutations_to_path(
        &path,
        &[pen_binding(&["Ctrl+Alt+P"], ConfigWriteReceipt::initial())],
        &mut backup,
    )
    .expect("the first batch should persist");
    persist_mutations_to_path(&path, &[quick_color(0, 0.5)], &mut backup)
        .expect("the second batch should persist");

    assert_eq!(backup_contents(&path), vec![original.to_string()]);
}

/// A batch whose every mutation drops out changes nothing, so it must not
/// spend the snapshot the next real save will want.
#[test]
fn a_batch_that_writes_nothing_leaves_the_snapshot_unspent() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    let original = "[keybindings]\nselect_pen_tool = ['F']\n";
    fs::write(&path, original).expect("test config should be written");
    let mut backup = test_backup(&path);

    persist_mutations_to_path(
        &path,
        &[ConfigMutation::Keybinding {
            action: Action::ReplayTour,
            bindings: vec!["R".to_string()],
            receipt: ConfigWriteReceipt::initial(),
        }],
        &mut backup,
    )
    .expect("an unwritable action should not fail the batch");
    assert!(backup_contents(&path).is_empty());

    persist_mutations_to_path(
        &path,
        &[pen_binding(&["Ctrl+Alt+P"], ConfigWriteReceipt::initial())],
        &mut backup,
    )
    .expect("the first real batch should persist");

    assert_eq!(backup_contents(&path), vec![original.to_string()]);
}

/// The net must never be the reason an edit fails to save.
#[test]
fn an_unusable_backup_directory_does_not_block_the_save() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    fs::write(&path, "[keybindings]\nselect_pen_tool = ['F']\n")
        .expect("test config should be written");
    // A regular file where the backup directory belongs.
    fs::write(backup_dir(&path), "not a directory\n").expect("blocking file should be written");
    let mut backup = RuntimeConfigBackup::with_directory(backup_dir(&path));

    persist_mutations_to_path(
        &path,
        &[pen_binding(&["Ctrl+Alt+P"], ConfigWriteReceipt::initial())],
        &mut backup,
    )
    .expect("a failed snapshot must not fail the save");

    let reloaded = ConfigDocument::load_from_path(&path).expect("saved config should parse");
    assert_eq!(
        reloaded
            .config()
            .keybindings
            .bindings_for_action(Action::SelectPenTool),
        Some(&["Ctrl+Alt+P".to_string()][..])
    );
}

/// `apply` reports that it stored a value, not that the value differed, so an
/// edit set back to where it started reaches this point looking like a real
/// one. It must not rewrite the file or spend the snapshot.
#[test]
fn a_batch_that_changes_nothing_leaves_the_file_and_the_snapshot_alone() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    let original = "# authored by hand\n[keybindings]\nselect_pen_tool = ['F']\n";
    fs::write(&path, original).expect("test config should be written");
    let authored_at = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    fs::File::options()
        .write(true)
        .open(&path)
        .expect("test config should be openable")
        .set_modified(authored_at)
        .expect("test config mtime should be settable");
    let mut backup = test_backup(&path);

    persist_mutations_to_path(
        &path,
        &[pen_binding(&["F"], ConfigWriteReceipt::initial())],
        &mut backup,
    )
    .expect("a batch that changes nothing should still succeed");

    assert!(backup_contents(&path).is_empty());
    assert_eq!(
        fs::metadata(&path)
            .expect("config metadata should be readable")
            .modified()
            .expect("config mtime should be readable"),
        authored_at,
        "a no-op batch must not touch the file at all"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("config should be readable"),
        original
    );

    persist_mutations_to_path(
        &path,
        &[pen_binding(&["Ctrl+Alt+P"], ConfigWriteReceipt::initial())],
        &mut backup,
    )
    .expect("the first real change should persist");

    assert_eq!(backup_contents(&path), vec![original.to_string()]);
    assert!(
        fs::read_to_string(&path)
            .expect("config should be readable")
            .contains("Ctrl+Alt+P")
    );
}

/// The editor keeps replaying a shortcut edit until its receipt comes back, so
/// a batch skipped as a no-op has to settle exactly like one that wrote.
#[test]
fn a_keybinding_edit_that_changes_nothing_still_settles_its_receipt() {
    let temp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = temp.path().join("config.toml");
    let original = "[keybindings]\nselect_pen_tool = ['F']\n";
    fs::write(&path, original).expect("test config should be written");
    let mut writer = ConfigWriter::for_path(path.clone(), test_backup(&path));
    let receipt = ConfigWriteReceipt::initial();

    assert!(writer.request(&ConfigMutation::Keybinding {
        action: Action::SelectPenTool,
        bindings: vec!["F".to_string()],
        receipt,
    }));
    writer.shutdown();

    assert_eq!(writer.take_completed_keybinding_writes(), vec![receipt]);
    assert_eq!(
        fs::read_to_string(&path).expect("config should be readable"),
        original
    );
    assert!(backup_contents(&path).is_empty());
}
