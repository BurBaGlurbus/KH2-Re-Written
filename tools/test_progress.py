"""Authored synthetic coverage fixtures; no installed game data."""

import copy
import json
import tempfile
import unittest
from pathlib import Path

import progress


def fixture():
    return {
        "schema_version": 1,
        "game": "KH2 Final Mix",
        "baseline": "Synthetic test build - unqualified",
        "inventories": {
            "decompilation": {"complete": False, "inventory_evidence": [], "units": []},
            "fidelity": {"complete": False, "inventory_evidence": [], "units": []},
            "roadmap": {
                "complete": True,
                "inventory_evidence": ["docs/record.md"],
                "units": [{"id": f"P{i:02}", "label": "Synthetic gate",
                           "status": "NOT RUN", "evidence": []} for i in range(32)],
            },
        },
    }


def unit(identifier, status):
    return {"id": identifier, "label": "Synthetic function",
            "status": status, "evidence": ["docs/record.md"] if status == "PASS" else []}


class ProgressTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "docs/progress").mkdir(parents=True)
        (self.root / "docs/record.md").write_text("Authored fixture.\n", encoding="utf-8")
        self.data = fixture()

    def test_unknown_denominator_never_reports_zero_or_full_game_percent(self):
        self.data["inventories"]["decompilation"]["units"] = [unit("FN-01", "PASS")]
        progress.validate(self.data, self.root)
        result = progress.summarize(self.data)["inventories"]
        self.assertIsNone(result["fidelity"]["overall_percent"])
        self.assertIsNone(result["decompilation"]["overall_percent"])
        self.assertEqual(result["decompilation"]["tracked_percent"], 100)
        self.assertEqual(result["roadmap"]["overall_percent"], 0)

    def test_only_pass_counts_and_unknown_failed_blocked_units_stay(self):
        inventory = self.data["inventories"]["fidelity"]
        inventory["complete"] = True
        inventory["inventory_evidence"] = ["docs/record.md"]
        inventory["units"] = [unit(f"SC-{i}", status) for i, status in
                              enumerate(sorted(progress.STATUSES))]
        progress.validate(self.data, self.root)
        result = progress.summarize(self.data)["inventories"]["fidelity"]
        self.assertEqual(result["overall_percent"], 20)
        self.assertEqual(result["tracked"], 5)
        self.assertEqual(result["passed"], 1)

    def test_regression_reduces_coverage(self):
        inventory = self.data["inventories"]["decompilation"]
        inventory.update(complete=True, inventory_evidence=["docs/record.md"],
                         units=[unit("FN-01", "PASS"), unit("FN-02", "PASS")])
        before = progress.summarize(self.data)
        inventory["units"][0]["status"] = "FAIL"
        after = progress.summarize(self.data)
        self.assertEqual(before["inventories"]["decompilation"]["overall_percent"], 100)
        self.assertEqual(after["inventories"]["decompilation"]["overall_percent"], 50)
        self.assertNotEqual(before["ledger_sha256"], after["ledger_sha256"])

    def test_missing_gate_duplicate_and_unproved_pass_rejected(self):
        for change in ("missing", "duplicate", "pass"):
            with self.subTest(change=change):
                data = copy.deepcopy(self.data)
                units = data["inventories"]["roadmap"]["units"]
                if change == "missing":
                    units.pop()
                elif change == "duplicate":
                    units.append(copy.deepcopy(units[0]))
                else:
                    units[0]["status"] = "PASS"
                with self.assertRaises(ValueError):
                    progress.validate(data, self.root)

    def test_missing_private_and_traversing_evidence_rejected(self):
        for link in ("docs/missing.md", "../private/report.md", "docs/../record.md",
                     "C:/private/report.md", "https://example.com/report.md"):
            with self.subTest(link=link):
                self.data["inventories"]["roadmap"]["inventory_evidence"] = [link]
                with self.assertRaises(ValueError):
                    progress.validate(self.data, self.root)

    def test_complete_inventory_requires_evidence_and_units(self):
        for units, links in (([], ["docs/record.md"]), ([unit("FN-01", "PASS")], [])):
            with self.subTest(units=units):
                self.data["inventories"]["decompilation"].update(
                    complete=True, units=units, inventory_evidence=links)
                with self.assertRaises(ValueError):
                    progress.validate(self.data, self.root)

    def test_private_metadata_markup_and_other_game_rejected(self):
        for value in ("C:/Users/name/game", "0x140000000", "<script>", "private\\file"):
            with self.subTest(value=value):
                self.data["baseline"] = value
                with self.assertRaises(ValueError):
                    progress.validate(self.data, self.root)
        self.data = fixture()
        self.data["game"] = "Other collection game"
        with self.assertRaises(ValueError):
            progress.validate(self.data, self.root)
        self.data = fixture()
        self.data["source_path"] = "private"
        with self.assertRaises(ValueError):
            progress.validate(self.data, self.root)

    def test_generation_is_idempotent_and_preserves_surrounding_readme(self):
        prefix, suffix = "# User introduction\n\n", "\n\nUser instructions.\n"
        readme = prefix + progress.START + "\n" + progress.END + suffix
        (self.root / "README.md").write_text(readme, encoding="utf-8")
        (self.root / "docs/progress/coverage.json").write_text(
            json.dumps(self.data), encoding="utf-8")
        self.assertEqual(len(progress.generate(self.root)), 3)
        self.assertEqual(progress.generate(self.root), [])
        self.assertEqual(progress.generate(self.root, check=True), [])
        output = (self.root / "README.md").read_text(encoding="utf-8")
        self.assertTrue(output.startswith(prefix))
        self.assertTrue(output.endswith(suffix))
        (self.root / "docs/progress/summary.json").write_text("stale", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "Stale progress outputs"):
            progress.generate(self.root, check=True)
        self.assertEqual((self.root / "docs/progress/summary.json").read_text(), "stale")

    def test_readme_without_markers_is_left_alone(self):
        readme = "# Project\n\nNo progress block here.\n"
        (self.root / "README.md").write_text(readme, encoding="utf-8")
        (self.root / "docs/progress/coverage.json").write_text(
            json.dumps(self.data), encoding="utf-8")
        self.assertNotIn("README.md", progress.generate(self.root))
        self.assertEqual((self.root / "README.md").read_text(encoding="utf-8"), readme)

    def test_bad_markers_fail_without_mutating_readme(self):
        for text in ("user text", progress.END + progress.START,
                     progress.START + progress.START + progress.END):
            with self.subTest(text=text):
                with self.assertRaises(ValueError):
                    progress.replace_block(text, "generated")


if __name__ == "__main__":
    unittest.main()
