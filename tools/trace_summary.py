"""Total the spans of a Chrome trace written by a `perf-trace` build.

Usage: python tools/trace_summary.py <trace.json> [--top N] [--frame-span NAME]
           [--from-s S] [--to-s S]

Prints, per span name, its total, count, mean and per-frame time, sorted by
total. A frame is one occurrence of --frame-span (default: Bevy's
`main app` update span). Spans nest, so totals of parents include children.
--from-s/--to-s keep spans starting inside that window of the trace.
"""

import argparse
import json
from collections import defaultdict


def events(path):
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip().rstrip(",")
            if not line or line in ("[", "]"):
                continue
            try:
                yield json.loads(line)
            except ValueError:
                continue


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("trace")
    parser.add_argument("--top", type=int, default=60)
    parser.add_argument("--frame-span", default="main app")
    parser.add_argument("--from-s", type=float, default=0.0)
    parser.add_argument("--to-s", type=float, default=float("inf"))
    args = parser.parse_args()

    totals = defaultdict(float)
    counts = defaultdict(int)
    worst = defaultdict(float)
    stacks = defaultdict(list)
    t0 = None

    def add(name, start, dur):
        rel = (start - t0) / 1e6
        if rel < args.from_s or rel > args.to_s:
            return
        totals[name] += dur
        counts[name] += 1
        worst[name] = max(worst[name], dur)

    for e in events(args.trace):
        ph = e.get("ph")
        ts = e.get("ts")
        if ts is None:
            continue
        ts = float(ts)
        if t0 is None:
            t0 = ts
        key = (e.get("pid"), e.get("tid"))
        if ph == "X":
            add(e.get("name", "?"), ts, float(e.get("dur", 0)))
        elif ph == "B":
            stacks[key].append((e.get("name", "?"), ts))
        elif ph == "E" and stacks[key]:
            name, start = stacks[key].pop()
            add(name, start, ts - start)

    frames = counts.get(args.frame_span, 0) or 1
    print(f"frames ({args.frame_span!r}): {counts.get(args.frame_span, 0)}")
    print(f"{'total ms':>10} {'count':>8} {'mean us':>9} {'max ms':>8} {'ms/frame':>9}  span")
    for name, total in sorted(totals.items(), key=lambda kv: -kv[1])[: args.top]:
        print(f"{total / 1e3:10.1f} {counts[name]:8d} {total / counts[name]:9.1f} "
              f"{worst[name] / 1e3:8.2f} {total / 1e3 / frames:9.3f}  {name}")


if __name__ == "__main__":
    main()
