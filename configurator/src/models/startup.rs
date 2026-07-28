use std::ffi::OsString;
use std::fmt::Write as _;

use wayscriber::configurator_destination::{
    ConfiguratorDestination, ConfiguratorScreen, OPEN_FLAG,
};

/// An argument this build could not act on.
///
/// Kept rather than dropped so a launcher that names a screen this build does
/// not have shows up as a message in the window instead of as a configurator
/// that silently opened somewhere else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StartupArgProblem {
    /// `--open` named a destination this build does not understand.
    UnknownDestination(String),
    /// `--open` was the last argument, with nothing to open.
    MissingDestination,
    /// An argument with no meaning to this build at all.
    UnrecognizedArgument(String),
}

impl StartupArgProblem {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::UnknownDestination(raw) => {
                format!("Unknown destination: {raw}. Showing the usual first screen.")
            }
            Self::MissingDestination => {
                format!("{OPEN_FLAG} needs a destination. Showing the usual first screen.")
            }
            Self::UnrecognizedArgument(raw) => {
                format!("Ignored an unrecognized startup argument: {raw}.")
            }
        }
    }
}

/// What the process that started this configurator asked for.
///
/// The overlay and the tray exit or hide before launching, so this is the only
/// thing they can say about where the user wanted to land.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StartupRequest {
    destination: Option<ConfiguratorDestination>,
    problem: Option<StartupArgProblem>,
    help: bool,
}

impl StartupRequest {
    /// Reads a full `argv`, including the program name in the first slot.
    ///
    /// Hand-rolled to match the main crate's `cli.rs`; nothing here aborts,
    /// because a launcher from a different version is not a reason to refuse
    /// to show the user their configuration.
    pub(crate) fn from_args<I>(args: I) -> Self
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut request = Self::default();
        let mut args = args.into_iter().skip(1);

        while let Some(argument) = args.next() {
            let argument = match argument.into_string() {
                Ok(argument) => argument,
                // Not UTF-8, so it is not one of ours; name it as best the
                // platform allows.
                Err(raw) => {
                    request.note(StartupArgProblem::UnrecognizedArgument(
                        raw.to_string_lossy().into_owned(),
                    ));
                    continue;
                }
            };

            if argument == "-h" || argument == "--help" {
                request.help = true;
            } else if argument == OPEN_FLAG {
                match args.next() {
                    Some(value) => request.note_destination(&value.to_string_lossy()),
                    None => request.note(StartupArgProblem::MissingDestination),
                }
            } else if let Some(value) = equals_value(&argument) {
                request.note_destination(value);
            } else {
                request.note(StartupArgProblem::UnrecognizedArgument(argument));
            }
        }

        request
    }

    pub(crate) fn destination(&self) -> Option<&ConfiguratorDestination> {
        self.destination.as_ref()
    }

    pub(crate) fn problem(&self) -> Option<&StartupArgProblem> {
        self.problem.as_ref()
    }

    pub(crate) fn wants_help(&self) -> bool {
        self.help
    }

    fn note_destination(&mut self, value: &str) {
        match ConfiguratorDestination::parse(value) {
            Some(destination) => self.destination = Some(destination),
            None => self.note(StartupArgProblem::UnknownDestination(value.to_string())),
        }
    }

    /// Keeps the first problem only: the window shows one status line, and a
    /// launcher that got its arguments wrong got them wrong once.
    fn note(&mut self, problem: StartupArgProblem) {
        if self.problem.is_none() {
            self.problem = Some(problem);
        }
    }
}

/// The value of an `--open=<destination>` argument.
fn equals_value(argument: &str) -> Option<&str> {
    argument
        .strip_prefix(OPEN_FLAG)
        .and_then(|rest| rest.strip_prefix('='))
}

/// The text `--help` prints.
pub(crate) fn startup_usage() -> String {
    let mut usage = String::from(
        "Wayscriber Configurator\n\n\
         Usage: wayscriber-configurator [OPTIONS]\n\n\
         Options:\n  \
         -h, --help              Show this help and exit\n      \
         --open <DESTINATION>    Open a screen once the configuration loads\n\n\
         Destinations:\n",
    );
    for screen in ConfiguratorScreen::ALL {
        // `write!` to a String cannot fail; the result is dropped rather than
        // asserted so an infallibility claim stays out of the binary.
        let _ = writeln!(usage, "  {}", screen.as_arg());
    }
    usage.push_str(
        "\nAppend ?search=<TERM> to a destination to open it with the search box filled,\n\
         for example: --open drawing?search=Quick Colors\n",
    );
    usage
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        std::iter::once("wayscriber-configurator")
            .chain(values.iter().copied())
            .map(OsString::from)
            .collect()
    }

    #[test]
    fn no_arguments_asks_for_nothing() {
        let request = StartupRequest::from_args(args(&[]));

        assert_eq!(request, StartupRequest::default());
        assert_eq!(request.destination(), None);
        assert_eq!(request.problem(), None);
        assert!(!request.wants_help());
    }

    #[test]
    fn separate_open_value_parses() {
        let request = StartupRequest::from_args(args(&["--open", "ui/status-bar"]));

        assert_eq!(
            request.destination(),
            Some(&ConfiguratorDestination::new(
                ConfiguratorScreen::UiStatusBar
            ))
        );
        assert_eq!(request.problem(), None);
    }

    #[test]
    fn equals_form_parses_including_a_search_term_with_spaces() {
        let request = StartupRequest::from_args(args(&["--open=drawing?search=Quick Colors"]));

        assert_eq!(
            request.destination(),
            Some(&ConfiguratorDestination::with_search(
                ConfiguratorScreen::Drawing,
                "Quick Colors"
            ))
        );
        assert_eq!(request.problem(), None);
    }

    #[test]
    fn unknown_destination_is_reported_without_a_destination() {
        let request = StartupRequest::from_args(args(&["--open", "ui/nowhere"]));

        assert_eq!(request.destination(), None);
        assert_eq!(
            request.problem(),
            Some(&StartupArgProblem::UnknownDestination(
                "ui/nowhere".to_string()
            ))
        );
        assert!(request.problem().is_some_and(|problem| {
            problem
                .message()
                .contains("Unknown destination: ui/nowhere")
        }));
    }

    #[test]
    fn missing_open_value_is_reported() {
        let request = StartupRequest::from_args(args(&["--open"]));

        assert_eq!(request.destination(), None);
        assert_eq!(
            request.problem(),
            Some(&StartupArgProblem::MissingDestination)
        );
    }

    #[test]
    fn unknown_flags_are_reported_and_do_not_stop_parsing() {
        let request = StartupRequest::from_args(args(&["--frobnicate", "--open", "session"]));

        assert_eq!(
            request.destination(),
            Some(&ConfiguratorDestination::new(ConfiguratorScreen::Session))
        );
        assert_eq!(
            request.problem(),
            Some(&StartupArgProblem::UnrecognizedArgument(
                "--frobnicate".to_string()
            ))
        );
    }

    #[test]
    fn only_the_first_problem_is_kept() {
        let request = StartupRequest::from_args(args(&["--first", "--second"]));

        assert_eq!(
            request.problem(),
            Some(&StartupArgProblem::UnrecognizedArgument(
                "--first".to_string()
            ))
        );
    }

    #[test]
    fn help_is_recognized() {
        assert!(StartupRequest::from_args(args(&["--help"])).wants_help());
        assert!(StartupRequest::from_args(args(&["-h"])).wants_help());
    }

    #[test]
    fn usage_lists_every_destination() {
        let usage = startup_usage();

        for screen in ConfiguratorScreen::ALL {
            assert!(
                usage.contains(screen.as_arg()),
                "usage text omits {}",
                screen.as_arg()
            );
        }
    }
}
