//! The shell under GTK: a language change rebuilds the window in place.
//!
//! These checks launch the real component on the repository's private
//! headless compositor (`tools/test-gtk-widgets.sh`) and never on the user's
//! display, so ordinary test runs skip them. They are the spike that chose
//! the rebuild for decision D1, kept as regression tests:
//!
//! - a language-only change, delivered as the selection message itself,
//!   loses no editor, recorder or conflict state, even after the popovers
//!   the old pages owned have closed and every queued message has run, and
//!   an open edit's popover reopens only once its row is back on screen;
//! - focus returns to the control that had it, with its caret and
//!   selection, and the visible page keeps its scroll;
//! - navigation works in the new language (finding N1);
//! - a load that changes the language keeps the resets it always makes;
//! - repeated switches leave no extra pages or bindings behind;
//! - the Language row comes first on the UI tab, with its icon and a title in
//!   every supported language, whichever language is active.

use std::cell::RefCell;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};
use std::{env, fs, iter, thread};

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;

use gtk::glib::{MainContext, SignalHandlerId};
use gtk::graphene::Point;
use gtk::prelude::GtkWindowExt;
use wayscriber::domain::Language;
use wayscriber::{env_vars, i18n};

use crate::messages::{CommandMessage, Message};
use crate::models::{
    KeybindingField, KeybindingsTabId, LanguageOption, StartupRequest, TabId, UiTabId,
    keybinding_tab,
};
use crate::test_temp;

use super::super::pages;
use super::super::state::ConfiguratorApp;

const MARKER: &str = "EXECUTED: configurator GTK language rebuild assertions";
const CHILD_ENV: &str = "WAYSCRIBER_CONFIGURATOR_GTK_CHILD";

#[test]
fn language_changes_rebuild_the_window_in_place() {
    // A window is allowed only on the dedicated test compositor.
    if env::var_os("WAYSCRIBER_REQUIRE_GTK_TESTS").is_none()
        || env::var("WAYLAND_DISPLAY").as_deref() != Ok("wayscriber-widget-tests")
    {
        eprintln!("skipping configurator GTK test: not on the private test compositor");
        return;
    }

    if env::var_os(CHILD_ENV).is_none() {
        run_in_child();
        return;
    }

    run_scenario();
    eprintln!("{MARKER}");
}

/// The window, config and catalogs belong to a fresh process with its own
/// homes, so nothing reads or writes the user's files, no setup tool runs,
/// and the catalogs load from the embedded copies.
fn run_in_child() {
    let homes = test_temp::tempdir().expect("child homes");
    let mut command = Command::new(env::current_exe().expect("test binary"));
    command
        .args([
            concat!(
                module_path!(),
                "::language_changes_rebuild_the_window_in_place"
            )
            .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
            .expect("test path starts with the crate name"),
            "--exact",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(CHILD_ENV, "1")
        .env_remove(env_vars::LANGUAGE_OVERRIDE_ENV);
    for (key, directory) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
        (env_vars::LOCALE_DIR_ENV, "locales"),
        // No setup tool (systemctl, gsettings) is reachable from here.
        ("PATH", "bin"),
    ] {
        let path = homes.path().join(directory);
        fs::create_dir_all(&path).expect("child home");
        command.env(key, path);
    }
    for key in env_vars::LOCALE_ENV_KEYS {
        command.env_remove(key);
    }
    write_config(&homes.path().join("config"), "[ui]\nlanguage = \"en\"\n");

    let status = command.status().expect("run the GTK child");
    assert!(status.success(), "configurator GTK child failed");
}

fn write_config(config_home: &Path, contents: &str) {
    let directory = config_home.join("wayscriber");
    fs::create_dir_all(&directory).expect("config directory");
    fs::write(directory.join("config.toml"), contents).expect("write config");
}

fn run_scenario() {
    adw::init().expect("libadwaita initializes on the test compositor");
    let app = ConfiguratorApp::builder()
        .launch(StartupRequest::default())
        .detach();
    app.widget().present();
    pump_until("the first config load", || {
        !app.model().document.is_loading()
    });
    settle();
    assert_eq!(i18n::language(), Language::English);
    assert_eq!(sidebar_label(&app, TabId::Drawing), "Drawing");
    let pages = app.widgets().stack.observe_children().n_items();
    let bindings = app.widgets().bindings.len();

    open_edits_survive_a_language_change(&app);
    settling_keeps_cancels_and_leave_confirmations(&app);
    focus_caret_and_scroll_survive_a_language_change(&app);
    navigation_works_after_a_language_change(&app);
    repeated_switches_leave_nothing_behind(&app, pages, bindings);
    a_load_that_changes_the_language_keeps_its_resets(&app);
    service_buttons_describe_service_actions_in_each_language(&app);
    the_language_row_leads_the_ui_tab_in_each_language(&app);
    super::rebuild::assert_widget_paths_find_only_the_same_shape();
}

fn the_language_row_leads_the_ui_tab_in_each_language(app: &Controller<ConfiguratorApp>) {
    app.emit(Message::TabSelected(TabId::Ui));
    for (language, title) in [
        (LanguageOption::SimplifiedChinese, "语言 / Language"),
        (LanguageOption::English, "Language / 语言"),
    ] {
        app.emit(Message::UiLanguageChanged(language));
        settle();
        let row = language_row(app).unwrap_or_else(|| panic!("no {title:?} row"));
        assert_eq!(row.title(), title);
        // The first row of General UI, which tops the UI tab.
        let rows = row.parent().expect("the group's row list");
        assert_eq!(
            rows.first_child(),
            Some(row.clone().upcast::<gtk::Widget>()),
            "{title:?}"
        );
        let icons: Vec<_> = descendants(row.upcast())
            .into_iter()
            .filter_map(|widget| widget.downcast::<gtk::Image>().ok())
            .filter_map(|image| image.icon_name().map(|icon| icon.to_string()))
            .collect();
        assert!(
            icons.iter().any(|icon| icon == LanguageOption::ROW_ICON),
            "{title:?} has no globe: {icons:?}"
        );
    }
}

fn service_buttons_describe_service_actions_in_each_language(app: &Controller<ConfiguratorApp>) {
    app.emit(Message::TabSelected(TabId::Daemon));
    pump_until("the background service status", || {
        app.model().daemon.status.is_some()
    });
    settle();

    for (language, start, restart, stop) in [
        (
            LanguageOption::SimplifiedChinese,
            "启动服务",
            "重启",
            "停止并禁用",
        ),
        (
            LanguageOption::English,
            "Start Service",
            "Restart",
            "Stop & Disable",
        ),
    ] {
        app.emit(Message::UiLanguageChanged(language));
        settle();

        for running in [false, true] {
            let mut status = app.model().daemon.status.clone().expect("service status");
            status.service_installed = true;
            status.service_active = running;
            status.service_enabled = running;
            let request_id = app.model().daemon.latest_status_request_id;
            let effects = app
                .state()
                .get_mut()
                .model
                .update_command(CommandMessage::DaemonStatusLoaded(request_id, Ok(status)));
            assert!(effects.is_empty());
            settle();

            // The page hides the Restart/Stop row, not its buttons, so a button
            // counts as shown only when every container up to the page is.
            let page = app.widgets().stack.visible_child().expect("visible page");
            let buttons: Vec<_> = visible_page_descendants(app)
                .into_iter()
                .filter(|widget| {
                    iter::successors(Some(widget.clone()), gtk::Widget::parent)
                        .take_while(|ancestor| *ancestor != page)
                        .all(|ancestor| ancestor.is_visible())
                })
                .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
                .filter_map(|button| button.label().map(|label| label.to_string()))
                .collect();
            let (shown, hidden) = if running {
                (vec![restart, stop], vec![start])
            } else {
                (vec![start], vec![restart, stop])
            };
            for label in shown {
                assert!(
                    buttons.iter().any(|button| button == label),
                    "missing {label:?}: {buttons:?}"
                );
            }
            for label in hidden {
                assert!(
                    !buttons.iter().any(|button| button == label),
                    "unexpected {label:?}: {buttons:?}"
                );
            }
        }
    }
}

fn open_edits_survive_a_language_change(app: &Controller<ConfiguratorApp>) {
    // A recording starts from a click on its row, so the row is on screen;
    // Undo is far down the page.
    app.emit(Message::TabSelected(TabId::Keybindings));
    settle();
    scroll_row_into_view(app, KeybindingField::Undo);
    app.emit(Message::ShortcutRecordingStarted(KeybindingField::Undo));
    settle();
    assert!(app.model().shortcuts.recorder().is_some());

    for language in [
        LanguageOption::SimplifiedChinese,
        LanguageOption::English,
        LanguageOption::SimplifiedChinese,
    ] {
        let openings = switch_language_and_await_reopening(app, language);
        // The rebuilt page starts unscrolled, and a translation moves rows;
        // a recorder opened on a row off screen is one GTK may close.
        assert_eq!(
            openings,
            vec![true],
            "switching to {language:?} must reopen the recorder once, on its row on screen"
        );
        assert_eq!(
            app.model()
                .shortcuts
                .recorder()
                .map(|recorder| recorder.field),
            Some(KeybindingField::Undo),
            "switching to {language:?} canceled the recording"
        );
    }
    assert_eq!(sidebar_label(app, TabId::Drawing), "绘图");
    assert_eq!(app.widgets().save_button.label().as_deref(), Some("保存"));
    // The reopened recorder prompts in the new language.
    let prompt = app
        .model()
        .shortcuts
        .recorder()
        .map(|recorder| recorder.prompt())
        .expect("recorder");
    assert_eq!(prompt, "请按一个快捷键。");
    assert!(
        visible_labels(app).contains(&prompt),
        "the reopened recorder must show {prompt:?}"
    );
    app.emit(Message::ShortcutRecordingCanceled(KeybindingField::Undo));
    settle();
    assert!(app.model().shortcuts.recorder().is_none());

    // An unapplied text edit, then the conflict applying it raises.
    app.emit(Message::ShortcutTextEditStarted(KeybindingField::Undo));
    app.emit(Message::ShortcutTextEditChanged(
        "Ctrl+Z, Bogus+".to_string(),
    ));
    settle();
    // The caret inside the editor's entry survives the editor reopening.
    focused_editable(app).select_region(2, 5);
    let openings = switch_language_and_await_reopening(app, LanguageOption::English);
    assert_eq!(
        openings,
        vec![true],
        "the text editor must reopen once, on its row on screen"
    );
    assert_eq!(
        app.model()
            .shortcuts
            .editor()
            .map(|editor| editor.text.clone()),
        Some("Ctrl+Z, Bogus+".to_string()),
        "the language change dropped the text edit"
    );
    let editable = focused_editable(app);
    assert_eq!(
        editable.text(),
        "Ctrl+Z, Bogus+",
        "focus is in the reopened editor"
    );
    assert_eq!(
        editable.selection_bounds(),
        Some((2, 5)),
        "the reopened editor keeps its selection"
    );
    app.emit(Message::ShortcutTextEditChanged("E".to_string()));
    app.emit(Message::ShortcutTextEditApplied);
    settle();
    assert!(
        app.model().shortcuts.conflict().is_some(),
        "E is Clear Canvas"
    );
    app.emit(Message::UiLanguageChanged(
        LanguageOption::SimplifiedChinese,
    ));
    settle();
    let conflict = app
        .model()
        .shortcuts
        .conflict()
        .map(|conflict| conflict.prompt());
    let conflict = conflict.expect("the language change dropped the conflict");
    assert!(conflict.contains("清空画布"), "{conflict}");
    app.emit(Message::ShortcutConflictCanceled);
    settle();
    assert!(app.model().shortcuts.conflict().is_none());
}

/// Rebuilt pages reopen an open edit a few frames after a language change.
/// What happens in those frames still counts: a cancel stays a cancel, focus
/// kept for a canceled editor does not reach the next edit, and a
/// Leave confirmation asked meanwhile is not withdrawn by the reopening.
fn settling_keeps_cancels_and_leave_confirmations(app: &Controller<ConfiguratorApp>) {
    start_recording_on_screen(app, KeybindingField::Undo);
    let openings = watch_popover_openings(app);
    app.emit(Message::UiLanguageChanged(LanguageOption::English));
    app.emit(Message::ShortcutRecordingCanceled(KeybindingField::Undo));
    await_settled();
    assert!(
        app.model().shortcuts.recorder().is_none(),
        "the cancel made while the pages settled was undone"
    );
    assert_eq!(
        openings.finish(),
        Vec::<bool>::new(),
        "a canceled recording must not reopen"
    );

    // The open recording is unsaved work, so Reload asks first.
    start_recording_on_screen(app, KeybindingField::Undo);
    let openings = watch_popover_openings(app);
    app.emit(Message::UiLanguageChanged(
        LanguageOption::SimplifiedChinese,
    ));
    // A click on Reload comes after the rebuild's own refresh, which may move
    // focus to the edited row and select it.
    run_queued_messages();
    assert!(
        !pages::pages_settled(),
        "Reload must be asked while the pages settle"
    );
    app.emit(Message::ReloadRequested);
    await_settled();
    assert!(
        app.model().pending_leave().is_some(),
        "reopening the recorder withdrew the Leave confirmation"
    );
    assert_eq!(
        openings.seen(),
        0,
        "the recorder must wait while Leave asks"
    );
    app.emit(Message::LeaveCanceled);
    pump_until("the recorder to reopen once Leave is answered", || {
        openings.seen() > 0
    });
    settle();
    assert_eq!(
        openings.finish(),
        vec![true],
        "the recorder must reopen once, on its row on screen"
    );
    assert!(
        app.model().shortcuts.recorder().is_some(),
        "canceling Leave keeps the recording"
    );
    app.emit(Message::ShortcutRecordingCanceled(KeybindingField::Undo));
    settle();

    // Focus kept for an editor whose edit ends before it reopens must not
    // land on the next edit opened there.
    app.emit(Message::TabSelected(TabId::Keybindings));
    settle();
    scroll_row_into_view(app, KeybindingField::Undo);
    app.emit(Message::ShortcutTextEditStarted(KeybindingField::Undo));
    app.emit(Message::ShortcutTextEditChanged(
        "Ctrl+Z, Bogus+".to_string(),
    ));
    settle();
    focused_editable(app).select_region(2, 5);
    app.emit(Message::UiLanguageChanged(LanguageOption::English));
    app.emit(Message::ShortcutTextEditCanceled(KeybindingField::Undo));
    await_settled();
    app.emit(Message::ShortcutTextEditStarted(KeybindingField::Undo));
    pump_until("the next text edit to open", || open_popover(app).is_some());
    settle();
    assert_ne!(
        focused_editable(app).selection_bounds(),
        Some((2, 5)),
        "the canceled editor's selection landed on the next edit"
    );
    app.emit(Message::ShortcutTextEditCanceled(KeybindingField::Undo));
    settle();

    // A second rebuild before the editor reopens keeps the selection the
    // first one saved for it.
    app.emit(Message::TabSelected(TabId::Keybindings));
    settle();
    scroll_row_into_view(app, KeybindingField::Undo);
    app.emit(Message::ShortcutTextEditStarted(KeybindingField::Undo));
    app.emit(Message::ShortcutTextEditChanged(
        "Ctrl+Z, Bogus+".to_string(),
    ));
    settle();
    focused_editable(app).select_region(2, 5);
    app.emit(Message::UiLanguageChanged(
        LanguageOption::SimplifiedChinese,
    ));
    app.emit(Message::UiLanguageChanged(LanguageOption::English));
    await_settled();
    pump_until("the text editor to reopen", || open_popover(app).is_some());
    settle();
    assert_eq!(
        focused_editable(app).selection_bounds(),
        Some((2, 5)),
        "two rebuilds before the editor reopened lost its selection"
    );
    app.emit(Message::ShortcutTextEditCanceled(KeybindingField::Undo));
    settle();

    // An edit whose category is hidden while the pages settle reopens when
    // the category comes back.
    let home = keybinding_tab(KeybindingField::Undo);
    let other = KeybindingsTabId::ALL
        .into_iter()
        .find(|tab| *tab != home)
        .expect("another category");
    start_recording_on_screen(app, KeybindingField::Undo);
    app.emit(Message::UiLanguageChanged(
        LanguageOption::SimplifiedChinese,
    ));
    app.emit(Message::KeybindingsTabSelected(other));
    await_settled();
    assert!(
        app.model().shortcuts.recorder().is_some(),
        "hiding the category ended the recording"
    );
    assert!(
        open_popover(app).is_none(),
        "the recorder must not open on a hidden row"
    );
    app.emit(Message::KeybindingsTabSelected(home));
    pump_until("the recorder to reopen with its category", || {
        open_popover(app).is_some()
    });
    settle();
    app.emit(Message::ShortcutRecordingCanceled(KeybindingField::Undo));
    app.emit(Message::ShortcutManagerShowAll);
    settle();

    // Back to the language the scenarios after this one start from.
    app.emit(Message::UiLanguageChanged(
        LanguageOption::SimplifiedChinese,
    ));
    await_settled();
}

/// The editor popover open on the visible page, if any.
fn open_popover(app: &Controller<ConfiguratorApp>) -> Option<gtk::Popover> {
    visible_page_descendants(app)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Popover>().ok())
        .find(|popover| popover.is_visible())
}

/// Starts recording `field` the way a click does: on its row, on screen.
fn start_recording_on_screen(app: &Controller<ConfiguratorApp>, field: KeybindingField) {
    app.emit(Message::TabSelected(TabId::Keybindings));
    settle();
    scroll_row_into_view(app, field);
    app.emit(Message::ShortcutRecordingStarted(field));
    settle();
    assert!(
        app.model().shortcuts.recorder().is_some(),
        "recording {field:?} started"
    );
}

/// Runs the main loop until the latest rebuild has settled, then a little
/// longer for the refresh that follows it. The queued messages run first:
/// until the language change in them rebuilds the pages, the old pages
/// still count as settled.
fn await_settled() {
    run_queued_messages();
    pump_until("the rebuilt pages to settle", pages::pages_settled);
    settle();
}

/// Runs the queued messages, their refreshes, and the messages those
/// refreshes queue, without waiting for a frame.
fn run_queued_messages() {
    let context = MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
}

fn focus_caret_and_scroll_survive_a_language_change(app: &Controller<ConfiguratorApp>) {
    // The Language row keeps focus after a pick in it.
    app.emit(Message::TabSelected(TabId::Ui));
    settle();
    let row = language_row(app).expect("Language row");
    assert!(row.grab_focus());
    app.emit(Message::UiLanguageChanged(LanguageOption::English));
    settle();
    let focused = focus(app).expect("something has focus");
    let new_row = language_row(app).expect("rebuilt Language row");
    assert!(
        focused == new_row.clone().upcast::<gtk::Widget>() || focused.is_ancestor(&new_row),
        "focus moved to {focused:?}"
    );
    assert_ne!(new_row, row, "the page was rebuilt");
    assert_eq!(new_row.title(), "Language / 语言");

    // An entry keeps its caret and selection.
    app.emit(Message::TabSelected(TabId::Drawing));
    settle();
    let entry = visible_page_descendants(app)
        .into_iter()
        .find_map(|widget| widget.downcast::<adw::EntryRow>().ok())
        .expect("an entry row on the Drawing page");
    entry.set_text("12.5");
    settle();
    assert!(entry.grab_focus());
    entry.select_region(1, 3);
    app.emit(Message::UiLanguageChanged(
        LanguageOption::SimplifiedChinese,
    ));
    settle();
    assert_caret(app, (1, 3), 3, "a forward selection");

    // Selected backwards, the caret sits at the start; Shift+Arrow must keep
    // moving that end after the switch.
    focused_editable(app).select_region(3, 1);
    app.emit(Message::UiLanguageChanged(LanguageOption::English));
    settle();
    assert_caret(app, (1, 3), 1, "a backward selection");

    // The visible page keeps its scroll position.
    let scrolled = scrollable(app).expect("the Drawing page scrolls");
    let target = (scrolled.vadjustment().upper() - scrolled.vadjustment().page_size()).min(150.0);
    assert!(target > 0.0, "the page must be taller than the window");
    scrolled.vadjustment().set_value(target);
    app.emit(Message::UiLanguageChanged(
        LanguageOption::SimplifiedChinese,
    ));
    // The message is queued: let it run before judging the scroll.
    settle();
    assert_ne!(scrollable(app), Some(scrolled), "the page was rebuilt");
    pump_until("the rebuilt page to scroll back", || {
        scrollable(app)
            .is_some_and(|scrolled| (scrolled.vadjustment().value() - target).abs() < 0.5)
    });
}

fn navigation_works_after_a_language_change(app: &Controller<ConfiguratorApp>) {
    app.emit(Message::UiLanguageChanged(
        LanguageOption::SimplifiedChinese,
    ));
    settle();

    // The sidebar, as a click on it reports.
    for tab in [TabId::Capture, TabId::Boards, TabId::Ui] {
        let row = app
            .widgets()
            .sidebar_rows
            .iter()
            .find(|(row_tab, _)| *row_tab == tab)
            .map(|(_, row)| row.clone())
            .expect("sidebar row");
        app.widgets().sidebar.select_row(Some(&row));
        settle();
        assert_eq!(app.model().active_tab, tab);
        assert_eq!(
            app.widgets().stack.visible_child_name().as_deref(),
            Some(tab.stack_name()),
            "{tab:?} did not open in Chinese"
        );
    }

    // A UI sub-tab, as its switcher reports.
    let inner = visible_page_descendants(app)
        .into_iter()
        .find_map(|widget| {
            widget.downcast::<gtk::Stack>().ok().filter(|stack| {
                stack
                    .child_by_name(UiTabId::StatusBar.stack_name())
                    .is_some()
            })
        })
        .expect("the UI sub-page stack");
    inner.set_visible_child_name(UiTabId::StatusBar.stack_name());
    settle();
    assert_eq!(app.model().active_ui_tab, UiTabId::StatusBar);
}

fn repeated_switches_leave_nothing_behind(
    app: &Controller<ConfiguratorApp>,
    pages: u32,
    bindings: usize,
) {
    for language in [
        LanguageOption::English,
        LanguageOption::SimplifiedChinese,
        LanguageOption::English,
        LanguageOption::SimplifiedChinese,
    ] {
        app.emit(Message::UiLanguageChanged(language));
        settle();
        assert_eq!(app.widgets().stack.observe_children().n_items(), pages);
        assert_eq!(app.widgets().bindings.len(), bindings);
    }
}

fn a_load_that_changes_the_language_keeps_its_resets(app: &Controller<ConfiguratorApp>) {
    // Back to what the file says, so the reload is a real language change.
    app.emit(Message::UiLanguageChanged(LanguageOption::English));
    app.emit(Message::TabSelected(TabId::Keybindings));
    settle();
    scroll_row_into_view(app, KeybindingField::Undo);
    app.emit(Message::ShortcutRecordingStarted(KeybindingField::Undo));
    settle();
    assert!(
        app.model().shortcuts.recorder().is_some(),
        "Reload must find an open recording to ask about"
    );
    write_config(
        Path::new(&env::var_os("XDG_CONFIG_HOME").expect("child config home")),
        "[ui]\nlanguage = \"zh-CN\"\n",
    );

    // The open recorder is unsaved work, so Reload asks first; discarding
    // is the user's choice to drop the edit.
    app.emit(Message::ReloadRequested);
    settle();
    app.emit(Message::LeaveDiscardRequested);
    pump_until("the reload", || {
        !app.model().document.is_loading() && i18n::language() == Language::SimplifiedChinese
    });
    settle();

    assert!(
        app.model().shortcuts.recorder().is_none(),
        "a load clears edits"
    );
    assert_eq!(sidebar_label(app, TabId::Drawing), "绘图");
    // The pages are insensitive while a reload runs, so focus has already
    // left them for the chrome by the time the rebuild happens; the rebuild
    // leaves it there, on a control that still exists.
    let focused = focus(app).expect("focus is never lost");
    assert!(focused.is_visible(), "{focused:?}");
    assert_eq!(
        focused.root().map(|root| root.upcast::<gtk::Widget>()),
        Some(app.widget().clone().upcast::<gtk::Widget>())
    );
}

fn sidebar_label(app: &Controller<ConfiguratorApp>, tab: TabId) -> String {
    app.widgets()
        .chrome
        .sidebar_labels()
        .iter()
        .find(|(label_tab, _)| *label_tab == tab)
        .map(|(_, label)| label.label().to_string())
        .expect("sidebar label")
}

fn focused_editable(app: &Controller<ConfiguratorApp>) -> gtk::Editable {
    focus(app)
        .and_then(|widget| widget.downcast::<gtk::Editable>().ok())
        .expect("the caret is in an editable")
}

/// The focused entry after a rebuild: the same text, the same selected
/// range, and the caret at the same end of it.
fn assert_caret(
    app: &Controller<ConfiguratorApp>,
    selection: (i32, i32),
    position: i32,
    what: &str,
) {
    let editable = focused_editable(app);
    assert_eq!(editable.text(), "12.5", "{what}");
    assert_eq!(editable.selection_bounds(), Some(selection), "{what}");
    assert_eq!(editable.position(), position, "{what} keeps its active end");
}

fn focus(app: &Controller<ConfiguratorApp>) -> Option<gtk::Widget> {
    GtkWindowExt::focus(app.widget())
}

fn visible_page_descendants(app: &Controller<ConfiguratorApp>) -> Vec<gtk::Widget> {
    descendants(app.widgets().stack.visible_child().expect("visible page"))
}

/// `root` and every widget under it, hidden ones included.
fn descendants(root: gtk::Widget) -> Vec<gtk::Widget> {
    let mut found = Vec::new();
    let mut pending = vec![root];
    while let Some(widget) = pending.pop() {
        pending.extend(iter::successors(
            widget.first_child(),
            gtk::Widget::next_sibling,
        ));
        found.push(widget);
    }
    found
}

fn visible_labels(app: &Controller<ConfiguratorApp>) -> Vec<String> {
    visible_page_descendants(app)
        .into_iter()
        .filter(|widget| widget.is_visible())
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .map(|label| label.label().to_string())
        .collect()
}

fn language_row(app: &Controller<ConfiguratorApp>) -> Option<adw::ComboRow> {
    let title = LanguageOption::row_title();
    visible_page_descendants(app)
        .into_iter()
        .filter_map(|widget| widget.downcast::<adw::ComboRow>().ok())
        .find(|row| row.title() == title.as_str())
}

fn scrollable(app: &Controller<ConfiguratorApp>) -> Option<gtk::ScrolledWindow> {
    visible_page_descendants(app)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::ScrolledWindow>().ok())
        .find(|scrolled| {
            let adjustment = scrolled.vadjustment();
            adjustment.upper() - adjustment.page_size() > 1.0
        })
}

/// Scrolls the visible page so `field`'s row is on screen, as it is when the
/// user clicks it.
fn scroll_row_into_view(app: &Controller<ConfiguratorApp>, field: KeybindingField) {
    let title = field.label().to_string();
    let row = visible_page_descendants(app)
        .into_iter()
        .filter_map(|widget| widget.downcast::<adw::PreferencesRow>().ok())
        .find(|row| row.title() == title.as_str())
        .unwrap_or_else(|| panic!("no row titled {title:?}"));
    let scrolled = scrollable(app).expect("a scrolled page");
    // The row's offset from the top of the visible area, whatever the scroll.
    let top = row
        .compute_point(&scrolled, &Point::new(0.0, 0.0))
        .expect("the row lies inside the scrolled page");
    let adjustment = scrolled.vadjustment();
    adjustment.set_value(adjustment.value() + f64::from(top.y()));
    // A frame lays the page out at the new offset; frames can be slow here.
    pump_until(&format!("{title:?} to scroll on screen"), || {
        shown_in_viewport(row.upcast_ref())
    });
}

/// Whether `widget` has a layout and overlaps the visible part of the
/// scrolled page around it.
fn shown_in_viewport(widget: &gtk::Widget) -> bool {
    let Some(scrolled) = widget.ancestor(gtk::ScrolledWindow::static_type()) else {
        return widget.height() > 0;
    };
    let Some(bounds) = widget.compute_bounds(&scrolled) else {
        return false;
    };
    widget.height() > 0
        && bounds.y() + bounds.height() > 0.0
        && f64::from(bounds.y()) < f64::from(scrolled.height())
}

/// Records, for every popover the next rebuilt page opens, whether its row
/// was on screen at that moment. Listeners attach when the rebuilt page
/// becomes the visible one, which is before its first refresh writes the
/// model into it, so a popover opened by that refresh is seen too.
struct PopoverOpenings {
    stack: gtk::Stack,
    handler: SignalHandlerId,
    openings: Rc<RefCell<Vec<bool>>>,
}

impl PopoverOpenings {
    fn seen(&self) -> usize {
        self.openings.borrow().len()
    }

    fn finish(self) -> Vec<bool> {
        self.stack.disconnect(self.handler);
        self.openings.take()
    }
}

/// Switches the language with an edit open and waits until the rebuilt page
/// reopens the edit's popover, which happens a few frames after the rebuild.
/// Returns whether each popover the rebuilt page opened was on screen.
fn switch_language_and_await_reopening(
    app: &Controller<ConfiguratorApp>,
    language: LanguageOption,
) -> Vec<bool> {
    let openings = watch_popover_openings(app);
    app.emit(Message::UiLanguageChanged(language));
    pump_until("the rebuilt page to reopen the open edit", || {
        openings.seen() > 0
    });
    settle();
    openings.finish()
}

fn watch_popover_openings(app: &Controller<ConfiguratorApp>) -> PopoverOpenings {
    let stack = app.widgets().stack.clone();
    let openings = Rc::new(RefCell::new(Vec::new()));
    let watched = Rc::new(RefCell::new(Vec::<gtk::Popover>::new()));
    let handler = {
        let openings = openings.clone();
        stack.connect_visible_child_notify(move |stack| {
            let Some(page) = stack.visible_child() else {
                return;
            };
            let popovers = descendants(page)
                .into_iter()
                .filter_map(|widget| widget.downcast::<gtk::Popover>().ok());
            for popover in popovers {
                if watched.borrow().contains(&popover) {
                    continue;
                }
                watched.borrow_mut().push(popover.clone());
                let openings = openings.clone();
                popover.connect_visible_notify(move |popover| {
                    if !popover.is_visible() {
                        return;
                    }
                    let shown = popover.parent().is_some_and(|row| shown_in_viewport(&row));
                    openings.borrow_mut().push(shown);
                });
            }
        })
    };

    PopoverOpenings {
        stack,
        handler,
        openings,
    }
}

/// Runs the main loop until `done`, failing after a generous timeout.
fn pump_until(what: &str, mut done: impl FnMut() -> bool) {
    let context = MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        while context.pending() {
            context.iteration(false);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

/// Lets queued messages, refreshes and a few frames run.
fn settle() {
    let context = MainContext::default();
    let until = Instant::now() + Duration::from_millis(300);
    while Instant::now() < until {
        while context.pending() {
            context.iteration(false);
        }
        thread::sleep(Duration::from_millis(5));
    }
}
