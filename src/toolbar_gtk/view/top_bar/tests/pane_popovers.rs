//! Session, settings, and canvas pane assertions.

use super::super::{
    FeedbackSender, GtkToolbarFeedback, ToolbarEvent, ToolbarLayoutMode, ToolbarSnapshot, TopBar,
    Updater, model, plan_top_strip,
};
use super::widget_support::{collect_descendants, detach_test_popovers, find_widget_named};
use crate::ui::toolbar::RuntimeUiPersistenceMode;
use crate::ui::toolbar::RuntimeUiPersistenceSnapshot;
use gtk4::prelude::*;
use std::time::Duration;

pub(super) fn assert_menu_popover_contracts(regular: &ToolbarSnapshot) {
    // --- Session/Settings popovers: the re-hosted pane content ---------------
    let mut session_snapshot = regular.clone();
    session_snapshot.session_popover_open = true;
    session_snapshot.active_session_name = Some("lecture.wayscriber-session".to_string());
    session_snapshot.active_session_path =
        Some(std::path::PathBuf::from("/tmp/lecture.wayscriber-session"));
    session_snapshot.recent_sessions = vec![crate::ui::toolbar::SessionRecentSnapshot {
        display_name: "recent-0.wayscriber-session".to_string(),
        path: std::path::PathBuf::from("/tmp/recent-0.wayscriber-session"),
    }];
    let (tx, menu_rx) = std::sync::mpsc::channel();
    let mut menu_top = TopBar::new_for_test(FeedbackSender::new(tx));
    // Building the strip creates the two overflow-anchored native popovers.
    menu_top.build_strip(
        &session_snapshot,
        &plan_top_strip(&crate::ui_text::UiTextEngine::default(), &session_snapshot),
    );
    assert!(menu_top.session.mounted.is_some(), "session popover exists");
    assert!(
        menu_top.settings.mounted.is_some(),
        "settings popover exists"
    );

    assert_session_popover_contract(&menu_top, &session_snapshot, &menu_rx);
    assert_settings_popover_contract(&menu_top, regular, &menu_rx);
    assert_canvas_popover_contract(&menu_top, regular, &menu_rx);

    detach_test_popovers(&mut menu_top);
}

fn assert_session_popover_contract(
    top: &TopBar,
    snapshot: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let model = model::ToolbarSessionModel::for_popover(snapshot).expect("session model");
    let content = top.build_session_popover_content(snapshot, 1.0);
    let panel =
        find_widget_named(&content, "top.menu.session.panel").expect("session popover panel box");
    let mut buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(&panel, &mut buttons);
    assert_eq!(
        buttons.len(),
        model.buttons.len() + model.recents.len(),
        "the popover exposes exactly the pane's controls"
    );
    for (button, button_model) in buttons.iter().zip(model.buttons.iter()) {
        assert_eq!(button.tooltip_text().as_deref(), Some(button_model.label));
        assert_eq!(button.is_sensitive(), button_model.enabled);
    }
    buttons[0].emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK session open event"),
        GtkToolbarFeedback::Event {
            event: model.buttons[0].event.clone(),
            rebind_requested: false,
        }
    );
    buttons.last().expect("recent row button").emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK recent event"),
        GtkToolbarFeedback::Event {
            event: model.recents[0].event(),
            rebind_requested: false,
        }
    );
}

fn assert_settings_popover_contract(
    top: &TopBar,
    regular: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let mut snapshot = regular.clone();
    snapshot.layout_mode = ToolbarLayoutMode::Advanced;
    snapshot.settings_popover_open = true;
    snapshot.runtime_ui_persistence = Some(RuntimeUiPersistenceSnapshot {
        path: "/home/user/.local/share/wayscriber/runtime-ui.toml".into(),
        mode: RuntimeUiPersistenceMode::Supported,
        detail: None,
        recovery_artifacts: Vec::new(),
    });
    let model = model::ToolbarSettingsModel::for_popover(&snapshot).expect("settings model");
    let (content, updaters) = top.build_settings_popover_content(&snapshot, 1.0);
    content.add_css_class("wayscriber-toolbar");
    let scroller = content
        .clone()
        .downcast::<gtk4::ScrolledWindow>()
        .expect("settings popover scroll viewport");
    let panel =
        find_widget_named(&content, "top.menu.settings.panel").expect("settings popover panel box");
    let (_, natural_height, _, _) = panel.measure(gtk4::Orientation::Vertical, -1);
    assert!(natural_height <= scroller.max_content_height());
    assert_settings_toggle_contract(&mut snapshot, &model, &panel, &updaters, rx);
    assert_settings_button_contract(&snapshot, &model, &panel, rx);
}

fn assert_settings_toggle_contract(
    snapshot: &mut ToolbarSnapshot,
    model: &model::ToolbarSettingsModel,
    panel: &gtk4::Widget,
    updaters: &[Updater],
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let mut checks: Vec<gtk4::CheckButton> = Vec::new();
    collect_descendants(panel, &mut checks);
    let toggles: Vec<_> = model.toggle_rows().into_iter().flatten().collect();
    assert_eq!(checks.len(), toggles.len(), "settings toggle parity");
    for (check, toggle) in checks.iter().zip(&toggles) {
        assert_eq!(check.label().as_deref(), Some(toggle.label.as_ref()));
        assert_eq!(check.is_active(), toggle.checked, "{}", toggle.label);
    }
    checks[0].set_active(!toggles[0].checked);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK settings toggle event"),
        GtkToolbarFeedback::Event {
            event: toggles[0].activation.clone(),
            rebind_requested: false,
        }
    );
    let updated = !toggles[0].checked;
    snapshot.context_aware_ui = updated;
    for updater in updaters {
        updater(snapshot);
    }
    assert_eq!(checks[0].is_active(), updated);
    checks[0].set_active(!updated);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("updated GTK settings toggle event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::ToggleContextAwareUi(!updated),
            rebind_requested: false,
        }
    );
}

fn assert_settings_button_contract(
    snapshot: &ToolbarSnapshot,
    model: &model::ToolbarSettingsModel,
    panel: &gtk4::Widget,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let mut buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(panel, &mut buttons);
    // The "Details" disclosure toggle shares the compact tab styling but is
    // not a layout segment.
    let (details, buttons): (Vec<_>, Vec<_>) = buttons
        .into_iter()
        .partition(|button| button.has_css_class("details"));
    let expected_details = model
        .details()
        .expect("the runtime path waits behind Details");
    assert_eq!(details.len(), 1);
    assert_eq!(details[0].label().as_deref(), Some(expected_details.label));
    details[0].emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK details toggle event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::SetSettingsDetailsOpen(true),
            rebind_requested: false,
        }
    );
    let tabs: Vec<_> = buttons
        .iter()
        .filter(|button| button.has_css_class("tab"))
        .collect();
    let control = model::layout_mode_control(snapshot.layout_mode);
    let model::ToolbarControlKind::Segmented(segmented) = &control.kind else {
        panic!("layout mode control is segmented");
    };
    assert_eq!(tabs.len(), segmented.segments().len());
    tabs[0].emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK layout mode event"),
        GtkToolbarFeedback::Event {
            event: segmented.segments()[0].activation.clone(),
            rebind_requested: false,
        }
    );
    let plain: Vec<_> = buttons
        .iter()
        .filter(|button| !button.has_css_class("tab"))
        .collect();
    assert_eq!(plain.len(), model.buttons().len(), "settings button parity");
    plain[0].emit_clicked();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK settings button event"),
        GtkToolbarFeedback::Event {
            event: model.buttons()[0].event.clone(),
            rebind_requested: false,
        }
    );
}

fn assert_canvas_popover_contract(
    top: &TopBar,
    regular: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    assert!(top.canvas.mounted.is_some(), "canvas popover exists");
    assert_canvas_command_sections(top, regular, rx);
    assert_canvas_delay_updates(top, regular);
    assert_empty_canvas_popover(top, regular);
}

fn assert_canvas_command_sections(
    top: &TopBar,
    regular: &ToolbarSnapshot,
    rx: &std::sync::mpsc::Receiver<GtkToolbarFeedback>,
) {
    let mut snapshot = regular.clone();
    snapshot.canvas_popover_open = true;
    snapshot.show_actions_section = true;
    snapshot.show_boards_section = true;
    snapshot.show_pages_section = true;
    snapshot.show_zoom_actions = true;
    snapshot.show_actions_advanced = true;
    snapshot.show_step_section = true;
    let (content, _) = top.build_canvas_popover_content(&snapshot, 1.0);
    let panel =
        find_widget_named(&content, "top.menu.canvas.panel").expect("canvas popover panel box");
    assert_eq!(
        panel.width_request(),
        crate::ui::theme::toolbar::CANVAS_MENU_CONTENT_W as i32
    );
    assert_eq!(panel.margin_start(), 10);
    assert_eq!(panel.margin_end(), 10);
    let mut buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(&panel, &mut buttons);
    assert!(buttons.len() >= 4);
    for noun in ["Board", "Page"] {
        assert_canvas_command_button_layout(&buttons, noun);
    }
    let mut checks: Vec<gtk4::CheckButton> = Vec::new();
    collect_descendants(&panel, &mut checks);
    assert_eq!(checks.len(), 2, "Step buttons + Delay sliders toggles");
    checks[0].set_active(!snapshot.custom_section_enabled);
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("GTK canvas step toggle event"),
        GtkToolbarFeedback::Event {
            event: ToolbarEvent::ToggleCustomSection(!snapshot.custom_section_enabled),
            rebind_requested: false,
        }
    );
}

fn assert_canvas_command_button_layout(buttons: &[gtk4::Button], noun: &str) {
    let button_with_tooltip = |prefix: &str| {
        buttons
            .iter()
            .find(|button| {
                button
                    .tooltip_text()
                    .is_some_and(|tooltip| tooltip.starts_with(prefix))
            })
            .unwrap_or_else(|| panic!("{prefix} Canvas button"))
    };
    let duplicate = button_with_tooltip(&format!("Duplicate {noun}"));
    let delete = button_with_tooltip(&format!("Delete {noun}"));
    for button in [duplicate, delete] {
        assert_eq!(button.width_request(), 32);
        assert_eq!(button.halign(), gtk4::Align::Center);
        assert!(!button.hexpands());
    }
    assert_eq!(delete.margin_end(), 6);
    let safe_parent = duplicate.parent().expect("safe-action group");
    let destructive_parent = delete.parent().expect("destructive-action row");
    assert_ne!(safe_parent, destructive_parent);
    assert!(
        safe_parent
            .downcast::<gtk4::Box>()
            .expect("homogeneous safe-action box")
            .is_homogeneous()
    );
    assert_eq!(
        destructive_parent
            .downcast::<gtk4::Box>()
            .expect("guarded command row")
            .spacing(),
        12
    );
}

fn undo_all_slider_tooltip(boxes: &[gtk4::Box]) -> String {
    boxes
        .iter()
        .find_map(|widget| {
            widget
                .tooltip_text()
                .filter(|tooltip| tooltip.contains("Undo-all delay"))
                .map(|tooltip| tooltip.to_string())
        })
        .expect("undo-all delay slider tooltip")
}

fn assert_canvas_delay_updates(top: &TopBar, regular: &ToolbarSnapshot) {
    let mut snapshot = regular.clone();
    snapshot.canvas_popover_open = true;
    snapshot.show_step_section = true;
    snapshot.show_delay_sliders = true;
    snapshot.custom_section_enabled = false;
    snapshot.undo_all_delay_ms = 1000;
    let (content, updaters) = top.build_canvas_popover_content(&snapshot, 1.0);
    assert!(!updaters.is_empty());
    let panel = find_widget_named(&content, "top.menu.canvas.panel")
        .expect("delay canvas popover panel box");
    let mut boxes: Vec<gtk4::Box> = Vec::new();
    collect_descendants(&panel, &mut boxes);
    assert!(undo_all_slider_tooltip(&boxes).contains("1.0s"));
    let mut bumped = snapshot;
    bumped.undo_all_delay_ms = 2500;
    for updater in &updaters {
        updater(&bumped);
    }
    assert!(undo_all_slider_tooltip(&boxes).contains("2.5s"));
}

fn assert_empty_canvas_popover(top: &TopBar, regular: &ToolbarSnapshot) {
    let mut snapshot = regular.clone();
    snapshot.canvas_popover_open = true;
    snapshot.show_actions_section = false;
    snapshot.show_boards_section = false;
    snapshot.show_pages_section = false;
    snapshot.show_zoom_actions = false;
    snapshot.show_actions_advanced = false;
    snapshot.show_step_section = false;
    let (content, _) = top.build_canvas_popover_content(&snapshot, 1.0);
    let panel = find_widget_named(&content, "top.menu.canvas.panel")
        .expect("empty canvas popover panel box");
    let mut buttons: Vec<gtk4::Button> = Vec::new();
    collect_descendants(&panel, &mut buttons);
    assert!(buttons.is_empty());
}
