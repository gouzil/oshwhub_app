import type { Scene } from "@oshwhub/bridge";
import { describe, expect, it } from "vitest";
import { collectNets, DEFAULT_NET_COLORS, defaultNetColor, netKind } from "./nets";

const scene: Scene = {
  id: "nets",
  name: "网络夹具",
  revision: "1",
  bounds: { minX: 0, minY: 0, maxX: 100, maxY: 100 },
  layers: [{ id: 1, name: "顶层", color: "#f00", visible: true, order: 1 }],
  batches: [
    {
      layer: 1,
      path: "M0 0L10 0",
      fill: false,
      width: 1,
      bounds: { minX: 0, minY: 0, maxX: 10, maxY: 0 },
      net: "GND",
      kind: "LINE",
    },
    {
      layer: 1,
      path: "M20 20L30 20",
      fill: false,
      width: 1,
      bounds: { minX: 20, minY: 20, maxX: 30, maxY: 20 },
      net: "GND",
      kind: "PAD",
    },
    {
      layer: 1,
      path: "M60 60L70 60",
      fill: false,
      width: 1,
      bounds: { minX: 60, minY: 60, maxX: 70, maxY: 60 },
      net: "3V3",
      kind: "VIA",
    },
    {
      layer: 1,
      path: "M80 80L90 80",
      fill: false,
      width: 1,
      bounds: { minX: 80, minY: 80, maxX: 90, maxY: 80 },
    },
  ],
  texts: [],
  objectCount: 4,
  warnings: [],
  metrics: { loadMs: 0, parseMs: 0, sceneMs: 0, recordCount: 4, historyCount: 1, downloadBytes: 0 },
};

describe("PCB network summaries", () => {
  it("deduplicates networks and unions copper bounds", () => {
    const nets = collectNets(scene);
    expect(nets.map((net) => net.name)).toEqual(["3V3", "GND"]);
    expect(nets[1]).toMatchObject({
      primitiveCount: 2,
      bounds: { minX: 0, minY: 0, maxX: 30, maxY: 20 },
    });
  });

  it("classifies professional-style pad, track, and via labels", () => {
    expect(netKind("PAD")).toBe("pad");
    expect(netKind("LINE")).toBe("track");
    expect(netKind("ARC")).toBe("track");
    expect(netKind("VIA")).toBe("via");
    expect(netKind("POUR")).toBe("other");
  });

  it("assigns stable colors from the built-in palette", () => {
    expect(DEFAULT_NET_COLORS).toContain(defaultNetColor("GND"));
    expect(defaultNetColor("GND")).toBe(defaultNetColor("GND"));
  });

  it("ignores network metadata on drill and mask drawings", () => {
    const nets = collectNets({
      batches: [
        { ...scene.batches[0], layer: 47, net: "DRILL" },
        { ...scene.batches[0], layer: 5, net: "MASK" },
      ],
    });
    expect(nets).toEqual([]);
  });
});
