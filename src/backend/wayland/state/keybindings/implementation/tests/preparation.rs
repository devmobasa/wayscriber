use super::*;

/// The point of the restored flow: the chord the user typed is in the
/// keymap the moment the edit is accepted, and the message claims a durable
/// change because the write below makes it one.
#[test]
fn a_captured_chord_replaces_the_binding_and_reports_a_durable_change() {
    let mut keybindings = KeybindingsConfig::default();
    let request = replace(Action::SelectPenTool, "Ctrl+Alt+Shift+K");

    apply_keybinding_edit(&mut keybindings, &request).expect("a free chord is accepted");

    assert_eq!(
        keybindings.bindings_for_action(Action::SelectPenTool),
        Some(&["Ctrl+Alt+Shift+K".to_string()][..])
    );
    assert_eq!(
        shortcut_applied_message(&request),
        "Updated shortcut for Pen Tool."
    );
}

#[test]
fn a_taken_chord_is_refused_and_names_the_action_that_owns_it() {
    let mut keybindings = KeybindingsConfig::default();

    let error = apply_keybinding_edit(&mut keybindings, &replace(Action::ClearCanvas, "F"))
        .expect_err("the pen shortcut is taken");

    match error {
        KeybindingEditError::Conflict {
            binding,
            existing_action,
        } => {
            assert_eq!(binding, "F");
            assert_eq!(existing_action, Action::SelectPenTool);
            assert_eq!(
                shortcut_conflict_message(&binding, existing_action),
                "Shortcut not changed — F is already assigned to Pen Tool."
            );
        }
        other => panic!("expected a structured shortcut conflict, got {other:?}"),
    }
    assert_eq!(
        keybindings.bindings_for_action(Action::ClearCanvas),
        KeybindingsConfig::default().bindings_for_action(Action::ClearCanvas),
        "a refused edit leaves the keymap alone"
    );
}

/// The claim check works on parsed bindings, whose equality folds key case,
/// so respelling a taken chord cannot sneak it past the gate.
#[test]
fn an_edit_onto_a_key_spelled_in_a_different_case_is_still_refused() {
    let mut keybindings = KeybindingsConfig::default();
    keybindings.core.undo = vec!["Ctrl+Alt+U".to_string()];

    let error = apply_keybinding_edit(
        &mut keybindings,
        &replace(Action::ClearCanvas, "ctrl+alt+u"),
    )
    .expect_err("the chord is taken regardless of spelling");

    match error {
        KeybindingEditError::Conflict {
            binding,
            existing_action,
        } => {
            assert_eq!(binding, "ctrl+alt+u");
            assert_eq!(existing_action, Action::Undo);
        }
        other => panic!("expected a case-insensitive conflict, got {other:?}"),
    }
}

/// Rebinding an action onto a key it already holds is not a self-conflict:
/// the edited action is excluded from the claim lookup.
#[test]
fn an_action_can_be_rebound_onto_a_key_it_already_holds() {
    let mut keybindings = KeybindingsConfig::default();

    apply_keybinding_edit(&mut keybindings, &replace(Action::SelectPenTool, "F"))
        .expect("an action never conflicts with itself");

    assert_eq!(
        keybindings.bindings_for_action(Action::SelectPenTool),
        Some(&["F".to_string()][..])
    );
}

#[test]
fn one_chord_listed_twice_is_refused_without_naming_another_action() {
    let mut keybindings = KeybindingsConfig::default();

    let error = apply_keybinding_edit(
        &mut keybindings,
        &KeybindingEditRequest {
            action: Action::SelectPenTool,
            operation: KeybindingEditOperation::Replace(vec![
                "Ctrl+Alt+Shift+K".to_string(),
                "ctrl+alt+shift+k".to_string(),
            ]),
        },
    )
    .expect_err("a repeated chord is not a usable list");

    match error {
        KeybindingEditError::Edit(message) => assert_eq!(
            message,
            "Shortcut not changed — Ctrl+Alt+Shift+K is listed twice for Pen Tool."
        ),
        other => panic!("expected a plain edit refusal, got {other:?}"),
    }
}

#[test]
fn unbinding_empties_the_actions_binding_list() {
    let mut keybindings = KeybindingsConfig::default();
    let request = KeybindingEditRequest {
        action: Action::SelectPenTool,
        operation: KeybindingEditOperation::Delete,
    };

    apply_keybinding_edit(&mut keybindings, &request).expect("an unbind always applies");

    assert_eq!(
        keybindings.bindings_for_action(Action::SelectPenTool),
        Some(&[][..])
    );
    assert_eq!(shortcut_applied_message(&request), "Unbound Pen Tool.");
}

#[test]
fn resetting_restores_the_compiled_default() {
    let mut keybindings = KeybindingsConfig::default();
    let default = keybindings
        .bindings_for_action(Action::SelectPenTool)
        .map(<[String]>::to_vec)
        .expect("the pen tool ships a shortcut");
    keybindings
        .set_bindings_for_action(Action::SelectPenTool, vec!["Ctrl+Alt+Shift+J".to_string()])
        .expect("the pen tool stores a shortcut");

    let request = KeybindingEditRequest {
        action: Action::SelectPenTool,
        operation: KeybindingEditOperation::Reset,
    };
    apply_keybinding_edit(&mut keybindings, &request).expect("a reset to defaults applies");

    assert_eq!(
        keybindings.bindings_for_action(Action::SelectPenTool),
        Some(default.as_slice())
    );
    assert_eq!(
        shortcut_applied_message(&request),
        "Reset the shortcut for Pen Tool to default."
    );
}

/// A rebound keymap still builds both runtime views; without that the
/// handler would refuse its own accepted edit.
#[test]
fn an_edited_keymap_still_builds_both_runtime_views() {
    let mut keybindings = KeybindingsConfig::default();
    apply_keybinding_edit(
        &mut keybindings,
        &replace(Action::ClearCanvas, "Ctrl+Alt+Shift+K"),
    )
    .expect("a free chord is accepted");

    let action_map = keybindings.build_action_map().expect("action map");
    let action_bindings = keybindings
        .build_action_bindings()
        .expect("action bindings");

    let chord = ShortcutTrigger::parse("Ctrl+Alt+Shift+K").expect("a parseable chord");
    assert_eq!(action_map.get(&chord), Some(&Action::ClearCanvas));
    assert_eq!(
        action_bindings.get(&Action::ClearCanvas),
        Some(&vec![chord])
    );
}

/// The ordering, at the seam the async write moved it to.
///
/// Preparing an edit cannot touch the run's keymap — the signature only
/// lends it — so the chord the user typed exists nowhere but the write until
/// the file answers. That is what makes the refusal below a refusal rather
/// than a rollback.
#[test]
fn preparing_an_edit_leaves_the_running_keymap_to_the_completion() {
    let running = KeybindingsConfig::default();
    let before = running
        .bindings_for_action(Action::SelectPenTool)
        .map(<[String]>::to_vec);

    let write = prepare(&running, replace(Action::SelectPenTool, "Ctrl+Alt+Shift+K"))
        .expect("a free chord is accepted");

    assert_eq!(
        write.bindings,
        ["Ctrl+Alt+Shift+K".to_string()],
        "the write carries the list the file is asked to hold"
    );
    assert_eq!(
        write.request.action,
        Action::SelectPenTool,
        "and the action it belongs to, which is the whole of the delta"
    );
    assert_eq!(
        running
            .bindings_for_action(Action::SelectPenTool)
            .map(<[String]>::to_vec),
        before,
        "the running keymap must be exactly as it was"
    );
}

/// The chord an outstanding edit is giving up is free for the next one.
///
/// The palette moves Pen off `F` and, before that write reports back, binds
/// Marker to `F`. Nothing is installed until a completion arrives, so the
/// running keymap still shows Pen on `F` — and checking against it alone
/// refuses the second gesture over a claim the file is about to drop, while
/// the file itself, which will have taken the first edit by the time the
/// second is written, accepts it. The claim check reads the keymap with the
/// outstanding delta folded in, so the two agree.
#[test]
fn a_chord_an_in_flight_edit_is_giving_up_is_free_for_the_next_edit() {
    let running = KeybindingsConfig::default();
    assert_eq!(
        running.bindings_for_action(Action::SelectPenTool),
        Some(&["F".to_string()][..]),
        "the fixture is the shipped keymap the palette would be reading"
    );

    let refused = prepare(&running, replace(Action::SelectMarkerTool, "F"))
        .expect_err("with nothing queued, Pen still holds the chord");
    assert_eq!(
        refused, "Shortcut not changed — F is already assigned to Pen Tool.",
        "and that refusal is the honest one while no edit is outstanding"
    );

    let write = prepare_behind(
        &running,
        &[(Action::SelectPenTool, "Ctrl+Alt+Shift+P")],
        replace(Action::SelectMarkerTool, "F"),
    )
    .expect("the chord the queued edit is giving up is free");

    assert_eq!(write.request.action, Action::SelectMarkerTool);
    assert_eq!(write.bindings, ["F".to_string()]);
}

/// The other direction: a chord an outstanding edit has asked *for* is
/// taken, though no keymap holds it yet.
#[test]
fn a_chord_an_in_flight_edit_asked_for_is_already_taken() {
    let running = KeybindingsConfig::default();

    let refused = prepare_behind(
        &running,
        &[(Action::SelectPenTool, "Ctrl+Alt+Shift+P")],
        replace(Action::SelectMarkerTool, "Ctrl+Alt+Shift+P"),
    )
    .expect_err("the queued edit has already asked for this chord");

    assert_eq!(
        refused, "Shortcut not changed — Ctrl+Alt+Shift+P is already assigned to Pen Tool.",
        "and the refusal names the action that asked for it, not a keymap holder"
    );
}
