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


def raw_fixture():
    columns = ["time", "core-index", "class", "subclass", "code", "function",
               "arg1", "arg2", "arg3", "arg4"]
    events = []
    for swap, surface, ticks in [(100, 6, 1200), (101, 8, 1400), (102, 9, 3400)]:
        for core in [17, 18]:
            events.extend([
                [1, core, 0x31, 0x80, 0xc8, 0, swap, surface, 0, 0],
                [2_000_000_000, core, 0x31, 0x80, 0xd1, 0,
                 swap, 1, 0, ticks + (1 if swap == 100 and core == 18 else 0)],
            ])
    raw = table("kdebug", columns, [
        ''.join('<uint64>' + str(v) + '</uint64>' for v in event) for event in events
    ])
    clock = table("time-info", ["update-time", "mct-epoch", "timebase-info"], [
        '<sample-time>0</sample-time><mach-continuous-time>1000</mach-continuous-time>'
        '<mach-timebase-info><field>1000000</field><field>2</field></mach-timebase-info>'
    ])
    return raw, clock


class PresentFramesTest(unittest.TestCase):
    def test_raw_inputs_must_be_supplied_together(self):
        raw, clock = raw_fixture()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "raw.xml"
            ET.ElementTree(raw).write(path)
            for extra in [{"raw_display": path}, {"time_info": clock}]:
                with self.subTest(extra=list(extra)), self.assertRaises(ValueError):
                    module.present_frames(7, *fixture(), **extra)

    def test_timebase_fields_resolve_their_document_references(self):
        raw, clock = raw_fixture()
        epoch = clock.find(".//mach-continuous-time")
        epoch.text = "1000000"
        epoch.set("id", "epoch")
        numerator = clock.find(".//mach-timebase-info")[0]
        numerator.text = None
        numerator.set("ref", "epoch")
        for row in raw.findall(".//row"):
            if row[4].text == str(0xd1):
                row[-1].text = str(int(row[-1].text) + 999000)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "raw.xml"
            ET.ElementTree(raw).write(path)
            result = module.present_frames(7, *fixture(), raw_display=path, time_info=clock)
        self.assertEqual(result["frameMs"], [1100.0])

    def test_recovers_only_verified_on_glass_events_and_keeps_time_bounds(self):
        docs = fixture()
        docs[2].find("node").remove(docs[2].findall(".//row")[0])
        for row in docs[2].findall(".//row"):
            row[2].attrib.clear()
            row[2].text = "Built-In Display"
        raw, clock = raw_fixture()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "raw.xml"
            ET.ElementTree(raw).write(path)
            result = module.present_frames(7, *docs, raw_display=path, time_info=clock)
        self.assertEqual(result["rawValidatedSwaps"], 2)
        self.assertEqual(result["recoveredSwaps"], 1)
        self.assertEqual(result["frames"][0]["presentNs"], 100000000)
        self.assertEqual(result["frames"][0]["presentMaxNs"], 100500000)
        self.assertEqual(result["frameMs"], [1100.0])
        self.assertEqual(result["frameMsLower"], [1099.5])

    def test_recovered_later_frame_uses_upper_interval_bound(self):
        docs = fixture()
        docs[2].find("node").remove(docs[2].findall(".//row")[2])
        raw, clock = raw_fixture()
        raw.findall(".//row")[-1][-1].text = "3401"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "raw.xml"
            ET.ElementTree(raw).write(path)
            result = module.present_frames(7, *docs, raw_display=path, time_info=clock)
        self.assertEqual(result["frameMs"], [1100.5])
        self.assertEqual(result["frameMsLower"], [1100.0])

    def test_cli_raw_sources_are_hashed_and_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = []
            for i, document in enumerate([*fixture(), *raw_fixture()]):
                path = Path(directory) / (str(i) + ".xml")
                ET.ElementTree(document).write(path)
                paths.append(path)
            output = Path(directory) / "present.json"
            args = [sys.executable, str(SCRIPT), "7", *map(str, paths[:3]),
                    str(output), *map(str, paths[3:])]
            first = subprocess.run(args, capture_output=True, text=True)
            self.assertEqual(first.returncode, 0, first.stderr)
            saved = output.read_bytes()
            result = json.loads(saved)
            self.assertEqual(len(result["sources"]), 5)
            self.assertEqual(result["recoveredSwaps"], 0)
            second = subprocess.run(args, capture_output=True)
            self.assertNotEqual(second.returncode, 0)
            self.assertEqual(output.read_bytes(), saved)

    def test_raw_recovery_rejects_clock_surface_and_event_conflicts(self):
        for mutation in ["clock", "surface", "duplicate", "missing_program", "late_program", "unknown_ref"]:
            docs = fixture()
            raw, clock = raw_fixture()
            raw_rows = raw.findall(".//row")
            if mutation == "clock":
                clock.find(".//mach-continuous-time").text = "999"
            elif mutation == "surface":
                raw_rows[2][7].text = "99"
            elif mutation == "duplicate":
                raw.find("node").append(copy.deepcopy(raw_rows[1]))
            elif mutation == "missing_program":
                for row in raw_rows[:4:2]:
                    raw.find("node").remove(row)
            elif mutation == "late_program":
                for row in raw_rows[:4:2]:
                    raw.find("node").remove(row)
                    raw.find("node").append(row)
            else:
                raw_rows[0][0].set("ref", "missing")
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "raw.xml"
                ET.ElementTree(raw).write(path)
                with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                    module.present_frames(7, *docs, raw_display=path, time_info=clock)

    def test_uses_target_raw_swaps_and_keeps_idle_gap(self):
        result = module.present_frames(7, *fixture())
        self.assertEqual([f["presentNs"] for f in result["frames"]], [100000000, 1200000000])
        self.assertEqual([f["endMinusSwapNs"] for f in result["frames"]], [82, 125])
        self.assertEqual(result["frameMs"], [1100.0])
        self.assertEqual(result["kind"], "macos-present-diagnostic")

    def test_rejects_missing_target(self):
        with self.assertRaisesRegex(ValueError, "target"):
            module.present_frames(99, *fixture())

    def test_nested_update_rows_describe_one_presented_frame(self):
        for nested_first in [False, True]:
            docs = fixture()
            rows = docs[0].find("node")
            nested = copy.deepcopy(rows.findall("row")[1])
            nested[6].text = "Red"
            nested[7].text = "1"
            rows.insert(1 if nested_first else len(rows), nested)
            with self.subTest(nested_first=nested_first):
                result = module.present_frames(7, *docs)
                self.assertEqual([f["presentNs"] for f in result["frames"]], [100000000, 1200000000])
                self.assertEqual(result["frameMs"], [1100.0])
                self.assertEqual(result["nestedUpdateRows"], 1)

    def test_rejects_conflicting_or_duplicate_nested_updates(self):
        for column, value in [(0, "80000001"), (1, "11"), (7, "0")]:
            docs = fixture()
            rows = docs[0].find("node")
            nested = copy.deepcopy(rows.findall("row")[1])
            nested[7].text = "1"
            nested[column].text = value
            rows.append(nested)
            with self.subTest(column=column), self.assertRaisesRegex(ValueError, "ambiguous"):
                module.present_frames(7, *docs)

    def test_rejects_nested_updates_without_one_root_or_with_repeated_depth(self):
        for depths in [[1, 2], [0, 1, 1]]:
            docs = fixture()
            rows = docs[0].find("node")
            original = rows.findall("row")[1]
            original[7].text = str(depths[0])
            for depth in depths[1:]:
                nested = copy.deepcopy(original)
                nested[7].text = str(depth)
                rows.append(nested)
            with self.subTest(depths=depths), self.assertRaisesRegex(ValueError, "ambiguous"):
                module.present_frames(7, *docs)

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
