import { describe, expect, it } from "vitest";
import { fitViewport, gestureViewport, isTap, worldPoint, zoomAt } from "./viewport";

describe("PCB viewport", () => {
  it("fits non-origin bounds with an equal border without changing world coordinates", () => {
    expect(fitViewport({ minX: 10, minY: 20, maxX: 110, maxY: 70 }, 600, 400, 50)).toEqual({
      scale: 5,
      x: 0,
      y: -25,
    });
  });
  it("keeps the world point under the zoom anchor fixed and clamps scale", () => {
    const start = { scale: 2, x: 10, y: 20 };
    expect(zoomAt(start, 4, { x: 110, y: 70 }, 1, 3)).toEqual({ scale: 3, x: -40, y: -5 });
    expect(worldPoint({ scale: 4, x: -90, y: -30 }, { x: 110, y: 70 })).toEqual({ x: 50, y: 25 });
  });
  it("pans with one pointer and moves a pinch around its moving center", () => {
    const start = { scale: 2, x: 10, y: 20 };
    expect(gestureViewport(start, [{ x: 100, y: 100 }], [{ x: 130, y: 90 }], 1, 10)).toEqual({
      scale: 2,
      x: 40,
      y: 10,
    });
    expect(
      gestureViewport(
        start,
        [
          { x: 100, y: 100 },
          { x: 200, y: 100 },
        ],
        [
          { x: 110, y: 120 },
          { x: 310, y: 120 },
        ],
        1,
        10,
      ),
    ).toEqual({ scale: 4, x: -70, y: -40 });
  });
  it("keeps degenerate geometry finite and treats dragging as different from tapping", () => {
    const fit = fitViewport({ minX: 4, minY: 4, maxX: 4, maxY: 4 }, 0, 0);
    expect(Object.values(fit).every(Number.isFinite)).toBe(true);
    expect(isTap({ x: 10, y: 10 }, { x: 12, y: 13 }, 1)).toBe(true);
    expect(isTap({ x: 10, y: 10 }, { x: 20, y: 10 }, 1)).toBe(false);
    expect(isTap({ x: 10, y: 10 }, { x: 10, y: 10 }, 2)).toBe(false);
  });
});
