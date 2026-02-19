use std::env;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug)]
pub(super) struct CommandCapture {
    pub(super) success: bool,
    pub(super) stdout: String,
    pub(super) stderr: String,
}

pub(super) fn command_available(program: &str) -> bool {
    find_in_path(program).is_some()
}

pub(super) fn find_in_path(binary_name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var)
        .map(|directory| directory.join(binary_name))
        .find(|path| path.exists())
}

pub(super) fn run_command_checked(program: &str, args: &[&str]) -> Result<CommandCapture, String> {
    let capture = run_command(program, args)?;
    if capture.success {
        return Ok(capture);
    }
    Err(format_command_failure(program, args, &capture))
}

pub(super) fn run_command(program: &str, args: &[&str]) -> Result<CommandCapture, String> {
    let output = Command::new(program).args(args).output().map_err(|err| {
        format!(
            "Failed to execute `{}` with args [{}]: {}",
            program,
            args.join(" "),
            err
        )
    })?;
    Ok(CommandCapture {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn format_command_failure(program: &str, args: &[&str], capture: &CommandCapture) -> String {
    format!(
        "`{}` failed with args [{}]\nstdout: {}\nstderr: {}",
        program,
        args.join(" "),
        capture.stdout.trim(),
        capture.stderr.trim()
    )
}
