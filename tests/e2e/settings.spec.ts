import { expect, test } from "./fixtures";

test.use({ colorScheme: "light" });

test("appearance follows the system until a manual mode is selected", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  const system = page.getByRole("radio", { name: "外观 跟随系统", exact: true });
  const light = page.getByRole("radio", { name: "外观 亮色模式", exact: true });
  const dark = page.getByRole("radio", { name: "外观 暗色模式", exact: true });
  const heading = page.getByRole("heading", { name: "设置", exact: true });
  const appearance = page.getByRole("heading", { name: "外观", exact: true }).locator("..");
  await expect(system).toBeChecked();
  await expect(page.locator("html")).toHaveCSS("color-scheme", "dark");
  await expect(heading).toHaveCSS("color", "rgb(230, 237, 247)");
  await expect(appearance).toHaveCSS("background-color", "rgb(24, 34, 48)");

  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).toHaveCSS("color-scheme", "light");
  await expect(heading).toHaveCSS("color", "rgb(32, 45, 64)");
  await expect(appearance).toHaveCSS("background-color", "rgb(255, 255, 255)");
  await dark.click();
  await expect(dark).toBeChecked();
  await expect(page.locator("html")).toHaveCSS("color-scheme", "dark");
  await expect(heading).toHaveCSS("color", "rgb(230, 237, 247)");
  await page.emulateMedia({ colorScheme: "dark" });
  await page.emulateMedia({ colorScheme: "light" });
  await expect(dark).toBeChecked();
  await expect(heading).toHaveCSS("color", "rgb(230, 237, 247)");

  await light.click();
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(light).toBeChecked();
  await expect(page.locator("html")).toHaveCSS("color-scheme", "light");
  await expect(heading).toHaveCSS("color", "rgb(32, 45, 64)");
  await system.click();
  await expect(heading).toHaveCSS("color", "rgb(230, 237, 247)");
  await page.screenshot({ path: test.info().outputPath("dark-settings.png") });
});

test("saved appearance and accent stay independent across reloads and mode changes", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  await page.getByRole("radio", { name: "外观 暗色模式", exact: true }).click();
  await page.getByRole("radio", { name: "主题颜色 紫色", exact: true }).click();
  await page.reload();
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  await expect(page.getByRole("radio", { name: "外观 暗色模式", exact: true })).toBeChecked();
  await expect(page.getByRole("radio", { name: "主题颜色 紫色", exact: true })).toBeChecked();
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  const demo = page.getByRole("button", { name: "打开示例板", exact: true });
  await expect(demo).toHaveCSS("background-color", "rgb(191, 165, 255)");
  await expect(demo.getByText("打开示例板", { exact: true })).toHaveCSS("color", "rgb(15, 23, 36)");

  await page.getByRole("tab", { name: "设置", exact: true }).click();
  await page.getByRole("radio", { name: "外观 亮色模式", exact: true }).click();
  await page.reload();
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  await expect(page.getByRole("radio", { name: "外观 亮色模式", exact: true })).toBeChecked();
  await expect(page.getByRole("radio", { name: "主题颜色 紫色", exact: true })).toBeChecked();
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await expect(demo).toHaveCSS("background-color", "rgb(118, 84, 209)");
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  await page.getByRole("radio", { name: "外观 跟随系统", exact: true }).click();
  await page.reload();
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  await expect(page.getByRole("radio", { name: "外观 跟随系统", exact: true })).toBeChecked();
  await expect(page.getByRole("radio", { name: "主题颜色 紫色", exact: true })).toBeChecked();
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await expect(demo).toHaveCSS("background-color", "rgb(191, 165, 255)");
});

test("an unknown saved appearance falls back to following the system", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("oshwhub.themeMode", "removed-mode"));
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await page.getByRole("tab", { name: "设置", exact: true }).click();
  await expect(page.getByRole("radio", { name: "外观 跟随系统", exact: true })).toBeChecked();
  await expect(page.getByRole("heading", { name: "设置", exact: true })).toHaveCSS(
    "color",
    "rgb(230, 237, 247)",
  );
});

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

test("unavailable storage still applies appearance and color changes with a save warning", async ({
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
  const dark = page.getByRole("radio", { name: "外观 暗色模式", exact: true });
  await dark.click();
  await expect(dark).toBeChecked();
  await expect(page.getByRole("alert").filter({ hasText: "外观已应用" })).toContainText("无法保存");
  const purple = page.getByRole("radio", { name: "主题颜色 紫色", exact: true });
  await purple.click();
  await expect(purple).toBeChecked();
  await expect(page.getByRole("alert").filter({ hasText: "颜色已应用" })).toContainText("无法保存");
  await expect(page.getByRole("alert").filter({ hasText: "外观已应用" })).toContainText("无法保存");
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await expect(page.getByRole("button", { name: "打开示例板", exact: true })).toHaveCSS(
    "background-color",
    "rgb(191, 165, 255)",
  );
});
