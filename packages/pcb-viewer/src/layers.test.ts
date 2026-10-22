import type { Scene } from "@oshwhub/bridge";
import { describe, expect, it } from "vitest";
import { planLayers } from "./layers";

const scene: Scene = {
  id: "offline-test",
  name: "自制离线夹具",
  revision: "1",
  bounds: { minX: 0, minY: 0, maxX: 100, maxY: 60 },
  layers: [
    { id: 1, name: "顶层", color: "#e86c68", visible: true, order: 2 },
    { id: 2, name: "底层", color: "#61a6e8", visible: false, order: 1 },
  ],
  batches: [
    {
      layer: 1,
      path: "M0 0H100V60H0Z M10 10V20H20V10Z",
      fill: true,
      width: 0,
      bounds: { minX: 0, minY: 0, maxX: 100, maxY: 60 },
    },
    {
      layer: 2,
      path: "M0 0L20 30",
      fill: false,
      width: 2,
      bounds: { minX: 0, minY: 0, maxX: 20, maxY: 30 },
    },
  ],
  texts: [{ layer: 1, x: 20, y: 30, text: "U1", size: 3, rotation: 90, align: "center" }],
  objectCount: 3,
  warnings: [],
  metrics: { loadMs: 0, parseMs: 0, sceneMs: 0, recordCount: 3, historyCount: 1, downloadBytes: 0 },
};

describe("offline scene draw plan", () => {
  it("preserves layer paint order, visibility, paths, and transformed labels", () => {
    const layers = planLayers(scene);
    expect(layers.map(({ layer }) => [layer.id, layer.visible])).toEqual([
      [2, false],
      [1, true],
    ]);
    expect(layers[1].batches[0].path).toBe("M0 0H100V60H0Z M10 10V20H20V10Z");
    expect(layers[1].texts[0]).toEqual({
      layer: 1,
      x: 20,
      y: 30,
      text: "U1",
      size: 3,
      rotation: 90,
      align: "center",
    });
  });
  it("reports an undeclared layer rather than silently dropping geometry", () => {
    expect(() => planLayers({ ...scene, layers: [scene.layers[0]] })).toThrow("2");
  });
});
