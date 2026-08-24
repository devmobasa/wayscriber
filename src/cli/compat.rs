//! The token rules the hand-rolled parser had, kept as a scan of its own.
//!
//! `usage` binds the values; this module decides the terminals and the
//! rejections, because both are position-sensitive in a way a parser that reads
//! the whole command line at once cannot reproduce: `--help --clear-session=false`
//! is help, not an error, and `--unknown --mode=` names `--unknown`.
//!
//! It is deliberately a second reading of argv, and that is a drift risk — the
//! `-é` case below was one. Everything it decides is read off the derived flag
//! table, and `src/cli/tests.rs` pins it against the `main` binary's wording.

use std::ffi::OsStr;

use super::{BUILTINS, Builtin, bundle_value_boundary, long_flag, long_flag_names, short_flag};

/// What a scan of the tokens concluded.
pub(super) enum Scan {
    Run,
    Help,
    Version,
}

/// Walk argv the way the hand-rolled parser did, reporting the first terminal
/// or the first bad token.
///
/// `usage` still binds the values; this pass exists for the two things a parser
/// that reads the whole command line at once cannot reproduce. Both are
/// position-sensitive: `--help --clear-session=false` is help, not an error,
/// and `--unknown --mode=` names `--unknown`. It also owns the wording, which
/// is why the rules below mirror the old `match` arm for arm — including its
/// quirk of naming a standalone `-m` after its long form and a bundled one
/// after its letter.
///
/// Every rule reads the derived flag table, so adding a flag changes nothing.
pub(super) fn scan_tokens(argv: &[&OsStr]) -> Result<Scan, String> {
    let mut index = 0;
    while index < argv.len() {
        let Some(token) = argv[index].to_str() else {
            index += 1;
            continue;
        };

        if let Some(builtin) = BUILTINS.iter().find(|builtin| {
            token == format!("--{}", builtin.long) || token == format!("-{}", builtin.short)
        }) {
            return Ok(terminal(builtin));
        }

        if let Some(body) = token.strip_prefix("--") {
            index = scan_long(argv, index, token, body)?;
            continue;
        }

        if let Some(letters) = token.strip_prefix('-').filter(|rest| !rest.is_empty()) {
            match scan_shorts(argv, index, token, letters)? {
                ShortStep::Next(next) => index = next,
                ShortStep::Terminal(scan) => return Ok(scan),
            }
            continue;
        }

        // A bare `-`, or a word where a flag was expected: wayscriber takes no
        // positional arguments.
        return Err(unknown_argument(token));
    }
    Ok(Scan::Run)
}

fn terminal(builtin: &Builtin) -> Scan {
    if builtin.long == "help" {
        Scan::Help
    } else {
        Scan::Version
    }
}

/// One `--long` token; returns the index of the next one.
fn scan_long(argv: &[&OsStr], index: usize, token: &str, body: &str) -> Result<usize, String> {
    if let Some((name, value)) = body.split_once('=') {
        return match long_flag(name) {
            Some(flag) if flag.takes_value => {
                if value.is_empty() {
                    Err(format!("--{name} requires a value"))
                } else {
                    Ok(index + 1)
                }
            }
            // A switch takes no value, so `--clear-session=false` is not a
            // spelling of it — and must not bind it.
            _ => Err(unknown_argument(token)),
        };
    }

    match long_flag(body) {
        Some(flag) if flag.takes_value => {
            value_after(argv, index + 1, &format!("--{body}"))?;
            Ok(index + 2)
        }
        Some(_) => Ok(index + 1),
        None => Err(unknown_argument(token)),
    }
}

/// Where a bundle left the scan: at the next token, or at a terminal.
enum ShortStep {
    Next(usize),
    Terminal(Scan),
}

/// One `-abc` token. A bundle can hold `h` or `V`, and the old parser stopped
/// there rather than reading the rest.
fn scan_shorts(
    argv: &[&OsStr],
    index: usize,
    token: &str,
    letters: &str,
) -> Result<ShortStep, String> {
    // Byte length, as the old parser's `arg.len() > 2` did: `-é` is two bytes
    // behind a dash, so it counted as a bundle and named the letter.
    let bundled = token.len() > 2;

    // Where the bundle stops being flags and starts being a value, from the one
    // helper that decides it. Letters past this point are the value's own
    // characters: `-mam=` is a board id of `am=`, not another `-m`.
    let boundary = bundle_value_boundary(letters);
    let flags_end = boundary.map_or(letters.len(), |(_, value_start)| value_start);

    for letter in letters[..flags_end].chars() {
        if let Some(builtin) = BUILTINS.iter().find(|builtin| builtin.short == letter) {
            return Ok(ShortStep::Terminal(terminal(builtin)));
        }
        if short_flag(letter).is_none() {
            // A lone unknown short was never a bundle, so the old parser
            // reported the whole token; inside a bundle it named the letter.
            return Err(if bundled {
                format!("unknown short option '-{letter}'")
            } else {
                unknown_argument(token)
            });
        }
    }

    let Some((flag, value_start)) = boundary else {
        return Ok(ShortStep::Next(index + 1));
    };

    // The value is whatever follows that letter, or the next argument.
    let letter = letters[..value_start]
        .chars()
        .next_back()
        .unwrap_or_default();
    let rest = &letters[value_start..];
    if rest.is_empty() {
        // Named after the long form when the letter stood alone, after the
        // letter when it came in a bundle. The old parser did both.
        let name = if bundled {
            format!("-{letter}")
        } else {
            format!("--{}", flag.longs.first().copied().unwrap_or(flag.name))
        };
        value_after(argv, index + 1, &name)?;
        return Ok(ShortStep::Next(index + 2));
    }
    if rest.strip_prefix('=').unwrap_or(rest).is_empty() {
        return Err(format!("-{letter} requires a value"));
    }
    Ok(ShortStep::Next(index + 1))
}

/// The value at `index`, refusing a missing one and one that looks like a flag.
fn value_after(argv: &[&OsStr], index: usize, name: &str) -> Result<(), String> {
    let missing = argv
        .get(index)
        .and_then(|value| value.to_str())
        .is_none_or(|value| value.starts_with('-'));
    if missing {
        return Err(format!("{name} requires a value"));
    }
    Ok(())
}

/// Levenshtein distance, two rows at a time.
fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0usize; right.len() + 1];

    for (row, left_char) in left.chars().enumerate() {
        current[0] = row + 1;
        for (column, right_char) in right.iter().enumerate() {
            let substitution = usize::from(left_char != *right_char);
            current[column + 1] = (previous[column] + substitution)
                .min(previous[column + 1] + 1)
                .min(current[column] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right.len()]
}

/// `unknown argument '--activ'`, with `(did you mean '--active'?)` when one of
/// the declared flags is close enough to be what was meant.
fn unknown_argument(token: &str) -> String {
    match did_you_mean(token) {
        Some(suggestion) => format!("unknown argument '{token}' (did you mean '{suggestion}'?)"),
        None => format!("unknown argument '{token}'"),
    }
}

/// The closest long flag to `token`, when one is close enough to be a typo.
fn did_you_mean(token: &str) -> Option<String> {
    let typed = token.trim_start_matches('-');
    if typed.is_empty() {
        return None;
    }

    let mut best: Option<(usize, String)> = None;
    for long in long_flag_names() {
        let distance = edit_distance(typed, &long);
        if best.as_ref().is_none_or(|(seen, _)| distance < *seen) {
            best = Some((distance, format!("--{long}")));
        }
    }

    best.filter(|(distance, _)| *distance <= typed.len().div_ceil(3).max(1))
        .map(|(_, flag)| flag)
}
