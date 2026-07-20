use super::*;
use std::fs;

#[test]
fn onboarding_defaults_when_missing() {
    let tmp = crate::test_temp::tempdir().expect("tempdir should succeed");
    let path = tmp.path().join(ONBOARDING_DIR).join(ONBOARDING_FILE);
    let store = OnboardingStore::load_from_path(path.clone());
    assert!(!store.state().welcome_shown);
    assert!(!store.state().toolbar_hint_shown);
    assert!(!store.state().first_run_completed);
    assert!(store.state().active_step.is_none());

    store.save();
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
    store.save();

    let reloaded = OnboardingStore::load_from_path(path.clone());
    assert!(reloaded.state().welcome_shown);
    assert!(reloaded.state().toolbar_hint_shown);
    assert!(reloaded.state().used_help_overlay);
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
