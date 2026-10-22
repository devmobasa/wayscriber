use std::env;
use std::process::{Command, ExitStatus};

pub(super) fn run_isolated_gtk_test(child_env: &str, test_name: &str) -> ExitStatus {
    Command::new(env::current_exe().expect("test binary"))
        .arg(test_name)
        .arg("--exact")
        .arg("--test-threads=1")
        .env(child_env, "1")
        .env("G_DEBUG", "fatal-criticals")
        .status()
        .expect("run isolated GTK popup capture test")
}
