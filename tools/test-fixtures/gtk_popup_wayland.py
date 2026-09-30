"""Run a real GTK popup behind a private Weston with one frame event withheld.

All protocol messages and SCM_RIGHTS descriptors pass through unchanged, except
the selected popup callback and its delete_id event. The XML schemas provide
opcodes and new-object types; this fixture neither renders nor invents feedback.
"""

import array
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import xml.etree.ElementTree as ET


def schemas():
    interfaces = {}
    for package, relative in (("wayland-client", "wayland.xml"),
                              ("wayland-protocols", "stable/xdg-shell/xdg-shell.xml")):
        directory = subprocess.check_output(
            ["pkg-config", "--variable=pkgdatadir", package], text=True
        ).strip()
        for interface in ET.parse(Path(directory) / relative).getroot().findall("interface"):
            interfaces[interface.attrib["name"]] = {
                direction: interface.findall(tag)
                for direction, tag in (("request", "request"), ("event", "event"))
            }
    return interfaces


def arguments(message, payload):
    values = {}
    offset = 0
    for argument in message.findall("arg"):
        kind = argument.attrib["type"]
        if kind == "fd":
            continue
        value = struct.unpack_from("=I", payload, offset)[0]
        offset += 4
        if kind in ("string", "array"):
            raw = payload[offset:offset + value]
            offset += (value + 3) & ~3
            value = raw.rstrip(b"\0").decode() if kind == "string" else raw
        elif kind == "new_id" and "interface" not in argument.attrib:
            # A dynamic new_id carries its interface name, version, then ID.
            interface = payload[offset:offset + value].rstrip(b"\0").decode()
            offset += (value + 3) & ~3
            version, value = struct.unpack_from("=II", payload, offset)
            offset += 8
            values["interface"] = interface
            values["version"] = version
        values[argument.attrib["name"]] = value
    return values


def receive(sock):
    data = bytearray()
    fds = array.array("i")
    size = 8
    while len(data) < size:
        chunk, ancillary, flags, _ = sock.recvmsg(
            size - len(data), socket.CMSG_SPACE(256 * fds.itemsize)
        )
        if not chunk:
            for fd in fds:
                os.close(fd)
            return None
        if flags & socket.MSG_CTRUNC:
            raise RuntimeError("truncated Wayland descriptors")
        data.extend(chunk)
        for level, kind, raw in ancillary:
            if level == socket.SOL_SOCKET and kind == socket.SCM_RIGHTS:
                fds.frombytes(raw)
        if len(data) == 8:
            size = struct.unpack_from("=I", data, 4)[0] >> 16
            if size < 8 or size % 4:
                raise RuntimeError("invalid Wayland message length")
    return bytes(data), fds


def send(sock, packet):
    data, fds = packet
    try:
        ancillary = [(socket.SOL_SOCKET, socket.SCM_RIGHTS, fds)] if fds else []
        sent = sock.sendmsg([data], ancillary)
        sock.sendall(data[sent:])
    finally:
        for fd in fds:
            os.close(fd)


class Connection:
    def __init__(self, client, upstream, interfaces):
        self.client = client
        self.server = socket.socket(socket.AF_UNIX)
        self.server.connect(upstream)
        self.interfaces = interfaces
        self.objects = {1: "wl_display"}
        self.xdg_surfaces = {}
        self.popups = set()
        self.callbacks = {}
        self.lock = threading.RLock()
        self.armed = False
        self.selected = None
        self.held = []
        self.commits = 0
        self.delivered = 0
        self.error = None

    def start(self):
        for source, destination, direction in (
            (self.client, self.server, "request"), (self.server, self.client, "event")
        ):
            threading.Thread(target=self.forward, args=(source, destination, direction),
                             daemon=True).start()

    def inspect(self, packet, direction):
        data, _ = packet
        object_id, header = struct.unpack_from("=II", data)
        interface = self.objects.get(object_id)
        if interface not in self.interfaces:
            return False
        message = self.interfaces[interface][direction][header & 0xffff]
        values = arguments(message, data[8:])
        for argument in message.findall("arg"):
            if argument.attrib["type"] == "new_id":
                self.objects[values[argument.attrib["name"]]] = argument.attrib.get(
                    "interface", values.get("interface")
                )
        name = message.attrib["name"]

        if direction == "request":
            if interface == "xdg_wm_base" and name == "get_xdg_surface":
                self.xdg_surfaces[values["id"]] = values["surface"]
            elif interface == "xdg_surface" and name == "get_popup":
                self.popups.add(self.xdg_surfaces[object_id])
            elif interface == "wl_surface" and name == "frame":
                self.callbacks[values["callback"]] = object_id
                if self.armed and object_id in self.popups:
                    self.selected = values["callback"]
                    self.armed = False
            elif interface == "wl_surface" and name == "commit" and object_id in self.popups:
                self.commits += 1
            return False

        if interface == "wl_callback" and name == "done":
            if object_id == self.selected:
                return True
            if self.callbacks.pop(object_id, None) in self.popups:
                self.delivered += 1
        elif interface == "wl_display" and name == "delete_id":
            if values["id"] == self.selected:
                return True
            self.objects.pop(values["id"], None)
        return False

    def forward(self, source, destination, direction):
        try:
            while (packet := receive(source)) is not None:
                with self.lock:
                    if self.inspect(packet, direction):
                        self.held.append(packet)
                    else:
                        send(destination, packet)
        except Exception as error:
            with self.lock:
                self.error = f"{type(error).__name__}: {error}"

    def control(self, command):
        with self.lock:
            if self.error:
                return {"error": self.error}
            if command == "arm":
                self.armed = True
                self.commits = 0
                self.delivered = 0
            elif command == "release":
                if not self.held:
                    return {"error": "no popup callback held"}
                self.selected = None
                for packet in self.held:
                    self.inspect(packet, "event")
                    send(self.client, packet)
                self.held.clear()
            elif command != "status":
                return {"error": "unknown control command"}
            return {"held": bool(self.held), "popup_commits": self.commits,
                    "popup_callbacks_delivered": self.delivered}


def listener(path):
    sock = socket.socket(socket.AF_UNIX)
    sock.bind(str(path))
    sock.listen()
    return sock


def run_fixture(directory, executable, test_name):
    environment = dict(os.environ, XDG_RUNTIME_DIR=directory, WAYLAND_DISPLAY="upstream",
                       GDK_BACKEND="wayland", GSK_RENDERER="gl", LIBGL_ALWAYS_SOFTWARE="1",
                       GTK_A11Y="test", GDK_DEBUG="no-portals")
    environment.pop("DISPLAY", None)
    environment.pop("WAYLAND_SOCKET", None)
    environment.pop("DBUS_SESSION_BUS_ADDRESS", None)
    with open(Path(directory) / "weston.log", "w+") as log:
        weston = subprocess.Popen(
            ["weston", "--backend=headless-backend.so", "--renderer=pixman", "--no-config",
             "--socket=upstream", "--idle-time=0"], env=environment, stdout=log, stderr=log
        )
        try:
            upstream = str(Path(directory) / "upstream")
            deadline = time.monotonic() + 10
            while not Path(upstream).is_socket():
                if weston.poll() is not None or time.monotonic() >= deadline:
                    log.seek(0)
                    raise RuntimeError(log.read())
                time.sleep(0.01)
            interfaces = schemas()
            proxy = listener(Path(directory) / "proxy")
            control = listener(Path(directory) / "control")
            connections = []

            def accept_clients():
                while True:
                    client, _ = proxy.accept()
                    connection = Connection(client, upstream, interfaces)
                    connections.append(connection)
                    connection.start()

            def accept_control():
                while True:
                    client, _ = control.accept()
                    with client, client.makefile("r") as reader:
                        command = reader.readline().strip()
                        popup_connections = [connection for connection in connections
                                             if connection.popups]
                        response = popup_connections[-1].control(command) if popup_connections else {
                            "error": "no GTK popup connection"
                        }
                        client.sendall((json.dumps(response) + "\n").encode())

            threading.Thread(target=accept_clients, daemon=True).start()
            threading.Thread(target=accept_control, daemon=True).start()
            environment.update(WAYLAND_DISPLAY="proxy", G_DEBUG="fatal-criticals",
                               WAYSCRIBER_GTK_WAYLAND_CONTROL=str(Path(directory) / "control"))
            child = subprocess.Popen(
                ["dbus-run-session", "--", executable, test_name, "--exact", "--test-threads=1",
                 "--nocapture"], env=environment, start_new_session=True
            )
            try:
                return child.wait(timeout=20)
            finally:
                # Includes the private bus and any services it activated.
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait()
        finally:
            weston.terminate()
            try:
                weston.wait(timeout=3)
            except subprocess.TimeoutExpired:
                weston.kill()
                weston.wait()


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="wayscriber-popup-test-") as directory:
        raise SystemExit(run_fixture(directory, *sys.argv[1:]))
