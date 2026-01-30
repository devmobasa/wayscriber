use cfg_aliases::cfg_aliases;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    cfg_aliases! {
        // Single source of truth for feature aliases used in cfg attributes
        tablet: { feature = "tablet-input" },
    }

    println!("cargo:rerun-if-env-changed=WAYSCRIBER_RELEASE_VERSION");
    match env::var("WAYSCRIBER_RELEASE_VERSION") {
        Ok(release_version) if !release_version.is_empty() => {
            println!("cargo:rustc-env=WAYSCRIBER_RELEASE_VERSION={release_version}");
        }
        _ => {}
    }

    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());

    println!("cargo:rustc-env=WAYSCRIBER_GIT_HASH={hash}");

    if let Some(git_dir) = resolve_git_dir() {
        emit_rerun(&git_dir.join("HEAD"));
        emit_rerun(&git_dir.join("refs"));
        emit_rerun(&git_dir.join("packed-refs"));
    }
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
