import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const app = join(root, "apps/nsh");

// Compile the pinned crate's Java facade with the app's small Activity/component.
export function androidRuntime() {
  const sdk =
    process.env.ANDROID_HOME ||
    process.env.ANDROID_SDK_ROOT ||
    (process.platform === "win32"
      ? join(process.env.LOCALAPPDATA || "", "Android/Sdk")
      : join(
          process.env.HOME || "",
          process.platform === "darwin" ? "Library/Android/sdk" : "Android/Sdk",
        ));
  const androidJar = join(sdk, "platforms/android-35/android.jar");
  assert.ok(existsSync(androidJar), "Install Android SDK platform 35");
  const versions = readdirSync(join(sdk, "build-tools"))
    .filter((version) => /^\d+\.\d+\.\d+$/.test(version))
    .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
  assert.ok(versions.length, "Install Android SDK build-tools");
  const d8 = join(sdk, "build-tools", versions.at(-1), "lib/d8.jar");
  assert.ok(existsSync(d8), "Android D8 compiler is missing");
  const metadata = JSON.parse(
    execFileSync(
      "cargo",
      [
        "metadata",
        "--format-version",
        "1",
        "--locked",
        "--filter-platform",
        "aarch64-linux-android",
      ],
      {
        cwd: root,
        encoding: "utf8",
        maxBuffer: 16 * 1024 * 1024,
      },
    ),
  );
  const platform = metadata.packages.find(
    (item) => item.name === "nsh-platform-android" && item.version === "0.4.0",
  );
  assert.ok(platform, "Resolve the pinned nsh-platform-android 0.4.0 crate first");
  const upstream = join(dirname(platform.manifest_path), "java");
  const sources = (directory) =>
    readdirSync(directory, { recursive: true })
      .filter((name) => name.endsWith(".java"))
      .map((name) => join(directory, name));
  const java = [
    ...sources(upstream).filter((name) => !name.endsWith("AppActivity.java")),
    ...sources(join(app, "android")),
  ];
  const output = join(app, ".nsh/android-host");
  const classes = join(output, "classes");
  const runtime = join(output, "runtime");
  const dex = join(runtime, "android/release");
  rmSync(classes, { recursive: true, force: true });
  mkdirSync(classes, { recursive: true });
  mkdirSync(dex, { recursive: true });
  for (const abi of ["arm64-v8a", "armeabi-v7a", "x86_64"])
    mkdirSync(join(dex, "lib", abi), { recursive: true });
  const tool = (name) =>
    process.env.JAVA_HOME
      ? join(process.env.JAVA_HOME, "bin", name + (process.platform === "win32" ? ".exe" : ""))
      : name;
  const run = (command, args) => execFileSync(command, args, { cwd: root, stdio: "inherit" });
  run(tool("javac"), [
    "--release",
    "17",
    "-encoding",
    "UTF-8",
    "-classpath",
    androidJar,
    "-d",
    classes,
    ...java,
  ]);
  const jar = join(output, "classes.jar");
  run(tool("jar"), ["cf", jar, "-C", classes, "."]);
  run(tool("java"), [
    "-cp",
    d8,
    "com.android.tools.r8.D8",
    "--min-api",
    "26",
    "--lib",
    androidJar,
    "--output",
    dex,
    jar,
  ]);
  return runtime;
}
