//! Answering `__complete_word__` for the generated shell scripts.
//!
//! `usage`'s own pipeline does the work — `split` for the shell's quoting,
//! `complete` for the candidates, `render` for the record shape each script
//! reads. What lives here are the corrections applied to the *typed* answer in
//! between, each for something the derived spec cannot express. They are listed
//! on [`completion_answer`].

use std::ffi::OsString;

use usage_rs as usage;

use super::{BUILTINS, Cli, bundle_value_boundary, long_flag};

/// Answer a `__complete_word__` request from the generated shell scripts.
///
/// This is `usage`'s own pipeline — `split` for the shell's quoting, `complete`
/// for the candidates, `render` for the protocol each script reads — with four
/// corrections applied to the *typed* answer in between:
///
/// 1. `usage` answers `-h`/`--help` and `-V`/`--version` itself and keeps them
///    out of the tables, so nothing would ever offer them.
/// 2. It falls back to path completion wherever it has nothing to say, but
///    wayscriber takes no positional arguments: paths belong only to a flag
///    that asked for them.
/// 3. It does not look inside an attached value, so `--mode=whi` and `-amwhi`
///    are re-expressed as the separated spelling. What comes back depends on
///    what the shell will replace: bash broke the word at `=` and gets the
///    value alone; zsh, fish and PowerShell get the whole token, so their
///    generated scripts isolate the value before native path completion; Nu's
///    external-completer protocol needs a narrow direct-listing fallback.
/// 4. It offers the first letter of a bundle on its own, which would delete the
///    rest of what was typed.
///
/// Doing this on `Completions` rather than on rendered text is what keeps the
/// quoting and the per-shell record shapes `usage`'s problem rather than ours.
pub(super) fn completion_answer(argv: &[OsString]) -> Option<String> {
    let request = argv.first()?.to_str()?;
    if request != "__complete_word__" {
        return None;
    }
    let shell = usage::complete::Shell::from_name(&named_argument(argv, "--shell")?)?;
    let line = named_argument(argv, "--line")?;

    // The scripts cut the line at the cursor themselves, so its end is the
    // cursor. See the comment in the generated script on why it happens there.
    let split = usage::complete::split(&line, line.len(), shell);

    // `complete` does not look inside an attached value, so the word is rewritten
    // into the separated spelling it is equivalent to and the answer computed for
    // the value's position.
    let attached = attached_value_split(&split);
    let (effective, prefix) = match &attached {
        Some((split, prefix)) => (split, Some(prefix.as_str())),
        None => (&split, None),
    };

    let mut answer = usage::complete::complete(Cli::spec(), effective);
    // Whether a candidate must carry the attached prefix depends on what the
    // shell will replace. bash breaks words at `=` (it is in `COMP_WORDBREAKS`),
    // so its current word is the value alone and a prefixed candidate would land
    // *over* it as `--session-file=--session-file=…`. The others hand over the
    // whole token, so the prefix has to come back.
    if let Some(prefix) = prefix.filter(|_| !breaks_words_at_equals(shell)) {
        for candidate in &mut answer.candidates {
            candidate.value = format!("{prefix}{}", candidate.value);
        }
        // Nu's external-completer API can delegate to file completion only for
        // the original last span; it cannot substitute the value after `=` and
        // then restore the flag. The generated zsh, fish and PowerShell scripts
        // are adapted below to do that in their native path completers. Nu needs
        // this narrow fallback until its protocol can express the same handoff.
        if matches!(shell, usage::complete::Shell::Nu) && answer.files.take().is_some() {
            answer
                .candidates
                .extend(nu_path_candidates(&effective.prefix, prefix));
        }
    }

    if !wants_paths(&split) {
        answer.files = None;
    }

    // A bundle is already several flags; `usage` still offers the first letter
    // alone, and accepting it would delete the rest of what was typed. Nothing
    // shorter than the word can be a completion of it.
    let word = split
        .words
        .get(split.cword)
        .map(String::as_str)
        .unwrap_or("");
    if attached.is_none() && is_bundle(word) {
        answer
            .candidates
            .retain(|candidate| candidate.value.len() >= word.len());
    }
    if attached.is_none() && split.prefix.starts_with('-') {
        for builtin in &BUILTINS {
            for candidate in [format!("--{}", builtin.long), format!("-{}", builtin.short)] {
                if candidate.starts_with(&split.prefix)
                    && !answer.candidates.iter().any(|seen| seen.value == candidate)
                {
                    answer
                        .candidates
                        .push(usage::spec::Candidate::described(candidate, builtin.help));
                }
            }
        }
    }

    Some(usage::complete::render(&answer, shell))
}

/// The word being completed re-expressed as the separated spelling, when it
/// carries its value attached (`--mode=whi`, `-mwhi`, `-amwhi`), plus the prefix
/// that has to go back onto every candidate so it replaces the whole word.
///
/// The bundle is walked to the first letter that takes a value, the way the
/// parser walks it: in `-amwhi` the value belongs to `m`, and offering `-a`
/// here would replace what the user typed.
fn attached_value_split(
    split: &usage::complete::Split,
) -> Option<(usage::complete::Split, String)> {
    let word = split.words.get(split.cword)?;

    let (flag, value, prefix) = match word.strip_prefix("--") {
        Some(body) => {
            let (name, value) = body.split_once('=')?;
            let flag = long_flag(name).filter(|flag| flag.takes_value)?;
            (flag, value.to_string(), format!("--{name}="))
        }
        None => {
            let letters = word
                .strip_prefix('-')
                .filter(|rest| !rest.starts_with('-'))?;
            let (flag, value_start) = bundle_value_boundary(letters)?;
            let rest = &letters[value_start..];
            if rest.is_empty() {
                return None;
            }
            let value = rest.strip_prefix('=').unwrap_or(rest).to_string();
            let prefix = word[..word.len() - value.len()].to_string();
            (flag, value, prefix)
        }
    };

    // The rewritten word is the canonical long form, which is what `complete`
    // walks; the prefix carries the spelling the user actually typed.
    let mut words = split.words.clone();
    words[split.cword] = format!("--{}", flag.longs.first().copied().unwrap_or(flag.name));
    words.insert(split.cword + 1, value.clone());

    Some((
        usage::complete::Split {
            words,
            cword: split.cword + 1,
            prefix: value,
        },
        prefix,
    ))
}

/// The completion script for `shell`, generated from the derived spec.
///
/// usage-rs 6.1.1 hands attached paths to the shell as whole words outside
/// bash, so the native completer looks for a literal path beginning with
/// `--session-file=`. The dependency is exact-pinned; these checked adaptations
/// isolate the value and restore the prefix in each shell. If upstream changes
/// one of the scripts, generation fails rather than silently dropping paths.
pub(super) fn completion_script(shell: &str) -> Result<String, String> {
    use usage::complete::Shell;

    // `Shell::from_name` is the same seven spellings the `choices(...)` list
    // declares, so the mapping is not written out a second time here.
    let shell = Shell::from_name(shell).ok_or_else(|| {
        format!("unknown shell '{shell}'; expected bash, zsh, fish, nu, or powershell")
    })?;
    adapt_completion_script(Cli::completion_script(shell), shell)
}

pub(super) fn adapt_completion_script(
    script: String,
    shell: usage::complete::Shell,
) -> Result<String, String> {
    use usage::complete::Shell;

    match shell {
        Shell::Bash | Shell::Nu => Ok(script),
        Shell::Zsh => replace_script_once(
            script,
            "zsh",
            "    case \"$__usage_files\" in\n",
            r#"    # File completion sees the whole `--flag=value` word. Move the exact
    # flag prefix into IPREFIX so `_files` completes only the value and zsh
    # restores the prefix when it inserts the result.
    if [[ -n "$__usage_files" && "$PREFIX" == --*=* ]]; then
        local __usage_attached="${PREFIX%%=*}="
        compset -P "${(b)__usage_attached}"
    fi

    case "$__usage_files" in
"#,
        ),
        Shell::Fish => {
            replace_script_once(script, "fish", FISH_PATH_BLOCK, FISH_ATTACHED_PATH_BLOCK)
        }
        Shell::PowerShell => replace_script_once(
            script,
            "powershell",
            POWERSHELL_PATH_BLOCK,
            POWERSHELL_ATTACHED_PATH_BLOCK,
        ),
        _ => Ok(script),
    }
}

fn replace_script_once(
    script: String,
    shell: &str,
    expected: &str,
    replacement: &str,
) -> Result<String, String> {
    if script.match_indices(expected).count() != 1 {
        return Err(format!(
            "usage-rs {shell} completion template changed; attached path adapter needs updating"
        ));
    }
    Ok(script.replacen(expected, replacement, 1))
}

const FISH_PATH_BLOCK: &str = r#"    # fish's own path completion, which knows about `~`, variables and remote paths.
    switch $files
        case any
            __fish_complete_path (commandline -ct)
        case dirs
            __fish_complete_directories (commandline -ct)
        case executables
            for candidate in (__fish_complete_path (commandline -ct))
                set -l value (string split -m 1 (printf '\t') -- $candidate)[1]
                if test -d "$value"; or test -x "$value"
                    printf '%s\n' $candidate
                end
            end
        case commands
            __fish_complete_command (commandline -ct)
    end
"#;

const FISH_ATTACHED_PATH_BLOCK: &str = r#"    # fish's native path completer sees the whole attached token. Isolate the
    # value for it, then restore the flag prefix on every result it returns.
    set -l __usage_cur (commandline -ct)
    set -l __usage_attached ""
    if contains -- "$files" any dirs executables; and string match -qr '^--[^=]+=' -- "$__usage_cur"
        set __usage_attached (string replace -r '=.*$' '=' -- "$__usage_cur")
        set __usage_cur (string replace -r '^--[^=]+=' '' -- "$__usage_cur")
    end
    switch $files
        case any
            for candidate in (__fish_complete_path "$__usage_cur")
                printf '%s%s\n' "$__usage_attached" "$candidate"
            end
        case dirs
            for candidate in (__fish_complete_directories "$__usage_cur")
                printf '%s%s\n' "$__usage_attached" "$candidate"
            end
        case executables
            for candidate in (__fish_complete_path "$__usage_cur")
                set -l value (string split -m 1 (printf '\t') -- $candidate)[1]
                if test -d "$value"; or test -x "$value"
                    printf '%s%s\n' "$__usage_attached" "$candidate"
                end
            end
        case commands
            __fish_complete_command (commandline -ct)
    end
"#;

const POWERSHELL_PATH_BLOCK: &str = r#"    } elseif ($files) {
        # PowerShell's own, so that `~`, drive-relative paths and provider paths behave as they
        # do everywhere else in the shell.
        foreach ($path in [System.Management.Automation.CompletionCompleters]::CompleteFilename($wordToComplete)) {
            # Trust PowerShell's result type for directories because CompletionText may already
            # carry quoting. Executable leaves are checked as commands after stripping only the
            # outer quote characters PowerShell added.
            if ($files -eq 'dirs' -and $path.ResultType -ne 'ProviderContainer') {
                continue
            }
            if ($files -eq 'executables' -and $path.ResultType -ne 'ProviderContainer') {
                $candidatePath = $path.CompletionText.Trim([char[]]@([char]39, [char]34))
                if (-not (Get-Command -Name $candidatePath -CommandType Application, ExternalScript -ErrorAction SilentlyContinue)) {
                    continue
                }
            }
            $results.Add($path)
        }
    }
"#;

const POWERSHELL_ATTACHED_PATH_BLOCK: &str = r#"    } elseif ($files) {
        # Complete only the value in `--flag=value`, then put the exact prefix
        # back on PowerShell's already-quoted native result.
        $pathWord = $wordToComplete
        $attachedPrefix = ''
        if ($wordToComplete -match '^(--[^=]+=)(.*)$') {
            $attachedPrefix = $Matches[1]
            $pathWord = $Matches[2]
        }
        foreach ($path in [System.Management.Automation.CompletionCompleters]::CompleteFilename($pathWord)) {
            # Trust PowerShell's result type for directories because CompletionText may already
            # carry quoting. Executable leaves are checked as commands after stripping only the
            # outer quote characters PowerShell added.
            if ($files -eq 'dirs' -and $path.ResultType -ne 'ProviderContainer') {
                continue
            }
            if ($files -eq 'executables' -and $path.ResultType -ne 'ProviderContainer') {
                $candidatePath = $path.CompletionText.Trim([char[]]@([char]39, [char]34))
                if (-not (Get-Command -Name $candidatePath -CommandType Application, ExternalScript -ErrorAction SilentlyContinue)) {
                    continue
                }
            }
            if ($attachedPrefix) {
                $path = [System.Management.Automation.CompletionResult]::new(
                    $attachedPrefix + $path.CompletionText,
                    $path.ListItemText,
                    $path.ResultType,
                    $path.ToolTip
                )
            }
            $results.Add($path)
        }
    }
"#;

/// Paths beginning with `partial`, each carrying `prefix` so Nu can replace the
/// whole token.
///
/// Nu cannot delegate a substituted last span to its native path completer. The
/// fallback deliberately drops names that the line/tab protocol cannot carry;
/// zsh, fish, PowerShell and every separated path stay shell-native.
fn nu_path_candidates(partial: &str, prefix: &str) -> Vec<usage::spec::Candidate<'static>> {
    let expanded = expand_tilde(partial);
    let path = std::path::Path::new(&expanded);
    let (directory, start) = if expanded.ends_with('/') {
        (path, "")
    } else {
        (
            path.parent().unwrap_or(std::path::Path::new(".")),
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(""),
        )
    };

    let listed = if directory.as_os_str().is_empty() {
        std::path::Path::new(".")
    } else {
        directory
    };
    let Ok(entries) = std::fs::read_dir(listed) else {
        return Vec::new();
    };

    let mut candidates: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.starts_with(start) || name.chars().any(char::is_control) {
                return None;
            }
            let full = listed.join(&name);
            let mut value = full.to_str()?.to_string();
            // `metadata` follows the link, which is what makes a symlinked
            // directory complete like the directory it points at.
            if std::fs::metadata(&full).is_ok_and(|meta| meta.is_dir()) {
                value.push('/');
            }
            Some(usage::spec::Candidate::new(format!("{prefix}{value}")))
        })
        .collect();
    candidates.sort_by(|left, right| left.value.cmp(&right.value));
    candidates
}

/// A leading `~` replaced with the home directory, for listing only.
fn expand_tilde(partial: &str) -> String {
    let Some(rest) = partial.strip_prefix('~') else {
        return partial.to_string();
    };
    if !rest.is_empty() && !rest.starts_with('/') {
        // `~other` is another user's home, which is the shell's business.
        return partial.to_string();
    }
    match std::env::var_os("HOME") {
        Some(home) => format!("{}{rest}", home.to_string_lossy()),
        None => partial.to_string(),
    }
}

/// Whether this shell's completion splits `--flag=value` into separate words.
///
/// bash does, because `=` is in its default `COMP_WORDBREAKS`, so the word it
/// asks about is the value alone. The others hand over the whole token.
fn breaks_words_at_equals(shell: usage::complete::Shell) -> bool {
    matches!(shell, usage::complete::Shell::Bash)
}

/// Whether `word` is two or more short flags behind one dash.
fn is_bundle(word: &str) -> bool {
    word.strip_prefix('-')
        .filter(|rest| !rest.starts_with('-'))
        .is_some_and(|letters| letters.chars().count() > 1)
}

/// Whether the word being completed is some flag's value.
///
/// `usage` offers paths both when a flag asks for them — it reads that off the
/// value name, so `PATH` means paths — and as a fallback wherever it has
/// nothing else to say. Only the first is right here, and the difference is
/// whether a flag is waiting for a value. Derived rather than naming
/// `--session-file`, so a second path option needs no change.
fn wants_paths(split: &usage::complete::Split) -> bool {
    let word = split
        .words
        .get(split.cword)
        .map(String::as_str)
        .unwrap_or("");
    if let Some((name, _)) = word
        .strip_prefix("--")
        .and_then(|body| body.split_once('='))
    {
        return long_flag(name).is_some_and(|flag| flag.takes_value);
    }

    let Some(previous) = split
        .cword
        .checked_sub(1)
        .and_then(|index| split.words.get(index))
    else {
        return false;
    };

    if let Some(name) = previous.strip_prefix("--") {
        return long_flag(name).is_some_and(|flag| flag.takes_value);
    }
    // A bundle hands its value to the *first* letter that takes one, and the
    // rest of the token is that value. `-mcustom` is already satisfied; only a
    // value-taking letter with nothing after it is still waiting.
    let Some(letters) = previous
        .strip_prefix('-')
        .filter(|rest| !rest.starts_with('-') && !rest.is_empty())
    else {
        return false;
    };
    bundle_value_boundary(letters).is_some_and(|(_, value_start)| letters[value_start..].is_empty())
}

/// The value of a `--name value` pair in a completion request's own arguments.
fn named_argument(argv: &[OsString], name: &str) -> Option<String> {
    let index = argv.iter().position(|arg| arg == name)?;
    Some(argv.get(index + 1)?.to_str()?.to_string())
}
