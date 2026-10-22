import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export function targetPlan(platform) {
  if (platform === "android") {
    return [
      { triple: "aarch64-linux-android", abi: "arm64-v8a" },
      { triple: "x86_64-linux-android", abi: "x86_64" },
    ];
  }
  if (platform === "ios" || platform === "ios-sim") {
    return [
      { triple: "aarch64-apple-ios" },
      { triple: "aarch64-apple-ios-sim" },
      { triple: "x86_64-apple-ios" },
    ];
  }
  throw new Error(`Unsupported platform: ${platform}`);
}

function run(command, args, env = process.env) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit", env });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited with status ${result.status}`);
}

export function androidTools(ndkRoot, triple, platform = process.platform) {
  const host = { darwin: "darwin-x86_64", linux: "linux-x86_64", win32: "windows-x86_64" }[
    platform
  ];
  if (!host) throw new Error(`Unsupported NDK host: ${platform}`);
  const bin = join(ndkRoot, "toolchains/llvm/prebuilt", host, "bin");
  return {
    cc: join(bin, `${triple}24-clang${platform === "win32" ? ".cmd" : ""}`),
    ar: join(bin, `llvm-ar${platform === "win32" ? ".exe" : ""}`),
  };
}

function androidEnvironment(triple) {
  const defaultSdk =
    process.platform === "win32"
      ? join(process.env.LOCALAPPDATA || "", "Android/Sdk")
      : join(
          process.env.HOME || "",
          process.platform === "darwin" ? "Library/Android/sdk" : "Android/Sdk",
        );
  const sdkRoot = process.env.ANDROID_HOME || process.env.ANDROID_SDK_ROOT || defaultSdk;
  const ndkBase = join(sdkRoot, "ndk");
  const ndkVersion = existsSync(ndkBase)
    ? readdirSync(ndkBase)
        .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }))
        .at(-1)
    : null;
  const ndkRoot = process.env.ANDROID_NDK_HOME || (ndkVersion && join(ndkBase, ndkVersion));
  if (!ndkRoot)
    throw new Error("Install an Android NDK and set ANDROID_HOME (see docs/development.md)");
  const { cc, ar } = androidTools(ndkRoot, triple);
  if (!existsSync(cc)) throw new Error(`Android clang not found: ${cc}`);
  const key = triple.replaceAll("-", "_");
  return {
    ...process.env,
    [`CARGO_TARGET_${key.toUpperCase()}_LINKER`]: cc,
    [`CARGO_TARGET_${key.toUpperCase()}_RUSTFLAGS`]: "-C link-arg=-Wl,-z,max-page-size=16384",
    [`CC_${key}`]: cc,
    [`AR_${key}`]: ar,
  };
}

export function build(platform) {
  const plan = targetPlan(platform);
  const targetDir = resolve(root, process.env.CARGO_TARGET_DIR || "target");
  for (const item of plan) {
    console.info(`rust.build target=${item.triple}`);
    run(
      "cargo",
      ["build", "--locked", "--release", "-p", "oshwhub-ffi", "--target", item.triple],
      platform === "android" ? androidEnvironment(item.triple) : process.env,
    );
    if (item.abi) {
      const destination = join(root, "modules/oshwhub-core/android/src/main/jniLibs", item.abi);
      mkdirSync(destination, { recursive: true });
      copyFileSync(
        join(targetDir, item.triple, "release/liboshwhub_ffi.so"),
        join(destination, "liboshwhub_ffi.so"),
      );
    }
  }
  if (platform === "ios" || platform === "ios-sim") {
    const temporary = join(targetDir, "ios-bridge");
    const headers = join(temporary, "headers");
    mkdirSync(headers, { recursive: true });
    copyFileSync(
      join(root, "modules/oshwhub-core/ios/OshwhubCore.h"),
      join(headers, "OshwhubCore.h"),
    );
    const simulator = join(temporary, "liboshwhub_ffi.a");
    run("lipo", [
      "-create",
      join(targetDir, "aarch64-apple-ios-sim/release/liboshwhub_ffi.a"),
      join(targetDir, "x86_64-apple-ios/release/liboshwhub_ffi.a"),
      "-output",
      simulator,
    ]);
    const framework = join(root, "modules/oshwhub-core/ios/OshwhubRust.xcframework");
    rmSync(framework, { recursive: true, force: true });
    run("xcodebuild", [
      "-create-xcframework",
      "-library",
      join(targetDir, "aarch64-apple-ios/release/liboshwhub_ffi.a"),
      "-headers",
      headers,
      "-library",
      simulator,
      "-headers",
      headers,
      "-output",
      framework,
    ]);
  }
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  try {
    build(process.argv[2]);
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
