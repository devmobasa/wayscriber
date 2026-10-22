//! The standalone About dialog (`wayscriber --about`).
//!
//! Runs its own small Wayland client rather than borrowing the annotation
//! overlay's backend: the dialog is a plain toplevel with its own chrome,
//! sized to its content and painted from the shared theme tokens.

use std::ffi::OsString;
use std::thread::JoinHandle;

use anyhow::{Context, Result};
use log::debug;
use smithay_client_toolkit::compositor::CompositorState;
use smithay_client_toolkit::output::OutputState;
use smithay_client_toolkit::registry::RegistryState;
use smithay_client_toolkit::seat::SeatState;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::xdg::XdgShell;
use smithay_client_toolkit::shell::xdg::window::{Window, WindowDecorations};
use smithay_client_toolkit::shm::Shm;
use smithay_client_toolkit::shm::slot::SlotPool;
use wayland_client::Connection;
use wayland_client::globals::registry_queue_init;

use crate::app_id::runtime_app_id;
use crate::config::Config;
use crate::domain::Language;
use crate::i18n;
use crate::i18n::tr;
use crate::ui::theme::Theme;
use crate::ui_text::UiTextEngine;

mod clipboard;
mod content;
mod diagnostics;
mod handlers;
mod icon;
mod interaction;
mod layout;
mod render;
mod state;

use content::{AboutContent, UpdateState};
use interaction::Element;
use layout::Plan;
use smithay_client_toolkit::seat::pointer::{PointerData, ThemedPointer};

pub fn run_about_window() -> Result<()> {
    let (theme, language) = chrome_settings();
    // This is its own process, so it selects its own language before any of
    // its text is built.
    i18n::initialize(language);

    let conn = Connection::connect_to_env().context("Failed to connect to Wayland compositor")?;
    let (globals, mut event_queue) =
        registry_queue_init(&conn).context("Failed to initialize Wayland registry")?;
    let qh = event_queue.handle();

    let compositor_state =
        CompositorState::bind(&globals, &qh).context("wl_compositor not available")?;
    let shm = Shm::bind(&globals, &qh).context("wl_shm not available")?;
    let xdg_shell = XdgShell::bind(&globals, &qh).context("xdg-shell not available")?;
    let output_state = OutputState::new(&globals, &qh);
    let seat_state = SeatState::new(&globals, &qh);
    let registry_state = RegistryState::new(&globals);

    let content = AboutContent::build();
    let plan = layout::plan(&content);
    let (width, height) = surface_size(&plan);

    let wl_surface = compositor_state.create_surface(&qh);
    let window = xdg_shell.create_window(wl_surface, WindowDecorations::None, &qh);
    window.set_title(tr("About Wayscriber"));
    window.set_app_id(runtime_app_id());
    // The dialog is content-sized; letting the compositor resize it would only
    // clip rows or leave dead space.
    window.set_min_size(Some((width, height)));
    window.set_max_size(Some((width, height)));
    window.commit();

    let mut state = AboutWindowState::new(
        registry_state,
        compositor_state,
        shm,
        output_state,
        seat_state,
        xdg_shell,
        window,
        content,
        plan,
        theme,
    );

    // Join helpers on every return path so ProcessBrokerGuard teardown cannot
    // cancel an in-flight Report/open/copy that already showed a success notice.
    let result = run_about_event_loop(&conn, &mut event_queue, &mut state);
    state.join_helper_workers();
    result
}

fn run_about_event_loop(
    conn: &Connection,
    event_queue: &mut wayland_client::EventQueue<AboutWindowState>,
    state: &mut AboutWindowState,
) -> Result<()> {
    loop {
        event_queue.blocking_dispatch(state)?;
        if state.should_exit {
            break;
        }
        if state.needs_redraw {
            state.render()?;
        }
        if state.check_requested {
            state.check_requested = false;
            // Show the "Checking…" frame before blocking on the network, then
            // repaint with the verdict. A few seconds of a frozen dialog is
            // preferable to threading an event source into this tiny loop.
            conn.flush().context("Failed to flush Wayland connection")?;
            state.run_update_check();
            state.render()?;
        }
    }

    Ok(())
}

/// The arguments that start the About window: `--about`, and the `--language`
/// this process runs with, if any, so the window matches the one that opened
/// it.
pub(crate) fn launch_arguments() -> Vec<OsString> {
    launch_arguments_for(i18n::command_line_language())
}

fn launch_arguments_for(language: Option<Language>) -> Vec<OsString> {
    let mut arguments = vec!["--about".into()];
    if let Some(language) = language {
        arguments.push("--language".into());
        arguments.push(language.code().into());
    }
    arguments
}

/// What the dialog reads from the config before it draws: the theme, from
/// the same `[ui] theme` key as the overlay so the dialog matches the toolbars
/// the user already sees, and the UI language. A config that cannot be read
/// falls back to the defaults for both.
fn chrome_settings() -> (Theme, Language) {
    let ui = match Config::load() {
        Ok(loaded) => loaded.config.ui,
        Err(err) => {
            debug!("About dialog falling back to the default theme and language: {err}");
            Config::default().ui
        }
    };

    (Theme::resolve(ui.theme.to_theme_mode()), ui.language)
}

/// Logical surface size for a plan, rounded up so nothing is clipped by a
/// fractional pixel.
fn surface_size(plan: &Plan) -> (u32, u32) {
    (
        plan.width.ceil().max(1.0) as u32,
        plan.height.ceil().max(1.0) as u32,
    )
}

struct AboutWindowState {
    ui_text: UiTextEngine,
    theme: Theme,
    registry_state: RegistryState,
    compositor_state: CompositorState,
    shm: Shm,
    output_state: OutputState,
    seat_state: SeatState,
    #[allow(dead_code)]
    xdg_shell: XdgShell,
    window: Window,
    pool: Option<SlotPool>,
    width: u32,
    height: u32,
    scale: i32,
    configured: bool,
    should_exit: bool,
    needs_redraw: bool,
    /// Set when the update card is activated; serviced by the event loop so the
    /// blocking fetch never runs inside a protocol handler.
    check_requested: bool,
    /// Open/copy workers that must finish before the process broker shuts down.
    helper_workers: Vec<JoinHandle<()>>,
    content: AboutContent,
    plan: Plan,
    update: UpdateState,
    /// Focus order for the current update state.
    elements: Vec<Element>,
    hover: Option<Element>,
    focus: Option<Element>,
    /// Live Shift state, so Shift-Tab works on layouts that send a plain `Tab`.
    shift_held: bool,
    notice: Option<String>,
    icon: Option<cairo::ImageSurface>,
    themed_pointer: Option<ThemedPointer<PointerData>>,
}

#[cfg(test)]
mod tests {
    use std::{fs, iter};

    use crate::cli::{Cli, CliOutcome};
    use crate::test_env;

    use super::*;
    use crate::config::test_helpers::with_temp_config_home;
    use crate::domain::Language;

    fn write_config(contents: &str) {
        let path = Config::config_path().expect("config path");
        fs::create_dir_all(path.parent().expect("config dir")).expect("create config dir");
        fs::write(path, contents).expect("write config");
    }

    #[test]
    fn the_window_is_opened_with_the_openers_language() {
        assert_eq!(launch_arguments_for(None), ["--about"]);
        assert_eq!(
            launch_arguments_for(Some(Language::SimplifiedChinese)),
            ["--about", "--language", "zh-CN"]
        );
        // The arguments it is given parse back to the same language.
        let parsed = Cli::try_parse_from(
            iter::once("wayscriber".into())
                .chain(launch_arguments_for(Some(Language::SimplifiedChinese))),
        );
        assert!(
            matches!(
                parsed,
                Ok(CliOutcome::Run(ref cli))
                    if cli.about && cli.language == Some(Language::SimplifiedChinese)
            ),
            "{parsed:?}"
        );
    }

    #[test]
    fn the_dialog_reads_its_language_with_its_theme() {
        with_temp_config_home(|_| {
            write_config("[ui]\nlanguage = \"zh-CN\"\n");
            assert_eq!(chrome_settings().1, Language::SimplifiedChinese);

            write_config("[ui\n");
            assert_eq!(chrome_settings().1, Language::Auto);
        });
    }

    #[test]
    fn the_dialog_text_is_built_in_the_saved_language() {
        test_env::in_language_isolated_child(
            concat!(
                module_path!(),
                "::the_dialog_text_is_built_in_the_saved_language"
            ),
            || {
                with_temp_config_home(|_| {
                    write_config("[ui]\nlanguage = \"zh-CN\"\n");

                    // What `run_about_window` does before it builds content.
                    let (_theme, language) = chrome_settings();
                    i18n::initialize(language);
                    let content = AboutContent::build();

                    let labels: Vec<&str> = content
                        .buttons()
                        .iter()
                        .map(|button| button.label)
                        .collect();
                    assert!(labels.contains(&"复制诊断信息"), "{labels:?}");
                    assert_eq!(i18n::language(), Language::SimplifiedChinese);
                });
            },
        );
    }
}
