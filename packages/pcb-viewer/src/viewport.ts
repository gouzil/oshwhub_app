import type { Bounds } from "@oshwhub/bridge";
export type Point = { x: number; y: number };
export type Viewport = { scale: number; x: number; y: number };
export function fitViewport(bounds: Bounds, width: number, height: number, padding = 24): Viewport {
  const worldWidth = Math.max(bounds.maxX - bounds.minX, 0.001);
  const worldHeight = Math.max(bounds.maxY - bounds.minY, 0.001);
  const inset = Math.min(padding, width * 0.15, height * 0.15);
  const scale = Math.min(
    Math.max(1, width - inset * 2) / worldWidth,
    Math.max(1, height - inset * 2) / worldHeight,
  );
  return {
    scale,
    x: (width - worldWidth * scale) / 2 - bounds.minX * scale,
    y: (height - worldHeight * scale) / 2 - bounds.minY * scale,
  };
}
export function zoomAt(
  view: Viewport,
  nextScale: number,
  anchor: Point,
  min: number,
  max: number,
): Viewport {
  const scale = Math.max(min, Math.min(max, nextScale));
  const world = worldPoint(view, anchor);
  return { scale, x: anchor.x - world.x * scale, y: anchor.y - world.y * scale };
}
export function worldPoint(view: Viewport, point: Point): Point {
  return { x: (point.x - view.x) / view.scale, y: (point.y - view.y) / view.scale };
}
const center = (points: Point[]) =>
  points.length > 1
    ? { x: (points[0].x + points[1].x) / 2, y: (points[0].y + points[1].y) / 2 }
    : points[0];
export function gestureViewport(
  view: Viewport,
  start: Point[],
  current: Point[],
  min: number,
  max: number,
): Viewport {
  if (!start.length || !current.length) return view;
  const from = center(start);
  const to = center(current);
  let scale = view.scale;
  if (start.length > 1 && current.length > 1) {
    const before = Math.hypot(start[1].x - start[0].x, start[1].y - start[0].y);
    const after = Math.hypot(current[1].x - current[0].x, current[1].y - current[0].y);
    if (before > 0) scale = Math.max(min, Math.min(max, (view.scale * after) / before));
  }
  const world = worldPoint(view, from);
  return { scale, x: to.x - world.x * scale, y: to.y - world.y * scale };
}
export function isTap(start: Point, end: Point, pointerCount: number): boolean {
  return pointerCount === 1 && Math.hypot(end.x - start.x, end.y - start.y) <= 5;
}
