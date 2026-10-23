import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { describe, it } from "node:test";
import { compileFunction } from "node:vm";

const require = createRequire(import.meta.url);
const applePath = require.resolve("@expo/cli/build/src/start/platforms/ios/AppleDeviceManager");
const appleRequire = createRequire(applePath);
const { PlatformManager } = require("@expo/cli/build/src/start/platforms/PlatformManager");
const url = "exp+oshwhub://?__expo_url=http%3A%2F%2Flocalhost%3A8081";
const denied = Object.assign(new Error("osascript exited with non-zero code: 1"), {
  stderr: "execution error: 未获得授权将Apple事件发送给System Events。 (-1743)",
});

function launcher({ checkError, focusError, launchError } = {}) {
  const warnings = [];
  const opened = [];
  const dependencies = {
    "./ensureSimulatorAppRunning": {
      async ensureSimulatorAppRunningAsync() {
        if (checkError) throw checkError;
      },
    },
    "@expo/osascript": {
      async spawnAsync() {
        if (focusError) throw focusError;
      },
    },
    "../../../utils/interactive": { isInteractive: () => true },
    "../../../log": { warn: (message) => warnings.push(message) },
    "./simctl": {
      getContainerPathAsync: async () => "/installed/OSHWHub.app",
      async openUrlAsync(device, options) {
        if (launchError) throw launchError;
        opened.push({ device, ...options });
      },
    },
  };
  const exports = {};
  compileFunction(readFileSync(applePath, "utf8"), ["require", "exports"], { filename: applePath })(
    (id) => dependencies[id] ?? appleRequire(id),
    exports,
  );
  const device = new exports.AppleDeviceManager({ udid: "test-iphone", name: "iPhone" });
  const platform = new PlatformManager(process.cwd(), {
    platform: "ios",
    resolveDeviceAsync: async () => device,
    getCustomRuntimeUrl: () => url,
  });
  return {
    open: () =>
      platform.openProjectInCustomRuntimeAsync({}, { applicationId: "com.gouzil.oshwhub" }),
    warnings,
    opened,
  };
}

describe("Expo iOS launch with macOS automation restrictions", () => {
  for (const phase of ["checkError", "focusError"]) {
    it(`opens the installed app when ${phase} denies Apple events`, async () => {
      const app = launcher({ [phase]: denied });
      assert.deepEqual(await app.open(), { url });
      assert.equal(app.opened[0].url, url);
      assert.equal(app.opened[0].appId, "com.gouzil.oshwhub");
      assert.equal(app.warnings.length, 1);
    });
  }

  it("keeps the regular launch path when automation is allowed", async () => {
    const app = launcher();
    assert.deepEqual(await app.open(), { url });
    assert.equal(app.warnings.length, 0);
  });

  it("preserves unexpected simulator errors", async () => {
    const error = new Error("Simulator didn't boot fast enough");
    const app = launcher({ checkError: error });
    await assert.rejects(app.open, (actual) => actual === error);
    assert.equal(app.opened.length, 0);
  });

  it("preserves actual app launch failures after denied automation", async () => {
    const error = new Error("simctl openurl failed");
    const app = launcher({ checkError: denied, launchError: error });
    await assert.rejects(app.open, (actual) => actual === error);
  });
});
