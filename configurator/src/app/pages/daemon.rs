//! Daemon page: background service, shortcut, and light-control setup.
//!
//! The one page that is a setup wizard rather than a list of preferences,
//! so its rows are custom widgets inside `AdwPreferencesGroup`s instead of
//! `PageBuilder` rows. Structure is built once; every label, button state,
//! and section visibility is a binding that reads the same
//! `DaemonRuntimeStatus` fields the Iced view read, and every button sends
//! the `DaemonAction` that view sent.

use relm4::prelude::*;
use relm4::{adw, gtk};

use adw::prelude::*;

use crate::messages::Message;
use crate::models::{
    DaemonAction, DaemonRuntimeStatus, LightShortcutApplyCapability, ShortcutApplyCapability, TabId,
};

use super::super::search::{AppSearchSummary, SearchArea};
use super::super::state::ConfiguratorApp;
use super::{Binding, BuiltPage, set_text_blocked};

pub(super) fn build(sender: &ComponentSender<ConfiguratorApp>) -> BuiltPage {
    let page = adw::PreferencesPage::new();
    let mut bindings: Vec<Binding> = Vec::new();

    page.add(&overview_group(sender, &mut bindings));

    for section in DaemonSection::ALL {
        let group = match section {
            DaemonSection::Install => install_group(sender, &mut bindings),
            DaemonSection::Shortcut => shortcut_group(sender, &mut bindings),
            DaemonSection::LightControls => light_controls_group(sender, &mut bindings),
            DaemonSection::Start => start_group(sender, &mut bindings),
            DaemonSection::TechnicalDetails => details_group(sender, &mut bindings),
        };
        page.add(&group);
        bindings.push(Box::new(move |app, summary| {
            // The Iced view answered a status load with the details section
            // alone: until the environment is known there is nothing for the
            // setup steps to say.
            let visible = daemon_section_visible(section, shown_areas(summary))
                && (section == DaemonSection::TechnicalDetails || app.daemon_status.is_some());
            set_visible(&group, visible);
        }));
    }

    BuiltPage {
        widget: page.upcast(),
        bindings,
    }
}

// ---- Sections ----------------------------------------------------------

/// The setup steps, in the order the Iced view pushed them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DaemonSection {
    Install,
    Shortcut,
    LightControls,
    Start,
    TechnicalDetails,
}

impl DaemonSection {
    const ALL: [Self; 5] = [
        Self::Install,
        Self::Shortcut,
        Self::LightControls,
        Self::Start,
        Self::TechnicalDetails,
    ];
}

/// Which daemon search areas the current query left on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ShownAreas {
    status: bool,
    service: bool,
    shortcut: bool,
    light: bool,
}

fn daemon_section_visible(section: DaemonSection, shown: ShownAreas) -> bool {
    match section {
        DaemonSection::Install | DaemonSection::Start => shown.service,
        DaemonSection::Shortcut => shown.shortcut,
        DaemonSection::LightControls => shown.light,
        DaemonSection::TechnicalDetails => shown.status || shown.service,
    }
}

fn shown_areas(summary: &AppSearchSummary) -> ShownAreas {
    let matches = |area| {
        !summary.is_active()
            || summary
                .tab(TabId::Daemon)
                .is_some_and(|tab| tab.area_matches(area))
    };
    ShownAreas {
        status: matches(SearchArea::DaemonStatus),
        service: matches(SearchArea::DaemonService),
        shortcut: matches(SearchArea::DaemonShortcut),
        light: matches(SearchArea::DaemonLightControls),
    }
}

// ---- Groups ------------------------------------------------------------

fn overview_group(
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Background Mode")
        .description("Run wayscriber in the background and toggle it with a keyboard shortcut.")
        .build();

    let body = column_box();
    let status_row = row_box();
    let status_label = body_label("");
    status_label.add_css_class("heading");
    let refresh = action_button("Refresh", DaemonAction::RefreshStatus, sender);
    status_row.append(&status_label);
    status_row.append(&refresh);
    body.append(&status_row);

    let feedback_label = body_label("");
    body.append(&feedback_label);
    let busy_label = caption_label("Working...");
    body.append(&busy_label);
    let loading_label = hint_label("Checking your system and background service status...");
    body.append(&loading_label);
    group.add(&body);

    bindings.push(Box::new(move |app, summary| {
        let (text, tone) = overall_status(app.daemon_status.as_ref());
        set_label(&status_label, text);
        apply_tone(&status_label, tone);
        set_visible(&status_row, shown_areas(summary).status);
        set_sensitive(&refresh, !app.daemon_busy);

        match app.daemon_feedback.as_deref() {
            Some(feedback) => {
                set_label(&feedback_label, feedback);
                apply_tone(&feedback_label, feedback_tone(feedback));
                set_visible(&feedback_label, true);
            }
            None => set_visible(&feedback_label, false),
        }

        set_visible(&busy_label, app.daemon_busy);
        set_visible(&loading_label, app.daemon_status.is_none());
    }));

    group
}

fn install_group(
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Step 1 \u{2014} Install the service")
        .description("Install wayscriber as a background service.")
        .build();

    let row = row_box();
    let state_label = body_label("");
    let install = action_button(
        install_button_label(false),
        DaemonAction::InstallOrUpdateService,
        sender,
    );
    row.append(&state_label);
    row.append(&install);
    group.add(&row);

    bindings.push(Box::new(move |app, _summary| {
        let installed = service_installed(app);
        let (text, tone) = install_status(installed);
        set_label(&state_label, text);
        apply_tone(&state_label, tone);
        set_button_label(&install, install_button_label(installed));
        set_sensitive(&install, !app.daemon_busy);
    }));

    group
}

fn shortcut_group(
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Step 2 \u{2014} Set your shortcut")
        .build();

    let locked_label = hint_label("Install the background service first, then set your shortcut.");
    group.add(&locked_label);

    let body = column_box();
    body.append(&body_label(
        "Choose a keyboard shortcut to toggle drawing on/off.",
    ));
    body.append(&hint_label(
        "The shortcut takes effect after the background service is installed and running.",
    ));
    let configured_label = hint_label("");
    body.append(&configured_label);
    let manual_label = warning_label(
        "Automatic shortcut setup is unavailable here. Add a manual keybind for `wayscriber --daemon-toggle`.",
    );
    body.append(&manual_label);

    let entry = gtk::Entry::builder().hexpand(true).build();
    let entry_handler = {
        let sender = sender.clone();
        entry.connect_changed(move |entry| {
            sender.input(Message::DaemonShortcutInputChanged(
                entry.text().to_string(),
            ));
        })
    };
    body.append(&entry);

    let apply = action_button("Apply Shortcut", DaemonAction::ApplyShortcut, sender);
    apply.add_css_class("suggested-action");
    body.append(&apply);
    group.add(&body);

    bindings.push(Box::new(move |app, _summary| {
        let installed = service_installed(app);
        set_visible(&locked_label, !installed);
        set_visible(&body, installed);

        let capability = app
            .daemon_status
            .as_ref()
            .map(|status| status.shortcut_apply_capability);
        let placeholder = shortcut_placeholder(capability);
        if entry.placeholder_text().as_deref() != Some(placeholder) {
            entry.set_placeholder_text(Some(placeholder));
        }
        set_text_blocked(&entry, &entry_handler, &app.daemon_shortcut_input);

        match app
            .daemon_status
            .as_ref()
            .and_then(|status| status.configured_shortcut.as_deref())
        {
            Some(configured) => {
                set_label(
                    &configured_label,
                    &format!("Current shortcut: {configured}"),
                );
                set_visible(&configured_label, true);
            }
            None => set_visible(&configured_label, false),
        }

        let manual = capability == Some(ShortcutApplyCapability::Manual);
        set_visible(&manual_label, manual);
        set_sensitive(&apply, !app.daemon_busy && !manual);
    }));

    group
}

fn light_controls_group(
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Light passthrough controls")
        .description("Install global controls for light passthrough and quick drawing.")
        .build();

    let path_label = hint_label("");
    group.add(&path_label);

    let native_body = column_box();
    let service_warning = warning_label(
        "Install the background service first so these bindings have a daemon to control.",
    );
    native_body.append(&service_warning);
    let native_row = row_box();
    let state_label = body_label("");
    let install = action_button(
        "Install Hyprland Light Controls",
        DaemonAction::ApplyLightControls,
        sender,
    );
    install.add_css_class("suggested-action");
    native_row.append(&state_label);
    native_row.append(&install);
    native_body.append(&native_row);
    group.add(&native_body);

    let manual_label = warning_label(
        "Automatic light controls setup is unavailable here. Add compositor bindings for `wayscriber --light-toggle` and `wayscriber --light-draw-toggle`.",
    );
    group.add(&manual_label);

    bindings.push(Box::new(move |app, _summary| {
        let status = app.daemon_status.as_ref();
        match status.and_then(|status| status.light_controls_config_path.as_deref()) {
            Some(path) => {
                set_label(&path_label, &format!("Hyprland include: {path}"));
                set_visible(&path_label, true);
            }
            None => set_visible(&path_label, false),
        }

        let capability = status
            .map(|status| status.light_shortcut_apply_capability)
            .unwrap_or(LightShortcutApplyCapability::Manual);
        let native = capability == LightShortcutApplyCapability::HyprlandNative;
        set_visible(&native_body, native);
        set_visible(&manual_label, !native);

        let installed = service_installed(app);
        set_visible(&service_warning, !installed);
        let (text, tone) =
            light_controls_status(status.is_some_and(|status| status.light_controls_configured));
        set_label(&state_label, text);
        apply_tone(&state_label, tone);
        set_sensitive(&install, !app.daemon_busy && installed);
    }));

    group
}

fn start_group(
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Step 3 \u{2014} Start the service")
        .build();

    let locked_label = hint_label("Install the background service first.");
    group.add(&locked_label);

    let body = column_box();
    body.append(&body_label("Enable and start the background service."));
    let state_label = body_label("");
    body.append(&state_label);

    let running_row = row_box();
    let restart = action_button("Restart", DaemonAction::RestartService, sender);
    let stop = action_button(
        "Stop & Disable",
        DaemonAction::StopAndDisableService,
        sender,
    );
    running_row.append(&restart);
    running_row.append(&stop);
    body.append(&running_row);

    let start = action_button("Start", DaemonAction::EnableAndStartService, sender);
    start.add_css_class("suggested-action");
    body.append(&start);
    group.add(&body);

    bindings.push(Box::new(move |app, _summary| {
        let installed = service_installed(app);
        set_visible(&locked_label, !installed);
        set_visible(&body, installed);

        let status = app.daemon_status.as_ref();
        let running = status.is_some_and(|status| status.service_active);
        let enabled = status.is_some_and(|status| status.service_enabled);
        let (text, tone) = service_status(running, enabled);
        set_label(&state_label, text);
        apply_tone(&state_label, tone);

        set_visible(&running_row, running);
        set_visible(&start, !running);
        for button in [&restart, &stop, &start] {
            set_sensitive(button, !app.daemon_busy);
        }
    }));

    group
}

fn details_group(
    sender: &ComponentSender<ConfiguratorApp>,
    bindings: &mut Vec<Binding>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("Details").build();

    let body = column_box();
    let detecting_label = hint_label("Detecting environment...");
    let desktop_label = hint_label("");
    let backend_label = hint_label("");
    let shortcut_capability_label = hint_label("");
    let light_capability_label = hint_label("");
    let service_file_label = hint_label("");
    let light_controls_label = hint_label("");
    let missing_tools_label = warning_label("");
    for label in [
        &detecting_label,
        &desktop_label,
        &backend_label,
        &shortcut_capability_label,
        &light_capability_label,
        &service_file_label,
        &light_controls_label,
        &missing_tools_label,
    ] {
        body.append(label);
    }

    let refresh = action_button("Refresh", DaemonAction::RefreshStatus, sender);
    body.append(&refresh);
    group.add(&body);

    bindings.push(Box::new(move |app, _summary| {
        set_sensitive(&refresh, !app.daemon_busy);
        let Some(status) = app.daemon_status.as_ref() else {
            set_visible(&detecting_label, true);
            for label in [
                &desktop_label,
                &backend_label,
                &shortcut_capability_label,
                &light_capability_label,
                &service_file_label,
                &light_controls_label,
                &missing_tools_label,
            ] {
                set_visible(label, false);
            }
            return;
        };

        set_visible(&detecting_label, false);
        for (label, text) in [
            (
                &desktop_label,
                format!("Desktop: {}", status.desktop.label()),
            ),
            (
                &backend_label,
                status.shortcut_backend.friendly_label().to_string(),
            ),
            (
                &shortcut_capability_label,
                status
                    .shortcut_apply_capability
                    .friendly_label()
                    .to_string(),
            ),
            (
                &light_capability_label,
                status
                    .light_shortcut_apply_capability
                    .friendly_label()
                    .to_string(),
            ),
        ] {
            set_label(label, &text);
            set_visible(label, true);
        }

        for (label, text) in [
            (
                &service_file_label,
                status
                    .service_unit_path
                    .as_deref()
                    .map(|path| format!("Service file: {path}")),
            ),
            (&light_controls_label, light_controls_details(status)),
            (&missing_tools_label, missing_tools(status)),
        ] {
            match text {
                Some(text) => {
                    set_label(label, &text);
                    set_visible(label, true);
                }
                None => set_visible(label, false),
            }
        }
    }));

    group
}

// ---- Status wording ----------------------------------------------------

/// Emphasis for a status line, mapped to the Adwaita state classes that
/// stand in for the Iced view's literal colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    Neutral,
    Positive,
    Caution,
    Negative,
}

impl Tone {
    const CLASSES: [&'static str; 4] = ["dim-label", "success", "warning", "error"];

    fn css_class(self) -> &'static str {
        match self {
            Self::Neutral => "dim-label",
            Self::Positive => "success",
            Self::Caution => "warning",
            Self::Negative => "error",
        }
    }
}

fn overall_status(status: Option<&DaemonRuntimeStatus>) -> (&'static str, Tone) {
    match status {
        None => ("Status: Detecting...", Tone::Neutral),
        Some(status) if status.service_active => ("Status: Running", Tone::Positive),
        Some(status) if status.service_installed => {
            ("Status: Installed, not running", Tone::Caution)
        }
        Some(_) => ("Status: Not installed", Tone::Neutral),
    }
}

fn feedback_tone(feedback: &str) -> Tone {
    let feedback = feedback.to_ascii_lowercase();
    if feedback.contains("failed") || feedback.contains("error") {
        Tone::Negative
    } else {
        Tone::Positive
    }
}

fn install_status(installed: bool) -> (&'static str, Tone) {
    if installed {
        ("Installed \u{2713}", Tone::Positive)
    } else {
        ("Not installed", Tone::Neutral)
    }
}

fn install_button_label(installed: bool) -> &'static str {
    if installed {
        "Update Service"
    } else {
        "Install Service"
    }
}

fn light_controls_status(configured: bool) -> (&'static str, Tone) {
    if configured {
        ("Configured \u{2713}", Tone::Positive)
    } else {
        ("Not configured", Tone::Neutral)
    }
}

fn service_status(running: bool, enabled: bool) -> (&'static str, Tone) {
    match (running, enabled) {
        (true, true) => ("Running \u{2713}", Tone::Positive),
        (true, false) => ("Running (not enabled)", Tone::Caution),
        (false, true) => ("Enabled, not running", Tone::Caution),
        (false, false) => ("Stopped and disabled", Tone::Neutral),
    }
}

fn shortcut_placeholder(capability: Option<ShortcutApplyCapability>) -> &'static str {
    match capability {
        Some(ShortcutApplyCapability::GnomeCustomShortcut) => "e.g. Super+G or <Super>g",
        Some(ShortcutApplyCapability::PortalServiceDropIn) => "e.g. Ctrl+Shift+G or <Ctrl><Shift>g",
        _ => "e.g. Ctrl+Shift+G",
    }
}

fn light_controls_details(status: &DaemonRuntimeStatus) -> Option<String> {
    let path = status.light_controls_config_path.as_deref()?;
    Some(if status.light_controls_configured {
        format!("Light controls: configured at {path}")
    } else {
        format!("Light controls include: {path}")
    })
}

/// The tool availability line, shown only when something is missing.
fn missing_tools(status: &DaemonRuntimeStatus) -> Option<String> {
    let mut missing = Vec::new();
    if !status.systemctl_available {
        missing.push("systemctl");
    }
    if !status.gsettings_available {
        missing.push("gsettings");
    }
    (!missing.is_empty()).then(|| format!("Missing tools: {}", missing.join(", ")))
}

fn service_installed(app: &ConfiguratorApp) -> bool {
    app.daemon_status
        .as_ref()
        .is_some_and(|status| status.service_installed)
}

// ---- Widget helpers ----------------------------------------------------

fn column_box() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .build()
}

fn row_box() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build()
}

fn body_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .build()
}

fn caption_label(text: &str) -> gtk::Label {
    let label = body_label(text);
    label.add_css_class("caption");
    label
}

fn hint_label(text: &str) -> gtk::Label {
    let label = caption_label(text);
    label.add_css_class("dim-label");
    label
}

fn warning_label(text: &str) -> gtk::Label {
    let label = caption_label(text);
    label.add_css_class("warning");
    label
}

fn action_button(
    label: &str,
    action: DaemonAction,
    sender: &ComponentSender<ConfiguratorApp>,
) -> gtk::Button {
    let button = gtk::Button::builder()
        .label(label)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .build();
    let sender = sender.clone();
    button.connect_clicked(move |_| sender.input(Message::DaemonActionRequested(action)));
    button
}

fn set_label(label: &gtk::Label, text: &str) {
    if label.label() != text {
        label.set_label(text);
    }
}

fn set_button_label(button: &gtk::Button, text: &str) {
    if button.label().as_deref() != Some(text) {
        button.set_label(text);
    }
}

/// Writes the widget's own visibility flag, never `is_visible`: a child of a
/// hidden group reports invisible while its own flag still says otherwise,
/// and skipping the write there would leak the stale state the moment the
/// group comes back.
fn set_visible(widget: &impl IsA<gtk::Widget>, visible: bool) {
    if widget.get_visible() != visible {
        widget.set_visible(visible);
    }
}

fn set_sensitive(widget: &impl IsA<gtk::Widget>, sensitive: bool) {
    if widget.is_sensitive() != sensitive {
        widget.set_sensitive(sensitive);
    }
}

fn apply_tone(widget: &impl IsA<gtk::Widget>, tone: Tone) {
    let wanted = tone.css_class();
    for class in Tone::CLASSES {
        let has_class = widget.has_css_class(class);
        if class == wanted {
            if !has_class {
                widget.add_css_class(class);
            }
        } else if has_class {
            widget.remove_css_class(class);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DesktopEnvironment, ShortcutBackend};

    fn shown_all() -> ShownAreas {
        ShownAreas {
            status: true,
            service: true,
            shortcut: true,
            light: true,
        }
    }

    fn visible_sections(shown: ShownAreas) -> Vec<DaemonSection> {
        DaemonSection::ALL
            .into_iter()
            .filter(|section| daemon_section_visible(*section, shown))
            .collect()
    }

    fn test_status() -> DaemonRuntimeStatus {
        DaemonRuntimeStatus {
            desktop: DesktopEnvironment::Hyprland,
            shortcut_backend: ShortcutBackend::Manual,
            shortcut_apply_capability: ShortcutApplyCapability::Manual,
            light_shortcut_apply_capability: LightShortcutApplyCapability::HyprlandNative,
            systemctl_available: true,
            gsettings_available: true,
            service_installed: false,
            service_enabled: false,
            service_active: false,
            service_unit_path: None,
            configured_shortcut: None,
            light_controls_configured: false,
            light_controls_config_path: None,
        }
    }

    #[test]
    fn daemon_sections_keep_default_setup_order() {
        assert_eq!(
            visible_sections(shown_all()),
            vec![
                DaemonSection::Install,
                DaemonSection::Shortcut,
                DaemonSection::LightControls,
                DaemonSection::Start,
                DaemonSection::TechnicalDetails,
            ],
        );
    }

    #[test]
    fn service_area_keeps_install_start_and_details() {
        assert_eq!(
            visible_sections(ShownAreas {
                status: false,
                service: true,
                shortcut: false,
                light: false,
            }),
            vec![
                DaemonSection::Install,
                DaemonSection::Start,
                DaemonSection::TechnicalDetails,
            ],
        );
    }

    #[test]
    fn status_area_alone_keeps_only_details() {
        assert_eq!(
            visible_sections(ShownAreas {
                status: true,
                service: false,
                shortcut: false,
                light: false,
            }),
            vec![DaemonSection::TechnicalDetails],
        );
    }

    #[test]
    fn shortcut_area_alone_keeps_only_the_shortcut_step() {
        assert_eq!(
            visible_sections(ShownAreas {
                status: false,
                service: false,
                shortcut: true,
                light: false,
            }),
            vec![DaemonSection::Shortcut],
        );
    }

    #[test]
    fn overall_status_follows_service_state() {
        assert_eq!(overall_status(None).0, "Status: Detecting...");

        let mut status = test_status();
        assert_eq!(overall_status(Some(&status)).0, "Status: Not installed");

        status.service_installed = true;
        assert_eq!(
            overall_status(Some(&status)),
            ("Status: Installed, not running", Tone::Caution)
        );

        status.service_active = true;
        assert_eq!(
            overall_status(Some(&status)),
            ("Status: Running", Tone::Positive)
        );
    }

    #[test]
    fn feedback_tone_flags_failures() {
        assert_eq!(
            feedback_tone("Background setup action failed"),
            Tone::Negative
        );
        assert_eq!(feedback_tone("gsettings ERROR: nope"), Tone::Negative);
        assert_eq!(
            feedback_tone("Background mode status loaded."),
            Tone::Positive
        );
    }

    #[test]
    fn service_status_separates_running_from_enabled() {
        assert_eq!(service_status(true, true).0, "Running \u{2713}");
        assert_eq!(service_status(true, false).0, "Running (not enabled)");
        assert_eq!(service_status(false, true).0, "Enabled, not running");
        assert_eq!(service_status(false, false).0, "Stopped and disabled");
    }

    #[test]
    fn shortcut_placeholder_follows_apply_capability() {
        assert_eq!(
            shortcut_placeholder(Some(ShortcutApplyCapability::GnomeCustomShortcut)),
            "e.g. Super+G or <Super>g"
        );
        assert_eq!(
            shortcut_placeholder(Some(ShortcutApplyCapability::PortalServiceDropIn)),
            "e.g. Ctrl+Shift+G or <Ctrl><Shift>g"
        );
        assert_eq!(
            shortcut_placeholder(Some(ShortcutApplyCapability::Manual)),
            "e.g. Ctrl+Shift+G"
        );
        assert_eq!(shortcut_placeholder(None), "e.g. Ctrl+Shift+G");
    }

    #[test]
    fn missing_tools_only_reports_absent_ones() {
        let mut status = test_status();
        assert_eq!(missing_tools(&status), None);

        status.gsettings_available = false;
        assert_eq!(
            missing_tools(&status).as_deref(),
            Some("Missing tools: gsettings")
        );

        status.systemctl_available = false;
        assert_eq!(
            missing_tools(&status).as_deref(),
            Some("Missing tools: systemctl, gsettings")
        );
    }

    #[test]
    fn light_controls_details_reflect_configured_state() {
        let mut status = test_status();
        assert_eq!(light_controls_details(&status), None);

        status.light_controls_config_path = Some("/tmp/wayscriber.conf".to_string());
        assert_eq!(
            light_controls_details(&status).as_deref(),
            Some("Light controls include: /tmp/wayscriber.conf")
        );

        status.light_controls_configured = true;
        assert_eq!(
            light_controls_details(&status).as_deref(),
            Some("Light controls: configured at /tmp/wayscriber.conf")
        );
    }
}
