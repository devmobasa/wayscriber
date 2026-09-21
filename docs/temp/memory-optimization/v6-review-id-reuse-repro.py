"""Read-only reproducer using the selector from the pre-correction V6 probe."""
from pathlib import Path
import re
import tempfile


def fill_slots(log, compositor_log, surface, app_pid):
    """Snapshot of the historical ID-only selection being reviewed."""
    text = log.read_text()
    last_pool = text.rfind("Creating new SlotPool (")
    buffers = list(dict.fromkeys(re.findall(
        rf"-> wl_surface@{surface}\.attach\(wl_buffer@(\d+),", text[last_pool:])))
    states = dict(re.findall(
        rf"pid={app_pid} buffer=(\d+) release=(held|delivered|cancelled)",
        compositor_log.read_text()))
    active = [buffer for buffer in buffers if states.get(buffer) == "held"]
    return active[-3:] if len(active) >= 3 else None


def new_frame_overlap(log, surface, old_buffers, before_transition):
    """Snapshot of the historical attach and ID-only overlap check."""
    trace = log.read_text()
    pool = trace.find("Creating new SlotPool (", before_transition)
    if pool < 0:
        return None
    created = {}
    for match in re.finditer(r"create_buffer\(wl_buffer@(\d+),", trace[pool:]):
        created.setdefault(match.group(1), match.start())
    for match in re.finditer(rf"-> wl_surface@{surface}\.attach\(wl_buffer@(\d+),", trace[pool:]):
        buffer = match.group(1)
        if buffer not in created or created[buffer] > match.start():
            continue
        attach = pool + match.start()
        released_before = [old for old in old_buffers
                           if 0 <= trace.find(f"<- wl_buffer@{old}.release", before_transition, attach)]
        destroyed_before = [old for old in old_buffers
                            if 0 <= trace.find(f"-> wl_buffer@{old}.destroy", before_transition, attach)]
        return {"new_buffer": buffer,
                "old_released_before_new_attach": released_before,
                "old_destroyed_before_new_attach": destroyed_before}
    return None

with tempfile.TemporaryDirectory(prefix='v6-id-reuse-review-') as directory:
    app = Path(directory) / 'app.log'
    compositor = Path(directory) / 'compositor.log'
    app.write_text('''Creating new SlotPool (1280x720)
-> wl_shm_pool@8.create_buffer(wl_buffer@22, 0, 1280, 720, 5120, 0)
-> wl_surface@26.attach(wl_buffer@22, 0, 0)
-> wl_surface@26.commit()
-> wl_shm_pool@8.create_buffer(wl_buffer@23, 3686400, 1280, 720, 5120, 0)
-> wl_surface@26.attach(wl_buffer@23, 0, 0)
-> wl_surface@26.commit()
-> wl_shm_pool@8.create_buffer(wl_buffer@25, 7372800, 1280, 720, 5120, 0)
-> wl_surface@26.attach(wl_buffer@25, 0, 0)
-> wl_surface@26.commit()
<- wl_buffer@22.release, ()
-> wl_buffer@22.destroy()
-> wl_shm_pool@58.create_buffer(wl_buffer@22, 0, 1227, 104, 4908, 0)
-> wl_surface@41.attach(wl_buffer@22, 0, 0)
-> wl_surface@41.commit()
''')
    compositor.write_text('''v6-fixture pid=123 buffer=22 release=held
v6-fixture pid=123 buffer=23 release=held
v6-fixture pid=123 buffer=25 release=held
v6-fixture pid=123 buffer=22 release=delivered
v6-fixture pid=123 buffer=22 release=held
''')
    selected = fill_slots(app, compositor, '26', 123)
    assert selected == ['22', '23', '25'], selected
    start = len(app.read_text())
    with app.open('a') as stream:
        stream.write('''Creating new SlotPool (1600x900)
-> wl_shm_pool@38.create_buffer(wl_buffer@39, 0, 1600, 900, 6400, 0)
-> wl_surface@26.attach(wl_buffer@39, 0, 0)
-> wl_surface@26.commit()
<- wl_buffer@22.release, ()
-> wl_buffer@22.destroy()
<- wl_buffer@23.release, ()
-> wl_buffer@23.destroy()
<- wl_buffer@25.release, ()
-> wl_buffer@25.destroy()
''')
    overlap = new_frame_overlap(app, '26', selected, start)
    assert overlap and not overlap['old_released_before_new_attach']
    assert not overlap['old_destroyed_before_new_attach']
    print('REPRODUCED: fill_slots returns three IDs although only 23 and 25 are main-surface buffers.')
    print('The overlap check also accepts the reused toolbar ID 22. No UI commands executed.')
