import type { Bounds, Scene } from "@oshwhub/bridge";
import { Skia, type SkPicture, type SkTypeface } from "@shopify/react-native-skia";
import { netKind } from "./nets";
import type { Viewport } from "./viewport";

export type NetLabelOptions = {
  visibleLayers: ReadonlySet<number>;
  showLabels: boolean;
  showPadLabels: boolean;
  showTrackLabels: boolean;
  showViaLabels: boolean;
  viewport: Viewport;
  width: number;
  height: number;
};

export function buildNetLabelPicture(
  scene: Scene,
  typeface: SkTypeface | undefined,
  options: NetLabelOptions,
): SkPicture | null {
  const { viewport: view, width, height } = options;
  if (
    !typeface ||
    !options.showLabels ||
    !scene.netLabels?.length ||
    view.scale <= 0 ||
    !width ||
    !height
  )
    return null;
  const recorder = Skia.PictureRecorder();
  const canvas = recorder.beginRecording(
    Skia.XYWHRect(
      -view.x / view.scale,
      -view.y / view.scale,
      width / view.scale,
      height / view.scale,
    ),
  );
  const font = Skia.Font(typeface, 12);
  const paint = Skia.Paint();
  paint.setAntiAlias(true);
  paint.setColor(Skia.Color("#ffffff"));
  const occupied = new Map<string, Bounds[]>();
  const cells = (rect: Bounds) => {
    const keys: string[] = [];
    for (
      let x = Math.floor(Math.max(0, rect.minX) / 32);
      x <= Math.floor(Math.min(width, rect.maxX) / 32);
      x++
    )
      for (
        let y = Math.floor(Math.max(0, rect.minY) / 32);
        y <= Math.floor(Math.min(height, rect.maxY) / 32);
        y++
      )
        keys.push(`${x}/${y}`);
    return keys;
  };
  const occupy = (rect: Bounds, keys: string[]) => {
    for (const key of keys) {
      const cell = occupied.get(key) ?? [];
      cell.push(rect);
      occupied.set(key, cell);
    }
  };
  // Drill areas stay clear even when their own label category is disabled.
  for (const label of scene.netLabels) {
    if (!label.hole || (!options.visibleLayers.has(label.layer) && !options.visibleLayers.has(47)))
      continue;
    const angle = (label.rotation * Math.PI) / 180;
    const cos = Math.abs(Math.cos(angle)),
      sin = Math.abs(Math.sin(angle));
    const halfWidth = ((label.width * cos + label.height * sin) * view.scale) / 2;
    const halfHeight = ((label.width * sin + label.height * cos) * view.scale) / 2;
    const x = label.x * view.scale + view.x,
      y = label.y * view.scale + view.y;
    const rect = {
      minX: x - halfWidth,
      maxX: x + halfWidth,
      minY: y - halfHeight,
      maxY: y + halfHeight,
    };
    occupy(rect, cells(rect));
  }
  let drawn = false;
  try {
    for (const label of scene.netLabels) {
      const kind = netKind(label.kind);
      if (
        !options.visibleLayers.has(label.layer) ||
        (kind === "pad" && !options.showPadLabels) ||
        (kind === "track" && !options.showTrackLabels) ||
        (kind === "via" && !options.showViaLabels) ||
        kind === "other"
      )
        continue;
      let w = label.width * view.scale;
      let h = label.height * view.scale;
      if (!(w > 0 && h > 0)) continue;
      const outside = label.hole || kind === "via";
      let rotation = outside ? 0 : label.rotation;
      if (!outside && h > w) {
        [w, h] = [h, w];
        rotation += 90;
      }
      rotation = (((rotation % 180) + 270) % 180) - 90;
      const radians = (rotation * Math.PI) / 180;
      const cos = Math.cos(radians),
        sin = Math.sin(radians);
      const x = label.x * view.scale + view.x;
      const y = label.y * view.scale + view.y;
      const radius = Math.hypot(w, h) / 2;
      if (
        x + radius + (outside ? 128 : 0) < 0 ||
        x - radius > width ||
        y + radius < 0 ||
        y - radius > height
      )
        continue;
      if (outside && Math.min(w, h) < 12) continue;
      const lines = [kind === "pad" ? label.padNumber : "", label.net?.trim() ?? ""].filter(
        Boolean,
      );
      const availableWidth = outside ? 120 : w * 0.72;
      const availableHeight = outside ? Math.min(w, h) * 0.45 : h * (lines.length > 1 ? 0.25 : 0.6);
      if (availableHeight < 8) continue;
      for (let i = 0; i < lines.length; i++) {
        const text = lines[i];
        font.setSize(12);
        const measure = font.measureText(text);
        const size =
          12 * Math.min(1, availableWidth / Math.max(measure.width, 1), availableHeight / 12);
        if (size < 8) continue;
        font.setSize(size);
        const bounds = font.measureText(text);
        const offset = (i - (lines.length - 1) / 2) * (outside ? 16 : h * 0.46);
        const offsetX = outside ? w / 2 + 4 + bounds.width / 2 : 0;
        const centerX = x + offsetX - sin * offset;
        const centerY = y + cos * offset;
        const halfWidth = (Math.abs(cos) * bounds.width + Math.abs(sin) * bounds.height) / 2 + 2;
        const halfHeight = (Math.abs(sin) * bounds.width + Math.abs(cos) * bounds.height) / 2 + 2;
        const rect = {
          minX: centerX - halfWidth,
          maxX: centerX + halfWidth,
          minY: centerY - halfHeight,
          maxY: centerY + halfHeight,
        };
        if (rect.maxX < 0 || rect.minX > width || rect.maxY < 0 || rect.minY > height) continue;
        const keys = cells(rect);
        const collision = keys.some((key) =>
          occupied
            .get(key)
            ?.some(
              (r) =>
                rect.minX < r.maxX &&
                rect.maxX > r.minX &&
                rect.minY < r.maxY &&
                rect.maxY > r.minY,
            ),
        );
        if (collision) continue;
        occupy(rect, keys);
        canvas.save();
        canvas.translate(label.x + offsetX / view.scale, label.y);
        canvas.rotate(rotation, 0, 0);
        canvas.scale(1 / view.scale, 1 / view.scale);
        canvas.drawText(
          text,
          -bounds.x - bounds.width / 2,
          offset - bounds.y - bounds.height / 2,
          paint,
          font,
        );
        canvas.restore();
        drawn = true;
      }
    }
    return drawn ? recorder.finishRecordingAsPicture() : null;
  } finally {
    font.dispose();
    paint.dispose();
    recorder.dispose();
  }
}
