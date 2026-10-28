#!/usr/bin/env python3
"""Exercise Orchid's installed Codex connection in an isolated, tool-free session."""

import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default=shutil.which("orchid") or "./bin/orchid")
    parser.add_argument("--config", type=Path, default=Path.home() / ".config/orchid")
    parser.add_argument("--connection", default="codex-astra")
    parser.add_argument("--timeout", type=int, default=45)
    args = parser.parse_args()
    binary = str(Path(args.binary).resolve(strict=True))
    connection = json.loads((args.config / "connections" / (args.connection + ".json")).read_text())
    if connection.get("model") != "gpt-6-astra":
        raise RuntimeError("selected connection must use gpt-6-astra")
    root = Path(tempfile.mkdtemp(prefix="orchid-astra-smoke-"))
    for name in ("agents", "connections", "prompts"):
        (root / name).mkdir()
    # Reuse OAuth storage so refreshes persist; never copy or print credentials.
    (root / "auth").symlink_to((args.config / "auth").resolve(), target_is_directory=True)
    resources = {
        "config.json": {"log_level": "info"},
        "agents/default.json": {"prompt": "default", "connection": args.connection, "max_tokens": 4096},
        "connections/" + args.connection + ".json": connection,
    }
    for name, value in resources.items():
        (root / name).write_text(json.dumps(value))
    (root / "prompts/default.txt").write_text("Follow the user's requested output exactly. Do not use tools.")
    version = subprocess.run([binary, "--version"], capture_output=True, text=True)
    print("Binary:", binary, flush=True)
    print("Version:", version.stdout.strip() or "not reported by this build", flush=True)
    print("Model:", connection["model"], flush=True)
    print("Evidence:", root, flush=True)

    def command(*parts):
        result = subprocess.run([binary, "--config", str(root), *parts], capture_output=True, text=True, timeout=60)
        if result.returncode:
            raise RuntimeError(result.stderr.strip())
        return json.loads(result.stdout)

    session_id = command("create", "--label", "astra-smoke")["id"]
    print("Session:", session_id, flush=True)
    expected = "ORCHID_ASTRA_SMOKE_PASS"
    command("send", "--id", session_id, "Reply with exactly: " + expected)
    status = command("await", session_id, "--timeout", str(min(args.timeout, 50)))["sessions"][0]["status"]
    if status == "running":
        command("stop", session_id)
        raise RuntimeError("smoke test timed out; test worker stopped")
    events = command("get", session_id)["events"]
    failures = [event.get("message", event.get("reason")) for event in events if event.get("type") in ("failure", "termination")]
    if failures:
        raise RuntimeError(str(failures))
    actual = command("get", session_id, "--last-message").get("last_message")
    if status != "idle" or actual != expected:
        raise RuntimeError(f"status={status}, response={actual!r}")
    print("PASS:", actual, flush=True)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, ValueError, subprocess.TimeoutExpired) as error:
        raise SystemExit("FAIL: " + str(error))
