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


def present_frames(pid, updates, lifetimes, swaps):
    if not isinstance(pid, int) or pid <= 0:
        raise ValueError("target PID must be positive")
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
    frames = []
    for update in target:
        k = key(update)
        if k not in swap_index or k not in lifetime_index:
            raise ValueError("target update is missing a swap or lifetime")
        swap, lifetime = swap_index[k], lifetime_index[k]
        present = integer(swap["timestamp"])
        start = integer(update["start"])
        if present <= 0 or start > present:
            raise ValueError("invalid presentation timestamp")
        frames.append({
            "swapId": k[0], "surfaceId": k[1], "presentNs": present,
            "updateStartNs": start,
            "endMinusSwapNs": integer(lifetime["start"]) + integer(lifetime["duration"]) - present,
        })
    frames.sort(key=lambda frame: frame["presentNs"])
    intervals = [(b["presentNs"] - a["presentNs"]) / 1_000_000 for a, b in zip(frames, frames[1:])]
    if len(frames) < 2 or any(ms <= 0 for ms in intervals):
        raise ValueError("requires distinct, increasing target presentations")
    return {
        "kind": "macos-present-diagnostic", "pid": pid,
        "nestedUpdateRows": nested_update_rows,
        "frames": frames, "frameMs": intervals,
        "notAcceptance": "Process-attributed compositor swaps; input coverage, layer completeness, GPU memory and other acceptance metrics require separate evidence.",
    }


def main():
    if len(sys.argv) != 6:
        raise ValueError("Usage: python3 scripts/nsh-macos-frames.py PID updates.xml lifetimes.xml swaps.xml output.json")
    paths = [Path(path) for path in sys.argv[2:5]]
    documents, sources = [], []
    for path in paths:
        if path.stat().st_size > 64 * 1024 * 1024:
            raise ValueError("XML export exceeds 64 MiB")
        data = path.read_bytes()
        documents.append(ET.fromstring(data))
        sources.append({"path": str(path.resolve()), "sha256": hashlib.sha256(data).hexdigest()})
    result = present_frames(int(sys.argv[1]), *documents)
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
