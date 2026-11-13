"""Extract process-attributed compositor swaps from three xctrace XML exports."""

import hashlib
import json
from pathlib import Path
import re
import sys
import xml.etree.ElementTree as ET


def integer(value):
    if not isinstance(value, str) or not re.fullmatch(r"[0-9]+", value):
        raise ValueError("expected a non-negative integer in xctrace data")
    return int(value)


def rows(root, schema_name, required):
    ids = {}
    for node in root.iter():
        if "id" in node.attrib:
            key = node.get("id")
            if key in ids or "ref" in node.attrib:
                raise ValueError("duplicate or ambiguous xctrace id")
            ids[key] = node

    def resolve(node):
        seen = set()
        while "ref" in node.attrib:
            ref = node.get("ref")
            if ref in seen or ref not in ids:
                raise ValueError("cyclic or unknown xctrace ref")
            seen.add(ref)
            node = ids[ref]
        return node

    # References are scoped to each exported document, never shared across files.
    for node in root.iter():
        resolve(node)
    tables = [node for node in root.findall(".//node")
              if node.find("schema") is not None
              and node.find("schema").get("name") == schema_name]
    if len(tables) != 1:
        raise ValueError("expected one xctrace table: " + schema_name)
    table = tables[0]
    names = [col.findtext("mnemonic") for col in table.findall("schema/col")]
    if len(set(names)) != len(names) or not required.issubset(names):
        raise ValueError("missing or ambiguous xctrace columns")
    result = []
    for row in table.findall("row"):
        if len(row) != len(names):
            raise ValueError("xctrace row/column count mismatch")
        values = {}
        for name, cell in zip(names, row):
            node = resolve(cell)
            if name == "process":
                node = node.find("pid")
                if node is None:
                    raise ValueError("missing target process PID")
                node = resolve(node)
            values[name] = node.text
        result.append(values)
    return result


def raw_swap_bounds(path, time_info, swaps):
    clock = rows(time_info, "time-info", {"update-time", "mct-epoch", "timebase-info"})
    bases = time_info.findall(".//mach-timebase-info")
    if len(clock) != 1 or clock[0]["update-time"] != "0" or len(bases) != 1 or len(bases[0]) != 2:
        raise ValueError("requires one initial timebase")
    epoch = integer(clock[0]["mct-epoch"])
    clock_ids = {node.get("id"): node for node in time_info.iter() if "id" in node.attrib}
    numerator, denominator = [integer((clock_ids[field.get("ref")] if "ref" in field.attrib else field).text)
                              for field in bases[0]]
    if not epoch or not numerator or not denominator:
        raise ValueError("invalid continuous timebase")
    if path.stat().st_size > 1024 * 1024 * 1024:
        raise ValueError("raw display export exceeds 1 GiB")

    ids, surfaces, glass = {}, {}, {}
    columns, table, tables = None, None, 0
    required = {"time", "core-index", "class", "subclass", "code", "function",
                "arg1", "arg2", "arg3", "arg4"}
    for event, node in ET.iterparse(path, events=("start", "end")):
        if event == "start":
            if node.tag == "node":
                table, tables = node, tables + 1
                if tables != 1:
                    raise ValueError("requires one raw display table")
            continue
        if node.tag == "schema":
            columns = [col.findtext("mnemonic") for col in node.findall("col")]
            if node.get("name") != "kdebug" or len(set(columns)) != len(columns) or not required.issubset(columns):
                raise ValueError("invalid raw display schema")
        if node.tag != "row":
            continue
        if columns is None or table is None or len(node) != len(columns):
            raise ValueError("invalid raw display row")
        for cell in node.iter():
            key, ref = cell.get("id"), cell.get("ref")
            if key is not None:
                if key in ids or ref is not None:
                    raise ValueError("duplicate or ambiguous raw display id")
                ids[key] = cell.text
            if ref is not None and ref not in ids:
                raise ValueError("unknown raw display ref")
        value = {name: ids[cell.get("ref")] if "ref" in cell.attrib else cell.text
                 for name, cell in zip(columns, node) if name in required}
        value = {name: integer(text) for name, text in value.items()}
        # shortcut: IOMFB 0x31/0x80 with continuous time; other drivers need a calibrated decoder.
        if (value["class"], value["subclass"], value["function"]) == (0x31, 0x80, 0):
            swap, core = value["arg1"], value["core-index"]
            if value["code"] == 0xc8:
                surface = value["arg2"]
                if swap in surfaces and surfaces[swap] != surface:
                    raise ValueError("conflicting raw display surface")
                surfaces[swap] = surface
            elif value["code"] == 0xd1 and swap in surfaces:
                if core in glass.get(swap, {}) or value["arg3"] >= 2**32 or value["arg4"] >= 2**32:
                    raise ValueError("duplicate or invalid on-glass event")
                ticks = (value["arg3"] << 32) | value["arg4"]
                present = (ticks - epoch) * numerator // denominator
                if present <= 0 or present > value["time"]:
                    raise ValueError("invalid on-glass timestamp")
                glass.setdefault(swap, {})[core] = present
        table.remove(node)
    if tables != 1 or columns is None:
        raise ValueError("missing raw display table")

    validated_cores = set()
    for row in swaps:
        swap, surface, present = integer(row["swap-id"]), integer(row["surface-id"]), integer(row["timestamp"])
        matched = {core for core, time in glass.get(swap, {}).items() if time == present}
        if surfaces.get(swap) != surface or not matched:
            raise ValueError("raw display does not exactly match every exported swap")
        validated_cores.update(matched)
    bounds = {}
    for swap, observations in glass.items():
        if swap in surfaces:
            times = [time for core, time in observations.items() if core in validated_cores]
            if times:
                bounds[swap, surfaces[swap]] = times
    return {key: (min(times), max(times)) for key, times in bounds.items()}


def present_frames(pid, updates, lifetimes, swaps, *, raw_display=None, time_info=None):
    if not isinstance(pid, int) or pid <= 0:
        raise ValueError("target PID must be positive")
    if (raw_display is None) != (time_info is None):
        raise ValueError("raw display and timebase must be supplied together")
    updates = rows(updates, "hitches-updates", {"process", "start", "duration", "display", "swap-id", "surface-id", "containment-level"})
    lifetimes = rows(lifetimes, "hitches-frame-lifetimes", {"start", "duration", "display", "swap-id", "surface-id"})
    swaps = rows(swaps, "display-surface-swap", {"timestamp", "display-name", "swap-id", "surface-id"})
    target = [row for row in updates if integer(row["process"]) == pid]
    if not target:
        raise ValueError("no updates attributed to target PID")
    # Hitches and display-surface-swap use different display names; only one is unambiguous.
    for data, column in [(updates, "display"), (lifetimes, "display"), (swaps, "display-name")]:
        names = {row[column] for row in data}
        if len(names) != 1 or None in names:
            raise ValueError("requires one display; multiple display attribution is unsupported")
    if target[0]["display"] != lifetimes[0]["display"]:
        raise ValueError("update and lifetime display do not match")

    def key(row):
        return integer(row["swap-id"]), integer(row["surface-id"])

    groups = {}
    for update in target:
        groups.setdefault(key(update), []).append(update)
    nested_update_rows = len(target) - len(groups)
    target = []
    for group in groups.values():
        if len(group) > 1:
            levels = [integer(row["containment-level"]) for row in group]
            identity = ("process", "start", "duration", "display", "swap-id", "surface-id")
            # Hitches repeats a frame for nested visualization rows; these are not extra swaps.
            if (levels.count(0) != 1 or len(set(levels)) != len(levels)
                    or any(row[column] != group[0][column] for row in group for column in identity)):
                raise ValueError("ambiguous swap/surface association in nested updates")
        target.append(group[0])

    def index(data):
        result = {}
        for row in data:
            k = key(row)
            if k in result:
                raise ValueError("ambiguous swap/surface association")
            result[k] = row
        return result

    swap_index, lifetime_index = index(swaps), index(lifetimes)
    bounds = raw_swap_bounds(raw_display, time_info, swaps) if raw_display is not None else {}
    recovered = 0
    frames = []
    for update in target:
        k = key(update)
        if k not in lifetime_index or (k not in swap_index and k not in bounds):
            raise ValueError("target update is missing a swap or lifetime")
        lifetime = lifetime_index[k]
        if k in swap_index:
            present = upper = integer(swap_index[k]["timestamp"])
        else:
            present, upper = bounds[k]
            recovered += 1
        start = integer(update["start"])
        if present <= 0 or start > present:
            raise ValueError("invalid presentation timestamp")
        frame = {
            "swapId": k[0], "surfaceId": k[1], "presentNs": present,
            "updateStartNs": start,
            "endMinusSwapNs": integer(lifetime["start"]) + integer(lifetime["duration"]) - present,
        }
        if k not in swap_index:
            frame["presentMaxNs"] = upper
            frame["presentationSource"] = "IOMFB_V2_SWAP_ON_GLASS"
        frames.append(frame)
    frames.sort(key=lambda frame: frame["presentNs"])
    intervals = [(b.get("presentMaxNs", b["presentNs"]) - a["presentNs"]) / 1_000_000 for a, b in zip(frames, frames[1:])]
    lower = [(b["presentNs"] - a.get("presentMaxNs", a["presentNs"])) / 1_000_000 for a, b in zip(frames, frames[1:])]
    if len(frames) < 2 or any(ms <= 0 for ms in lower):
        raise ValueError("requires distinct, increasing target presentations")
    result = {
        "kind": "macos-present-diagnostic", "pid": pid,
        "nestedUpdateRows": nested_update_rows,
        "frames": frames, "frameMs": intervals,
        "notAcceptance": "Process-attributed compositor swaps; input coverage, layer completeness, GPU memory and other acceptance metrics require separate evidence.",
    }
    if raw_display is not None:
        result.update(rawValidatedSwaps=len(swaps), recoveredSwaps=recovered, frameMsLower=lower)
    return result


def main():
    if len(sys.argv) not in (6, 8):
        raise ValueError("Usage: python3 scripts/nsh-macos-frames.py PID updates.xml lifetimes.xml swaps.xml output.json [raw-display.xml time-info.xml]")
    paths = [Path(path) for path in sys.argv[2:5]]
    documents, sources = [], []
    for path in paths:
        if path.stat().st_size > 64 * 1024 * 1024:
            raise ValueError("XML export exceeds 64 MiB")
        data = path.read_bytes()
        documents.append(ET.fromstring(data))
        sources.append({"path": str(path.resolve()), "sha256": hashlib.sha256(data).hexdigest()})
    extra = {}
    if len(sys.argv) == 8:
        raw_path, clock_path = map(Path, sys.argv[6:8])
        if raw_path.stat().st_size > 1024 * 1024 * 1024 or clock_path.stat().st_size > 64 * 1024:
            raise ValueError("raw display or timebase export exceeds size limit")
        clock_data = clock_path.read_bytes()
        extra = {"raw_display": raw_path, "time_info": ET.fromstring(clock_data)}
        with raw_path.open("rb") as raw:
            digest = hashlib.file_digest(raw, "sha256").hexdigest()
        sources.extend([
            {"path": str(raw_path.resolve()), "sha256": digest},
            {"path": str(clock_path.resolve()), "sha256": hashlib.sha256(clock_data).hexdigest()},
        ])
    result = present_frames(int(sys.argv[1]), *documents, **extra)
    result["sources"] = sources
    with Path(sys.argv[5]).open("x") as output:
        json.dump(result, output, indent=2)
        output.write("\n")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, ET.ParseError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
