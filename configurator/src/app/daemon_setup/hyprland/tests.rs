use super::*;
use std::env;
use std::sync::Mutex;
use wayscriber::env_vars::HOME_ENV;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn render_light_controls_quotes_binary_with_spaces() {
    let rendered = render_light_controls(Path::new("/tmp/My Apps/wayscriber"));
    assert!(rendered.contains("'/tmp/My Apps/wayscriber' --light-toggle"));
    assert!(rendered.contains("'/tmp/My Apps/wayscriber' --light-draw-toggle"));
    assert!(rendered.contains("'/tmp/My Apps/wayscriber' --light-draw-on"));
    assert!(rendered.contains("'/tmp/My Apps/wayscriber' --light-draw-off"));
}

#[test]
fn render_light_controls_escapes_single_quotes() {
    let rendered = render_light_controls(Path::new("/tmp/O'Brien/wayscriber"));
    assert!(rendered.contains("'/tmp/O'\\''Brien/wayscriber' --light-toggle"));
}

#[test]
fn render_light_controls_unbinds_default_keys_before_binding() {
    let rendered = render_light_controls(Path::new("/tmp/wayscriber"));

    let unbind_l = rendered.find("unbind = SUPER ALT, L").unwrap();
    let bind_l = rendered.find("\nbind = SUPER ALT, L").unwrap();
    assert!(unbind_l < bind_l);

    let unbind_d = rendered.find("unbind = SUPER ALT, D").unwrap();
    let bind_d = rendered.find("\nbind = SUPER ALT, D").unwrap();
    assert!(unbind_d < bind_d);

    let unbind_f = rendered.find("unbind = SUPER ALT, F").unwrap();
    let bind_f = rendered.find("\nbind = SUPER ALT, F").unwrap();
    let bindr_f = rendered.find("bindr = SUPER ALT, F").unwrap();
    assert!(unbind_f < bind_f);
    assert!(unbind_f < bindr_f);
}

#[test]
fn ensure_source_line_appends_once() {
    let source_line = "source = /tmp/hypr/wayscriber-light.conf";
    let (updated, changed) = ensure_source_line("source = ~/.config/hypr/base.conf\n", source_line);
    assert!(changed);
    assert!(updated.contains(LIGHT_CONTROLS_COMMENT));
    assert!(updated.contains(source_line));

    let (again, changed_again) = ensure_source_line(&updated, source_line);
    assert!(!changed_again);
    assert_eq!(again.matches(source_line).count(), 1);
}

#[test]
fn has_source_line_ignores_comments_and_spacing() {
    let source_line = "source = /tmp/hypr/wayscriber-light.conf";
    assert!(!has_source_line(
        "# source = /tmp/hypr/wayscriber-light.conf\n",
        source_line
    ));
    assert!(has_source_line(
        "  source   =   /tmp/hypr/wayscriber-light.conf  \n",
        source_line
    ));
}

#[test]
fn has_source_line_matches_quoted_and_inline_commented_targets() {
    let source_line = "source = /tmp/hypr/wayscriber-light.conf";
    assert!(has_source_line(
        "source = '/tmp/hypr/wayscriber-light.conf' # installed by wayscriber\n",
        source_line
    ));
    assert!(has_source_line(
        "source = \"/tmp/hypr/wayscriber-light.conf\" # installed by wayscriber\n",
        source_line
    ));
}

#[test]
fn has_source_line_matches_tilde_target() {
    let _guard = ENV_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = crate::test_temp::tempdir().unwrap();
    let home = tmp.path();
    let prev_home = env::var_os(HOME_ENV);
    unsafe {
        env::set_var(HOME_ENV, home);
    }

    let absolute = home
        .join(".config")
        .join("hypr")
        .join(LIGHT_CONTROLS_INCLUDE);
    let source_line = format!("source = {}", absolute.display());
    assert!(has_source_line(
        "source = ~/.config/hypr/wayscriber-light.conf # already sourced\n",
        &source_line
    ));

    match prev_home {
        Some(value) => unsafe { env::set_var(HOME_ENV, value) },
        None => unsafe { env::remove_var(HOME_ENV) },
    }
}

#[test]
fn write_light_controls_writes_include_and_sources_existing_main() {
    let tmp = crate::test_temp::tempdir().unwrap();
    let hypr_dir = tmp.path().join(HYPRLAND_DIR);
    fs::create_dir_all(&hypr_dir).unwrap();
    let main = hypr_dir.join(MAIN_CONFIG);
    fs::write(&main, "source = ~/.config/hypr/base.conf\n").unwrap();

    let result = write_light_controls(tmp.path(), Path::new("/tmp/My Apps/wayscriber")).unwrap();

    assert!(result.source_configured);
    assert!(result.source_updated);
    assert!(result.include_path.exists());
    let include = fs::read_to_string(&result.include_path).unwrap();
    assert!(include.contains("'/tmp/My Apps/wayscriber' --light-toggle"));
    assert!(include.contains("unbind = SUPER ALT, L"));
    let main_content = fs::read_to_string(&main).unwrap();
    assert!(main_content.contains(&result.source_line));
}

#[test]
fn write_light_controls_is_idempotent_for_existing_source() {
    let tmp = crate::test_temp::tempdir().unwrap();
    let hypr_dir = tmp.path().join(HYPRLAND_DIR);
    fs::create_dir_all(&hypr_dir).unwrap();
    let main = hypr_dir.join(MAIN_CONFIG);
    let include = light_controls_include_path(tmp.path());
    let source_line = source_line_for_include(&include);
    fs::write(&main, format!("{source_line}\n")).unwrap();

    let result = write_light_controls(tmp.path(), Path::new("/tmp/wayscriber")).unwrap();

    assert!(result.source_configured);
    assert!(!result.source_updated);
    let main_content = fs::read_to_string(&main).unwrap();
    assert_eq!(main_content.matches(&source_line).count(), 1);
}

#[test]
fn write_light_controls_handles_missing_main_config() {
    let tmp = crate::test_temp::tempdir().unwrap();

    let result = write_light_controls(tmp.path(), Path::new("/tmp/wayscriber")).unwrap();

    assert!(result.include_path.exists());
    assert!(!result.source_configured);
    assert!(!result.source_updated);
}
