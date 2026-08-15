use super::*;

fn prepared() -> KeybindingEditWrite {
    prepare(
        &KeybindingsConfig::default(),
        replace(Action::SelectPenTool, "Ctrl+Alt+Shift+K"),
    )
    .expect("a free chord is accepted")
}

/// A landed write is the only outcome that both installs and claims a save.
#[test]
fn a_write_that_landed_installs_the_prepared_delta() {
    let completion = shortcut_completion(
        prepared(),
        Ok(ConfigEditOutcome {
            backup_path: None,
            write: ConfigEditWrite::Wrote,
        }),
    );

    assert!(completion.saved);
    assert_eq!(completion.message, "Updated shortcut for Pen Tool.");
    let install = completion.install.expect("a landed write installs");
    let installed = install_keybinding_edit(&KeybindingsConfig::default(), &install)
        .expect("the delta folds into the keymap it was prepared against");
    assert_eq!(
        installed
            .keybindings
            .bindings_for_action(Action::SelectPenTool),
        Some(&["Ctrl+Alt+Shift+K".to_string()][..])
    );
}

/// A file that already said this installs too — it is what the file holds —
/// but the wording must not claim a save that did not happen.
#[test]
fn a_write_with_nothing_to_do_installs_and_says_so() {
    let completion = shortcut_completion(
        prepared(),
        Ok(ConfigEditOutcome {
            backup_path: None,
            write: ConfigEditWrite::AlreadyCurrent,
        }),
    );

    assert!(completion.saved);
    assert_eq!(completion.message, "Pen Tool already uses that shortcut.");
    assert!(completion.install.is_some());
    assert_eq!(
        completion.file,
        ShortcutFileOutcome::AlreadyCurrent,
        "the file holds the shortcut, but this gesture is not what put it \
         there, and the question asked after this one is worded on that"
    );
}

/// The refusal: the file gave the chord away since this run read it, so
/// nothing is installed. This is the property the move off the dispatch
/// thread had to preserve — severing the completion from the install, by
/// installing regardless of the answer, fails here.
#[test]
fn a_chord_claimed_on_disk_installs_nothing_and_names_the_owner() {
    let completion = shortcut_completion(
        prepared(),
        Err(anyhow::anyhow!(ShortcutClaimedOnDisk {
            binding: "Ctrl+Alt+Shift+K".to_string(),
            claimed_by: Action::Undo,
        })),
    );

    assert!(
        completion.install.is_none(),
        "a refused edit must leave the run holding its old keymap"
    );
    assert!(!completion.saved);
    assert_eq!(
        completion.message,
        "Shortcut not changed — config.toml now assigns Ctrl+Alt+Shift+K to Undo."
    );
}

/// Every other failure degrades rather than refusing: the shortcut works for
/// the run and the toast says the file missed it.
#[test]
fn a_failed_write_still_installs_for_the_run() {
    let completion = shortcut_completion(prepared(), Err(anyhow::anyhow!("the disk is full")));

    assert!(
        completion.install.is_some(),
        "throwing away a shortcut the user just typed is the worse outcome"
    );
    assert!(!completion.saved);
    assert_eq!(completion.message, SHORTCUT_SAVE_FAILED);
    assert_eq!(
        completion.file,
        ShortcutFileOutcome::Rejected,
        "the file is not carrying this shortcut, and anything said later \
         about it has to start from that"
    );
}

/// A write that landed without reading back is still a file that does not
/// hold the shortcut.
///
/// The file *did* change, which is why the message sends the user to it
/// rather than claiming it is untouched — but the value is not in it, so
/// nothing downstream may treat this as a save.
#[test]
fn a_write_that_does_not_read_back_leaves_the_file_without_the_shortcut() {
    let completion = shortcut_completion(
        prepared(),
        Err(anyhow::anyhow!(ConfigEditNotReadBack {
            what: "Shortcut".to_string(),
            path: PathBuf::from("/somewhere/config.toml"),
        })),
    );

    assert_eq!(completion.message, SHORTCUT_WRITE_UNVERIFIED);
    assert!(!completion.saved);
    assert_eq!(completion.file, ShortcutFileOutcome::Rejected);
}

/// Two edits in flight at once, and both must survive.
///
/// The palette rebinds Pen and then Marker before the first write finishes,
/// so the second is prepared against a keymap that does not have the first
/// edit in it yet. Installing what each *write* was prepared with would let
/// the second put Pen back on `F` while its toast claimed a save, and the
/// file — which has both — would disagree with the run until restart. Each
/// completion installs its own action's bindings and nothing else.
#[test]
fn a_second_edits_completion_keeps_the_first_edits_chord() {
    let running = KeybindingsConfig::default();
    assert_eq!(
        running.bindings_for_action(Action::SelectPenTool),
        Some(&["F".to_string()][..]),
        "the fixture is the shipped keymap the palette would be reading"
    );
    assert_eq!(
        running.bindings_for_action(Action::SelectMarkerTool),
        Some(&["H".to_string()][..])
    );

    // Both accepted before either write reports back, so both are prepared
    // against the same starting keymap.
    let pen = prepare(&running, replace(Action::SelectPenTool, "Ctrl+Alt+Shift+P"))
        .expect("a free chord is accepted");
    let marker = prepare(
        &running,
        replace(Action::SelectMarkerTool, "Ctrl+Alt+Shift+M"),
    )
    .expect("a free chord is accepted");

    let landed = || {
        Ok(ConfigEditOutcome {
            backup_path: None,
            write: ConfigEditWrite::Wrote,
        })
    };
    let pen_completion = shortcut_completion(pen, landed());
    let after_pen = install_keybinding_edit(
        &running,
        &pen_completion.install.expect("a landed write installs"),
    )
    .expect("the first delta folds in");

    let marker_completion = shortcut_completion(marker, landed());
    let after_both = install_keybinding_edit(
        &after_pen.keybindings,
        &marker_completion.install.expect("a landed write installs"),
    )
    .expect("the second delta folds into what the first left");

    assert_eq!(
        after_both
            .keybindings
            .bindings_for_action(Action::SelectPenTool),
        Some(&["Ctrl+Alt+Shift+P".to_string()][..]),
        "the second completion must not take the first edit back out"
    );
    assert_eq!(
        after_both
            .keybindings
            .bindings_for_action(Action::SelectMarkerTool),
        Some(&["Ctrl+Alt+Shift+M".to_string()][..])
    );
    let pen_chord = Shortcut::parse("Ctrl+Alt+Shift+P").expect("a parseable chord");
    let marker_chord = Shortcut::parse("Ctrl+Alt+Shift+M").expect("a parseable chord");
    assert_eq!(
        after_both.action_map.get(&pen_chord),
        Some(&Action::SelectPenTool)
    );
    assert_eq!(
        after_both.action_map.get(&marker_chord),
        Some(&Action::SelectMarkerTool),
        "both runtime views are rebuilt from the keymap that holds both edits"
    );
    assert_eq!(
        after_both.action_bindings.get(&Action::SelectPenTool),
        Some(&vec![pen_chord])
    );
    assert_eq!(
        after_both.action_bindings.get(&Action::SelectMarkerTool),
        Some(&vec![marker_chord])
    );

    // And each gesture gets its own toast, claiming its own save.
    assert!(pen_completion.saved);
    assert_eq!(pen_completion.message, "Updated shortcut for Pen Tool.");
    assert!(marker_completion.saved);
    assert_eq!(
        marker_completion.message,
        "Updated shortcut for Marker Tool."
    );
}

/// The one state a delta install can refuse, and what the user is told.
///
/// Two deltas contesting one chord do not normally meet here: the claim
/// check reads the running keymap with the outstanding deltas folded in, so
/// the second gesture is refused before it is ever queued. What still
/// reaches this point is a projection that did not come true — a completion
/// in front installing something other than what it promised, or nothing at
/// all — leaving the run dispatching a chord the arriving delta wants. The
/// run cannot dispatch a keymap with two actions on one chord, so it keeps
/// the one it has and says which way the two now disagree.
///
/// The pair is built directly rather than driven through a sequence: what is
/// under test is the refusal and its wording, and every route to it ends in
/// the same two values.
#[test]
fn an_edit_the_running_keymap_cannot_take_is_not_installed() {
    let running = KeybindingsConfig::default();
    // The chord in the run's keymap, put there the way a failed write does:
    // kept for the session, with the file never hearing about it.
    let degraded = shortcut_completion(
        prepare(&running, replace(Action::SelectPenTool, "Ctrl+Alt+Shift+P"))
            .expect("a free chord is accepted"),
        Err(anyhow::anyhow!("the disk is full")),
    );
    let after_pen = install_keybinding_edit(
        &running,
        &degraded.install.expect("a failed write still installs"),
    )
    .expect("the first delta folds in");

    // The second edit, checked against a keymap that does not carry the
    // first chord — which is what a projection promises and a completion can
    // fail to deliver — and written to a file that never got it either.
    let marker = prepare(
        &running,
        replace(Action::SelectMarkerTool, "Ctrl+Alt+Shift+P"),
    )
    .expect("the chord is free in the keymap this was prepared against");
    let completion = shortcut_completion(
        marker,
        Ok(ConfigEditOutcome {
            backup_path: None,
            write: ConfigEditWrite::Wrote,
        }),
    );

    assert!(
        install_keybinding_edit(
            &after_pen.keybindings,
            &completion.install.expect("a landed write installs"),
        )
        .is_err(),
        "two actions on one chord is not a keymap the run can dispatch from"
    );
    assert_eq!(
        after_pen
            .keybindings
            .bindings_for_action(Action::SelectPenTool),
        Some(&["Ctrl+Alt+Shift+P".to_string()][..]),
        "and the refusal leaves the run holding what it had"
    );
    assert_eq!(
        completion.file,
        ShortcutFileOutcome::Wrote,
        "the file took this one, which is what the wording below rests on"
    );
    assert_eq!(
        shortcut_not_installed_message(completion.file),
        SHORTCUT_NOT_INSTALLED
    );
    assert_eq!(
        SHORTCUT_NOT_INSTALLED,
        "Shortcut saved to config.toml, but this run kept its own — another \
         edit here already uses that key (see logs).",
        "the wording must not claim the run took it"
    );
}

/// The same collision over a file that was never written at all.
///
/// The second edit asks for a shortcut `config.toml` already resolves to, so
/// the write has no delta, touches nothing, and spends no backup — and the
/// run still cannot take a second action onto a chord the first edit is
/// dispatching. Reporting "saved to config.toml" here would credit this
/// gesture with a write it never made and send the user to a `.bak` that
/// does not exist; what is true is that the file and the run disagree.
#[test]
fn an_edit_the_file_already_had_and_the_run_cannot_take_must_not_claim_a_save() {
    let running = KeybindingsConfig::default();
    let contested = "Ctrl+Alt+Shift+P";
    let degraded = shortcut_completion(
        prepare(&running, replace(Action::SelectPenTool, contested))
            .expect("a free chord is accepted"),
        Err(anyhow::anyhow!("the disk is full")),
    );
    let after_pen = install_keybinding_edit(
        &running,
        &degraded.install.expect("a failed write still installs"),
    )
    .expect("the first delta folds in");

    // Checked against the same keymap without the first chord in it, and
    // answered by a file that already says exactly this.
    let marker = prepare(&running, replace(Action::SelectMarkerTool, contested))
        .expect("the chord is free in the keymap this was checked against");
    let completion = shortcut_completion(
        marker,
        Ok(ConfigEditOutcome {
            backup_path: None,
            write: ConfigEditWrite::AlreadyCurrent,
        }),
    );

    assert_eq!(
        completion.file,
        ShortcutFileOutcome::AlreadyCurrent,
        "the file holds the shortcut, and no write of this gesture's put it there"
    );
    assert!(
        install_keybinding_edit(
            &after_pen.keybindings,
            &completion
                .install
                .expect("a file that already agreed still offers its delta"),
        )
        .is_err(),
        "two actions on one chord is not a keymap the run can dispatch from"
    );
    assert_eq!(
        shortcut_not_installed_message(completion.file),
        SHORTCUT_ALREADY_CURRENT_NOT_INSTALLED
    );
    assert_eq!(
        SHORTCUT_ALREADY_CURRENT_NOT_INSTALLED,
        "config.toml already has this shortcut, but this run kept its own — \
         another edit here already uses that key (see logs).",
    );
    assert!(
        !SHORTCUT_ALREADY_CURRENT_NOT_INSTALLED.contains("saved to config.toml"),
        "nothing was written, so nothing may report a save"
    );
}

/// The same collision with the file on the other side: neither edit landed.
///
/// Both writes failed, so `config.toml` has neither shortcut, and the run
/// still cannot put two actions on one chord. The refusal used to borrow the
/// wording of the case above and tell the user their shortcut was "saved to
/// config.toml" — over a file that never received either edit.
#[test]
fn an_edit_neither_the_file_nor_the_run_took_must_not_claim_a_save() {
    let running = KeybindingsConfig::default();
    let contested = "Ctrl+Alt+Shift+P";
    let degraded = shortcut_completion(
        prepare(&running, replace(Action::SelectPenTool, contested))
            .expect("a free chord is accepted"),
        Err(anyhow::anyhow!("the disk is full")),
    );
    let after_pen = install_keybinding_edit(
        &running,
        &degraded.install.expect("a failed write still installs"),
    )
    .expect("the first delta folds in");

    // The second edit is checked against the same keymap without the first
    // chord in it, and its write fails the same way the first one's did —
    // the disk did not get better in between.
    let marker = prepare(&running, replace(Action::SelectMarkerTool, contested))
        .expect("the chord is free in the keymap this was checked against");
    let completion = shortcut_completion(marker, Err(anyhow::anyhow!("the disk is full")));

    assert_eq!(
        completion.file,
        ShortcutFileOutcome::Rejected,
        "the file got neither edit"
    );
    assert!(
        install_keybinding_edit(
            &after_pen.keybindings,
            &completion
                .install
                .expect("a failed write still offers its delta"),
        )
        .is_err(),
        "two actions on one chord is not a keymap the run can dispatch from"
    );
    assert_eq!(
        shortcut_not_installed_message(completion.file),
        SHORTCUT_NOT_SAVED_OR_INSTALLED
    );
    assert_eq!(
        SHORTCUT_NOT_SAVED_OR_INSTALLED,
        "Shortcut not changed — config.toml did not take it and another edit \
         here already uses that key (see logs).",
    );
    assert!(
        !SHORTCUT_NOT_SAVED_OR_INSTALLED.contains("saved to config.toml"),
        "nothing reached the file, so nothing may send the user to it"
    );
}
