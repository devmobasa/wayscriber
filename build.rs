use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=WAYSCRIBER_RELEASE_VERSION");
    match env::var("WAYSCRIBER_RELEASE_VERSION") {
        Ok(release_version) if !release_version.is_empty() => {
            println!("cargo:rustc-env=WAYSCRIBER_RELEASE_VERSION={release_version}");
        }
        _ => {}
    }

    // Packagers set this so the About window and update notice can point at
    // the update instructions that actually apply (apt, dnf, AUR, Nix, ...).
    println!("cargo:rerun-if-env-changed=WAYSCRIBER_INSTALL_SOURCE");
    match env::var("WAYSCRIBER_INSTALL_SOURCE") {
        Ok(install_source) if !install_source.is_empty() => {
            println!("cargo:rustc-env=WAYSCRIBER_INSTALL_SOURCE={install_source}");
        }
        _ => {}
    }

    // Distributions that forbid outbound version checks can compile the check
    // out entirely, so no config or environment mistake can re-enable it. This
    // is deliberately *not* the runtime opt-out variable
    // (`WAYSCRIBER_DISABLE_UPDATE_CHECK`): having it exported in a developer's
    // shell must not silently change what the binary can do.
    println!("cargo:rerun-if-env-changed=WAYSCRIBER_NO_UPDATE_CHECK");
    match env::var("WAYSCRIBER_NO_UPDATE_CHECK") {
        Ok(value) if !value.is_empty() => {
            println!("cargo:rustc-env=WAYSCRIBER_NO_UPDATE_CHECK={value}");
        }
        _ => {}
    }

    let hash = git_output(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());

    println!("cargo:rustc-env=WAYSCRIBER_GIT_HASH={hash}");

    // Commit date rather than build time: it keeps rebuilds reproducible and
    // still answers "how old is this binary?" in the About window.
    let build_date = git_output(&["log", "-1", "--format=%cs"]).unwrap_or_default();
    println!("cargo:rustc-env=WAYSCRIBER_BUILD_DATE={build_date}");

    if let Some(git_dir) = resolve_git_dir() {
        emit_rerun(&git_dir.join("HEAD"));
        emit_rerun(&git_dir.join("refs"));
        emit_rerun(&git_dir.join("packed-refs"));
    }
}

fn git_output(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

#[allow(clippy::collapsible_if)]
fn resolve_git_dir() -> Option<PathBuf> {
    if let Some(from_env) = env::var_os("GIT_DIR") {
        return Some(PathBuf::from(from_env));
    }

    let dot_git = PathBuf::from(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }

    if dot_git.is_file() {
        if let Ok(contents) = fs::read_to_string(&dot_git) {
            if let Some(rest) = contents.strip_prefix("gitdir:") {
                let mut resolved = PathBuf::from(rest.trim());
                if resolved.is_relative() {
                    if let Some(parent) = dot_git.parent() {
                        resolved = parent.join(resolved);
                    }
                }
                return Some(resolved);
            }
        }
    }

    None
}

#[allow(clippy::collapsible_if)]
fn emit_rerun(path: &Path) {
    if path.exists() {
        if let Some(display) = path.to_str() {
            println!("cargo:rerun-if-changed={display}");
        }
    }
}
