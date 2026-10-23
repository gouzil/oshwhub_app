import type { PickResult } from "@oshwhub/bridge";
import { describe, expect, it } from "vitest";
import { describeSelection } from "./selection";

const selected = (properties: Record<string, string>, kind = "POLY"): PickResult => ({
  id: "opaque-id/e5",
  kind,
  layer: 1,
  net: "GND",
  properties,
  bounds: { minX: 0, minY: 0, maxX: 1, maxY: 1 },
});

describe("selection presentation", () => {
  it("separates readable component properties from opaque source references", () => {
    expect(
      describeSelection(
        selected({
          Designator: " C49 ",
          Value: "100 nF",
          FootprintName: "C0603",
          Footprint: "9a146d511b4d87b3",
          Device: "dd165e9494ea163f",
          componentId: "f0d8af1247985cb7",
          recordId: "e5",
          ticket: "29",
          "Channel ID": "$4I73",
          "Unique ID": "gge875",
        }),
      ),
    ).toEqual({
      title: "C49",
      kind: "轮廓",
      fields: [
        ["网络", "GND"],
        ["参数", "100 nF"],
        ["封装", "C0603"],
      ],
    });
  });

  it("identifies the selected pad without discarding zero or custom properties", () => {
    expect(
      describeSelection(
        selected({ Designator: "U1", PadNumber: "3", Value: "0", 备注: "测试点" }, "PAD"),
      ),
    ).toEqual({
      title: "U1 · 焊盘 3",
      kind: "焊盘",
      fields: [
        ["网络", "GND"],
        ["参数", "0"],
        ["备注", "测试点"],
      ],
    });
  });

  it("uses a neutral title for unknown primitives instead of exposing a raw ID", () => {
    expect(
      describeSelection({ ...selected({ recordId: "e5", ticket: "0" }, "FUTURE_KIND"), net: null }),
    ).toEqual({ title: "图元", kind: "图元", fields: [] });
  });

  it("treats prototype names in source attributes as ordinary text", () => {
    expect(
      describeSelection({ ...selected({ constructor: "自定义字段" }, "constructor"), net: null }),
    ).toEqual({ title: "图元", kind: "图元", fields: [["constructor", "自定义字段"]] });
  });
});
