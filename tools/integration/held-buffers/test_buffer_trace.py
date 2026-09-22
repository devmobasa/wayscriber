import tempfile
import unittest
from pathlib import Path

from buffer_trace import current_main_buffers, new_frame_overlap, old_late_events


def event(message, direction="->"):
    return f"[100.000][rs] {direction} {message}\n"


def main_buffer(buffer):
    return (event(f"wl_shm_pool@8.create_buffer(wl_buffer@{buffer}, 0, 1280, 720, 5120, 0)")
            + event(f"wl_surface@26.attach(wl_buffer@{buffer}, 0, 0)")
            + event("wl_surface@26.commit()"))


class BufferLifetimeTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.log = Path(temp.name) / "app.log"

    def write(self, trace):
        self.log.write_text(trace)
        return len(trace)

    def test_toolbar_reuse_before_transition_is_not_a_held_main_buffer(self):
        initial = ("Creating new SlotPool (1280x720, gen 1)\n"
                   + main_buffer(22) + main_buffer(23) + main_buffer(25))
        self.write(initial)
        stale_selection = current_main_buffers(self.log, "26")
        trace = (initial + event("wl_buffer@22.release, ()", "<-")
                 + event("wl_buffer@22.destroy()")
                 + event("wl_shm_pool@58.create_buffer(wl_buffer@22, 0, 1227, 104, 4908, 0)")
                 + event("wl_surface@41.attach(wl_buffer@22, 0, 0)")
                 + event("wl_surface@41.commit()"))
        self.write(trace)

        self.assertEqual([item["buffer"] for item in current_main_buffers(self.log, "26")],
                         ["23", "25"])
        with self.assertRaisesRegex(ValueError, "not held at transition start"):
            new_frame_overlap(self.log, "26", stale_selection, len(trace))

    def test_commit_is_required_and_old_lifetimes_release_later(self):
        trace = ("Creating new SlotPool (1280x720, gen 1)\n"
                 + main_buffer(22) + main_buffer(23) + main_buffer(25))
        start = self.write(trace)
        old = current_main_buffers(self.log, "26")
        next_frame = ("Creating new SlotPool (1600x900, gen 2)\n"
                      + event("wl_shm_pool@9.create_buffer(wl_buffer@39, 0, 1600, 900, 6400, 0)")
                      + event("wl_surface@26.attach(wl_buffer@39, 0, 0)"))
        self.write(trace + next_frame)
        self.assertIsNone(new_frame_overlap(self.log, "26", old, start))

        self.write(trace + next_frame + event("wl_surface@26.commit()"))
        overlap = new_frame_overlap(self.log, "26", old, start)
        self.assertEqual(overlap["new_buffer"], "39")
        self.assertEqual(overlap["old_released_before_new_commit"], [])

        self.write(trace + next_frame + event("wl_surface@26.commit()")
                   + event("wl_buffer@22.release, ()", "<-")
                   + event("wl_buffer@22.destroy()")
                   + event("wl_buffer@23.release, ()", "<-")
                   + event("wl_buffer@23.destroy()")
                   + event("wl_buffer@25.release, ()", "<-")
                   + event("wl_buffer@25.destroy()"))
        release, destroy = old_late_events(self.log, "26", old,
                                           overlap["app_trace_commit_offset"])
        self.assertEqual(release, {"22": True, "23": True, "25": True})
        self.assertEqual(destroy, {"22": True, "23": True, "25": True})

    def test_release_before_new_commit_is_rejected(self):
        trace = ("Creating new SlotPool (1280x720, gen 1)\n"
                 + main_buffer(22) + main_buffer(23) + main_buffer(25))
        start = self.write(trace)
        old = current_main_buffers(self.log, "26")
        self.write(trace + "Creating new SlotPool (1600x900, gen 2)\n"
                   + event("wl_shm_pool@9.create_buffer(wl_buffer@39, 0, 1600, 900, 6400, 0)")
                   + event("wl_surface@26.attach(wl_buffer@39, 0, 0)")
                   + event("wl_buffer@22.release, ()", "<-")
                   + event("wl_buffer@22.destroy()")
                   + event("wl_surface@26.commit()"))
        overlap = new_frame_overlap(self.log, "26", old, start)
        self.assertEqual(overlap["old_released_before_new_commit"], ["22"])
        self.assertEqual(overlap["old_destroyed_before_new_commit"], ["22"])

    def test_separate_gtk_connection_does_not_share_buffer_ids(self):
        trace = ("Creating new SlotPool (1280x720, gen 1)\n" + main_buffer(22)
                 + "[100.001] {Default Queue}  -> wl_shm_pool#8.create_buffer(wl_buffer#22)\n")
        self.write(trace)
        self.assertEqual([item["buffer"] for item in current_main_buffers(self.log, "26")],
                         ["22"])


if __name__ == "__main__":
    unittest.main()
