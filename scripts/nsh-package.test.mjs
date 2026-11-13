import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { packageTarget } from "./nsh-package.mjs";

test("NSH desktop packages require the matching host", () => {
  for (const [target, host] of [
    ["macos", "darwin"],
    ["windows", "win32"],
    ["linux", "linux"],
  ]) {
    assert.equal(packageTarget(target, host), target);
    for (const other of ["darwin", "win32", "linux"].filter((item) => item !== host)) {
      assert.throws(() => packageTarget(target, other), /matching host/);
    }
  }
});

test("iOS packaging requires macOS and portable targets remain available", () => {
  for (const target of ["ios", "ios-simulator"]) {
    assert.equal(packageTarget(target, "darwin"), target);
    assert.throws(() => packageTarget(target, "linux"), /matching host/);
  }
  for (const host of ["darwin", "win32", "linux"]) {
    for (const target of ["web", "android"]) assert.equal(packageTarget(target, host), target);
  }
  assert.throws(() => packageTarget("gpui", "darwin"), /Unsupported/);
});

test("CLI rejects a cross-host package before invoking NSH", () => {
  const target = process.platform === "win32" ? "linux" : "windows";
  const result = spawnSync(process.execPath, ["scripts/nsh-package.mjs", target], {
    encoding: "utf8",
    env: { ...process.env, NSH_BIN: "/missing/nsh" },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /matching host/);
  assert.doesNotMatch(result.stderr, /ENOENT/);
});
