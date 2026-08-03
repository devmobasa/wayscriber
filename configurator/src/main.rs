mod app;
mod messages;
mod models;
#[cfg(test)]
mod test_env;
#[cfg(test)]
mod test_temp;

use models::{StartupRequest, startup_usage};

fn main() {
    let startup = StartupRequest::from_args(std::env::args_os());
    if startup.wants_help() {
        print!("{}", startup_usage());
        return;
    }

    app::run(startup)
}
