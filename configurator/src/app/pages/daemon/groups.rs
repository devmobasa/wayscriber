use super::*;

pub(super) fn overview_group(
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

pub(super) fn install_group(
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

pub(super) fn shortcut_group(
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

pub(super) fn light_controls_group(
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

pub(super) fn start_group(
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

pub(super) fn details_group(
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
