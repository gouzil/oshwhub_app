import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { _electron, expect } from "@playwright/test";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const executablePath = process.argv[2] ? path.resolve(process.argv[2]) : undefined;
const userData = await mkdtemp(path.join(os.tmpdir(), "oshwhub-desktop-smoke-"));
const output = path.join(root, ".work/desktop-smoke");
await mkdir(output, { recursive: true });
const env = { ...process.env, OSHWHUB_RENDERER_URL: "" };
delete env.ELECTRON_RUN_AS_NODE;
let desktop;
let page;
const errors = [];
const requests = [];
try {
  desktop = await _electron.launch({
    executablePath,
    cwd: path.join(root, "apps/desktop"),
    args: [...(executablePath ? [] : ["."]), `--user-data-dir=${userData}`],
    env,
    timeout: 30_000,
  });
  page = await desktop.firstWindow();
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  page.on("request", (request) => requests.push(request.url()));
  await page.route(/^https?:\/\//, (route) => {
    if (route.request().resourceType() === "image") {
      return route.fulfill({
        contentType: "image/png",
        body: Buffer.from(
          "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
          "base64",
        ),
      });
    }
    return route.abort();
  });
  await expect(page.getByRole("tab", { name: "PCB 实验室", exact: true })).toBeVisible({
    timeout: 30_000,
  });
  assert.equal(page.url(), "oshwhub://app/");
  const health = await page.evaluate(async () => {
    const bridge = window.oshwhubDesktop;
    if (!bridge) throw new Error("Missing desktop preload API");
    return {
      keys: Object.keys(bridge).sort(),
      node: typeof window.require,
      response: JSON.parse(await bridge.dispatch(JSON.stringify({ command: "health" }))),
    };
  });
  assert.deepEqual(health.keys, ["dispatch", "openExternal"]);
  assert.equal(health.node, "undefined");
  assert.equal(health.response.ok, true);
  assert.equal(health.response.data.protocol, 1);
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  const openDemo = page.getByRole("button", { name: "打开示例板", exact: true });
  await expect(openDemo).toHaveCSS("background-color", "rgb(44, 101, 232)");
  assert.ok((await openDemo.boundingBox()).height >= 44);
  await page.screenshot({ path: path.join(output, "lab.png") });
  await openDemo.click();
  await expect(page.locator("canvas").first()).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole("button", { name: "放大 PCB", exact: true })).toBeEnabled();
  await page.getByRole("button", { name: "放大 PCB", exact: true }).click();
  await expect(page.getByText("125%", { exact: true })).toBeVisible();
  await page.screenshot({ path: path.join(output, "pcb-2d.png") });
  await page.getByRole("tab", { name: "三维模式", exact: true }).click();
  await expect(page.getByTestId("pcb-3d-surface")).toBeVisible();
  await expect(page.getByText("3D 视图已就绪")).toBeVisible({ timeout: 30_000 });
  await page.mouse.move(500, 500);
  await expect(page.getByRole("tab", { name: "三维模式", exact: true })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page.getByRole("tab", { name: "三维模式", exact: true })).toHaveCSS(
    "background-color",
    "rgb(44, 101, 232)",
  );
  await expect(page.getByRole("tab", { name: "二维模式", exact: true })).toHaveCSS(
    "background-color",
    "rgba(0, 0, 0, 0)",
  );
  await page.screenshot({ path: path.join(output, "pcb-3d.png") });
  const fontFiles = await desktop.evaluate(({ app }) => {
    const fs = process.getBuiltinModule("node:fs");
    const path = process.getBuiltinModule("node:path");
    return fs
      .readdirSync(path.join(app.getAppPath(), "dist/assets"))
      .filter((name) => name.endsWith(".ttf"));
  });
  assert.equal(fontFiles.length, 1);
  const font = await page.evaluate(async (filename) => {
    const response = await fetch(new URL(`/assets/${filename}`, location.href));
    const bytes = new Uint8Array(await response.arrayBuffer());
    return { ok: response.ok, size: bytes.length, header: Array.from(bytes.slice(0, 4)) };
  }, fontFiles[0]);
  assert.equal(font.ok, true);
  assert.ok(font.size > 1_000_000, "Bundled font served by production protocol");
  assert.deepEqual(font.header, [0, 1, 0, 0]);
  assert.ok(
    !requests.some((url) => /^http:\/\/127\.0\.0\.1:4318/.test(url)),
    "No browser development bridge",
  );
  assert.deepEqual(errors, []);
  console.log(
    "Electron production-origin smoke passed: isolated preload, Rust health/demo, Tailwind controls, local fonts, PCB 2D/3D; remote renderer code requests blocked.",
  );
} catch (error) {
  if (page) {
    console.error(
      JSON.stringify(
        {
          errors,
          requests,
          tabs: await page.getByRole("tab").evaluateAll((nodes) =>
            nodes.map((node) => ({
              label: node.getAttribute("aria-label"),
              selected: node.getAttribute("aria-selected"),
              background: getComputedStyle(node).backgroundColor,
            })),
          ),
          body: await page.locator("body").innerText(),
          gpu: await desktop.evaluate(({ app }) => app.getGPUFeatureStatus()),
        },
        null,
        2,
      ),
    );
    await page.screenshot({ path: path.join(output, "failure.png") });
  }
  throw error;
} finally {
  await desktop?.close();
  await rm(userData, { recursive: true, force: true });
}
