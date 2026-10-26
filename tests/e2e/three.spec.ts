import { expect, test } from "./fixtures";

test("project 2D and 3D switches reuse the loaded PCB scene", async ({ page, bridge }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  await page.getByRole("tab", { name: "板子与 BOM (2)", exact: true }).click();
  await page.getByRole("button", { name: "查看 PCB", exact: true }).click();
  await page.getByRole("button", { name: "放大 PCB", exact: true }).click();
  await expect(page.getByText("125%", { exact: true })).toBeVisible();
  const original = await page.locator("canvas").first().elementHandle();

  await page.getByRole("tab", { name: "三维模式", exact: true }).click();
  await expect(page.getByText("3D 视图已就绪")).toBeVisible();
  expect(bridge.requests.filter((r) => r.command === "pcb")).toHaveLength(1);
  expect(bridge.requests.filter((r) => r.command === "pcb3d")).toHaveLength(1);

  await page.getByRole("tab", { name: "二维模式", exact: true }).click();
  await expect(page.getByText("125%", { exact: true })).toBeVisible();
  expect(await original?.evaluate((element) => element.isConnected)).toBe(true);
  await page.getByRole("tab", { name: "三维模式", exact: true }).click();
  await expect(page.getByText("3D 视图已就绪")).toBeVisible();
  expect(bridge.requests.filter((r) => r.command === "pcb")).toHaveLength(1);
  expect(bridge.requests.filter((r) => r.command === "pcb3d")).toHaveLength(1);
});

test("3D orbits without rebuilding the board and preserves the 2D camera and layers", async ({
  page,
  bridge,
}) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室" }).click();
  await page.getByRole("button", { name: "打开示例板" }).click();
  await page.getByRole("button", { name: "放大 PCB" }).click();
  const zoom = "125%";
  await expect(page.getByText(zoom, { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "图层", exact: true }).click();
  await page.getByRole("checkbox", { name: "顶层铜", exact: true }).click();
  await page.getByRole("button", { name: "关闭图层面板", exact: true }).click();
  const original = await page.locator("canvas").first().elementHandle();
  await page.getByRole("tab", { name: "三维模式" }).click();
  const canvas = page.getByTestId("pcb-3d-surface");
  await expect(canvas).toBeVisible();
  await expect(page.getByText("3D 视图已就绪")).toBeVisible();
  expect(
    await page
      .getByRole("button", { name: "放大 PCB", includeHidden: true })
      .evaluate((element) => Boolean(element.closest("[inert]"))),
  ).toBe(true);
  const before = await canvas.screenshot();
  const rect = await canvas.boundingBox();
  if (!rect) throw new Error("Missing 3D canvas bounds");
  await page.mouse.move(rect.x + rect.width * 0.45, rect.y + rect.height * 0.45);
  await page.mouse.down();
  await page.mouse.move(rect.x + rect.width * 0.65, rect.y + rect.height * 0.6, { steps: 12 });
  await page.mouse.up();
  await expect.poll(async () => Buffer.compare(before, await canvas.screenshot())).not.toBe(0);
  expect(bridge.requests.filter((r) => r.command === "pcb3d")).toHaveLength(1);
  await page.getByRole("tab", { name: "二维模式" }).click();
  expect(
    await page
      .getByRole("button", { name: "放大三维 PCB", includeHidden: true })
      .evaluate((element) => Boolean(element.closest("[inert]"))),
  ).toBe(true);
  await expect(page.getByText(zoom, { exact: true })).toBeVisible();
  expect(await original?.evaluate((element) => element.isConnected)).toBe(true);
  await page.getByRole("button", { name: "图层", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: "顶层铜", exact: true })).not.toBeChecked();
  await page.getByRole("button", { name: "关闭图层面板", exact: true }).click();
  await page.getByRole("tab", { name: "三维模式" }).click();
  await expect(canvas).toBeVisible();
  expect(bridge.requests.filter((r) => r.command === "pcb3d")).toHaveLength(1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
  await test.info().attach("3d-view", { body: await page.screenshot(), contentType: "image/png" });
});

test("board stays interactive during model loading and a failed model can be retried", async ({
  page,
  bridge,
}) => {
  let release = () => {};
  bridge.modelGate = new Promise<void>((resolve) => {
    release = resolve;
  });
  bridge.failModels = true;
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室" }).click();
  await page.getByRole("button", { name: "打开示例板" }).click();
  await page.getByRole("tab", { name: "三维模式" }).click();
  const surface = page.getByTestId("pcb-3d-surface");
  await expect(page.getByText("加载器件 0 / 1", { exact: true })).toBeVisible();
  const before = await surface.screenshot();
  await page.getByRole("button", { name: "放大三维 PCB" }).click();
  await expect.poll(async () => Buffer.compare(before, await surface.screenshot())).not.toBe(0);
  release();
  await expect(page.getByText("1 类模型加载失败，可在信息中重试")).toBeVisible();
  await page.getByRole("checkbox", { name: "三维查看信息" }).click();
  await expect(page.getByText("未绑定模型的器件暂不显示实体。")).toBeVisible();
  bridge.failModels = false;
  await page.getByRole("button", { name: "重试失败的器件模型" }).click();
  await expect(page.getByText("已显示 1 / 2 个器件", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "关闭三维信息" }).click();
  expect(bridge.requests.filter((r) => r.command === "pcb3d")).toHaveLength(1);
  expect(bridge.requests.filter((r) => r.command === "model3d")).toHaveLength(2);
  const withPart = await surface.screenshot();
  await page.getByRole("checkbox", { name: "显示器件模型" }).click();
  await expect.poll(async () => Buffer.compare(withPart, await surface.screenshot())).not.toBe(0);
  await page.getByRole("button", { name: "查看 PCB 底面" }).click();
  await expect(surface).toBeVisible();
});

test("3D resources are released and stale model responses are ignored after leaving", async ({
  page,
  bridge,
}) => {
  await page.addInitScript(() => {
    const originalCreate = WebGLRenderingContext.prototype.createBuffer;
    const originalDelete = WebGLRenderingContext.prototype.deleteBuffer;
    const originalDraw = WebGLRenderingContext.prototype.drawArrays;
    const originalDrawElements = WebGLRenderingContext.prototype.drawElements;
    const originalGetExtension = WebGLRenderingContext.prototype.getExtension;
    const probe = { created: 0, deleted: 0, drawn: 0 };
    Object.assign(window, { __threeGpu: probe });
    WebGLRenderingContext.prototype.createBuffer = function () {
      const b = originalCreate.call(this);
      if (b) probe.created++;
      return b;
    };
    WebGLRenderingContext.prototype.deleteBuffer = function (buffer) {
      if (buffer) probe.deleted++;
      originalDelete.call(this, buffer);
    };
    WebGLRenderingContext.prototype.drawArrays = function (...args) {
      probe.drawn++;
      originalDraw.apply(this, args);
    };
    WebGLRenderingContext.prototype.drawElements = function (...args) {
      probe.drawn++;
      originalDrawElements.apply(this, args);
    };
    WebGLRenderingContext.prototype.getExtension = function (name) {
      const extension = originalGetExtension.call(this, name);
      if (name !== "ANGLE_instanced_arrays" || !extension) return extension;
      const instanced = extension as ANGLE_instanced_arrays;
      const drawArrays = instanced.drawArraysInstancedANGLE;
      const drawElements = instanced.drawElementsInstancedANGLE;
      instanced.drawArraysInstancedANGLE = function (...args) {
        probe.drawn++;
        return drawArrays.apply(this, args);
      };
      instanced.drawElementsInstancedANGLE = function (...args) {
        probe.drawn++;
        return drawElements.apply(this, args);
      };
      return instanced;
    };
  });
  const gpu = () =>
    page.evaluate(
      () =>
        (window as unknown as { __threeGpu: { created: number; deleted: number; drawn: number } })
          .__threeGpu,
    );
  let release = () => {};
  bridge.modelGate = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室" }).click();
  await page.getByRole("button", { name: "打开示例板" }).click();
  await page.getByRole("tab", { name: "三维模式" }).click();
  await expect(page.getByText("加载器件 0 / 1", { exact: true })).toBeVisible();
  await expect.poll(async () => (await gpu()).drawn).toBeGreaterThan(0);
  const loaded = await gpu();
  await page.getByRole("tab", { name: "二维模式" }).click();
  // Advancing several display frames must not schedule an idle or hidden 3D loop.
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        let n = 0;
        const next = () => {
          if (++n === 12) resolve();
          else requestAnimationFrame(next);
        };
        requestAnimationFrame(next);
      }),
  );
  expect((await gpu()).drawn).toBe(loaded.drawn);
  await page.getByRole("button", { name: "返回上一页" }).click();
  await expect.poll(async () => (await gpu()).deleted).toBe(loaded.created);
  release();
  await expect(page.getByTestId("pcb-3d-surface")).toHaveCount(0);
  expect((await gpu()).created).toBe(loaded.created);
});

test("WebGL context loss is recoverable while 2D remains available", async ({ page, bridge }) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室" }).click();
  await page.getByRole("button", { name: "打开示例板" }).click();
  await page.getByRole("tab", { name: "三维模式" }).click();
  await expect(page.getByText("3D 视图已就绪")).toBeVisible();
  expect(bridge.requests.some((r) => r.command === "pcb3d")).toBe(true);
  await page
    .getByTestId("pcb-3d-surface")
    .locator("canvas")
    .evaluate((canvas) => {
      (canvas as HTMLCanvasElement)
        .getContext("webgl")
        ?.getExtension("WEBGL_lose_context")
        ?.loseContext();
    });
  await expect(page.getByRole("alert")).toContainText("上下文已丢失");
  await page.getByRole("tab", { name: "二维模式" }).click();
  await expect(page.getByRole("button", { name: "放大 PCB" })).toBeVisible();
  await page.getByRole("tab", { name: "三维模式" }).click();
  await page.getByRole("button", { name: "重新载入三维视图" }).click();
  await expect(page.getByText("3D 视图已就绪")).toBeVisible();
});

test("two-finger pan and pinch work and resizing keeps the same 3D scene", async ({
  page,
  bridge,
}) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室" }).click();
  await page.getByRole("button", { name: "打开示例板" }).click();
  await page.getByRole("tab", { name: "三维模式" }).click();
  await expect(page.getByText("3D 视图已就绪")).toBeVisible();
  const surface = page.getByTestId("pcb-3d-surface");
  const before = await surface.screenshot();
  const bounds = await surface.boundingBox();
  if (!bounds) throw new Error("Missing 3D bounds");
  const center = { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
  const touch = await page.context().newCDPSession(page);
  await touch.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: [
      { x: center.x - 20, y: center.y, id: 0 },
      { x: center.x + 20, y: center.y, id: 1 },
    ],
  });
  await touch.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [
      { x: center.x - 21, y: center.y, id: 0 },
      { x: center.x + 21, y: center.y, id: 1 },
    ],
  });
  await touch.send("Input.dispatchTouchEvent", {
    type: "touchMove",
    touchPoints: [
      { x: center.x - 40, y: center.y + 15, id: 0 },
      { x: center.x + 60, y: center.y + 15, id: 1 },
    ],
  });
  await touch.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await touch.detach();
  await expect.poll(async () => Buffer.compare(before, await surface.screenshot())).not.toBe(0);
  await page.setViewportSize({ width: 660, height: 420 });
  await page.getByRole("button", { name: "适应三维窗口" }).click();
  await expect(surface).toBeInViewport({ ratio: 1 });
  await expect(page.getByRole("alert")).toHaveCount(0);
  expect(bridge.requests.filter((r) => r.command === "pcb3d")).toHaveLength(1);
});
