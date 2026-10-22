import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 2 : 3,
  reporter: "list",
  timeout: 45_000,
  expect: { timeout: 10_000 },
  use: {
    baseURL: "http://127.0.0.1:5173",
    browserName: "chromium",
    hasTouch: true,
    launchOptions: {
      args: ["--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
    },
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    { name: "phone-320", use: { viewport: { width: 320, height: 720 } } },
    { name: "phone-390", use: { viewport: { width: 390, height: 844 } } },
    { name: "tablet-768", use: { viewport: { width: 768, height: 1024 } } },
    { name: "desktop-1024", use: { viewport: { width: 1024, height: 768 } } },
    { name: "wide-1440", use: { viewport: { width: 1440, height: 900 } } },
    { name: "landscape", use: { viewport: { width: 844, height: 390 } } },
  ],
  webServer: {
    command: "pnpm --filter @oshwhub/desktop dev --host 127.0.0.1",
    url: "http://127.0.0.1:5173",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
