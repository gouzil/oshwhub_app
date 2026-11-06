import { expect, test } from "@playwright/test";
import { decodePng } from "../e2e/pixels";

test.use({ viewport: { width: 1024, height: 768 }, deviceScaleFactor: 2 });

test("Rust GPU canvas mounts within NSH layout, resizes and preserves overlay/focus", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/surface/");
  await expect(page.getByTestId("gpu-status")).toContainText("Ready");
  const canvas = page.locator("canvas");
  await expect(canvas).toHaveCount(1);
  await expect(canvas).toHaveAttribute("width", "640");
  await expect(canvas).toHaveAttribute("height", "480");
  const host = page.getByTestId("gpu-host");
  await expect(host).toHaveCSS("overflow", /hidden|clip/);
  await page.getByLabel("三维绘图区域", { exact: true }).focus();
  await expect(page.getByLabel("三维绘图区域", { exact: true })).toBeFocused();
  await page.getByRole("button", { name: "Resize GPU", exact: true }).click();
  await expect(canvas).toHaveAttribute("width", "960");
  await expect(page.getByTestId("gpu-status")).toContainText("width: 960");
  const overlay = page.getByTestId("gpu-overlay");
  const rect = await overlay.boundingBox();
  expect(rect).toBeTruthy();
  const top = await page.evaluate(
    ({ x, y }) => document.elementFromPoint(x, y)?.closest('[data-testid="gpu-overlay"]') !== null,
    { x: (rect?.x ?? 0) + 20, y: (rect?.y ?? 0) + 20 },
  );
  expect(top).toBe(true);
  await expect(host).toHaveScreenshot("gpu-host.png", { mask: [overlay] });
  await host.screenshot({ path: "output/nsh-web-tests/surface.png" });
  expect(errors).toEqual([]);
});

test("twenty NSH mount/dispose cycles remove each GPU canvas", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/surface/");
  for (let cycle = 0; cycle < 20; cycle++) {
    await expect(page.getByTestId("gpu-status")).toContainText("Ready");
    await expect(page.locator("canvas")).toHaveCount(1);
    await page.getByRole("button", { name: "Toggle GPU", exact: true }).click();
    await expect(page.locator("canvas")).toHaveCount(0);
    await expect(page.getByLabel("三维绘图区域", { exact: true })).toHaveCount(0);
    await page.getByRole("button", { name: "Toggle GPU", exact: true }).click();
  }
  await expect(page.getByTestId("gpu-status")).toContainText("Ready");
  expect(errors).toEqual([]);
});

test("typed PCB surface renders and zooms from toolbar and browser wheel", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/surface/");
  await expect(page.getByTestId("gpu-status")).toContainText("Ready");
  await page.getByRole("button", { name: "Show PCB", exact: true }).click();
  const canvas = page.locator("canvas");
  await expect(canvas).toHaveCount(1);
  await expect(page.getByText("0 / 0 个器件", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "重试三维模型", exact: true })).toHaveCount(0);
  const initial = await canvas.screenshot();
  await page.getByRole("button", { name: "放大三维场景", exact: true }).click();
  await expect.poll(async () => (await canvas.screenshot()).equals(initial)).toBe(false);
  const zoomed = await canvas.screenshot();
  await canvas.hover();
  await page.mouse.wheel(0, 100);
  await expect.poll(async () => (await canvas.screenshot()).equals(zoomed)).toBe(false);
  await canvas.screenshot({ path: "output/nsh-web-tests/pcb3d.png" });
  expect(errors).toEqual([]);
});

test("retained surface draws resources received while hidden when shown again", async ({
  page,
}) => {
  await page.goto("/surface/");
  await page.getByRole("button", { name: "Retained GPU", exact: true }).click();
  const canvas = page.locator("canvas");
  await expect(page.getByText(/Retained Ready/)).toBeVisible();
  await expect(canvas).toHaveCount(1);
  await page.getByRole("button", { name: "Hide retained GPU", exact: true }).click();
  await expect(canvas).toBeHidden();
  await page.getByRole("button", { name: "Load retained board", exact: true }).click();
  await page.getByRole("button", { name: "Show retained GPU", exact: true }).click();
  await expect
    .poll(async () => {
      const { pixels, channels } = decodePng(await canvas.screenshot());
      let red = 0;
      for (let at = 0; at < pixels.length; at += channels)
        if (pixels[at] > 70 && pixels[at + 1] < 40 && pixels[at + 2] < 40) red++;
      return red;
    })
    .toBeGreaterThan(100);
  await expect(canvas).toHaveCount(1);
  await canvas.screenshot({ path: "output/nsh-web-tests/retained-restored.png" });
});
