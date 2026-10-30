"""Growth over a run: `census` lines and frame records by time slice.

Usage: python tools/census_report.py <run-dir> [slices]

The directory holds a run's log as run.jsonl and, from a `perf-diag` build's
`--perf`, frames.csv. Prints every census number that changed (first, last,
peak), then median, p99, 1% low, entities, process memory and heap by slice.
"""
import csv
import json
import statistics
import sys
from pathlib import Path


def flat(prefix, v, out):
    if isinstance(v, dict):
        for k, x in v.items():
            flat(f"{prefix}.{k}" if prefix else k, x, out)
    elif isinstance(v, (int, float)) and not isinstance(v, bool):
        out[prefix] = v


def main():
    d = Path(sys.argv[1])
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 6
    census, cleanups = [], []
    for line in (d / "run.jsonl").read_text(encoding="utf-8").splitlines():
        try:
            e = json.loads(line)
        except ValueError:
            continue
        if e.get("event") == "census":
            census.append(e)
        elif e.get("event") == "cleanup":
            cleanups.append(e)
    print("census lines", len(census), "cleanups", len(cleanups))
    if census:
        first, last = {}, {}
        flat("", census[0], first)
        flat("", census[-1], last)
        peak = {}
        for c in census:
            f = {}
            flat("", c, f)
            for k, v in f.items():
                peak[k] = max(peak.get(k, v), v)
        print("seconds", census[0]["seconds"], "->", census[-1]["seconds"], "tick", census[-1]["tick"])
        for k in sorted(set(first) | set(last) | set(peak)):
            if k in ("tick", "seconds"):
                continue
            a, b, p = first.get(k), last.get(k), peak.get(k)
            if a != b or a != p:
                print(f"  {k}: first {a} last {b} peak {p}")
        print("  last soras", census[-1].get("soras"))
    path = d / "frames.csv"
    if path.exists():
        with open(path, newline="") as f:
            rows = [r for r in csv.DictReader(f)]
        start = next((i for i, r in enumerate(rows) if r["paused"] == "0"), 0)
        rows = rows[start:]
        size = max(1, len(rows) // n)
        print("slice frames  median   p99   low1%ms worst  ent   procGiB heapMiB meshAdd")
        for i in range(n):
            part = rows[i * size : (i + 1) * size]
            if not part:
                continue
            ms = sorted(float(r["frame_ms"]) for r in part)
            low = ms[-max(1, len(ms) // 100) :]
            mem = [float(r["process_gib"]) for r in part if float(r["process_gib"]) > 0]
            heap = [int(r.get("heap_bytes") or 0) for r in part]
            print(
                f"{i:5d} {len(part):6d} {statistics.median(ms):7.2f} {ms[int(len(ms) * .99)]:6.2f}"
                f" {sum(low) / len(low):7.2f} {ms[-1]:6.1f}"
                f" {statistics.mean(int(r['entities']) for r in part):6.0f}"
                f" {(mem[-1] if mem else 0):7.3f} {heap[-1] / 2**20:8.1f}"
                f" {statistics.mean(int(r['meshes_added']) for r in part):6.1f}"
            )


main()
