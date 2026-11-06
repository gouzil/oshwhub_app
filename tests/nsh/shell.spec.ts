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
