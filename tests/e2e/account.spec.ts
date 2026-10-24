import { expect, test } from "./fixtures";

test("guest account opens official Passport and callback updates the user", async ({
  page,
  bridge,
}) => {
  await page.addInitScript(() => {
    const original = window.open;
    window.open = (url, ...args) => {
      (window as unknown as { __openedExternal?: string }).__openedExternal = String(url);
      return original.call(window, url, ...args);
    };
  });
  await page.goto("/");
  await page.getByRole("tab", { name: "账户", exact: true }).click();
  await expect(page.getByText("登录 OSHWHub", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "登录", exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(() => (window as unknown as { __openedExternal?: string }).__openedExternal),
    )
    .toContain("https://passport.jlc.com/login?appId=JLC_OSHWHUB");

  await page.evaluate(() =>
    window.dispatchEvent(
      new CustomEvent("oshwhub:auth-callback", {
        detail: "oshwhub://auth/callback?code=e2e-code",
      }),
    ),
  );
  await expect(page.getByText("测试用户", { exact: true })).toBeVisible();
  expect(bridge.requests).toContainEqual({ command: "login", code: "e2e-code" });

  await page.getByRole("button", { name: "退出登录", exact: true }).click();
  await expect(page.getByText("登录 OSHWHub", { exact: true })).toBeVisible();
  expect(bridge.requests).toContainEqual({ command: "logout" });
});

test("login failures keep the request id without displaying the callback code", async ({
  page,
  bridge,
}) => {
  bridge.authError = {
    code: "AUTH_REQUIRED",
    message: "登录授权码无效或已过期",
    requestId: "login-e2e-request",
  };
  await page.goto("/");
  await page.getByRole("tab", { name: "账户", exact: true }).click();
  await page.evaluate(() =>
    window.dispatchEvent(
      new CustomEvent("oshwhub:auth-callback", {
        detail: "oshwhub://auth/callback?code=secret-e2e-code",
      }),
    ),
  );
  await expect(page.getByRole("alert")).toContainText("login-e2e-request");
  await expect(page.getByRole("alert")).not.toContainText("secret-e2e-code");
});
