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
  it("keeps overlapping copper solid and reverse-winding holes empty", async () => {
    const { pictures } = await buildPictures(scene);
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
  it("records embedded images at their transformed sheet position", async () => {
    const source = Skia.Surface.Make(10, 10);
    if (!source) throw new Error("CanvasKit software surface unavailable");
    source.getCanvas().clear(Skia.Color("#0000ff"));
    const bitmap = source.makeImageSnapshot();
    const content = `data:image/png;base64,${bitmap.encodeToBase64()}`;
    const { pictures } = await buildPictures({
      ...scene,
      batches: [],
      images: [{ layer: 1, content, width: 10, height: 10, transform: [1, 0, 0, 1, 5, 7] }],
    });
    bitmap.dispose();
    source.dispose();
    const surface = Skia.Surface.Make(40, 30);
    if (!surface) throw new Error("CanvasKit software surface unavailable");
    const canvas = surface.getCanvas();
    canvas.clear(Skia.Color("#000000"));
    for (const { picture } of pictures) canvas.drawPicture(picture);
    const image = surface.makeImageSnapshot();
    const pixels = image.readPixels();
    if (!pixels) throw new Error("CanvasKit pixels unavailable");
    expect(Array.from(pixels.slice((10 * 40 + 8) * 4, (10 * 40 + 8) * 4 + 4))).toEqual([
      0, 0, 255, 255,
    ]);
    expect(Array.from(pixels.slice((3 * 40 + 3) * 4, (3 * 40 + 3) * 4 + 4))).toEqual([
      0, 0, 0, 255,
    ]);
    for (const { picture } of pictures) picture.dispose();
    image.dispose();
    surface.dispose();
  });
  it("reports corrupt SVG instead of drawing a partial board", async () => {
    await expect(
      buildPictures({ ...scene, batches: [{ ...scene.batches[0], path: "broken path" }] }),
    ).rejects.toThrow("无法绘制");
  });
  it("keeps label spacing proportional when the camera enlarges small text", async () => {
    const data = Skia.Data.fromBytes(
      readFileSync(new URL("./assets/NotoSansSC-Regular.ttf", import.meta.url)),
    );
    const typeface = Skia.Typeface.MakeFreeTypeFaceFromData(data);
    if (!typeface) throw new Error("Bundled font unavailable");
    const render = async (scale: number) => {
      const { pictures } = await buildPictures(
        {
          ...scene,
          bounds: { minX: 0, minY: 0, maxX: 400, maxY: 80 },
          batches: [],
          texts: [
            {
              layer: 1,
              x: 8 / scale,
              y: 60 / scale,
              text: "USB1 USB_N SBU2",
              size: 24 / scale,
              rotation: 0,
              align: "left",
            },
          ],
        },
        typeface,
      );
      const surface = Skia.Surface.Make(400, 80);
      if (!surface) throw new Error("CanvasKit software surface unavailable");
      const canvas = surface.getCanvas();
      canvas.clear(Skia.Color("#000000"));
      canvas.scale(scale, scale);
      for (const { picture } of pictures) canvas.drawPicture(picture);
      const image = surface.makeImageSnapshot();
      const pixels = image.readPixels();
      if (!pixels) throw new Error("CanvasKit pixels unavailable");
      const ink = new Set(
        Array.from(pixels).flatMap((value, i) => (i % 4 === 0 && value > 127 ? [i / 4] : [])),
      );
      for (const { picture } of pictures) picture.dispose();
      image.dispose();
      surface.dispose();
      return ink;
    };
    try {
      const enlarged = await render(4);
      const nativeSize = await render(1);
      const different =
        [...enlarged].filter((p) => !nativeSize.has(p)).length +
        [...nativeSize].filter((p) => !enlarged.has(p)).length;
      expect(different / nativeSize.size).toBeLessThan(0.02);
    } finally {
      typeface.dispose();
      data.dispose();
    }
  });
  it.each([
    "top",
    "middle",
    "bottom",
  ] as const)("keeps %s-aligned label baselines independent of glyph contents", async (verticalAlign) => {
    const data = Skia.Data.fromBytes(
      readFileSync(new URL("./assets/NotoSansSC-Regular.ttf", import.meta.url)),
    );
    const typeface = Skia.Typeface.MakeFreeTypeFaceFromData(data);
    if (!typeface) throw new Error("Bundled font unavailable");
    const renderPrefix = async (text: string, rotation: number) => {
      const { pictures } = await buildPictures(
        {
          ...scene,
          bounds: { minX: 0, minY: 0, maxX: 100, maxY: 100 },
          batches: [],
          texts: [
            { layer: 1, x: 32, y: 32, text, size: 6, rotation, align: "left", verticalAlign },
          ],
        },
        typeface,
      );
      const surface = Skia.Surface.Make(400, 400);
      if (!surface) throw new Error("CanvasKit software surface unavailable");
      const canvas = surface.getCanvas();
      canvas.clear(Skia.Color("#000000"));
      canvas.scale(4, 4);
      for (const { picture } of pictures) canvas.drawPicture(picture);
      const image = surface.makeImageSnapshot();
      try {
        const pixels = image.readPixels();
        if (!pixels) throw new Error("CanvasKit pixels unavailable");
        const prefix: number[] = [];
        // Compare only the shared first glyph, excluding the varying suffix.
        for (let along = 0; along < 10; along++) {
          for (let across = -64; across < 64; across++) {
            const x = rotation === 0 ? 128 + along : 128 - across;
            const y = rotation === 0 ? 128 + across : 128 + along;
            prefix.push(pixels[(y * 400 + x) * 4]);
          }
        }
        return prefix;
      } finally {
        for (const { picture } of pictures) picture.dispose();
        image.dispose();
        surface.dispose();
      }
    };
    try {
      for (const rotation of [0, 90]) {
        const plain = await renderPrefix("CLK", rotation);
        expect(plain.some((pixel) => pixel > 0)).toBe(true);
        for (const text of ["CLK/DMIC_SCL", "CLKgj", "CLK中文"]) {
          expect(await renderPrefix(text, rotation)).toEqual(plain);
        }
      }
    } finally {
      typeface.dispose();
      data.dispose();
    }
  });
  it("records visible Latin and Chinese labels with the bundled typeface", async () => {
    const data = Skia.Data.fromBytes(
      readFileSync(new URL("./assets/NotoSansSC-Regular.ttf", import.meta.url)),
    );
    const typeface = Skia.Typeface.MakeFreeTypeFaceFromData(data);
    if (!typeface) throw new Error("Bundled font unavailable");
    const { pictures, warnings } = await buildPictures(
      {
        ...scene,
        batches: [],
        texts: [{ layer: 1, x: 2, y: 20, text: "U1中文𝐼𝐵⁄", size: 10, rotation: 0, align: "left" }],
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
