import { expect, test } from "@playwright/test";

test("packaged Rust NSH shell keeps appearance across navigation and responsive layouts", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/app/");
  await expect(page.getByText("发现下一个好想法")).toBeVisible();
  await page.getByRole("button", { name: "打开设置", exact: true }).click();
  await page.getByRole("button", { name: "暗色模式", exact: true }).click();
  await expect(page.getByText("当前外观：暗色模式", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "打开关于", exact: true }).click();
  await expect(page.getByText("公开工程只读浏览", { exact: true })).toBeVisible();
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.getByRole("button", { name: "打开设置", exact: true }).click();
  await expect(page.getByText("当前外观：暗色模式", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "暗色模式", exact: true })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await page.screenshot({ path: "output/nsh-web-tests/shell-dark.png" });
});

test("packaged shell buttons remain keyboard operable on a short landscape viewport", async ({
  page,
}) => {
  await page.setViewportSize({ width: 844, height: 390 });
  await page.goto("/app/");
  await page.getByRole("button", { name: "打开设置", exact: true }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByText("外观", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "暗色模式", exact: true }).focus();
  await page.keyboard.press("Space");
  await expect(page.getByText("当前外观：暗色模式", { exact: true })).toBeVisible();
});

test("Tab skips retained pages and inactive viewer modes", async ({ page }) => {
  await page.route("**/fetch", (route) =>
    route.fulfill({
      json: {
        success: true,
        code: 0,
        result: { lists: [], page: 1, pageSize: 20, total: 0, totalPage: 0 },
      },
    }),
  );
  await page.goto("/app/");
  await page.getByRole("button", { name: "打开 PCB 实验室", exact: true }).click();
  await page.getByRole("button", { name: "打开示例查看器", exact: true }).click();
  await expect(page.getByTestId("pcb-canvas")).toBeVisible();
  await page.getByLabel("切换到 PCB 3D", { exact: true }).click();
  await expect(page.getByText(/个器件/)).toBeVisible();
  for (const mode of ["切换到 PCB 2D", "切换到 PCB 3D"]) {
    await page.getByLabel(mode, { exact: true }).click();
    const hiddenStops: string[] = [];
    for (let index = 0; index < 40; index++) {
      await page.keyboard.press("Tab");
      const hidden = await page.evaluate(() => {
        const el = document.activeElement;
        if (!el || !el.closest('[aria-hidden="true"]')) return null;
        return el.getAttribute("aria-label") || el.getAttribute("placeholder") || el.tagName;
      });
      if (hidden) hiddenStops.push(hidden);
    }
    expect(hiddenStops).toEqual([]);
  }
});
