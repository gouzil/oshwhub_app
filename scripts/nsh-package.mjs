import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { androidRuntime } from "./nsh-android-host.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export function packageTarget(target, host = process.platform) {
  const required = {
    macos: "darwin",
    windows: "win32",
    linux: "linux",
    ios: "darwin",
    "ios-simulator": "darwin",
  };
  if (!Object.hasOwn(required, target) && target !== "web" && target !== "android") {
    throw new Error(`Unsupported NSH package target: ${target}`);
  }
  // NSH 0.4.0 otherwise packages a host binary with the requested platform's name.
  if (required[target] && required[target] !== host) {
    throw new Error(
      `NSH ${target} packaging requires the matching host (${required[target]}), current: ${host}`,
    );
  }
  return target;
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  try {
    const target = packageTarget(process.argv[2]);
    assert.equal(process.argv.length, 3, "Usage: node scripts/nsh-package.mjs <target>");
    const nsh = process.env.NSH_BIN || "nsh";
    assert.match(execFileSync(nsh, ["--version"], { encoding: "utf8" }), /\b0\.4\.0\b/);
    const env =
      target === "android" ? { ...process.env, NSH_RUNTIME: androidRuntime() } : process.env;
    const abis = target === "android" ? ["--abi", "arm64-v8a,armeabi-v7a,x86_64"] : [];
    execFileSync(nsh, ["package", "--target", target, "--out", `.nsh/out/${target}`, ...abis], {
      cwd: join(root, "apps/nsh"),
      stdio: "inherit",
      env,
    });
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
