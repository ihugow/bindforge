#!/usr/bin/env python3
"""Opt-in smoke test on the real KDE session. No package/config installation.

Starts an isolated child daemon on the real session. The daemon loads its own
temporary bridge, publishes focus, then unloads it on SIGTERM. Refuses to replace
an existing daemon or migrate an enabled legacy bridge. No window activation.
"""
import argparse
import ast
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
SERVICE = "org.appbuttons.Remap"
PATH = "/org/appbuttons/Remap"


def call(destination, path, method, *arguments):
    return subprocess.run(
        ["gdbus", "call", "--session", "--dest", destination,
         "--object-path", path, "--method", method, *arguments],
        text=True, capture_output=True, timeout=5, check=True,
    ).stdout.strip()


def kwin(method, *arguments):
    return call("org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting." + method, *arguments)


def has_receiver():
    return call("org.freedesktop.DBus", "/org/freedesktop/DBus",
                "org.freedesktop.DBus.NameHasOwner", SERVICE) == "(true,)"


def read_status():
    # Decode the fixed GVariant tuple without evaluating code or changing strings.
    reply = call(SERVICE, PATH, "org.appbuttons.Remap.Status1.GetStatus")
    reply = re.sub(r"^\(uint32 (\d+), (true|false),", lambda m: f"({m[1]}, {m[2] == 'true'},", reply)
    reply = re.sub(r", uint64 (\d+)\)$", r", \1)", reply)
    version, simulation, profile, focus, revision = ast.literal_eval(reply)
    if version != 1 or simulation is not True:
        raise RuntimeError("Unsupported status protocol or mode.")
    return profile, focus, revision


def run(binary, check_status=False):
    # Do not connect to, terminate, or replace another instance.
    if has_receiver():
        raise RuntimeError("A remap daemon already owns the service; stop it before this test.")
    owner_before = call("org.freedesktop.DBus", "/org/freedesktop/DBus",
                        "org.freedesktop.DBus.GetNameOwner", "org.kde.KWin")
    if kwin("isScriptLoaded", "app-buttons-remap-focus") == "(true,)":
        raise RuntimeError("Legacy bridge is loaded. Complete migration before running this smoke test.")
    legacy = subprocess.run(["kreadconfig6", "--file", "kwinrc", "--group", "Plugins",
                             "--key", "app-buttons-remap-focusEnabled", "--default", "false"],
                            text=True, capture_output=True, timeout=5, check=True).stdout.strip()
    if legacy == "true":
        raise RuntimeError("Legacy bridge is enabled. Complete migration before this smoke test.")
    daemon = None
    try:
        daemon = subprocess.Popen([str(binary), "--kde"], stdout=subprocess.DEVNULL,
                                  stderr=subprocess.DEVNULL, text=True)
        deadline = time.monotonic() + 8
        while not has_receiver():
            if daemon.poll() is not None:
                raise RuntimeError("Daemon exited before becoming ready.")
            if time.monotonic() >= deadline:
                raise RuntimeError("Timed out waiting for daemon service.")
            time.sleep(0.05)
        receiver_pid = call("org.freedesktop.DBus", "/org/freedesktop/DBus",
                            "org.freedesktop.DBus.GetConnectionUnixProcessID", SERVICE)
        if receiver_pid != f"(uint32 {daemon.pid},)":
            raise RuntimeError("Service owner is not the child daemon; refusing to continue.")
        try:
            call(SERVICE, PATH, "org.appbuttons.Remap.Focus1.FocusChanged", "firefox", "")
        except subprocess.CalledProcessError as error:
            if "AccessDenied" not in error.stderr:
                raise RuntimeError("Unexpected rejection from receiver.") from error
        else:
            raise RuntimeError("Receiver accepted an unauthenticated focus message.")
        print("PASS: ordinary D-Bus client rejected (AccessDenied).", flush=True)
        deadline = time.monotonic() + 10
        while True:
            current = read_status()
            if current[1] in ("focused", "no_active_window") and kwin("isScriptLoaded", "app-buttons-remap-runtime") == "(true,)":
                break
            if time.monotonic() >= deadline:
                raise RuntimeError("Automatic bridge did not deliver its initial snapshot.")
            time.sleep(0.05)
        owner_after = call("org.freedesktop.DBus", "/org/freedesktop/DBus",
                           "org.freedesktop.DBus.GetNameOwner", "org.kde.KWin")
        if owner_after != owner_before:
            raise RuntimeError("KWin changed owner during the test; result is inconclusive.")
        print("PASS: daemon automatically loaded the bridge and received real KWin focus.", flush=True)
        daemon.terminate()
        daemon.wait(timeout=8)
        if daemon.returncode != 0:
            raise RuntimeError("Daemon failed during graceful shutdown.")
        if kwin("isScriptLoaded", "app-buttons-remap-runtime") != "(false,)":
            raise RuntimeError("Runtime bridge remained loaded after shutdown.")
        print("PASS: SIGTERM reset the simulation and unloaded the temporary bridge.", flush=True)
    finally:
        if daemon is not None and daemon.poll() is None:
            daemon.terminate()
            try:
                daemon.wait(timeout=8)
            except subprocess.TimeoutExpired:
                daemon.kill()
                daemon.wait(timeout=3)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/remap-daemon")
    parser.add_argument("--check-status", action="store_true", help="Compatibility flag; live status is always checked")
    args = parser.parse_args()
    try:
        run(args.binary.resolve(), args.check_status)
    except Exception as error:
        print("FAIL:", error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
