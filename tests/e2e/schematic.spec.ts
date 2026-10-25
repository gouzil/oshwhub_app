import { expect, test } from "./fixtures";

test("opens the native schematic viewer and switches back to PCB", async ({ page, bridge }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  await page.getByRole("tab", { name: "板子与 BOM (2)", exact: true }).click();
  await page.getByRole("button", { name: "查看原理图", exact: true }).click();

  await expect
    .poll(() =>
      bridge.requests.some(
        (command) =>
          command.command === "schematic" &&
          command.projectId === "self-made-1" &&
          command.documentId === "schematic-main",
      ),
    )
    .toBe(true);
  const surface = page.getByRole("img", { name: /原理图 画布/ });
  await expect(surface).toBeVisible();
  await expect(page.locator("canvas")).toBeVisible();
  await expect(page.getByText("离线原理图夹具", { exact: true })).toHaveCount(0);

  await page.getByRole("button", { name: "图层", exact: true }).click();
  await page.getByRole("checkbox", { name: "连线", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: "连线", exact: true })).not.toBeChecked();
  await page.getByRole("button", { name: "关闭图层面板", exact: true }).click();
  await page.getByRole("button", { name: "放大 原理图", exact: true }).click();
  await expect(page.getByText("125%", { exact: true })).toBeVisible();

  await surface.tap();
  await expect(page.getByRole("heading", { name: "U1", exact: true })).toBeVisible();
  const pick = bridge.requests.filter((command) => command.command === "pick").at(-1);
  expect(pick?.command).toBe("pick");
  if (pick?.command === "pick") expect(pick.sceneId).toBe("self-made-schematic-scene");
  await page.getByRole("button", { name: "关闭图元属性", exact: true }).click();

  await page.getByRole("tab", { name: "二维模式", exact: true }).click();
  await expect(page.getByRole("img", { name: /PCB 画布/ })).toBeVisible();
  await expect.poll(() => bridge.requests.some((command) => command.command === "pcb")).toBe(true);
  expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
});
