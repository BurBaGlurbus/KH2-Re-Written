"""Summarise a `--perf` frame record (frames.csv) and, optionally, the run log.

Usage: python tools/perf_report.py <frames.csv> [run-log.jsonl] [--skip-ms N]
           [--ticks A:B]

Only frames after virtual time first runs count (unattended runs warm up
paused), minus the first --skip-ms of that (default 0). --ticks keeps the
frames whose fixed-tick count since then lies in [A, B) (a phase of a run). Prints one JSON object:
average FPS, 1% / 0.1% low FPS (mean frame time of the slowest 1% / 0.1%),
frame time average / p50 / p95 / p99 / max, entity and memory ranges,
allocations per frame and the log's startup and area-load timings.
"""

import argparse
import csv
import json
from pathlib import Path


def percentile(sorted_values, p):
    if not sorted_values:
        return 0.0
    k = (len(sorted_values) - 1) * p / 100.0
    lo = int(k)
    hi = min(lo + 1, len(sorted_values) - 1)
    return sorted_values[lo] + (sorted_values[hi] - sorted_values[lo]) * (k - lo)


def low_fps(sorted_ms, fraction):
    n = max(1, int(len(sorted_ms) * fraction))
    worst = sorted_ms[-n:]
    return 1000.0 / (sum(worst) / len(worst))


def frames(path, skip_ms, ticks=None):
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f))
    start = next((i for i, r in enumerate(rows) if r["paused"] == "0"), len(rows))
    rows = rows[start:]
    if rows and skip_ms:
        t0 = float(rows[0]["at_ms"])
        rows = [r for r in rows if float(r["at_ms"]) - t0 >= skip_ms]
    if ticks:
        lo, hi = (int(v) for v in ticks.split(":"))
        kept, tick = [], 0
        for r in rows:
            if lo <= tick < hi:
                kept.append(r)
            tick += int(r["fixed_ticks"])
        rows = kept
    return rows


def summary(rows):
    ms = [float(r["frame_ms"]) for r in rows]
    s = sorted(ms)
    allocs = sorted(int(r["allocations"]) for r in rows)
    alloc_bytes = sorted(int(r["allocated_bytes"]) for r in rows)
    ents = [int(r["entities"]) for r in rows]
    mem = [float(r["process_gib"]) for r in rows if float(r["process_gib"]) > 0]
    total = sum(ms)
    return {
        "frames": len(ms),
        "seconds": round(total / 1000.0, 2),
        "avg_fps": round(len(ms) / (total / 1000.0), 1),
        "low_1pct_fps": round(low_fps(s, 0.01), 1),
        "low_0_1pct_fps": round(low_fps(s, 0.001), 1),
        "frame_ms": {
            "avg": round(total / len(ms), 3),
            "p50": round(percentile(s, 50), 3),
            "p95": round(percentile(s, 95), 3),
            "p99": round(percentile(s, 99), 3),
            "max": round(s[-1], 3),
        },
        "over_16_7ms": sum(1 for v in ms if v > 1000.0 / 60.0),
        "over_33ms": sum(1 for v in ms if v > 1000.0 / 30.0),
        "entities": {"min": min(ents), "max": max(ents)},
        "process_gib": {"min": round(min(mem), 3), "max": round(max(mem), 3)} if mem else None,
        "allocations_per_frame": {
            "avg": round(sum(allocs) / len(allocs), 1),
            "p50": percentile(allocs, 50),
            "p99": percentile(allocs, 99),
            "max": allocs[-1],
        },
        "allocated_kib_per_frame": {
            "avg": round(sum(alloc_bytes) / len(alloc_bytes) / 1024.0, 1),
            "p99": round(percentile(alloc_bytes, 99) / 1024.0, 1),
            "max": round(alloc_bytes[-1] / 1024.0, 1),
        },
    }


def log_timings(path):
    out = {"area_loads": [], "hitches": 0}
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        try:
            e = json.loads(line)
        except ValueError:
            continue
        kind = e.get("event")
        if kind == "first_frame":
            out["first_frame_ms"] = round(e["ms"])
        elif kind == "area_load":
            out["area_loads"].append({
                "area": e.get("area"),
                "worker_ms": round(e.get("worker_ms") or 0),
                "main_total_ms": round(e.get("main_total_ms") or 0, 1),
            })
        elif kind == "hitch":
            out["hitches"] += 1
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("frames")
    parser.add_argument("log", nargs="?")
    parser.add_argument("--skip-ms", type=float, default=0.0)
    parser.add_argument("--ticks")
    args = parser.parse_args()
    report = summary(frames(args.frames, args.skip_ms, args.ticks))
    if args.log:
        report["log"] = log_timings(args.log)
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
