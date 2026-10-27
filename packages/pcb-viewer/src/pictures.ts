import type { Scene, SceneLayer } from "@oshwhub/bridge";
import {
  PaintStyle,
  type SkFont,
  type SkImage,
  Skia,
  type SkPaint,
  type SkPicture,
  type SkTypeface,
  StrokeCap,
  StrokeJoin,
} from "@shopify/react-native-skia";
import { planLayers } from "./layers";
import { defaultNetColor, isCopperLayer } from "./nets";
import type { RasterPlan } from "./raster";

export type LayerPicture = { layer: SceneLayer; picture: SkPicture };
export type BoardRaster = RasterPlan & { image: SkImage; elapsedMs: number };

export type NetworkOverlayOptions = {
  highlighted: ReadonlySet<string>;
  visibleLayers: ReadonlySet<number>;
  colors: Readonly<Record<string, string>>;
};

export function rasterize(pictures: LayerPicture[], plan: RasterPlan): BoardRaster {
  const started = performance.now();
  // A CPU surface makes the snapshot portable between CanvasKit's WebGL contexts.
  const surface = Skia.Surface.Make(plan.pixelWidth, plan.pixelHeight);
  if (!surface) throw new Error("无法分配 PCB 绘制缓存。");
  try {
    const canvas = surface.getCanvas();
    canvas.clear(Skia.Color("#0b121a"));
    canvas.scale(plan.pixelWidth / plan.width, plan.pixelHeight / plan.height);
    canvas.translate(-plan.x, -plan.y);
    for (const { picture } of pictures) canvas.drawPicture(picture);
    surface.flush();
    const image = surface.makeImageSnapshot();
    const elapsedMs = performance.now() - started;
    console.info("pcb.raster.built", {
      layers: pictures.length,
      width: plan.pixelWidth,
      height: plan.pixelHeight,
      elapsedMs: Math.round(elapsedMs),
    });
    return { ...plan, image, elapsedMs };
  } finally {
    surface.dispose();
  }
}

export function buildPictures(scene: Scene, typeface?: SkTypeface) {
  if (scene.texts.length && !typeface) throw new Error("PCB 字体尚未加载。");
  const pictures: LayerPicture[] = [];
  const missing = new Set<string>();
  try {
    for (const { layer, batches, texts } of planLayers(scene)) {
      const recorder = Skia.PictureRecorder();
      const margin =
        Math.max(scene.bounds.maxX - scene.bounds.minX, scene.bounds.maxY - scene.bounds.minY, 1) *
        0.1;
      const canvas = recorder.beginRecording(
        Skia.XYWHRect(
          scene.bounds.minX - margin,
          scene.bounds.minY - margin,
          scene.bounds.maxX - scene.bounds.minX + margin * 2,
          scene.bounds.maxY - scene.bounds.minY + margin * 2,
        ),
      );
      const paints = new Map<string, SkPaint>();
      const fonts = new Map<number, SkFont>();
      const textPaint = Skia.Paint();
      try {
        for (const batch of batches) {
          const color = batch.color || layer.color;
          // Pro's copper areas are 30% transparent; pads and tracks stay opaque.
          const opacity =
            isCopperLayer(layer.id) && (batch.kind === "POURED" || batch.kind === "FILL") ? 0.7 : 1;
          const key = `${color}/${batch.fill}/${batch.width}/${opacity}`;
          let paint = paints.get(key);
          if (!paint) {
            paint = Skia.Paint();
            paint.setAntiAlias(true);
            paint.setColor(Skia.Color(color));
            paint.setAlphaf(opacity);
            paint.setStyle(batch.fill ? PaintStyle.Fill : PaintStyle.Stroke);
            paint.setStrokeWidth(batch.width);
            paint.setStrokeCap(StrokeCap.Round);
            paint.setStrokeJoin(StrokeJoin.Round);
            paints.set(key, paint);
          }
          const path = Skia.Path.MakeFromSVGString(batch.path);
          if (!path) throw new Error(`图层「${layer.name}」包含无法绘制的路径。`);
          try {
            canvas.drawPath(path, paint);
          } finally {
            path.dispose();
          }
        }
        textPaint.setAntiAlias(true);
        textPaint.setColor(Skia.Color(layer.color));
        for (const text of texts) {
          let font = fonts.get(text.size);
          if (!font) {
            font = Skia.Font(typeface, text.size);
            fonts.set(text.size, font);
          }
          const characters = Array.from(text.text);
          const glyphs = font.getGlyphIDs(text.text, characters.length);
          characters.forEach((character, i) => {
            if (glyphs[i] === 0) missing.add(character);
          });
          const measure = font.measureText(text.text);
          const x =
            text.align === "center"
              ? -measure.width / 2
              : text.align === "right"
                ? -measure.width
                : 0;
          canvas.save();
          canvas.translate(text.x, text.y);
          canvas.rotate(text.rotation, 0, 0);
          canvas.drawText(text.text, x, 0, textPaint, font);
          canvas.restore();
        }
        pictures.push({ layer, picture: recorder.finishRecordingAsPicture() });
      } finally {
        for (const paint of paints.values()) paint.dispose();
        for (const font of fonts.values()) font.dispose();
        textPaint.dispose();
        recorder.dispose();
      }
    }
    return {
      pictures,
      warnings: missing.size
        ? [
            `部分标注含当前字体不支持的字符：${[...missing].slice(0, 12).join("")}。这些文字可能显示为方框。`,
          ]
        : [],
    };
  } catch (error) {
    for (const { picture } of pictures) picture.dispose();
    throw error;
  }
}

function colorWithAlpha(color: string, alpha: string) {
  return /^#[\da-f]{6}$/i.test(color) ? `${color}${alpha}` : color;
}

export function buildNetworkOverlayPicture(
  scene: Scene,
  options: NetworkOverlayOptions,
): SkPicture | null {
  const span = Math.max(
    scene.bounds.maxX - scene.bounds.minX,
    scene.bounds.maxY - scene.bounds.minY,
    1,
  );
  const margin = span * 0.1;
  const recorder = Skia.PictureRecorder();
  const canvas = recorder.beginRecording(
    Skia.XYWHRect(
      scene.bounds.minX - margin,
      scene.bounds.minY - margin,
      scene.bounds.maxX - scene.bounds.minX + margin * 2,
      scene.bounds.maxY - scene.bounds.minY + margin * 2,
    ),
  );
  const paints = new Map<string, SkPaint>();
  let hasContent = false;
  try {
    for (const batch of scene.batches) {
      const name = batch.net?.trim();
      if (
        !name ||
        !options.highlighted.has(name) ||
        !options.visibleLayers.has(batch.layer) ||
        !isCopperLayer(batch.layer)
      )
        continue;
      const color = options.colors[name] || defaultNetColor(name);
      const key = `${color}/${batch.fill}/${batch.width}`;
      let paint = paints.get(key);
      if (!paint) {
        paint = Skia.Paint();
        paint.setAntiAlias(true);
        paint.setColor(Skia.Color(batch.fill ? colorWithAlpha(color, "88") : color));
        paint.setStyle(batch.fill ? PaintStyle.Fill : PaintStyle.Stroke);
        paint.setStrokeWidth(batch.width);
        paint.setStrokeCap(StrokeCap.Round);
        paint.setStrokeJoin(StrokeJoin.Round);
        paints.set(key, paint);
      }
      const path = Skia.Path.MakeFromSVGString(batch.path);
      if (!path) continue;
      try {
        canvas.drawPath(path, paint);
        hasContent = true;
      } finally {
        path.dispose();
      }
    }

    if (!hasContent) return null;
    return recorder.finishRecordingAsPicture();
  } finally {
    for (const paint of paints.values()) paint.dispose();
    recorder.dispose();
  }
}
