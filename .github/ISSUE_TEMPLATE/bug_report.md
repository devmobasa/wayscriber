---
name: Bug report
about: Create a report to help us improve
title: "[BUG]"
labels: ''
assignees: devmobasa

---

## Summary
Explain what went wrong and what you were doing when it happened. Include any on-screen error message.

## Steps to Reproduce
List the exact sequence of actions that leads to the issue. If you are unsure about an intermediate step, please so note.

1. Launch Wayscriber  
   - If you normally run it once: open a terminal and run  
     ```
     RUST_LOG=wayscriber=debug wayscriber --active
     ```  
   - If you normally run the daemon: open a terminal and run  
     ```
     pkill wayscriber
     RUST_LOG=wayscriber=debug wayscriber --daemon
     ```  
     Then trigger your usual hotkey.
2. Describe the actions you perform inside Wayscriber (key presses, mouse actions, menu selections, etc.).
3. Mention any additional system commands or applications involved.
4. Describe the problem you observe (e.g. capture fails, overlay freezes, unexpected output).

## Expected Result
Tell us what you expected to happen instead.

## Actual Result / Logs
Include the full terminal output from the commands above. Copy everything beginning with the command itself down to the last line printed.  
If you start Wayscriber through systemd, also run the command below and attach the relevant lines:
```
journalctl --user -u wayscriber.service -b
```

## Configuration Details
- Wayscriber version (`wayscriber --version`, or the package name and version you installed)
- Whether you run in daemon mode or one-shot mode
- Relevant parts of your configuration file (usually `~/.config/wayscriber/config.toml`); remove or mask any private data

## Environment
- Linux distribution and version (e.g. `Arch Linux 2025.02.01`, `Fedora 41`)
- Wayland compositor/window manager (e.g. `Hyprland 0.44`, `Sway 1.9`)
- Installed portal backend (run `pacman -Qs xdg-desktop-portal` or `apt list xdg-desktop-portal*` and list the packages)
- Capture tools status (run each of the following and report their versions or any errors):
  ```
  grim --version
  slurp --version
  wl-copy --version
  ```
- GPU model and graphics driver (if the issue appears rendering-related)

## Repro Frequency / Workarounds
Does it happen every time or only sometimes? Mention any workaround you found that temporarily avoids the bug.

## Additional Context
Attach screenshots, screen recordings, crash dumps, or links to related issues/PRs that might help us understand the bug.

## System Logs (optional)
If you see messages that look related in other logs (for example, compositor logs or `dmesg`), include them here. Mention the command you ran to obtain the logs.

---

Thank you for helping us improve Wayscriber! Detailed reports make a big difference.
