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
    ).toMatchObject({
      title: "C49",
      kind: "元件",
      fields: [
        ["网络", "GND"],
        ["值", "100 nF"],
        ["封装", "C0603"],
      ],
    });
  });

  it("identifies the selected pad without discarding zero or custom properties", () => {
    expect(
      describeSelection(
        selected({ Designator: "U1", PadNumber: "3", Value: "0", 备注: "测试点" }, "PAD"),
      ),
    ).toMatchObject({
      title: "U1 · 焊盘 3",
      kind: "焊盘",
      fields: [
        ["网络", "GND"],
        ["值", "0"],
        ["备注", "测试点"],
      ],
    });
  });

  it("uses a neutral title for unknown primitives instead of exposing a raw ID", () => {
    expect(
      describeSelection({ ...selected({ recordId: "e5", ticket: "0" }, "FUTURE_KIND"), net: null }),
    ).toMatchObject({ title: "图元", kind: "图元", fields: [] });
  });

  it("treats prototype names in source attributes as ordinary text", () => {
    expect(
      describeSelection({ ...selected({ constructor: "自定义字段" }, "constructor"), net: null }),
    ).toMatchObject({ title: "图元", kind: "图元", fields: [["constructor", "自定义字段"]] });
  });

  it("presents resolved schematic values and translates technical details without duplicates", () => {
    const result = describeSelection({
      ...selected(
        {
          Designator: "R14",
          Name: "100kΩ",
          Value: "100kΩ",
          DeviceName: "RC0402FR-07100KL",
          FootprintName: "R0402",
          Manufacturer: "YAGEO(国巨)",
          "Supplier Part": "C60491",
          "Add into BOM": "yes",
          Description: "厚膜电阻",
          Datasheet: "https://example.com/r14.pdf",
          Symbol: "symbol-id",
          Device: "device-id",
          componentId: "e16953",
          recordId: "e11",
          ticket: "null",
          "Channel ID": "channel-1",
          "Group ID": "",
          "Reuse Block": "null",
          "Unique ID": "gge60_1",
        },
        "RECT",
      ),
      net: null,
    });
    expect(result).toEqual({
      title: "R14",
      kind: "元件",
      fields: [
        ["值", "100kΩ"],
        ["器件", "RC0402FR-07100KL"],
        ["封装", "R0402"],
        ["制造商", "YAGEO(国巨)"],
        ["供应商编号", "C60491"],
      ],
      extraFields: [
        ["加入 BOM", "是"],
        ["描述", "厚膜电阻"],
        ["数据手册", "https://example.com/r14.pdf"],
      ],
      technicalFields: [
        ["对象 ID", "opaque-id/e5"],
        ["原始类型", "RECT"],
        ["符号 ID", "symbol-id"],
        ["器件 ID", "device-id"],
        ["元件 ID", "e16953"],
        ["记录 ID", "e11"],
        ["通道 ID", "channel-1"],
        ["唯一 ID", "gge60_1"],
      ],
    });
  });

  it("omits null attributes while keeping zero and custom English attribute names", () => {
    expect(
      describeSelection(selected({ Value: "null", Name: " ", Count: "0", Custom: "keep" })).fields,
    ).toEqual([
      ["网络", "GND"],
      ["Count", "0"],
      ["Custom", "keep"],
    ]);
  });
});
