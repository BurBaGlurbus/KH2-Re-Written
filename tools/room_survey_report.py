"""Rooms by frame time from a `--room-survey` run.

Usage: python tools/room_survey_report.py <run-dir> [top]

The directory holds the run's log as run.jsonl.
"""
import json
import statistics
import sys
from pathlib import Path

d = Path(sys.argv[1])
top = int(sys.argv[2]) if len(sys.argv) > 2 else 25
rows, errors = [], []
for l in (d / "run.jsonl").read_text(encoding="utf-8").splitlines():
    try:
        e = json.loads(l)
    except ValueError:
        continue
    if e.get("event") == "room_survey":
        rows.append(e)
    elif e.get("event") == "area_error":
        errors.append((e["world"], e["area"], e["error"][:70]))
print("rooms", len(rows), "load errors", len(errors))
med = [r["times"]["median_ms"] for r in rows if r["times"].get("frames")]
if med:
    print("median of medians", round(statistics.median(med), 2), "max", round(max(med), 2))
rows.sort(key=lambda r: -r["times"].get("median_ms", 0))
print("room     median  p99 low1%  frames ents drawn hidden  verts   images npc bob")
for r in rows[:top]:
    t = r["times"]
    print(
        f"{r['code']}{r['area']:02d}   {t.get('median_ms', 0):7.2f} {t.get('p99_ms', 0):6.1f} {t.get('low_1pct_ms', 0):6.1f}"
        f" {t.get('frames', 0):6d} {r['entities']:5d} {r['drawn_meshes']:5d} {r['hidden_meshes']:5d} {r['vertices']:7d}"
        f" {r['images']:5d} {r['npcs']:3d} {r['bobs']:3d}"
    )
print("errors", errors[:12])
xs = [(r["drawn_meshes"], r["vertices"], r["times"]["median_ms"]) for r in rows if r["times"].get("frames")]
if len(xs) > 2:
    print("corr median vs drawn", round(statistics.correlation([x[0] for x in xs], [x[2] for x in xs]), 2))
    print("corr median vs verts", round(statistics.correlation([x[1] for x in xs], [x[2] for x in xs]), 2))
