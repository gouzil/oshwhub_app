import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export function hostTarget(platform = process.platform, arch = process.arch) {
  const cpu = { arm64: "aarch64", x64: "x86_64" }[arch];
  const os = { darwin: "apple-darwin", win32: "pc-windows-msvc", linux: "unknown-linux-gnu" }[
    platform
  ];
  if (!cpu || !os) throw new Error(`Unsupported desktop host ${platform}/${arch}`);
  return `${cpu}-${os}`;
}

export function assertHostBuild(args) {
  hostTarget();
  const permitted = new Set(["--dir", "--mac", "--win", "--linux", "--x64", "--arm64"]);
  const platformFlag = { darwin: "--mac", win32: "--win", linux: "--linux" }[process.platform];
  for (const arg of args) {
    if (
      !permitted.has(arg) ||
      (["--mac", "--win", "--linux"].includes(arg) && arg !== platformFlag) ||
      (["--x64", "--arm64"].includes(arg) && arg !== `--${process.arch}`)
    ) {
      throw new Error(`Desktop packaging must run on the target host and architecture: ${arg}`);
    }
  }
}

export function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: root,
      stdio: "inherit",
      shell: process.platform === "win32",
      ...options,
    });
    child.once("error", reject);
    child.once("exit", (code, signal) =>
      code === 0 ? resolve() : reject(new Error(`${command} exited ${code ?? signal}`)),
    );
  });
}

export async function buildCore(release = true) {
  const target = hostTarget();
  await run("cargo", [
    "build",
    "-p",
    "oshwhub-desktop-core",
    "--target",
    target,
    ...(release ? ["--release"] : []),
  ]);
  const name = `oshwhub-desktop-core${process.platform === "win32" ? ".exe" : ""}`;
  const directory = path.join(
    root,
    "apps/desktop/electron/bin",
    `${process.platform}-${process.arch}`,
  );
  await mkdir(directory, { recursive: true });
  const targetDirectory = process.env.CARGO_TARGET_DIR
    ? path.resolve(root, process.env.CARGO_TARGET_DIR)
    : path.join(root, "target");
  const binary = path.join(directory, name);
  await copyFile(path.join(targetDirectory, target, release ? "release" : "debug", name), binary);
  const sha256 = createHash("sha256")
    .update(await readFile(binary))
    .digest("hex");
  await writeFile(
    path.join(directory, "target.json"),
    JSON.stringify({ platform: process.platform, arch: process.arch, target, sha256 }),
  );
}

async function main() {
  const args = process.argv.slice(2);
  assertHostBuild(args);
  await run("pnpm", ["--filter", "@oshwhub/desktop", "build"]);
  await buildCore();
  await run("pnpm", [
    "--filter",
    "@oshwhub/desktop",
    "exec",
    "electron-builder",
    "--config",
    "electron-builder.config.cjs",
    "--publish",
    "never",
    ...args,
  ]);
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
