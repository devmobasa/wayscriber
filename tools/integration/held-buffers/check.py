#!/usr/bin/env python3
"""Exercise held SHM buffers across real nested Wayland geometry changes."""

import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

from buffer_trace import current_main_buffers, new_frame_overlap, old_late_events, parse_buffer_trace


def run(args, env):
    return subprocess.run(args, env=env, check=True, capture_output=True, text=True,
                          timeout=6).stdout


def wait_for(check, description, seconds=12):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            value = check()
            if value:
                return value
        except (OSError, subprocess.SubprocessError, json.JSONDecodeError, IndexError):
            pass
        time.sleep(0.05)
    raise RuntimeError(f"timed out waiting for {description}")


def start(args, env, log):
    with log.open("wb") as output:
        return subprocess.Popen(args, env=env, stdout=output, stderr=subprocess.STDOUT,
                                start_new_session=True)


def stop(process):
    if process is None or process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)


def main_surface(log):
    match = re.search(
        r'get_layer_surface\(zwlr_layer_surface_v1@\d+, wl_surface@(\d+), .*Some\("wayscriber"\)\)',
        log.read_text(errors="replace"))
    return match.group(1) if match else None


def config(path, mode, scale):
    path.write_text(
        f"output HEADLESS-1 resolution {mode} scale {scale}\n"
        "focus_follows_mouse no\n")


def execute(app, fixture, pointer, root):
    runtime = root / "runtime"
    runtime.mkdir(mode=0o700)
    xdg = root / "xdg"
    for name in ("config", "data", "cache", "state", "home"):
        (xdg / name).mkdir(parents=True)
    (xdg / "config" / "wayscriber").mkdir()
    (xdg / "config" / "wayscriber" / "config.toml").write_text(
        "[ui]\nshow_onboarding_hints = false\n"
        "[ui.toolbar]\nbackend = \"builtin\"\n"
        "[performance]\nbuffer_count = 3\nenable_vsync = false\nmax_fps_no_vsync = 0\n"
        "[capture]\ncopy_to_clipboard = false\n")
    base = os.environ.copy()
    for name in ("HYPRLAND_INSTANCE_SIGNATURE", "DBUS_SESSION_BUS_ADDRESS", "DISPLAY",
                 "SWAYSOCK", "WAYLAND_SOCKET"):
        base.pop(name, None)
    base["XDG_RUNTIME_DIR"] = str(runtime)
    base.pop("WAYLAND_DISPLAY", None)
    base.pop("DISPLAY", None)
    base.update(WLR_BACKENDS="headless", WLR_RENDERER="pixman",
                WLR_HEADLESS_OUTPUTS="1")
    compositor = wayscriber = None
    try:
        conf = root / "sway.conf"
        config(conf, "1280x720", 1)
        schedule = root / "release-schedule"
        compositor_env = base | {"LD_PRELOAD": str(fixture),
                                 "WAYSCRIBER_RELEASE_HOLD_MS": "8000",
                                 "WAYSCRIBER_RELEASE_SCHEDULE_FILE": str(schedule)}
        compositor = start(["sway", "--config", str(conf), "--debug"],
                           compositor_env, root / "sway.log")
        socket = wait_for(lambda: next(runtime.glob("sway-ipc.*.sock"), None),
                          "headless compositor IPC")
        display = wait_for(lambda: next((path.name for path in runtime.glob("wayland-*")
                                         if path.is_socket()), None), "headless Wayland socket")
        control = ["swaymsg", "--socket", str(socket)]
        def monitor():
            output = json.loads(run(control + ["--type", "get_outputs", "--raw"], base))[0]
            mode = output["current_mode"]
            return {"width": mode["width"], "height": mode["height"],
                    "scale": output["scale"]}
        wait_for(lambda: monitor()["width"] == 1280, "initial output mode")

        app_env = base | {"WAYLAND_DISPLAY": display,
                          "XDG_CONFIG_HOME": str(xdg / "config"),
                          "XDG_DATA_HOME": str(xdg / "data"),
                          "XDG_CACHE_HOME": str(xdg / "cache"),
                          "XDG_STATE_HOME": str(xdg / "state"),
                          "HOME": str(xdg / "home"),
                          "XDG_CURRENT_DESKTOP": "sway",
                          "GDK_BACKEND": "wayland", "GSK_RENDERER": "cairo",
                          "WAYSCRIBER_NO_DETACH": "1", "WAYLAND_DEBUG": "1",
                          "RUST_LOG": "debug", "WAYSCRIBER_ISOLATED_POINTER": "1"}
        app_log = root / "app.log"
        wayscriber = start([str(app), "--active", "--no-resume-session",
                           "--no-exit-after-capture"], app_env, app_log)
        surface = wait_for(lambda: main_surface(app_log), "main layer surface")
        wait_for(lambda: current_main_buffers(app_log, surface), "first main frame")

        for mode, scale in (("1600x900", 1), ("1600x900", 1.6666666)):
            current = monitor()
            def occupied():
                active = current_main_buffers(app_log, surface)
                return active[-3:] if len(active) >= 3 else None
            old = None
            for attempt in range(8):
                old = occupied()
                if old:
                    break
                if attempt:
                    x = 80 + 25 * attempt
                    run([str(pointer), "drag", str(x), "220", str(x + 60), "280",
                         str(current["width"]), str(current["height"])], app_env)
            old = old or wait_for(occupied, "three occupied main slots", 3)
            schedule.write_text(f"{wayscriber.pid} " + " ".join(
                item["buffer"] for item in old) + "\n")
            before = len(app_log.read_text(errors="replace"))
            fixture_before = len((root / "sway.log").read_text(errors="replace"))
            old_pool_count = app_log.read_text(errors="replace").count("Creating new SlotPool (")
            old_preferred_count = app_log.read_text(errors="replace").count("/120 for main surface")
            old_configure_count = len(re.findall(
                rf"pid={wayscriber.pid} layer=\d+ configure=",
                (root / "sway.log").read_text(errors="replace")))
            config(conf, mode, scale)
            run(control + ["reload"], base)
            width, height = map(int, mode.split("x"))
            wait_for(lambda: monitor()["width"] == width and monitor()["height"] == height
                     and (scale == 1 or monitor()["scale"] > 1), "output geometry change")
            wait_for(lambda: app_log.read_text(errors="replace").count("Creating new SlotPool (")
                     > old_pool_count, "new pool generation")
            wait_for(lambda: len(re.findall(
                rf"pid={wayscriber.pid} layer=\d+ configure=",
                (root / "sway.log").read_text(errors="replace"))) > old_configure_count,
                "main layer configure")
            if scale != 1:
                wait_for(lambda: app_log.read_text(errors="replace").count("/120 for main surface")
                         > old_preferred_count, "fractional preferred scale")
                wait_for(lambda: re.search(
                    rf"pid={wayscriber.pid} fractional=\d+ preferred=\d+/120",
                    (root / "sway.log").read_text(errors="replace")),
                    "server fractional preferred scale")
            overlap = wait_for(lambda: new_frame_overlap(app_log, surface, old, before),
                               "new main frame while old slots are held")
            assert not overlap["old_released_before_new_commit"], overlap
            assert not overlap["old_destroyed_before_new_commit"], overlap
            wait_for(lambda: all(old_late_events(app_log, surface, old,
                     overlap["app_trace_commit_offset"])[0].values()) and
                     all(old_late_events(app_log, surface, old,
                     overlap["app_trace_commit_offset"])[1].values()),
                     "late release and destruction", 16)
            trace = parse_buffer_trace(app_log.read_text(errors="replace"), surface)
            lifetimes = [trace.lifetime(identity) for identity in old]
            assert all(item and item.destroyed_offset and item.releases for item in lifetimes)
            assert all(item.destroyed_offset > item.releases[-1] for item in lifetimes)
            assert [item.releases[-1] for item in lifetimes] != sorted(
                item.releases[-1] for item in lifetimes), "old releases stayed in commit order"
            fixture_events = (root / "sway.log").read_text(errors="replace")[fixture_before:]
            for identity in old:
                event = rf"pid={wayscriber.pid} buffer={identity['buffer']} release="
                assert re.search(event + "delivered", fixture_events), identity
                assert not re.search(event + "cancelled", fixture_events), identity
            commits_before_drag = len(trace.main_commits)
            run([str(pointer), "drag", "80", "220", "180", "280",
                 str(width), str(height)], app_env)
            wait_for(lambda: len(parse_buffer_trace(
                app_log.read_text(errors="replace"), surface).main_commits)
                > commits_before_drag, "rendering after late releases")
            assert wayscriber.poll() is None, "app exited during held-buffer transition"
            print(f"PASS {mode} scale={scale}: 3 old lifetimes held through new commit; "
                  "released and destroyed afterward", flush=True)
    finally:
        stop(wayscriber)
        stop(compositor)
    summaries = re.findall(r"held-buffer-fixture summary .*outstanding=(\d+)",
                           (root / "sway.log").read_text(errors="replace"))
    assert summaries and all(count == "0" for count in summaries), \
        "server retained a delayed release after shutdown"


if __name__ == "__main__":
    app, fixture, pointer = map(lambda value: Path(value).resolve(), sys.argv[1:])
    root = Path(tempfile.mkdtemp(prefix="wb", dir="/tmp"))
    try:
        execute(app, fixture, pointer, root)
    except Exception:
        print(f"FAIL: inspect isolated logs in {root}", file=sys.stderr)
        raise
    else:
        shutil.rmtree(root)
