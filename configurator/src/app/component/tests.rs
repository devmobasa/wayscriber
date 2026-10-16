//! The shell under GTK: a language change rebuilds the window in place.
//!
//! These checks launch the real component on the repository's private
//! headless compositor (`tools/test-gtk-widgets.sh`) and never on the user's
//! display, so ordinary test runs skip them. They are the spike that chose
//! the rebuild for decision D1, kept as regression tests:
//!
//! - a language-only change, delivered as the selection message itself,
//!   loses no editor, recorder or conflict state, even after the popovers
//!   the old pages owned have closed and every queued message has run;
//! - focus returns to the control that had it, with its caret and
//!   selection, and the visible page keeps its scroll;
//! - navigation works in the new language (finding N1);
//! - a load that changes the language keeps the resets it always makes;
//! - repeated switches leave no extra pages or bindings behind.

use std::path::Path;
use std::time::{Duration, Instant};

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;

use wayscriber::domain::Language;

use crate::messages::Message;
use crate::models::{KeybindingField, LanguageOption, StartupRequest, TabId, UiTabId};

use super::super::state::ConfiguratorApp;

const MARKER: &str = "EXECUTED: configurator GTK language rebuild assertions";
const CHILD_ENV: &str = "WAYSCRIBER_CONFIGURATOR_GTK_CHILD";

#[test]
fn language_changes_rebuild_the_window_in_place() {
    // A window is allowed only on the dedicated test compositor.
    if std::env::var_os("WAYSCRIBER_REQUIRE_GTK_TESTS").is_none()
        || std::env::var("WAYLAND_DISPLAY").as_deref() != Ok("wayscriber-widget-tests")
    {
        eprintln!("skipping configurator GTK test: not on the private test compositor");
        return;
    }

    if std::env::var_os(CHILD_ENV).is_none() {
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
    let homes = crate::test_temp::tempdir().expect("child homes");
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
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
        .env_remove(wayscriber::env_vars::LANGUAGE_OVERRIDE_ENV);
    for (key, directory) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
        (wayscriber::env_vars::LOCALE_DIR_ENV, "locales"),
        // No setup tool (systemctl, gsettings) is reachable from here.
        ("PATH", "bin"),
    ] {
        let path = homes.path().join(directory);
        std::fs::create_dir_all(&path).expect("child home");
        command.env(key, path);
    }
    for key in wayscriber::env_vars::LOCALE_ENV_KEYS {
        command.env_remove(key);
    }
    write_config(&homes.path().join("config"), "[ui]\nlanguage = \"en\"\n");

    let status = command.status().expect("run the GTK child");
    assert!(status.success(), "configurator GTK child failed");
}

fn write_config(config_home: &Path, contents: &str) {
    let directory = config_home.join("wayscriber");
    std::fs::create_dir_all(&directory).expect("config directory");
    std::fs::write(directory.join("config.toml"), contents).expect("write config");
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
    assert_eq!(wayscriber::i18n::language(), Language::English);
    assert_eq!(sidebar_label(&app, TabId::Drawing), "Drawing");
    let pages = app.widgets().stack.observe_children().n_items();
    let bindings = app.widgets().bindings.len();

    open_edits_survive_a_language_change(&app);
    focus_caret_and_scroll_survive_a_language_change(&app);
    navigation_works_after_a_language_change(&app);
    repeated_switches_leave_nothing_behind(&app, pages, bindings);
    a_load_that_changes_the_language_keeps_its_resets(&app);
    super::rebuild::assert_widget_paths_find_only_the_same_shape();
}

fn open_edits_survive_a_language_change(app: &Controller<ConfiguratorApp>) {
    app.emit(Message::TabSelected(TabId::Keybindings));
    app.emit(Message::ShortcutRecordingStarted(KeybindingField::Undo));
    settle();
    assert!(app.model().shortcuts.recorder().is_some());

    for language in [
        LanguageOption::SimplifiedChinese,
        LanguageOption::English,
        LanguageOption::SimplifiedChinese,
    ] {
        app.emit(Message::UiLanguageChanged(language));
        settle();
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
    app.emit(Message::UiLanguageChanged(LanguageOption::English));
    settle();
    assert_eq!(
        app.model()
            .shortcuts
            .editor()
            .map(|editor| editor.text.clone()),
        Some("Ctrl+Z, Bogus+".to_string()),
        "the language change dropped the text edit"
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
    assert_eq!(new_row.title(), "Language");

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
    app.emit(Message::ShortcutRecordingStarted(KeybindingField::Undo));
    settle();
    write_config(
        Path::new(&std::env::var_os("XDG_CONFIG_HOME").expect("child config home")),
        "[ui]\nlanguage = \"zh-CN\"\n",
    );

    // The open recorder is unsaved work, so Reload asks first; discarding
    // is the user's choice to drop the edit.
    app.emit(Message::ReloadRequested);
    settle();
    app.emit(Message::LeaveDiscardRequested);
    pump_until("the reload", || {
        !app.model().document.is_loading()
            && wayscriber::i18n::language() == Language::SimplifiedChinese
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
    gtk::prelude::GtkWindowExt::focus(app.widget())
}

fn visible_page_descendants(app: &Controller<ConfiguratorApp>) -> Vec<gtk::Widget> {
    let page = app.widgets().stack.visible_child().expect("visible page");
    let mut found = Vec::new();
    let mut pending = vec![page];
    while let Some(widget) = pending.pop() {
        pending.extend(std::iter::successors(
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
    let title = wayscriber::i18n::tr("Language");
    visible_page_descendants(app)
        .into_iter()
        .filter_map(|widget| widget.downcast::<adw::ComboRow>().ok())
        .find(|row| row.title() == title)
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

/// Runs the main loop until `done`, failing after a generous timeout.
fn pump_until(what: &str, mut done: impl FnMut() -> bool) {
    let context = gtk::glib::MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        while context.pending() {
            context.iteration(false);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Lets queued messages, refreshes and a few frames run.
fn settle() {
    let context = gtk::glib::MainContext::default();
    let until = Instant::now() + Duration::from_millis(300);
    while Instant::now() < until {
        while context.pending() {
            context.iteration(false);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
