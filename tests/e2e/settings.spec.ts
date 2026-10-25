import { expect, test } from "./fixtures";

test("theme colors are configured in settings and survive a reload", async ({ page, bridge }) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  const demo = page.getByRole("button", { name: "打开示例板", exact: true });
  await expect(demo).toHaveCSS("background-color", "rgb(44, 101, 232)");

  await page.getByRole("tab", { name: "设置", exact: true }).click();
  await expect(page.getByRole("heading", { name: "设置", exact: true })).toBeVisible();
  await expect(page.getByRole("radio", { name: "主题颜色 OSHWHub 蓝", exact: true })).toBeChecked();
  const purple = page.getByRole("radio", { name: "主题颜色 紫色", exact: true });
  await purple.click();
  await expect(purple).toBeChecked();
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await expect(demo).toHaveCSS("background-color", "rgb(118, 84, 209)");
  expect(bridge.requests.filter((command) => command.command === "feed")).toHaveLength(1);

  await page.reload();
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await expect(demo).toHaveCSS("background-color", "rgb(118, 84, 209)");
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  await expect(purple).toBeChecked();
  await page.getByRole("button", { name: "恢复默认颜色", exact: true }).click();
  await expect(page.getByRole("radio", { name: "主题颜色 OSHWHub 蓝", exact: true })).toBeChecked();
  await page.getByRole("radio", { name: "主题颜色 橙色", exact: true }).click();
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await expect(demo).toHaveCSS("background-color", "rgb(180, 83, 9)");
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  await page.getByRole("button", { name: "恢复默认颜色", exact: true }).click();
  await expect(page.getByRole("button", { name: "恢复默认颜色", exact: true })).toHaveCSS(
    "background-color",
    "rgb(237, 243, 255)",
  );
  await page.screenshot({ path: test.info().outputPath("theme-settings.png") });
});

test("an unknown saved color falls back to the default theme", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("oshwhub.themeColor", "removed-theme"));
  await page.goto("/");
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  await expect(page.getByRole("radio", { name: "主题颜色 OSHWHub 蓝", exact: true })).toBeChecked();
});

test("unavailable storage still allows color changes and explains that they cannot be saved", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, "localStorage", {
      get() {
        throw new DOMException("Storage is disabled", "SecurityError");
      },
    });
  });
  await page.goto("/");
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  const purple = page.getByRole("radio", { name: "主题颜色 紫色", exact: true });
  await purple.click();
  await expect(purple).toBeChecked();
  await expect(page.getByRole("alert")).toContainText("无法保存");
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await expect(page.getByRole("button", { name: "打开示例板", exact: true })).toHaveCSS(
    "background-color",
    "rgb(118, 84, 209)",
  );
});
