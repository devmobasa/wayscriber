# Session Manager Examples

These examples cover named sessions. Default persistence still works without a
named file: start Wayscriber normally and it restores the configured session
storage on the next run.

## Direct CLI Workflows

Use a named session file when you want one drawing set per meeting, lecture, or
project:

```bash
wayscriber --active --session-file ~/Documents/lecture-04.wayscriber-session
wayscriber --freeze --session-file ~/Documents/lecture-04.wayscriber-session
```

Named sessions can also be the target for daemon launches:

```bash
wayscriber --daemon --session-file ~/Documents/default-work.wayscriber-session
wayscriber --daemon-toggle --session-file ~/Documents/meeting.wayscriber-session
```

Inspect or clear one named session without touching the configured default
session:

```bash
wayscriber --session-info --session-file ~/Documents/lecture-04.wayscriber-session
wayscriber --clear-session --session-file ~/Documents/lecture-04.wayscriber-session
```

`--session-file` uses the exact selected file, rejects directories, symlinks,
and special files, and conflicts with `--no-resume-session`. Launch, Open, and
Save As flows require an existing parent directory. `--session-info` and
`--clear-session` can still report or clean up stale named-session paths when
the parent directory is already gone.

## Overlay Workflow

Open Wayscriber with any persisted session target, then use the side toolbar's
Settings drawer:

- `Open` loads an existing named session and records it in the recent catalog.
- `Save As` writes the current overlay to another named session and switches the
  active target. It appends `.wayscriber-session` when no extension is supplied
  and asks before replacing existing session artifacts.
- `Info` reports the active session file size, board shape counts, and history
  status.
- `Clear` writes a durable empty session boundary for the active target.
- Recent session rows reopen other named sessions.
- `Manager` opens the configurator.

The overlay file picker uses `zenity` first and falls back to `kdialog`.

## Configurator Catalog Workflow

Run:

```bash
wayscriber-configurator
```

Open the Session tab and use the Saved Sessions section. It shows recent named
sessions recorded when named-session targets are opened or saved from the CLI,
daemon, or overlay.

- `Save Name` changes only the catalog display label.
- `Reveal File` opens the session's parent folder.
- `Forget` removes catalog metadata without deleting session files.
- `Duplicate` copies the primary session file to a new named target.
- `Move` moves an inactive session's primary file and non-lock sidecars.
- `Clear Saved Data` removes saved data and sidecars for that catalog entry.

Duplicate, Move, and Clear are disabled while an overlay, manually started
daemon, or background service is active. Stop the service or close the overlay
before managing inactive session files from the configurator.
