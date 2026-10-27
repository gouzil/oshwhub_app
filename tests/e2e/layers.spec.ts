import { expect, test } from "./fixtures";
import { canvasColors } from "./pixels";

test("switching active PCB layers changes the foreground and picking without moving the view", async ({
  page,
  bridge,
}) => {
  await page.goto("/");
  await page.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await page.getByRole("button", { name: "打开示例板", exact: true }).click();
  const surface = page.getByRole("img", { name: /^自制中文测试 PCB，PCB 画布/ });
  const canvas = page.locator("canvas");
  const top = page.getByRole("radio", { name: "查看顶层", exact: true });
  const bottom = page.getByRole("radio", { name: "查看底层", exact: true });
  await expect(canvas).toBeVisible();
  await expect(top).toBeChecked();
  await page.getByRole("button", { name: "放大 PCB", exact: true }).click();
  const bounds = await surface.boundingBox();
  const before = canvasColors(
    await canvas.screenshot({ path: test.info().outputPath("pcb-top-active.png") }),
  );
  await bottom.click();
  await expect(bottom).toBeChecked();
  await expect(top).not.toBeChecked();
  await expect
    .poll(async () => canvasColors(await canvas.screenshot()).bottom)
    .toBeGreaterThan(before.bottom * 2);
  await canvas.screenshot({ path: test.info().outputPath("pcb-bottom-active.png") });
  await expect(page.getByText("125%", { exact: true })).toBeVisible();
  expect(await surface.boundingBox()).toEqual(bounds);
  await surface.tap();
  await expect(page.getByRole("heading", { name: "焊盘 1", exact: true })).toBeVisible();
  const pick = bridge.requests.filter((command) => command.command === "pick").at(-1);
  expect(pick?.command === "pick" && pick.activeLayer).toBe(2);
  await page.getByRole("button", { name: "关闭图元属性", exact: true }).click();
  await page.getByRole("button", { name: "图层", exact: true }).click();
  const bottomLayer = page.getByRole("checkbox", { name: "底层铜", exact: true });
  await bottomLayer.click();
  await expect(bottomLayer).not.toBeChecked();
  await page.getByRole("button", { name: "关闭图层面板", exact: true }).click();
  await expect(top).toBeChecked();
  await bottom.click();
  await page.getByRole("button", { name: "图层", exact: true }).click();
  await expect(bottomLayer).toBeChecked();
  await expect(page.getByRole("checkbox", { name: "顶层铜", exact: true })).toBeChecked();
});
