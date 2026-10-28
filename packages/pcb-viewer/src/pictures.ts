import type { Scene, SceneLayer } from "@oshwhub/bridge";
import {
  PaintStyle,
  type SkFont,
  type SkImage,
  Skia,
  type SkPaint,
  type SkPicture,
  type SkSVG,
  type SkTypeface,
  StrokeCap,
  StrokeJoin,
} from "@shopify/react-native-skia";
import { decodeSvg } from "./embeddedImage";
import { planLayers } from "./layers";
import type { RasterPlan } from "./raster";

export type LayerPicture = { layer: SceneLayer; picture: SkPicture };
export type BoardRaster = RasterPlan & { image: SkImage; elapsedMs: number };

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

export async function buildPictures(scene: Scene, typeface?: SkTypeface) {
  if (scene.texts.length && !typeface) throw new Error("PCB 字体尚未加载。");
  const pictures: LayerPicture[] = [];
  const missing = new Set<string>();
  try {
    for (const { layer, batches, texts, images } of planLayers(scene)) {
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
        for (const image of images) {
          const data = Skia.Data.fromBase64(image.content.slice(image.content.indexOf(",") + 1));
          let decoded: SkImage | SkSVG | null = null;
          try {
            decoded = image.content.startsWith("data:image/svg+xml;")
              ? await decodeSvg(image.content)
              : Skia.Image.MakeImageFromEncoded(data);
            if (!decoded) throw new Error("原理图嵌入图片无法解码。");
            if (decoded.__typename__ === "Image" && decoded.width() * decoded.height() > 16_777_216)
              throw new Error("原理图图片尺寸超过限制。");
            const [a, b, c, d, x, y] = image.transform;
            canvas.save();
            canvas.concat([a, c, x, b, d, y, 0, 0, 1]);
            if (decoded.__typename__ === "SVG") {
              canvas.scale(image.width / decoded.width(), image.height / decoded.height());
              canvas.drawSvg(decoded);
            } else {
              canvas.drawImageRect(
                decoded,
                Skia.XYWHRect(0, 0, decoded.width(), decoded.height()),
                Skia.XYWHRect(0, 0, image.width, image.height),
                textPaint,
              );
            }
            canvas.restore();
          } finally {
            decoded?.dispose();
            data.dispose();
          }
        }
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
        for (const text of texts) {
          let font = fonts.get(text.size);
          if (!font) {
            font = Skia.Font(typeface, text.size);
            fonts.set(text.size, font);
          }
          font.setEmbolden(text.bold ?? false);
          textPaint.setColor(Skia.Color(text.color || layer.color));
          const characters = Array.from(text.text);
          const glyphs = font.getGlyphIDs(text.text, characters.length);
          const renderedText = characters
            .map((character, i) => {
              if (glyphs[i] !== 0) return character;
              // shortcut: use compatible math glyphs; add a fallback typeface for exact styling.
              const plain = character === "⁄" ? "/" : character.normalize("NFKC");
              if (/^[A-Za-z0-9/]$/.test(plain) && font.getGlyphIDs(plain, 1)[0] !== 0) return plain;
              missing.add(character);
              return character;
            })
            .join("");
          const measure = font.measureText(renderedText);
          const x =
            text.align === "center"
              ? -measure.width / 2
              : text.align === "right"
                ? -measure.width
                : 0;
          canvas.save();
          canvas.translate(text.x, text.y);
          canvas.rotate(text.rotation, 0, 0);
          const y =
            text.verticalAlign === "middle"
              ? -measure.y - measure.height / 2
              : text.verticalAlign === "top"
                ? -measure.y
                : text.verticalAlign === "bottom"
                  ? -measure.y - measure.height
                  : 0;
          canvas.drawText(renderedText, x, y, textPaint, font);
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
