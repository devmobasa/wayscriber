"""Bound the app's wait while every buffer slot is held.

The live driver measures one quiet second after a redraw is requested and the
selected lifetimes are still held. It samples the main thread, which owns the
Wayland poll. A sleeping loop stays near zero CPU and logs a skip only when a
real event wakes it. A zero-timeout loop costs about one CPU-second per
wall-second and logs a skip on every pass. These limits sit far above a
sleeping loop and far below that busy loop.
"""

QUIET_SECONDS = 1.0
# Absolute CPU time for the quiet second, not a ratio. One busy core costs
# about one CPU-second; 0.35 still passes ordinary timer and logging noise.
MAX_CPU_SECONDS = 0.35
# A correct loop logs once per wake. A zero-timeout loop logs thousands.
MAX_EXTRA_SKIP_LOGS = 40
SKIP_LOG_MARK = "Skipping render - all buffers still held by the compositor"


class QuietHold:
    def __init__(self, wall_seconds, cpu_seconds, extra_skip_logs, redraw_was_pending,
                 commits_during, still_held):
        self.wall_seconds = wall_seconds
        self.cpu_seconds = cpu_seconds
        self.extra_skip_logs = extra_skip_logs
        self.redraw_was_pending = redraw_was_pending
        self.commits_during = commits_during
        self.still_held = still_held


def quiet_hold_failure(sample):
    if sample.wall_seconds + 1e-9 < QUIET_SECONDS:
        return "quiet interval ended early"
    if not sample.redraw_was_pending:
        return "redraw was not pending while every slot was held"
    if sample.commits_during:
        return "quiet interval committed a main frame"
    if not sample.still_held:
        return "a selected buffer was released during the quiet interval"
    if sample.cpu_seconds > MAX_CPU_SECONDS:
        return (f"held-buffer wait used {sample.cpu_seconds:.3f} CPU-seconds "
                f"in {sample.wall_seconds:.3f}s; limit is {MAX_CPU_SECONDS:.2f}")
    if sample.extra_skip_logs > MAX_EXTRA_SKIP_LOGS:
        return (f"held-buffer wait logged {sample.extra_skip_logs} extra skips; "
                f"limit is {MAX_EXTRA_SKIP_LOGS}")
    return None


def require_quiet_hold(sample):
    failure = quiet_hold_failure(sample)
    if failure:
        raise AssertionError(failure)
