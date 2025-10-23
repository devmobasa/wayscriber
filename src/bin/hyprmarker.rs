use std::env;
use std::process::{Command, exit};

fn main() {
    match run_alias() {
        Ok(code) => exit(code),
        Err(err) => {
            eprintln!("Failed to launch wayscriber: {err}");
            exit(1);
        }
    }
}

fn run_alias() -> Result<i32, Box<dyn std::error::Error>> {
    let alias_exe = env::current_exe()?;
    let target_exe = alias_exe.with_file_name("wayscriber");

    if !target_exe.exists() {
        return Err(format!("expected companion binary at {}", target_exe.display()).into());
    }

    let mut command = Command::new(target_exe);
    command.args(env::args_os().skip(1));
    command.env(
        wayscriber::legacy::LEGACY_ALIAS_ENV,
        alias_exe
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("hyprmarker"),
    );

    let status = command.status()?;
    Ok(status.code().unwrap_or(1))
}
