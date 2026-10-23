import { expect, test } from "./fixtures";

test("BOM keeps each value attached to its field and preserves missing values", async ({
  page,
  bridge,
}) => {
  bridge.projectContent = [];
  await page.goto("/");
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  await page.getByRole("tab", { name: "板子与 BOM (2)", exact: true }).click();
  const first = page.getByTestId("bom-item-0");
  const fields = [
    ["name", "名称 / 参数", "中文微控制器"],
    ["designator", "位号", "U1"],
    ["footprint", "封装", "QFN-32"],
    ["quantity", "数量", "1"],
    ["partNumber", "制造商型号", "SELF-MCU-001"],
    ["manufacturer", "制造商", "自制半导体"],
    ["supplier", "供应商", "测试供应商"],
    ["supplierPart", "供应商料号", "TEST-001"],
  ];
  for (const [key, label, value] of fields) {
    const field = first.getByTestId(`bom-field-${key}`);
    await expect(field).toHaveText(new RegExp(value));
    await expect(field).toHaveAccessibleName(`${label}：${value}`);
  }
  for (const key of ["partNumber", "manufacturer", "supplier", "supplierPart"]) {
    await expect(page.getByTestId("bom-item-1").getByTestId(`bom-field-${key}`)).toContainText(
      "未提供",
    );
  }
  const width = page.viewportSize()?.width ?? 0;
  if (width < 720) {
    for (const [, label] of fields) {
      await expect(first.getByText(label, { exact: true })).toBeVisible();
    }
  } else {
    const table = page.getByRole("table", { name: "物料清单" });
    for (const [, label] of fields) {
      await expect(table.getByRole("columnheader", { name: label, exact: true })).toBeAttached();
    }
    await expect(table.getByRole("row")).toHaveCount(3);
    const scroller = page.getByTestId("bom-table-scroll");
    await scroller.scrollIntoViewIfNeeded();
    if (width >= 1400) {
      await expect(
        table.getByRole("columnheader", { name: "供应商料号", exact: true }),
      ).toBeInViewport({ ratio: 1 });
    } else {
      await scroller.focus();
      await expect
        .poll(async () => {
          await scroller.press("ArrowRight");
          return scroller.evaluate((element) => element.scrollLeft);
        })
        .toBeGreaterThan(0);
      await scroller.evaluate((element) => element.scrollTo({ left: 0 }));
    }
  }
  await expect(page.getByText("2 种物料 · 3 个元件", { exact: true })).toBeVisible();
  const size = await page.evaluate(() => ({
    width: document.documentElement.scrollWidth,
    viewport: window.innerWidth,
  }));
  expect(size.width).toBeLessThanOrEqual(size.viewport + 1);
  const screenshot = test.info().outputPath("bom-labelled.png");
  await page.getByTestId("bom-list").screenshot({ path: screenshot });
  await test.info().attach("labelled-bom", { path: screenshot, contentType: "image/png" });
  await page.getByRole("radio", { name: "选择板子 自制扩展板", exact: true }).click();
  await expect(page.getByText("暂无物料清单", { exact: true })).toBeVisible();
  await expect(page.getByTestId("bom-list")).toHaveCount(0);
});

test("long BOM values wrap completely and zero quantity survives layout changes", async ({
  page,
  bridge,
}) => {
  const designators = Array.from({ length: 60 }, (_, index) => `C${index + 1}`).join(",");
  const partNumber = "LONG-MANUFACTURER-PART-".repeat(6);
  bridge.projectContent = [];
  bridge.projectBom = [
    {
      name: "100nF",
      designator: designators,
      footprint: "CAP-SMD_L1.0-W0.5",
      quantity: 0,
      partNumber,
      manufacturer: "测试制造商",
      supplier: "",
      supplierPart: "",
    },
  ];
  await page.goto("/");
  await page.getByRole("button", { name: /^自制开源控制板 01，/ }).click();
  await page.getByRole("tab", { name: "板子与 BOM (2)", exact: true }).click();
  for (const [key, text] of [
    ["designator", designators],
    ["partNumber", partNumber],
  ]) {
    const value = page.getByTestId(`bom-field-${key}`).getByText(text, { exact: true });
    await expect(value).toBeVisible();
    const bounds = await value.evaluate((element) => ({
      scrollWidth: element.scrollWidth,
      width: element.clientWidth,
      height: element.getBoundingClientRect().height,
    }));
    expect(bounds.scrollWidth).toBeLessThanOrEqual(bounds.width + 1);
    expect(bounds.height).toBeGreaterThan(44);
  }
  await expect(page.getByTestId("bom-field-quantity")).toHaveAccessibleName("数量：0");
  const width = page.viewportSize()?.width ?? 0;
  await page.setViewportSize({ width: width < 720 ? 1024 : 390, height: 844 });
  await expect(page.getByRole("table", { name: "物料清单" })).toHaveCount(width < 720 ? 1 : 0);
  await expect(page.getByTestId("bom-field-quantity")).toHaveAccessibleName("数量：0");
  await expect(page.getByTestId("bom-field-partNumber")).toContainText(partNumber);
  await expect(page.getByRole("radio", { name: "选择板子 自制主板" })).toBeChecked();
});
