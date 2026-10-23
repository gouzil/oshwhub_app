import { join } from "node:path";
import { expect, test } from "./fixtures";

test("slow feed and detail show animated skeletons and respect reduced motion", async ({
  page,
  bridge,
}) => {
  let feedReady!: () => void;
  bridge.feedGate = new Promise((resolve) => {
    feedReady = resolve;
  });
  await page.goto("/");
  const feed = page.getByRole("progressbar", { name: "正在寻找好项目", exact: true });
  await expect(feed.getByTestId("skeleton-block").first()).toBeVisible();
  const opacity = () => feed.evaluate((element) => getComputedStyle(element).opacity);
  const first = await opacity();
  await expect.poll(opacity).not.toBe(first);
  feedReady();
  await expect(feed).toHaveCount(0);

  await page.emulateMedia({ reducedMotion: "reduce" });
  let projectReady!: () => void;
  bridge.projectGate = new Promise((resolve) => {
    projectReady = resolve;
  });
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  const detail = page.getByRole("progressbar", { name: "正在打开项目", exact: true });
  await expect(detail.getByTestId("skeleton-block").first()).toBeVisible();
  await expect
    .poll(() => detail.evaluate((element) => getComputedStyle(element).opacity))
    .toBe("1");
  await page.waitForTimeout(250);
  expect(await detail.evaluate((element) => getComputedStyle(element).opacity)).toBe("1");
  projectReady();
  await expect(page.getByRole("tab", { name: "项目介绍", exact: true })).toBeVisible();
  await expect(detail).toHaveCount(0);
});

test("article images load near the viewport with placeholders and an original link", async ({
  page,
  bridge,
}, testInfo) => {
  const path = "/__e2e__/deferred-image.png";
  bridge.projectContent = [
    ...Array.from({ length: 24 }, (_, index) => ({
      kind: "text",
      text: `分段正文 ${index + 1}。${"公开硬件的设计说明。".repeat(35)}`,
      url: null,
    })),
    { kind: "image", text: "分块示意图", url: `http://127.0.0.1:5173${path}` },
  ];
  let requests = 0;
  let imageReady!: () => void;
  const imageGate = new Promise<void>((resolve) => {
    imageReady = resolve;
  });
  await page.route(`**${path}`, async (route) => {
    requests += 1;
    await imageGate;
    await route.fulfill({
      contentType: "image/png",
      path: join(testInfo.project.testDir, "assets/article.png"),
    });
  });
  await page.goto("/");
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  await expect(page.getByRole("tab", { name: "项目介绍", exact: true })).toBeVisible();
  const article = page.getByTestId("project-content-list");
  await expect(article).toBeVisible();
  expect(requests).toBe(0);
  await expect.poll(() => article.getByText(/^分段正文 /).count()).toBeLessThan(24);
  await expect
    .poll(
      async () => {
        await article.evaluate((element) => {
          element.scrollTop += element.clientHeight;
        });
        return requests;
      },
      { timeout: 20_000, intervals: [100] },
    )
    .toBeGreaterThan(0);
  const placeholder = page.getByRole("progressbar", {
    name: "正在加载图片：分块示意图",
    exact: true,
  });
  await expect(placeholder).toBeVisible();
  imageReady();
  await expect(placeholder).toHaveCount(0);
  await expect(page.getByRole("img", { name: "分块示意图", exact: true })).toBeVisible();
  await expect(
    page.getByRole("link", { name: "查看原图：分块示意图，在浏览器中打开", exact: true }),
  ).toBeVisible();
});

test("failed thumbnails fall back to the original and can be retried independently", async ({
  page,
  bridge,
}, testInfo) => {
  let failed = true;
  const original = "https://image.lceda.cn/__e2e__/retry-image.png";
  const requests: string[] = [];
  bridge.projectContent = [{ kind: "image", text: "可重试图片", url: original }];
  await page.route("https://image.lceda.cn/__e2e__/retry-image.png*", (route) => {
    requests.push(route.request().url());
    return route.fulfill(
      failed || route.request().url() !== original
        ? { contentType: "image/png", body: "invalid image bytes" }
        : { contentType: "image/png", path: join(testInfo.project.testDir, "assets/article.png") },
    );
  });
  await page.goto("/");
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  const list = page.getByTestId("project-content-list");
  await expect(list).toBeVisible();
  await list.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  const retry = page.getByRole("button", { name: "重新加载图片：可重试图片", exact: true });
  await expect(retry).toBeVisible();
  expect(new URL(requests[0]).searchParams.has("x-oss-process")).toBe(true);
  expect(requests).toContain(original);
  failed = false;
  await retry.click();
  await expect(page.getByRole("img", { name: "可重试图片", exact: true })).toBeVisible();
  expect(bridge.requests.filter((command) => command.command === "project")).toHaveLength(1);
});

test("failed project covers can be retried without refetching the project", async ({
  page,
  bridge,
}, testInfo) => {
  let failed = true;
  bridge.projectCover = "http://127.0.0.1:5173/__e2e__/cover-retry.png";
  bridge.projectContent = [];
  await page.route("**/__e2e__/cover-retry.png", (route) =>
    route.fulfill(
      failed
        ? { contentType: "image/png", body: "invalid image bytes" }
        : { contentType: "image/png", path: join(testInfo.project.testDir, "assets/article.png") },
    ),
  );
  await page.goto("/");
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  const retry = page.getByRole("button", { name: "重新加载图片：自制开源控制板 01", exact: true });
  await expect(retry).toBeVisible();
  failed = false;
  await retry.click();
  await expect(page.getByRole("img", { name: "自制开源控制板 01", exact: true })).toBeVisible();
  expect(bridge.requests.filter((command) => command.command === "project")).toHaveLength(1);
});

test("tall article images keep a readable width and their natural aspect ratio", async ({
  page,
  bridge,
}, testInfo) => {
  bridge.projectContent = [
    { kind: "image", text: "长图说明", url: "http://127.0.0.1:5173/__e2e__/tall.png" },
  ];
  await page.route("**/__e2e__/tall.png", (route) =>
    route.fulfill({
      contentType: "image/png",
      path: join(testInfo.project.testDir, "assets/article-tall.png"),
    }),
  );
  await page.goto("/");
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  const list = page.getByTestId("project-content-list");
  await list.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  const image = page.getByRole("img", { name: "长图说明", exact: true });
  await expect(image).toBeAttached();
  await expect
    .poll(() => image.evaluate((element) => element.clientHeight / element.clientWidth))
    .toBeCloseTo(12.5, 1);
  expect(await image.evaluate((element) => element.clientWidth)).toBeGreaterThan(200);
});
