import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET

SCRIPT = Path(__file__).with_name("nsh-macos-frames.py")
spec = importlib.util.spec_from_file_location("frames", SCRIPT)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def table(name, columns, rows):
    return ET.fromstring(
        '<trace-query-result><node><schema name="' + name + '">'
        + ''.join('<col><mnemonic>' + col + '</mnemonic></col>' for col in columns)
        + '</schema>' + ''.join('<row>' + row + '</row>' for row in rows)
        + '</node></trace-query-result>'
    )


def fixture():
    updates = table("hitches-updates", ["start", "duration", "process", "display", "swap-id", "surface-id", "frame-color", "containment-level", "label"], [
        '<start-time>1180000000</start-time><duration>10</duration><process id="10"><pid id="11">7</pid></process><display-name id="12">Display 1</display-name><uint32>102</uint32><uint32>9</uint32><sentinel/><uint32>0</uint32><sentinel/>',
        '<start-time>80000000</start-time><duration>10</duration><process ref="10"/><display-name ref="12"/><uint32>100</uint32><uint32>6</uint32><sentinel/><uint32>0</uint32><sentinel/>',
        '<start-time>180000000</start-time><duration>10</duration><process><pid>8</pid></process><display-name ref="12"/><uint32>101</uint32><uint32>8</uint32><sentinel/><uint32>0</uint32><sentinel/>',
    ])
    lifetimes = table("hitches-frame-lifetimes", ["start", "duration", "display", "swap-id", "surface-id", "frame-color", "layout-qualifier", "label"], [
        '<start-time id="10">80000000</start-time><duration>20000082</duration><display-name id="11">Display 1</display-name><uint32>100</uint32><uint32>6</uint32><sentinel/><sentinel/><sentinel/>',
        '<start-time>180000000</start-time><duration>20000000</duration><display-name ref="11"/><uint32>101</uint32><uint32>8</uint32><sentinel/><sentinel/><sentinel/>',
        '<start-time>1180000000</start-time><duration>20000125</duration><display-name ref="11"/><uint32>102</uint32><uint32>9</uint32><sentinel/><sentinel/><sentinel/>',
    ])
    swap_columns = ["timestamp", "delay", "display-name", "surface-id", "framebuffer-index", "swap-id", "color", "pixel-format", "hid-time", "generation-time", "min-quanta", "desired-presentation-time", "layer1-surface-id", "layer2-surface-id", "layer1-pixel-format", "layer2-pixel-format"]
    swaps = table("display-surface-swap", swap_columns, [
        '<start-time>100000000</start-time><start-time>0</start-time><display-name id="10">Built-In Display</display-name><displayed-surface-swap>6</displayed-surface-swap><uint32>17</uint32><displayed-surface-swap>100</displayed-surface-swap>' + '<sentinel/>' * 10,
        '<start-time>200000000</start-time><start-time>0</start-time><display-name ref="10"/><displayed-surface-swap>8</displayed-surface-swap><uint32>17</uint32><displayed-surface-swap>101</displayed-surface-swap>' + '<sentinel/>' * 10,
        '<start-time>1200000000</start-time><start-time>0</start-time><display-name ref="10"/><displayed-surface-swap>9</displayed-surface-swap><uint32>17</uint32><displayed-surface-swap>102</displayed-surface-swap>' + '<sentinel/>' * 10,
    ])
    return updates, lifetimes, swaps


class PresentFramesTest(unittest.TestCase):
    def test_uses_target_raw_swaps_and_keeps_idle_gap(self):
        result = module.present_frames(7, *fixture())
        self.assertEqual([f["presentNs"] for f in result["frames"]], [100000000, 1200000000])
        self.assertEqual([f["endMinusSwapNs"] for f in result["frames"]], [82, 125])
        self.assertEqual(result["frameMs"], [1100.0])
        self.assertEqual(result["kind"], "macos-present-diagnostic")

    def test_rejects_missing_target(self):
        with self.assertRaisesRegex(ValueError, "target"):
            module.present_frames(99, *fixture())

    def test_rejects_missing_or_ambiguous_swap(self):
        for duplicate in [False, True]:
            docs = fixture()
            rows = docs[2].find("node")
            if duplicate:
                rows.append(copy.deepcopy(rows.findall("row")[0]))
            else:
                rows.remove(rows.findall("row")[0])
            with self.subTest(duplicate=duplicate), self.assertRaises(ValueError):
                module.present_frames(7, *docs)

    def test_rejects_missing_or_ambiguous_lifetime(self):
        for duplicate in [False, True]:
            docs = fixture()
            rows = docs[1].find("node")
            if duplicate:
                rows.append(copy.deepcopy(rows.findall("row")[2]))
            else:
                rows.remove(rows.findall("row")[2])
            with self.subTest(duplicate=duplicate), self.assertRaises(ValueError):
                module.present_frames(7, *docs)

    def test_rejects_unknown_refs_and_duplicate_ids(self):
        for duplicate in [False, True]:
            docs = fixture()
            cell = docs[0].findall(".//row")[1][2]
            cell.set("id" if duplicate else "ref", "10" if duplicate else "999")
            with self.subTest(duplicate=duplicate), self.assertRaises(ValueError):
                module.present_frames(7, *docs)

    def test_rejects_multiple_displays_instead_of_interleaving_them(self):
        docs = fixture()
        cell = docs[2].findall(".//row")[2][2]
        cell.attrib.clear()
        cell.text = "External Display"
        with self.assertRaisesRegex(ValueError, "display"):
            module.present_frames(7, *docs)

    def test_rejects_non_integer_and_non_positive_timestamps(self):
        for timestamp in ["NaN", "-1", "100.5"]:
            docs = fixture()
            docs[2].find(".//row")[0].text = timestamp
            with self.subTest(timestamp=timestamp), self.assertRaises(ValueError):
                module.present_frames(7, *docs)

    def test_cli_saves_evidence_without_overwriting(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = [Path(directory) / (name + ".xml") for name in ["updates", "lifetimes", "swaps"]]
            for path, root in zip(paths, fixture()):
                ET.ElementTree(root).write(path, encoding="utf-8")
            output = Path(directory) / "result.json"
            args = [sys.executable, str(SCRIPT), "7", *map(str, paths), str(output)]
            first = subprocess.run(args, capture_output=True, text=True)
            self.assertEqual(first.returncode, 0, first.stderr)
            self.assertTrue(output.is_file(), "CLI must save the raw attributed frames")
            result = json.loads(output.read_text())
            self.assertEqual(result["frameMs"], [1100.0])
            self.assertEqual(len(result["sources"]), 3)
            saved = output.read_bytes()
            second = subprocess.run(args, capture_output=True)
            self.assertNotEqual(second.returncode, 0)
            self.assertEqual(output.read_bytes(), saved)


if __name__ == "__main__":
    unittest.main()
