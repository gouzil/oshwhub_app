import type { Bounds } from "@oshwhub/bridge";
import type { Viewport } from "./viewport";

export type RasterPlan = {
  x: number;
  y: number;
  width: number;
  height: number;
  pixelWidth: number;
  pixelHeight: number;
};

export function planRaster(bounds: Bounds, density: number): RasterPlan {
  const width = Math.max(bounds.maxX - bounds.minX, 0.001);
  const height = Math.max(bounds.maxY - bounds.minY, 0.001);
  const scale = Math.min(
    density,
    4096 / Math.max(width, height),
    Math.sqrt(4_194_304 / width / height),
  );
  return {
    x: bounds.minX,
    y: bounds.minY,
    width,
    height,
    pixelWidth: Math.max(1, Math.floor(width * scale)),
    pixelHeight: Math.max(1, Math.floor(height * scale)),
  };
}

export function detailBounds(view: Viewport, width: number, height: number): Bounds {
  const marginX = width * 0.125;
  const marginY = height * 0.125;
  return {
    minX: (-view.x - marginX) / view.scale,
    minY: (-view.y - marginY) / view.scale,
    maxX: (-view.x + width + marginX) / view.scale,
    maxY: (-view.y + height + marginY) / view.scale,
  };
}

export function coversView(plan: RasterPlan, view: Viewport, width: number, height: number) {
  return (
    plan.x * view.scale + view.x <= 0 &&
    plan.y * view.scale + view.y <= 0 &&
    (plan.x + plan.width) * view.scale + view.x >= width &&
    (plan.y + plan.height) * view.scale + view.y >= height
  );
}
