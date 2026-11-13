import { expect, test } from "@playwright/test";

const row = (id: string, name: string) => ({
  uuid: id.repeat(32),
  name,
  path: `creator/${name}`,
  owner: { nickname: "开源作者" },
  introduction: "开源硬件说明",
  count: { like: 4, views: 12 },
  tags: ["电源"],
});

test("Rust discovery submits, sorts and clears real typed feed requests", async ({ page }) => {
  const requests: { url: string; body: null | { keywords: string; sort: string } }[] = [];
  await page.route("**/fetch", async (route) => {
    const request = route.request().postDataJSON();
    requests.push(request);
    const name = request.body?.keywords || "精选项目";
    await route.fulfill({
      json: {
        success: true,
        code: 0,
        result: {
          lists: [row("a", name)],
          page: 1,
          pageSize: 20,
          totalPage: 1,
          total: 1,
        },
      },
    });
  });
  await page.goto("/app/");
  await expect(page.getByRole("button", { name: "打开项目：精选项目" })).toBeVisible();
  const search = page.getByPlaceholder("搜索项目、开发板、灵感…");
  await search.fill("电源");
  await search.press("Enter");
  await expect(page.getByRole("button", { name: "打开项目：电源" })).toBeVisible();
  await page.getByRole("button", { name: "最多喜欢", exact: true }).click();
  await expect.poll(() => requests.at(-1)?.body).toMatchObject({ keywords: "电源", sort: "like" });
  await page.getByRole("button", { name: "打开设置", exact: true }).click();
  await expect(page.getByText("按你的习惯浏览开源硬件", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "打开发现项目", exact: true }).click();
  await expect(search).toHaveValue("电源");
  await page.getByRole("button", { name: "清除搜索", exact: true }).click();
  await expect(search).toHaveValue("");
  await expect(page.getByRole("button", { name: "打开项目：精选项目" })).toBeVisible();
  expect(new URL(requests.at(-1)?.url ?? "").searchParams.get("type")).toBe("like");
});

test("Rust discovery exposes bounded service errors and retry", async ({ page }) => {
  let blocked = true;
  await page.route("**/fetch", async (route) => {
    if (blocked) {
      await route.fulfill({
        status: 400,
        json: { code: "BLOCKED", message: "访问受限，请稍后再试" },
      });
    } else {
      await route.fulfill({
        json: {
          success: true,
          code: 0,
          result: {
            lists: [],
            page: 1,
            pageSize: 20,
            totalPage: 0,
            total: 0,
          },
        },
      });
    }
  });
  await page.goto("/app/");
  await expect(page.getByText("访问受限，请稍后再试", { exact: true })).toBeVisible();
  blocked = false;
  await page.getByRole("button", { name: "重试加载项目", exact: true }).click();
  await expect(page.getByText("还没有找到相关项目", { exact: true })).toBeVisible();
});
