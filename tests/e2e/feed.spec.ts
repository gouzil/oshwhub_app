import { expect, test } from "./fixtures";

test("search and sorting stay fixed while only the project list scrolls", async ({
  page,
  bridge,
}) => {
  await page.goto("/");
  const search = page.getByRole("textbox", { name: "搜索开源硬件项目" });
  const sort = page.getByRole("tab", { name: "为你推荐", exact: true });
  await expect(page.getByRole("button", { name: /^自制开源控制板 01，/ })).toBeVisible();
  const searchBounds = await search.boundingBox();
  const sortBounds = await sort.boundingBox();
  await page.getByTestId("project-feed").evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(page.getByRole("button", { name: /^自制开源控制板 21，/ })).toBeAttached();
  expect(bridge.requests.at(-1)).toMatchObject({ command: "feed", page: 2 });
  expect(await search.boundingBox()).toEqual(searchBounds);
  expect(await sort.boundingBox()).toEqual(sortBounds);
  await expect(search).toBeInViewport({ ratio: 1 });
  await expect(sort).toBeInViewport({ ratio: 1 });
  await page.screenshot({ path: test.info().outputPath("fixed-discover-toolbar.png") });
});

test("refresh keeps the current cards until ready and restarts filtered pagination", async ({
  page,
  bridge,
}) => {
  await page.goto("/");
  await page.getByRole("textbox", { name: "搜索开源硬件项目" }).fill("电源");
  await page.getByRole("button", { name: "搜索", exact: true }).click();
  await page.getByRole("tab", { name: "最多喜欢", exact: true }).click();
  await expect(page.getByRole("button", { name: "加载更多项目", exact: true })).toBeAttached();
  await page.getByTestId("project-feed").evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(page.getByRole("button", { name: /^自制开源控制板 21，/ })).toBeAttached();
  const refresh = page.getByRole("button", { name: "刷新项目", exact: true });
  await refresh.scrollIntoViewIfNeeded();
  let release!: () => void;
  bridge.feedGate = new Promise<void>((resolve) => {
    release = resolve;
  });
  try {
    await refresh.click();
    await expect(page.getByRole("button", { name: "正在刷新", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: /^自制开源控制板 01，/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /^自制开源控制板 21，/ })).toBeAttached();
    expect(bridge.requests.at(-1)).toMatchObject({
      command: "feed",
      query: "电源",
      sort: "like",
      page: 1,
    });
  } finally {
    release();
    bridge.feedGate = undefined;
  }
  await expect(refresh).toBeEnabled();
  await expect(page.getByRole("button", { name: /^自制开源控制板 21，/ })).toHaveCount(0);
  await expect(page.getByRole("textbox", { name: "搜索开源硬件项目" })).toHaveValue("电源");
  await page.getByTestId("project-feed").evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await expect(page.getByRole("button", { name: /^自制开源控制板 21，/ })).toBeAttached();
  await page.getByRole("button", { name: /^自制开源控制板 40，/ }).scrollIntoViewIfNeeded();
  await expect(page.getByText("已经看到全部项目了", { exact: true })).toBeVisible();
  expect(
    bridge.requests.filter((command) => command.command === "feed" && command.page === 2),
  ).toHaveLength(2);
});

test("a failed refresh preserves cards and can be retried from its visible error", async ({
  page,
  bridge,
}) => {
  await page.goto("/");
  const first = page.getByRole("button", { name: /^自制开源控制板 01，/ });
  await expect(first).toBeVisible();
  bridge.failNextFeed = true;
  await page.getByRole("button", { name: "刷新项目", exact: true }).click();
  await expect(page.getByRole("alert")).toBeInViewport();
  await expect(first).toBeVisible();
  await page.getByRole("button", { name: "重新加载", exact: true }).click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(first).toBeVisible();
  expect(
    bridge.requests.filter((command) => command.command === "feed").map((command) => command.page),
  ).toEqual([1, 1, 1]);
});
