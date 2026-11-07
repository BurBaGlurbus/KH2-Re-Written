"""Generate public KH2FM progress from an explicit, reviewed coverage ledger."""

import argparse
import hashlib
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
START = "<!-- KH2FM-PROGRESS:START -->"
END = "<!-- KH2FM-PROGRESS:END -->"
STATUSES = {"PASS", "PARTIAL", "FAIL", "NOT RUN", "BLOCKED"}
LABELS = {
    "decompilation": "Complete functions decompiled and checked",
    "fidelity": "Required behavior scenarios verified 1:1",
    "roadmap": "Final roadmap gates accepted",
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def keys(value, expected):
    require(isinstance(value, dict) and set(value) == set(expected),
            "Unexpected or missing ledger fields")


def public_text(value):
    require(isinstance(value, str) and 0 < len(value) <= 180,
            "Expected a short public label")
    require(re.fullmatch(r"[A-Za-z0-9 .,:;!?()/'&+_-]+", value) is not None,
            "Labels must contain plain public text, without paths or markup")
    require(not re.search(r"(?i)([a-z]:|/users/|/home/|%|0x[0-9a-f]+)", value),
            "Private paths and executable addresses are not public labels")


def evidence(value, root):
    require(isinstance(value, list), "Evidence must be a list")
    for link in value:
        require(isinstance(link, str) and re.fullmatch(
            r"docs/[a-zA-Z0-9_/-]+\.md(?:#[a-zA-Z0-9_-]+)?", link),
            "Evidence must link to a public repository Markdown record")
        file = root / link.split("#", 1)[0]
        require(file.is_file() and file.resolve().is_relative_to(root.resolve()),
                "Evidence file is missing or outside the repository")


def validate(data, root):
    keys(data, {"schema_version", "game", "baseline", "inventories"})
    require(type(data["schema_version"]) is int and data["schema_version"] == 1,
            "Unsupported ledger schema")
    require(data["game"] == "KH2 Final Mix", "Only KH2 Final Mix is in scope")
    public_text(data["baseline"])
    keys(data["inventories"], LABELS)
    for kind, inventory in data["inventories"].items():
        keys(inventory, {"complete", "inventory_evidence", "units"})
        require(type(inventory["complete"]) is bool, "complete must be boolean")
        evidence(inventory["inventory_evidence"], root)
        require(isinstance(inventory["units"], list), "units must be a list")
        if inventory["complete"]:
            require(inventory["units"] and inventory["inventory_evidence"],
                    "A complete denominator needs units and inventory evidence")
        seen = set()
        for unit in inventory["units"]:
            keys(unit, {"id", "label", "status", "evidence"})
            require(isinstance(unit["id"], str) and
                    re.fullmatch(r"[A-Z][A-Z0-9-]{1,39}", unit["id"]),
                    "Use opaque public unit IDs")
            require(unit["id"] not in seen, "Duplicate unit ID")
            seen.add(unit["id"])
            public_text(unit["label"])
            require(isinstance(unit["status"], str) and unit["status"] in STATUSES,
                    "Unknown unit status")
            evidence(unit["evidence"], root)
            require(unit["status"] != "PASS" or unit["evidence"],
                    "PASS requires a public acceptance evidence record")
        if kind == "roadmap":
            require(inventory["complete"] and
                    seen == {f"P{i:02}" for i in range(32)},
                    "The roadmap denominator must retain every P00-P31 gate")


def summarize(data):
    result = {
        "schema_version": 1,
        "game": data["game"],
        "baseline": data["baseline"],
        "ledger_sha256": hashlib.sha256(json.dumps(
            data, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
        "inventories": {},
    }
    for kind, inventory in data["inventories"].items():
        counts = {status: 0 for status in sorted(STATUSES)}
        for unit in inventory["units"]:
            counts[unit["status"]] += 1
        total = len(inventory["units"])
        passed = counts["PASS"]
        result["inventories"][kind] = {
            "complete": inventory["complete"],
            "tracked": total,
            "passed": passed,
            "statuses": counts,
            "tracked_percent": round(100 * passed / total, 2) if total else None,
            "overall_percent": (round(100 * passed / total, 2)
                                if inventory["complete"] else None),
        }
    return result


def percent(value):
    return "Unmeasured" if value is None else f"{value:.2f}%"


def overview(summary):
    lines = [
        "| Measure | Accepted / tracked | Percentage |",
        "|---|---:|---:|",
    ]
    for kind, label in LABELS.items():
        row = summary["inventories"][kind]
        count = f"{row['passed']} / {row['tracked']}"
        if not row["complete"]:
            count += " (inventory incomplete)"
        lines.append(f"| {label} | {count} | {percent(row['overall_percent'])} |")
    return "\n".join(lines)


def readme_block(summary):
    return "\n".join([
        START,
        "### Verified progress",
        "",
        overview(summary),
        "",
        "Whole-game 1:1 coverage is **unmeasured** until the complete function and "
        "behavior inventories are qualified. Partial work, synthetic tests and "
        "successful builds do not count as 1:1 passes. Gate percentage measures "
        "accepted gates, not game size or implementation effort.",
        "",
        "[Progress details](docs/progress/README.md) · "
        "[Coverage ledger](docs/progress/coverage.json) · "
        "[Automatic updates](.github/workflows/progress.yml)",
        END,
    ])


def replace_block(readme, block):
    require(readme.count(START) == 1 and readme.count(END) == 1,
            "README must contain exactly one progress marker pair")
    begin = readme.index(START)
    finish = readme.index(END)
    require(begin < finish, "README progress markers are reversed")
    return readme[:begin] + block + readme[finish + len(END):]


def report(data, summary):
    lines = [
        "# KH2 Final Mix progress",
        "",
        f"Baseline: {data['baseline']}.",
        "",
        overview(summary),
        "",
        "A PASS counts one reviewed unit. All other statuses count zero. Unknown "
        "required units stay in the denominator; an incomplete inventory suppresses "
        "the overall percentage. Functions and behavior scenarios have separate "
        "denominators and are never averaged into a whole-game score.",
        "",
        "## Roadmap acceptance",
        "",
        "| Gate | Objective | Verdict | Evidence |",
        "|---|---|---|---|",
    ]
    for unit in sorted(data["inventories"]["roadmap"]["units"],
                       key=lambda item: item["id"]):
        links = ", ".join(f"[record](../../{link})" for link in unit["evidence"])
        lines.append(f"| {unit['id']} | {unit['label']} | {unit['status']} | {links} |")
    for kind in ("decompilation", "fidelity"):
        lines.extend(["", f"## {LABELS[kind]}", ""])
        inventory = data["inventories"][kind]
        if not inventory["units"]:
            lines.append("No complete accepted units have been entered; the full "
                         "denominator remains unqualified. This does not mean no "
                         "research or implementation exists.")
        else:
            lines.extend(["| Unit | Scope | Verdict | Evidence |",
                          "|---|---|---|---|"])
            for unit in sorted(inventory["units"], key=lambda item: item["id"]):
                links = ", ".join(f"[record](../../{link})" for link in unit["evidence"])
                lines.append(f"| {unit['id']} | {unit['label']} | {unit['status']} | {links} |")
        row = summary["inventories"][kind]
        lines.extend(["", f"Accepted share of the tracked subset: "
                      f"{percent(row['tracked_percent'])}. "
                      "This is not a whole-game percentage."])
    lines.extend([
        "",
        "## Maintaining the ledger",
        "",
        "Edit `docs/progress/coverage.json` when a reviewed evidence record changes "
        "a verdict. Add one opaque ID per complete logical function or required "
        "behavior scenario, including untested and blocked units. Keep IDs stable "
        "and retain failed units. Do not enter addresses, disassembly, pseudocode, "
        "source hashes, personal paths or private evidence files.",
        "",
        "For decompilation, PASS requires qualified complete logical function "
        "bounds and a reviewed source-bound decoding/decompilation check. An "
        "interior slice or byte identity alone is PARTIAL. For fidelity, PASS "
        "requires a controlled installed/rewrite comparison under the fidelity "
        "contract with no unresolved required discrepancy. Roadmap PASS requires "
        "the final gate acceptance, not a task's local build/test PASS.",
        "",
        "Mark an inventory complete only after its public inventory evidence "
        "documents full KH2FM scope, shared dependencies, deduplication, exclusions "
        "and remaining unknown units. Every PASS links to its public acceptance "
        "record. The generator validates structure and links; evidence review "
        "still determines whether the claim is justified.",
        "",
        "Run `python tools/progress.py` to refresh the README, this report and "
        "`summary.json`. Run `python tools/progress.py --check` to detect stale "
        "outputs. JSON null means unmeasured, never zero. The output is deterministic "
        "and contains only public ledger labels, statuses, counts and a ledger "
        "digest. It never reads Steam files or private research.",
        "",
        "## GitHub refresh",
        "",
        "The progress workflow tests and generates on each main-branch push, daily "
        "at 13:17 UTC and on manual dispatch. Pull requests validate and preview "
        "without write permissions. Only changed generated files are committed; "
        "unchanged schedules create no commits. A concurrent main update is "
        "rejected by normal fast-forward push; the next push or scheduled run "
        "regenerates from the new head. Branch protection may require a maintainer "
        "to allow the bot's documentation updates.",
        "",
        "GitHub schedules run on the default branch and may be delayed: "
        "[workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax). "
        "The built-in GITHUB_TOKEN prevents generated commits from recursively "
        "triggering more push workflows: "
        "[triggering workflows](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow). "
        "Refreshing reports cannot discover private evidence or advance a verdict "
        "without a reviewed ledger update.",
        "",
    ])
    return "\n".join(lines)


def generate(root, check=False):
    data = json.loads((root / "docs/progress/coverage.json").read_text(encoding="utf-8"))
    validate(data, root)
    summary = summarize(data)
    outputs = {
        "docs/progress/README.md": report(data, summary),
        "docs/progress/summary.json": json.dumps(summary, indent=2) + "\n",
    }
    # The README progress block is optional: only refresh it when the markers exist.
    readme = (root / "README.md").read_text(encoding="utf-8")
    if START in readme or END in readme:
        outputs["README.md"] = replace_block(readme, readme_block(summary))
    stale = []
    for name, content in outputs.items():
        file = root / name
        if file.is_file() and file.read_text(encoding="utf-8") == content:
            continue
        stale.append(name)
        if not check:
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text(content, encoding="utf-8", newline="\n")
    if check and stale:
        raise ValueError("Stale progress outputs: " + ", ".join(stale))
    return stale


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        changed = generate(ROOT, args.check)
    except (ValueError, OSError, TypeError) as error:
        parser.exit(1, f"Progress error: {error}\n")
    print("Progress current" if not changed else "Updated: " + ", ".join(changed))
