//! The worker's own contract: order, completion, and teardown.
//!
//! The routing each completion gets is tested beside the gesture that produced
//! it (`state/keybindings.rs`, `state/toolbar/events/presets.rs`,
//! `state/toolbar/events/quick_colors.rs`), where the wording lives. What is
//! left here is what the worker itself promises: every submitted edit produces
//! exactly one completion, in submission order — including when the channel is
//! full, which is the case that used to answer the newest gesture first — and an
//! edit still waiting when the overlay quits reaches the file anyway.

use super::*;
use crate::backend::wayland::RuntimeWakeSource;
use crate::config::ConfigEditWrite;
use crate::config::test_helpers::with_temp_config_home;
use crate::input::Tool;
use crate::input::state::PresetAction;
use std::fs::{self, File, OpenOptions};
use std::path::Path;

const AUTHORED_FILE: &str = "\
# Wayscriber configuration.
[ui]
setting_from_a_later_release = 7
";

fn preset(name: &str) -> Box<crate::config::ToolPresetConfig> {
    Box::new(crate::config::ToolPresetConfig {
        name: Some(name.to_string()),
        tool: Tool::Pen,
        color: crate::config::ColorSpec::from(crate::draw::Color {
            r: 0.13,
            g: 0.27,
            b: 0.41,
            a: 1.0,
        }),
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
    })
}

fn save(slot: usize, name: &str) -> ConfigEdit {
    ConfigEdit::Preset(PresetAction::Save {
        slot,
        preset: preset(name),
    })
}

fn config_in(config_root: &Path) -> std::path::PathBuf {
    let directory = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
    fs::create_dir_all(&directory).expect("the config directory this test named");
    let path = directory.join("config.toml");
    fs::write(&path, AUTHORED_FILE).expect("the fixture this test named a directory for");
    path
}

/// The completions as the event loop sees them.
///
/// Through `try_recv`, not through the worker's channel: an edit waiting for
/// room has no completion in that channel yet, and `try_recv` is also what gives
/// it room. Polling with a deadline rather than blocking, because what is being
/// waited on is a disk write finishing somewhere else.
fn next_completion(worker: &mut ConfigEditWorker) -> ConfigEditCompletion {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(completion) = worker.try_recv() {
            return completion;
        }
        assert!(
            Instant::now() < deadline,
            "every submitted edit is answered exactly once"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

/// The preset name a completion is about, for asserting the order they arrive
/// in rather than only what the file ends up with.
fn completed_preset_name(completion: &ConfigEditCompletion) -> String {
    match &completion.edit {
        ConfigEdit::Preset(PresetAction::Save { preset, .. }) => preset
            .name
            .clone()
            .expect("this suite's presets are all named"),
        other => panic!("expected a preset save, got {other:?}"),
    }
}

/// The lock a config write takes, held the way another process inside its own
/// write window holds it. Nothing else stalls a write for a bounded, staged
/// interval, and a stalled write is the only way to fill the worker's channel on
/// purpose.
fn hold_config_write_lock(config_path: &Path) -> File {
    let name = config_path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("the fixture path this test built has a file name");
    let path = config_path.with_file_name(format!("{name}.lock"));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .expect("the lock file beside a fixture this test created");
    crate::session::try_lock_exclusive(&file)
        .expect("nothing else can hold the lock on a directory this test just made");
    file
}

fn slot_name(path: &Path, slot: usize) -> Option<String> {
    crate::config::ConfigDocument::load_from_path(path)
        .expect("the written config reloads")
        .config()
        .presets
        .get_slot(slot)
        .and_then(|preset| preset.name.clone())
}

/// Two gestures in a row are written in the order they were made.
///
/// They are separate keys here, so the file would hold both either way; the
/// order the *completions* come back in is what the caller relies on to keep
/// each toast with its own gesture.
#[test]
fn two_rapid_edits_are_written_and_completed_in_order() {
    with_temp_config_home(|config_root| {
        let path = config_in(config_root);
        let wake = RuntimeWakeSource::new().expect("an eventfd");
        let mut worker = ConfigEditWorker::new(wake.handle());

        worker.submit(save(1, "First"));
        worker.submit(save(2, "Second"));

        let first = next_completion(&mut worker);
        let second = next_completion(&mut worker);

        for (completion, expected) in [(&first, 1), (&second, 2)] {
            match &completion.edit {
                ConfigEdit::Preset(PresetAction::Save { slot, .. }) => {
                    assert_eq!(*slot, expected, "completions arrive in submission order");
                }
                other => panic!("expected a preset save, got {other:?}"),
            }
            assert_eq!(
                completion
                    .result
                    .as_ref()
                    .expect("a writable fixture accepts the edit")
                    .write,
                ConfigEditWrite::Wrote
            );
        }

        assert_eq!(slot_name(&path, 1).as_deref(), Some("First"));
        assert_eq!(slot_name(&path, 2).as_deref(), Some("Second"));
        // Nothing else in the file moved, which is what makes these one-key
        // writes rather than a rewrite from the worker's own view of the config.
        let after = fs::read_to_string(&path).expect("readable");
        assert!(
            after.contains("setting_from_a_later_release = 7"),
            "{after}"
        );
    });
}

/// Two shortcut edits queued from one batch of input both reach the file.
///
/// The palette can record a capture and then a correction before the backend
/// drains, and each is its own write with its own completion. They are separate
/// actions here, so nothing arbitrates between them: what is under test is that
/// neither is dropped on the way to the worker and both come back, in order,
/// with their own delta attached for the caller to install.
#[test]
fn two_shortcut_edits_are_written_and_completed_in_order() {
    use crate::config::Action;
    use crate::input::state::{KeybindingEditOperation, KeybindingEditRequest};

    fn rebind(action: Action, binding: &str) -> ConfigEdit {
        ConfigEdit::Keybinding(KeybindingEditWrite {
            request: KeybindingEditRequest {
                action,
                operation: KeybindingEditOperation::Replace(vec![binding.to_string()]),
            },
            bindings: vec![binding.to_string()],
        })
    }

    with_temp_config_home(|config_root| {
        let path = config_in(config_root);
        let wake = RuntimeWakeSource::new().expect("an eventfd");
        let mut worker = ConfigEditWorker::new(wake.handle());

        worker.submit(rebind(Action::SelectPenTool, "Ctrl+Alt+Shift+P"));
        worker.submit(rebind(Action::SelectMarkerTool, "Ctrl+Alt+Shift+M"));

        let first = next_completion(&mut worker);
        let second = next_completion(&mut worker);

        for (completion, expected) in [
            (&first, Action::SelectPenTool),
            (&second, Action::SelectMarkerTool),
        ] {
            match &completion.edit {
                ConfigEdit::Keybinding(write) => assert_eq!(
                    write.request.action, expected,
                    "completions arrive in submission order"
                ),
                other => panic!("expected a shortcut edit, got {other:?}"),
            }
            assert_eq!(
                completion
                    .result
                    .as_ref()
                    .expect("a writable fixture accepts the edit")
                    .write,
                ConfigEditWrite::Wrote
            );
        }

        let reloaded = crate::config::ConfigDocument::load_from_path(&path)
            .expect("the written config reloads");
        assert_eq!(
            reloaded
                .config()
                .keybindings
                .bindings_for_action(Action::SelectPenTool),
            Some(&["Ctrl+Alt+Shift+P".to_string()][..]),
            "the first edit must not be lost to the second"
        );
        assert_eq!(
            reloaded
                .config()
                .keybindings
                .bindings_for_action(Action::SelectMarkerTool),
            Some(&["Ctrl+Alt+Shift+M".to_string()][..])
        );
    });
}

/// An edit made a moment before quitting still reaches the file.
///
/// Teardown drops the queue's sender and waits for the worker to finish what it
/// has; without that wait the process would exit with the write in flight and
/// the user's last gesture would be the one that did not stick.
#[test]
fn shutdown_drains_a_queued_edit_before_returning() {
    with_temp_config_home(|config_root| {
        let path = config_in(config_root);
        let wake = RuntimeWakeSource::new().expect("an eventfd");
        let mut worker = ConfigEditWorker::new(wake.handle());

        worker.submit(save(3, "Quitting"));
        worker.shutdown();

        assert_eq!(
            slot_name(&path, 3).as_deref(),
            Some("Quitting"),
            "the queued write must have landed before shutdown returned"
        );
    });
}

/// A stopped worker does not swallow edits: the gesture still gets exactly one
/// completion, so it degrades with wording instead of vanishing.
#[test]
fn an_edit_submitted_after_shutdown_still_gets_exactly_one_completion() {
    with_temp_config_home(|config_root| {
        let _path = config_in(config_root);
        let wake = RuntimeWakeSource::new().expect("an eventfd");
        let mut worker = ConfigEditWorker::new(wake.handle());

        worker.submit(save(4, "Before"));
        worker.shutdown();

        // Shutdown clears the worker, so this one starts a fresh thread rather
        // than reporting a stop; the honest shape to check is that whichever
        // happens, exactly one completion describes the edit, and it is this
        // edit rather than a leftover.
        worker.submit(save(5, "After"));
        let completion = next_completion(&mut worker);
        assert_eq!(completed_preset_name(&completion), "After");
        assert!(
            worker.try_recv().is_none(),
            "one submission, one completion"
        );
        worker.shutdown();
    });
}

/// The case the staging queue exists for: more gestures than the worker's
/// channel can hold, all of them onto the same slot.
///
/// The first write is stalled behind another process's write lock, so the
/// channel is provably full by the time the later edits arrive. Answering one of
/// those on the spot — as a failed write, because it could not be queued —
/// reports the *newest* gesture before the older ones it was made after: their
/// completions then land on top of it, so the slot keeps an older value while
/// the newest gesture's own toast says it was saved. Every edit here is written,
/// and every completion arrives in the order the gesture was made.
#[test]
fn a_full_queue_answers_edits_in_submission_order_and_keeps_the_newest() {
    with_temp_config_home(|config_root| {
        let path = config_in(config_root);
        let wake = RuntimeWakeSource::new().expect("an eventfd");
        // One slot in the worker's channel, so the third gesture finds it full.
        let mut worker = ConfigEditWorker::with_capacity(wake.handle(), 1);
        let names = ["First", "Second", "Third", "Fourth"];

        let held = hold_config_write_lock(&path);
        for name in names {
            worker.submit(save(1, name));
        }
        assert!(
            worker.staged.len() >= 2,
            "the channel must really be full, or this test is about nothing"
        );
        // Let the stalled write through; everything behind it follows.
        drop(held);

        let completed: Vec<String> = names
            .iter()
            .map(|_| {
                let completion = next_completion(&mut worker);
                completion
                    .result
                    .as_ref()
                    .expect("a writable fixture accepts every edit, queued or staged");
                completed_preset_name(&completion)
            })
            .collect();

        assert_eq!(
            completed, names,
            "completions must arrive in the order the gestures were made"
        );
        assert_eq!(
            slot_name(&path, 1).as_deref(),
            Some("Fourth"),
            "and the newest gesture is the one the file keeps"
        );
        worker.shutdown();
    });
}

/// Teardown drains the staging queue too.
///
/// An edit waiting for room is an edit the user made; quitting is not a reason
/// to drop it. Each of these goes to its own slot, so the file shows exactly
/// which ones reached it.
#[test]
fn shutdown_drains_edits_the_channel_had_no_room_for() {
    with_temp_config_home(|config_root| {
        let path = config_in(config_root);
        let wake = RuntimeWakeSource::new().expect("an eventfd");
        let mut worker = ConfigEditWorker::with_capacity(wake.handle(), 1);
        let staged = [(1, "First"), (2, "Second"), (3, "Third"), (4, "Fourth")];

        let held = hold_config_write_lock(&path);
        for (slot, name) in staged {
            worker.submit(save(slot, name));
        }
        assert!(
            worker.staged.len() >= 2,
            "the channel must really be full, or this test is about nothing"
        );
        drop(held);

        worker.shutdown();

        for (slot, name) in staged {
            assert_eq!(
                slot_name(&path, slot).as_deref(),
                Some(name),
                "an edit still waiting for room must not be lost to teardown"
            );
        }
    });
}

/// The palette's own path for one shortcut gesture, over the fields it needs.
///
/// Through `queue_keybinding_edit` rather than `worker.submit`, because the
/// check that decides whether the gesture is queued at all is the subject: it
/// reads the deltas already outstanding on the worker, and submitting straight
/// past it would test nothing.
fn rebind_through_the_palette(
    keybindings: &crate::config::KeybindingsConfig,
    input: &mut crate::input::state::InputState,
    worker: &mut ConfigEditWorker,
    action: crate::config::Action,
    binding: &str,
) {
    use crate::backend::wayland::state::queue_keybinding_edit;
    use crate::input::state::{KeybindingEditOperation, KeybindingEditRequest};

    queue_keybinding_edit(
        keybindings,
        input,
        worker,
        KeybindingEditRequest {
            action,
            operation: KeybindingEditOperation::Replace(vec![binding.to_string()]),
        },
    );
}

fn written_bindings(path: &Path, action: crate::config::Action) -> Option<Vec<String>> {
    crate::config::ConfigDocument::load_from_path(path)
        .expect("the written config reloads")
        .config()
        .keybindings
        .bindings_for_action(action)
        .map(<[String]>::to_vec)
}

/// The gesture a chord freed by an outstanding edit is made for.
///
/// The palette moves Pen off `F` and, before that write has reported back,
/// binds Marker to `F`. Nothing is installed until a completion arrives, so the
/// running keymap still shows Pen on `F`: checked against it alone the second
/// gesture is refused — "F is already assigned to Pen Tool" — over a claim the
/// file is about to drop, and the refusal is a toast the user may never see,
/// because the overlay can be quitting in the same breath. The file would have
/// taken it: by the time the second write runs, the first is in the file and
/// `F` belongs to nobody.
///
/// Both edits must reach `config.toml`, including through the exit path, where
/// there is no toast left to explain a refusal at all.
#[test]
fn a_chord_an_outstanding_edit_freed_is_rebindable_and_both_edits_reach_the_file() {
    use crate::config::{Action, Config, KeybindingsConfig};

    with_temp_config_home(|config_root| {
        let path = config_in(config_root);
        let wake = RuntimeWakeSource::new().expect("an eventfd");
        let mut worker = ConfigEditWorker::new(wake.handle());
        let mut config = Config::default();
        let mut input = test_input_state();
        let keybindings = KeybindingsConfig::default();
        assert_eq!(
            keybindings.bindings_for_action(Action::SelectPenTool),
            Some(&["F".to_string()][..]),
            "the fixture is the shipped keymap the palette would be reading"
        );

        // The keymap stays exactly as it is across both: a shortcut edit is
        // installed by its completion, and neither has reported back yet.
        rebind_through_the_palette(
            &keybindings,
            &mut input,
            &mut worker,
            Action::SelectPenTool,
            "Ctrl+Alt+Shift+P",
        );
        rebind_through_the_palette(
            &keybindings,
            &mut input,
            &mut worker,
            Action::SelectMarkerTool,
            "F",
        );

        assert!(
            input.toasts_idle(),
            "a gesture the file will accept must not be refused with a warning"
        );

        // The exit path, where a refusal would have had nowhere to go.
        finish_config_edits(&mut config, &mut input, &mut worker);

        assert_eq!(
            written_bindings(&path, Action::SelectPenTool).as_deref(),
            Some(&["Ctrl+Alt+Shift+P".to_string()][..]),
            "the edit that freed the chord must have reached the file"
        );
        assert_eq!(
            written_bindings(&path, Action::SelectMarkerTool).as_deref(),
            Some(&["F".to_string()][..]),
            "and so must the edit that took it"
        );
    });
}

/// The same pair without an exit: the run drains the completions itself.
///
/// Two deliberate gestures in a row are the ordinary case, not a teardown
/// special. Both are answered here through `try_recv`, the way the event loop
/// picks them up, and both are answered as writes that landed.
#[test]
fn a_freed_chord_is_rebindable_between_two_ordinary_drains() {
    use crate::config::{Action, KeybindingsConfig};

    with_temp_config_home(|config_root| {
        let path = config_in(config_root);
        let wake = RuntimeWakeSource::new().expect("an eventfd");
        let mut worker = ConfigEditWorker::new(wake.handle());
        let mut input = test_input_state();
        let keybindings = KeybindingsConfig::default();

        rebind_through_the_palette(
            &keybindings,
            &mut input,
            &mut worker,
            Action::SelectPenTool,
            "Ctrl+Alt+Shift+P",
        );
        rebind_through_the_palette(
            &keybindings,
            &mut input,
            &mut worker,
            Action::SelectMarkerTool,
            "F",
        );
        assert!(input.toasts_idle(), "neither gesture may be refused here");

        for expected in [Action::SelectPenTool, Action::SelectMarkerTool] {
            let completion = next_completion(&mut worker);
            match &completion.edit {
                ConfigEdit::Keybinding(write) => {
                    assert_eq!(write.request.action, expected, "in submission order");
                }
                other => panic!("expected a shortcut edit, got {other:?}"),
            }
            assert_eq!(
                completion
                    .result
                    .as_ref()
                    .expect("the file takes both, in this order")
                    .write,
                ConfigEditWrite::Wrote
            );
        }

        assert!(
            worker.projected_shortcuts().is_empty(),
            "every answered edit gives its projection back"
        );
        assert_eq!(
            written_bindings(&path, Action::SelectMarkerTool).as_deref(),
            Some(&["F".to_string()][..])
        );
        worker.shutdown();
    });
}

/// A projection lasts exactly as long as its edit is unanswered.
///
/// The write here fails — the file cannot be parsed, so the edit is refused
/// before anything is written — and a failed write is still an answer: the run
/// keeps the shortcut in its keymap and the toast says the file missed it. Once
/// that completion is taken, the running keymap is what the next gesture has to
/// be checked against, and it is the caller's business whether the delta went
/// into it. Leaving the projection up would keep the chord this edit gave up
/// looking free on the word of an edit nobody is waiting on any more.
#[test]
fn an_answered_edit_gives_its_projection_back_even_when_the_write_failed() {
    use crate::config::{Action, KeybindingsConfig};

    with_temp_config_home(|config_root| {
        let path = config_in(config_root);
        fs::write(&path, "this is not TOML =\n").expect("an unparseable fixture");
        let wake = RuntimeWakeSource::new().expect("an eventfd");
        let mut worker = ConfigEditWorker::new(wake.handle());
        let mut input = test_input_state();
        let keybindings = KeybindingsConfig::default();

        rebind_through_the_palette(
            &keybindings,
            &mut input,
            &mut worker,
            Action::SelectPenTool,
            "Ctrl+Alt+Shift+P",
        );
        assert_eq!(
            worker.projected_shortcuts().len(),
            1,
            "an unanswered edit is what a projection is for"
        );

        let completion = next_completion(&mut worker);
        assert!(
            completion.result.is_err(),
            "an unparseable config is refused rather than rebuilt"
        );
        assert!(
            worker.projected_shortcuts().is_empty(),
            "and a refusal retires the projection exactly as a landed write does"
        );
        worker.shutdown();
    });
}

/// An input state the way the overlay's own suites build one, so the gestures
/// below are recorded by the real code rather than poked into a field.
fn test_input_state() -> crate::input::state::InputState {
    use crate::config::{Action, BoardsConfig, KeyBinding, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};
    use std::collections::HashMap;

    let mut action_map = HashMap::new();
    action_map.insert(
        KeyBinding::parse("Escape").expect("a chord this test spelled"),
        Action::Exit,
    );
    crate::input::state::InputState::with_defaults(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        3.0,
        12.0,
        EraserMode::Brush,
        0.32,
        false,
        32.0,
        FontDescriptor::default(),
        false,
        20.0,
        30.0,
        false,
        true,
        BoardsConfig::default(),
        action_map,
        usize::MAX,
        ClickHighlightSettings::disabled(),
        0,
        0,
        true,
        0,
        0,
        5,
        5,
        PresenterModeConfig::default(),
    )
}

/// The gestures the overlay can still be holding when it is told to quit, and
/// all of them reach the file.
///
/// One batch of input events can carry a gesture and the exit that follows it —
/// a chord captured with one key press, then Escape — and the loop breaks on the
/// exit before the pass that would have queued the gesture ever runs. Teardown
/// is the last place that can still notice; without it the edit is lost with no
/// error, no toast, and nothing in the file.
///
/// All three kinds are staged, through the same input-state calls their real
/// gestures make, because the inventory is the part that rots: a pending slot
/// whose drain queues a config edit and is not drained here is silently dropped
/// at exit.
#[test]
fn gestures_still_pending_when_the_overlay_quits_reach_the_file() {
    use crate::config::{Action, Config, ConfigDocument};
    use crate::draw::Color;
    use crate::input::state::KeybindingEditOperation;

    const RECOLORED: Color = Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    };

    with_temp_config_home(|config_root| {
        let path = config_in(config_root);
        let wake = RuntimeWakeSource::new().expect("an eventfd");
        let mut worker = ConfigEditWorker::new(wake.handle());
        let mut config = Config::default();
        let mut input = test_input_state();

        // Recorded and never drained: exactly the state the input side is in
        // when the exit arrives in the same batch.
        assert!(
            input.save_preset(1),
            "the preset gesture must record something to write"
        );
        assert!(
            input.open_color_picker_popup_for_quick_color(0),
            "the picker must open on the slot this test recolors"
        );
        input.color_picker_popup_set_color(RECOLORED);
        input.apply_color_picker_popup();
        assert!(
            input.request_keybinding_edit(
                Action::SelectPenTool,
                KeybindingEditOperation::Replace(vec!["Ctrl+Alt+Shift+K".to_string()]),
            ),
            "the shortcut gesture must record something to write"
        );

        // The teardown path, which is all `shutdown_config_edits` performs.
        finish_config_edits(&mut config, &mut input, &mut worker);

        let written = ConfigDocument::load_from_path(&path).expect("the written config reloads");
        assert!(
            written.config().presets.get_slot(1).is_some(),
            "the preset gesture must have reached the file"
        );
        assert_eq!(
            written
                .config()
                .keybindings
                .bindings_for_action(Action::SelectPenTool),
            Some(&["Ctrl+Alt+Shift+K".to_string()][..]),
            "the shortcut captured just before quitting must have reached the file"
        );
        let swatch = written
            .config()
            .drawing
            .quick_colors
            .effective_entries()
            .first()
            .map(|entry| entry.color.to_color())
            .expect("the palette always resolves a first slot");
        assert_eq!(
            crate::config::ColorSpec::from(swatch),
            crate::config::ColorSpec::from(RECOLORED),
            "the accepted recolor must have reached the file"
        );
    });
}

/// The worker wakes the event loop when it finishes, so a completion is applied
/// on the next pass rather than whenever the next unrelated event arrives.
#[test]
fn a_finished_write_wakes_the_event_loop() {
    with_temp_config_home(|config_root| {
        let _path = config_in(config_root);
        let wake = RuntimeWakeSource::new().expect("an eventfd");
        let mut worker = ConfigEditWorker::new(wake.handle());
        // Drain anything the descriptor already carried, so the wait below is
        // about this edit.
        let _ = wake.drain();

        worker.submit(save(6, "Woken"));

        assert!(
            wake.wait_readable(Some(Duration::from_secs(10)))
                .expect("the eventfd this test created"),
            "the loop must be woken by a finished config write"
        );
        worker.shutdown();
    });
}
