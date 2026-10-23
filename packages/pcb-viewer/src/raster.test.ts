import { describe, expect, it } from "vitest";
import { coversView, detailBounds, planRaster } from "./raster";

describe("bounded PCB raster cache", () => {
  it("caps Retina and extremely thin regions without zero-sized images", () => {
    for (const [width, height] of [
      [1000, 1000],
      [1e8, 0.001],
      [0.001, 1e8],
      [0, 0],
    ]) {
      const plan = planRaster({ minX: 10, minY: -20, maxX: width + 10, maxY: height - 20 }, 100);
      expect(plan.pixelWidth).toBeGreaterThanOrEqual(1);
      expect(plan.pixelHeight).toBeGreaterThanOrEqual(1);
      expect(Math.max(plan.pixelWidth, plan.pixelHeight)).toBeLessThanOrEqual(4096);
      expect(plan.pixelWidth * plan.pixelHeight).toBeLessThanOrEqual(4_194_304);
      expect([plan.x, plan.y]).toEqual([10, -20]);
    }
  });

  it("covers a panned and zoomed viewport with room for motion", () => {
    const view = { scale: 2, x: -100, y: 40 };
    const bounds = detailBounds(view, 800, 400);
    expect(bounds).toEqual({ minX: 0, minY: -45, maxX: 500, maxY: 205 });
    const plan = planRaster(bounds, 4);
    expect([plan.pixelWidth, plan.pixelHeight]).toEqual([2000, 1000]);
    expect(coversView(plan, view, 800, 400)).toBe(true);
    expect(coversView(plan, { ...view, x: -180 }, 800, 400)).toBe(true);
    expect(coversView(plan, { ...view, x: -400 }, 800, 400)).toBe(false);
  });
});
