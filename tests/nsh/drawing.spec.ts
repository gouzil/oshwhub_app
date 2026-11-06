import { expect, test } from "@playwright/test";
import { decodePng } from "../e2e/pixels";

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
