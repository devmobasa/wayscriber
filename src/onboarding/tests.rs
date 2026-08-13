use super::*;
use std::fs;

#[test]
fn onboarding_defaults_when_missing() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    let mut store = OnboardingStore::load_from_path(path.clone());
    assert!(!store.state().welcome_shown);
    assert!(!store.state().toolbar_hint_shown);
    assert!(!store.state().first_run_completed);
    assert!(store.state().active_step.is_none());

    store.save().expect("default state should persist");
    assert!(path.exists());
}

#[test]
fn onboarding_persists_flags() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    let mut store = OnboardingStore::load_from_path(path.clone());
    store.state_mut().welcome_shown = true;
    store.state_mut().toolbar_hint_shown = true;
    store.state_mut().used_help_overlay = true;
    store.save().expect("updated state should persist");

    let reloaded = OnboardingStore::load_from_path(path.clone());
    assert!(reloaded.state().welcome_shown);
    assert!(reloaded.state().toolbar_hint_shown);
    assert!(reloaded.state().used_help_overlay);
}

#[cfg(unix)]
#[test]
fn rejected_persistence_disables_automatic_onboarding_for_the_session() {
    use std::os::unix::fs::symlink;

    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let target = tmp.path().join("real-onboarding.toml");
    fs::write(&target, "version = 5\n").expect("seed should be writable");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    fs::create_dir_all(path.parent().expect("onboarding path has a parent"))
        .expect("state directory should be writable");
    symlink(&target, &path).expect("test symlink should be created");

    let mut store = OnboardingStore::load_from_path(path);
    store.state_mut().first_run_completed = true;

    assert!(
        store.save().is_err(),
        "symlink rejection must reach the caller"
    );
    assert!(
        !store.persistence_available(),
        "a process that cannot remember an acknowledgement must not show automatic onboarding"
    );
}

#[test]
fn unwritable_state_location_disables_automatic_onboarding_for_the_session() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let blocker = tmp.path().join("not-a-directory");
    fs::write(&blocker, "occupied").expect("blocker file should be created");
    let path = blocker.join(ONBOARDING_FILE);
    let mut store = OnboardingStore::load_from_path(path);

    assert!(
        store.begin_session(true).is_err(),
        "a state path below a regular file cannot be persisted"
    );
    assert!(!store.persistence_available());
    assert!(
        !store.state().first_run_active(),
        "an unreadable state location must recover suppressively as well as disabling hints"
    );
}

#[test]
fn startup_notice_acknowledgement_survives_reload_and_is_content_specific() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    let mut store = OnboardingStore::load_from_path(path.clone());

    assert!(!store.startup_notice_acknowledged("skipped-default:f2-cycle"));
    store
        .acknowledge_startup_notice("skipped-default:f2-cycle")
        .expect("acknowledgement should persist");

    let reloaded = OnboardingStore::load_from_path(path);
    assert!(reloaded.startup_notice_acknowledged("skipped-default:f2-cycle"));
    assert!(
        !reloaded.startup_notice_acknowledged("skipped-default:new-binding"),
        "a changed diagnostic must be eligible for one new notice"
    );
}

#[test]
fn deferred_hint_cap_survives_repeated_process_launches() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);

    for expected_count in 0..DEFERRED_HINT_REPEAT_MAX {
        let mut store = OnboardingStore::load_from_path(path.clone());
        store.state_mut().first_run_completed = true;
        store
            .begin_session(true)
            .expect("session state should persist");
        assert!(!store.state().hint_zoom_chip_shown);
        assert_eq!(store.state().hint_zoom_chip_count, expected_count);

        store.state_mut().hint_zoom_chip_shown = true;
        store.state_mut().hint_zoom_chip_count += 1;
        store.save().expect("shown hint should persist");
    }

    let mut capped = OnboardingStore::load_from_path(path);
    capped
        .begin_session(true)
        .expect("capped state should remain writable");
    assert!(capped.state().hint_zoom_chip_shown);
    assert_eq!(
        capped.state().hint_zoom_chip_count,
        DEFERRED_HINT_REPEAT_MAX
    );
}

#[test]
fn using_surface_features_stops_their_tips_from_rearming() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    let mut store = OnboardingStore::load_from_path(path);
    let state = store.state_mut();
    state.first_run_completed = true;
    state.hint_status_bar_shown = true;
    state.hint_zoom_chip_shown = true;
    state.hint_canvas_popover_shown = true;
    state.used_board_picker = true;
    state.used_zoom_control = true;
    state.used_canvas_popover = true;

    store
        .begin_session(true)
        .expect("surface usage should persist");

    assert!(store.state().hint_status_bar_shown);
    assert!(store.state().hint_zoom_chip_shown);
    assert!(store.state().hint_canvas_popover_shown);
}

#[test]
fn acknowledging_one_tip_survives_reload_without_suppressing_other_tips() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    let mut store = OnboardingStore::load_from_path(path.clone());

    store
        .acknowledge_tip(crate::domain::OnboardingTip::StatusBar)
        .expect("tip acknowledgement should persist");

    let reloaded = OnboardingStore::load_from_path(path);
    assert!(reloaded.state().hint_status_bar_shown);
    assert_eq!(
        reloaded.state().hint_status_bar_count,
        DEFERRED_HINT_REPEAT_MAX
    );
    assert_eq!(reloaded.state().hint_zoom_chip_count, 0);
}

#[cfg(unix)]
#[test]
fn failed_tip_acknowledgement_suppresses_this_session_without_claiming_persistence() {
    use std::os::unix::fs::symlink;

    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let target = tmp.path().join("real-onboarding.toml");
    fs::write(
        &target,
        format!("version = {ONBOARDING_VERSION}\nfirst_run_completed = true\n"),
    )
    .expect("seed should be writable");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    fs::create_dir_all(path.parent().expect("onboarding path has a parent"))
        .expect("state directory should be writable");
    symlink(&target, &path).expect("test symlink should be created");
    let mut store = OnboardingStore::load_from_path(path.clone());

    assert!(
        store
            .acknowledge_tip(crate::domain::OnboardingTip::ZoomChip)
            .is_err()
    );
    assert!(store.state().hint_zoom_chip_shown);
    assert_eq!(store.state().hint_zoom_chip_count, DEFERRED_HINT_REPEAT_MAX);
    assert!(!store.persistence_available());

    let reloaded = OnboardingStore::load_from_path(path);
    assert_eq!(
        reloaded.state().hint_zoom_chip_count,
        0,
        "the rejected write must not alter the target file"
    );
}

#[test]
fn disabled_automatic_guidance_does_not_activate_first_run() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    let mut store = OnboardingStore::load_from_path(path);

    store
        .begin_session(false)
        .expect("disabled preference should still persist session state");

    assert!(store.state().active_step.is_none());
    assert!(!store.state().first_run_completed);
}

#[test]
fn onboarding_recovers_from_parse_error() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create onboarding dir");
    }
    fs::write(&path, "not = [toml").expect("write invalid toml");

    let store = OnboardingStore::load_from_path(path.clone());
    assert!(store.state().welcome_shown);
    assert!(store.state().first_run_completed);
    assert!(path.exists());

    let backup_found = fs::read_dir(path.parent().expect("parent dir"))
        .expect("read onboarding dir")
        .filter_map(|entry| entry.ok())
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("onboarding.bak")
        });
    assert!(backup_found);

    let contents = fs::read_to_string(&path).expect("read recovered file");
    let state: OnboardingState = toml::from_str(&contents).expect("recovered file should parse");
    assert!(state.welcome_shown);
    assert!(state.first_run_completed);
}

#[test]
fn onboarding_version_bump_saves() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create onboarding dir");
    }
    let seed = "version = 0\nwelcome_shown = true\ntoolbar_hint_shown = false\n";
    fs::write(&path, seed).expect("write seed");

    let store = OnboardingStore::load_from_path(path.clone());
    assert!(store.state().welcome_shown);
    assert_eq!(store.state().version, ONBOARDING_VERSION);
    assert!(store.state().first_run_completed);

    let contents = fs::read_to_string(&path).expect("read bumped file");
    let state: OnboardingState = toml::from_str(&contents).expect("bumped file should parse");
    assert_eq!(state.version, ONBOARDING_VERSION);
    assert!(state.welcome_shown);
    assert!(state.first_run_completed);
}

#[test]
fn v3_file_migrates_to_current_version_preserving_completion() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create onboarding dir");
    }
    // A v3 file that finished the old first-run flow. It has none of the v4
    // first-run/coach fields; migration must bump the version and the new
    // fields must default sensibly (serde defaults) without re-running setup.
    let seed = "\
version = 3
welcome_shown = true
toolbar_hint_shown = true
first_run_completed = true
first_run_background_mode_prompted = true
used_help_overlay = true
used_command_palette = true
";
    fs::write(&path, seed).expect("write v3 seed");

    let store = OnboardingStore::load_from_path(path.clone());
    assert_eq!(store.state().version, ONBOARDING_VERSION);
    assert!(store.state().first_run_completed);
    assert!(store.state().active_step.is_none());
    // New fields default off — the migration does not fabricate progress.
    assert!(!store.state().first_color_done);
    assert!(!store.state().first_thickness_done);
    assert!(!store.state().radial_flick_done);
    assert!(!store.state().coach_hint_shown);
    assert_eq!(store.state().coach_hint_count, 0);

    // The bumped file round-trips through a reload unchanged.
    let reloaded = OnboardingStore::load_from_path(path);
    assert_eq!(reloaded.state().version, ONBOARDING_VERSION);
    assert!(reloaded.state().first_run_completed);
    assert!(reloaded.state().used_command_palette);
}

#[test]
fn completed_pre_v6_profile_does_not_receive_new_surface_tips() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create onboarding dir");
    }
    // A v5 file is the state written by the release that introduced the M9
    // surface hints. Upgrading must preserve the user's completed-onboarding
    // expectation even when those fields were never durably advanced.
    let seed = "\
version = 5
welcome_shown = true
toolbar_hint_shown = true
first_run_completed = true
first_run_background_mode_prompted = true
used_help_overlay = true
used_command_palette = true
coach_hint_count = 1
";
    fs::write(&path, seed).expect("write v5 seed");

    let store = OnboardingStore::load_from_path(path.clone());
    assert_eq!(store.state().version, ONBOARDING_VERSION);
    assert_eq!(ONBOARDING_VERSION, 6);
    assert!(store.state().first_run_completed);
    assert!(store.state().hint_status_bar_shown);
    assert_eq!(
        store.state().hint_status_bar_count,
        DEFERRED_HINT_REPEAT_MAX
    );
    assert!(store.state().hint_zoom_chip_shown);
    assert_eq!(store.state().hint_zoom_chip_count, DEFERRED_HINT_REPEAT_MAX);
    assert!(store.state().hint_canvas_popover_shown);
    assert_eq!(
        store.state().hint_canvas_popover_count,
        DEFERRED_HINT_REPEAT_MAX
    );
    // Prior coach bookkeeping is preserved across the bump.
    assert_eq!(store.state().coach_hint_count, 1);

    // The bumped file round-trips through a reload unchanged.
    let reloaded = OnboardingStore::load_from_path(path);
    assert_eq!(reloaded.state().version, ONBOARDING_VERSION);
    assert_eq!(
        reloaded.state().hint_status_bar_count,
        DEFERRED_HINT_REPEAT_MAX
    );
}

#[test]
fn m9_surface_hint_fields_persist_and_reconcile() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    let mut store = OnboardingStore::load_from_path(path.clone());
    store.state_mut().hint_status_bar_count = 2;
    store.state_mut().hint_zoom_chip_count = 1;
    store.state_mut().hint_canvas_popover_count = 3;
    store.save().expect("surface hint counters should persist");

    let reloaded = OnboardingStore::load_from_path(path.clone());
    assert_eq!(reloaded.state().hint_status_bar_count, 2);
    assert_eq!(reloaded.state().hint_zoom_chip_count, 1);
    assert_eq!(reloaded.state().hint_canvas_popover_count, 3);

    // A hand-written file where a surface hint's `*_shown` flag was set but the
    // count is still zero reconciles the count up to 1, mirroring the existing
    // help/palette/coach bookkeeping.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create onboarding dir");
    }
    let seed = format!(
        "version = {ONBOARDING_VERSION}\nfirst_run_completed = true\nhint_status_bar_shown = true\n"
    );
    fs::write(&path, seed).expect("write shown-without-count seed");
    let store = OnboardingStore::load_from_path(path);
    assert!(store.state().hint_status_bar_shown);
    assert_eq!(store.state().hint_status_bar_count, 1);
}

#[test]
fn coach_bookkeeping_reconciles_capped_count_to_learned_flag() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create onboarding dir");
    }
    // A capped coach count without the learned flag must reconcile to learned.
    let seed = format!(
        "version = {ONBOARDING_VERSION}\nfirst_run_completed = true\ncoach_hint_count = {DEFERRED_HINT_REPEAT_MAX}\n"
    );
    fs::write(&path, seed).expect("write coach seed");

    let store = OnboardingStore::load_from_path(path);
    assert!(store.state().coach_hint_shown);
    assert_eq!(store.state().coach_hint_count, DEFERRED_HINT_REPEAT_MAX);
}
