import type { Locator } from "@playwright/test";
import { expect, test } from "./fixtures";

async function scrollToEnd(target: Locator) {
  await target.evaluate((element) => {
    for (let parent = element.parentElement; parent; parent = parent.parentElement) {
      if (parent.scrollHeight > parent.clientHeight) parent.scrollTop = parent.scrollHeight;
    }
  });
}

test("glass navigation switches pages and leaves page endings reachable", async ({
  page,
  bridge,
}) => {
  await page.goto("/");
  const navigation = page.getByTestId("navigation-glass");
  await expect(navigation).toBeVisible();
  expect(
    await navigation.evaluate((element) => getComputedStyle(element).backdropFilter),
  ).toContain("blur(");
  const desktop = (page.viewportSize()?.width ?? 0) >= 1024;
  const bounds = await navigation.boundingBox();
  if (!bounds) throw new Error("Missing navigation bounds");
  if (desktop) {
    expect(bounds.x).toBeGreaterThan(0);
    expect(bounds.width).toBeLessThan(216);
  } else {
    expect(bounds.x).toBeGreaterThan(0);
    expect(bounds.y + bounds.height).toBeLessThan(page.viewportSize()?.height ?? 0);
    expect(bounds.width).toBeLessThanOrEqual(480);
  }
  for (const label of ["PCB 实验室", "关于", "发现"]) {
    const tab = navigation.getByRole("tab", { name: label, exact: true });
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");
    const item = await tab.boundingBox();
    expect(item?.height).toBeGreaterThanOrEqual(44);
  }

  // Short landscape screens must be able to scroll the last action above the dock.
  await navigation.getByRole("tab", { name: "关于", exact: true }).click();
  const ending = page.getByText("v0.1.0 · 为每一位硬件创造者而做", { exact: true });
  await scrollToEnd(ending);
  await expect(ending).toBeInViewport();
  if (!desktop) {
    await expect
      .poll(async () => (await ending.boundingBox())?.y ?? Infinity)
      .toBeLessThan(bounds.y);
    const last = await ending.boundingBox();
    expect((last?.y ?? Infinity) + (last?.height ?? 0)).toBeLessThanOrEqual(bounds.y);
  }
  await navigation.getByRole("tab", { name: "发现", exact: true }).click();
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  const footer = page.getByRole("button", { name: "查看板子与物料清单", exact: true });
  await expect(footer).toBeAttached();
  await scrollToEnd(footer);
  await expect(footer).toBeInViewport();
  if (!desktop) {
    await expect
      .poll(async () => {
        const item = await footer.boundingBox();
        return (item?.y ?? Infinity) + (item?.height ?? 0);
      })
      .toBeLessThanOrEqual(bounds.y);
  }
  await footer.click();
  await expect(page.getByText("选择板子", { exact: true })).toBeVisible();
  expect(bridge.requests.some((command) => command.command === "project")).toBe(true);
  await page.screenshot({ path: test.info().outputPath("glass-navigation.png") });
});

test("reduced transparency uses an opaque navigation surface", async ({ page, browserName }) => {
  test.skip(
    browserName !== "chromium",
    "Chromium exposes reduced-transparency emulation through CDP",
  );
  const session = await page.context().newCDPSession(page);
  await session.send("Emulation.setEmulatedMedia", {
    features: [{ name: "prefers-reduced-transparency", value: "reduce" }],
  });
  await page.goto("/");
  const navigation = page.getByTestId("navigation-glass");
  await expect(navigation).toBeVisible();
  await expect
    .poll(() =>
      navigation.evaluate((element) => {
        const style = getComputedStyle(element);
        return { filter: style.backdropFilter, background: style.backgroundColor };
      }),
    )
    .toEqual({ filter: "none", background: "rgb(246, 249, 247)" });
  await navigation.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await expect(page.getByRole("button", { name: "打开示例板", exact: true })).toBeAttached();
});
