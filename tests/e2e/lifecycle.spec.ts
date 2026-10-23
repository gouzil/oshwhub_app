import { expect, scene, test } from "./fixtures";
import { canvasColors } from "./pixels";

type PictureGc = { created: number; finalized: number; images: number; releasedImages: number };
declare global {
  interface Window {
    __pictureGc: PictureGc;
    __strictViewerRoot?: { unmount: () => void };
  }
}

test("SkPicture and raster image ownership survive StrictMode, Fast Refresh, and unmount", async ({
  page,
  bridge,
}) => {
  let viewerUrl = "";
  page.on("request", (request) => {
    if (
      !viewerUrl &&
      new URL(request.url()).pathname.endsWith("/packages/pcb-viewer/src/PcbViewer.tsx")
    )
      viewerUrl = request.url();
  });
  await page.addInitScript(() => {
    const probe = window;
    probe.__pictureGc = { created: 0, finalized: 0, images: 0, releasedImages: 0 };
    const handleName = (value: unknown) =>
      (value as { Fd?: { ae?: { name?: string } } } | null)?.Fd?.ae?.name;
    globalThis.FinalizationRegistry = new Proxy(globalThis.FinalizationRegistry, {
      construct(target, [cleanup]) {
        const registry = new target((value: unknown) => {
          if (handleName(value) === "sk_sp<Picture>") probe.__pictureGc.finalized++;
          if (handleName(value) === "sk_sp<Image>") probe.__pictureGc.releasedImages++;
          cleanup(value);
        });
        const register = registry.register.bind(registry);
        registry.register = (target, value, token) => {
          if (handleName(value) === "sk_sp<Picture>") probe.__pictureGc.created++;
          if (handleName(value) === "sk_sp<Image>") probe.__pictureGc.images++;
          register(target, value, token);
        };
        return registry;
      },
    });
  });
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await page.getByRole("button", { name: "打开示例板", exact: true }).click();
  const mainCanvas = page.locator("canvas").first();
  await expect
    .poll(async () => canvasColors(await mainCanvas.screenshot()).chinese)
    .toBeGreaterThan(10);
  expect(bridge.requests.some((command) => command.command === "demo")).toBe(true);
  expect(viewerUrl).toBeTruthy();

  await page.evaluate(
    async ({ url, scene }) => {
      const reactUrl = "/node_modules/.vite/deps/react.js";
      const domUrl = "/node_modules/.vite/deps/react-dom_client.js";
      const React = (await import(reactUrl)).default;
      const { createRoot } = (await import(domUrl)).default;
      const { PcbViewer } = await import(url);
      const host = document.createElement("div");
      host.id = "strict-viewer";
      Object.assign(host.style, {
        position: "fixed",
        inset: "0",
        display: "flex",
        background: "#fff",
        zIndex: "10000",
      });
      document.body.append(host);
      const root = createRoot(host);
      window.__strictViewerRoot = root;
      root.render(
        React.createElement(
          React.StrictMode,
          null,
          React.createElement(PcbViewer, { scene: { ...scene, texts: [] } }),
        ),
      );
    },
    { url: viewerUrl, scene },
  );
  const strictCanvas = page.locator("#strict-viewer canvas");
  await expect
    .poll(async () => canvasColors(await strictCanvas.screenshot()).copper)
    .toBeGreaterThan(100);
  for (let index = 0; index < 3; index++) {
    const created = await page.evaluate(() => window.__pictureGc.created);
    viewerUrl = await page.evaluate(
      async ({ url, index }) => {
        const previous = await import(url);
        const nextUrl = new URL(url);
        nextUrl.searchParams.set("t", String(Date.now() + index));
        const next = await import(nextUrl.href);
        const refreshUrl = "/@react-refresh";
        const refresh = await import(refreshUrl);
        const error = refresh.validateRefreshBoundaryAndEnqueueUpdate(url, previous, next);
        if (error) throw new Error(error);
        return nextUrl.href;
      },
      { url: viewerUrl, index },
    );
    await expect
      .poll(() => page.evaluate(() => window.__pictureGc.created))
      .toBeGreaterThan(created);
    await expect
      .poll(async () => canvasColors(await strictCanvas.screenshot()).copper)
      .toBeGreaterThan(100);
  }
  await page.evaluate(() => {
    const probe = window;
    probe.__strictViewerRoot?.unmount();
    delete probe.__strictViewerRoot;
    document.querySelector("#strict-viewer")?.remove();
  });
  await expect(strictCanvas).toHaveCount(0);
  await expect
    .poll(async () => canvasColors(await mainCanvas.screenshot()).chinese)
    .toBeGreaterThan(10);
  const cdp = await page.context().newCDPSession(page);
  await expect
    .poll(async () => {
      await cdp.send("HeapProfiler.collectGarbage");
      return page.evaluate(() => window.__pictureGc.finalized);
    })
    .toBeGreaterThanOrEqual(scene.layers.length);
  await expect
    .poll(async () => {
      await cdp.send("HeapProfiler.collectGarbage");
      return page.evaluate(() => window.__pictureGc.releasedImages);
    })
    .toBeGreaterThanOrEqual(3);
  await test.info().attach("picture-gc", {
    body: JSON.stringify(await page.evaluate(() => window.__pictureGc)),
    contentType: "application/json",
  });
  await cdp.detach();
});
