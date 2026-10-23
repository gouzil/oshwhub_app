import { describe, expect, it } from "vitest";
import { fitCamera, orbitCamera, panCamera, projection, zoomCamera } from "./camera3d";

describe("3D camera", () => {
  const bounds = { min: [-30, -20, 0], max: [30, 20, 8] };
  it("fits all board corners in both a narrow portrait and a wide viewport", () => {
    for (const aspect of [0.44, 2.4]) {
      const camera = fitCamera(bounds, aspect);
      const m = projection(camera, aspect);
      for (const x of [-30, 30])
        for (const y of [-20, 20])
          for (const z of [0, 8]) {
            const p = [x, y, z, 1];
            const out = [0, 1, 2, 3].map((r) => p.reduce((sum, v, c) => sum + m[c * 4 + r] * v, 0));
            expect(out[3]).toBeGreaterThan(0);
            expect(Math.abs(out[0] / out[3])).toBeLessThan(0.96);
            expect(Math.abs(out[1] / out[3])).toBeLessThan(0.96);
          }
    }
  });
  it("keeps repeated zoom, pole rotation and panning finite and bounded", () => {
    const start = fitCamera(bounds, 1);
    let camera = start;
    for (let i = 0; i < 100; i++) camera = zoomCamera(orbitCamera(camera, 400, 400), 2);
    expect(camera.distance).toBeGreaterThan(0);
    expect(projection(camera, 0).every(Number.isFinite)).toBe(true);
    expect(Math.abs(camera.pitch)).toBeLessThanOrEqual(Math.PI / 2);
    const panned = panCamera(start, 100, 0, 800);
    expect(panned.target).not.toEqual(start.target);
    expect(panned.distance).toBe(start.distance);
    expect(start.target).toEqual([0, 0, 4]);
  });
  it("separates board and copper planes with a native 16-bit depth buffer", () => {
    const camera = { ...fitCamera(bounds, 1), pitch: Math.PI / 2 };
    const m = projection(camera, 1);
    const depth = (z: number) => {
      const clipZ = m[10] * z + m[14];
      const w = m[11] * z + m[15];
      return Math.round(((clipZ / w) * 0.5 + 0.5) * 65535);
    };
    expect(Math.abs(depth(0.01) - depth(0))).toBeGreaterThan(0);
  });
});
