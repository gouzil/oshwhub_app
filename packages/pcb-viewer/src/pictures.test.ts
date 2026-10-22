import { readFileSync } from "node:fs";
import type { Scene } from "@oshwhub/bridge";
import { Skia } from "@shopify/react-native-skia";
import { describe, expect, it, vi } from "vitest";
import { buildPictures } from "./pictures";

// Run the shipped CanvasKit backend, replacing only the native module entry point.
vi.mock("@shopify/react-native-skia", async () => {
  const { createRequire } = await import("node:module");
  const require = createRequire(import.meta.url);
  const init = require("canvaskit-wasm/bin/full/canvaskit.js");
  const kit = await init({});
  const { JsiSkApi } = require("@shopify/react-native-skia/lib/commonjs/skia/web/JsiSkia.js");
  return {
    Skia: JsiSkApi(kit),
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

describe("real CanvasKit picture recording", () => {
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
  it("reports corrupt SVG instead of drawing a partial board", () => {
    expect(() =>
      buildPictures({ ...scene, batches: [{ ...scene.batches[0], path: "broken path" }] }),
    ).toThrow("无法绘制");
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
});
