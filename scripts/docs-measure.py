#!/usr/bin/env python3
"""Measure how long WireTrail takes, and how much memory it uses, on a large capture.

reference/performance.md quotes these figures. The large capture is made here by repeating the
entries of docs/user-guide/assets/downloads/checkout.har a thousand times (about 150 MB). It is a
synthetic file: it parses like a real one, but its content is repetitive, so treat the figures as a
guide to scale and not as a benchmark of your own captures.

A temporary file of the repeated entries minus the 429 responses is built (95,000 entries) and removed afterwards.
Each command runs once against it. Then `rate-limit` runs against a smaller file with a growing number of 429
responses on one route, to show how the cooldown check scales. Output is a table in Markdown.

usage: python3 scripts/docs-measure.py [--repeat N] [--binary PATH]
Needs /usr/bin/time (macOS: -l, Linux: -v).
"""
from __future__ import annotations

import argparse
import json
import platform
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "docs" / "user-guide" / "assets" / "downloads" / "checkout.har"

COMMANDS = ["hosts", "errors", "duplicates", "timeline", "export", "summary", "diagnose", "auto"]


def run(binary: Path, har: Path, command: str) -> tuple[float, float]:
    """Wall-clock seconds and peak resident memory in MiB for one run."""
    flag = "-l" if platform.system() == "Darwin" else "-v"
    started = time.perf_counter()
    proc = subprocess.run(
        ["/usr/bin/time", flag, str(binary), str(har), command, "--top", "5"],
        capture_output=True, text=True,
    )
    wall = time.perf_counter() - started
    err = proc.stderr
    if platform.system() == "Darwin":
        m = re.search(r"(\d+)\s+maximum resident set size", err)
        rss = int(m.group(1)) / 1048576 if m else float("nan")
    else:
        m = re.search(r"Maximum resident set size \(kbytes\): (\d+)", err)
        rss = int(m.group(1)) / 1024 if m else float("nan")
    return wall, rss


def write(path: Path, doc: dict, entries: list) -> None:
    path.write_text(json.dumps({"log": {**doc["log"], "entries": entries}}, separators=(",", ":")))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repeat", type=int, default=1000)
    ap.add_argument("--binary", default=str(ROOT / "target" / "release" / "wiretrail"))
    args = ap.parse_args()
    binary = Path(args.binary)
    if not binary.exists():
        sys.exit(f"binary not found: {binary} (run cargo build --release)")

    doc = json.loads(SOURCE.read_text())
    entries = doc["log"]["entries"]
    typical = [e for e in entries if e["response"]["status"] != 429]
    only_429 = [e for e in entries if e["response"]["status"] == 429]
    version = subprocess.run([str(binary), "--version"], capture_output=True, text=True).stdout.strip()

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        big = tmp / "typical.har"
        write(big, doc, typical * args.repeat)
        size_mb = big.stat().st_size / 1e6
        print(f"{version} on {platform.platform()} ({platform.machine()})")
        print(f"typical.har: {len(typical) * args.repeat:,} entries, {size_mb:.0f} MB\n")
        print("| Command | Time | Peak memory |\n|---|---|---|")
        for c in COMMANDS:
            wall, rss = run(binary, big, c)
            print(f"| `{c}` | {wall:.2f} s | {rss:.0f} MiB |")

        print("\n| 429 responses on one route | `rate-limit` |\n|---|---|")
        base = typical * (args.repeat // 50)
        for n in (250, 500, 1000, 2000, 4000):
            reps = n // len(only_429) + 1
            rows = (only_429 * reps)[:n]
            f = tmp / "scale.har"
            write(f, doc, base + rows)
            wall, _ = run(binary, f, "rate-limit")
            print(f"| {n:,} | {wall:.2f} s |")
    return 0


if __name__ == "__main__":
    sys.exit(main())
