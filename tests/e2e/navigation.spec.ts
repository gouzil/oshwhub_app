import type { Locator } from "@playwright/test";
import { expect, test } from "./fixtures";

async function scrollToEnd(target: Locator) {
  await target.evaluate((element) => {
    for (let parent = element.parentElement; parent; parent = parent.parentElement) {
      if (parent.scrollHeight > parent.clientHeight) parent.scrollTop = parent.scrollHeight;
    }
  });
}

test("selected lens keeps up when JavaScript animation frames are suspended", async ({ page }) => {
  await page.addInitScript(() => {
    window.requestAnimationFrame = () => 0;
  });
  await page.goto("/");
  const navigation = page.getByTestId("navigation-glass");
  const selected = navigation.getByRole("tab", { name: "关于", exact: true });
  await expect(navigation.getByTestId("navigation-lens")).toBeVisible();
  await selected.click();
  await expect(selected).toHaveAttribute("aria-selected", "true");
  await expect
    .poll(async () => {
      const lens = await navigation.getByTestId("navigation-lens").boundingBox();
      const target = await selected.boundingBox();
      return lens && target
        ? Math.max(Math.abs(lens.x - target.x), Math.abs(lens.y - target.y))
        : Infinity;
    })
    .toBeLessThan(1);
});

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
  const lens = navigation.getByTestId("navigation-lens");
  await expect(lens).toBeVisible();
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
  for (const label of ["PCB 实验室", "设置", "关于", "发现"]) {
    const tab = navigation.getByRole("tab", { name: label, exact: true });
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");
    const item = await tab.boundingBox();
    expect(item?.height).toBeGreaterThanOrEqual(44);
    await expect
      .poll(async () => {
        const active = await lens.boundingBox();
        const selected = await tab.boundingBox();
        if (!active || !selected) return Infinity;
        return Math.max(
          Math.abs(active.x - selected.x),
          Math.abs(active.y - selected.y),
          Math.abs(active.width - selected.width),
          Math.abs(active.height - selected.height),
        );
      })
      .toBeLessThan(1);
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
  await navigation.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await expect(page.getByRole("button", { name: "打开示例板", exact: true })).toBeVisible();
  await page.screenshot({ path: test.info().outputPath("navigation-lab.png") });
});

test("reduced transparency uses an opaque navigation surface in both appearances", async ({
  page,
  browserName,
}) => {
  test.skip(
    browserName !== "chromium",
    "Chromium exposes reduced-transparency emulation through CDP",
  );
  const session = await page.context().newCDPSession(page);
  await session.send("Emulation.setEmulatedMedia", {
    features: [{ name: "prefers-reduced-transparency", value: "reduce" }],
  });
  await page.emulateMedia({ reducedMotion: "reduce" });
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
    .toEqual({ filter: "none", background: "rgb(245, 246, 250)" });
  await navigation.getByRole("tab", { name: "PCB 实验室", exact: true }).click();
  await expect(page.getByRole("button", { name: "打开示例板", exact: true })).toBeAttached();
  const lens = navigation.getByTestId("navigation-lens");
  await expect(lens).toHaveCSS("background-color", "rgb(237, 243, 255)");
  // With reduced motion the selected lens must settle without a spring running afterward.
  const selected = navigation.getByRole("tab", { name: "PCB 实验室", exact: true });
  const activeBounds = await lens.boundingBox();
  const selectedBounds = await selected.boundingBox();
  // React Native Web measures layout with integer offsetLeft / offsetTop values.
  expect(Math.abs((activeBounds?.x ?? Infinity) - (selectedBounds?.x ?? 0))).toBeLessThanOrEqual(
    0.5,
  );
  expect(Math.abs((activeBounds?.y ?? Infinity) - (selectedBounds?.y ?? 0))).toBeLessThanOrEqual(
    0.5,
  );
  await session.send("Emulation.setEmulatedMedia", {
    features: [
      { name: "prefers-reduced-transparency", value: "reduce" },
      { name: "prefers-color-scheme", value: "dark" },
      { name: "prefers-reduced-motion", value: "reduce" },
    ],
  });
  await expect(navigation).toHaveCSS("background-color", "rgb(21, 30, 44)");
  await expect(navigation).toHaveCSS("backdrop-filter", "none");
  await expect(lens).toHaveCSS("background-color", "rgb(29, 45, 75)");
});
