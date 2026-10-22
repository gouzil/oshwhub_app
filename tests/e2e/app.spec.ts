import type { Page } from "@playwright/test";
import { expect, test, unsafeMarkup } from "./fixtures";
import { canvasColors } from "./pixels";

async function expectViewport(page: Page) {
  const dimensions = await page.evaluate(() => ({
    width: document.documentElement.scrollWidth,
    height: document.documentElement.scrollHeight,
    viewportWidth: window.innerWidth,
    viewportHeight: window.innerHeight,
    appHeight: document.querySelector("#root > div")?.getBoundingClientRect().height,
  }));
  expect(dimensions.width).toBeLessThanOrEqual(dimensions.viewportWidth + 1);
  expect(dimensions.height).toBeLessThanOrEqual(dimensions.viewportHeight + 1);
  expect(dimensions.appHeight).toBeLessThanOrEqual(dimensions.viewportHeight + 1);
}

test("idle and hidden feed do not request extra pages; real fonts render in the demo", async ({
  page,
  bridge,
}) => {
  bridge.endlessFeed = true;
  await page.goto("/");
  await expect(page.getByRole("button", { name: /^自制开源控制板 01，/ })).toBeVisible();
  await expectViewport(page);
  // A bounded idle interval catches onEndReached cascades without a user scroll.
  await page.waitForTimeout(400);
  expect(bridge.requests.filter((command) => command.command === "feed")).toHaveLength(1);
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await page.waitForTimeout(400);
  expect(bridge.requests.filter((command) => command.command === "feed")).toHaveLength(1);
  await page.getByRole("button", { name: "打开示例板", exact: true }).click();
  await expect(page.locator("canvas")).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect
    .poll(async () => canvasColors(await page.locator("canvas").screenshot()).chinese)
    .toBeGreaterThan(10);
  await expectViewport(page);
  await expect
    .poll(async () => canvasColors(await page.locator("canvas").screenshot()).copper)
    .toBeGreaterThan(100);
  const screenshotPath = test.info().outputPath("real-canvaskit-chinese-board.png");
  const screenshot = await page.locator("canvas").screenshot({ path: screenshotPath });
  await test
    .info()
    .attach("real-canvaskit-chinese-board", { path: screenshotPath, contentType: "image/png" });
  const initialColors = canvasColors(screenshot);
  const canvasBounds = await page.locator("canvas").boundingBox();
  if (!canvasBounds) throw new Error("Missing canvas dimensions");
  expect(initialColors.copper).toBeGreaterThan(canvasBounds.width * canvasBounds.height * 0.05);
  expect(initialColors.bottom).toBeGreaterThan(50);
  expect(initialColors.chinese).toBeGreaterThan(10);
  await page.waitForTimeout(400);
  expect(bridge.requests.filter((command) => command.command === "feed")).toHaveLength(1);
});

test("search, sort, paging, safe detail, BOM, canvas controls, pick, and return", async ({
  page,
  bridge,
}) => {
  await page.goto("/");
  await expect(page.getByRole("button", { name: /^自制开源控制板 01，/ })).toBeVisible();
  await page.getByRole("textbox", { name: "搜索开源硬件项目" }).fill("电源");
  await page.getByRole("button", { name: "搜索", exact: true }).click();
  await expect(page.getByText("“电源”的搜索结果", { exact: true })).toBeVisible();
  await page.getByRole("tab", { name: "最多喜欢", exact: true }).click();
  await expect(page.getByRole("tab", { name: "最多喜欢", exact: true })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect
    .poll(() =>
      bridge.requests.some(
        (command) =>
          command.command === "feed" &&
          command.query === "电源" &&
          command.sort === "like" &&
          command.page === 1,
      ),
    )
    .toBe(true);
  await page.getByRole("button", { name: /^自制开源控制板 20，/ }).scrollIntoViewIfNeeded();
  await expect
    .poll(() =>
      bridge.requests.some(
        (command) =>
          command.command === "feed" &&
          command.page === 2 &&
          command.query === "电源" &&
          command.sort === "like",
      ),
    )
    .toBe(true);
  const card = page.getByRole("button", { name: /^自制开源控制板 21，/ });
  await expect(card).toBeAttached();
  await card.scrollIntoViewIfNeeded();
  await expect(card).toBeVisible();
  await expectViewport(page);
  const pagesBeforeOverlay = bridge.requests.filter((command) => command.command === "feed").length;
  await card.click();
  await expect(page.getByRole("heading", { name: "自制项目说明", exact: true })).toBeVisible();
  await expect(page.getByText(unsafeMarkup, { exact: true })).toBeVisible();
  const articleImage = page.locator('img[alt="本地自制说明图片"]');
  await expect(articleImage).toBeAttached();
  expect(
    await articleImage.evaluate(async (element) => {
      const image = element as HTMLImageElement;
      await image.decode();
      return [image.naturalWidth, image.naturalHeight];
    }),
  ).toEqual([200, 100]);
  await expect
    .poll(() =>
      articleImage.evaluate((element) => {
        const bounds = element.parentElement?.getBoundingClientRect();
        return bounds ? bounds.width / bounds.height : 0;
      }),
    )
    .toBeCloseTo(2, 2);
  expect(await page.evaluate(() => "__e2eUnsafe" in window)).toBe(false);
  await expect(page.locator('a[href^="javascript:"], a[href^="data:"]')).toHaveCount(0);
  await expect(page.getByRole("link", { name: "无效脚本链接" })).toHaveCount(0);
  await expect(page.getByRole("link", { name: "无效附件" })).toHaveCount(0);
  await page.getByRole("tab", { name: "板子与 BOM (2)", exact: true }).click();
  await expect(page.getByText("中文微控制器 · QFN-32", { exact: true })).toBeVisible();
  await page.getByRole("radio", { name: "选择板子 自制扩展板", exact: true }).click();
  await expect(page.getByText("暂无物料清单", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "未公开 PCB 文件", exact: true })).toBeDisabled();
  await page.getByRole("radio", { name: "选择板子 自制主板", exact: true }).click();
  await page.getByRole("button", { name: "查看 PCB", exact: true }).click();
  const surface = page.getByRole("img", { name: /^自制中文测试 PCB，PCB 画布/ });
  await expect(page.locator("canvas")).toBeVisible();
  await expect(page.getByRole("alert")).toHaveCount(0);
  const layerControls = page.getByRole("checkbox");
  const expectedLayerOrder = [
    "顶层铜",
    "底层铜",
    "中文丝印",
    "孔洞",
    "技术参考层 A",
    "技术参考层 B",
  ];
  const layerOrder = () =>
    layerControls.evaluateAll((elements) =>
      elements.map((element) => element.getAttribute("aria-label")),
    );
  expect(await layerOrder()).toEqual(expectedLayerOrder);
  const bounds = await surface.boundingBox();
  const viewport = page.viewportSize();
  if (!bounds || !viewport) throw new Error("Missing canvas dimensions");
  await expect(
    page.getByText("拖动平移 · 双指 / 滚轮缩放 · 点选图元", { exact: true }),
  ).toHaveCount(viewport.height < 500 ? 0 : 1);
  expect(bounds.width).toBeGreaterThan(
    viewport.width >= 1024 ? viewport.width - 260 : viewport.width - 8,
  );
  expect(bounds.height).toBeGreaterThan(150);
  expect(bounds.width * bounds.height).toBeGreaterThan(viewport.width * viewport.height * 0.35);
  await expectViewport(page);
  await expect
    .poll(async () => canvasColors(await page.locator("canvas").screenshot()).copper)
    .toBeGreaterThan(bounds.width * bounds.height * 0.05);
  const fittedPixels = await page.locator("canvas").screenshot();
  const fittedCopper = canvasColors(fittedPixels).copper;
  await page.getByRole("button", { name: "放大 PCB", exact: true }).click();
  await expect(page.getByText("125%", { exact: true })).toBeVisible();
  await expect
    .poll(async () => canvasColors(await page.locator("canvas").screenshot()).copper)
    .toBeGreaterThan(fittedCopper * 1.1);
  await surface.press("+");
  await expect(page.getByText("156%", { exact: true })).toBeVisible();
  await surface.press("0");
  await expect(page.getByText("100%", { exact: true })).toBeVisible();
  const center = { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
  await page.mouse.move(center.x, center.y);
  await page.mouse.wheel(0, -100);
  await expect(page.getByText("135%", { exact: true })).toBeVisible();
  await surface.press("0");
  const touches = await page.context().newCDPSession(page);
  const picksBeforeDrag = bridge.requests.filter((command) => command.command === "pick").length;
  await touches.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [{ ...center, id: 0 }],
  });
  await touches.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [{ x: center.x + 30, y: center.y + 16, id: 0 }],
  });
  await touches.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await expect
    .poll(async () => fittedPixels.equals(await page.locator("canvas").screenshot()))
    .toBe(false);
  expect(bridge.requests.filter((command) => command.command === "pick")).toHaveLength(
    picksBeforeDrag,
  );
  await surface.tap();
  await expect(page.getByText("焊盘 · self-top-pad", { exact: true })).toBeVisible();
  const pannedPick = bridge.requests.filter((command) => command.command === "pick").at(-1);
  if (pannedPick?.command !== "pick") throw new Error("Missing panned pick request");
  expect(pannedPick.x).toBeLessThan(60);
  expect(pannedPick.y).toBeLessThan(40);
  if (viewport.height < 500) {
    await page.setViewportSize({ width: viewport.width, height: 240 });
    await expect(page.getByText("网络：GND", { exact: true })).toBeInViewport({ ratio: 1 });
    await expect(page.getByRole("button", { name: "关闭图元属性", exact: true })).toBeInViewport({
      ratio: 1,
    });
  }
  await page.getByRole("button", { name: "关闭图元属性", exact: true }).click();
  if (viewport.height < 500) await page.setViewportSize(viewport);
  await surface.press("0");
  await touches.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [
      { x: center.x - 30, y: center.y, id: 0 },
      { x: center.x + 30, y: center.y, id: 1 },
    ],
  });
  await touches.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [
      { x: center.x - 31, y: center.y, id: 0 },
      { x: center.x + 31, y: center.y, id: 1 },
    ],
  });
  await touches.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [
      { x: center.x - 46.5, y: center.y, id: 0 },
      { x: center.x + 46.5, y: center.y, id: 1 },
    ],
  });
  await touches.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await expect(page.getByText("150%", { exact: true })).toBeVisible();
  await surface.press("0");
  await touches.detach();
  await page.getByRole("checkbox", { name: "顶层铜", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: "顶层铜", exact: true })).not.toBeChecked();
  expect(await layerOrder()).toEqual(expectedLayerOrder);
  const technicalLayer = page.getByRole("checkbox", { name: "技术参考层 A", exact: true });
  await technicalLayer.click();
  await expect(technicalLayer).toBeChecked();
  expect(await layerOrder()).toEqual(expectedLayerOrder);
  await technicalLayer.click();
  await expect
    .poll(async () => canvasColors(await page.locator("canvas").screenshot()).copper)
    .toBe(0);
  await surface.tap();
  await expect(page.getByText("焊盘 · self-bottom-pad", { exact: true })).toBeVisible();
  await expect(page.getByText("网络：GND", { exact: true })).toBeVisible();
  const pick = bridge.requests.filter((command) => command.command === "pick").at(-1);
  expect(pick?.command).toBe("pick");
  if (pick?.command === "pick") {
    expect(pick.layers).not.toContain(1);
    expect(pick.x).toBeCloseTo(60, 0);
    expect(pick.y).toBeCloseTo(40, 0);
    expect(pick.tolerance).toBeGreaterThan(0);
  }
  await page.getByRole("button", { name: "关闭图元属性", exact: true }).click();
  await page.getByRole("button", { name: "返回上一页", exact: true }).click();
  await expect(page.getByRole("button", { name: "查看 PCB", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "返回上一页", exact: true }).click();
  await expect(card).toBeVisible();
  await expect(card).toBeInViewport();
  expect(bridge.requests.filter((command) => command.command === "feed")).toHaveLength(
    pagesBeforeOverlay,
  );
  await expectViewport(page);
});

test("Enter submits current native text and rapid typing supports the search button", async ({
  page,
  bridge,
}) => {
  await page.goto("/");
  const input = page.getByRole("textbox", { name: "搜索开源硬件项目" });
  await input.fill("ESP32-");
  // Emulate native control text arriving before its final change updates JS draft state.
  await input.evaluate((element) => {
    (element as HTMLInputElement).value = "ESP32-S31";
  });
  await input.press("Enter");
  await expect(input).toHaveValue("ESP32-S31");
  await expect(page.getByText("“ESP32-S31”的搜索结果", { exact: true })).toBeVisible();
  await expect
    .poll(() =>
      bridge.requests.some(
        (command) => command.command === "feed" && command.query === "ESP32-S31",
      ),
    )
    .toBe(true);
  await input.fill("");
  await input.pressSequentially("ESP32-S31-USB", { delay: 0 });
  await page.getByRole("button", { name: "搜索", exact: true }).click();
  await expect(page.getByText("“ESP32-S31-USB”的搜索结果", { exact: true })).toBeVisible();
  await expect
    .poll(() =>
      bridge.requests.some(
        (command) => command.command === "feed" && command.query === "ESP32-S31-USB",
      ),
    )
    .toBe(true);
  await page.getByRole("button", { name: "清除搜索", exact: true }).click();
  await expect(input).toHaveValue("");
  await expect(page.getByText("精选开源项目", { exact: true })).toBeVisible();
});

test("HTTP 418 is visible and the same failed page can be retried", async ({ page, bridge }) => {
  bridge.failNextFeed = true;
  await page.goto("/");
  await expect(page.getByRole("alert")).toContainText("暂时无法加载");
  await expect(page.getByRole("alert")).toContainText("本地核心返回 418");
  await expect(page.getByRole("alert")).toContainText("bridge_http");
  await page.getByRole("button", { name: "重新加载", exact: true }).click();
  await expect(page.getByRole("button", { name: /^自制开源控制板 01，/ })).toBeVisible();
  const feedRequests = bridge.requests.filter((command) => command.command === "feed");
  expect(feedRequests.slice(0, 2).map((command) => command.page)).toEqual([1, 1]);
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expectViewport(page);
});
