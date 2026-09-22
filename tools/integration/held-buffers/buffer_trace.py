"""Reconstruct Wayland buffer lifetimes from the app's Rust-client debug trace.

Only ``[rs]`` events belong to the main Wayland connection. GTK's separate
connection writes a different debug format, and IDs can be reused after a
``wl_buffer.destroy`` even within one connection.
"""

from dataclasses import dataclass, field
import re


_PREFIX = r"\[\d+(?:\.\d+)?\]\[rs\]\s*"
_EVENTS = {
    "pool": re.compile(r"Creating new SlotPool \([^\n]*? gen (\d+)\)"),
    "create": re.compile(_PREFIX + r"-> wl_shm_pool@\d+\.create_buffer\(wl_buffer@(\d+),"),
    "attach": re.compile(_PREFIX + r"-> wl_surface@(\d+)\.attach\(wl_buffer@(\d+),"),
    "commit": re.compile(_PREFIX + r"-> wl_surface@(\d+)\.commit\(\)"),
    "release": re.compile(_PREFIX + r"<- wl_buffer@(\d+)\.release, \(\)"),
    "destroy": re.compile(_PREFIX + r"-> wl_buffer@(\d+)\.destroy\(\)"),
}


@dataclass
class BufferLifetime:
    buffer: str
    generation: int
    created_offset: int
    commits: list[tuple[int, str]] = field(default_factory=list)
    releases: list[int] = field(default_factory=list)
    destroyed_offset: int | None = None

    def current_commit(self, at: int):
        return next(((offset, surface) for offset, surface in reversed(self.commits)
                     if offset <= at), None)

    def held_on(self, surface: str, at: int):
        commit = self.current_commit(at)
        return (commit is not None and commit[1] == surface
                and not any(commit[0] < release <= at for release in self.releases)
                and (self.destroyed_offset is None or self.destroyed_offset > at))

    def identity(self):
        return {"connection": "rs", "buffer": self.buffer,
                "generation": self.generation, "created_offset": self.created_offset}


@dataclass
class BufferTrace:
    lifetimes: list[BufferLifetime]
    main_commits: list[tuple[int, BufferLifetime]]
    latest_generation: int
    length: int

    def lifetime(self, identity):
        return next((buffer for buffer in self.lifetimes
                     if buffer.created_offset == identity["created_offset"]
                     and buffer.buffer == identity["buffer"]
                     and buffer.generation == identity["generation"]), None)

    def held_main(self, surface: str):
        return sorted((buffer for buffer in self.lifetimes
                       if buffer.generation == self.latest_generation
                       and buffer.held_on(surface, self.length)),
                      key=lambda buffer: buffer.current_commit(self.length)[0])


def parse_buffer_trace(text: str, main_surface: str):
    """Track create/attach/commit/release/destroy on the Rust Wayland client."""
    events = sorted((match.start(), kind, match.groups())
                    for kind, pattern in _EVENTS.items()
                    for match in pattern.finditer(text))
    lifetimes = []
    current = {}
    pending = {}
    main_commits = []
    generation = 0

    for offset, kind, values in events:
        if kind == "pool":
            generation = int(values[0])
        elif kind == "create":
            buffer = values[0]
            if buffer in current:
                raise ValueError(f"buffer {buffer} recreated without a recorded destroy")
            lifetime = BufferLifetime(buffer, generation, offset)
            lifetimes.append(lifetime)
            current[buffer] = lifetime
        elif kind == "attach":
            surface, buffer = values
            if buffer not in current:
                raise ValueError(f"attachment of uncreated buffer {buffer}")
            pending[surface] = current[buffer]
        elif kind == "commit":
            surface = values[0]
            lifetime = pending.pop(surface, None)
            if lifetime is not None:
                lifetime.commits.append((offset, surface))
                if surface == main_surface:
                    main_commits.append((offset, lifetime))
        elif kind == "release":
            buffer = values[0]
            if buffer in current:
                current[buffer].releases.append(offset)
        elif kind == "destroy":
            buffer = values[0]
            if buffer in current:
                current[buffer].destroyed_offset = offset
                del current[buffer]

    return BufferTrace(lifetimes, main_commits, generation, len(text))


def current_main_buffers(log, surface):
    trace = parse_buffer_trace(log.read_text(errors="replace"), surface)
    return [buffer.identity() for buffer in trace.held_main(surface)]


def new_frame_overlap(log, surface, old_buffers, before_transition):
    if len(old_buffers) != 3 or len({item["created_offset"] for item in old_buffers}) != 3:
        raise ValueError("transition requires three distinct old main buffer lifetimes")

    trace = parse_buffer_trace(log.read_text(errors="replace"), surface)
    old = [trace.lifetime(identity) for identity in old_buffers]
    if any(buffer is None or not buffer.held_on(surface, before_transition)
           for buffer in old):
        raise ValueError("selected old main buffer was not held at transition start")
    old_generation = old[0].generation
    if any(buffer.generation != old_generation for buffer in old):
        raise ValueError("selected old buffers belong to different generations")

    for offset, buffer in trace.main_commits:
        if (offset <= before_transition or buffer.generation <= old_generation
                or buffer.created_offset <= before_transition):
            continue
        released = [item.buffer for item in old
                    if any(before_transition < release < offset for release in item.releases)]
        destroyed = [item.buffer for item in old
                     if item.destroyed_offset is not None
                     and before_transition < item.destroyed_offset < offset]
        return {"new_buffer": buffer.buffer,
                "new_lifetime": buffer.identity(),
                "app_trace_commit_offset": offset,
                "old_released_before_new_commit": released,
                "old_destroyed_before_new_commit": destroyed}
    return None


def old_late_events(log, surface, old_buffers, new_commit_offset):
    trace = parse_buffer_trace(log.read_text(errors="replace"), surface)
    release = {}
    destroy = {}
    for identity in old_buffers:
        buffer = trace.lifetime(identity)
        if buffer is None:
            raise ValueError("selected old lifetime disappeared from the trace")
        release[buffer.buffer] = any(value > new_commit_offset for value in buffer.releases)
        destroy[buffer.buffer] = (buffer.destroyed_offset is not None
                                  and buffer.destroyed_offset > new_commit_offset)
    return release, destroy
