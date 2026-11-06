import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  fullyParallel: false,
  workers: 1,
  timeout: 30_000,
  reporter: "list",
  use: {
    browserName: "chromium",
    baseURL: "http://127.0.0.1:5374",
    trace: "retain-on-failure",
    launchOptions: { args: ["--use-gl=angle", "--use-angle=metal"] },
  },
  webServer: {
    command: "python3 -m http.server 5374 --bind 127.0.0.1 --directory output/nsh-web-tests",
    cwd: "../..",
    url: "http://127.0.0.1:5374",
    reuseExistingServer: false,
  },
});
