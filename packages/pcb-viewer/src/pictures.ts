import type { Scene, SceneLayer } from "@oshwhub/bridge";
import {
  PaintStyle,
  type SkFont,
  Skia,
  type SkPaint,
  type SkPicture,
  type SkTypeface,
  StrokeCap,
  StrokeJoin,
} from "@shopify/react-native-skia";
import { planLayers } from "./layers";

export type LayerPicture = { layer: SceneLayer; picture: SkPicture };

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
          const key = `${color}/${batch.fill}/${batch.width}`;
          let paint = paints.get(key);
          if (!paint) {
            paint = Skia.Paint();
            paint.setAntiAlias(true);
            paint.setColor(Skia.Color(color));
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
