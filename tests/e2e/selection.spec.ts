import { expect, test } from "./fixtures";

test("selected components show readable properties in a compact card and keep IDs collapsed", async ({
  page,
  bridge,
}) => {
  bridge.pickResult = {
    id: "f0d8af1247985cb7/e5",
    kind: "POLY",
    layer: 1,
    net: "GND",
    properties: {
      Designator: "C49",
      Value: "100 nF",
      FootprintName: "C0603",
      Footprint: "9a146d511b4d87b3",
      Device: "dd165e9494ea163f",
      componentId: "f0d8af1247985cb7",
      recordId: "e5",
      ticket: "29",
      "Channel ID": "$4I73",
      "Unique ID": "gge875",
    },
    bounds: { minX: 56, minY: 36, maxX: 64, maxY: 44 },
  };
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await page.getByRole("button", { name: "打开示例板", exact: true }).click();
  await expect(page.locator("canvas")).toBeVisible();
  const surface = page.getByRole("img", { name: /^自制中文测试 PCB，PCB 画布/ });
  await surface.tap();
  await expect(page.getByRole("heading", { name: "C49", exact: true })).toBeVisible();
  await expect(page.getByText("GND", { exact: true })).toBeInViewport({ ratio: 1 });
  await expect(page.getByText("参数", { exact: true })).toBeVisible();
  await expect(page.getByText("100 nF", { exact: true })).toBeVisible();
  await expect(page.getByText("封装", { exact: true })).toBeVisible();
  await expect(page.getByText("C0603", { exact: true })).toBeVisible();
  await expect(
    page.getByText(/f0d8af1247985cb7|9a146d511b4d87b3|dd165e9494ea163f|\$4I73|gge875/),
  ).toHaveCount(0);
  const panel = page.getByTestId("pcb-selection-panel");
  const box = await panel.boundingBox();
  const viewport = page.viewportSize();
  if (!box || !viewport) throw new Error("Selection panel is missing");
  expect(box.height).toBeLessThan(400);
  expect(box.y + box.height).toBeLessThanOrEqual(viewport.height);
  await page.getByRole("button", { name: "技术详情", exact: true }).click();
  await expect(page.getByText("f0d8af1247985cb7/e5", { exact: true })).toBeAttached();
  await page.getByRole("button", { name: "关闭图元属性", exact: true }).click();
  await expect(panel).toHaveCount(0);
  await surface.tap();
  await expect(page.getByRole("button", { name: "技术详情", exact: true })).toHaveAttribute(
    "aria-expanded",
    "false",
  );
  bridge.pickResult = null;
  // Tap outside the card, where the fitted fixture has no interactive controls.
  await surface.tap({ position: { x: 4, y: 4 } });
  await expect(panel).toHaveCount(0);
});

test("long properties wrap inside the inspector without covering its close control", async ({
  page,
  bridge,
}) => {
  const longName = "超长器件名称与封装说明".repeat(30);
  bridge.pickResult = {
    id: "test-part/3",
    kind: "PAD",
    layer: 1,
    net: "USB_D+",
    properties: { Designator: "U1", PadNumber: "3", Name: longName, recordId: "3" },
    bounds: { minX: 56, minY: 36, maxX: 64, maxY: 44 },
  };
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await page.getByRole("button", { name: "打开示例板", exact: true }).click();
  await expect(page.locator("canvas")).toBeVisible();
  const surface = page.getByRole("img", { name: /^自制中文测试 PCB，PCB 画布/ });
  await surface.tap();
  await expect(page.getByRole("heading", { name: "U1 · 焊盘 3", exact: true })).toBeVisible();
  await expect(page.getByText("USB_D+", { exact: true })).toBeInViewport({ ratio: 1 });
  const panel = await page.getByTestId("pcb-selection-panel").boundingBox();
  const text = await page.getByText(longName, { exact: true }).boundingBox();
  if (!panel || !text) throw new Error("Missing inspector bounds");
  expect(text.x).toBeGreaterThan(panel.x);
  expect(text.x + text.width).toBeLessThanOrEqual(panel.x + panel.width);
  const viewport = page.viewportSize();
  if (!viewport) throw new Error("Missing viewport");
  expect(panel.y + panel.height).toBeLessThanOrEqual(viewport.height);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(
    viewport.width,
  );
  await page.getByRole("button", { name: "关闭图元属性", exact: true }).click();
  await expect(page.getByTestId("pcb-selection-panel")).toHaveCount(0);
  await expect(page.locator("canvas")).toBeVisible();
});
