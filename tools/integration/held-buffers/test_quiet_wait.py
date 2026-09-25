import unittest

from quiet_wait import QuietHold, quiet_hold_failure, require_quiet_hold


def sample(**overrides):
    values = dict(wall_seconds=1.0, cpu_seconds=0.02, extra_skip_logs=2,
                  redraw_was_pending=True, commits_during=0, still_held=True)
    values.update(overrides)
    return QuietHold(**values)


class QuietHoldTests(unittest.TestCase):
    def test_sleeping_wait_is_accepted(self):
        require_quiet_hold(sample())

    def test_spinning_cpu_is_rejected(self):
        failure = quiet_hold_failure(sample(cpu_seconds=0.95, extra_skip_logs=0))
        self.assertIn("CPU-seconds", failure)

    def test_spinning_dispatch_is_rejected(self):
        failure = quiet_hold_failure(sample(cpu_seconds=0.02, extra_skip_logs=5000))
        self.assertIn("extra skips", failure)

    def test_missing_pending_redraw_is_rejected(self):
        self.assertIn("not pending", quiet_hold_failure(sample(redraw_was_pending=False)))

    def test_commit_or_release_during_the_quiet_interval_is_rejected(self):
        self.assertIn("committed", quiet_hold_failure(sample(commits_during=1)))
        self.assertIn("released", quiet_hold_failure(sample(still_held=False)))

    def test_short_interval_is_rejected(self):
        self.assertIn("ended early", quiet_hold_failure(sample(wall_seconds=0.2)))


if __name__ == "__main__":
    unittest.main()
