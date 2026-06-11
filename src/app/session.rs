use crate::cli::Cli;
use crate::env_vars::WAYLAND_DISPLAY_ENV;

pub(crate) fn run_session_cli_commands(cli: &Cli) -> anyhow::Result<()> {
    let loaded = crate::config::Config::load()?;
    let config_dir = crate::config::Config::config_directory_from_source(&loaded.source)?;
    let display_env = std::env::var(WAYLAND_DISPLAY_ENV).ok();

    let options = if let Some(raw_path) = cli.session_file.as_ref() {
        let raw = raw_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("--session-file path must be valid UTF-8"))?;
        let path = crate::session::normalize_named_session_file_arg(raw);
        if cli.clear_session {
            crate::session::validate_named_session_file_for_clear(&path)?;
        } else {
            crate::session::validate_named_session_file_for_info(&path)?;
        }
        let mut options = crate::session::options_from_config_for_named_file(
            &loaded.config.session,
            path,
            display_env.as_deref(),
        );
        options.force_resume_persistence();
        options
    } else {
        crate::session::options_from_config(
            &loaded.config.session,
            &config_dir,
            display_env.as_deref(),
        )?
    };

    if cli.clear_session {
        let outcome = crate::session::clear_session(&options)?;
        println!("Session file: {}", options.session_file_path().display());
        if outcome.removed_session {
            println!("  Removed session file");
        } else {
            println!("  No session file present");
        }
        if outcome.removed_backup {
            println!("  Removed backup file");
        }
        if outcome.removed_recovery {
            println!("  Removed recovery file");
        }
        if outcome.removed_lock {
            println!("  Removed lock file");
        }
        if !outcome.removed_session
            && !outcome.removed_backup
            && !outcome.removed_recovery
            && !outcome.removed_lock
        {
            println!("  No session artefacts found");
        }
        return Ok(());
    }

    if cli.session_info {
        let inspection = crate::session::inspect_session(&options)?;
        println!("Session persistence status:");
        println!("  Persist transparent: {}", inspection.persist_transparent);
        println!("  Persist whiteboard : {}", inspection.persist_whiteboard);
        println!("  Persist blackboard : {}", inspection.persist_blackboard);
        println!("  Persist history    : {}", inspection.persist_history);
        if inspection.persist_history {
            match inspection.history_limit {
                Some(limit) => println!("    Max persisted undo depth: {}", limit),
                None => println!("    Max persisted undo depth: follows runtime limit"),
            }
        } else {
            println!("    (history disabled; only visible drawings are saved)");
        }
        println!("  Restore tool state : {}", inspection.restore_tool_state);
        println!("  Per-output persistence: {}", inspection.per_output);
        println!(
            "  Session file       : {}",
            inspection.session_path.display()
        );
        if let Some(identity) = &inspection.active_identity {
            println!("    Output identity: {}", identity);
        }
        if inspection.exists {
            if let Some(size) = inspection.size_bytes {
                println!("    Size     : {} bytes", size);
            }
            if let Some(ts) = inspection
                .modified
                .and_then(|m| crate::time_utils::format_system_time(m, "%Y-%m-%d %H:%M:%S"))
            {
                println!("    Modified : {}", ts);
            }
            println!("    Compressed: {}", inspection.compressed);
            if let Some(version) = inspection.file_version {
                println!("    File version: {}", version);
            }
            if let Some(counts) = inspection.frame_counts {
                println!(
                    "    Shapes   : transparent {}, whiteboard {}, blackboard {}",
                    counts.transparent, counts.whiteboard, counts.blackboard
                );
            }
            println!("    History present: {}", inspection.history_present);
            if let Some(hist) = &inspection.history_counts {
                println!(
                    "    History (undo/redo): transparent {} / {}, whiteboard {} / {}, blackboard {} / {}",
                    hist.transparent.undo,
                    hist.transparent.redo,
                    hist.whiteboard.undo,
                    hist.whiteboard.redo,
                    hist.blackboard.undo,
                    hist.blackboard.redo
                );
            }
            println!("    Tool state stored: {}", inspection.tool_state_present);
        } else {
            println!("    (not found)");
        }

        println!("  Backup file       : {}", inspection.backup_path.display());
        if inspection.backup_exists {
            if let Some(size) = inspection.backup_size_bytes {
                println!("    Size     : {} bytes", size);
            }
        } else {
            println!("    (not found)");
        }

        println!("  Storage directory : {}", options.base_dir.display());

        return Ok(());
    }

    Ok(())
}
