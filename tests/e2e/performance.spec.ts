import { fitViewport } from "../../packages/pcb-viewer/src/viewport";
import { expect, test } from "./fixtures";
import { canvasColors } from "./pixels";

type Method = (this: object, ...args: unknown[]) => unknown;
type Probe = { pictures: number; frames: number };
declare global {
  interface Window {
    __pcbRasterProbe: Probe;
  }
}

test("camera motion reuses rasterized layers and panels do not redraw the board", async ({
  page,
  bridge,
}) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await page.getByRole("button", { name: "打开示例板", exact: true }).click();
  const canvas = page.locator("canvas");
  await expect
    .poll(async () => canvasColors(await canvas.screenshot()).chinese)
    .toBeGreaterThan(10);
  expect(bridge.requests.some((request) => request.command === "demo")).toBe(true);
  await page.evaluate(() => {
    const kit = (
      window as unknown as {
        CanvasKit: {
          Canvas: { prototype: { drawPicture: Method } };
          Surface: { prototype: { flush: Method } };
        };
      }
    ).CanvasKit;
    const probe = { pictures: 0, frames: 0 };
    window.__pcbRasterProbe = probe;
    const draw = kit.Canvas.prototype.drawPicture;
    kit.Canvas.prototype.drawPicture = function (...args) {
      probe.pictures++;
      return draw.apply(this, args);
    };
    const flush = kit.Surface.prototype.flush;
    kit.Surface.prototype.flush = function (...args) {
      probe.frames++;
      return flush.apply(this, args);
    };
  });
  const box = await canvas.boundingBox();
  if (!box) throw new Error("PCB canvas has no bounds");
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (let index = 0; index < 20; index++) await page.mouse.wheel(0, index < 10 ? -25 : 25);
  await page.mouse.down();
  for (let index = 0; index < 30; index++)
    await page.mouse.move(box.x + box.width / 2 + index, box.y + box.height / 2);
  await page.mouse.up();
  const motion = await page.evaluate(() => window.__pcbRasterProbe);
  expect(motion.frames).toBeGreaterThan(5);
  // The root display list is replayed once per frame; layer vectors must stay cached.
  expect(motion.pictures).toBeLessThanOrEqual(motion.frames + 4);
  await page.getByRole("button", { name: "图层", exact: true }).click();
  const beforePanel = await page.evaluate(() => window.__pcbRasterProbe.frames);
  await page.getByRole("button", { name: "关闭图层面板", exact: true }).click();
  await page.getByRole("button", { name: "查看器信息", exact: true }).click();
  expect(await page.evaluate(() => window.__pcbRasterProbe.frames)).toBe(beforePanel);
  await test.info().attach("camera-render-work", {
    body: JSON.stringify(motion),
    contentType: "application/json",
  });
});

test("a failed overview can recover on the next layer rebuild", async ({ page, bridge }) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await page.getByRole("button", { name: "打开示例板", exact: true }).click();
  const canvas = page.locator("canvas");
  await expect
    .poll(async () => canvasColors(await canvas.screenshot()).copper)
    .toBeGreaterThan(100);
  expect(bridge.requests.some((request) => request.command === "demo")).toBe(true);
  await page.evaluate(() => {
    const kit = (window as unknown as { CanvasKit: { MakeSurface: Method } }).CanvasKit;
    const make = kit.MakeSurface;
    kit.MakeSurface = () => {
      kit.MakeSurface = make;
      throw new Error("one-shot raster allocation failure");
    };
  });
  await page.getByRole("button", { name: "图层", exact: true }).click();
  const copper = page.getByRole("checkbox", { name: "顶层铜", exact: true });
  await copper.click();
  await expect(page.getByRole("alert")).toHaveText("one-shot raster allocation failure");
  await copper.click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await page.getByRole("button", { name: "关闭图层面板", exact: true }).click();
  await expect
    .poll(async () => canvasColors(await canvas.screenshot()).copper)
    .toBeGreaterThan(100);
});

test("idle detail stays aligned after high zoom, pan, and layer invalidation", async ({
  page,
  bridge,
}) => {
  let builds = 0;
  page.on("console", (message) => {
    if (message.text().startsWith("pcb.raster.built")) builds++;
  });
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await page.getByRole("button", { name: "打开示例板", exact: true }).click();
  const canvas = page.locator("canvas");
  await expect
    .poll(async () => canvasColors(await canvas.screenshot()).copper)
    .toBeGreaterThan(100);
  expect(bridge.requests.some((request) => request.command === "demo")).toBe(true);
  for (let index = 0; index < 10; index++)
    await page.getByRole("button", { name: "放大 PCB", exact: true }).click();
  await expect(page.getByText("931%", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "查看器信息", exact: true }).click();
  await expect(page.getByText("视区细节", { exact: true })).toHaveCount(1);
  await page.getByRole("button", { name: "关闭查看器信息", exact: true }).click();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("PCB canvas has no bounds");
  const center = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  const centerColors = async () =>
    canvasColors(
      await page.screenshot({ clip: { x: center.x - 4, y: center.y - 4, width: 8, height: 8 } }),
    );
  expect((await centerColors()).copper).toBeGreaterThan(50);
  const beforePan = builds;
  // Move from world y=40 (top copper) to y=56 (bottom copper), beyond the old detail bounds.
  const fit = fitViewport({ minX: 0, minY: 0, maxX: 120, maxY: 80 }, box.width, box.height);
  let remaining = 16 * fit.scale * 1.25 ** 10;
  while (remaining > 0.01) {
    const step = Math.min(remaining, box.height * 0.5);
    const start = { x: center.x, y: box.y + box.height * 0.8 };
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(start.x, start.y - step, { steps: 4 });
    await page.mouse.up();
    remaining -= step;
  }
  await expect.poll(() => builds).toBeGreaterThan(beforePan);
  await expect.poll(async () => (await centerColors()).bottom).toBeGreaterThan(50);
  await page.getByRole("button", { name: "图层", exact: true }).click();
  await page.getByRole("checkbox", { name: "底层铜", exact: true }).click();
  await page.getByRole("button", { name: "关闭图层面板", exact: true }).click();
  await expect.poll(async () => (await centerColors()).bottom).toBe(0);
  await expect.poll(async () => (await centerColors()).copper).toBeGreaterThan(50);
  await page.getByRole("button", { name: "查看器信息", exact: true }).click();
  await expect(page.getByText("视区细节", { exact: true })).toHaveCount(1);
  await page.getByRole("button", { name: "关闭查看器信息", exact: true }).click();
  expect((await centerColors()).bottom).toBe(0);
  expect((await centerColors()).copper).toBeGreaterThan(50);
});
