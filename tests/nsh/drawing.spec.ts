import assert from "node:assert/strict";
import { expect, test } from "@playwright/test";
import { decodePng } from "../e2e/pixels";

test("2D gestures composite cached pixels and refine only after settling", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.addInitScript(() => {
    const counts = { close: 0 };
    Object.assign(window, { pathBuilds: counts });
    const close = Path2D.prototype.closePath;
    Path2D.prototype.closePath = function () {
      counts.close++;
      return close.call(this);
    };
  });
  await page.goto("/surface/");
  await page.getByRole("button", { name: "Show drawing", exact: true }).click();
  const canvas = page.getByTestId("pcb-canvas");
  await expect(canvas).toBeVisible();
  await expect(page.getByTestId("pcb-overview")).toBeVisible();
  await page.waitForTimeout(300);
  const builds = () =>
    page.evaluate(() => (window as unknown as { pathBuilds: { close: number } }).pathBuilds.close);
  const before = await builds();
  expect(before).toBeGreaterThan(0);
  const image = await canvas.screenshot();
  await canvas.hover();
  for (let i = 0; i < 20; i++) {
    await page.mouse.wheel(0, -30);
    await page.waitForTimeout(40);
  }
  expect(await builds()).toBe(before);
  await expect(page.getByTestId("pcb-detail")).toBeVisible();
  expect(await builds()).toBeGreaterThan(before);
  expect((await canvas.screenshot()).equals(image)).toBe(false);
});

test.describe("Retina drawing", () => {
  test.use({ deviceScaleFactor: 2 });

  test("detail preserves the zoom anchor, translucent fill and nonzero hole", async ({ page }) => {
    await page.setViewportSize({ width: 900, height: 700 });
    await page.goto("/surface/");
    await page.getByRole("button", { name: "Show drawing", exact: true }).click();
    const canvas = page.getByTestId("pcb-canvas");
    const overview = page.getByTestId("pcb-overview");
    await expect(overview).toBeVisible();
    const anchor = await overview.boundingBox();
    assert(anchor);
    const x = Math.round(anchor.x + (anchor.width * 15) / 120);
    const y = Math.round(anchor.y + (anchor.height * 50) / 80);
    const anchorX = (x - anchor.x) / anchor.width;
    const anchorY = (y - anchor.y) / anchor.height;
    await page.mouse.move(x, y);
    // Chromium divides CDP wheel deltas by the emulated device scale factor.
    for (let i = 0; i < 20; i++) {
      await page.mouse.wheel(0, -60);
      await page.waitForTimeout(40);
    }
    const detail = page.getByTestId("pcb-detail");
    await expect(detail).toBeVisible();
    const zoomed = await overview.boundingBox();
    assert(zoomed);
    expect(Math.abs(zoomed.x + zoomed.width * anchorX - x)).toBeLessThan(1);
    expect(Math.abs(zoomed.y + zoomed.height * anchorY - y)).toBeLessThan(1);
    expect(zoomed.width / anchor.width).toBeCloseTo(6.04964746, 3);
    for (const layer of [overview, detail]) {
      const size = await layer
        .locator("canvas")
        .evaluate((el: HTMLCanvasElement) => [el.width, el.height]);
      expect(size[0] * size[1]).toBeLessThanOrEqual(4_194_304);
    }
    const frame = await canvas.boundingBox();
    assert(frame);
    const fillX = Math.round((x - frame.x) * 2);
    const holeX = Math.round((zoomed.x + (zoomed.width * 25) / 120 - frame.x) * 2);
    const row = Math.round((y - frame.y) * 2);
    for (const visible of [true, false]) {
      await detail.evaluate((el, visible) => {
        el.style.visibility = visible ? "visible" : "hidden";
      }, visible);
      const png = decodePng(await canvas.screenshot());
      for (const [column, expected] of [
        [fillX, [136, 63, 19]],
        [holeX, [17, 24, 39]],
      ] as const) {
        expect(column).toBeGreaterThanOrEqual(0);
        expect(column).toBeLessThan(png.width);
        const at = (row * png.width + column) * png.channels;
        for (let channel = 0; channel < 3; channel++) {
          expect(Math.abs(png.pixels[at + channel] - expected[channel])).toBeLessThanOrEqual(2);
        }
      }
    }
  });
});

for (const [width, height] of [
  [390, 844],
  [844, 390],
  [768, 1024],
  [1024, 768],
  [1440, 900],
  [1920, 1080],
]) {
  test(`drawing preserves Chinese text and nested SVG text/images at ${width}x${height}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height });
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto("/surface/");
    await page.getByRole("button", { name: "Show drawing", exact: true }).click();
    const canvas = page.getByTestId("pcb-canvas");
    await expect(canvas).toBeVisible();
    await expect(page.getByText(/加载原理图图片/)).toHaveCount(0);
    await expect
      .poll(async () => {
        const { width: w, height: h, channels, pixels } = decodePng(await canvas.screenshot());
        let left = w;
        let right = -1;
        let top = h;
        let bottom = -1;
        for (let at = 0; at < pixels.length; at += channels) {
          if (pixels[at] < 30 && pixels[at + 1] > 200 && pixels[at + 2] < 30) {
            const p = at / channels;
            const x = p % w;
            const y = Math.floor(p / w);
            left = Math.min(left, x);
            right = Math.max(right, x);
            top = Math.min(top, y);
            bottom = Math.max(bottom, y);
          }
        }
        if (right < 0) return { chinese: false, svgText: false, svgImage: false };
        const counts = { chinese: 0, svgText: 0, svgImage: 0 };
        for (let at = 0; at < pixels.length; at += channels) {
          if (pixels[at] < 220 || pixels[at + 1] < 220 || pixels[at + 2] < 220) continue;
          const p = at / channels;
          const x = p % w;
          const y = Math.floor(p / w);
          if (x < left || x > right || y < top || y > bottom) counts.chinese++;
          else if (y < top + (bottom - top) * 0.55) counts.svgText++;
          else counts.svgImage++;
        }
        return {
          chinese: counts.chinese > 50,
          svgText: counts.svgText > 20,
          svgImage: counts.svgImage > 20,
        };
      })
      .toEqual({
        chinese: true,
        svgText: true,
        svgImage: true,
      });
    const screenshot = decodePng(await canvas.screenshot());
    expect(screenshot.width).toBeGreaterThan(100);
    expect(errors).toEqual([]);
    await canvas.screenshot({ path: `output/nsh-web-tests/drawing-${width}x${height}.png` });
  });
}
