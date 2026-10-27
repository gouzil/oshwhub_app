import { readFileSync, writeFileSync } from "node:fs";
import type { Scene } from "@oshwhub/bridge";
import { Skia } from "@shopify/react-native-skia";
import { describe, expect, it, vi } from "vitest";
import { labelScene } from "../../../tests/fixtures/pcb-labels";
import { buildNetLabelPicture, type NetLabelOptions } from "./labels";
import { buildNetworkOverlayPicture, buildPictures } from "./pictures";

// Run the shipped CanvasKit backend, replacing only the native module entry point.
vi.mock("@shopify/react-native-skia", async () => {
  const { createRequire } = await import("node:module");
  const require = createRequire(import.meta.url);
  const init = require("canvaskit-wasm/bin/full/canvaskit.js");
  const kit = await init({});
  const { JsiSkApi } = require("@shopify/react-native-skia/lib/commonjs/skia/web/JsiSkia.js");
  return {
    Skia: JsiSkApi(kit),
    ...require("@shopify/react-native-skia/lib/commonjs/skia/types/Canvas"),
    ...require("@shopify/react-native-skia/lib/commonjs/skia/types/Paint"),
  };
});

const scene: Scene = {
  id: "offline-pixels",
  name: "离线绘制夹具",
  revision: "1",
  bounds: { minX: 0, minY: 0, maxX: 40, maxY: 30 },
  layers: [{ id: 1, name: "铜层", color: "#ff0000", visible: true, order: 1 }],
  batches: [
    {
      layer: 1,
      path: "M2 2H22V22H2Z M8 8V16H16V8Z M18 2H30V22H18Z",
      fill: true,
      width: 0,
      bounds: { minX: 2, minY: 2, maxX: 30, maxY: 22 },
    },
  ],
  texts: [],
  objectCount: 3,
  warnings: [],
  metrics: { loadMs: 0, parseMs: 0, sceneMs: 0, recordCount: 3, historyCount: 1, downloadBytes: 0 },
};

function labelPixels(source: Scene, overrides: Partial<NetLabelOptions> = {}, artifact?: string) {
  const data = Skia.Data.fromBytes(
    readFileSync(new URL("./assets/NotoSansSC-Regular.ttf", import.meta.url)),
  );
  const typeface = Skia.Typeface.MakeFreeTypeFaceFromData(data);
  if (!typeface) throw new Error("Bundled font unavailable");
  const options = {
    visibleLayers: new Set([1, 3]),
    showLabels: true,
    showPadLabels: true,
    showTrackLabels: true,
    showViaLabels: true,
    viewport: { scale: 8, x: -80, y: 20 },
    width: 560,
    height: 480,
    ...overrides,
  };
  const picture = buildNetLabelPicture(source, typeface, options);
  if (!picture) {
    typeface.dispose();
    data.dispose();
    return null;
  }
  const surface = Skia.Surface.Make(options.width, options.height);
  if (!surface) throw new Error("CanvasKit software surface unavailable");
  const canvas = surface.getCanvas();
  canvas.clear(Skia.Color("transparent"));
  canvas.translate(options.viewport.x, options.viewport.y);
  canvas.scale(options.viewport.scale, options.viewport.scale);
  canvas.drawPicture(picture);
  const image = surface.makeImageSnapshot();
  const pixels = image.readPixels();
  if (!pixels) throw new Error("CanvasKit pixels unavailable");
  if (artifact) {
    const { pictures } = buildPictures(source, typeface);
    canvas.clear(Skia.Color("#0b121a"));
    for (const layer of pictures) canvas.drawPicture(layer.picture);
    canvas.drawPicture(picture);
    const preview = surface.makeImageSnapshot();
    writeFileSync(artifact, preview.encodeToBytes());
    preview.dispose();
    for (const layer of pictures) layer.picture.dispose();
  }
  picture.dispose();
  image.dispose();
  surface.dispose();
  typeface.dispose();
  data.dispose();
  return pixels;
}

describe("real CanvasKit picture recording", () => {
  it("keeps each pad's number and network inside that pad at high zoom", () => {
    const pixels = labelPixels(labelScene, {}, process.env.PCB_LABEL_ARTIFACT);
    if (!pixels) throw new Error("Expected readable pad labels");
    const regions = [
      { left: 64, right: 160, top: 140, bottom: 220 },
      { left: 256, right: 352, top: 140, bottom: 220 },
      { left: 64, right: 160, top: 332, bottom: 412 },
      { left: 256, right: 352, top: 332, bottom: 412 },
    ];
    const counts = [0, 0, 0, 0];
    let outside = 0;
    for (let y = 0; y < 480; y++) {
      for (let x = 0; x < 560; x++) {
        const i = (y * 560 + x) * 4;
        if (pixels[i + 3] < 20) continue;
        const region = regions.findIndex(
          (r) => x >= r.left && x < r.right && y >= r.top && y < r.bottom,
        );
        if (region < 0) outside++;
        else counts[region]++;
      }
    }
    expect(outside, "Labels must not cover the gaps or silkscreen").toBe(0);
    for (const count of counts)
      expect(count, "Every pad must have its own label").toBeGreaterThan(20);
  });

  it.each([8, 80])("caps label glyphs at screen size and uses white text at %sx zoom", (scale) => {
    const pad = labelScene.netLabels?.[0];
    if (!pad) throw new Error("Missing test pad");
    const pixels = labelPixels(
      { ...labelScene, netLabels: [pad] },
      {
        viewport: { scale, x: 280 - pad.x * scale, y: 240 - pad.y * scale },
      },
    );
    if (!pixels) throw new Error("Expected readable pad labels");
    const runs: number[] = [];
    let run = 0;
    for (let y = 0; y < 480; y++) {
      let ink = false;
      for (let x = 0; x < 560; x++) {
        const i = (y * 560 + x) * 4;
        if (pixels[i + 3] < 20) continue;
        ink = true;
        expect(pixels[i]).toBe(255);
        expect(pixels[i + 1]).toBe(255);
        expect(pixels[i + 2]).toBe(255);
      }
      if (ink) run++;
      else if (run) {
        runs.push(run);
        run = 0;
      }
    }
    if (run) runs.push(run);
    expect(runs).toHaveLength(2);
    for (const height of runs) expect(height).toBeLessThanOrEqual(13);
  });

  it("hides labels below readable size, outside the viewport, and on disabled layers or categories", () => {
    for (const options of [
      { showLabels: false },
      { showPadLabels: false },
      { visibleLayers: new Set<number>() },
      { viewport: { scale: 0.1, x: 0, y: 0 } },
      { viewport: { scale: 8, x: -10000, y: 0 } },
    ])
      expect(labelPixels(labelScene, options)).toBeNull();
  });

  it("suppresses duplicate labels on coincident pads", () => {
    const pad = labelScene.netLabels?.[0];
    if (!pad) throw new Error("Missing test pad");
    const single = labelPixels({ ...labelScene, netLabels: [pad] });
    const duplicate = labelPixels({ ...labelScene, netLabels: [pad, { ...pad, net: "OTHER" }] });
    expect(single).not.toBeNull();
    expect(duplicate).toEqual(single);
  });

  it.each([
    [1, 12],
    [1, 47],
  ])("keeps track text off drills with layers %s/%s and via labels off", (...layers) => {
    const pad = labelScene.netLabels?.[0];
    if (!pad) throw new Error("Missing test pad");
    const pixels = labelPixels(
      {
        ...labelScene,
        netLabels: [
          { ...pad, kind: "VIA", layer: 12, hole: true, width: 8, height: 8, padNumber: "" },
          { ...pad, kind: "LINE", net: "GND", width: 30, height: 3, padNumber: "" },
        ],
      },
      { visibleLayers: new Set(layers), showViaLabels: false },
    );
    expect(pixels).toBeNull();
  });

  it("fits rotated pads along their long axis and keeps via labels beside the hole", () => {
    const pad = labelScene.netLabels?.[0];
    if (!pad) throw new Error("Missing test pad");
    for (const label of [
      { ...pad, rotation: 45 },
      { ...pad, kind: "VIA", layer: 12, hole: true, padNumber: "", width: 4, height: 4 },
    ]) {
      const pixels = labelPixels(
        { ...labelScene, netLabels: [label] },
        { visibleLayers: new Set([1, 12]) },
      );
      if (!pixels) throw new Error("Expected readable label");
      let ink = 0;
      for (let y = 0; y < 480; y++)
        for (let x = 0; x < 560; x++) {
          if (pixels[(y * 560 + x) * 4 + 3] < 20) continue;
          ink++;
          if (label.kind === "VIA") expect(x).toBeGreaterThan(128);
          else {
            const localX = (x - 112 + (y - 180)) / Math.SQRT2;
            const localY = (-(x - 112) + (y - 180)) / Math.SQRT2;
            expect(Math.abs(localX)).toBeLessThan(48);
            expect(Math.abs(localY)).toBeLessThan(40);
          }
        }
      expect(ink).toBeGreaterThan(20);
      expect(
        labelPixels(
          { ...labelScene, netLabels: [label] },
          {
            visibleLayers: new Set([1, 12]),
            showPadLabels: false,
            showViaLabels: false,
          },
        ),
      ).toBeNull();
    }
  });
  it("keeps overlapping copper solid and reverse-winding holes empty", () => {
    const { pictures } = buildPictures(scene);
    const surface = Skia.Surface.Make(40, 30);
    if (!surface) throw new Error("CanvasKit software surface unavailable");
    const canvas = surface.getCanvas();
    canvas.clear(Skia.Color("#000000"));
    for (const { picture } of pictures) canvas.drawPicture(picture);
    const image = surface.makeImageSnapshot();
    const pixels = image.readPixels();
    if (!pixels) throw new Error("CanvasKit pixels unavailable");
    const pixel = (x: number, y: number) =>
      Array.from(pixels.slice((y * 40 + x) * 4, (y * 40 + x) * 4 + 4));
    expect(pixel(20, 10)).toEqual([255, 0, 0, 255]);
    expect(pixel(12, 12)).toEqual([0, 0, 0, 255]);
    for (const { picture } of pictures) picture.dispose();
    image.dispose();
    surface.dispose();
  });
  it.each([
    { kind: "POURED", layer: 1, planeRed: 179 },
    { kind: "FILL", layer: 1, planeRed: 179 },
    { kind: "FILL", layer: 15, planeRed: 179 },
    { kind: "FILL", layer: 3, planeRed: 255 },
  ])("renders $kind on layer $layer with opaque pads and intact clearance holes", ({
    kind,
    layer,
    planeRed,
  }) => {
    const { pictures } = buildPictures({
      ...scene,
      layers: [{ ...scene.layers[0], id: layer }],
      batches: [
        {
          ...scene.batches[0],
          layer,
          kind,
          net: "GND",
          path: "M2 2H34V26H2Z M20 10V18H28V10Z",
        },
        { ...scene.batches[0], layer, kind: "PAD", net: "GND", path: "M4 6H8V10H4Z" },
        {
          ...scene.batches[0],
          layer,
          kind: "LINE",
          net: "GND",
          fill: false,
          width: 2,
          path: "M8 8H18",
        },
        { ...scene.batches[0], layer, kind: "PAD", net: "VCC", path: "M22 12H26V16H22Z" },
      ],
    });
    const surface = Skia.Surface.Make(40, 30);
    if (!surface) throw new Error("CanvasKit software surface unavailable");
    const canvas = surface.getCanvas();
    canvas.clear(Skia.Color("#000000"));
    for (const { picture } of pictures) canvas.drawPicture(picture);
    const image = surface.makeImageSnapshot();
    const pixels = image.readPixels();
    if (!pixels) throw new Error("CanvasKit pixels unavailable");
    const pixel = (x: number, y: number) =>
      Array.from(pixels.slice((y * 40 + x) * 4, (y * 40 + x) * 4 + 4));
    expect(pixel(12, 20)).toEqual([planeRed, 0, 0, 255]);
    expect(pixel(6, 8)).toEqual([255, 0, 0, 255]);
    expect(pixel(12, 8)).toEqual([255, 0, 0, 255]);
    expect(pixel(24, 14)).toEqual([255, 0, 0, 255]);
    expect(pixel(21, 14)).toEqual([0, 0, 0, 255]);
    expect(pixel(24, 11)).toEqual([0, 0, 0, 255]);
    for (const { picture } of pictures) picture.dispose();
    image.dispose();
    surface.dispose();
  });
  it("reports corrupt SVG instead of drawing a partial board", () => {
    expect(() =>
      buildPictures({ ...scene, batches: [{ ...scene.batches[0], path: "broken path" }] }),
    ).toThrow("无法绘制");
  });
  it("shows the pad boundary where a same-colored track meets it", () => {
    const { pictures } = buildPictures({
      ...scene,
      batches: [
        { ...scene.batches[0], kind: "LINE", fill: false, width: 4, path: "M2 13H14" },
        { ...scene.batches[0], kind: "PAD", path: "M12 8H22V18H12Z M17 10V16H20V10Z" },
      ],
    });
    const surface = Skia.Surface.Make(40, 30);
    if (!surface) throw new Error("CanvasKit software surface unavailable");
    const canvas = surface.getCanvas();
    canvas.clear(Skia.Color("#000000"));
    for (const { picture } of pictures) canvas.drawPicture(picture);
    const image = surface.makeImageSnapshot();
    const pixels = image.readPixels();
    if (!pixels) throw new Error("CanvasKit pixels unavailable");
    const pixel = (x: number, y: number) =>
      Array.from(pixels.slice((y * 40 + x) * 4, (y * 40 + x) * 4 + 4));
    expect(pixel(12, 13)[1], "The copper connection must not hide the pad edge").toBeGreaterThan(
      20,
    );
    expect(pixel(14, 13)).toEqual([255, 0, 0, 255]);
    expect(pixel(6, 13)).toEqual([255, 0, 0, 255]);
    expect(pixel(18, 13)).toEqual([0, 0, 0, 255]);
    expect(pixel(23, 20)).toEqual([0, 0, 0, 255]);
    for (const { picture } of pictures) picture.dispose();
    image.dispose();
    surface.dispose();
  });
  it("records visible Latin and Chinese labels with the bundled typeface", () => {
    const data = Skia.Data.fromBytes(
      readFileSync(new URL("./assets/NotoSansSC-Regular.ttf", import.meta.url)),
    );
    const typeface = Skia.Typeface.MakeFreeTypeFaceFromData(data);
    if (!typeface) throw new Error("Bundled font unavailable");
    const { pictures, warnings } = buildPictures(
      {
        ...scene,
        batches: [],
        texts: [{ layer: 1, x: 2, y: 20, text: "U1中文", size: 10, rotation: 0, align: "left" }],
      },
      typeface,
    );
    const surface = Skia.Surface.Make(40, 30);
    if (!surface) throw new Error("CanvasKit software surface unavailable");
    const canvas = surface.getCanvas();
    canvas.clear(Skia.Color("#000000"));
    for (const { picture } of pictures) canvas.drawPicture(picture);
    const image = surface.makeImageSnapshot();
    const pixels = image.readPixels();
    if (!pixels) throw new Error("CanvasKit pixels unavailable");
    expect(
      Array.from(pixels).filter((value, index) => index % 4 !== 3 && value > 0).length,
    ).toBeGreaterThan(10);
    expect(warnings).toEqual([]);
    for (const { picture } of pictures) picture.dispose();
    image.dispose();
    surface.dispose();
    typeface.dispose();
    data.dispose();
  });

  it("draws a selected network on a separate overlay", () => {
    const picture = buildNetworkOverlayPicture(
      {
        ...scene,
        batches: [
          {
            ...scene.batches[0],
            path: "M4 4L20 4",
            fill: false,
            width: 2,
            net: "GND",
            kind: "LINE",
          },
          { ...scene.batches[0], layer: 47, net: "GND", kind: "VIA", path: "M8 8H16V16H8Z" },
        ],
      },
      {
        highlighted: new Set(["GND"]),
        visibleLayers: new Set([1, 47]),
        colors: { GND: "#00ff00" },
      },
    );
    expect(picture).not.toBeNull();
    const surface = Skia.Surface.Make(40, 30);
    if (!surface || !picture) throw new Error("CanvasKit software surface unavailable");
    const canvas = surface.getCanvas();
    canvas.clear(Skia.Color("#000000"));
    canvas.drawPicture(picture);
    const image = surface.makeImageSnapshot();
    const pixels = image.readPixels();
    if (!pixels) throw new Error("CanvasKit pixels unavailable");
    const pixel = (x: number, y: number) =>
      Array.from(pixels.slice((y * 40 + x) * 4, (y * 40 + x) * 4 + 4));
    expect(pixel(12, 4)[1]).toBeGreaterThan(180);
    expect(pixel(12, 2)).toEqual([0, 0, 0, 255]);
    expect(pixel(12, 12)).toEqual([0, 0, 0, 255]);
    picture.dispose();
    image.dispose();
    surface.dispose();
  });
});
